//! **P1 and P15 — the projection, built and checked per module** (SPEC III.1;
//! D-20 Phase C step 7; `design_docs/review/MEASUREMENTS.md` §L).
//!
//! P1: the emitted state machine is bisimilar to H(M) (SPEC I.0.4) under a relation
//! `R` between the emitted control state and the coroutine's **continuations**
//! (resumption start points): `(e, k) ∈ R` iff the code the emitted machine runs in
//! state `e` is the resumption that starts at `k`. P15: the emitted initial state
//! encodes the entry (I.9.2) — `s_pre` once, then the first continuation.
//!
//! This test BUILDS `R` for every clocked module the corpus sweep covers and checks
//! every transition of it statically. It runs no simulator and no Verilator.
//!
//! # The two sides, and why they are independent
//!
//! **Source (H(M)).** The module's `syn::ItemFn`, walked by a continuation-stack
//! interpreter of Rust control flow written here (`Src`): `if`/`else`, `match`,
//! `loop`, `while`, a counted `for`, `break`, `continue`, a block, and the `tick`
//! statement. A continuation is a `tick` site — named by the reference
//! interpreter's index-path convention and cross-checked against
//! `copper_ref::static_tick_sites` — plus, for each enclosing counted `for` the
//! next resumption leaves or re-enters, which of the two it does (below). Nothing
//! on this side calls `copper_codegen`: not `control_extract`'s desugarings, not
//! its state allocation. The only shared code is the constant evaluator of SPEC
//! I.14.4 (`copper_analysis::consts`), used to decide whether a `for` range is
//! statically empty or non-empty — a fact about the program, not about the
//! extractor.
//!
//! **Emitted.** The module's CHIR as `copper_codegen::lower_source_module` lowers
//! it (every front-end rule, control extraction, `chir_lower` — the last IR with a
//! source span on every statement), and its SHIR for what CHIR does not yet say:
//! the phase register's next values, `__copper_started`'s update, the power-on
//! values. The extraction route's control state is `pc` (the arms of the
//! top-level `match pc`); the linear route's is the phase index of SHIR
//! (`phase_r`, implicit when there is one phase), phase `k` running CHIR segment
//! `k` and phase 0 running the trailing segment first under `__copper_started` —
//! `shir_lower::lower_seq_body`'s composition, restated here and cross-checked
//! against SHIR's phase count and guarded trailing updates. Both routes carry
//! `__copper_started`, so an emitted state is `(pc or phase, started, counter
//! facts)`.
//!
//! # What a transition is compared on
//!
//! A resumption is a set of PATHS; a path is a sequence of labels ending at the
//! next state. Labels:
//! * `Run(p)` — an atomic source statement (one that contains no `tick` and no
//!   `break`/`continue` leaving it) ran; `p` is its start position. An emitted
//!   statement is mapped to the outermost atomic source statement whose span
//!   contains its span's start; consecutive equal `Run`s are collapsed on both
//!   sides (one source statement may lower to several CHIR statements).
//! * `Test(p, outcome)` — a control `if` (`then` 0 / `else` 1), `match` (arm
//!   index) or `while` (`continue` 0 / `exit` 1) at `p` took that outcome. An
//!   emitted `if`/`match` is control iff its subtree assigns `pc`.
//! * `Count(p, …)` — a counted `for` at `p`: `Empty`/`Enter` at entry, and
//!   `Last`/`More` before the tick that ends an iteration. Rust's `for` tests
//!   `v + 1 >= end` AFTER that tick; since the test is pure, choosing its outcome
//!   BEFORE the tick is the same program (the source continuation records the
//!   choice), and it is the reading under which the emitted machine — which
//!   increments and tests before the tick — can be related state by state. The
//!   emitted side recognizes the `for`'s synthesized statements by their span (the
//!   `for`'s own) and runs a two-fact abstract interpretation of each counter
//!   (`advanced from start?`, `at end?`), so an exit test whose outcome is known
//!   (the rotated head's re-test) is a pruned branch, not a label.
//!
//! For each pair `(e, k)` reached from the initial pair, the set of label
//! sequences of `e`'s paths must EQUAL that of `k`'s; each matched pair of paths
//! yields the successor pair, added to `R`. So every transition of `R`, every
//! branch outcome, is checked, and `R` is the reachable product — many-to-many
//! where the extractor merges continuations into one `pc` or clones one per arm.
//!
//! # P15
//!
//! The initial pair is (`pc`/phase at its SHIR power-on value, `__copper_started`
//! at its power-on value) against the coroutine's `Entry`, whose one transition is
//! the pre-loop expression statements, then the head to the first tick. Also
//! checked: `pc`/`phase_r`/`__copper_started` power on at 0 in SHIR and in the
//! emitted `initial` block; `__copper_started` exists whenever something reads it
//! and is set to 1 at phase 0's edge and assigned nowhere else; no reachable pair
//! but the initial one has `started = 0`; `Entry` is in no other pair.
//!
//! # What it cannot see (MEASUREMENTS §L)
//!
//! The content of an atomic statement (its expressions, forwarding, widths: P8, the
//! sweep); data (a branch is a label, never evaluated — the counter facts are the
//! only data tracked, so a correlated pair of tests is two independent labels on
//! both sides); a statement run twice in a row (collapsed); anything below CHIR
//! (`shir_lower`'s per-phase lowering is restated, not read; VLIR/emit only through
//! the `initial` block).

use copper_core::chir::{CHIRBinOp, CHIRBody, CHIRExpr, CHIRSeqBody, CHIRStmt, CHIRUnOp, STARTED_REG};
use copper_core::shir::{SHIRBody, SHIRExpr, SHIRSeqBody};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};
use syn::spanned::Spanned;
use syn::{Expr, Stmt};

include!(concat!(env!("OUT_DIR"), "/corpus_manifest.rs"));

/// Modules the test examines but cannot relate, each with a reason — printed on
/// every run; a row whose module IS related fails the test (stale), and a module
/// that cannot be related without a row fails it too.
const UNCOVERED: &[(&str, &str)] = &[];

// ── positions ────────────────────────────────────────────────────────────────

type Pos = (usize, usize);

#[derive(Clone, Copy, Debug)]
struct Range {
    start: Pos,
    end: Pos,
}

fn range_of(node: &impl Spanned) -> Range {
    let sp = node.span();
    Range { start: (sp.start().line, sp.start().column), end: (sp.end().line, sp.end().column) }
}

fn pos_of(node: &impl Spanned) -> Pos {
    range_of(node).start
}

fn fmt_pos(p: Pos) -> String {
    format!("{}:{}", p.0, p.1)
}

// ── labels ───────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
enum Cnt {
    Empty,
    Enter,
    Last,
    More,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
enum Lab {
    Run(Pos),
    Test(Pos, usize),
    Count(Pos, Cnt),
}

impl std::fmt::Display for Lab {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Lab::Run(p) => write!(f, "run@{}", fmt_pos(*p)),
            Lab::Test(p, k) => write!(f, "test@{}={k}", fmt_pos(*p)),
            Lab::Count(p, c) => write!(f, "for@{}={c:?}", fmt_pos(*p)),
        }
    }
}

fn push_lab(labs: &mut Vec<Lab>, l: Lab) {
    if let (Lab::Run(a), Some(Lab::Run(b))) = (l, labs.last()) {
        if a == *b {
            return;
        }
    }
    labs.push(l);
}

fn fmt_path(labs: &[Lab]) -> String {
    if labs.is_empty() {
        return "(nothing)".into();
    }
    labs.iter().map(|l| l.to_string()).collect::<Vec<_>>().join(" ")
}

// ── the source side: H(M) from the syn::ItemFn ──────────────────────────────

/// A continuation: the entry, or a `tick` site plus the outcome chosen for each
/// counted `for` whose iteration that tick ends (`true` = last iteration).
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
enum Cont {
    Entry,
    After { site: Vec<usize>, last: Vec<(Pos, bool)> },
}

