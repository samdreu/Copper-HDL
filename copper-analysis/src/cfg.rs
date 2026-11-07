//! The intra-module **control-flow graph** (CFG) — item 2 of
//! `design_docs/SYNCHRONOUS_SEMANTICS_IMPL_PLAN.md`, the analysis both the sim
//! macro and the transpiler consume (c2, gate G6).
//!
//! A `#[hardware(sequential)]` module is an `async fn` whose body is a top-level
//! `loop`. This module models that loop as a real CFG over the `syn` AST and runs
//! two analyses on it:
//!
//! * **Backward liveness → register inference** ([`Cfg::registers`]). A local is a
//!   flip-flop iff (a) it is *defined inside the loop* and (b) its value is *live
//!   across a clock edge* — the pre-tick value is read post-tick. This is the T1
//!   answer (the synthesizable register set computed from control flow, not read
//!   off rustc's over-capturing `Future` layout), and it generalizes the G6 slice's
//!   minimal "pre-loop binding reassigned in loop" criterion to registers *born
//!   inside* the loop and live across an interior `.await` (e.g. `mac_pipeline`'s
//!   pipeline registers).
//!
//! * **Reachability well-formedness** ([`Cfg::check_reachability`]). Copper's core
//!   invariant — *every path through a hardware loop must eventually reach a tick*
//!   — is not checked anywhere today; it holds only as an accident of the
//!   single-trailing-tick construction. Here it is a real analysis: delete every
//!   tick edge and the reachable subgraph must be acyclic (a DFS back-edge check).
//!   A tickless cycle is a zero-time combinational loop and is rejected.
//!
//! **Edges** are classified [`EdgeKind::Comb`] (same-cycle control flow) or
//! [`EdgeKind::Tick`] (the edge *out of* a `clk.tick().await`, a clock-cycle
//! boundary). Tick edges are labeled with the clock **receiver identity** (the
//! groundwork item 4 needs to tag which domain a boundary belongs to — today's
//! `control_extract.rs` matches ticks by method name only, losing the receiver).
//!
//! **v1 scope.** Straight-line code, statement-position `if`/`else` (incl.
//! `else if`) and `match` arms, and ticks anywhere among them. A genuine
//! basic-block builder for *nested* loops (AST duplication doesn't terminate on
//! back-edges) is the one follow-on phase after v1 lands and is verified — a nested
//! loop is folded here into a single opaque node.

use std::collections::{BTreeMap, BTreeSet};

use proc_macro2::{Span, TokenStream, TokenTree};
use syn::spanned::Spanned;
use syn::visit::Visit;
use syn::{BinOp, Expr, ItemFn, Pat, Stmt};

/// How control reaches the successor of a CFG node.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EdgeKind {
    /// Same clock cycle — ordinary (combinational) control flow.
    Comb,
    /// Crosses a clock edge — the edge *out of* a `clk.tick().await`.
    Tick,
}