impl std::fmt::Display for Cont {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Cont::Entry => write!(f, "Entry"),
            Cont::After { site, last } => {
                let s: Vec<String> = site.iter().map(|i| i.to_string()).collect();
                write!(f, "[{}]", s.join("."))?;
                for (p, l) in last {
                    write!(f, " for@{}:{}", fmt_pos(*p), if *l { "last" } else { "more" })?;
                }
                Ok(())
            }
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum ForBounds {
    StaticEmpty,
    StaticNonEmpty,
    Unknown,
}

#[derive(Clone, Debug)]
enum Frame<'a> {
    Seq { stmts: &'a [Stmt], next: usize, prefix: Vec<usize> },
    /// One control expression with no statement index of its own (an `else if`,
    /// a non-block `match` arm); `done` once it ran.
    One { expr: &'a Expr, pos: Pos, prefix: Vec<usize>, done: bool },
    ModLoop,
    Loop { body: &'a [Stmt], prefix: Vec<usize> },
    While { pos: Pos, body: &'a [Stmt], prefix: Vec<usize> },
    For { pos: Pos, body: &'a [Stmt], prefix: Vec<usize>, last: Option<bool> },
}

struct Src<'a> {
    loop_idx: usize,
    body: &'a [Stmt],
    pre: &'a [Stmt],
    /// Atomic statements' spans (loop body and the pre-loop expression statements).
    atomic: Vec<Range>,
    /// Pre-loop `let`s: initializers (I.9.2), not statements of a resumption.
    init: Vec<Range>,
    /// Control constructs by start position.
    ifs: HashSet<Pos>,
    matches: HashSet<Pos>,
    whiles: HashSet<Pos>,
    fors: HashMap<Pos, ForBounds>,
    sites: BTreeSet<Vec<usize>>,
    /// Sites inside a statically empty `for` — no continuation reaches them.
    dead_sites: BTreeSet<Vec<usize>>,
    /// The const-generic ranges R3 takes as non-empty and D-30 holds per instance
    /// (`copper_analysis::assumed_nonempty_generic_ranges`), by `range_key`: non-empty here
    /// too, as the transpiler lowers them.
    assumed: HashSet<String>,
}

/// A `const { … }` block statement: evaluated by rustc, no statement of any
/// resumption (`shift_register`'s width assertion; MEASUREMENTS §L.1).
fn is_compile_time(s: &Stmt) -> bool {
    matches!(s, Stmt::Expr(Expr::Const(_), _))
}

fn is_tick_expr(e: &Expr) -> bool {
    matches!(e, Expr::Await(_))
}

fn contains_tick_stmts(stmts: &[Stmt]) -> bool {
    stmts.iter().any(|s| matches!(s, Stmt::Expr(e, _) if contains_tick(e)))
}

fn contains_tick(e: &Expr) -> bool {
    match e {
        Expr::Await(_) => true,
        Expr::If(i) => {
            contains_tick_stmts(&i.then_branch.stmts) || i.else_branch.as_ref().is_some_and(|(_, el)| contains_tick(el))
        }
        Expr::Match(m) => m.arms.iter().any(|a| contains_tick(&a.body)),
        Expr::Loop(l) => contains_tick_stmts(&l.body.stmts),
        Expr::While(w) => contains_tick_stmts(&w.body.stmts),
        Expr::ForLoop(f) => contains_tick_stmts(&f.body.stmts),
        Expr::Block(b) => contains_tick_stmts(&b.block.stmts),
        _ => false,
    }
}

/// A `break`/`continue` that leaves `e` (not one inside a loop nested in it).
fn escapes(e: &Expr) -> bool {
    fn stmts(ss: &[Stmt]) -> bool {
        ss.iter().any(|s| matches!(s, Stmt::Expr(e, _) if escapes(e)))
    }
    match e {
        Expr::Break(_) | Expr::Continue(_) => true,
        Expr::If(i) => stmts(&i.then_branch.stmts) || i.else_branch.as_ref().is_some_and(|(_, el)| escapes(el)),
        Expr::Match(m) => m.arms.iter().any(|a| escapes(&a.body)),
        Expr::Block(b) => stmts(&b.block.stmts),
        _ => false,
    }
}

fn is_control(e: &Expr) -> bool {
    contains_tick(e) || escapes(e)
}

impl<'a> Src<'a> {
    fn new(f: &'a syn::ItemFn, consts: &dyn Fn(&str) -> Option<i128>) -> Result<Src<'a>, String> {
        let stmts = &f.block.stmts;
        let loop_idx = stmts
            .iter()
            .position(|s| matches!(s, Stmt::Expr(Expr::Loop(_), _)))
            .ok_or("no module loop")?;
        let Stmt::Expr(Expr::Loop(l), _) = &stmts[loop_idx] else { unreachable!() };
        if stmts.len() != loop_idx + 1 {
            return Err("statements after the module loop".into());
        }
        let mut src = Src {
            loop_idx,
            body: &l.body.stmts,
            pre: &stmts[..loop_idx],
            atomic: Vec::new(),
            init: Vec::new(),
            ifs: HashSet::new(),
            matches: HashSet::new(),
            whiles: HashSet::new(),
            fors: HashMap::new(),
            sites: BTreeSet::new(),
            dead_sites: BTreeSet::new(),
            assumed: copper_analysis::assumed_nonempty_generic_ranges(f).iter().map(copper_analysis::range_key).collect(),
        };
        for s in src.pre {
            match s {
                Stmt::Local(_) => src.init.push(range_of(s)),
                Stmt::Item(_) => {}
                s if is_compile_time(s) => src.init.push(range_of(s)),
                _ => src.atomic.push(range_of(s)),
            }
        }
        let body = src.body;
        src.classify_stmts(body, &mut vec![loop_idx, 0], consts, false);
        Ok(src)
    }

    fn classify_stmts(&mut self, stmts: &'a [Stmt], prefix: &mut Vec<usize>, consts: &dyn Fn(&str) -> Option<i128>, dead: bool) {
        for (i, s) in stmts.iter().enumerate() {
            prefix.push(i);
            match s {
                s if is_compile_time(s) => self.init.push(range_of(s)),
                Stmt::Expr(e, _) => self.classify_expr(e, range_of(s), prefix, consts, dead),
                Stmt::Item(_) => {}
                _ => self.atomic.push(range_of(s)),
            }
            prefix.pop();
        }
    }

    fn classify_expr(&mut self, e: &'a Expr, r: Range, prefix: &mut Vec<usize>, consts: &dyn Fn(&str) -> Option<i128>, dead: bool) {
        if is_tick_expr(e) {
            self.sites.insert(prefix.clone());
            if dead {
                self.dead_sites.insert(prefix.clone());
            }
            return;
        }
        if !is_control(e) {
            self.atomic.push(r);
            return;
        }
        match e {
            Expr::If(i) => {
                self.ifs.insert(pos_of(i));
                prefix.push(0);
                self.classify_stmts(&i.then_branch.stmts, prefix, consts, dead);
                prefix.pop();
                if let Some((_, el)) = &i.else_branch {
                    prefix.push(1);
                    match &**el {
                        Expr::Block(b) => self.classify_stmts(&b.block.stmts, prefix, consts, dead),
                        other => self.classify_expr(other, range_of(other), prefix, consts, dead),
                    }
                    prefix.pop();
                }
            }
            Expr::Match(m) => {
                self.matches.insert(pos_of(m));
                for (k, arm) in m.arms.iter().enumerate() {
                    prefix.push(k);
                    match &*arm.body {
                        Expr::Block(b) => self.classify_stmts(&b.block.stmts, prefix, consts, dead),
                        other => {
                            prefix.push(0);
                            self.classify_expr(other, range_of(arm), prefix, consts, dead);
                            prefix.pop();
                        }
                    }
                    prefix.pop();
                }
            }
            Expr::Loop(l) => {
                prefix.push(0);
                self.classify_stmts(&l.body.stmts, prefix, consts, dead);
                prefix.pop();
            }
            Expr::While(w) => {
                self.whiles.insert(pos_of(w));
                prefix.push(0);
                self.classify_stmts(&w.body.stmts, prefix, consts, dead);
                prefix.pop();
            }
            Expr::ForLoop(f) => {
                let mut b = for_bounds(f, consts);
                if matches!(b, ForBounds::Unknown) && matches!(&*f.expr, Expr::Range(r) if self.assumed.contains(&copper_analysis::range_key(r))) {
                    b = ForBounds::StaticNonEmpty;
                }
                self.fors.insert(pos_of(f), b);
                prefix.push(0);
                self.classify_stmts(&f.body.stmts, prefix, consts, dead || matches!(b, ForBounds::StaticEmpty));
                prefix.pop();
            }
            Expr::Block(b) => self.classify_stmts(&b.block.stmts, prefix, consts, dead),
            // A bare `break`/`continue` statement: control, no span of its own to map.
            _ => {}
        }
    }

    /// The initial frame stack of `Entry` (after the pre-loop statements) and the
    /// labels of the pre-loop expression statements.
    fn entry(&self) -> (Vec<Frame<'a>>, Vec<Lab>) {
        let mut labs = Vec::new();
        for s in self.pre {
            if !matches!(s, Stmt::Local(_) | Stmt::Item(_)) && !is_compile_time(s) {
                push_lab(&mut labs, Lab::Run(pos_of(s)));
            }
        }
        let stack = vec![Frame::ModLoop, Frame::Seq { stmts: self.body, next: 0, prefix: vec![self.loop_idx, 0] }];
        (stack, labs)
    }

    /// Every path of the resumption that starts with `stack`: its labels, the
    /// continuation it parks at, and that continuation's frame stack.
    #[allow(clippy::type_complexity)]
    fn paths(&self, stack: Vec<Frame<'a>>, labs: Vec<Lab>) -> Result<Vec<(Vec<Lab>, Cont, Vec<Frame<'a>>)>, String> {
        let mut out = Vec::new();
        let mut work: Vec<(Vec<Frame<'a>>, Vec<Lab>, usize)> = vec![(stack, labs, 0)];
        while let Some((mut stack, mut labs, mut steps)) = work.pop() {
            loop {
                steps += 1;
                if steps > 20_000 {
                    return Err(format!("source path did not reach a tick in 20000 steps: {}", fmt_path(&labs)));
                }
                enum Act<'a> {
                    Stmt(&'a Stmt, Vec<usize>),
                    One(&'a Expr, Pos, Vec<usize>),
                    Pop,
                    Back,
                }
                let act = match stack.last_mut() {
                    None => return Err("source frame stack emptied (the module loop returned)".into()),
                    Some(Frame::Seq { stmts, next, prefix }) if *next < stmts.len() => {
                        let i = *next;
                        *next += 1;
                        let mut path = prefix.clone();
                        path.push(i);
                        Act::Stmt(&stmts[i], path)
                    }
                    Some(Frame::One { expr, pos, prefix, done }) if !*done => {
                        *done = true;
                        Act::One(expr, *pos, prefix.clone())
                    }
                    Some(Frame::Seq { .. } | Frame::One { .. }) => Act::Pop,
                    Some(_) => Act::Back,
                };
                let step = match act {
                    Act::Pop => {
                        stack.pop();
                        Step::Go
                    }
                    // A loop frame on top: its body just ran off its end — the back edge.
                    Act::Back => {
                        self.back_edge(&mut stack, &mut labs, &mut work, steps)?;
                        Step::Go
                    }
                    Act::Stmt(s, path) => match s {
                        Stmt::Item(_) => Step::Go,
                        s if is_compile_time(s) => Step::Go,
                        Stmt::Expr(e, _) => self.expr(e, pos_of(s), path, &mut stack, &mut labs, &mut work, steps)?,
                        _ => {
                            push_lab(&mut labs, Lab::Run(pos_of(s)));
                            Step::Go
                        }
                    },
                    Act::One(e, pos, path) => self.expr(e, pos, path, &mut stack, &mut labs, &mut work, steps)?,
                };
                if let Step::Parked(site) = step {
                    for (extra, cont_stack, last) in lookahead(&stack) {
                        let mut l = labs.clone();
                        for x in extra {
                            push_lab(&mut l, x);
                        }
                        out.push((l, Cont::After { site: site.clone(), last }, cont_stack));
                    }
                    break;
                }
            }
        }
        Ok(out)
    }

    /// The back edge of the loop frame on top of `stack` (its body ran off its end,
    /// or a `continue` reached it).
    fn back_edge(&self, stack: &mut Vec<Frame<'a>>, labs: &mut Vec<Lab>, work: &mut Vec<(Vec<Frame<'a>>, Vec<Lab>, usize)>, steps: usize) -> Result<(), String> {
        match stack.last().unwrap().clone() {
            Frame::ModLoop => {
                stack.push(Frame::Seq { stmts: self.body, next: 0, prefix: vec![self.loop_idx, 0] });
            }
            Frame::Loop { body, prefix } => stack.push(Frame::Seq { stmts: body, next: 0, prefix }),
            Frame::While { pos, body, prefix } => {
                let mut exit = stack.clone();
                exit.pop();
                let mut el = labs.clone();
                push_lab(&mut el, Lab::Test(pos, 1));
                work.push((exit, el, steps));
                push_lab(labs, Lab::Test(pos, 0));
                stack.push(Frame::Seq { stmts: body, next: 0, prefix });
            }
            Frame::For { body, prefix, last, pos } => match last {
                Some(true) => {
                    stack.pop();
                }
                Some(false) => {
                    if let Some(Frame::For { last, .. }) = stack.last_mut() {
                        *last = None;
                    }
                    stack.push(Frame::Seq { stmts: body, next: 0, prefix });
                }
                // The iteration ended at a nested loop's exit, not at a tick (a body ending in a
                // wait, D-29): Rust's test runs here, in this resumption — `Last` leaves the
                // `for`, `More` re-enters its body. The emitted `D` tests after its increment.
                None => {
                    let mut exit = stack.clone();
                    exit.pop();
                    let mut el = labs.clone();
                    push_lab(&mut el, Lab::Count(pos, Cnt::Last));
                    work.push((exit, el, steps));
                    push_lab(labs, Lab::Count(pos, Cnt::More));
                    stack.push(Frame::Seq { stmts: body, next: 0, prefix });
                }
            },
            _ => unreachable!(),
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn expr(
        &self,
        e: &'a Expr,
        pos: Pos,
        path: Vec<usize>,
        stack: &mut Vec<Frame<'a>>,
        labs: &mut Vec<Lab>,
        work: &mut Vec<(Vec<Frame<'a>>, Vec<Lab>, usize)>,
        steps: usize,
    ) -> Result<Step, String> {
        if is_tick_expr(e) {
            return Ok(Step::Parked(path));
        }
        if !is_control(e) {
            push_lab(labs, Lab::Run(pos));
            return Ok(Step::Go);
        }
        let with = |p: &Vec<usize>, k: usize| {
            let mut v = p.clone();
            v.push(k);
            v
        };
        match e {
            Expr::If(i) => {
                let ip = pos_of(i);
                // else
                let mut es = stack.clone();
                let mut el = labs.clone();
                push_lab(&mut el, Lab::Test(ip, 1));
                if let Some((_, other)) = &i.else_branch {
                    match &**other {
                        Expr::Block(b) => es.push(Frame::Seq { stmts: &b.block.stmts, next: 0, prefix: with(&path, 1) }),
                        other => es.push(Frame::One { expr: other, pos: pos_of(other), prefix: with(&path, 1), done: false }),
                    }
                }
                work.push((es, el, steps));
                push_lab(labs, Lab::Test(ip, 0));
                stack.push(Frame::Seq { stmts: &i.then_branch.stmts, next: 0, prefix: with(&path, 0) });
            }
            Expr::Match(m) => {
                let mp = pos_of(m);
                for (k, arm) in m.arms.iter().enumerate().rev() {
                    let mut s2 = stack.clone();
                    let mut l2 = labs.clone();
                    push_lab(&mut l2, Lab::Test(mp, k));
                    match &*arm.body {
                        Expr::Block(b) => s2.push(Frame::Seq { stmts: &b.block.stmts, next: 0, prefix: with(&path, k) }),
                        other => s2.push(Frame::One { expr: other, pos: pos_of(arm), prefix: with(&with(&path, k), 0), done: false }),
                    }
                    if k == 0 {
                        *stack = s2;
                        *labs = l2;
                    } else {
                        work.push((s2, l2, steps));
                    }
                }
            }
            Expr::Loop(l) => {
                let p = with(&path, 0);
                stack.push(Frame::Loop { body: &l.body.stmts, prefix: p.clone() });
                stack.push(Frame::Seq { stmts: &l.body.stmts, next: 0, prefix: p });
            }
            Expr::While(w) => {
                let wp = pos_of(w);
                let mut el = labs.clone();
                push_lab(&mut el, Lab::Test(wp, 1));
                work.push((stack.clone(), el, steps));
                push_lab(labs, Lab::Test(wp, 0));
                let p = with(&path, 0);
                stack.push(Frame::While { pos: wp, body: &w.body.stmts, prefix: p.clone() });
                stack.push(Frame::Seq { stmts: &w.body.stmts, next: 0, prefix: p });
            }
            Expr::ForLoop(f) => {
                let fp = pos_of(f);
                let p = with(&path, 0);
                match self.fors[&fp] {
                    ForBounds::StaticEmpty => {}
                    b => {
                        if matches!(b, ForBounds::Unknown) {
                            let mut el = labs.clone();
                            push_lab(&mut el, Lab::Count(fp, Cnt::Empty));
                            work.push((stack.clone(), el, steps));
                            push_lab(labs, Lab::Count(fp, Cnt::Enter));
                        }
                        stack.push(Frame::For { pos: fp, body: &f.body.stmts, prefix: p.clone(), last: None });
                        stack.push(Frame::Seq { stmts: &f.body.stmts, next: 0, prefix: p });
                    }
                }
            }
            Expr::Block(b) => stack.push(Frame::Seq { stmts: &b.block.stmts, next: 0, prefix: path }),
            Expr::Break(_) => loop {
                match stack.pop() {
                    Some(Frame::Loop { .. } | Frame::While { .. } | Frame::For { .. }) => break,
                    Some(Frame::ModLoop) | None => return Err("`break` of the module loop".into()),
                    _ => {}
                }
            },
            Expr::Continue(_) => {
                loop {
                    match stack.last() {
                        Some(Frame::Seq { .. } | Frame::One { .. }) => {
                            stack.pop();
                        }
                        Some(Frame::For { pos, .. }) => {
                            return Err(format!("`continue` of the counted `for` at {}", fmt_pos(*pos)))
                        }
                        Some(_) => break,
                        None => return Err("`continue` outside a loop".into()),
                    }
                }
                self.back_edge(stack, labs, work, steps)?;
            }
            other => return Err(format!("unhandled control expression at {}: {}", fmt_pos(pos), kind_of(other))),
        }
        Ok(Step::Go)
    }
}

enum Step {
    Go,
    Parked(Vec<usize>),
}

fn kind_of(e: &Expr) -> &'static str {
    match e {
        Expr::If(_) => "if",
        Expr::Match(_) => "match",
        Expr::Block(_) => "block",
        _ => "other",
    }
}

/// The counted-`for` choices a tick makes when it ends iterations: walking down
/// from the top past exhausted frames, a `for` reached with no choice yet branches
/// into `More` (stop) and `Last` (the `for` exits; keep walking below it).
#[allow(clippy::type_complexity)]
fn lookahead<'a>(stack: &[Frame<'a>]) -> Vec<(Vec<Lab>, Vec<Frame<'a>>, Vec<(Pos, bool)>)> {
    fn go<'a>(stack: &[Frame<'a>], depth: usize, mut labs: Vec<Lab>, cont: Vec<Frame<'a>>, out: &mut Vec<(Vec<Lab>, Vec<Frame<'a>>, Vec<(Pos, bool)>)>) {
        let mut i = depth;
        while i > 0 {
            match &stack[i - 1] {
                Frame::Seq { stmts, next, .. } if *next >= stmts.len() => i -= 1,
                Frame::One { done: true, .. } => i -= 1,
                _ => break,
            }
        }
        if i > 0 {
            if let Frame::For { pos, last: None, .. } = &stack[i - 1] {
                let pos = *pos;
                // More: the next resumption re-enters the body.
                let mut more = cont.clone();
                if let Frame::For { last, .. } = &mut more[i - 1] {
                    *last = Some(false);
                }
                let mut ml = labs.clone();
                ml.push(Lab::Count(pos, Cnt::More));
                out.push((ml, more.clone(), flags(&more)));
                // Last: it exits, and the frames below may end iterations too.
                let mut last_c = cont;
                if let Frame::For { last, .. } = &mut last_c[i - 1] {
                    *last = Some(true);
                }
                labs.push(Lab::Count(pos, Cnt::Last));
                go(stack, i - 1, labs, last_c, out);
                return;
            }
        }
        out.push((labs, cont.clone(), flags(&cont)));
    }
    fn flags(stack: &[Frame]) -> Vec<(Pos, bool)> {
        stack
            .iter()
            .filter_map(|f| match f {
                Frame::For { pos, last: Some(l), .. } => Some((*pos, *l)),
                _ => None,
            })
            .collect()
    }
    let mut out = Vec::new();
    go(stack, stack.len(), Vec::new(), stack.to_vec(), &mut out);
    out
}

fn for_bounds(f: &syn::ExprForLoop, consts: &dyn Fn(&str) -> Option<i128>) -> ForBounds {
    let Expr::Range(r) = &*f.expr else { return ForBounds::Unknown };
    // An inclusive range (`a..=b`, D-27) is empty when `a > b`, an exclusive one when `a >= b`.
    let inclusive = matches!(r.limits, syn::RangeLimits::Closed(_));
    // A path of several segments (`T::K`, an associated const) is looked up by its full name,
    // as `ConstEnv` keys it (the transpiler folds it the same way; MEASUREMENTS §Z.3).
    fn eval(e: &Expr, consts: &dyn Fn(&str) -> Option<i128>) -> Option<i128> {
        match e {
            // A path of several segments: an associated const by its full name, else a
            // primitive's `MIN`/`MAX`/`BITS` (`i128::MIN`, MEASUREMENTS §AG).
            Expr::Path(p) if p.path.segments.len() > 1 => {
                let n = p.path.segments.iter().map(|s| s.ident.to_string()).collect::<Vec<_>>().join("::");
                consts(&n).or_else(|| copper_analysis::consts::primitive_assoc_const(&n).map(|(v, _)| v))
            }
            Expr::Paren(p) => eval(&p.expr, consts),
            Expr::Group(g) => eval(&g.expr, consts),
            Expr::Unary(syn::ExprUnary { op: syn::UnOp::Neg(_), expr, .. }) => eval(expr, consts)?.checked_neg(),
            Expr::Binary(b) if matches!(b.op, syn::BinOp::Add(_) | syn::BinOp::Sub(_) | syn::BinOp::Mul(_)) => {
                let (l, r) = (eval(&b.left, consts)?, eval(&b.right, consts)?);
                match b.op {
                    syn::BinOp::Add(_) => l.checked_add(r),
                    syn::BinOp::Sub(_) => l.checked_sub(r),
                    _ => l.checked_mul(r),
                }
            }
            other => copper_analysis::consts::eval_syn(other, consts),
        }
    }
    let eval = |e: &Expr| eval(e, consts);
    let start = match &r.start {
        Some(s) => eval(s),
        None => Some(0),
    };
    let end = r.end.as_ref().and_then(|e| eval(e));
    match (start, end) {
        (Some(a), Some(b)) if a > b || (a == b && !inclusive) => ForBounds::StaticEmpty,
        (Some(_), Some(_)) => ForBounds::StaticNonEmpty,
        _ => ForBounds::Unknown,
    }
}

// ── the emitted side: CHIR (+ SHIR) ─────────────────────────────────────────

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
struct Ctr {
    advanced: bool,
    at_end: Option<bool>,
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
struct EState {
    /// `pc` (extraction route) or the phase index (linear route).
    ctl: usize,
    started: bool,
    ctrs: BTreeMap<String, Ctr>,
}

impl std::fmt::Display for EState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}{}", self.ctl, if self.started { "" } else { " (!started)" })?;
        for (n, c) in &self.ctrs {
            write!(f, " {n}:{}{}", if c.advanced { "adv" } else { "start" }, match c.at_end {
                Some(true) => ",end",
                Some(false) => ",below",
                None => "",
            })?;
        }
        Ok(())
    }
}

enum Route<'c> {
    Extracted { pc: String, entry: Option<&'c [CHIRStmt]>, arms: BTreeMap<usize, &'c [CHIRStmt]> },
    Linear { segs: Vec<&'c [CHIRStmt]>, next: Vec<usize> },
}

enum Mapped {
    Atomic(Pos),
    Init,
    None,
}

struct Emit<'c, 's> {
    route: Route<'c>,
    src: &'s Src<'s>,
    /// I.8.5's done flag of a counted `for` (D-28, a 128-bit inclusive range): the flag's name
    /// by the `for`'s position, recorded at its declaration (`d = a > e`). Its `Ctr` stands for
    /// "passed" (`at_end`) and "entered" (`advanced`); the increment `if c == e { d = true }
    /// else { c = c + 1 }` advances it, so the exit test `if d` takes Last/More as `c > e` does
    /// (MEASUREMENTS §AG: the form was unswept until §AG, so this walker never met it).
    done_at: std::cell::RefCell<BTreeMap<Pos, String>>,
}

fn is_done_flag(n: &str) -> bool {
    n.starts_with("__copper_ctr") && n.ends_with("_done")
}

fn strip(e: &CHIRExpr) -> &CHIRExpr {
    match e {
        CHIRExpr::Resize { expr, .. } | CHIRExpr::SignCast { expr, .. } => strip(expr),
        _ => e,
    }
}

fn var_name(e: &CHIRExpr) -> Option<&str> {
    match strip(e) {
        CHIRExpr::Var(n) => Some(n),
        _ => None,
    }
}

fn is_started(e: &CHIRExpr) -> Option<bool> {
    match strip(e) {
        CHIRExpr::Var(n) if n == STARTED_REG => Some(true),
        CHIRExpr::UnOp { op: CHIRUnOp::LogicalNot, expr } if var_name(expr) == Some(STARTED_REG) => Some(false),
        _ => None,
    }
}

fn assigns_pc(stmts: &[CHIRStmt], pc: &str) -> bool {
    stmts.iter().any(|s| match s {
        CHIRStmt::Assign { target, .. } => target == pc,
        CHIRStmt::If { then_body, else_body, .. } => {
            assigns_pc(then_body, pc) || else_body.as_ref().is_some_and(|b| assigns_pc(b, pc))
        }
        CHIRStmt::Match { arms, .. } => arms.iter().any(|a| assigns_pc(&a.body, pc)),
        CHIRStmt::ForLoop { body, .. } => assigns_pc(body, pc),
        _ => false,
    })
}

fn lit_value(e: &CHIRExpr) -> Option<u128> {
    match strip(e) {
        CHIRExpr::Lit(l) => Some(l.value),
        _ => None,
    }
}

type EPath = (Vec<Lab>, EState);

impl<'c, 's> Emit<'c, 's> {
    fn pc_name(&self) -> Option<&str> {
        match &self.route {
            Route::Extracted { pc, .. } => Some(pc),
            Route::Linear { .. } => None,
        }
    }

    fn map(&self, p: Pos) -> Mapped {
        let inside = |r: &Range| r.start <= p && p < r.end || r.start == p;
        if self.src.init.iter().any(inside) {
            return Mapped::Init;
        }
        // The OUTERMOST atomic statement containing p.
        let mut best: Option<Range> = None;
        for r in self.src.atomic.iter().filter(|r| inside(r)) {
            if best.is_none_or(|b| r.start < b.start || (r.start == b.start && r.end > b.end)) {
                best = Some(*r);
            }
        }
        match best {
            Some(r) => Mapped::Atomic(r.start),
            None => Mapped::None,
        }
    }

    /// Every path of the emitted transition out of `st`.
    fn paths(&self, st: &EState) -> Result<Vec<EPath>, String> {
        let mut base = st.clone();
        base.started = true; // after any edge (checked against SHIR in `check_started`)
        let mut stack: Vec<&'c [CHIRStmt]> = Vec::new();
        match &self.route {
            Route::Extracted { entry, arms, .. } => {
                let Some(arm) = arms.get(&st.ctl) else {
                    return Err(format!("pc = {} has no arm of the `match pc` (it falls to `_`)", st.ctl));
                };
                stack.push(arm);
                if let (Some(e), false) = (entry, st.started) {
                    stack.push(e);
                }
                base.ctl = st.ctl; // held unless a path assigns pc
            }
            Route::Linear { segs, next } => {
                let n = next.len();
                stack.push(segs[st.ctl]);
                if st.ctl == 0 && st.started && !segs[n].is_empty() {
                    stack.push(segs[n]);
                }
                base.ctl = next[st.ctl];
            }
        }
        let mut out = Vec::new();
        // (remaining statement lists, top last; labels; state)
        let mut work: Vec<(Vec<&'c [CHIRStmt]>, Vec<Lab>, EState)> = vec![(stack, Vec::new(), {
            let mut s = base.clone();
            s.started = st.started; // the walk reads the CURRENT started; the successor's is set at the end
            s
        })];
        while let Some((mut stack, mut labs, mut cur)) = work.pop() {
            loop {
                let Some(top) = stack.last_mut() else {
                    cur.started = true;
                    out.push((labs, cur));
                    break;
                };
                let Some((s, rest)) = top.split_first() else {
                    stack.pop();
                    continue;
                };
                *top = rest;
                self.stmt(s, &mut stack, &mut labs, &mut cur, &mut work)?;
            }
        }
        Ok(out)
    }

    fn stmt(
        &self,
        s: &'c CHIRStmt,
        stack: &mut Vec<&'c [CHIRStmt]>,
        labs: &mut Vec<Lab>,
        cur: &mut EState,
        work: &mut Vec<(Vec<&'c [CHIRStmt]>, Vec<Lab>, EState)>,
    ) -> Result<(), String> {
        let p = (s.span().start_line, s.span().start_col);
        let empty: &'c [CHIRStmt] = &[];
        let pc = self.pc_name();
        match s {
            CHIRStmt::If { condition, then_body, else_body, .. } => {
                let else_b: &'c [CHIRStmt] = else_body.as_deref().unwrap_or(empty);
                if let Some(pol) = is_started(condition) {
                    if pol == cur.started {
                        stack.push(then_body);
                    } else {
                        stack.push(else_b);
                    }
                    return Ok(());
                }
                if let Some(bounds) = self.src.fors.get(&p) {
                    let _ = bounds;
                    // The done-flag increment: advances the flag (and the counter), no branch.
                    if let (CHIRExpr::BinOp { left, op: CHIRBinOp::Eq, .. }, [CHIRStmt::Assign { target, .. }]) =
                        (strip(condition), &then_body[..])
                    {
                        if is_done_flag(target) {
                            cur.ctrs.insert(target.clone(), Ctr { advanced: true, at_end: None });
                            if let Some(c) = var_name(left).and_then(|n| cur.ctrs.get_mut(n)) {
                                c.advanced = true;
                                c.at_end = None;
                            }
                            return Ok(());
                        }
                    }
                    let take = self.counter_test(condition, cur, p)?;
                    match take {
                        Some(b) => stack.push(if b { then_body } else { else_b }),
                        None => {
                            // An exit test whose outcome is unknown: both, with labels.
                            let v = var_name(match strip(condition) {
                                CHIRExpr::BinOp { left, .. } => left,
                                done => done,
                            })
                            .unwrap()
                            .to_string();
                            let adv = cur.ctrs[&v].advanced;
                            let mut s2 = stack.clone();
                            s2.push(else_b);
                            let mut l2 = labs.clone();
                            push_lab(&mut l2, Lab::Count(p, if adv { Cnt::More } else { Cnt::Enter }));
                            let mut c2 = cur.clone();
                            c2.ctrs.get_mut(&v).unwrap().at_end = Some(false);
                            work.push((s2, l2, c2));
                            push_lab(labs, Lab::Count(p, if adv { Cnt::Last } else { Cnt::Empty }));
                            cur.ctrs.get_mut(&v).unwrap().at_end = Some(true);
                            stack.push(then_body);
                        }
                    }
                    return Ok(());
                }
                let control = pc.is_some_and(|pc| assigns_pc(std::slice::from_ref(s), pc));
                if !control {
                    return self.atomic(p, labs, "if");
                }
                let (then_k, else_k) = if self.src.whiles.contains(&p) {
                    if !matches!(strip(condition), CHIRExpr::UnOp { op: CHIRUnOp::LogicalNot, .. }) {
                        return Err(format!("the `while` guard at {} is not a negation", fmt_pos(p)));
                    }
                    (1, 0)
                } else if self.src.ifs.contains(&p) {
                    (0, 1)
                } else {
                    return Err(format!("an emitted control `if` at {} that is no source control construct", fmt_pos(p)));
                };
                let mut s2 = stack.clone();
                s2.push(else_b);
                let mut l2 = labs.clone();
                push_lab(&mut l2, Lab::Test(p, else_k));
                work.push((s2, l2, cur.clone()));
                push_lab(labs, Lab::Test(p, then_k));
                stack.push(then_body);
            }
            CHIRStmt::Match { arms, .. } => {
                let control = pc.is_some_and(|pc| assigns_pc(std::slice::from_ref(s), pc));
                if !control {
                    return self.atomic(p, labs, "match");
                }
                if !self.src.matches.contains(&p) {
                    return Err(format!("an emitted control `match` at {} that is no source control `match`", fmt_pos(p)));
                }
                for (k, arm) in arms.iter().enumerate().skip(1) {
                    let mut s2 = stack.clone();
                    s2.push(&arm.body);
                    let mut l2 = labs.clone();
                    push_lab(&mut l2, Lab::Test(p, k));
                    work.push((s2, l2, cur.clone()));
                }
                if let Some(a0) = arms.first() {
                    push_lab(labs, Lab::Test(p, 0));
                    stack.push(&a0.body);
                }
            }
            CHIRStmt::Assign { target, value, .. } if Some(target.as_str()) == pc => {
                let v = lit_value(value).ok_or_else(|| format!("`{target}` assigned a non-literal at {}", fmt_pos(p)))?;
                cur.ctl = v as usize;
            }
            CHIRStmt::Assign { target, value, .. } | CHIRStmt::Wire { name: target, value, .. }
                if self.src.fors.contains_key(&p) =>
            {
                // A counted `for`'s synthesized declaration, increment or bound snapshot.
                let incr = matches!(strip(value), CHIRExpr::BinOp { left, op: CHIRBinOp::Add { .. }, right }
                    if var_name(left) == Some(target.as_str()) && lit_value(right) == Some(1));
                if is_done_flag(target) {
                    self.done_at.borrow_mut().insert(p, target.clone());
                }
                let c = if incr {
                    Ctr { advanced: true, at_end: None }
                } else {
                    let known = matches!(self.src.fors[&p], ForBounds::StaticNonEmpty);
                    Ctr { advanced: false, at_end: if known { Some(false) } else { None } }
                };
                cur.ctrs.insert(target.clone(), c);
            }
            CHIRStmt::AwaitTick { .. } => return Err(format!("a tick inside a state body at {}", fmt_pos(p))),
            _ => return self.atomic(p, labs, "statement"),
        }
        Ok(())
    }

    fn atomic(&self, p: Pos, labs: &mut Vec<Lab>, what: &str) -> Result<(), String> {
        match self.map(p) {
            Mapped::Atomic(sp) => {
                push_lab(labs, Lab::Run(sp));
                Ok(())
            }
            Mapped::Init => Err(format!("a pre-loop `let` lowered into a resumption ({what} at {})", fmt_pos(p))),
            Mapped::None => Err(format!("an emitted {what} at {} maps to no source statement", fmt_pos(p))),
        }
    }

    /// A counted `for`'s exit test or tick guard: `Some(outcome)` when the counter
    /// facts decide it, `None` for an exit test they do not.
    fn counter_test(&self, cond: &CHIRExpr, cur: &EState, p: Pos) -> Result<Option<bool>, String> {
        let fact = |v: &CHIRExpr| -> Result<&Ctr, String> {
            let n = var_name(v).ok_or_else(|| format!("counted-`for` test at {} on a non-variable", fmt_pos(p)))?;
            cur.ctrs.get(n).ok_or_else(|| format!("counted-`for` test at {} reads `{n}` before its declaration", fmt_pos(p)))
        };
        // The done flag's "entered" guard, `a <= e` (`control_extract`, D-28).
        let entered = || -> Result<bool, String> {
            let d = self.done_at.borrow().get(&p).cloned()
                .ok_or_else(|| format!("an `a <= e` guard at {} with no done flag", fmt_pos(p)))?;
            Ok(cur.ctrs.get(&d).ok_or_else(|| format!("done flag `{d}` read before its declaration"))?.advanced)
        };
        match strip(cond) {
            // The done flag: the counter has passed its range.
            CHIRExpr::Var(n) if is_done_flag(n) => Ok(fact(cond)?.at_end),
            CHIRExpr::BinOp { op: CHIRBinOp::Lte, .. } => Ok(Some(entered()?)),
            CHIRExpr::BinOp { left, op: CHIRBinOp::LogicalAnd, right }
                if var_name(left).is_some_and(is_done_flag) =>
            {
                let CHIRExpr::BinOp { op: CHIRBinOp::Lte, .. } = strip(right) else {
                    return Err(format!("unrecognized done-flag guard at {}", fmt_pos(p)));
                };
                let end = fact(left)?.at_end.ok_or_else(|| format!("tick guard at {} with the bound unknown", fmt_pos(p)))?;
                Ok(Some(end && entered()?))
            }
            // `c >= e` (exclusive) or `c > e` (inclusive, D-27): the counter has passed its range.
            CHIRExpr::BinOp { left, op: CHIRBinOp::Gte | CHIRBinOp::Gt, .. } => Ok(fact(left)?.at_end),
            CHIRExpr::BinOp { left, op: CHIRBinOp::Neq, .. } => Ok(Some(fact(left)?.advanced)),
            CHIRExpr::BinOp { left, op: CHIRBinOp::LogicalAnd, right } => {
                let (CHIRExpr::BinOp { left: a, op: CHIRBinOp::Gte | CHIRBinOp::Gt, .. }, CHIRExpr::BinOp { left: b, op: CHIRBinOp::Neq, .. }) =
                    (strip(left), strip(right))
                else {
                    return Err(format!("unrecognized counted-`for` guard at {}", fmt_pos(p)));
                };
                let end = fact(a)?.at_end.ok_or_else(|| format!("tick guard at {} with the bound unknown", fmt_pos(p)))?;
                Ok(Some(end && fact(b)?.advanced))
            }
            _ => Err(format!("unrecognized counted-`for` test at {}", fmt_pos(p))),
        }
    }
}