/// One **memory-port access site**, as it is spelled in the source.
///
/// The transpiler's memory rules are all about *when* an access happens relative
/// to a clock edge, so they have to be decided where the edges are still visible:
/// on the source. Downstream of `control_extract` they are not — a body whose
/// ticks live inside branches or loops is rewritten into a single-tick `match pc`
/// FSM, which is exactly the shape a segment-splitting check reads as "everything
/// happens in one cycle". See [`Cfg::check_memory_staging`].
#[derive(Clone, PartialEq, Eq, Debug)]
struct MemAccess {
    /// The memory local's name (`mem` in `mem.read_port::<0>()`).
    mem: String,
    /// The port index as written in the turbofish, kept as text: an index that is
    /// not a literal belongs to a generic memory, which the transpiler refuses
    /// downstream anyway, and text keeps the two spellings distinct meanwhile.
    port: String,
    kind: MemAccessKind,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum MemAccessKind {
    /// `read_port::<K>().read(addr)` — drives the port's address bus this cycle.
    StageRead,
    /// `write_port::<K>().write(addr, v)` — drives the write bus this cycle.
    StageWrite,
    /// `read_port::<K>().data()` / `.is_ready()` — observes what the *edge*
    /// produced, so it belongs to a later cycle than the staging that fed it.
    Observe(&'static str),
}

impl MemAccessKind {
    /// The physical bus an access drives, or `None` for an observation (which
    /// drives nothing and so never conflicts).
    fn bus(self) -> Option<bool> {
        match self {
            MemAccessKind::StageRead => Some(true),
            MemAccessKind::StageWrite => Some(false),
            MemAccessKind::Observe(_) => None,
        }
    }
}

/// One CFG node. Kept at single-statement granularity so `defs`/`uses` are clean
/// for the liveness dataflow (use-before-def within a node is handled by the
/// `use ∪ (out − def)` transfer, so `let x = f(x)` keeps `x` live-in correctly).
struct Node {
    /// Variables written here (full binds/assignments only — a partial write
    /// `x[i] = …` is a read-modify-write, so `x` is a *use*, not a killing def).
    defs: BTreeSet<String>,
    /// Variables read here.
    uses: BTreeSet<String>,
    /// True iff this node is a `clk.tick().await` (its out-edge is [`EdgeKind::Tick`]).
    is_tick: bool,
    /// The clock receiver identity of a tick node (e.g. `clk`) — item 4's domain tag.
    tick_clock: Option<String>,
    /// Successors and how control reaches them.
    succs: Vec<(usize, EdgeKind)>,
    /// Combinational output ports (`Out<…>`, not `RegOut`) driven here via
    /// `port.write(…)` — the def/use of *ports* the definite-assignment check keys on.
    writes: BTreeSet<String>,
    /// Memory-port access sites appearing in this node's own expression —
    /// `mem.read_port::<K>().read(a)` / `.data()` / `.is_ready()` and
    /// `mem.write_port::<K>().write(a, v)`. A *folded* nested loop node carries
    /// every access in its whole body (the region is opaque here; the sub-CFG in
    /// `nested_ticking_loops` checks its interior precisely).
    mem: Vec<MemAccess>,
    /// True iff this is a *folded* nested loop node whose `writes` is the
    /// conservative all-outputs over-approximation (not a real single `port.write`).
    /// The multi-write-collapse detector must not read it as an explicit write.
    folded: bool,
    /// The branch nodes (`if` conditions, `match` scrutinees) whose arms this node
    /// sits inside, innermost last — the guards an emitted drive at this node is
    /// nested under. Recorded by the builder (an arm's nodes are exactly the ones
    /// created while its branch is built); the pre-tick alignment rule's
    /// forwarded-guard clause reads it. Empty for a top-level node.
    guards: Vec<usize>,
    /// True iff this is a folded **tick-bearing** nested loop that can nonetheless
    /// be left *without* ever reaching one of its ticks — a `loop`/`while` whose
    /// body can `break` (or fall out of a `while` test) before its first tick.
    ///
    /// The out-edge stays [`EdgeKind::Tick`], because the loop *does* cross a clock
    /// edge on its other paths and every value live across it must still be a
    /// register. This flag is read **only** by
    /// [`check_reachability`](Cfg::check_reachability), which must additionally
    /// consider the zero-tick path: an enclosing loop whose sole clock boundary is
    /// such a nested loop can cycle in zero time, and that is a livelock the
    /// simulator exhibits (measured) while the transpiled FSM would happily run one
    /// cycle per iteration.
    may_exit_without_tick: bool,
    /// Source span, for spanned diagnostics.
    span: Span,
}

impl Node {
    /// An empty node at `span`; combine with struct-update (`..Node::empty(span)`).
    fn empty(span: Span) -> Node {
        Node {
            defs: BTreeSet::new(),
            uses: BTreeSet::new(),
            is_tick: false,
            tick_clock: None,
            succs: Vec::new(),
            writes: BTreeSet::new(),
            mem: Vec::new(),
            folded: false,
            guards: Vec::new(),
            may_exit_without_tick: false,
            span,
        }
    }
}

/// Facts about one clock phase (Comb-connected component), for the
/// cycle-dataflow derivation table. See [`Cfg::derivation_facts`].
#[derive(Clone, Debug)]
pub struct PhaseFacts {
    /// This is the head phase (which the trailing segment merges into).
    pub is_head: bool,
    /// Some node in the phase assigns a register.
    pub assigns_registers: bool,
    /// Some node in the phase reads an `In` port.
    pub has_input_read: bool,
    /// An `In` read comb-reaches a commit site in this phase — the first-cut
    /// evidence the phase is **closing-anchored** under the model (path-based
    /// over-approximation of value dependence). Commit sites are register
    /// assignments **and memory stagings**: `mem.read_port().read(a)` /
    /// `write_port().write(a, v)` latch their operands at the closing edge exactly
    /// as a register does.
    pub input_reaches_commit: bool,
    /// Some node in the phase stages a memory access (drives an address/write bus
    /// that latches at the closing edge).
    pub stages_memory: bool,
    /// Some `In`-reading node in the phase branches (≥ 2 comb successors) — the
    /// read steers control, and in a ticking module which suspension point comes
    /// next is a commit of the **implicit state register** (`pc`), which the CFG
    /// does not carry as a `def`. Closing-anchor evidence on the same footing as
    /// `input_reaches_commit`; it is what keeps `det_010_awaits`-style condition
    /// reads anchored to their edge (the D2 rule's condition-position exclusion,
    /// restated).
    pub control_input_read: bool,
    /// Plain combinational `Out` ports written in this phase (`RegOut` excluded,
    /// as everywhere in `Node::writes`).
    pub plain_out_writes: BTreeSet<String>,
    /// The subset of `plain_out_writes` whose write reads a register assigned
    /// earlier in the same phase — the forwarding delta is observable, so an
    /// opening-anchored phase's emitted SV changes under the model.
    pub forwarded_observable: BTreeSet<String>,
    /// The subset of `plain_out_writes` whose operand value flows from a
    /// same-cycle `In` read **through wires** (registers break the taint — a
    /// register read is commit-frontier by definition). These are the writes the
    /// review-row taxonomy cannot clear as frontier-expressible mechanically:
    /// the dual-anchor read question (table §8 item 1) applies, per-read.
    pub input_fed_writes: BTreeSet<String>,
}

/// Per-module facts for the cycle-dataflow derivation table.
/// See [`Cfg::derivation_facts`].
#[derive(Clone, Debug)]
pub struct DerivationFacts {
    pub registers: Vec<String>,
    /// Direct `clk.tick().await` nodes in the top-level loop (folded nested loops
    /// count as one node however many ticks they contain).
    pub tick_nodes: usize,
    /// One iteration crosses more than one clock edge (counts folded tick-bearing
    /// nested loops as more than one, unlike the phase count).
    pub multi_tick: bool,
    pub phases: Vec<PhaseFacts>,
}

/// The control-flow graph of a module's top-level `loop`.
pub struct Cfg {
    nodes: Vec<Node>,
    /// The loop-head node (empty; the DFS/liveness entry). Every "fall off the end
    /// of the body" and every trailing tick routes back here.
    head: usize,
    /// Register *candidates*: locals with a def site (let-binding or assignment)
    /// **inside** the loop. Pre-loop-only bindings (constants/wires like `lfsr`'s
    /// `xor_mask`) are excluded here, which is what stops them from being counted
    /// as registers even though they are live across ticks.
    defined_in_loop: BTreeSet<String>,
    /// The module's **combinational** output ports (`Out<…>`, excluding `RegOut`),
    /// from the signature — the ports the definite-assignment check requires to be
    /// driven on all paths (or none) per cycle.
    comb_outputs: BTreeSet<String>,
    /// The module's `In<…>` input ports, from the signature. Used by
    /// [`multi_write_collapse`](Self::multi_write_collapse) to spot a leading
    /// (deferred) port read before a straddling output write.
    inputs: BTreeSet<String>,
    /// The exit sink for a **combinational** body (loop-free): the single point all
    /// paths reach, where definite-assignment is checked. `None` for a sequential
    /// loop (definite-assignment does not apply — a sequential `Out` legitimately
    /// *holds* when unwritten, i.e. is an enabled register).
    exit: Option<usize>,
    /// Sub-CFGs of tick-containing nested loops, checked recursively by
    /// [`check_reachability`](Self::check_reachability). In the *parent* graph the
    /// nested loop stays a single conservatively-folded node (so its possible
    /// 0-iteration exit never false-rejects the outer loop); its own body's
    /// tickless-cycle well-formedness is enforced here instead.
    nested_ticking_loops: Vec<Cfg>,
    /// The register set, computed once. The backward-liveness fixed point behind
    /// [`Cfg::registers`] is the expensive part of every analysis on this graph
    /// (≈375 ms on the transpilable CPU, measured 2026-09-03 against 0.8 ms to
    /// build the graph), and five rules plus the read classifier each ask for it.
    /// The graph is immutable once built, so the answer cannot change.
    registers_cache: std::cell::OnceCell<Vec<String>>,
    /// The **register graph**: the same loop body with every nested loop built as
    /// real control flow (`Builder::expand_loops`), so a nested loop's interior
    /// ticks are tick nodes and its interior assignments kill. [`Cfg::registers`]
    /// is computed on it; every other rule reads `nodes`, where a nested loop is one
    /// folded node. Empty for a sub-CFG and a combinational body (no register set).
    reg_nodes: Vec<Node>,
}

impl Cfg {
    /// Build the CFG of `f`'s top-level `loop`, or `None` if `f` has no top-level
    /// loop (nothing sequential to analyze).
    pub fn build(f: &ItemFn) -> Option<Cfg> {
        let (loop_body, loop_span) = top_level_loop(f)?;

        let mut defined_in_loop = BTreeSet::new();
        let mut d = DefinedInLoop { set: &mut defined_in_loop };
        for stmt in &loop_body {
            d.visit_stmt(stmt);
        }

        let comb_outputs = combinational_outputs(f);

        let mut b = Builder::new(comb_outputs.clone(), registered_outputs(f), in_param_names(f), memory_locals(f));
        // The head is node 0: an empty node whose successor is the first body node.
        // Every back-edge (trailing tick, fall-through) targets it.
        let head = b.new_node(Node::empty(loop_span));
        // A `continue` written directly in the hardware loop targets this head.
        b.top_head = Some(head);
        let body_entry = b.build_block(&loop_body, head);
        b.nodes[head].succs.push((body_entry, EdgeKind::Comb));

        // The register graph: the same body, nested loops descended.
        let mut rb = Builder::new(BTreeSet::new(), BTreeSet::new(), BTreeSet::new(), memory_locals(f));
        rb.expand_loops = true;
        let rhead = rb.new_node(Node::empty(loop_span));
        rb.top_head = Some(rhead);
        let rentry = rb.build_block(&loop_body, rhead);
        rb.nodes[rhead].succs.push((rentry, EdgeKind::Comb));

        Some(Cfg {
            nodes: b.nodes,
            head,
            defined_in_loop,
            comb_outputs,
            inputs: in_param_names(f),
            exit: None,
            nested_ticking_loops: b.nested,
            registers_cache: std::cell::OnceCell::new(),
            reg_nodes: rb.nodes,
        })
    }

    /// Build the CFG of a **combinational** module body (`#[hardware(combinational)]`
    /// — a loop-free `fn` whose outputs are pure combinational functions of its
    /// inputs). Unlike a sequential module there is no clock, no tick, and no state
    /// to hold, so an output left unassigned on some control path is a genuine
    /// **latch** — which [`check_definite_assignment`](Self::check_definite_assignment)
    /// rejects. Structure: `head → body → exit` sink, no back-edge.
    pub fn build_combinational(f: &ItemFn) -> Cfg {
        let comb_outputs = combinational_outputs(f);
        let mut b = Builder::new(comb_outputs.clone(), registered_outputs(f), in_param_names(f), memory_locals(f));
        let span = f.sig.ident.span();
        let exit = b.new_node(Node::empty(span));
        let head = b.build_block(&f.block.stmts, exit);
        Cfg {
            nodes: b.nodes,
            head,
            defined_in_loop: BTreeSet::new(),
            comb_outputs,
            inputs: in_param_names(f),
            exit: Some(exit),
            nested_ticking_loops: b.nested,
            registers_cache: std::cell::OnceCell::new(),
            reg_nodes: Vec::new(),
        }
    }

    /// The inferred **register set** — sorted for stable structural comparison.
    ///
    /// SPEC I.7.1: a local is a register iff it is assigned inside the module loop
    /// and **live at a `tick`** — some assignment precedes the tick and some read
    /// follows it with no assignment between. Computed as `defined_in_loop` ∩ the
    /// union of the live-out sets of the tick nodes of the register graph
    /// (`reg_nodes`), where every nested loop is real control flow, so a nested
    /// loop's interior ticks count like the hardware loop's own (Phase C step 6).
    /// Same-cycle temporaries, killed by a reassignment before any post-tick use,
    /// are excluded.
    ///
    /// **No back-edge clause.** From 2026-08-21 to Phase C step 6 (2026-11-07) the
    /// locals live out of the loop head were added too, so that `sync_2ff`'s `ff2`
    /// (`q.write(ff2); tick; ff2 = ff1; ff1 = d.read()`) was a flop: under the
    /// two-job body that read `ff1` before the edge. Under D-8's one resumption per
    /// cycle such a local is assigned and consumed within one resumption — a wire
    /// (I.7.2(c)); the clause's whole surplus on the corpus was wires (I.7.5), and
    /// the anchored `sync_2ff` is spelled tick-last, where `ff2` is a wire on every
    /// route and `sync_2ff_ref.sv`'s second flop is the `RegOut` `q`.
    ///
    /// The direction of each remaining approximation (I.7.5): the graph is
    /// path-insensitive, so a local is counted when SOME path of the graph carries
    /// it across a tick, whether or not an execution takes that path — over, never
    /// under. A counted `for` whose range is not statically non-empty keeps its
    /// zero-iteration exit for that reason.
    pub fn registers(&self) -> Vec<String> {
        self.registers_cache.get_or_init(|| self.compute_registers()).clone()
    }

    /// The uncached computation behind [`Cfg::registers`].
    fn compute_registers(&self) -> Vec<String> {
        let live_out = self.liveness();
        let mut across_boundary = BTreeSet::new();
        for (i, node) in self.nodes.iter().enumerate() {
            if node.is_tick {
                across_boundary.extend(live_out[i].iter().cloned());
            }
        }
        self.defined_in_loop
            .intersection(&across_boundary)
            .cloned()
            .collect()
    }




    /// True iff `b` is reachable from `a` via comb-only edges (same cycle-region),
    /// not crossing a tick or the loop head.
    fn comb_reaches(&self, a: usize, b: usize) -> bool {
        let mut stack = vec![a];
        let mut seen = vec![false; self.nodes.len()];
        while let Some(n) = stack.pop() {
            if n == b {
                return true;
            }
            if n == self.head || std::mem::replace(&mut seen[n], true) {
                continue;
            }
            for &(s, k) in &self.nodes[n].succs {
                if k == EdgeKind::Comb {
                    stack.push(s);
                }
            }
        }
        false
    }















    /// Does one iteration of the loop cross **more than one clock edge**?
    ///
    /// Two tick nodes qualify. So does a single *folded tick-bearing nested loop*,
    /// which crosses one edge per iteration of its own and is therefore at least as
    /// separating as two plain ticks — `for _ in 0..2 { clk.tick().await; }` is the
    /// canonical instance, and the shape the trailing-segment divergence was measured
    /// on. Counting Comb-components instead would call that body single-phase, since
    /// the fold hides its interior.
    fn crosses_more_than_one_tick(&self) -> bool {
        let ticks = self.nodes.iter().filter(|n| n.is_tick).count();
        ticks >= 2 || self.nodes.iter().any(|n| n.is_tick && n.folded)
    }







    /// Enforce the memory-port rules **on the source**, where the clock edges are
    /// still visible.
    ///
    /// Copper's memory model is that `read`/`write` *stage* a bus during a cycle and
    /// `data`/`is_ready` *observe* what the following clock edge produced. Three
    /// things follow, and each guards a place where the emitted array would silently
    /// disagree with the simulator:
    ///
    /// * **One access per bus per cycle.** A physical port has one address bus. The
    ///   simulator silently keeps the last write of a cycle; hardware cannot.
    /// * **A result is observed after the edge that produces it** — never in the same
    ///   cycle as the staging that feeds it, which would read it a cycle early.
    /// * **A result has a staging somewhere.** Observing a port nothing ever stages
    ///   reads a port that never becomes ready.
    ///
    /// # Why this is not in codegen
    ///
    /// It was, in `chir_lower::validate_memory_usage`, expressed over the *segments*
    /// a loop body splits into at `clk.tick().await`. `control_extract` runs first
    /// and rewrites any body whose ticks live inside branches or loops into a
    /// **single-tick `match pc` FSM** — so the segments it counted are gone, and
    /// every access in the module lands in one of them. Measured: this transpiles
    ///
    /// ```text
    /// loop { rom.read(addr); tick; data.write(rom.data()); }
    /// ```
    ///
    /// and appending `for _ in 0..2 { tick }` — which changes nothing about the
    /// staging — made it fail with *"read before the `clk.tick().await` that produces
    /// it"*. A false positive that made **any** memory design needing control
    /// extraction unwritable. Same class as [`multi_phase_out_write`](Self::multi_phase_out_write):
    /// a check that counts a syntactic feature placed downstream of a pass that
    /// legitimately removes it.
    ///
    /// # What replaces "segment k precedes segment n"
    ///
    /// Reachability over **tick-free** paths, which is the thing segment order was
    /// approximating. An observation is early iff *every* staging of its port can
    /// reach it without crossing a tick edge, and two accesses share a bus cycle iff
    /// one reaches the other the same way. Walking through the loop head is
    /// deliberate: falling off the end of the body and re-entering it costs no
    /// cycle, so the trailing statements and the head run in the same one — which is
    /// also why the segment-index form got that pairing wrong in both directions.
    ///
    /// A **folded** nested loop is opaque here, so its accesses are attributed to the
    /// node as a whole (enough for "a staging exists" and for the tick edge that
    /// separates it from what follows) and its interior is checked precisely in its
    /// own sub-CFG, which is where the ticks inside it are visible.
    pub fn check_memory_staging(&self) -> Result<(), (Span, String)> {
        self.check_memory_staging_inner(true)
    }

    /// `require_staging` is false inside a nested loop's sub-CFG: the staging that
    /// feeds an observation there may well live in the enclosing loop, so only the
    /// *ordering* rules apply. The enclosing CFG sees every access (the folded node
    /// carries the whole body) and answers the existence question for both.
    fn check_memory_staging_inner(&self, require_staging: bool) -> Result<(), (Span, String)> {
        // A tick-free nested loop is unrolled, so an access inside it becomes N
        // accesses on one bus in one cycle. Refused rather than counted as one.
        for node in &self.nodes {
            if node.folded && !node.is_tick && !node.mem.is_empty() {
                let a = &node.mem[0];
                return Err((
                    node.span,
                    format!(
                        "memory `{}` is accessed inside a nested loop with no \
                         `clk.tick().await`; that loop is unrolled, so every iteration's \
                         access lands in the same cycle on one address bus",
                        a.mem
                    ),
                ));
            }
        }

        // One access per bus per cycle. Two accesses conflict iff one can reach the
        // other WITHOUT crossing a clock edge — not merely iff they share a phase.
        // The distinction is the whole difference between a bus conflict and a
        // multiplexer: `rv32i_cpu`'s seven regfile writebacks sit in exclusive
        // `match` arms, no path joins any two of them, and each drives the bus in
        // its own state — which is exactly what the emitted `always_comb` does.
        // Counting them instead reported a design error where there is a mux.
        //
        // A folded tick-bearing loop contributes at most ONE site per bus: its
        // interior may legitimately access the bus once per iteration (separate
        // cycles), and its own sub-CFG is where that is checked. It does still
        // contribute, because its first iteration runs in the cycle that reaches it.
        let mut sites: Vec<(usize, (String, bool, String), Span)> = Vec::new();
        for (i, node) in self.nodes.iter().enumerate() {
            let mut here: BTreeSet<(String, bool, String)> = BTreeSet::new();
            for a in &node.mem {
                let Some(is_read) = a.kind.bus() else { continue };
                let key = (a.mem.clone(), is_read, a.port.clone());
                if node.folded && !here.insert(key.clone()) {
                    continue;
                }
                sites.push((i, key, node.span));
            }
        }
        for (p, (i, key, span)) in sites.iter().enumerate() {
            let conflicting: Vec<&(usize, (String, bool, String), Span)> = sites
                .iter()
                .enumerate()
                .filter(|(q, (j, k, _))| {
                    *q != p
                        && k == key
                        && (self.tick_free_reaches(*i, *j) || self.tick_free_reaches(*j, *i))
                })
                .map(|(_, s)| s)
                .collect();
            if conflicting.is_empty() {
                continue;
            }
            let (mem, is_read, port) = key;
            let n = conflicting.len() + 1;
            return Err((
                *span,
                format!(
                    "memory `{mem}` {} port {port} is accessed {n} times in one cycle; a \
                     physical port has a single address bus. Merge the accesses into one, \
                     selecting the address and data with a conditional",
                    if *is_read { "read" } else { "write" }
                ),
            ));
        }

        // Where each read port is staged, so an observation can ask whether any
        // staging is separated from it by a clock edge. Positions come from THIS
        // graph; existence (below) looks through the folds as well, since a staging
        // inside a nested loop is still a staging.
        let mut staged: BTreeMap<(String, String), Vec<usize>> = BTreeMap::new();
        for (i, node) in self.nodes.iter().enumerate() {
            for a in &node.mem {
                if a.kind == MemAccessKind::StageRead {
                    staged.entry((a.mem.clone(), a.port.clone())).or_default().push(i);
                }
            }
        }
        let anywhere = self.staged_ports();

        for (i, node) in self.nodes.iter().enumerate() {
            for a in &node.mem {
                let MemAccessKind::Observe(what) = a.kind else { continue };
                let (mem, port) = (&a.mem, &a.port);
                let Some(sites) = staged.get(&(mem.clone(), port.clone())) else {
                    if !require_staging || anywhere.contains(&(mem.clone(), port.clone())) {
                        continue;
                    }
                    return Err((
                        node.span,
                        format!(
                            "`{mem}.read_port::<{port}>().{what}()` is read, but nothing stages a \
                             `read()` on that port earlier in the loop — the port never becomes \
                             ready. Stage the address with `read(addr)` before a \
                             `clk.tick().await`, and observe the result after it"
                        ),
                    ));
                };
                // A staging the observation is NOT tick-free-reachable from is one
                // whose result an edge has already produced — the shape the rule wants.
                if sites.iter().any(|&s| !self.tick_free_reaches(s, i)) {
                    continue;
                }
                return Err((
                    node.span,
                    format!(
                        "`{mem}.read_port::<{port}>().{what}()` is read before the \
                         `clk.tick().await` that produces it. The read result appears at the \
                         clock edge, so it must be observed after the tick — stage the address \
                         with `read(addr)` before a `clk.tick().await`, and observe the result \
                         after it"
                    ),
                ));
            }
        }

        for sub in &self.nested_ticking_loops {
            sub.check_memory_staging_inner(false)?;
        }
        Ok(())
    }


    /// How many **clock phases** the source has: the number of Comb-connected
    /// components reachable in the loop, i.e. the number of distinct cycles one
    /// iteration of the body occupies.
    ///
    /// This is the same notion `shir_lower` re-derives downstream by splitting the
    /// LOWERED body at `clk.tick().await` — and the two disagree whenever a pass
    /// between them changes the tick structure, which `control_extract` does by
    /// design. Exposed so that disagreement can be measured rather than argued
    /// about; see `design_docs/TIMING_MODEL_UNIFICATION.md`.
    pub fn clock_phase_count(&self) -> usize {
        let phases = self.comb_phases();
        let reachable: BTreeSet<usize> = (0..self.nodes.len())
            .filter(|&i| self.reaches_from_head(i))
            .map(|i| phases[i])
            .collect();
        reachable.len()
    }

    /// Is `n` reachable from the loop head at all? An unreachable node's phase must
    /// not be counted — `break` sinks in a nested sub-CFG, for instance.
    fn reaches_from_head(&self, n: usize) -> bool {
        let mut stack = vec![self.head];
        let mut seen = vec![false; self.nodes.len()];
        while let Some(x) = stack.pop() {
            if x == n {
                return true;
            }
            if std::mem::replace(&mut seen[x], true) {
                continue;
            }
            for &(s, _) in &self.nodes[x].succs {
                stack.push(s);
            }
        }
        false
    }

    /// Union-find over `Comb` edges: `phases[n]` is the representative of the clock
    /// phase (Comb-connected component) node `n` runs in. A tick edge is the only
    /// thing that separates two of them, and the trailing statements merge with the
    /// head — re-entering the body costs no cycle.
    fn comb_phases(&self) -> Vec<usize> {
        let n = self.nodes.len();
        let mut parent: Vec<usize> = (0..n).collect();
        fn find(p: &mut Vec<usize>, mut x: usize) -> usize {
            while p[x] != x {
                p[x] = p[p[x]];
                x = p[x];
            }
            x
        }
        for i in 0..n {
            let succs: Vec<usize> = self.nodes[i]
                .succs
                .iter()
                .filter(|(_, k)| *k == EdgeKind::Comb)
                .map(|&(s, _)| s)
                .collect();
            for s in succs {
                let (a, b) = (find(&mut parent, i), find(&mut parent, s));
                if a != b {
                    parent[a] = b;
                }
            }
        }
        (0..n).map(|i| find(&mut parent, i)).collect()
    }

    /// The accesses that run in the cycle this (nested-loop) CFG is *entered* in —
    /// those reachable from the head without crossing a tick edge. The enclosing
    /// graph folds this loop into one node and needs exactly these: the rest of the
    /// body is separated from that node's cycle by the loop's own clock edges.
    fn entry_cycle_mem(&self) -> Vec<MemAccess> {
        let mut out = Vec::new();
        for (i, node) in self.nodes.iter().enumerate() {
            if !node.mem.is_empty() && self.tick_free_reaches(self.head, i) {
                out.extend(node.mem.iter().cloned());
            }
        }
        out
    }

    /// Every read port staged **anywhere** in this loop, nested loops included.
    /// Existence does not depend on position, so unlike the ordering rules this one
    /// looks straight through the folds.
    fn staged_ports(&self) -> BTreeSet<(String, String)> {
        let mut out: BTreeSet<(String, String)> = self
            .nodes
            .iter()
            .flat_map(|n| n.mem.iter())
            .filter(|a| a.kind == MemAccessKind::StageRead)
            .map(|a| (a.mem.clone(), a.port.clone()))
            .collect();
        for sub in &self.nested_ticking_loops {
            out.extend(sub.staged_ports());
        }
        out
    }

    /// Is there a path from `a` to `b` that crosses no clock edge — i.e. do they run
    /// in the same cycle, with `a` before `b`? `a == b` counts (one statement, one
    /// cycle). Unlike [`comb_reaches`](Self::comb_reaches) this walks **through** the
    /// loop head, because re-entering the body costs no cycle.
    fn tick_free_reaches(&self, a: usize, b: usize) -> bool {
        let mut stack = vec![a];
        let mut seen = vec![false; self.nodes.len()];
        while let Some(n) = stack.pop() {
            if n == b {
                return true;
            }
            if std::mem::replace(&mut seen[n], true) {
                continue;
            }
            for &(s, k) in &self.nodes[n].succs {
                if k == EdgeKind::Comb {
                    stack.push(s);
                }
            }
        }
        false
    }

    /// Enforce the reachability invariant: with every tick edge deleted, the graph
    /// reachable from the loop head must be acyclic. A remaining cycle is a path
    /// that returns to the top of the loop without ever awaiting a clock tick — a
    /// zero-time combinational loop. Returns the span of the offending node and a
    /// message on violation.
    pub fn check_reachability(&self) -> Result<(), (Span, String)> {
        #[derive(Clone, Copy, PartialEq)]
        enum Color {
            White,
            Gray,
            Black,
        }
        let mut color = vec![Color::White; self.nodes.len()];

        // Iterative DFS (avoid recursion depth limits on large bodies). The stack
        // holds (node, next-successor-index); a Gray successor over a zero-time edge
        // is a back-edge — a tickless cycle.
        //
        // A zero-time edge is a Comb edge, OR the Tick edge of a folded nested loop
        // that can be left before it ever ticks (`may_exit_without_tick`): that
        // loop's zero-tick exit is a real path through the parent, even though the
        // edge stays Tick for liveness.
        let mut stack: Vec<(usize, usize)> = vec![(self.head, 0)];
        color[self.head] = Color::Gray;
        while let Some(&mut (n, ref mut si)) = stack.last_mut() {
            // Advance to the next zero-time successor of n.
            let mut advanced = false;
            while *si < self.nodes[n].succs.len() {
                let (s, kind) = self.nodes[n].succs[*si];
                *si += 1;
                if kind != EdgeKind::Comb && !self.nodes[n].may_exit_without_tick {
                    continue;
                }
                match color[s] {
                    Color::Gray => {
                        // Name the nested loop when one is on the cycle: "add a tick"
                        // is unfollowable advice for a path that already has ticks on
                        // its *other* branches, and the fix is a different one.
                        let culprit = stack
                            .iter()
                            .position(|&(m, _)| m == s)
                            .map(|start| &stack[start..])
                            .unwrap_or(&stack[..])
                            .iter()
                            .find(|&&(m, _)| self.nodes[m].may_exit_without_tick);
                        if let Some(&(m, _)) = culprit {
                            return Err((
                                self.nodes[m].span,
                                "this loop's only clock boundary is a nested loop that can be \
                                 left before it ticks, so the enclosing loop can return to its \
                                 top in zero time — a livelock in simulation, and a loop the \
                                 transpiler cannot give the same meaning in hardware. Give the \
                                 enclosing loop a `clk.tick().await` of its own, or make the \
                                 nested loop one that always ticks (a `for` over a constant \
                                 range, whose body runs before any exit test)"
                                    .to_string(),
                            ));
                        }
                        return Err((
                            self.nodes[n].span,
                            "this control-flow path returns to the top of the loop without \
                             awaiting a clock tick — every path through a `#[hardware]` loop must \
                             reach `clk.tick().await`, otherwise it is a zero-time combinational \
                             cycle"
                                .to_string(),
                        ));
                    }
                    Color::White => {
                        color[s] = Color::Gray;
                        stack.push((s, 0));
                        advanced = true;
                        break;
                    }
                    Color::Black => {}
                }
            }
            if !advanced && stack.last().is_some_and(|&(m, _)| m == n) {
                // Exhausted n's successors.
                color[n] = Color::Black;
                stack.pop();
            }
        }
        // Recurse into each tick-containing nested loop: its body must satisfy the
        // same tickless-cycle invariant (a clocked nested loop whose body can cycle
        // without ticking spins forever). Folded conservatively in *this* graph, so
        // this recursion is where nested-loop malformedness is actually caught.
        for sub in &self.nested_ticking_loops {
            sub.check_reachability()?;
        }
        Ok(())
    }

    /// Enforce **definite assignment** for a **combinational** module's outputs: a
    /// combinational `Out` port must be driven on *all* control paths or *none* —
    /// never some-but-not-all. A partial (conditional) write in a combinational body
    /// infers a **latch** (there is no clock to register it and no prior value to
    /// legitimately hold). This mirrors, at the syntax level and in the sim macro,
    /// the transpiler's own late `check_no_latches` (`vlir_lower.rs`, `any − all`) —
    /// so the sim rejects the latch at compile time instead of only at transpile.
    ///
    /// **Only applies to combinational bodies** (built via [`build_combinational`],
    /// so `exit` is `Some`). For a sequential loop (`exit == None`) this is a no-op:
    /// a sequential `Out` legitimately holds when unwritten (an *enabled register* —
    /// verified `sim ≡ BaseJump` on `bsg_dff_en`), so "assign on all paths" must not
    /// be imposed there.
    ///
    /// Criterion (`MAY − MUST` at the exit): an output written on some path to the
    /// body's exit but not all was partially assigned. Folded nested loops are
    /// treated as driving every output (opaque, never a *false* latch). Outputs
    /// *never* written are not flagged (they may be driven by a submodule).
    ///
    /// [`build_combinational`]: Self::build_combinational
    pub fn check_definite_assignment(&self) -> Result<(), (Span, String)> {
        let Some(exit) = self.exit else {
            return Ok(()); // sequential: `Out` holds when unwritten — not a latch.
        };
        if self.comb_outputs.is_empty() {
            return Ok(());
        }
        let n = self.nodes.len();
        let mut preds: Vec<Vec<(usize, EdgeKind)>> = vec![Vec::new(); n];
        for (i, node) in self.nodes.iter().enumerate() {
            for &(s, kind) in &node.succs {
                preds[s].push((i, kind));
            }
        }

        // MAY-write (union; a tick edge contributes nothing — new cycle).
        let mut may: Vec<BTreeSet<String>> = vec![BTreeSet::new(); n];
        loop {
            let mut changed = false;
            for i in 0..n {
                let mut out = BTreeSet::new();
                for &(p, kind) in &preds[i] {
                    if kind != EdgeKind::Tick {
                        out.extend(may[p].iter().cloned());
                    }
                }
                out.extend(self.nodes[i].writes.iter().cloned());
                if out != may[i] {
                    may[i] = out;
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }

        // MUST-write (intersection; a tick edge contributes ∅; init to TOP).
        let mut must: Vec<BTreeSet<String>> = vec![self.comb_outputs.clone(); n];
        loop {
            let mut changed = false;
            for i in 0..n {
                let mut acc: Option<BTreeSet<String>> = None;
                for &(p, kind) in &preds[i] {
                    let contrib = if kind == EdgeKind::Tick {
                        BTreeSet::new()
                    } else {
                        must[p].clone()
                    };
                    acc = Some(match acc {
                        None => contrib,
                        Some(a) => a.intersection(&contrib).cloned().collect(),
                    });
                }
                let mut out = acc.unwrap_or_default();
                out.extend(self.nodes[i].writes.iter().cloned());
                if out != must[i] {
                    must[i] = out;
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }

        // At the body's exit, an output written on some path but not all was
        // partially assigned — a latch. Report deterministically (sorted BTreeSet).
        for o in &self.comb_outputs {
            if may[exit].contains(o) && !must[exit].contains(o) {
                return Err((
                    self.nodes[exit].span,
                    format!(
                        "would infer a latch: combinational output `{o}` is assigned on some \
                         control paths but not all. Assign `{o}` on every path (add the missing \
                         branch / `_` arm)"
                    ),
                ));
            }
        }
        Ok(())
    }

    /// Backward-liveness fixpoint. Returns `live_out[i]` for every node. Edge kind is
    /// irrelevant to the propagation — a value used after a tick is still live
    /// *across* it, which is exactly what register inference keys on.
    fn liveness(&self) -> Vec<BTreeSet<String>> {
        liveness_of(&self.nodes)
    }

    /// Per-clock-phase facts for the cycle-dataflow derivation table
    /// (`design_docs/CYCLE_DATAFLOW_SEMANTICS.md` phase 1). **Reporting only** — no
    /// rule keys on this; it exists so the table's mechanical columns come from the
    /// same authority the rules read, instead of a hand scan that goes stale.
    ///
    /// A *phase* is a Comb-connected component of the CFG, exactly as
    /// [`multi_phase_out_write`](Self::multi_phase_out_write) counts them (the
    /// trailing segment merges with the head via the fall-through back-edge). Two
    /// deliberate first-cut approximations, refined by hand in the table:
    ///
    /// * `input_reaches_commit` uses comb-**path** reachability (an `In` read
    ///   comb-reaches a register assignment), not value dependence — an
    ///   over-approximation of the model's "some commit *depends on* a same-cycle
    ///   input read" closing-anchor condition.
    /// * reach queries are iteration-local (`comb_reaches`
    ///   stops at the head), so a trailing→head ordering within the merged phase is
    ///   not followed.
    /// * **`RegOut` writes are commits the CFG does not carry** (`Node::writes`
    ///   holds combinational outputs only), so a phase whose only input-dependent
    ///   commit is a `RegOut` write reads as opening-anchored with a stray input
    ///   read — the `match_on_updated_reg` false `(read-retime)`, hand-derived in
    ///   `DERIVATION_TABLE.md` §4b (really closing-anchored; no retiming).
    pub fn derivation_facts(&self) -> DerivationFacts {
        let regs: BTreeSet<String> = self.registers().into_iter().collect();

        // Union-find over Comb edges — the same phase notion multi_phase_out_write
        // uses. A Tick edge is the only separator.
        let n = self.nodes.len();
        let mut parent: Vec<usize> = (0..n).collect();
        fn find(p: &mut Vec<usize>, mut x: usize) -> usize {
            while p[x] != x {
                p[x] = p[p[x]];
                x = p[x];
            }
            x
        }
        for i in 0..n {
            for &(s, k) in &self.nodes[i].succs {
                if k == EdgeKind::Comb {
                    let (a, b) = (find(&mut parent, i), find(&mut parent, s));
                    if a != b {
                        parent[a] = b;
                    }
                }
            }
        }

        let head_root = find(&mut parent, self.head);
        let mut by_root: std::collections::BTreeMap<usize, Vec<usize>> = Default::default();
        for i in 0..n {
            let r = find(&mut parent, i);
            by_root.entry(r).or_default().push(i);
        }

        let mut phases = Vec::new();
        for (root, members) in &by_root {
            let is_head = *root == head_root;
            let mut ph = PhaseFacts {
                is_head,
                assigns_registers: false,
                has_input_read: false,
                input_reaches_commit: false,
                stages_memory: false,
                control_input_read: false,
                plain_out_writes: BTreeSet::new(),
                forwarded_observable: BTreeSet::new(),
                input_fed_writes: BTreeSet::new(),
            };
            // Wire taint: which same-cycle wire names carry an input's value.
            // Registers break the chain — reading one yields the committed
            // (frontier) value however it was computed.
            let tainted: BTreeSet<String> = {
                let mut t: BTreeSet<String> = BTreeSet::new();
                loop {
                    let mut changed = false;
                    for &m in members.iter() {
                        let node = &self.nodes[m];
                        let src_hit = !node.uses.is_disjoint(&self.inputs)
                            || !node.uses.is_disjoint(&t);
                        if src_hit {
                            for d in node.defs.difference(&regs) {
                                changed |= t.insert(d.clone());
                            }
                        }
                    }
                    if !changed {
                        break t;
                    }
                }
            };
            // Commit sites: register assignments and memory stagings — both latch
            // their operands at the closing edge.
            let commit_nodes: Vec<usize> = members
                .iter()
                .copied()
                .filter(|&m| {
                    !self.nodes[m].defs.is_disjoint(&regs)
                        || self.nodes[m].mem.iter().any(|a| a.kind.bus().is_some())
                })
                .collect();
            ph.assigns_registers =
                commit_nodes.iter().any(|&m| !self.nodes[m].defs.is_disjoint(&regs));
            ph.stages_memory = members
                .iter()
                .any(|&m| self.nodes[m].mem.iter().any(|a| a.kind.bus().is_some()));
            for &m in members {
                let node = &self.nodes[m];
                if !node.uses.is_disjoint(&self.inputs) {
                    ph.has_input_read = true;
                    if commit_nodes.iter().any(|&c| self.comb_reaches(m, c)) {
                        ph.input_reaches_commit = true;
                    }
                    let comb_succs =
                        node.succs.iter().filter(|(_, k)| *k == EdgeKind::Comb).count();
                    if comb_succs >= 2 {
                        ph.control_input_read = true;
                    }
                    // A folded tick-bearing loop hides its interior from the
                    // parent: it carries interior `uses` but NO `defs`, so a read
                    // that feeds a commit or steers a branch INSIDE the fold looks
                    // commit-free out here. The uart mid-bit sample does both
                    // (m4, DERIVATION_TABLE.md): in the sub-CFG it reaches
                    // `byte_val`'s update and branches. The precise answer lives in
                    // `nested_ticking_loops`; the honest first cut at this level is
                    // closing evidence, never a retime flag.
                    if node.folded && node.is_tick {
                        ph.input_reaches_commit = true;
                    }
                }
                if node.folded || node.writes.is_empty() {
                    continue;
                }
                for w in &node.writes {
                    ph.plain_out_writes.insert(w.clone());
                    // The write's operand reads a register some comb-preceding node
                    // in the phase assigns: the sim's forwarded value is observable,
                    // so under the model an OPENING-anchored phase's emission must
                    // be the forwarded expression (D1's family).
                    let observable = commit_nodes.iter().any(|&c| {
                        self.comb_reaches(c, m)
                            && !self.nodes[c]
                                .defs
                                .intersection(&regs)
                                .cloned()
                                .collect::<BTreeSet<_>>()
                                .is_disjoint(&node.uses)
                    });
                    if observable {
                        ph.forwarded_observable.insert(w.clone());
                    }
                    if !node.uses.is_disjoint(&self.inputs)
                        || !node.uses.is_disjoint(&tainted)
                    {
                        ph.input_fed_writes.insert(w.clone());
                    }
                }
            }
            // A component with no statements of interest (e.g. a lone tick node) is
            // still reported: phase COUNT is part of the table.
            phases.push(ph);
        }

        DerivationFacts {
            registers: self.registers(),
            tick_nodes: self.nodes.iter().filter(|n| n.is_tick).count(),
            multi_tick: self.crosses_more_than_one_tick(),
            phases,
        }
    }

    /// A minimal FSM report that falls out of the CFG for free (the item-2 byproduct
    /// tracked in `TODO`): the tick count (≈ number of cycle boundaries), the clock
    /// receivers seen, and the inferred registers. Kept intentionally small; a
    /// richer states/transitions dump can grow from the same `nodes` later.
    pub fn fsm_report(&self, module: &str) -> String {
        let ticks = self.nodes.iter().filter(|n| n.is_tick).count();
        let mut clocks: BTreeSet<&str> = BTreeSet::new();
        for n in &self.nodes {
            if let Some(c) = &n.tick_clock {
                clocks.insert(c);
            }
        }
        format!(
            "module {module}: {ticks} tick boundary(ies), clock(s) {:?}, registers {:?}",
            clocks.into_iter().collect::<Vec<_>>(),
            self.registers()
        )
    }
}


/// Backward-liveness fixpoint over a node set. Returns `live_out[i]` for every node.
/// Edge kind is irrelevant to the propagation — a value used after a tick is still
/// live *across* it, which is exactly what register inference keys on.
fn liveness_of(nodes: &[Node]) -> Vec<BTreeSet<String>> {
    let n = nodes.len();

    let mut live_in: Vec<BTreeSet<String>> = vec![BTreeSet::new(); n];
    let mut live_out: Vec<BTreeSet<String>> = vec![BTreeSet::new(); n];
    loop {
        let mut changed = false;
        // Reverse node order speeds convergence (successors tend to have higher
        // indices in this forward-built graph); correctness is order-independent.
        for i in (0..n).rev() {
            let mut out = BTreeSet::new();
            for &(s, _) in &nodes[i].succs {
                out.extend(live_in[s].iter().cloned());
            }
            let mut in_ = nodes[i].uses.clone();
            for v in &out {
                if !nodes[i].defs.contains(v) {
                    in_.insert(v.clone());
                }
            }
            if out != live_out[i] {
                live_out[i] = out;
                changed = true;
            }
            if in_ != live_in[i] {
                live_in[i] = in_;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    live_out
}

// ── CFG construction ────────────────────────────────────────────────────────

struct Builder {
    nodes: Vec<Node>,
    /// The module's combinational output ports, so terminal `port.write(…)` nodes
    /// can be tagged with the port they drive (definite-assignment).
    outputs: BTreeSet<String>,
    /// The module's `RegOut` ports, for [`Node::regout_writes`].
    reg_outputs: BTreeSet<String>,
    /// The module's `In` ports — propagated into nested-loop sub-CFGs so the
    /// multi-write-collapse detector can spot a leading read inside a nested loop.
    inputs: BTreeSet<String>,
    /// The locals bound to a `Memory<…>`, so a `read_port`/`write_port` call can be
    /// told from a same-named method on anything else. Propagated into nested-loop
    /// sub-CFGs for the same reason the port sets are.
    mems: BTreeSet<String>,
    /// Sub-CFGs of **tick-containing nested loops** — built alongside the (still
    /// folded) parent node so [`Cfg::check_reachability`] can recurse into each one
    /// and enforce the tickless-cycle invariant *inside* the nested loop. Only
    /// tick-containing loops are recorded; a tick-free nested loop is combinational
    /// (unrolled) and not subject to the "must reach a tick" rule.
    nested: Vec<Cfg>,
    /// Enclosing-loop targets while building a nested loop body: `(continue, break)`
    /// node indices. `continue`/fall-through routes to the loop head, `break` to the
    /// loop's exit sink — so a `break` before a tick does not read as a tickless
    /// cycle. Empty while building a top-level (hardware) loop, which never breaks.
    loop_ctx: Vec<(usize, usize)>,
    /// The label of each `loop_ctx` entry (`'outer: for …`), parallel to it. Read
    /// only when `expand_loops` is set: the register graph routes a labelled
    /// `break`/`continue` to the loop it names.
    loop_labels: Vec<Option<String>>,
    /// Build the **register graph** (SPEC I.7.1, Phase C step 6): every nested loop
    /// is built as real control flow — header, body, back edge, exit — instead of
    /// one folded node, so its interior `tick`s are tick suspensions of the graph
    /// and its interior assignments kill. Consulted by [`Cfg::registers`] only;
    /// every other rule keeps reading the folded graph.
    expand_loops: bool,
    /// The top-level hardware loop's head, for a `continue` written directly in it.
    ///
    /// `loop_ctx` stays empty there because a hardware loop never `break`s, and a
    /// top-level `continue` was consequently routed to its FALL-THROUGH — modelling
    /// it as "carry on with the next statement", which is the one thing it does not
    /// do. `loop { if c { continue; } clk.tick().await; }` then reached the tick on
    /// every path and passed, while the real program returns to the head having
    /// ticked zero times: a zero-time cycle the simulator livelocks on. Same class
    /// as the nested-loop hole cause K uncovered, and it had to be closed before
    /// codegen could emit a `continue` at all.
    top_head: Option<usize>,
}

impl Builder {
    fn new(
        outputs: BTreeSet<String>,
        reg_outputs: BTreeSet<String>,
        inputs: BTreeSet<String>,
        mems: BTreeSet<String>,
    ) -> Builder {
        Builder {
            nodes: Vec::new(),
            outputs,
            reg_outputs,
            inputs,
            mems,
            nested: Vec::new(),
            loop_ctx: Vec::new(),
            loop_labels: Vec::new(),
            expand_loops: false,
            top_head: None,
        }
    }
}

impl Builder {
    fn new_node(&mut self, node: Node) -> usize {
        self.nodes.push(node);
        self.nodes.len() - 1
    }

    /// The `loop_ctx` index of the nested loop a `break 'l` / `continue 'l` names,
    /// in the register graph only (`None` for an unlabelled jump, a label naming no
    /// nested loop, or the folded graph, which routes every jump innermost).
    fn labelled_ctx(&self, label: Option<&syn::Lifetime>) -> Option<usize> {
        if !self.expand_loops {
            return None;
        }
        let name = label?.ident.to_string();
        self.loop_labels.iter().rposition(|l| l.as_deref() == Some(name.as_str()))
    }

    /// A nested loop as real control flow, for the register graph (SPEC I.7.1;
    /// I.7.5's two fold defects; D-20 step 6). Shapes, `next` the loop's exit:
    ///
    /// ```text
    /// loop  { B }        H → B → H                  (exit only by `break`)
    /// while c { B }      H[uses c] → bind → B → H,  H → next
    /// for p in e { B }   E[uses e] → H | bind,      H → bind[defs p] → B → H,  H → next
    /// ```
    ///
    /// `continue` targets `H`, `break` targets `next`. A `for` whose range is
    /// statically non-empty (both bounds integer constant expressions of the source,
    /// `start < end`, or `<=` for `..=`) enters its first iteration without the
    /// zero-iteration exit, because no execution takes it; any other range keeps
    /// that exit (I.8.4(a): an empty range is zero iterations), which is the safe
    /// direction — a local is then possibly live around the loop, never missed.
    fn build_loop_expanded(&mut self, expr: &Expr, next: usize) -> usize {
        let label = match expr {
            Expr::While(w) => w.label.as_ref(),
            Expr::ForLoop(f) => f.label.as_ref(),
            Expr::Loop(l) => l.label.as_ref(),
            _ => None,
        }
        .map(|l| l.name.ident.to_string());
        let span = expr.span();
        let header = self.new_node(Node::empty(span));
        self.loop_ctx.push((header, next));
        self.loop_labels.push(label);
        let body_entry = self.build_block(loop_body_stmts(expr), header);
        self.loop_ctx.pop();
        self.loop_labels.pop();
        match expr {
            Expr::While(w) => {
                let mut uses = BTreeSet::new();
                collect_reads(&w.cond, &mut uses);
                let mut binds = BTreeSet::new();
                let_bindings(&w.cond, &mut binds);
                let into_body = if binds.is_empty() {
                    body_entry
                } else {
                    self.new_node(Node {
                        defs: binds,
                        succs: vec![(body_entry, EdgeKind::Comb)],
                        ..Node::empty(span)
                    })
                };
                self.nodes[header].uses = uses;
                self.nodes[header].succs = vec![(into_body, EdgeKind::Comb), (next, EdgeKind::Comb)];
                header
            }
            Expr::ForLoop(f) => {
                let mut binds = BTreeSet::new();
                pat_bindings(&f.pat, &mut binds);
                let bind = self.new_node(Node {
                    defs: binds,
                    succs: vec![(body_entry, EdgeKind::Comb)],
                    ..Node::empty(span)
                });
                self.nodes[header].succs = vec![(bind, EdgeKind::Comb), (next, EdgeKind::Comb)];
                let mut uses = BTreeSet::new();
                collect_reads(&f.expr, &mut uses);
                let first = if statically_nonempty_range(&f.expr) { bind } else { header };
                self.new_node(Node {
                    uses,
                    succs: vec![(first, EdgeKind::Comb)],
                    ..Node::empty(span)
                })
            }
            // `loop`: no test, left only by `break`.
            _ => {
                self.nodes[header].succs = vec![(body_entry, EdgeKind::Comb)];
                header
            }
        }
    }

    /// Build a real sub-CFG of a nested loop's body: `head → body → head` with a
    /// dedicated exit sink as the `break` target. Fresh `Builder`, so it collects
    /// its own grandchild nested loops. Used only for reachability of the nested
    /// loop (no register/output analysis), so `defined_in_loop`/`comb_outputs`/
    /// `exit` are empty/`None`.
    fn nested_loop_cfg(&self, body: &[Stmt], span: Span) -> Cfg {
        let mut b = Builder::new(self.outputs.clone(), self.reg_outputs.clone(), self.inputs.clone(), self.mems.clone());
        let head = b.new_node(Node::empty(span));
        let brk = b.new_node(Node::empty(span)); // exit sink for `break`
        b.loop_ctx.push((head, brk));
        let body_entry = b.build_block(body, head); // fall-through / `continue` → head
        b.loop_ctx.pop();
        b.nodes[head].succs.push((body_entry, EdgeKind::Comb));
        Cfg {
            nodes: b.nodes,
            head,
            defined_in_loop: BTreeSet::new(),
            // Port sets propagated from the parent so multi_write_collapse can recurse
            // into this nested loop (reachability, its original purpose, ignores them).
            comb_outputs: self.outputs.clone(),
            inputs: self.inputs.clone(),
            exit: None,
            nested_ticking_loops: b.nested,
            // Not consulted on a sub-CFG (the multi-phase rule runs on the top level).
            registers_cache: std::cell::OnceCell::new(),
            reg_nodes: Vec::new(),
        }
    }

    /// Build a straight-line block, threading `next` as the successor of the last
    /// statement. Processed in reverse so each statement's successor is already
    /// built when the statement is lowered; returns the entry node of the block.
    fn build_block(&mut self, stmts: &[Stmt], next: usize) -> usize {
        let mut cur = next;
        for stmt in stmts.iter().rev() {
            cur = self.build_stmt(stmt, cur);
        }
        cur
    }

    fn build_stmt(&mut self, stmt: &Stmt, next: usize) -> usize {
        match stmt {
            Stmt::Local(local) => {
                let mut defs = BTreeSet::new();
                pat_bindings(&local.pat, &mut defs);
                let mut uses = BTreeSet::new();
                let mut mem = Vec::new();
                if let Some(init) = &local.init {
                    collect_reads(&init.expr, &mut uses);
                    mem = mem_accesses(&init.expr, &self.mems);
                }
                self.terminal(defs, uses, mem, local.span(), next)
            }
            Stmt::Expr(expr, _) => self.build_expr(expr, next),
            Stmt::Macro(m) => {
                let mut uses = BTreeSet::new();
                collect_token_idents(&m.mac.tokens, &mut uses);
                self.terminal(BTreeSet::new(), uses, Vec::new(), m.span(), next)
            }
            Stmt::Item(item) => {
                self.terminal(BTreeSet::new(), BTreeSet::new(), Vec::new(), item.span(), next)
            }
        }
    }

    /// Lower a statement-position expression, expanding `if`/`match`/`tick`
    /// structurally so ticks nested in branches produce real tick edges.
    fn build_expr(&mut self, expr: &Expr, next: usize) -> usize {
        if let Some(clock) = tick_clock(expr) {
            return self.new_node(Node {
                uses: BTreeSet::from([clock.clone()]),
                is_tick: true,
                tick_clock: Some(clock),
                succs: vec![(next, EdgeKind::Tick)],
                ..Node::empty(expr.span())
            });
        }
        match expr {
            Expr::If(ei) => {
                // Every node created while the arms are built is inside them
                // (`next` already exists), so the branch node guards exactly
                // the range `[arms_start, branch)`.
                let arms_start = self.nodes.len();
                let then_entry = self.build_block(&ei.then_branch.stmts, next);
                let else_entry = match &ei.else_branch {
                    Some((_, e)) => self.build_expr(e, next),
                    None => next,
                };
                let mut uses = BTreeSet::new();
                collect_reads(&ei.cond, &mut uses);
                let branch = self.new_node(Node {
                    uses,
                    mem: mem_accesses(&ei.cond, &self.mems),
                    succs: vec![(then_entry, EdgeKind::Comb), (else_entry, EdgeKind::Comb)],
                    ..Node::empty(ei.span())
                });
                for n in arms_start..branch {
                    self.nodes[n].guards.push(branch);
                }
                branch
            }
            Expr::Match(em) => {
                let arms_start = self.nodes.len();
                let mut succs = Vec::with_capacity(em.arms.len());
                for arm in &em.arms {
                    let body_entry = self.build_expr(&arm.body, next);
                    let mut arm_defs = BTreeSet::new();
                    pat_bindings(&arm.pat, &mut arm_defs);
                    let mut arm_uses = BTreeSet::new();
                    let mut arm_mem = Vec::new();
                    if let Some((_, guard)) = &arm.guard {
                        collect_reads(guard, &mut arm_uses);
                        arm_mem = mem_accesses(guard, &self.mems);
                    }
                    // A pattern binding / guard needs its own entry node so its
                    // defs/uses sit on the path into the arm; otherwise route
                    // straight to the arm body.
                    let entry = if arm_defs.is_empty() && arm_uses.is_empty() && arm_mem.is_empty()
                    {
                        body_entry
                    } else {
                        self.new_node(Node {
                            defs: arm_defs,
                            uses: arm_uses,
                            mem: arm_mem,
                            succs: vec![(body_entry, EdgeKind::Comb)],
                            ..Node::empty(arm.pat.span())
                        })
                    };
                    succs.push((entry, EdgeKind::Comb));
                }
                let mut uses = BTreeSet::new();
                collect_reads(&em.expr, &mut uses);
                let branch = self.new_node(Node {
                    uses,
                    mem: mem_accesses(&em.expr, &self.mems),
                    succs,
                    ..Node::empty(em.span())
                });
                // As for `if`: the arms' nodes (pattern/guard entries included)
                // are exactly those created since `arms_start`.
                for n in arms_start..branch {
                    self.nodes[n].guards.push(branch);
                }
                branch
            }
            // A bare block is just its statements.
            Expr::Block(b) => self.build_block(&b.block.stmts, next),
            // `break` / `continue` inside a nested loop body: route to that loop's
            // exit sink / head (from `loop_ctx`), not to the fall-through `next`, so
            // a `break` before a tick is not misread as a tickless cycle. At the top
            // level `loop_ctx` is empty (a hardware loop never breaks) → fall through.
            Expr::Break(b) => {
                let target = match self.labelled_ctx(b.label.as_ref()) {
                    Some(i) => self.loop_ctx[i].1,
                    None => self.loop_ctx.last().map_or(next, |&(_, brk)| brk),
                };
                // `break v` reads `v` (only meaningful in the register graph; the
                // folded graph never carried it and keeps not doing so).
                let mut uses = BTreeSet::new();
                if self.expand_loops {
                    if let Some(v) = &b.expr {
                        collect_reads(v, &mut uses);
                    }
                }
                self.new_node(Node {
                    uses,
                    succs: vec![(target, EdgeKind::Comb)],
                    ..Node::empty(b.span())
                })
            }
            Expr::Continue(c) => {
                let target = match self.labelled_ctx(c.label.as_ref()) {
                    Some(i) => Some(self.loop_ctx[i].0),
                    // A label naming no nested loop names the hardware loop.
                    None if self.expand_loops && c.label.is_some() => self.top_head,
                    None => None,
                };
                let target = target
                    .or_else(|| self.loop_ctx.last().map(|&(cont, _)| cont))
                    .or(self.top_head)
                    .unwrap_or(next);
                self.new_node(Node {
                    succs: vec![(target, EdgeKind::Comb)],
                    ..Node::empty(c.span())
                })
            }
            // A **nested** loop (`for` / `while` / `loop`). In the *parent* graph it
            // stays folded into a single node. If it *contains a tick* its out-edge is
            // a **Tick** edge (a clock boundary for the parent's liveness) and — new in
            // the nested-loop builder — its body is *also* built as a real sub-CFG
            // (`nested`) so the tickless-cycle invariant is enforced *inside* it
            // (recursively, with `break`/`continue` modeled). A tick-free nested loop
            // is combinational (unrolled) and neither ticks nor is checked. `defs`
            // stays empty (don't kill across the opaque region); interior reads → `uses`.
            //
            // The Tick out-edge used to make the fold *unconditionally* optimistic —
            // "a possible 0-iteration exit must not make the outer loop look tickless",
            // so that a design ticking only inside a `for` (`uart_tx`, `rv32i_cpu`)
            // stays well-formed. That is right for a counted `for`, which runs its body
            // and therefore ticks; it is **wrong** for a `loop`/`while` that can break
            // before its first tick, where the zero-tick exit is a real path. An
            // enclosing loop whose only boundary is such a nested loop then cycles in
            // zero time — measured: the simulator livelocks (99.5% CPU, no progress)
            // while the flattened FSM runs one cycle per iteration. `may_exit_without_tick`
            // records that path for `check_reachability` WITHOUT weakening the Tick
            // edge, which liveness still needs (see the field's docs).
            Expr::While(_) | Expr::ForLoop(_) | Expr::Loop(_) if self.expand_loops => {
                self.build_loop_expanded(expr, next)
            }
            Expr::While(_) | Expr::ForLoop(_) | Expr::Loop(_) => {
                let mut uses = BTreeSet::new();
                collect_reads(expr, &mut uses);
                let clock = first_tick_clock(expr);
                let (is_tick, kind) = match &clock {
                    Some(_) => (true, EdgeKind::Tick),
                    None => (false, EdgeKind::Comb),
                };
                // Memory accesses inside the loop. For a TICK-FREE loop the whole
                // body runs in this cycle (and is refused outright by the staging
                // check, since unrolling puts every iteration's access on one bus).
                // For a tick-bearing one only the accesses before its FIRST tick run
                // in the cycle that reaches this node; the rest belong to later
                // cycles and are checked in the sub-CFG, where those ticks are real
                // edges. Attributing all of them here read `rv32i_cpu`'s
                // `read(addr); loop { tick; if is_ready { break } }` — the standard
                // wait — as a same-cycle observation.
                let mut mem = mem_accesses(expr, &self.mems);
                if is_tick {
                    let sub = self.nested_loop_cfg(loop_body_stmts(expr), expr.span());
                    mem = sub.entry_cycle_mem();
                    self.nested.push(sub);
                }
                self.new_node(Node {
                    uses,
                    is_tick,
                    tick_clock: clock,
                    mem,
                    may_exit_without_tick: is_tick && may_exit_without_tick(expr),
                    succs: vec![(next, kind)],
                    // Opaque region: conservatively assume it drives every output
                    // (so the definite-assignment check never *false*-flags a partial
                    // write around a folded loop — it under-reports here, by design).
                    writes: self.outputs.clone(),
                    folded: true,
                    ..Node::empty(expr.span())
                })
            }
            // Assignment / compound-assign / method call / `port.write(..)` / other
            // terminal expression: one node, combinational edge to the continuation.
            _ => {
                let (defs, uses) = terminal_defs_uses(expr);
                let writes: BTreeSet<String> = output_write_target(expr, &self.outputs)
                    .into_iter()
                    .collect();
                self.new_node(Node {
                    defs,
                    uses,
                    mem: mem_accesses(expr, &self.mems),
                    succs: vec![(next, EdgeKind::Comb)],
                    writes,
                    ..Node::empty(expr.span())
                })
            }
        }
    }

    fn terminal(
        &mut self,
        defs: BTreeSet<String>,
        uses: BTreeSet<String>,
        mem: Vec<MemAccess>,
        span: Span,
        next: usize,
    ) -> usize {
        self.new_node(Node {
            defs,
            uses,
            mem,
            succs: vec![(next, EdgeKind::Comb)],
            ..Node::empty(span)
        })
    }
}

// ── def / use extraction ────────────────────────────────────────────────────

/// Every memory-port access site in `expr` (recursively), for the memories in
/// `mems`. Restricting to *known* memory locals is what keeps an unrelated
/// `something.read_port::<0>()` out of the analysis; the set comes from
/// [`memory_locals`].
fn mem_accesses(expr: &Expr, mems: &BTreeSet<String>) -> Vec<MemAccess> {
    struct V<'a> {
        mems: &'a BTreeSet<String>,
        out: Vec<MemAccess>,
    }
    impl<'ast> Visit<'ast> for V<'_> {
        fn visit_expr_method_call(&mut self, mc: &'ast syn::ExprMethodCall) {
            if let Some((mem, port, is_read)) = mem_port_receiver(&mc.receiver, self.mems) {
                let kind = match (is_read, mc.method.to_string().as_str()) {
                    (true, "read") => Some(MemAccessKind::StageRead),
                    (false, "write") => Some(MemAccessKind::StageWrite),
                    (true, "data") => Some(MemAccessKind::Observe("data")),
                    (true, "is_ready") => Some(MemAccessKind::Observe("is_ready")),
                    _ => None,
                };
                if let Some(kind) = kind {
                    self.out.push(MemAccess { mem, port, kind });
                }
            }
            syn::visit::visit_expr_method_call(self, mc);
        }
    }
    if mems.is_empty() {
        return Vec::new();
    }
    let mut v = V { mems, out: Vec::new() };
    v.visit_expr(expr);
    v.out
}

/// If `recv` is `<mem>.read_port::<K>()` / `<mem>.write_port::<K>()` for a known
/// memory, return `(mem, K-as-text, is_read)`.
fn mem_port_receiver(recv: &Expr, mems: &BTreeSet<String>) -> Option<(String, String, bool)> {
    let Expr::MethodCall(mc) = recv else { return None };
    let is_read = match mc.method.to_string().as_str() {
        "read_port" => true,
        "write_port" => false,
        _ => return None,
    };
    if !mc.args.is_empty() {
        return None;
    }
    let mem = simple_ident(&mc.receiver)?;
    if !mems.contains(&mem) {
        return None;
    }
    let arg = mc.turbofish.as_ref()?.args.first()?;
    let port = match arg {
        syn::GenericArgument::Const(Expr::Lit(l)) => match &l.lit {
            syn::Lit::Int(i) => i.base10_digits().to_string(),
            _ => return None,
        },
        syn::GenericArgument::Const(e) => simple_ident(e)?,
        syn::GenericArgument::Type(syn::Type::Path(tp)) => {
            tp.path.get_ident().map(|i| i.to_string())?
        }
        _ => return None,
    };
    Some((mem, port, is_read))
}

/// The names bound to a `Memory<…>` in `f`, in either of its two spellings:
///
/// - **declared** — `let mem = Memory::<…>::new(…)`, including the
///   `.write_first()` / `.read_first()` builder forms. Declared before the
///   hardware loop, so the whole body is scanned.
/// - **received** — a `mem: Memory<…>` parameter, the spelling a module uses when
///   a testbench or a parent owns the storage and hands it over.
///
/// **Both, deliberately.** Every rule downstream of this — the one-access-per-bus
/// -per-cycle rule, the read-is-staged-before-its-tick rule, the unrolled-nested-
/// loop rule — is keyed on the *names* this returns. A received memory that this
/// function did not report would not be diagnosed as unsafe; it would simply stop
/// being examined, and the module would go quiet rather than fail. That is the
/// failure mode the wiring guards in `tools/regression.sh` exist to prevent, and
/// it is why the parameter spelling was taught here in the same change that
/// taught the macro to accept it, rather than afterwards.
fn memory_locals(f: &ItemFn) -> BTreeSet<String> {
    struct V {
        out: BTreeSet<String>,
    }
    impl<'ast> Visit<'ast> for V {
        fn visit_local(&mut self, l: &'ast syn::Local) {
            if let Some(init) = &l.init {
                if mentions_memory_type(&init.expr) {
                    pat_bindings(&l.pat, &mut self.out);
                }
            }
            syn::visit::visit_local(self, l);
        }
    }
    let mut v = V { out: BTreeSet::new() };
    v.visit_block(&f.block);

    // Received memories. The type is matched by its outer name for the same reason
    // the macro's allowlist does: `Memory` may arrive fully qualified
    // (`copper_core::Memory<…>`), and the last path segment is the type either way.
    for arg in &f.sig.inputs {
        let syn::FnArg::Typed(pt) = arg else { continue };
        let syn::Type::Path(tp) = &*pt.ty else { continue };
        if tp.path.segments.last().is_some_and(|s| s.ident == "Memory") {
            pat_bindings(&pt.pat, &mut v.out);
        }
    }

    v.out
}

/// Does `expr` name the `Memory` type anywhere in a path (`Memory::<…>::new`)?
fn mentions_memory_type(expr: &Expr) -> bool {
    struct V {
        found: bool,
    }
    impl<'ast> Visit<'ast> for V {
        fn visit_path(&mut self, p: &'ast syn::Path) {
            if p.segments.iter().any(|s| s.ident == "Memory") {
                self.found = true;
            }
            syn::visit::visit_path(self, p);
        }
    }
    let mut v = V { found: false };
    v.visit_expr(expr);
    v.found
}

/// The `(defs, uses)` of a terminal (non-branch, non-tick) statement expression.
fn terminal_defs_uses(expr: &Expr) -> (BTreeSet<String>, BTreeSet<String>) {
    let mut defs = BTreeSet::new();
    let mut uses = BTreeSet::new();
    match expr {
        Expr::Assign(a) => {
            assign_lhs(&a.left, &mut defs, &mut uses);
            collect_reads(&a.right, &mut uses);
        }
        Expr::Binary(b) if is_assign_op(&b.op) => {
            // Compound assign `x += y`: reads and writes `x`.
            if let Some(id) = simple_ident(&b.left) {
                defs.insert(id.clone());
                uses.insert(id);
            } else {
                collect_reads(&b.left, &mut uses);
            }
            collect_reads(&b.right, &mut uses);
        }
        _ => collect_reads(expr, &mut uses),
    }
    (defs, uses)
}

/// Split an assignment LHS into killing defs vs read-modify-write uses. A bare
/// `x` (or a tuple of them) is a full def; `x[i]`/`x.f` is a partial write, so `x`
/// stays a *use* (it isn't killed) — but see [`DefinedInLoop`], which still counts
/// `x` as assigned-in-loop so a partially-written register is a register candidate.
fn assign_lhs(left: &Expr, defs: &mut BTreeSet<String>, uses: &mut BTreeSet<String>) {
    match left {
        Expr::Path(_) => {
            if let Some(id) = simple_ident(left) {
                defs.insert(id);
            }
        }
        Expr::Tuple(t) => {
            for elem in &t.elems {
                assign_lhs(elem, defs, uses);
            }
        }
        Expr::Index(i) => {
            collect_reads(&i.expr, uses);
            collect_reads(&i.index, uses);
        }
        Expr::Field(f) => collect_reads(&f.base, uses),
        other => collect_reads(other, uses),
    }
}

/// Every variable **read** in `expr` (single-segment path identifiers). Method
/// names, multi-segment paths (enum variants like `State::A`), and struct-literal
/// field names are not idents-as-reads and are skipped; macro token streams are
/// scanned for idents (over-approx, but only affects vars that are also
/// assigned-in-loop, which is rare enough to be harmless in v1).
fn collect_reads(expr: &Expr, out: &mut BTreeSet<String>) {
    struct Reads<'a>(&'a mut BTreeSet<String>);
    impl<'ast> Visit<'ast> for Reads<'_> {
        fn visit_expr_path(&mut self, p: &'ast syn::ExprPath) {
            if p.qself.is_none() && p.path.segments.len() == 1 {
                self.0.insert(p.path.segments[0].ident.to_string());
            }
        }
        fn visit_macro(&mut self, m: &'ast syn::Macro) {
            collect_token_idents(&m.tokens, self.0);
        }
    }
    Reads(out).visit_expr(expr);
}

/// Insert every identifier token appearing in a macro's token stream.
fn collect_token_idents(tokens: &TokenStream, out: &mut BTreeSet<String>) {
    for tt in tokens.clone() {
        match tt {
            TokenTree::Ident(id) => {
                out.insert(id.to_string());
            }
            TokenTree::Group(g) => collect_token_idents(&g.stream(), out),
            _ => {}
        }
    }
}

/// Collect the identifiers bound by a pattern (recursively).
fn pat_bindings(pat: &Pat, out: &mut BTreeSet<String>) {
    match pat {
        Pat::Ident(pi) => {
            out.insert(pi.ident.to_string());
            if let Some((_, sub)) = &pi.subpat {
                pat_bindings(sub, out);
            }
        }
        Pat::Type(pt) => pat_bindings(&pt.pat, out),
        Pat::Reference(r) => pat_bindings(&r.pat, out),
        Pat::Paren(p) => pat_bindings(&p.pat, out),
        Pat::Tuple(t) => t.elems.iter().for_each(|e| pat_bindings(e, out)),
        Pat::TupleStruct(ts) => ts.elems.iter().for_each(|e| pat_bindings(e, out)),
        Pat::Slice(s) => s.elems.iter().for_each(|e| pat_bindings(e, out)),
        Pat::Or(o) => o.cases.iter().for_each(|c| pat_bindings(c, out)),
        Pat::Struct(s) => s.fields.iter().for_each(|f| pat_bindings(&f.pat, out)),
        _ => {}
    }
}

/// Visitor collecting the register **candidates**: every variable with a def site
/// (let-binding or assignment target, including partial-write and tuple targets)
/// anywhere inside the loop body. Deliberately *not* collecting match-arm pattern
/// bindings — those are transient arm-local names, not persistent state.
struct DefinedInLoop<'a> {
    set: &'a mut BTreeSet<String>,
}

impl<'ast> Visit<'ast> for DefinedInLoop<'_> {
    fn visit_local(&mut self, l: &'ast syn::Local) {
        pat_bindings(&l.pat, self.set);
        syn::visit::visit_local(self, l);
    }
    /// A **ticking** `for`'s binding is defined in the loop just as a `let` is, and
    /// it is read on the far side of the tick its own body contains — so it is a
    /// register, and the transpiler builds one for it (control extraction desugars
    /// the counted `for` into a counter-driven `loop`). Without this the shared
    /// inference reported no register for `for i in 0..8 { …; tick; }` while
    /// codegen emitted `i`, and the two front-ends disagreed about the language's
    /// central rule.
    ///
    /// A tick-FREE `for` is combinational and unrolls, so its variable is an
    /// elaboration-time index and not state. `_` binds nothing and contributes
    /// nothing, which is correct: the counter the transpiler synthesizes for it has
    /// no source-level name, exactly like `pc`.
    fn visit_expr_for_loop(&mut self, f: &'ast syn::ExprForLoop) {
        if f.body.stmts.iter().any(stmt_contains_tick) {
            pat_bindings(&f.pat, self.set);
        }
        syn::visit::visit_expr_for_loop(self, f);
    }
    fn visit_expr_assign(&mut self, a: &'ast syn::ExprAssign) {
        assign_targets(&a.left, self.set);
        syn::visit::visit_expr_assign(self, a);
    }
    fn visit_expr_binary(&mut self, b: &'ast syn::ExprBinary) {
        if is_assign_op(&b.op) {
            assign_targets(&b.left, self.set);
        }
        syn::visit::visit_expr_binary(self, b);
    }
}

/// The base variable names written by an assignment LHS — bare, tuple, indexed, or
/// field — so a partially-written register still counts as assigned-in-loop.
fn assign_targets(left: &Expr, out: &mut BTreeSet<String>) {
    match left {
        Expr::Path(_) => {
            if let Some(id) = simple_ident(left) {
                out.insert(id);
            }
        }
        Expr::Tuple(t) => t.elems.iter().for_each(|e| assign_targets(e, out)),
        Expr::Index(i) => assign_targets(&i.expr, out),
        Expr::Field(f) => assign_targets(&f.base, out),
        _ => {}
    }
}

// ── small syntactic helpers ─────────────────────────────────────────────────

/// The module's **combinational** output ports: signature parameters whose type
/// is `Out<…>` (exactly — `RegOut` is a registered output and is excluded, as are
/// `In`/`Clock`/`Memory`). These are the ports the definite-assignment check
/// requires to be driven on all-or-no paths per cycle.
fn combinational_outputs(f: &ItemFn) -> BTreeSet<String> {
    let mut outs = BTreeSet::new();
    for arg in &f.sig.inputs {
        if let syn::FnArg::Typed(pt) = arg {
            if matches!(&*pt.ty, syn::Type::Path(tp)
                if tp.path.segments.last().is_some_and(|s| s.ident == "Out"))
            {
                pat_bindings(&pt.pat, &mut outs);
            }
        }
    }
    outs
}

/// The module's `RegOut<…>` parameter names.
fn registered_outputs(f: &ItemFn) -> BTreeSet<String> {
    let mut outs = BTreeSet::new();
    for arg in &f.sig.inputs {
        if let syn::FnArg::Typed(pt) = arg {
            if matches!(&*pt.ty, syn::Type::Path(tp)
                if tp.path.segments.last().is_some_and(|s| s.ident == "RegOut"))
            {
                pat_bindings(&pt.pat, &mut outs);
            }
        }
    }
    outs
}

/// `Some(port)` iff `expr` is `<port>.write(…)` for a `port` in `outputs`.
fn output_write_target(expr: &Expr, outputs: &BTreeSet<String>) -> Option<String> {
    let Expr::MethodCall(mc) = expr else { return None };
    if mc.method != "write" {
        return None;
    }
    simple_ident(&mc.receiver).filter(|r| outputs.contains(r))
}


/// A **syntactic** constant: a value that cannot differ between two executions of
/// the expression. Literals; a multi-segment path (an enum unit variant such as
/// `Logic::One`, an associated const); a single-segment `SCREAMING_CASE` path (a
/// `const` item or const generic); a call through a multi-segment path whose
/// arguments are all constant (`Bits::zero()`, `Bits::from_lit::<1>()`,
/// `Bits::<8>::from_u8(9)`); parentheses, casts, unary and binary operators
/// over constants; and an array literal or repeat of constants. A method call (`a.read()`), a local, or a macro is not — the test
/// is conservative in the direction of **not** calling something constant, which
/// for [`Cfg::out_first_driven_late`] is the direction of not refusing — and for
/// the register power-on rule (`crate::init`, where it is exported from) the
/// direction of refusing: an initializer this does not call constant has no
/// stated power-on value.
pub fn is_constant_expr(e: &Expr) -> bool {
    match e {
        Expr::Lit(_) => true,
        Expr::Path(p) => {
            if p.qself.is_some() {
                return false;
            }
            if p.path.segments.len() > 1 {
                return true;
            }
            let name = p.path.segments[0].ident.to_string();
            name.starts_with(|c: char| c.is_ascii_uppercase())
                && name.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
        }
        Expr::Call(c) => {
            matches!(&*c.func, Expr::Path(p) if p.qself.is_none() && p.path.segments.len() > 1)
                && c.args.iter().all(is_constant_expr)
        }
        Expr::Paren(p) => is_constant_expr(&p.expr),
        Expr::Group(g) => is_constant_expr(&g.expr),
        Expr::Cast(c) => is_constant_expr(&c.expr),
        Expr::Unary(u) => is_constant_expr(&u.expr),
        Expr::Binary(b) => is_constant_expr(&b.left) && is_constant_expr(&b.right),
        // An array of constants is a constant: `[Logic::Zero; 16]` (a packed
        // shift register's start), `[Bits::<8>::zero(); 8]` (a register file's).
        // Added 2026-10-29 for the register power-on rule; no corpus write operand
        // has this shape, so `out_first_driven_late`'s exact-set pin is unmoved.
        Expr::Repeat(r) => is_constant_expr(&r.expr) && is_constant_expr(&r.len),
        Expr::Array(a) => a.elems.iter().all(is_constant_expr),
        _ => false,
    }
}

/// The statement list and span of the module's top-level `loop { … }`, if any.
fn top_level_loop(f: &ItemFn) -> Option<(Vec<Stmt>, Span)> {
    f.block.stmts.iter().find_map(|s| match s {
        Stmt::Expr(Expr::Loop(l), _) => Some((l.body.stmts.clone(), l.span())),
        _ => None,
    })
}

/// The statement list of a nested `while` / `for` / `loop` body.
/// The names bound by the `let` patterns of a `while let` / let-chain condition.
fn let_bindings(cond: &Expr, out: &mut BTreeSet<String>) {
    match cond {
        Expr::Let(l) => pat_bindings(&l.pat, out),
        Expr::Binary(b) if matches!(b.op, BinOp::And(_)) => {
            let_bindings(&b.left, out);
            let_bindings(&b.right, out);
        }
        Expr::Paren(p) => let_bindings(&p.expr, out),
        _ => {}
    }
}

/// True iff `range` is `a..b` with `a < b`, or `a..=b` with `a <= b`, both bounds
/// integer constant expressions of the source alone (literals and arithmetic over
/// them, [`crate::consts::eval_syn`] with no names) — a `for` over it runs its body
/// at least once on every execution. A bound naming anything (a const item, a
/// generic, a local) is not decided here: the caller keeps the zero-iteration exit.
fn statically_nonempty_range(range: &Expr) -> bool {
    let Expr::Range(r) = range else { return false };
    let (Some(a), Some(b)) = (&r.start, &r.end) else { return false };
    let none = |_: &str| None;
    let (Some(a), Some(b)) = (crate::consts::eval_syn(a, &none), crate::consts::eval_syn(b, &none)) else {
        return false;
    };
    match r.limits {
        syn::RangeLimits::HalfOpen(_) => a < b,
        syn::RangeLimits::Closed(_) => a <= b,
    }
}

fn loop_body_stmts(expr: &Expr) -> &[Stmt] {
    match expr {
        Expr::While(w) => &w.body.stmts,
        Expr::ForLoop(f) => &f.body.stmts,
        Expr::Loop(l) => &l.body.stmts,
        _ => &[],
    }
}

/// Can this **tick-bearing** nested loop be left without ever reaching a tick?
///
/// Only the *zero-tick exit* is asked about here; a loop that can cycle internally
/// without ticking is a different (and separately enforced) defect, caught by
/// `check_reachability` recursing into the loop's own sub-CFG.
///
/// * `loop` — exits only by `break`, so the answer is "yes" unless every top-level
///   path reaches a tick before any `break` can be taken.
/// * `while` — the test can be false on entry, so it always can (a zero-iteration
///   exit needs no `break` at all). This matches what the transpiler sees: a
///   tick-bearing `while` is desugared to `loop { if !cond { break; } … }`, whose
///   leading `break` gives the same answer. The two front-ends agreeing about this
///   is the point — them disagreeing is this pipeline's recurring bug class.
/// * `for` — the same question as `loop`: does every path through the body reach a
///   tick before it can `break`? A counted repetition over a constant range with the
///   tick last (`uart_tx`, `rv32i_cpu`, `for _ in 0..CLKS_PER_BIT { clk.tick().await; }`)
///   answers "no exit without a tick", as before. A body that `break`s BEFORE its
///   tick (`for i in 0..N { if miss { break; } clk.tick().await; }`) answers "yes":
///   the enclosing loop then needs a boundary of its own after the `for`. Until
///   2026-09-02 this arm was a flat `false` ("a `for` runs its body"), and a
///   detector written that way with nothing after the `for` passed the check and
///   hung the simulator; `tests/ui/fail/for_break_before_tick.rs` in copper-macros
///   pins the rejection. What is still not seen: an **empty** range, which needs
///   const evaluation and which this crate (syntax-level, `syn`-only) cannot do.
fn may_exit_without_tick(expr: &Expr) -> bool {
    match expr {
        Expr::ForLoop(f) => !ticks_before_any_break(&f.body.stmts),
        Expr::While(_) => true,
        Expr::Loop(l) => !ticks_before_any_break(&l.body.stmts),
        _ => false,
    }
}

/// Does every top-level path through this loop body reach a tick before it can
/// `break` out of the loop?
///
/// Deliberately conservative: a tick that sits inside a branch (`if c { tick; }`)
/// answers "no", because the other branch reaches the body's end — and thus the
/// loop head — without ticking. Only a tick in *statement* position counts, or a
/// nested loop that is itself a guaranteed boundary (mutually recursive with
/// [`may_exit_without_tick`]; both descend structurally, so this terminates).
fn ticks_before_any_break(body: &[Stmt]) -> bool {
    for stmt in body {
        let Stmt::Expr(e, _) = stmt else {
            // A `let` whose initializer breaks or ticks is not a shape this walk
            // models; treat it as neither, and keep looking.
            continue;
        };
        if tick_clock(e).is_some() {
            return true; // an unconditional clock boundary, reached first
        }
        if matches!(e, Expr::Loop(_) | Expr::While(_) | Expr::ForLoop(_)) {
            // A nested loop that always ticks is itself a boundary: reaching it
            // guarantees a clock edge. Without this clause `loop { <work>; for _ in
            // 0..N { tick; } }` — the shape cause K exists to support — would read
            // as tickless and be false-rejected.
            if first_tick_clock(e).is_some() && !may_exit_without_tick(e) {
                return true;
            }
            return false;
        }
        if breaks_enclosing_loop(e) {
            return false; // an exit is reachable with no tick behind it
        }
        if first_tick_clock(e).is_some() {
            return false; // ticks only on *some* path through this statement
        }
    }
    false // fell out of the body without ticking
}

/// Does `expr` contain a `break` belonging to the loop that encloses it? A `break`
/// under a nested loop of its own belongs to *that* loop and does not count.
fn breaks_enclosing_loop(expr: &Expr) -> bool {
    match expr {
        Expr::Break(_) => true,
        Expr::If(f) => {
            f.then_branch.stmts.iter().any(stmt_breaks_enclosing_loop)
                || f.else_branch.as_ref().is_some_and(|(_, e)| breaks_enclosing_loop(e))
        }
        Expr::Block(b) => b.block.stmts.iter().any(stmt_breaks_enclosing_loop),
        Expr::Match(m) => m.arms.iter().any(|a| breaks_enclosing_loop(&a.body)),
        Expr::Loop(_) | Expr::While(_) | Expr::ForLoop(_) => false,
        _ => false,
    }
}

fn stmt_breaks_enclosing_loop(s: &Stmt) -> bool {
    match s {
        Stmt::Expr(e, _) => breaks_enclosing_loop(e),
        _ => false,
    }
}

/// `Some(clock_name)` iff `expr` is `<clock>.tick().await`. The clock receiver
/// identity is preserved (item 4's per-domain tick tag); a non-simple receiver
/// yields the placeholder `"<clock>"`.
pub(crate) fn tick_clock(expr: &Expr) -> Option<String> {
    let Expr::Await(a) = expr else { return None };
    let Expr::MethodCall(mc) = a.base.as_ref() else {
        return None;
    };
    if mc.method != "tick" || !mc.args.is_empty() {
        return None;
    }
    Some(simple_ident(&mc.receiver).unwrap_or_else(|| "<clock>".to_string()))
}

/// Does this statement issue a `<clock>.tick().await` at any depth? Shares
/// [`first_tick_clock`]'s walk so the two cannot disagree about where a tick can
/// live — a drift that has caused real bugs in the transpiler's own gate.
pub(crate) fn stmt_contains_tick(s: &syn::Stmt) -> bool {
    struct FindTick(bool);
    impl<'ast> Visit<'ast> for FindTick {
        fn visit_expr(&mut self, e: &'ast Expr) {
            if self.0 {
                return;
            }
            if tick_clock(e).is_some() {
                self.0 = true;
                return;
            }
            syn::visit::visit_expr(self, e);
        }
    }
    let mut f = FindTick(false);
    f.visit_stmt(s);
    f.0
}

/// The clock receiver of the first `<clock>.tick().await` anywhere inside `expr`,
/// or `None` if `expr` contains no tick. Used to decide whether a folded nested
/// loop crosses a clock edge (and, for item 4, which domain it belongs to).
fn first_tick_clock(expr: &Expr) -> Option<String> {
    struct FindTick(Option<String>);
    impl<'ast> Visit<'ast> for FindTick {
        fn visit_expr(&mut self, e: &'ast Expr) {
            if self.0.is_some() {
                return;
            }
            if let Some(c) = tick_clock(e) {
                self.0 = Some(c);
                return;
            }
            syn::visit::visit_expr(self, e);
        }
    }
    let mut f = FindTick(None);
    f.visit_expr(expr);
    f.0
}

/// The single identifier of a bare path expression (`x`), else `None`.
fn simple_ident(e: &Expr) -> Option<String> {
    if let Expr::Path(p) = e {
        if p.qself.is_none() && p.path.segments.len() == 1 {
            return Some(p.path.segments[0].ident.to_string());
        }
    }
    None
}

fn is_assign_op(op: &BinOp) -> bool {
    matches!(
        op,
        BinOp::AddAssign(_)
            | BinOp::SubAssign(_)
            | BinOp::MulAssign(_)
            | BinOp::DivAssign(_)
            | BinOp::RemAssign(_)
            | BinOp::BitXorAssign(_)
            | BinOp::BitAndAssign(_)
            | BinOp::BitOrAssign(_)
            | BinOp::ShlAssign(_)
            | BinOp::ShrAssign(_)
    )
}

/// The names of the function's `In<T, D>` parameters (outer type `In`).
fn in_param_names(f: &ItemFn) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    for arg in &f.sig.inputs {
        if let syn::FnArg::Typed(pt) = arg {
            let is_in = matches!(&*pt.ty, syn::Type::Path(tp)
                if tp.path.segments.last().is_some_and(|s| s.ident == "In"));
            if is_in {
                pat_bindings(&pt.pat, &mut names);
            }
        }
    }
    names
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(src: &str) -> ItemFn {
        syn::parse_str(src).expect("parse ItemFn")
    }

    fn regs(src: &str) -> Vec<String> {
        Cfg::build(&parse(src)).map(|c| c.registers()).unwrap_or_default()
    }

    // ── register inference (backward liveness) ──────────────────────────────

    /// `mac_fsm`: four pre-loop locals, all reassigned in the loop and read a
    /// cycle later — the G6-slice shape, still correct under general liveness.
    #[test]
    fn mac_fsm_registers() {
        let src = r#"
            #[hardware(sequential)]
            async fn mac_fsm(clk: Clock<C>, a: In<Bits<8>, C>, b: In<Bits<8>, C>, c: In<Bits<8>, C>, out: RegOut<Bits<8>, C>) {
                let mut stage = Stage::Load;
                let mut product: Bits<8> = Bits::from_lit::<0>();
                let mut c_latch: Bits<8> = Bits::from_lit::<0>();
                let mut result: Bits<8> = Bits::from_lit::<0>();
                loop {
                    match stage {
                        Stage::Load => { product = a.read() * b.read(); c_latch = c.read(); stage = Stage::Mul; }
                        Stage::Mul  => { result = product.clone() + c_latch.clone(); stage = Stage::Out; }
                        Stage::Out  => { out.write(result.clone()); stage = Stage::Load; }
                    }
                    clk.tick().await;
                }
            }
        "#;
        assert_eq!(regs(src), ["c_latch", "product", "result", "stage"]);
    }

    /// The item-2 generalization: `mac_pipeline`'s registers are **born inside the
    /// loop** (`let` in the body) and live across *interior* awaits — the G6 slice's
    /// pre-loop-only criterion missed all three. Backward liveness catches them.
    #[test]
    fn mac_pipeline_interior_await_registers() {
        let src = r#"
            #[hardware(sequential)]
            async fn mac_pipeline(clk: Clock<C>, a: In<Bits<8>, C>, b: In<Bits<8>, C>, c: In<Bits<8>, C>, out: RegOut<Bits<8>, C>) {
                loop {
                    let product = a.read() * b.read();
                    let c_s = c.read();
                    clk.tick().await;
                    let sum = product + c_s;
                    clk.tick().await;
                    out.write(sum);
                    clk.tick().await;
                }
            }
        "#;
        assert_eq!(regs(src), ["c_s", "product", "sum"]);
    }

    /// A pre-loop constant only *read* in the loop is a wire, not a register —
    /// excluded because it has no def site inside the loop (even though it is live
    /// across ticks).
    #[test]
    fn constant_excluded() {
        let src = r#"
            #[hardware(sequential)]
            async fn m(clk: Clock<C>, o: RegOut<Bits<32>, C>) {
                let mask = Bits::from_u32(7);
                let mut state = Bits::from_u32(1);
                loop {
                    state = state ^ mask;
                    o.write(state);
                    clk.tick().await;
                }
            }
        "#;
        assert_eq!(regs(src), ["state"]);
    }

    /// A same-cycle combinational temp (`let t = …; use t; tick`) is redefined at
    /// the loop head before any post-tick use, so it is killed and not a register.
    #[test]
    fn combinational_temp_excluded() {
        let src = r#"
            #[hardware(sequential)]
            async fn m(clk: Clock<C>, a: In<Bits<8>, C>, o: RegOut<Bits<8>, C>) {
                let mut acc = Bits::from_u32(0);
                loop {
                    let t = a.read() + acc;
                    o.write(t);
                    acc = t;
                    clk.tick().await;
                }
            }
        "#;
        // `acc` crosses the tick (register); `t` is same-cycle (wire).
        assert_eq!(regs(src), ["acc"]);
    }

    // ── pre-tick alignment hazard (PRETICK_ALIGNMENT_GUARDRAIL.md) ──────────





    // ── write between a leading read and the update (V8 battery, m1) ────────










    // ── through `let` temporaries (2026-10-20) ──────────────────────────────








    // ── trailing plain `Out` written from an immediate input read (e8) ──────────








    // ── trailing `RegOut` written from an immediate input read ────────────────

















    // ── the re-derived clauses (2026-10-28): one flipping pair per clause ──────
    //
    // Each source below is a probe measured in tests/sequential_forwarding_divergence.rs
    // (the `d*` tests pin both traces); the verdict is in the test name.

















    // ── the multi-phase rule, narrowed (phase E, 2026-10-23) ───────────────────
    //
    // One unit test per clause, each the source of a probe measured in
    // tests/sequential_forwarding_divergence.rs (verdict named in the test name).

















    // ── §10.6 (2026-10-30): the path-dependent drive in a SINGLE non-head phase ──






    // ── the linear-route anchor rule (`linear_out_write_anchor_mismatch`, §10.7, 2026-10-30) ──














    // ── §10.8 (2026-11-02): the register-free class, and the forms by SHAPE ──






    // ── §10.9 (2026-11-02): the latch fixed, the residue of the conditional forms ──











    // ── the late-constant startup rule (`out_first_driven_late`, 2026-10-23) ──












    /// The **back-edge clause is gone** (Phase C step 6, SPEC I.7.5, D-9). A local
    /// assigned after the iteration's last tick and read in the next head before its
    /// first tick is assigned and consumed within ONE resumption — the one that runs
    /// from that tick to the next — so it is a wire of that resumption (I.7.2(c)),
    /// not a register. From 2026-08-21 to 2026-11-07 a clause of `Cfg::registers`
    /// counted the locals live out of the loop head and this test pinned
    /// `["ff1", "ff2"]`; that was the two-job body's reading (REG9/X9), under which
    /// `ff2 = ff1` read `ff1` before the edge. Under D-8's one resumption `q.write(ff2)`
    /// sees the `ff2 = ff1` of its own resumption, `sync_2ff_ref.sv`'s second flop is
    /// the `RegOut` `q`, and this spelling is one cycle late against the reference
    /// (I.7.5) — so the anchored `sync_2ff` is spelled tick-last, below.
    #[test]
    fn post_tick_def_read_pre_tick_is_a_wire() {
        let src = r#"
            #[hardware(synchronizer)]
            async fn sync_2ff(clk: Clock<D>, d: In<Logic, S>, q: RegOut<Logic, D>) {
                let mut ff1 = Logic::Zero;
                let mut ff2 = Logic::Zero;
                loop {
                    q.write(ff2);
                    clk.tick().await;
                    ff2 = ff1;
                    ff1 = d.read();
                }
            }
        "#;
        assert_eq!(regs(src), ["ff1"]);
        // The anchored, tick-last spelling (`src/sync.rs`): the same set.
        let tick_last = r#"
            #[hardware(synchronizer)]
            async fn sync_2ff(clk: Clock<D>, d: In<Logic, S>, q: RegOut<Logic, D>) {
                let mut ff1 = Logic::Zero;
                let mut ff2 = Logic::Zero;
                loop {
                    ff2 = ff1;
                    ff1 = d.read();
                    q.write(ff2);
                    clk.tick().await;
                }
            }
        "#;
        assert_eq!(regs(tick_last), ["ff1"]);
    }

    /// A temp defined *and* consumed within one post-tick segment dies there, so it
    /// is live at no tick and stays a wire (it is D-input logic in hardware). Kept
    /// from the back-edge clause's time, when it pinned that the clause did not
    /// swallow this neighbouring case.
    #[test]
    fn post_tick_temp_dying_in_its_own_segment_is_not_a_register() {
        let src = r#"
            #[hardware(sequential)]
            async fn m(clk: Clock<C>, a: In<Bits<8>, C>, o: RegOut<Bits<8>, C>) {
                let mut acc = Bits::from_u32(0);
                loop {
                    o.write(acc);
                    clk.tick().await;
                    let t = a.read() + Bits::from_lit::<1>();
                    acc = t + acc;
                }
            }
        "#;
        // `acc` is carried across the boundary; `t` is consumed where it is born.
        assert_eq!(regs(src), ["acc"]);
    }

    /// Tuple-destructuring assignment `(phase, timer) = match …` (traffic_light):
    /// both targets are registers.
    #[test]
    fn tuple_assign_targets_are_registers() {
        let src = r#"
            #[hardware(sequential)]
            async fn tl(clk: Clock<C>, req: In<Logic, C>, r: RegOut<Logic, C>) {
                let mut phase = Phase::Green;
                let mut timer: u8 = 0;
                loop {
                    match phase { Phase::Green => { r.write(Logic::One); } _ => { r.write(Logic::Zero); } }
                    clk.tick().await;
                    (phase, timer) = match (phase, timer, req.read()) {
                        (Phase::Green, _, Logic::One) => (Phase::Yellow, 0),
                        (Phase::Yellow, t, _) if t < 1 => (Phase::Yellow, t + 1),
                        _ => (Phase::Green, 0),
                    };
                }
            }
        "#;
        assert_eq!(regs(src), ["phase", "timer"]);
    }

    /// A tick nested inside one branch, the register read after the merge: `state`
    /// is live across the interior tick (det_010 shape).
    #[test]
    fn branch_tick_state_register() {
        let src = r#"
            #[hardware(sequential)]
            async fn det(clk: Clock<C>, rstn: In<Logic, C>, in_i: In<Logic, C>, out_o: RegOut<Logic, C>) {
                let mut state = State::A;
                loop {
                    if rstn.read() == Logic::Zero { state = State::A; }
                    else { state = next(state, in_i.read()); }
                    clk.tick().await;
                    if matches!(state, State::D) { out_o.write(Logic::One); }
                    else { out_o.write(Logic::Zero); }
                }
            }
        "#;
        assert_eq!(regs(src), ["state"]);
    }

    // ── reachability well-formedness ────────────────────────────────────────

    fn check(src: &str) -> Result<(), String> {
        Cfg::build(&parse(src))
            .expect("has a loop")
            .check_reachability()
            .map_err(|(_, m)| m)
    }

    /// A branch that ticks with no matching else tick falls through to the loop
    /// head combinationally — a zero-time cycle. Rejected.
    #[test]
    fn tickless_fallthrough_rejected() {
        let src = r#"
            #[hardware(sequential)]
            async fn bad(clk: Clock<C>, cond: In<Logic, C>) {
                loop {
                    if cond.read() == Logic::One { clk.tick().await; }
                }
            }
        "#;
        assert!(check(src).is_err(), "a tickless fall-through path must be rejected");
    }

    /// An empty loop never ticks. Rejected.
    #[test]
    fn empty_loop_rejected() {
        let src = r#"
            #[hardware(sequential)]
            async fn spin(clk: Clock<C>) { loop {} }
        "#;
        assert!(check(src).is_err());
    }

    /// Uneven per-branch tick counts are legitimate: every path still crosses a
    /// tick, so the design is well-formed. This is the regression the plan calls
    /// out — the check must not reject asymmetric-but-sound designs.
    #[test]
    fn uneven_but_both_tick_accepted() {
        let src = r#"
            #[hardware(sequential)]
            async fn ok(clk: Clock<C>, cond: In<Logic, C>) {
                loop {
                    if cond.read() == Logic::One { clk.tick().await; }
                    else { clk.tick().await; clk.tick().await; }
                }
            }
        "#;
        assert!(check(src).is_ok(), "asymmetric-but-ticking branches are well-formed");
    }

    /// A trailing unconditional tick after a branch: both arms merge and reach the
    /// tick. Well-formed (mac_fsm / traffic_light shape).
    #[test]
    fn trailing_tick_after_branch_accepted() {
        let src = r#"
            #[hardware(sequential)]
            async fn ok(clk: Clock<C>, cond: In<Logic, C>, o: RegOut<Logic, C>) {
                loop {
                    if cond.read() == Logic::One { o.write(Logic::One); }
                    else { o.write(Logic::Zero); }
                    clk.tick().await;
                }
            }
        "#;
        assert!(check(src).is_ok());
    }

    /// A match where one arm ticks and another falls through is rejected.
    #[test]
    fn match_arm_without_tick_rejected() {
        let src = r#"
            #[hardware(sequential)]
            async fn bad(clk: Clock<C>, sel: In<Bits<2>, C>) {
                loop {
                    match sel.read() {
                        s if s == Bits::from_u32(0) => { clk.tick().await; }
                        _ => { }
                    }
                }
            }
        "#;
        assert!(check(src).is_err());
    }

    // ── nested-loop reachability (recursive) ────────────────────────────────

    /// The nested-loop builder's payoff: a tick inside one branch of a nested
    /// loop, with no tick on the other path — the inner loop spins without ticking
    /// when the condition holds. v1 folded the inner loop opaquely and *missed*
    /// this; the recursive sub-CFG check now rejects it.
    #[test]
    fn nested_loop_tickless_inner_cycle_rejected() {
        let src = r#"
            #[hardware(sequential)]
            async fn bad(clk: Clock<C>, c: In<Logic, C>) {
                loop {
                    loop {
                        if c.read() == Logic::One { clk.tick().await; }
                    }
                }
            }
        "#;
        assert!(check(src).is_err(), "a nested loop that can cycle without ticking must be rejected");
    }

    /// The canonical well-formed nested loop (rv32i / uart shape): tick first, then
    /// an exit test. Every inner iteration ticks before it can break.
    #[test]
    fn nested_loop_tick_then_break_accepted() {
        let src = r#"
            #[hardware(sequential)]
            async fn ok(clk: Clock<C>, ready: In<Logic, C>, o: RegOut<Logic, C>) {
                loop {
                    loop {
                        clk.tick().await;
                        if ready.read() == Logic::One { break; }
                    }
                    o.write(Logic::One);
                    clk.tick().await;
                }
            }
        "#;
        assert!(check(src).is_ok());
    }

    /// `break` before the tick must not read as a tickless cycle: the break path
    /// exits the loop (to the sink), it does not return to the inner head.
    #[test]
    fn nested_loop_break_before_tick_accepted() {
        let src = r#"
            #[hardware(sequential)]
            async fn ok(clk: Clock<C>, r: In<Logic, C>, o: RegOut<Logic, C>) {
                loop {
                    loop {
                        if r.read() == Logic::One { break; }
                        clk.tick().await;
                    }
                    o.write(Logic::One);
                    clk.tick().await;
                }
            }
        "#;
        assert!(check(src).is_ok());
    }

    /// The **zero-tick exit**. `outer`'s only clock boundary is `inner`, and `inner`
    /// is the mandated test-before-tick shape — so when `b` is high on entry it
    /// breaks without ticking and `outer` returns to its top in zero time.
    ///
    /// Measured before this was rejected: the simulator livelocks on it (99.5% CPU,
    /// no progress) while the flattened FSM runs one cycle per iteration. The
    /// optimistic fold accepted it because a tick-bearing nested loop contributed a
    /// Tick edge unconditionally.
    #[test]
    fn nested_loop_that_can_exit_before_ticking_is_not_a_clock_boundary() {
        let src = r#"
            #[hardware(sequential)]
            async fn bad(clk: Clock<C>, a: In<Logic, C>, b: In<Logic, C>) {
                loop {
                    loop {
                        if a.read() == Logic::One { break; }
                        loop {
                            if b.read() == Logic::One { break; }
                            clk.tick().await;
                        }
                    }
                }
            }
        "#;
        let err = check(src).expect_err(
            "a loop whose only boundary is a nested loop that can break before ticking \
             is a zero-time cycle — the simulator livelocks on it",
        );
        assert!(
            err.contains("left before it ticks"),
            "the diagnostic must name the nested loop, not advise adding a tick to a \
             body that already has one: {}",
            err
        );
    }

    /// The same zero-tick exit one level up: the module's own loop has no tick of its
    /// own and the `loop` it delegates to can break immediately.
    #[test]
    fn a_lone_nested_wait_is_not_a_clock_boundary_for_the_module_loop() {
        let src = r#"
            #[hardware(sequential)]
            async fn bad(clk: Clock<C>, go: In<Logic, C>) {
                loop {
                    loop {
                        if go.read() == Logic::One { break; }
                        clk.tick().await;
                    }
                }
            }
        "#;
        assert!(check(src).is_err());
    }

    /// A `while` needs no `break` to exit in zero iterations, so it is never a
    /// guaranteed boundary either. The transpiler desugars a tick-bearing `while`
    /// to `loop { if !cond { break; } … }`, which reaches the same verdict by the
    /// `break`-before-tick route — the two front-ends must not disagree.
    #[test]
    fn a_lone_while_wait_is_not_a_clock_boundary() {
        let src = r#"
            #[hardware(sequential)]
            async fn bad(clk: Clock<C>, go: In<Logic, C>) {
                loop {
                    while go.read() == Logic::One {
                        clk.tick().await;
                    }
                }
            }
        "#;
        assert!(check(src).is_err());
    }

    /// The counted case the fold's optimism exists for, and which keeps it: a `for`
    /// runs its body, so it ticks. This is `uart_tx`'s bit-timing delay, and the
    /// shape cause K ultimately has to support — it must stay well-formed.
    #[test]
    fn a_counted_for_is_still_a_clock_boundary() {
        let src = r#"
            #[hardware(sequential)]
            async fn ok(clk: Clock<C>, o: RegOut<Logic, C>) {
                loop {
                    o.write(Logic::One);
                    for _ in 0..8 {
                        clk.tick().await;
                    }
                }
            }
        "#;
        assert!(
            check(src).is_ok(),
            "a counted repetition runs its body and therefore ticks; rejecting it \
             would break uart_tx and rv32i_cpu"
        );
    }

    /// A wait that can exit early is fine as long as the *enclosing* loop ticks on
    /// its own — the zero-tick exit only matters when it is the only boundary. This
    /// is the common `waiter` idiom and must not be caught by the tightening.
    #[test]
    fn an_early_exiting_wait_is_fine_when_the_outer_loop_ticks_too() {
        let src = r#"
            #[hardware(sequential)]
            async fn ok(clk: Clock<C>, go: In<Logic, C>, o: RegOut<Logic, C>) {
                loop {
                    loop {
                        if go.read() == Logic::One { break; }
                        clk.tick().await;
                    }
                    o.write(Logic::One);
                    clk.tick().await;
                }
            }
        "#;
        assert!(check(src).is_ok());
    }

    /// `continue` that skips the only tick creates a tickless inner cycle.
    #[test]
    fn nested_loop_continue_before_tick_rejected() {
        let src = r#"
            #[hardware(sequential)]
            async fn bad(clk: Clock<C>, c: In<Logic, C>) {
                loop {
                    loop {
                        if c.read() == Logic::One { continue; }
                        clk.tick().await;
                    }
                }
            }
        "#;
        assert!(check(src).is_err());
    }

    /// A `while` whose body has a tickless path is caught by the recursion, even
    /// though the *outer* loop is well-formed (the while is folded as ticking there).
    #[test]
    fn nested_while_tickless_path_rejected() {
        let src = r#"
            #[hardware(sequential)]
            async fn bad(clk: Clock<C>, cond: In<Logic, C>, c: In<Logic, C>) {
                loop {
                    while cond.read() == Logic::One {
                        if c.read() == Logic::One { clk.tick().await; }
                    }
                    clk.tick().await;
                }
            }
        "#;
        assert!(check(src).is_err());
    }

    /// A tick-free nested loop is combinational (an unrolled `for`) and is NOT
    /// subject to the must-tick rule — its `body → head` is not a clocked cycle.
    #[test]
    fn tick_free_nested_loop_accepted() {
        let src = r#"
            #[hardware(sequential)]
            async fn ok(clk: Clock<C>, o: RegOut<Bits<8>, C>) {
                loop {
                    let mut acc = Bits::from_u32(0);
                    for i in 0..8 { acc = acc + Bits::from_u32(1); }
                    o.write(acc);
                    clk.tick().await;
                }
            }
        "#;
        assert!(check(src).is_ok());
    }

    // ── definite assignment (combinational modules only) ────────────────────

    fn da(src: &str) -> Result<(), String> {
        Cfg::build_combinational(&parse(src))
            .check_definite_assignment()
            .map_err(|(_, m)| m)
    }

    /// A combinational output assigned on all paths (both `if` arms) is fine.
    #[test]
    fn comb_output_all_paths_ok() {
        let src = r#"
            #[hardware(combinational)]
            fn m(sel: In<Logic, ()>, o: Out<Logic, ()>) {
                if sel.read() == Logic::One { o.write(Logic::One); }
                else { o.write(Logic::Zero); }
            }
        "#;
        assert!(da(src).is_ok());
    }

    /// A straight-line unconditional write is fine.
    #[test]
    fn comb_output_unconditional_ok() {
        let src = r#"
            #[hardware(combinational)]
            fn m(a: In<Logic, ()>, b: In<Logic, ()>, o: Out<Logic, ()>) {
                let p = a.read() & b.read();
                o.write(p);
            }
        "#;
        assert!(da(src).is_ok());
    }

    /// A combinational output written in only one branch (no else) infers a latch.
    #[test]
    fn comb_output_partial_is_latch() {
        let src = r#"
            #[hardware(combinational)]
            fn m(sel: In<Logic, ()>, o: Out<Logic, ()>) {
                if sel.read() == Logic::One { o.write(Logic::One); }
            }
        "#;
        assert!(da(src).is_err(), "a partial combinational output must be flagged as a latch");
    }

    /// A `match` missing a write on one arm infers a latch.
    #[test]
    fn comb_output_partial_match_is_latch() {
        let src = r#"
            #[hardware(combinational)]
            fn m(sel: In<Bits<2>, ()>, o: Out<Logic, ()>) {
                match sel.read() {
                    s if s == Bits::from_u32(0) => { o.write(Logic::One); }
                    _ => {}
                }
            }
        "#;
        assert!(da(src).is_err());
    }

    /// An output never written is not a latch here (it may be driven by a
    /// submodule instantiation) — only a *partial* write is flagged.
    #[test]
    fn comb_output_never_written_not_flagged() {
        let src = r#"
            #[hardware(combinational)]
            fn m(a: In<Logic, ()>, o: Out<Logic, ()>) {
                let _ = a.read();
            }
        "#;
        assert!(da(src).is_ok());
    }

    /// A sequential `Out` written conditionally is an enabled register (holds),
    /// NOT a latch — definite-assignment must not fire on sequential modules
    /// (verified `sim ≡ BaseJump` on `bsg_dff_en`).
    #[test]
    fn sequential_conditional_output_not_flagged() {
        let src = r#"
            #[hardware(sequential)]
            async fn dff_en(clk: Clock<C>, d: In<Bits<8>, C>, en: In<Logic, C>, q: RegOut<Bits<8>, C>) {
                loop {
                    clk.tick().await;
                    if en.read() == Logic::One { q.write(d.read()); }
                }
            }
        "#;
        // The public router skips sequential modules entirely.
        assert!(crate::check_definite_assignment(&parse(src)).is_ok());
    }

    // ── memory staging rules ────────────────────────────────────────────────

    fn mem_check(src: &str) -> Result<(), String> {
        Cfg::build(&parse(src))
            .expect("has a loop")
            .check_memory_staging()
            .map_err(|(_, m)| m)
    }

    /// A ROM read, wrapped in a body one line away from the linear form: the
    /// trailing `for` adds two clock boundaries and nothing else, and it is what
    /// makes `control_extract` rewrite the module. Under the old segment-based
    /// check that rewrite collapsed the staging and the observation into one
    /// segment and the module was refused — the false positive this rule exists to
    /// fix. `tests/extracted_memory_equivalence.rs` carries the behavioural half.
    #[test]
    fn a_counted_pause_after_the_observation_is_not_a_same_cycle_read() {
        let src = r#"
            #[hardware(sequential)]
            async fn m(clk: Clock<C>, a: In<Bits<8>, C>, o: RegOut<Bits<16>, C>) {
                let rom = Memory::<Bits<16>, 1, 0, C, 1, 1>::new(clk.clone(), 256);
                loop {
                    rom.read_port::<0>().read(a.read().as_usize());
                    clk.tick().await;
                    o.write(rom.read_port::<0>().data());
                    for _ in 0..2 { clk.tick().await; }
                }
            }
        "#;
        assert_eq!(mem_check(src), Ok(()));
    }

    /// The same shape without the pause — the linear form, which always worked.
    #[test]
    fn a_read_observed_after_the_tick_is_accepted() {
        let src = r#"
            #[hardware(sequential)]
            async fn m(clk: Clock<C>, a: In<Bits<8>, C>, o: RegOut<Bits<16>, C>) {
                let rom = Memory::<Bits<16>, 1, 0, C, 1, 1>::new(clk.clone(), 256);
                loop {
                    rom.read_port::<0>().read(a.read().as_usize());
                    clk.tick().await;
                    o.write(rom.read_port::<0>().data());
                }
            }
        "#;
        assert_eq!(mem_check(src), Ok(()));
    }

    /// The rule itself, in a module control extraction rewrites: the observation
    /// is in the staging cycle, so it reads the result an edge early. The old
    /// check could not see this at all once the ticks became `pc` states.
    #[test]
    fn a_same_cycle_observation_is_rejected_even_when_extracted() {
        let src = r#"
            #[hardware(sequential)]
            async fn m(clk: Clock<C>, a: In<Bits<8>, C>, o: RegOut<Bits<16>, C>) {
                let rom = Memory::<Bits<16>, 1, 0, C, 1, 1>::new(clk.clone(), 256);
                loop {
                    rom.read_port::<0>().read(a.read().as_usize());
                    o.write(rom.read_port::<0>().data());
                    clk.tick().await;
                    for _ in 0..2 { clk.tick().await; }
                }
            }
        "#;
        let err = mem_check(src).expect_err("observing in the staging cycle must be rejected");
        assert!(err.contains("is read before the `clk.tick().await`"), "{err}");
    }

    /// Observing a port nothing ever stages.
    #[test]
    fn an_unstaged_port_is_rejected() {
        let src = r#"
            #[hardware(sequential)]
            async fn m(clk: Clock<C>, a: In<Bits<8>, C>, o: RegOut<Bits<16>, C>) {
                let rom = Memory::<Bits<16>, 1, 0, C, 1, 1>::new(clk.clone(), 256);
                loop {
                    rom.read_port::<0>().read(a.read().as_usize());
                    clk.tick().await;
                    o.write(rom.read_port::<1>().data());
                }
            }
        "#;
        let err = mem_check(src).expect_err("an unstaged port never becomes ready");
        assert!(err.contains("nothing stages a `read()`"), "{err}");
    }

    /// Two stagings of one bus that a single cycle really does reach — one after
    /// the other, no edge between them. There is one address bus, so the design
    /// has to say which address; the simulator would silently keep the last.
    #[test]
    fn two_stagings_of_one_bus_in_one_cycle_are_rejected() {
        let src = r#"
            #[hardware(sequential)]
            async fn m(clk: Clock<C>, a: In<Bits<8>, C>, o: RegOut<Bits<16>, C>) {
                let rom = Memory::<Bits<16>, 1, 0, C, 1, 1>::new(clk.clone(), 256);
                loop {
                    rom.read_port::<0>().read(a.read().as_usize());
                    rom.read_port::<0>().read(0);
                    clk.tick().await;
                    o.write(rom.read_port::<0>().data());
                }
            }
        "#;
        let err = mem_check(src).expect_err("one address bus, two drivers in a cycle");
        assert!(err.contains("accessed 2 times in one cycle"), "{err}");
    }

    /// Stagings on **exclusive** branches are a multiplexer, not a conflict: no
    /// path joins them, so no cycle drives the bus twice, and the emitted
    /// `always_comb` assigns the address inside the arm that runs. Counting sites
    /// per phase instead reported a design error on `rv32i_cpu`'s seven regfile
    /// writebacks, which are exactly this shape.
    /// `tests/extracted_memory_equivalence.rs::exclusive_arm_writes_…` is the
    /// behavioural half.
    #[test]
    fn stagings_on_exclusive_branches_are_a_mux() {
        let src = r#"
            #[hardware(sequential)]
            async fn m(clk: Clock<C>, s: In<Logic, C>, a: In<Bits<8>, C>, o: RegOut<Bits<16>, C>) {
                let rom = Memory::<Bits<16>, 1, 0, C, 1, 1>::new(clk.clone(), 256);
                loop {
                    if s.read() == Logic::One {
                        rom.read_port::<0>().read(a.read().as_usize());
                    } else {
                        rom.read_port::<0>().read(0);
                    }
                    clk.tick().await;
                    o.write(rom.read_port::<0>().data());
                }
            }
        "#;
        assert_eq!(mem_check(src), Ok(()));
    }

    /// A **received** memory is checked exactly like a declared one.
    ///
    /// This is the pin that matters for the parameter spelling. `memory_locals`
    /// used to find memories only through `let mem = Memory::<…>::new(…)`, so the
    /// moment `#[hardware]` began accepting a `Memory<…>` parameter, a module that
    /// received one would not have been diagnosed as safe — it would have stopped
    /// being examined, and every rule below would have gone quiet on it. The
    /// violation here is the same one `two_stagings_of_one_bus_in_one_cycle_are_rejected`
    /// uses, so a regression shows up as this test passing where that one fails.
    #[test]
    fn a_received_memory_is_checked_like_a_declared_one() {
        let src = r#"
            #[hardware(sequential)]
            async fn m(clk: Clock<C>, mem: Memory<Bits<16>, 1, 0, C, 1, 1>, a: In<Bits<8>, C>, o: RegOut<Bits<16>, C>) {
                loop {
                    mem.read_port::<0>().read(a.read().as_usize());
                    mem.read_port::<0>().read(0);
                    clk.tick().await;
                    o.write(mem.read_port::<0>().data());
                }
            }
        "#;
        let err = mem_check(src).expect_err("one address bus, two drivers in a cycle");
        assert!(err.contains("accessed 2 times in one cycle"), "{err}");
    }

    /// The well-formed half of the same shape: receiving a memory is not itself a
    /// finding. Without this, the test above would still pass if the check had
    /// started rejecting every received memory outright.
    #[test]
    fn a_well_formed_received_memory_is_accepted() {
        let src = r#"
            #[hardware(sequential)]
            async fn m(clk: Clock<C>, mem: Memory<Bits<16>, 1, 0, C, 1, 1>, a: In<Bits<8>, C>, o: RegOut<Bits<16>, C>) {
                loop {
                    mem.read_port::<0>().read(a.read().as_usize());
                    clk.tick().await;
                    o.write(mem.read_port::<0>().data());
                }
            }
        "#;
        assert_eq!(mem_check(src), Ok(()));
    }

    /// Stagings in *different* cycles are the ordinary multi-phase pattern — the
    /// address bus is phase-gated — and must not be read as a conflict.
    #[test]
    fn stagings_in_different_cycles_are_not_a_conflict() {
        let src = r#"
            #[hardware(sequential)]
            async fn m(clk: Clock<C>, a: In<Bits<8>, C>, b: In<Bits<8>, C>, o: RegOut<Bits<16>, C>) {
                let rom = Memory::<Bits<16>, 1, 0, C, 1, 1>::new(clk.clone(), 256);
                loop {
                    rom.read_port::<0>().read(a.read().as_usize());
                    clk.tick().await;
                    o.write(rom.read_port::<0>().data());
                    rom.read_port::<0>().read(b.read().as_usize());
                    clk.tick().await;
                }
            }
        "#;
        assert_eq!(mem_check(src), Ok(()));
    }

    /// A read port and a write port are different buses, so one of each in a cycle
    /// is the baseline RAM shape (`dual_port_ram`, every memory fixture).
    #[test]
    fn a_read_and_a_write_in_one_cycle_are_different_buses() {
        let src = r#"
            #[hardware(sequential)]
            async fn m(clk: Clock<C>, we: In<Logic, C>, a: In<Bits<8>, C>, d: In<Bits<16>, C>,
                       o: RegOut<Bits<16>, C>) {
                let mem = Memory::<Bits<16>, 1, 1, C, 1, 1>::new(clk.clone(), 256);
                loop {
                    if we.read() == Logic::One {
                        mem.write_port::<0>().write(a.read().as_usize(), d.read());
                    }
                    mem.read_port::<0>().read(a.read().as_usize());
                    clk.tick().await;
                    o.write(mem.read_port::<0>().data());
                }
            }
        "#;
        assert_eq!(mem_check(src), Ok(()));
    }

    /// The trailing statements of a body run in the same cycle as the head — the
    /// back edge costs nothing — so a staging in one and an observation in the
    /// other is a same-cycle read. The segment-index form of this rule got the
    /// pairing wrong here in both directions.
    #[test]
    fn a_trailing_staging_and_a_head_observation_share_a_cycle() {
        let src = r#"
            #[hardware(sequential)]
            async fn m(clk: Clock<C>, a: In<Bits<8>, C>, o: RegOut<Bits<16>, C>) {
                let rom = Memory::<Bits<16>, 1, 0, C, 1, 1>::new(clk.clone(), 256);
                loop {
                    o.write(rom.read_port::<0>().data());
                    clk.tick().await;
                    rom.read_port::<0>().read(a.read().as_usize());
                }
            }
        "#;
        let err = mem_check(src).expect_err("the trailing staging shares the head's cycle");
        assert!(err.contains("is read before the `clk.tick().await`"), "{err}");
    }






    /// A module with no `Memory` local has nothing to check — and a `read_port`
    /// method on something else is not a memory access.
    #[test]
    fn a_module_without_a_memory_is_unaffected() {
        let src = r#"
            #[hardware(sequential)]
            async fn m(clk: Clock<C>, a: In<Bits<8>, C>, o: RegOut<Bits<8>, C>) {
                loop {
                    o.write(a.read());
                    clk.tick().await;
                }
            }
        "#;
        assert_eq!(mem_check(src), Ok(()));
    }
}