// ── one module ───────────────────────────────────────────────────────────────

#[derive(Default, Debug)]
struct Report {
    route: &'static str,
    states: usize,
    sites: usize,
    pairs: usize,
    projected: usize,
    transitions: usize,
    notes: Vec<String>,
}

fn split_top_ticks(body: &[CHIRStmt]) -> Vec<&[CHIRStmt]> {
    let mut segs = Vec::new();
    let mut start = 0;
    for (i, s) in body.iter().enumerate() {
        if matches!(s, CHIRStmt::AwaitTick { .. }) {
            segs.push(&body[start..i]);
            start = i + 1;
        }
    }
    segs.push(&body[start..]);
    segs
}

fn shir_seq(shir: &copper_core::shir::SHIRModule) -> Option<&SHIRSeqBody> {
    match &shir.body {
        SHIRBody::Sequential(s) => Some(s),
        _ => None,
    }
}

/// The SV `initial` block's assignment of `name`, as written (`8'd0`, `1'b0`).
fn sv_initial(sv: &str, name: &str) -> Option<String> {
    let mut in_initial = false;
    for line in sv.lines() {
        let t = line.trim();
        if t.starts_with("initial begin") {
            in_initial = true;
            continue;
        }
        if in_initial && t == "end" {
            in_initial = false;
        }
        if in_initial {
            if let Some(rest) = t.strip_prefix(&format!("{name} = ")) {
                return Some(rest.trim_end_matches(';').to_string());
            }
        }
    }
    None
}

fn is_zero_lit(s: &str) -> bool {
    s.rsplit(['d', 'b', 'h']).next().is_some_and(|d| !d.is_empty() && d.chars().all(|c| c == '0'))
}

fn relate(file_src: &str, f: &syn::ItemFn, consts: &dyn Fn(&str) -> Option<i128>) -> Result<Report, String> {
    let name = f.sig.ident.to_string();
    let src = Src::new(f, consts)?;
    let interp_sites: BTreeSet<Vec<usize>> = copper_ref::static_tick_sites(f).into_iter().map(|s| s.0).collect();
    if interp_sites != src.sites {
        return Err(format!("tick sites differ from copper_ref's: {:?} vs {:?}", src.sites, interp_sites));
    }
    let low = copper_codegen::lower_source_module(file_src, &name).map_err(|e| format!("does not transpile: {e}"))?;
    let CHIRBody::Sequential(seq) = &low.chir.body else { return Err("not a sequential CHIR body".into()) };
    let shir = shir_seq(&low.shir).ok_or("not a sequential SHIR body")?;
    let mut rep = Report { sites: src.sites.len(), ..Default::default() };

    let reg_init = |n: &str| shir.registers.iter().find(|r| r.name == n).map(|r| r.init.as_ref().map(|l| l.value));
    let segs = split_top_ticks(&seq.loop_body);
    let n_ticks = segs.len() - 1;
    if shir.phases.len() != n_ticks {
        return Err(format!("{} CHIR ticks but {} SHIR phases", n_ticks, shir.phases.len()));
    }

    // ── route and the emitted initial state ──
    // Which route the source calls for, decided on the source: extraction iff some
    // tick is not a top-level statement of the module loop (a user's own `match pc`
    // on a linear body is atomic, `if_tick_explicit` and its twins; §L.1).
    let wants_extraction = src.sites.iter().any(|s| s.len() != 3);
    let shape = extracted_shape(seq);
    if wants_extraction && shape.is_none() {
        return Err("a tick inside a branch or nested loop, but no extraction `match pc` in the CHIR".into());
    }
    let (route, init_ctl) = match shape.filter(|_| wants_extraction) {
        Some((pc, entry, m_arms)) => {
            rep.route = "extracted";
            if n_ticks != 1 || !segs[1].is_empty() {
                return Err("extraction route with more than one top-level tick or a trailing segment".into());
            }
            let mut arms = BTreeMap::new();
            let mut wildcard = false;
            for a in m_arms {
                match a.patterns.as_slice() {
                    [copper_core::chir::CHIRPattern::Lit(l)] => {
                        arms.insert(l.value as usize, a.body.as_slice());
                    }
                    [copper_core::chir::CHIRPattern::Wildcard] => wildcard = true,
                    _ => return Err("a `match pc` arm that is not one literal".into()),
                }
            }
            if !wildcard {
                rep.notes.push("the `match pc` has no `_` arm".into());
            }
            let init = reg_init(&pc).ok_or_else(|| format!("no SHIR register `{pc}`"))?.ok_or("P15: `pc` has no power-on value")?;
            check_sv_zero(&low.sv, &pc)?;
            rep.states = arms.len();
            (Route::Extracted { pc, entry, arms }, init as usize)
        }
        None => {
            rep.route = if n_ticks == 1 { "single-tick" } else { "linear" };
            let mut next = Vec::new();
            for ph in &shir.phases {
                let upd = ph.post_edge.iter().find(|u| u.target == "phase_r");
                let nx = match (upd, n_ticks) {
                    (None, 1) => 0,
                    (Some(u), _) => match &u.next_value {
                        SHIRExpr::Lit(l) => l.value as usize,
                        _ => return Err("phase_r's update is not a literal".into()),
                    },
                    (None, _) => return Err(format!("phase {} does not update phase_r", ph.phase_idx)),
                };
                next.push(nx);
            }
            let init = if n_ticks > 1 {
                check_sv_zero(&low.sv, "phase_r")?;
                reg_init("phase_r").ok_or("no SHIR phase_r")?.ok_or("P15: phase_r has no power-on value")? as usize
            } else {
                if reg_init("phase_r").is_some() {
                    return Err("a phase_r on a single-phase body".into());
                }
                0
            };
            // The trailing segment runs in phase 0 under `__copper_started`.
            let has_t = !segs[n_ticks].is_empty();
            match &shir.phases[0].guarded_post_edge {
                Some((g, _)) if has_t => {
                    if !matches!(g, SHIRExpr::Var(v) if v == STARTED_REG) {
                        return Err("phase 0's trailing updates are not guarded by __copper_started".into());
                    }
                }
                None if !has_t => {}
                Some(_) => return Err("guarded trailing updates with no trailing segment".into()),
                None => {
                    // A trailing segment with no register update is legal (drives only).
                }
            }
            if shir.phases.iter().skip(1).any(|p| p.guarded_post_edge.is_some()) {
                return Err("guarded trailing updates outside phase 0".into());
            }
            rep.states = n_ticks;
            // Segment 0's leading wires that are pre-loop `let`s are declarations
            // (`chir_lower` prepends them), not statements of a resumption.
            let mut segs = segs;
            let decl = segs[0]
                .iter()
                .take_while(|s| {
                    matches!(s, CHIRStmt::Wire { span, .. }
                        if src.init.iter().any(|r| r.start <= (span.start_line, span.start_col) && (span.start_line, span.start_col) < r.end))
                })
                .count();
            segs[0] = &segs[0][decl..];
            (Route::Linear { segs, next }, init)
        }
    };

    // ── __copper_started (P15) ──
    let reads_started = seq.entry_guard || matches!(&route, Route::Linear { segs, .. } if !segs[segs.len() - 1].is_empty());
    let started0 = match reg_init(STARTED_REG) {
        Some(Some(v)) => {
            check_sv_zero(&low.sv, STARTED_REG)?;
            check_started(shir)?;
            v != 0
        }
        Some(None) => return Err("P15: __copper_started has no power-on value".into()),
        None if reads_started => return Err("P15: __copper_started is read but not declared".into()),
        None => false,
    };
    if started0 {
        return Err("P15: __copper_started powers on at 1".into());
    }

    let em = Emit { route, src: &src, done_at: Default::default() };
    let init = EState { ctl: init_ctl, started: started0, ctrs: BTreeMap::new() };

    // ── the product: R, every transition ──
    let (stack0, labs0) = src.entry();
    let mut stacks: HashMap<Cont, Vec<Frame>> = HashMap::new();
    let mut r: BTreeSet<(EState, Cont)> = BTreeSet::new();
    let mut q: VecDeque<(EState, Cont)> = VecDeque::new();
    r.insert((init.clone(), Cont::Entry));
    q.push_back((init, Cont::Entry));
    while let Some((e, k)) = q.pop_front() {
        let sp = match &k {
            Cont::Entry => src.paths(stack0.clone(), labs0.clone())?,
            _ => src.paths(stacks[&k].clone(), Vec::new())?,
        };
        let ep = em.paths(&e)?;
        let mut smap: BTreeMap<Vec<Lab>, (Cont, Vec<Frame>)> = BTreeMap::new();
        for (l, c, st) in sp {
            if let Some((c0, _)) = smap.get(&l) {
                if *c0 != c {
                    return Err(format!("source paths from {k} with labels {} park at {c0} and {c}", fmt_path(&l)));
                }
            }
            smap.insert(l, (c, st));
        }
        let mut emap: BTreeMap<Vec<Lab>, EState> = BTreeMap::new();
        for (l, s) in ep {
            if let Some(s0) = emap.get(&l) {
                if *s0 != s {
                    return Err(format!("emitted paths from {e} with labels {} end at {s0} and {s}", fmt_path(&l)));
                }
            }
            emap.insert(l, s);
        }
        let only_src: Vec<_> = smap.keys().filter(|l| !emap.contains_key(*l)).collect();
        let only_em: Vec<_> = emap.keys().filter(|l| !smap.contains_key(*l)).collect();
        if !only_src.is_empty() || !only_em.is_empty() {
            let mut msg = format!("transition ({e}) ~ {k}: the path sets differ");
            for l in only_src.iter().take(4) {
                msg += &format!("\n      source only : {}", fmt_path(l));
            }
            for l in only_em.iter().take(4) {
                msg += &format!("\n      emitted only: {}", fmt_path(l));
            }
            return Err(msg);
        }
        for (l, (c, st)) in smap {
            rep.transitions += 1;
            let es = emap[&l].clone();
            stacks.entry(c.clone()).or_insert(st);
            if r.insert((es.clone(), c.clone())) {
                q.push_back((es, c));
            }
        }
    }

    // ── what R covers ──
    for (e, k) in &r {
        if !e.started && *k != Cont::Entry {
            return Err(format!("P15: ({e}) has started = 0 and is related to {k}, not Entry"));
        }
        if *k == Cont::Entry && e.started {
            return Err(format!("P15: Entry is related to a started state ({e})"));
        }
    }
    let projected: BTreeSet<(usize, Option<Vec<usize>>)> = r
        .iter()
        .map(|(e, k)| (e.ctl, match k {
            Cont::Entry => None,
            Cont::After { site, .. } => Some(site.clone()),
        }))
        .collect();
    rep.pairs = r.len();
    rep.projected = projected.len();
    let reached_ctl: BTreeSet<usize> = r.iter().map(|(e, _)| e.ctl).collect();
    let all_ctl: BTreeSet<usize> = match &em.route {
        Route::Extracted { arms, .. } => arms.keys().copied().collect(),
        Route::Linear { next, .. } => (0..next.len()).collect(),
    };
    for c in all_ctl.difference(&reached_ctl) {
        rep.notes.push(format!("emitted state {c} is in no pair (unreachable)"));
    }
    let reached_sites: BTreeSet<Vec<usize>> = r
        .iter()
        .filter_map(|(_, k)| match k {
            Cont::After { site, .. } => Some(site.clone()),
            Cont::Entry => None,
        })
        .collect();
    for s in src.sites.difference(&reached_sites) {
        if src.dead_sites.contains(s) {
            rep.notes.push(format!("site {s:?} is inside a statically empty `for` (no continuation)"));
        } else {
            return Err(format!("tick site {s:?} is in no pair of R"));
        }
    }
    Ok(rep)
}

fn check_sv_zero(sv: &str, name: &str) -> Result<(), String> {
    match sv_initial(sv, name) {
        Some(v) if is_zero_lit(&v) => Ok(()),
        Some(v) => Err(format!("P15: the emitted `initial` gives `{name}` = {v}")),
        None => Err(format!("P15: the emitted `initial` block does not state `{name}`")),
    }
}

/// `__copper_started` is set to 1 at phase 0's edge and assigned nowhere else.
fn check_started(shir: &SHIRSeqBody) -> Result<(), String> {
    for ph in &shir.phases {
        let ups: Vec<_> = ph
            .post_edge
            .iter()
            .chain(ph.guarded_post_edge.iter().flat_map(|(_, u)| u.iter()))
            .filter(|u| u.target == STARTED_REG)
            .collect();
        let ok = match (ph.phase_idx, ups.as_slice()) {
            (0, [u]) => matches!(&u.next_value, SHIRExpr::Lit(l) if l.value == 1),
            (0, _) => false,
            (_, []) => true,
            _ => false,
        };
        if !ok {
            return Err(format!("P15: __copper_started's updates in phase {} are not exactly `<= 1` at phase 0", ph.phase_idx));
        }
    }
    Ok(())
}

/// The extraction route's top-level shape `[decl wire*] [entry guard] match pc {…} tick`.
fn extracted_shape(seq: &CHIRSeqBody) -> Option<(String, Option<&[CHIRStmt]>, &[copper_core::chir::CHIRMatchArm])> {
    let body = &seq.loop_body;
    let mi = body.iter().position(|s| matches!(s, CHIRStmt::Match { .. }))?;
    let CHIRStmt::Match { scrutinee, arms, .. } = &body[mi] else { return None };
    let pc = var_name(scrutinee)?;
    if !(pc == "pc" || pc.starts_with("pc_")) || !seq.registers.iter().any(|r| r.name == pc) {
        return None;
    }
    if !arms.iter().any(|a| matches!(a.patterns.as_slice(), [copper_core::chir::CHIRPattern::Wildcard])) {
        return None;
    }
    if body.len() != mi + 2 || !matches!(body[mi + 1], CHIRStmt::AwaitTick { .. }) {
        return None;
    }
    let mut entry = None;
    for s in &body[..mi] {
        match s {
            CHIRStmt::Wire { .. } => {}
            CHIRStmt::If { condition, then_body, else_body: None, .. } if is_started(condition) == Some(false) => {
                entry = Some(then_body.as_slice());
            }
            _ => return None,
        }
    }
    Some((pc.to_string(), entry, arms))
}

// ── the corpus ───────────────────────────────────────────────────────────────

fn corpus_files() -> Vec<std::path::PathBuf> {
    fn walk(d: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        let Ok(entries) = std::fs::read_dir(d) else { return };
        for e in entries.filter_map(Result::ok) {
            let p = e.path();
            if p.is_dir() {
                if p.file_name().is_some_and(|n| n == "old") {
                    continue;
                }
                walk(&p, out);
            } else if p.extension().is_some_and(|x| x == "rs") {
                out.push(p);
            }
        }
    }
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files = Vec::new();
    walk(&root.join("tests/fixtures"), &mut files);
    walk(&root.join("examples"), &mut files);
    files.sort();
    files
}

/// `build.rs`'s `wrapper_name`, so a key here is a key of `SWEEP_MODULES`.
fn wrapper_name(path: &std::path::Path) -> String {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let rel = path.strip_prefix(root).unwrap_or(path).with_extension("");
    let s: String = rel.to_string_lossy().chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect();
    let s = s.trim_start_matches("tests_fixtures_").trim_start_matches("examples_").to_string();
    let prefix = if path.components().any(|c| c.as_os_str() == "examples") { "ex_" } else { "fx_" };
    format!("{prefix}{s}")
}

/// The file's integer constants (SPEC I.14.4's shared evaluator).
fn file_consts(file: &syn::File) -> copper_analysis::consts::ConstEnv {
    use quote::ToTokens;
    let texts: Vec<String> = file
        .items
        .iter()
        .filter(|i| matches!(i, syn::Item::Const(_) | syn::Item::Impl(_) | syn::Item::Fn(_)))
        .filter(|i| !matches!(i, syn::Item::Fn(f) if copper_ref::module::hardware_mode(f).is_some()))
        .map(|i| i.to_token_stream().to_string())
        .collect();
    copper_analysis::consts::ConstEnv::from_items(texts.iter().map(|s| s.as_str()))
}

/// The file's constants with `f`'s body `const` items (any depth) added; `None` when the body
/// declares none.
fn body_consts_env(file: &syn::File, f: &syn::ItemFn) -> Option<copper_analysis::consts::ConstEnv> {
    use quote::ToTokens;
    use syn::visit::Visit;
    struct V(Vec<String>);
    impl<'a> Visit<'a> for V {
        fn visit_item_const(&mut self, c: &'a syn::ItemConst) {
            self.0.push(c.to_token_stream().to_string());
        }
    }
    let mut v = V(Vec::new());
    v.visit_block(&f.block);
    if v.0.is_empty() {
        return None;
    }
    let mut texts: Vec<String> = file
        .items
        .iter()
        .filter(|i| matches!(i, syn::Item::Const(_) | syn::Item::Impl(_) | syn::Item::Fn(_)))
        .filter(|i| !matches!(i, syn::Item::Fn(f) if copper_ref::module::hardware_mode(f).is_some()))
        .map(|i| i.to_token_stream().to_string())
        .collect();
    texts.extend(v.0);
    Some(copper_analysis::consts::ConstEnv::from_items(texts.iter().map(|s| s.as_str())))
}

#[test]
fn the_projection_relation_holds_on_the_corpus() {
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut failures: Vec<String> = Vec::new();
    let mut uncovered: Vec<(String, String)> = Vec::new();
    let mut by_route: BTreeMap<&str, usize> = BTreeMap::new();
    let mut trivial_comb = 0usize;
    let mut structural = 0usize;
    let (mut pairs, mut transitions) = (0usize, 0usize);
    let mut lines: Vec<String> = Vec::new();
    let mut notes: Vec<String> = Vec::new();
    for path in corpus_files() {
        let text = std::fs::read_to_string(&path).unwrap();
        let Ok(file) = syn::parse_file(&text) else { continue };
        let env = file_consts(&file);
        let lookup = |n: &str| env.get(n);
        let wrapper = wrapper_name(&path);
        for item in &file.items {
            let syn::Item::Fn(f) = item else { continue };
            let Some(mode) = copper_ref::module::hardware_mode(f) else { continue };
            let key = format!("{wrapper}::{}", f.sig.ident);
            seen.insert(key.clone());
            match mode.as_str() {
                "combinational" => {
                    trivial_comb += 1;
                    continue;
                }
                "structural" => {
                    structural += 1;
                    continue;
                }
                _ => {}
            }
            // The module's own `const` items, at any depth, join the file's (both front-ends
            // read them; MEASUREMENTS §Z.3).
            let fenv = body_consts_env(&file, f);
            let flookup = |n: &str| fenv.as_ref().map_or_else(|| lookup(n), |e| e.get(n));
            let listed = UNCOVERED.iter().find(|(m, _)| *m == key);
            match relate(&text, f, &flookup) {
                Ok(rep) => {
                    if listed.is_some() {
                        failures.push(format!("{key}: listed in UNCOVERED but related — delete the row"));
                    }
                    *by_route.entry(rep.route).or_insert(0) += 1;
                    pairs += rep.pairs;
                    transitions += rep.transitions;
                    lines.push(format!(
                        "{key:<64} {:<11} {:>3} {:>3} {:>4} {:>4} {:>5}",
                        rep.route, rep.states, rep.sites, rep.projected, rep.pairs, rep.transitions
                    ));
                    for n in rep.notes {
                        notes.push(format!("{key}: {n}"));
                    }
                }
                Err(e) => match listed {
                    Some((_, why)) => uncovered.push((key, why.to_string())),
                    None => failures.push(format!("{key}: {e}")),
                },
            }
        }
    }
    println!("\nP1/P15 projection relation, per module (SPEC III.1; MEASUREMENTS §L)");
    println!("{:<64} {:<11} {:>3} {:>3} {:>4} {:>4} {:>5}", "module", "route", "st", "sit", "R|ps", "R", "trans");
    for l in &lines {
        println!("{l}");
    }
    for n in &notes {
        println!("  note: {n}");
    }
    for (k, why) in &uncovered {
        println!("  UNCOVERED {k}: {why}");
    }
    // Every module the sweep covers was examined.
    let missing: Vec<&&str> = SWEEP_MODULES.iter().filter(|m| !seen.contains(**m)).collect();
    if !missing.is_empty() {
        failures.push(format!("sweep modules this test never examined: {missing:?}"));
    }
    for (m, _) in UNCOVERED {
        if !seen.contains(*m) {
            failures.push(format!("UNCOVERED row `{m}` names no corpus module"));
        }
    }
    let related: usize = by_route.values().sum();
    let summary = format!(
        "projection relation: {related} clocked modules related ({}), {trivial_comb} combinational (no H(M) state), \
         {structural} structural (no coroutine), {} uncovered with a reason; |R| = {pairs} pairs, {transitions} \
         transitions checked; sweep modules examined: {}/{}",
        by_route.iter().map(|(r, n)| format!("{n} {r}")).collect::<Vec<_>>().join(", "),
        uncovered.len(),
        SWEEP_MODULES.iter().filter(|m| seen.contains(**m)).count(),
        SWEEP_MODULES.len()
    );
    println!("{summary}");
    // tools/regression.sh's G-I reads this (a passing test's stdout is captured).
    if let Ok(p) = std::env::var("COPPER_PROJECTION_REPORT") {
        let mut body = format!("{summary}\n");
        for n in &notes {
            body += &format!("note: {n}\n");
        }
        for (k, why) in &uncovered {
            body += &format!("UNCOVERED {k}: {why}\n");
        }
        if failures.is_empty() {
            std::fs::write(&p, body).expect("write the projection report");
        }
    }
    assert!(failures.is_empty(), "{} module(s) fail P1/P15:\n  {}", failures.len(), failures.join("\n  "));
    assert!(related > 100, "the corpus walk stopped reaching the corpus: {related} related");
}
