use std::collections::{HashMap, HashSet};

use copper_core::chir::{
    CHIRBody, CHIRExpr, CHIRLit, CHIRMemInit, CHIRModule, CHIRPattern, CHIRPort, CHIRPortDir,
    CHIRPortKind, CHIRRegDecl, CHIRSeqBody, CHIRStmt, CHIRStructuralBody, CHIRSubmoduleInst,
    CHIRType, Width,
};
use copper_core::frontend_ir::SourceSpan;
use copper_core::shir::{
    SHIRBody, SHIRCaseArm, SHIRCombBody, SHIRExpr, SHIRLit, SHIRLowerError, SHIRMatchArm,
    SHIRMemInit,
    SHIRMemory, SHIRModule, SHIRPattern, SHIRPhase, SHIRPort,
    SHIRPortDir, SHIRPortKind, SHIRReg, SHIRRegUpdate, SHIRSeqBody, SHIRStmt,
    SHIRStructuralBody, SHIRSubmoduleInst,
};

// ── Public entry point ────────────────────────────────────────────────────────

pub fn lower_to_shir(chir: &CHIRModule) -> Result<SHIRModule, SHIRLowerError> {
    let ports = lower_ports(&chir.ports);

    let body = match &chir.body {
        CHIRBody::Combinational(comb) => {
            SHIRBody::Combinational(lower_comb_body(comb)?)
        }
        CHIRBody::Sequential(seq) => {
            SHIRBody::Sequential(lower_seq_body(seq, chir.span)?)
        }
        CHIRBody::Structural(st) => {
            SHIRBody::Structural(lower_structural_body(st)?)
        }
    };

    Ok(SHIRModule {
        name: chir.name.clone(),
        params: chir.params.clone(),
        localparams: chir.localparams.clone(),
        ports,
        body,
        span: chir.span,
    })
}

// ── Port lowering ─────────────────────────────────────────────────────────────

fn lower_ports(ports: &[CHIRPort]) -> Vec<SHIRPort> {
    ports.iter().map(|p| SHIRPort {
        name: p.name.clone(),
        direction: match p.direction {
            CHIRPortDir::Input => SHIRPortDir::Input,
            CHIRPortDir::Output => SHIRPortDir::Output,
        },
        kind: match &p.kind {
            CHIRPortKind::Clock { .. } => SHIRPortKind::Clock,
            CHIRPortKind::Data { ty } => SHIRPortKind::Data { ty: ty.clone() },
        },
        registered: p.registered,
        span: p.span,
    }).collect()
}

// ── Combinational body ────────────────────────────────────────────────────────

fn lower_comb_body(
    comb: &copper_core::chir::CHIRCombBody,
) -> Result<SHIRCombBody, SHIRLowerError> {
    let submodules = comb.submodules.iter()
        .map(lower_submodule)
        .collect::<Result<_, _>>()?;

    // A combinational local may be *reassigned* (`acc = acc + 1`, incl. inside a
    // loop). Such an Assign becomes another blocking assign to the same signal;
    // its declared type comes from the original `let` wire, collected here.
    let mut wire_types = HashMap::new();
    collect_wire_types(&comb.stmts, &mut wire_types);

    let stmts = lower_stmt_list(
        &comb.stmts,
        &std::collections::HashSet::new(),
        &HashMap::new(),
        &wire_types,
        &mut Forwarding::disabled(),
    )?;

    Ok(SHIRCombBody { submodules, stmts })
}

/// Collect every combinational wire's declared type (`let` bindings), recursing
/// into conditional and loop bodies.
fn collect_wire_types(
    stmts: &[CHIRStmt],
    out: &mut HashMap<String, copper_core::chir::CHIRType>,
) {
    for stmt in stmts {
        match stmt {
            CHIRStmt::Wire { name, ty, .. } => { out.insert(name.clone(), ty.clone()); }
            CHIRStmt::If { then_body, else_body, .. } => {
                collect_wire_types(then_body, out);
                if let Some(eb) = else_body { collect_wire_types(eb, out); }
            }
            CHIRStmt::Match { arms, .. } => {
                for a in arms { collect_wire_types(&a.body, out); }
            }
            CHIRStmt::ForLoop { body, .. } => collect_wire_types(body, out),
            _ => {}
        }
    }
}

/// The value each register holds at the current point of ONE segment, given the
/// assignments already executed in it.
///
/// # What it is for
///
/// A register assignment emits no statement on the pre-edge path — it becomes a
/// non-blocking update at the clock edge. So a **registered** output written after
/// it, which is also sampled AT that edge, would read the register's pre-edge value
/// and land a full cycle late. Without forwarding,
///
/// ```text
/// n = n + step.read();   acc.write(n);      // and its reverse
/// acc.write(n);          n = n + step.read();
/// ```
///
/// lower to the SAME SystemVerilog (`acc <= n; n <= n + step;`) — two programs with
/// different meanings, one silently wrong by a cycle. `TODO` records this as cause
/// L, with the measured sim ≢ SV trace.
///
/// # Where the forwarded value goes
///
/// Every output of a clocked body is a `RegOut` (SPEC II.1 R8), emitted as
/// `o <= <expr>` inside `always_ff` and so evaluated with PRE-edge register values:
/// a `PortDrive`'s value is the forwarded one, and so is a branch's
/// `edge_condition` (its `always_ff` copy). A branch's plain `condition` stays
/// unforwarded, for the `always_comb` copy that computes its wires. Until Phase C
/// step 4 (2026-11-06) a drive also carried an unforwarded form for the plain-`Out`
/// continuous `assign` (read after the edge — forwarding it applied the update
/// twice, measured on `lfsr`), chosen in `vlir_lower`; R8 removed that form.
///
/// # Why advancing it is delegated
///
/// [`Forwarding::advance`] calls `extract_updates_from_stmts` — the same walk that
/// computes the registers' own next-state expressions — rather than reimplementing
/// the rule. A second implementation would have to agree with that one about
/// `if`/`match` branch merging forever, and two halves of a lowering drifting apart
/// is this pipeline's most persistent bug class (`find_tick_in_expr` had just done
/// exactly that). The returned updates are discarded; only the map is wanted.
///
#[derive(Clone)]
struct Forwarding {
    /// `None` on the **combinational** path, where a reassignment lowers to a real
    /// blocking assign and SystemVerilog already sequences it — substituting there
    /// would duplicate work the language does.
    map: Option<HashMap<String, SHIRExpr>>,
    reg_names: HashSet<String>,
    span: SourceSpan,
}

impl Forwarding {
    /// The combinational path: no substitution, and assignments are not recorded.
    fn disabled() -> Self {
        Forwarding {
            map: None,
            reg_names: HashSet::new(),
            span: SourceSpan::default(),
        }
    }

    /// A fresh map for one sequential segment. Segments do not share it: a register
    /// assigned before a tick is *latched* by that tick, so the next segment reads
    /// the register itself, not the expression that fed it.
    fn for_segment(
        reg_names: HashSet<String>,
        span: SourceSpan,
    ) -> Self {
        Forwarding {
            map: Some(HashMap::new()),
            reg_names,
            span,
        }
    }

    /// The pre-edge form of an expression: registers this segment has assigned stand
    /// for the values it assigned them, and its `let` wires are inlined where that
    /// makes a difference. Returns the expression untouched on the combinational
    /// path, and wherever the segment has assigned nothing the expression reads.
    fn at_edge(&self, expr: &SHIRExpr) -> SHIRExpr {
        match &self.map {
            None => expr.clone(),
            Some(map) => subst_vars(expr.clone(), map),
        }
    }

    /// Step past one statement, recording what it assigns. A no-op when disabled.
    fn advance(
        &mut self,
        stmt: &CHIRStmt,
        renames: &HashMap<String, String>,
    ) -> Result<(), SHIRLowerError> {
        if let Some(map) = self.map.as_mut() {
            extract_updates_from_stmts(
                std::slice::from_ref(stmt),
                &self.reg_names,
                self.span,
                renames,
                map,
            )?;
        }
        Ok(())
    }
}

/// Lower a CHIR statement list into SHIR statements, handling wire promotion renames.
/// Used in both comb body lowering and pre_edge lowering. `wire_types` names the
/// combinational wires that a reassignment may target (empty for the sequential
/// pre-edge path, where an `Assign` is a register update handled elsewhere).
/// A statement list that contains at least one memory staging statement and
/// nothing except staging, wires, and staging-only sub-branches — the shape
/// whose gating condition must take the EDGE form (see the If arm below).
fn is_staging_only(stmts: &[SHIRStmt]) -> bool {
    fn all_staging(stmts: &[SHIRStmt], has_mem: &mut bool) -> bool {
        stmts.iter().all(|s| match s {
            SHIRStmt::MemRead { .. } | SHIRStmt::MemWrite { .. } => {
                *has_mem = true;
                true
            }
            SHIRStmt::Wire { .. } => true,
            SHIRStmt::If { then_stmts, else_stmts, .. } => {
                all_staging(then_stmts, has_mem)
                    && else_stmts.as_ref().is_none_or(|e| all_staging(e, has_mem))
            }
            _ => false,
        })
    }
    let mut has_mem = false;
    all_staging(stmts, &mut has_mem) && has_mem
}

fn lower_stmt_list(
    stmts: &[CHIRStmt],
    promoted_names: &std::collections::HashSet<String>,
    renames: &HashMap<String, String>,
    wire_types: &HashMap<String, copper_core::chir::CHIRType>,
    fwd: &mut Forwarding,
) -> Result<Vec<SHIRStmt>, SHIRLowerError> {
    let mut out = Vec::new();
    for stmt in stmts {
        match stmt {
            // A reassignment of a known combinational wire → another blocking
            // assign to it (`name = value;`). The single `logic` declaration is
            // deduplicated downstream, so repeated assigns share one wire.
            //
            // EVERY read below goes through `fwd.at_edge`: a register this segment
            // assigned earlier is read as the value it was assigned (SPEC I.3.4,
            // I.7.3), in the `always_comb` statements as in the edge-sampled ones.
            // Under one resumption per cycle (D-8) there is no other instant for a
            // read to see. Until Phase C step 5 (2026-11-06) only the edge-sampled
            // consumers (a drive, a staging, the `always_ff` copy of a test) were
            // forwarded; a wire, an element assignment and the combinational copy
            // of a test read the committed register, which a drive's inlining hid
            // until a value reached it through an element-assigned array (II.3
            // (f5) and its family, MEASUREMENTS §J.3).
            CHIRStmt::Assign { target, value, .. } if wire_types.contains_key(target) => {
                out.push(SHIRStmt::Wire {
                    name: target.clone(),
                    ty: wire_types[target].clone(),
                    value: fwd.at_edge(&rename_vars(lower_expr(value)?, renames)),
                });
            }
            CHIRStmt::Wire { name, ty, value, .. } => {
                out.push(SHIRStmt::Wire {
                    name: name.clone(),
                    ty: ty.clone(),
                    value: fwd.at_edge(&rename_vars(lower_expr(value)?, renames)),
                });
            }
            CHIRStmt::PortWrite { port_name, value, .. } => {
                let value = rename_vars(lower_expr(value)?, renames);
                // The drive is sampled at the edge: its forwarded value.
                let value = fwd.at_edge(&value);
                out.push(SHIRStmt::PortDrive { port_name: port_name.clone(), value });
            }
            CHIRStmt::If { condition, then_body, else_body, .. } => {
                // The condition is read BEFORE either branch runs, so it resolves
                // against the forwarding as it stands here. Each branch then gets
                // its own copy — an assignment in one arm must not leak into the
                // other. Merging the two back afterwards is `advance`'s job.
                //
                // Both forms travel on: the branch is split between `always_comb`
                // (its wires) and `always_ff` (its drives) by
                // `vlir_lower::split_output_reg`. Emitting the unforwarded test in
                // the `always_ff` copy is `TODO` cause N — it made
                // `c = c + 1; if c == 3 { … }` fire a cycle late.
                let arm_fwd = fwd.clone();
                let edge_condition = fwd.at_edge(&rename_vars(lower_expr(condition)?, renames));
                let condition = edge_condition.clone();
                let then_stmts =
                    lower_stmt_list(then_body, promoted_names, renames, wire_types, &mut arm_fwd.clone())?;
                let else_stmts = else_body.as_ref()
                    .map(|eb| lower_stmt_list(eb, promoted_names, renames, wire_types, &mut arm_fwd.clone()))
                    .transpose()?;
                // A branch that exists only to GATE memory staging is edge-
                // sampled through and through: its staged values already take
                // the edge form (see the MemRead/MemWrite arms), and the ENABLE
                // the branch structure becomes must switch in the same phase —
                // a comb-form guard opened one statement-order generation late
                // (the pipelined CPU staged the store's data from the committed
                // EX/MEM latch but its enable from the previous one). Applies
                // only when the branch holds nothing but staging (and wires,
                // which either feed the staging and are substituted through, or
                // are pruned as dead).
                let condition = if is_staging_only(&then_stmts)
                    && else_stmts.as_ref().is_none_or(|e| is_staging_only(e) || e.is_empty())
                {
                    edge_condition.clone()
                } else {
                    condition
                };
                if !then_stmts.is_empty() || else_stmts.as_ref().map_or(false, |e| !e.is_empty()) {
                    out.push(SHIRStmt::If { condition, edge_condition, then_stmts, else_stmts });
                }
            }
            CHIRStmt::Match { scrutinee, arms, .. } => {
                // Both forms, for the reason given on the `If` arm above.
                let arm_fwd = fwd.clone();
                let edge_scrutinee = fwd.at_edge(&rename_vars(lower_expr(scrutinee)?, renames));
                let scrutinee = edge_scrutinee.clone();
                let shir_arms = arms.iter()
                    .map(|arm| {
                        let stmts = lower_stmt_list(
                            &arm.body, promoted_names, renames, wire_types, &mut arm_fwd.clone(),
                        )?;
                        Ok(SHIRMatchArm {
                            patterns: arm.patterns.iter()
                                .map(|p| lower_pattern(p))
                                .collect::<Result<_, _>>()?,
                            guard: arm.guard.as_ref()
                                .map(|g| Ok(arm_fwd.at_edge(&rename_vars(lower_expr(g)?, renames))))
                                .transpose()?,
                            stmts,
                        })
                    })
                    .collect::<Result<Vec<_>, SHIRLowerError>>()?;
                if shir_arms.iter().any(|a| !a.stmts.is_empty()) {
                    out.push(SHIRStmt::Match { scrutinee, edge_scrutinee, arms: shir_arms });
                }
            }
            CHIRStmt::ForLoop { var, start, end, body, .. } => {
                let body_stmts =
                    lower_stmt_list(body, promoted_names, renames, wire_types, &mut fwd.clone())?;
                if !body_stmts.is_empty() {
                    out.push(SHIRStmt::ForLoop {
                        var: var.clone(),
                        start: fwd.at_edge(&rename_vars(lower_expr(start)?, renames)),
                        end: fwd.at_edge(&rename_vars(lower_expr(end)?, renames)),
                        body: body_stmts,
                    });
                }
            }
            CHIRStmt::IndexAssign { base, index, value, .. } => {
                out.push(SHIRStmt::IndexAssign {
                    base: base.clone(),
                    index: fwd.at_edge(&rename_vars(lower_expr(index)?, renames)),
                    value: fwd.at_edge(&rename_vars(lower_expr(value)?, renames)),
                });
            }
            // Memory accesses stage the address/data buses that this segment's
            // clock edge captures. They ride along with the pre-edge statements so
            // their surrounding `if` structure (the port enable) is preserved.
            //
            // Their expressions take the EDGE form: the staging nets exist only
            // to be sampled at the edge (the capture registers, the write
            // pipeline, an owner's array), so a register assigned earlier in
            // the segment must appear as the value it was assigned — exactly a
            // `PortDrive`'s value reasoning. The pipelined CPU is the
            // measured witness: it stages the NEXT fetch with the pc it just
            // committed (`pc = new_pc; … read(pc >> 2)`), and the unforwarded
            // form fetched one instruction behind, forever.
            CHIRStmt::MemRead { mem, port, addr, .. } => {
                out.push(SHIRStmt::MemRead {
                    mem: mem.clone(),
                    port: *port,
                    addr: fwd.at_edge(&rename_vars(lower_expr(addr)?, renames)),
                });
            }
            CHIRStmt::MemWrite { mem, port, addr, value, .. } => {
                out.push(SHIRStmt::MemWrite {
                    mem: mem.clone(),
                    port: *port,
                    addr: fwd.at_edge(&rename_vars(lower_expr(addr)?, renames)),
                    value: fwd.at_edge(&rename_vars(lower_expr(value)?, renames)),
                });
            }
            // Assign (to a register), AwaitTick — not pre_edge statements. A
            // register assign still has to be STEPPED OVER below, so later reads in
            // this segment see the value it produced.
            _ => {}
        }
        fwd.advance(stmt, renames)?;
    }
    Ok(out)
}

fn lower_submodule(sub: &CHIRSubmoduleInst) -> Result<SHIRSubmoduleInst, SHIRLowerError> {
    let inputs = sub.inputs.iter()
        .map(|(name, expr)| Ok((name.clone(), lower_expr(expr)?)))
        .collect::<Result<Vec<_>, SHIRLowerError>>()?;
    Ok(SHIRSubmoduleInst {
        inst_name: sub.inst_name.clone(),
        module_name: sub.module_name.clone(),
        inputs,
        output_wire: sub.output_wire.clone(),
        output_ty: sub.output_ty.clone(),
        clocks: sub.clocks.clone(),
        port_nets: sub.port_nets.clone(),
        output_port: sub.output_port.clone(),
    })
}

/// Structural body → SHIR. No timing regions to derive; nets and submodule
/// instances pass through 1:1.
fn lower_structural_body(st: &CHIRStructuralBody) -> Result<SHIRStructuralBody, SHIRLowerError> {
    let submodules = st.submodules.iter().map(lower_submodule).collect::<Result<Vec<_>, _>>()?;
    Ok(SHIRStructuralBody {
        nets: st.nets.clone(),
        submodules,
    })
}

// ── Sequential body ───────────────────────────────────────────────────────────

/// The guard register of the trailing region and of the pre-loop statements
/// (`lower_seq_body`, step 10): 0 until the first edge, 1 after it. A synthesized
/// name, like the extraction route's `__copper_ctr*` counters.
use copper_core::chir::STARTED_REG;

fn lower_seq_body(
    seq: &CHIRSeqBody,
    module_span: SourceSpan,
) -> Result<SHIRSeqBody, SHIRLowerError> {
    // Step 1: Validate CHIR preconditions
    validate_seq_chir(seq, module_span)?;

    // Step 2: Split loop_body into segments at AwaitTick boundaries
    let segments = split_at_ticks(&seq.loop_body);
    let n_ticks = segments.len() - 1; // N ticks → N+1 segments (0..=N)

    if n_ticks == 0 {
        return Err(SHIRLowerError::NoTick { span: module_span });
    }

    // Step 3: Build wire-to-segment map for register promotion analysis
    // Maps wire name → segment index where it was declared
    let wire_segment: HashMap<String, usize> = segments.iter().enumerate()
        .flat_map(|(seg_idx, stmts)| {
            stmts.iter().filter_map(move |s| {
                if let CHIRStmt::Wire { name, .. } = s {
                    Some((name.clone(), seg_idx))
                } else {
                    None
                }
            })
        })
        .collect();

    // Step 7: Find wires that need promotion (used in a different phase than declared)
    let promoted_wires: Vec<_> = find_promoted_wires(&segments, &wire_segment)
        .into_iter()
        .filter(|(name, _, _)| !seq.pre_loop_constants.contains(name))
        .collect();

    // Step 8: Lower registers (from CHIR register declarations)
    let mut registers: Vec<SHIRReg> = seq.registers.iter()
        .map(|r| lower_reg_decl(r))
        .collect::<Result<_, _>>()?;

    // Add promoted wire registers. A promoted wire with no literal initializer is a
    // loop-born value (`let product = a.read() * b.read(); tick;`) whose flop the
    // program assigns before it reads — but a registered output can forward the
    // flop before that, so its power-on value is observable: zero, like every
    // register codegen synthesizes without a Rust initializer (2026-10-29).
    for (wire_name, wire_ty, wire_init) in &promoted_wires {
        registers.push(SHIRReg {
            name: format!("{}_r", wire_name),
            ty: wire_ty.clone(),
            init: wire_init
                .clone()
                .or_else(|| Some(SHIRLit { ty: wire_ty.clone(), value: 0 })),
            init_param: None,
        });
    }

    let promoted_names: HashSet<String> = promoted_wires.iter()
        .map(|(name, _, _)| name.clone())
        .collect();

    // Step 9: Lower submodules
    let submodules = seq.submodules.iter()
        .map(lower_submodule)
        .collect::<Result<_, _>>()?;

    // Step 10: Build phases.
    //
    // One phase per tick: phase k runs segment k, the statements between tick k-1
    // and tick k, and its updates commit at the edge tick k awaits. The TRAILING
    // region T (segment n_ticks, after the last tick) is the HEAD of the next
    // resumption (SPEC I.0.2, D-8): the resumption that ends at the edge closing
    // phase 0 runs T and then segment 0, reading the inputs of that cycle, so T
    // lowers into phase 0 AHEAD of segment 0, with one forwarding walk across both
    // (T's register updates forwarded into segment 0's reads and drives), and every
    // drive takes its forwarded value. The FIRST resumption runs the pre-loop
    // statements and segment 0 only, so T is guarded by `__copper_started`, a
    // one-bit register powered on at 0 and set at phase 0's edge.
    //
    // Until Phase C step 4 (2026-11-06) T was FOLDED: its register updates went to
    // the last phase's edge (one edge early — so the memory read it observed had to
    // be the combinational one), its `RegOut` drives read the committed register
    // instead of the forwarded value, and nothing guarded edge 1. That encoded the
    // two-job reading, where T ran after the edge that opened its cycle; under one
    // job it is wrong whenever T reads an input (`regout_trailing_single_tick`,
    // P12) and at edge 1 whenever T has an effect (`trailing_fold`, II.3 (f4)).
    // MEASUREMENTS §H.
    let trailing = &segments[n_ticks];
    let has_trailing = !trailing.is_empty();
    // The pre-loop statements, guarded by `!__copper_started` in segment 0 by
    // `chir_lower` (SPEC I.9.2's entry), read the same bit.
    let has_started = has_trailing || seq.entry_guard;
    let started_ty = CHIRType::UInt { width: Width::Concrete(1) };
    let mut phases: Vec<SHIRPhase> = Vec::new();
    // `__copper_started` is read (never assigned) by the walks below; the
    // register-name set only matters for assigned names.
    let reg_names = reg_name_set(&seq.registers, &promoted_names);

    for phase_idx in 0..n_ticks {
        // Renames: a promoted wire is read through its register in every segment
        // after the one that declares it.
        let renames_for = |seg_idx: usize| -> HashMap<String, String> {
            promoted_wires.iter()
                .filter(|(wire_name, _, _)| wire_segment[wire_name.as_str()] < seg_idx)
                .map(|(wire_name, _, _)| (wire_name.clone(), format!("{}_r", wire_name)))
                .collect()
        };

        // The resumption's statements in execution order, each with its rename
        // scope: T (guarded) then segment 0 in phase 0, segment k alone otherwise.
        let mut parts: Vec<(Vec<CHIRStmt>, HashMap<String, String>)> = Vec::new();
        if phase_idx == 0 && has_trailing {
            parts.push((
                vec![CHIRStmt::If {
                    condition: CHIRExpr::Var(STARTED_REG.to_string()),
                    then_body: trailing.clone(),
                    else_body: None,
                    span: module_span,
                }],
                renames_for(n_ticks),
            ));
        }
        parts.push((segments[phase_idx].clone(), renames_for(phase_idx)));

        let mut fwd = Forwarding::for_segment(reg_names.clone(), module_span);
        let mut fwd_map: HashMap<String, SHIRExpr> = HashMap::new();
        let mut pre_edge = Vec::new();
        let mut post_edge = Vec::new();
        // The trailing region's own updates, emitted under the guard rather than
        // as `guard ? v : r` muxes, so a `match` next-state keeps its `case`
        // (`lower_reg_updates` lifts only a top-level one). The walk above over
        // the guarded `if` supplies the forwarding segment 0 reads; this walk
        // over the unguarded region supplies the values, from the same function.
        let guarded_post_edge = if phase_idx == 0 && has_trailing {
            let renames = renames_for(n_ticks);
            Some((
                SHIRExpr::Var(STARTED_REG.to_string()),
                extract_reg_updates_shared(
                    trailing,
                    &seq.registers,
                    &promoted_names,
                    module_span,
                    &renames,
                    &mut HashMap::new(),
                )?,
            ))
        } else {
            None
        };
        for (i, (stmts, renames)) in parts.iter().enumerate() {
            let is_guarded_trailing = guarded_post_edge.is_some() && i == 0;
            let mut wire_types = HashMap::new();
            collect_wire_types(stmts, &mut wire_types);
            pre_edge.extend(lower_stmt_list(stmts, &promoted_names, renames, &wire_types, &mut fwd)?);
            let updates = extract_reg_updates_shared(
                stmts,
                &seq.registers,
                &promoted_names,
                module_span,
                renames,
                &mut fwd_map,
            )?;
            // The guarded trailing region's updates are `guarded_post_edge`; this
            // walk only advanced the forwarding map past them.
            if !is_guarded_trailing {
                post_edge.extend(updates);
            }
        }

        // Capture promoted wires into their registers at this phase's clock edge
        for (wire_name, _ty, _init) in &promoted_wires {
            if wire_segment[wire_name.as_str()] == phase_idx {
                post_edge.push(SHIRRegUpdate {
                    target: format!("{}_r", wire_name),
                    next_value: SHIRExpr::Var(wire_name.clone()),
                });
            }
        }

        if phase_idx == 0 && has_started {
            post_edge.push(SHIRRegUpdate {
                target: STARTED_REG.to_string(),
                next_value: SHIRExpr::Lit(SHIRLit { ty: started_ty.clone(), value: 1 }),
            });
        }

        if n_ticks > 1 {
            // Phase advance (wraps back to 0 from N-1)
            let next_phase = (phase_idx + 1) % n_ticks;
            let phase_width = phase_register_width(n_ticks);
            post_edge.push(SHIRRegUpdate {
                target: "phase_r".to_string(),
                next_value: SHIRExpr::Lit(SHIRLit {
                    ty: CHIRType::UInt { width: Width::Concrete(phase_width) },
                    value: next_phase as u128,
                }),
            });
        }

        phases.push(SHIRPhase { phase_idx, pre_edge, guarded_post_edge, post_edge });
    }

    if has_started {
        registers.push(SHIRReg {
            name: STARTED_REG.to_string(),
            ty: started_ty.clone(),
            init: Some(SHIRLit { ty: started_ty, value: 0 }),
            init_param: None,
        });
    }
    if n_ticks > 1 {
        let phase_width = phase_register_width(n_ticks);
        registers.push(SHIRReg {
            name: "phase_r".to_string(),
            ty: CHIRType::UInt { width: Width::Concrete(phase_width) },
            init: Some(SHIRLit { ty: CHIRType::UInt { width: Width::Concrete(phase_width) }, value: 0 }),
            init_param: None,
        });
    }

    Ok(SHIRSeqBody {
        clock: seq.clock.clone(),
        registers,
        memories: seq.memories.iter().map(|m| Ok(SHIRMemory {
            received: m.received,
            name: m.name.clone(),
            elem_ty: m.elem_ty.clone(),
            depth: m.depth,
            read_ports: m.read_ports,
            write_ports: m.write_ports,
            read_lat: m.read_lat,
            write_lat: m.write_lat,
            init: m.init.as_ref().map(lower_mem_init).transpose()?,
            write_mode: m.write_mode,
        })).collect::<Result<Vec<_>, SHIRLowerError>>()?,
        submodules,
        phases,
    })
}

// ── Helpers: segment splitting ────────────────────────────────────────────────

/// Split `loop_body` into segments at each `AwaitTick` boundary.
///
/// N `AwaitTick` statements → N+1 segments:
///   `[seg_0, seg_1, ..., seg_N]`
/// where `seg_k` contains the statements between tick k-1 and tick k
/// (and `seg_0` is before the first tick, `seg_N` is after the last tick).
fn split_at_ticks(stmts: &[CHIRStmt]) -> Vec<Vec<CHIRStmt>> {
    let mut segments: Vec<Vec<CHIRStmt>> = vec![Vec::new()];
    for stmt in stmts {
        if matches!(stmt, CHIRStmt::AwaitTick { .. }) {
            segments.push(Vec::new());
        } else {
            segments.last_mut().unwrap().push(stmt.clone());
        }
    }
    segments
}

/// Minimum bit width needed to hold values 0..n_ticks-1.
fn phase_register_width(n_ticks: usize) -> usize {
    if n_ticks <= 1 { 1 }
    else { (usize::BITS - (n_ticks - 1).leading_zeros()) as usize }
}

// ── Helpers: validation ───────────────────────────────────────────────────────

fn validate_seq_chir(seq: &CHIRSeqBody, span: SourceSpan) -> Result<(), SHIRLowerError> {
    // Verify clock consistency in AwaitTick nodes
    for stmt in &seq.loop_body {
        if let CHIRStmt::AwaitTick { clock, span: s } = stmt {
            if clock != &seq.clock {
                return Err(SHIRLowerError::CrossClockTick {
                    expected: seq.clock.clone(),
                    found: clock.clone(),
                    span: *s,
                });
            }
        }
        // Check for ticks inside branches (should be caught by Phase B, but defensive)
        check_no_tick_in_branch(stmt, span)?;
    }
    Ok(())
}

fn check_no_tick_in_branch(stmt: &CHIRStmt, span: SourceSpan) -> Result<(), SHIRLowerError> {
    match stmt {
        CHIRStmt::If { then_body, else_body, span: _s, .. } => {
            for s in then_body {
                if matches!(s, CHIRStmt::AwaitTick { .. }) {
                    return Err(SHIRLowerError::TickInsideBranch { span: *s.span() });
                }
                check_no_tick_in_branch(s, span)?;
            }
            if let Some(eb) = else_body {
                for s in eb {
                    if matches!(s, CHIRStmt::AwaitTick { .. }) {
                        return Err(SHIRLowerError::TickInsideBranch { span: *s.span() });
                    }
                    check_no_tick_in_branch(s, span)?;
                }
            }
            Ok(())
        }
        CHIRStmt::Match { arms, .. } => {
            for arm in arms {
                for s in &arm.body {
                    if matches!(s, CHIRStmt::AwaitTick { .. }) {
                        return Err(SHIRLowerError::TickInsideBranch { span: *s.span() });
                    }
                    check_no_tick_in_branch(s, span)?;
                }
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

// ── Helpers: register names ──────────────────────────────────────

/// The names the segment walks treat as registers: declared registers plus the
/// `<name>_r` a promoted cross-phase wire becomes.
fn reg_name_set(registers: &[CHIRRegDecl], promoted_names: &HashSet<String>) -> HashSet<String> {
    registers.iter()
        .map(|r| r.name.clone())
        .chain(promoted_names.iter().map(|n| format!("{}_r", n)))
        .collect()
}

// ── Helpers: register update extraction ──────────────────────────────────────

/// Extract register updates from a segment's statements.
///
/// Applies `renames` (promoted wire x→x_r) and sequential forwarding so that
/// later assigns within the same segment see the updated values of earlier assigns.
/// The register updates a segment contributes to its phase's clock edge, with the
/// forwarding map supplied by the caller so **two segments that commit at the same
/// clock edge** share it.
///
/// The trailing segment and the phase before it are exactly that pair: the trailing
/// statements execute in the post-edge settle of that edge, so they read the values
/// the edge has just applied. Without sharing, `r = a.read(); tick; w = r + 1;`
/// emits `r <= a; w <= r + 1` — `w` from the PRE-edge `r`, a cycle behind what the
/// simulator computes. The single-tick path's comment has claimed this sharing since
/// the path was written; the code made a fresh map per call and neither path did it.
/// Found 2026-08-25 by a corpus differential case for the trailing-statement
/// semantics.
fn extract_reg_updates_shared(
    stmts: &[CHIRStmt],
    registers: &[CHIRRegDecl],
    promoted_names: &HashSet<String>,
    span: SourceSpan,
    renames: &HashMap<String, String>,
    forwarding: &mut HashMap<String, SHIRExpr>,
) -> Result<Vec<SHIRRegUpdate>, SHIRLowerError> {
    let reg_names = reg_name_set(registers, promoted_names);
    extract_updates_from_stmts(stmts, &reg_names, span, renames, forwarding)
}

/// Recursive helper that processes statements in order, threading a forwarding map
/// so sequential assigns within a segment see each other's new values (matching Rust semantics).
///
/// `forwarding`: maps register name → its current effective next-value expression.
/// `renames`: maps promoted wire name → `<name>_r` for the current phase.
/// The value a register ends a branch with: its LAST assignment there, if any.
///
/// A branch's updates are recorded in source order, so a register written more
/// than once appears more than once and only the final write is the branch's
/// result. Reading the first instead drops every later write — silently, since
/// the earlier one is still a well-typed value.
fn last_update_for(updates: &[SHIRRegUpdate], target: &str) -> Option<SHIRExpr> {
    updates
        .iter()
        .rev()
        .find(|u| u.target == target)
        .map(|u| u.next_value.clone())
}

fn extract_updates_from_stmts(
    stmts: &[CHIRStmt],
    reg_names: &HashSet<String>,
    _span: SourceSpan,
    renames: &HashMap<String, String>,
    forwarding: &mut HashMap<String, SHIRExpr>,
) -> Result<Vec<SHIRRegUpdate>, SHIRLowerError> {
    let mut updates: Vec<SHIRRegUpdate> = Vec::new();

    // Helper: resolve an expression — apply renames then substitute forwarded values.
    let resolve = |expr: SHIRExpr, fwd: &HashMap<String, SHIRExpr>| -> SHIRExpr {
        subst_vars(rename_vars(expr, renames), fwd)
    };

    for stmt in stmts {
        match stmt {
            CHIRStmt::Assign { target, value, .. } => {
                let resolved = resolve(lower_expr(value)?, forwarding);
                // Update forwarding so later assigns in this segment see the new value.
                // Done for a WIRE too: within a segment the statements are sequential,
                // so a register update that reads a wire between two of its assignments
                // must see the one that has run, not the wire's final value.
                forwarding.insert(target.clone(), resolved.clone());

                // ONLY A REGISTER GETS A REGISTER UPDATE. Reassigning a combinational
                // WIRE is not a clock-edge event: `lower_stmt_list` already emits it as
                // another blocking assign in the `always_comb`, so pushing an update
                // here as well gave the signal TWO DRIVERS — an `always_comb` defining
                // it and an `always_ff` holding it — which is MULTIDRIVEN, illegal per
                // IEEE 1800 9.2.2.2, and semantically a held register where the source
                // has a wire that is re-initialised every iteration.
                //
                // `reg_names` was already threaded into this function and simply not
                // consulted here. That is the whole defect: not a rival register
                // inference, but a register update emitted without asking whether the
                // target is a register. `copper_analysis::infer_registers` had the right
                // answer all along — `tests/fixtures/aggregate_locals_dut.rs` is the
                // reduced witness, and `register_reconciliation.rs` is what named it.
                if reg_names.contains(target) {
                    updates.push(SHIRRegUpdate { target: target.clone(), next_value: resolved });
                }
            }

            // A `let` wire declared in this segment. It is not a register update, so
            // nothing is pushed — it emits its own continuous assign elsewhere. But it
            // has to be RECORDED when its value reads a register this segment has
            // already assigned: the wire's `assign` reads the flop, which is still the
            // old value at the edge, so anything sampled at the edge through that wire
            // lags a cycle. Substituting the wire's definition closes it — `TODO`
            // cause L-2, measured as `acc <= t` dropping a whole `+ step`.
            //
            // Recorded ONLY when it changes something. A wire that reads no reassigned
            // register keeps its name, so existing Verilog is emitted unchanged and the
            // wire does not go unused — which `-Wall` would flag, failing the
            // equivalence harness rather than merely looking untidy.
            CHIRStmt::Wire { name, value, .. } => {
                let plain = rename_vars(lower_expr(value)?, renames);
                let resolved = subst_vars(plain.clone(), forwarding);
                // Recorded when substitution changed it (cause L-2) — and ALSO
                // when it reads a memory array (`MemIndex`): an edge-sampled
                // consumer must see the read through the wire's DEFINITION, so
                // `vlir_lower::forward_ff_mem_index` can mux the in-flight
                // write stage into the `always_ff` copy. Without this, a
                // register-file read that feeds a pipeline register through a
                // `let` (`rs1_val` into ID/EX) sampled the array during the
                // one-cycle window a write spends in its pipeline stage — in
                // the simulator that write is already visible. Ports keep
                // reading the wire's comb form, which post-edge observation
                // sees AFTER the commit, so they need no mux.
                if resolved != plain || contains_mem_index(&resolved) {
                    forwarding.insert(name.clone(), resolved);
                }
            }

            CHIRStmt::If { condition, then_body, else_body, span: s } => {
                let cond_expr = resolve(lower_expr(condition)?, forwarding);

                // Each branch starts from the current forwarding state.
                let mut then_fwd = forwarding.clone();
                let then_updates = extract_updates_from_stmts(then_body, reg_names, *s, renames, &mut then_fwd)?;

                let mut else_fwd = forwarding.clone();
                let else_updates = else_body.as_ref()
                    .map(|eb| extract_updates_from_stmts(eb, reg_names, *s, renames, &mut else_fwd))
                    .transpose()?
                    .unwrap_or_default();

                let mut touched: Vec<String> = Vec::new();
                for u in &then_updates { if !touched.contains(&u.target) { touched.push(u.target.clone()); } }
                for u in &else_updates { if !touched.contains(&u.target) { touched.push(u.target.clone()); } }

                let reg_touched: HashSet<String> = touched.iter().cloned().collect();
                for target in touched {
                    // Missing branch → hold: use current forwarding value or Var(target).
                    let hold = || forwarding.get(&target).cloned()
                        .unwrap_or_else(|| SHIRExpr::Var(target.clone()));

                    // The LAST assignment in the branch is the branch's result.
                    // Taking the first silently discarded a re-assignment, which is
                    // exactly the mod-N counter idiom `t = t + 1; if t == N { t = 0 }`
                    // — the reset vanished and the counter ran free.
                    let then_val = last_update_for(&then_updates, &target)
                        .unwrap_or_else(hold);

                    let else_val = last_update_for(&else_updates, &target)
                        .unwrap_or_else(hold);

                    let mux_val = SHIRExpr::Mux {
                        cond: Box::new(cond_expr.clone()),
                        then_val: Box::new(then_val),
                        else_val: Box::new(else_val),
                    };
                    forwarding.insert(target.clone(), mux_val.clone());
                    updates.push(SHIRRegUpdate { target, next_value: mux_val });
                }

                // The same sequential merge for WIRE forwarding. A wire assigned
                // inside a branch (the default-then-override lowering writes
                // `new_id_ex_rs1_val = rs1_val` inside an `if` arm) carries the
                // mux of its branch values forward, so an edge-sampled consumer
                // downstream still resolves to the segment-order value — the
                // register-file read that feeds ID/EX reaches the `always_ff`
                // copy only through this chain. Registers were merged above;
                // everything else whose forwarded value CHANGED in either
                // branch merges here.
                let mut wire_keys: Vec<String> = then_fwd
                    .keys()
                    .chain(else_fwd.keys())
                    .filter(|k| !reg_touched.contains(*k))
                    .cloned()
                    .collect::<HashSet<_>>()
                    .into_iter()
                    .collect();
                wire_keys.sort();
                for k in wire_keys {
                    let outer = forwarding.get(&k).cloned();
                    let t = then_fwd.get(&k).cloned();
                    let e = else_fwd.get(&k).cloned();
                    if t == outer && e == outer {
                        continue;
                    }
                    let fallthrough = || outer.clone().unwrap_or_else(|| SHIRExpr::Var(k.clone()));
                    let then_val = t.unwrap_or_else(fallthrough);
                    let else_val = e.unwrap_or_else(fallthrough);
                    let merged = if then_val == else_val {
                        then_val
                    } else {
                        SHIRExpr::Mux {
                            cond: Box::new(cond_expr.clone()),
                            then_val: Box::new(then_val),
                            else_val: Box::new(else_val),
                        }
                    };
                    forwarding.insert(k, merged);
                }
            }

            CHIRStmt::Match { scrutinee, arms, span: s } => {
                let scrutinee_expr = resolve(lower_expr(scrutinee)?, forwarding);

                // Process each arm independently from the current forwarding state.
                let arm_updates: Vec<Vec<SHIRRegUpdate>> = arms.iter()
                    .map(|arm| {
                        let mut arm_fwd = forwarding.clone();
                        extract_updates_from_stmts(&arm.body, reg_names, *s, renames, &mut arm_fwd)
                    })
                    .collect::<Result<_, _>>()?;

                let mut touched: Vec<String> = Vec::new();
                for arm_upds in &arm_updates {
                    for u in arm_upds {
                        if !touched.contains(&u.target) { touched.push(u.target.clone()); }
                    }
                }

                for target in touched {
                    let hold = || forwarding.get(&target).cloned()
                        .unwrap_or_else(|| SHIRExpr::Var(target.clone()));

                    let mut case_arms: Vec<SHIRCaseArm> = Vec::new();
                    let mut default_val: Option<SHIRExpr> = None;

                    for (arm, upds) in arms.iter().zip(arm_updates.iter()) {
                        // Last assignment wins — see the note in the `If` arm.
                        let next_val = last_update_for(upds, &target).unwrap_or_else(hold);

                        let guard = arm.guard.as_ref()
                            .map(|g| Ok(resolve(lower_expr(g)?, forwarding)))
                            .transpose()?;

                        let is_default = arm.patterns.iter().any(|p| matches!(p, CHIRPattern::Wildcard))
                            && guard.is_none();

                        if is_default {
                            default_val = Some(next_val);
                        } else {
                            for pattern in &arm.patterns {
                                case_arms.push(SHIRCaseArm {
                                    pattern: lower_pattern(pattern)?,
                                    guard: guard.clone(),
                                    value: next_val.clone(),
                                });
                            }
                        }
                    }

                    let default = default_val.unwrap_or_else(hold);
                    let case_val = SHIRExpr::Case {
                        scrutinee: Box::new(scrutinee_expr.clone()),
                        arms: case_arms,
                        default: Box::new(default),
                    };
                    forwarding.insert(target.clone(), case_val.clone());
                    updates.push(SHIRRegUpdate { target, next_value: case_val });
                }
            }

            // Wire, Emit, AwaitTick — not register updates
            _ => {}
        }
    }

    Ok(updates)
}

// ── Helpers: wire promotion ───────────────────────────────────────────────────

/// Find wires that need to be promoted to registers.
///
/// A wire declared in seg_j needs promotion if it is referenced in any other
/// segment (a later one: the reference crosses a tick).
///
/// Returns a list of (wire_name, wire_ty, wire_init) for promoted wires.
/// `wire_init` is always `None` — promoted wires start at 0 (unknown initial).
fn find_promoted_wires(
    segments: &[Vec<CHIRStmt>],
    wire_segment: &HashMap<String, usize>,
) -> Vec<(String, CHIRType, Option<SHIRLit>)> {
    // Collect all wire types from declarations
    let wire_types: HashMap<String, CHIRType> = segments.iter()
        .flat_map(|stmts| stmts.iter())
        .filter_map(|s| {
            if let CHIRStmt::Wire { name, ty, .. } = s {
                Some((name.clone(), ty.clone()))
            } else {
                None
            }
        })
        .collect();

    // Find which wires are referenced in a different phase from their declaration
    let mut promote: HashSet<String> = HashSet::new();

    // A wire read in a later segment than the one that declares it crosses at
    // least one tick, so it is carried by a register. Before Phase C step 4 the test
    // compared PHASES, with the trailing segment mapped to the last phase (the fold),
    // so a head wire read after the last tick of a single-tick body stayed a wire;
    // the trailing segment is now the next resumption's head, after the edge.
    for (seg_idx, stmts) in segments.iter().enumerate() {
        for stmt in stmts {
            collect_expr_vars_in_stmt(stmt, &mut |var_name: &str| {
                if let Some(&decl_seg) = wire_segment.get(var_name) {
                    if decl_seg != seg_idx {
                        promote.insert(var_name.to_string());
                    }
                }
            });
        }
    }

    // Sorted: `promote` is a HashSet, and iterating it directly made the
    // promoted-register DECLARATION ORDER nondeterministic — two transpiles of
    // sipo_block differed in `w0_r`/`w1_r` order (found by sv-baseline's
    // snapshot-then-diff on an unchanged tree, 2026-08-26). Semantically
    // harmless, but it poisons byte-level reproducibility and the migration's
    // exact-set emission gate.
    let mut promoted: Vec<String> = promote.into_iter().collect();
    promoted.sort();
    promoted.into_iter()
        .filter_map(|name| {
            let ty = wire_types.get(&name)?.clone();
            Some((name, ty, None))
        })
        .collect()
}

/// Walk a statement and call `visitor` on every variable name in every expression.
pub(crate) fn collect_expr_vars_in_stmt(stmt: &CHIRStmt, visitor: &mut impl FnMut(&str)) {
    match stmt {
        CHIRStmt::Wire { value, .. } => collect_expr_vars(value, visitor),
        CHIRStmt::Assign { value, .. } => collect_expr_vars(value, visitor),
        CHIRStmt::PortWrite { value, .. } => collect_expr_vars(value, visitor),
        CHIRStmt::If { condition, then_body, else_body, .. } => {
            collect_expr_vars(condition, visitor);
            for s in then_body { collect_expr_vars_in_stmt(s, visitor); }
            if let Some(eb) = else_body { for s in eb { collect_expr_vars_in_stmt(s, visitor); } }
        }
        CHIRStmt::Match { scrutinee, arms, .. } => {
            collect_expr_vars(scrutinee, visitor);
            for arm in arms {
                if let Some(g) = &arm.guard { collect_expr_vars(g, visitor); }
                for s in &arm.body { collect_expr_vars_in_stmt(s, visitor); }
            }
        }
        CHIRStmt::ForLoop { start, end, body, .. } => {
            collect_expr_vars(start, visitor);
            collect_expr_vars(end, visitor);
            for s in body { collect_expr_vars_in_stmt(s, visitor); }
        }
        CHIRStmt::IndexAssign { base, index, value, .. } => {
            visitor(base);
            collect_expr_vars(index, visitor);
            collect_expr_vars(value, visitor);
        }
        CHIRStmt::MemRead { addr, .. } => collect_expr_vars(addr, visitor),
        CHIRStmt::MemWrite { addr, value, .. } => {
            collect_expr_vars(addr, visitor);
            collect_expr_vars(value, visitor);
        }
        CHIRStmt::AwaitTick { .. } => {}
    }
}

fn collect_expr_vars(expr: &CHIRExpr, visitor: &mut impl FnMut(&str)) {
    match expr {
        CHIRExpr::Var(name) => visitor(name),
        CHIRExpr::MemIndex { mem, addr } => {
            visitor(mem);
            collect_expr_vars(addr, visitor);
        }
        CHIRExpr::Lit(_) => {}
        CHIRExpr::BinOp { left, right, .. } => {
            collect_expr_vars(left, visitor);
            collect_expr_vars(right, visitor);
        }
        CHIRExpr::UnOp { expr, .. } => collect_expr_vars(expr, visitor),
        CHIRExpr::Mux { cond, then_val, else_val } => {
            collect_expr_vars(cond, visitor);
            collect_expr_vars(then_val, visitor);
            collect_expr_vars(else_val, visitor);
        }
        CHIRExpr::Case { scrutinee, arms, default } => {
            collect_expr_vars(scrutinee, visitor);
            for arm in arms {
                if let Some(g) = &arm.guard { collect_expr_vars(g, visitor); }
                collect_expr_vars(&arm.value, visitor);
            }
            if let Some(d) = default { collect_expr_vars(d, visitor); }
        }
        CHIRExpr::Concat(exprs) => { for e in exprs { collect_expr_vars(e, visitor); } }
        CHIRExpr::Slice { expr, .. } => collect_expr_vars(expr, visitor),
        CHIRExpr::SignCast { expr, .. } => collect_expr_vars(expr, visitor),
        CHIRExpr::DynBit { base, index } => {
            collect_expr_vars(base, visitor);
            collect_expr_vars(index, visitor);
        }
        CHIRExpr::Resize { expr, .. } => collect_expr_vars(expr, visitor),
        // A memory read result is not a variable reference.
        CHIRExpr::MemData { .. } | CHIRExpr::MemValid { .. } => {}
    }
}

// ── Expression lowering ───────────────────────────────────────────────────────

/// Does the expression read a memory array combinationally?
fn contains_mem_index(e: &SHIRExpr) -> bool {
    let mut found = false;
    fn walk(e: &SHIRExpr, found: &mut bool) {
        if *found {
            return;
        }
        match e {
            SHIRExpr::MemIndex { .. } => *found = true,
            SHIRExpr::BinOp { left, right, .. } => {
                walk(left, found);
                walk(right, found);
            }
            SHIRExpr::UnOp { expr, .. }
            | SHIRExpr::Slice { expr, .. }
            | SHIRExpr::Resize { expr, .. }
            | SHIRExpr::SignCast { expr, .. } => walk(expr, found),
            SHIRExpr::Mux { cond, then_val, else_val } => {
                walk(cond, found);
                walk(then_val, found);
                walk(else_val, found);
            }
            SHIRExpr::Case { scrutinee, arms, default } => {
                walk(scrutinee, found);
                for a in arms {
                    if let Some(g) = &a.guard {
                        walk(g, found);
                    }
                    walk(&a.value, found);
                }
                walk(default, found);
            }
            SHIRExpr::Concat(parts) => {
                for pt in parts {
                    walk(pt, found);
                }
            }
            SHIRExpr::DynBit { base, index } => {
                walk(base, found);
                walk(index, found);
            }
            _ => {}
        }
    }
    walk(e, &mut found);
    found
}

/// Substitute variable references with expressions (used for sequential forwarding).
fn subst_vars(expr: SHIRExpr, subst: &HashMap<String, SHIRExpr>) -> SHIRExpr {
    if subst.is_empty() { return expr; }
    match expr {
        SHIRExpr::Var(ref name) => subst.get(name).cloned().unwrap_or(expr),
        SHIRExpr::MemIndex { mem, addr } => SHIRExpr::MemIndex {
            mem,
            addr: Box::new(subst_vars(*addr, subst)),
        },
        SHIRExpr::Lit(_)
        | SHIRExpr::PhaseEq(_)
        | SHIRExpr::MemData { .. }
        | SHIRExpr::MemValid { .. } => expr,
        SHIRExpr::BinOp { left, op, right } => SHIRExpr::BinOp {
            left: Box::new(subst_vars(*left, subst)),
            op,
            right: Box::new(subst_vars(*right, subst)),
        },
        SHIRExpr::UnOp { op, expr } => SHIRExpr::UnOp {
            op,
            expr: Box::new(subst_vars(*expr, subst)),
        },
        SHIRExpr::Mux { cond, then_val, else_val } => SHIRExpr::Mux {
            cond: Box::new(subst_vars(*cond, subst)),
            then_val: Box::new(subst_vars(*then_val, subst)),
            else_val: Box::new(subst_vars(*else_val, subst)),
        },
        SHIRExpr::Case { scrutinee, arms, default } => SHIRExpr::Case {
            scrutinee: Box::new(subst_vars(*scrutinee, subst)),
            arms: arms.into_iter().map(|a| SHIRCaseArm {
                pattern: a.pattern,
                guard: a.guard.map(|g| subst_vars(g, subst)),
                value: subst_vars(a.value, subst),
            }).collect(),
            default: Box::new(subst_vars(*default, subst)),
        },
        SHIRExpr::Concat(exprs) => SHIRExpr::Concat(
            exprs.into_iter().map(|e| subst_vars(e, subst)).collect()
        ),
        SHIRExpr::SignCast { signed, expr } => SHIRExpr::SignCast {
            signed,
            expr: Box::new(subst_vars(*expr, subst)),
        },
        SHIRExpr::Slice { expr, high, low } => SHIRExpr::Slice {
            expr: Box::new(subst_vars(*expr, subst)),
            high,
            low,
        },
        SHIRExpr::DynBit { base, index } => SHIRExpr::DynBit {
            base: Box::new(subst_vars(*base, subst)),
            index: Box::new(subst_vars(*index, subst)),
        },
        SHIRExpr::Resize { expr, width } => SHIRExpr::Resize {
            expr: Box::new(subst_vars(*expr, subst)),
            width,
        },
    }
}

/// Rename variables using a name→name map (used for promoted wire substitution).
fn rename_vars(expr: SHIRExpr, renames: &HashMap<String, String>) -> SHIRExpr {
    if renames.is_empty() { return expr; }
    let subst: HashMap<String, SHIRExpr> = renames.iter()
        .map(|(k, v)| (k.clone(), SHIRExpr::Var(v.clone())))
        .collect();
    subst_vars(expr, &subst)
}

/// Convert a `CHIRExpr` to a `SHIRExpr`.
///
/// This is mostly structural. The key differences:
/// - `CHIRExpr::Case::default` is `Option`; `SHIRExpr::Case::default` is required (`Box`).
///   If the CHIR case has no default, we error — Phase B's scope validation should have caught
///   non-exhaustive matches, but we may hit this for expressions.
pub fn lower_expr(expr: &CHIRExpr) -> Result<SHIRExpr, SHIRLowerError> {
    match expr {
        CHIRExpr::Var(name) => Ok(SHIRExpr::Var(name.clone())),
        CHIRExpr::MemIndex { mem, addr } => Ok(SHIRExpr::MemIndex {
            mem: mem.clone(),
            addr: Box::new(lower_expr(addr)?),
        }),
        CHIRExpr::Lit(lit) => Ok(SHIRExpr::Lit(lower_lit(lit))),
        CHIRExpr::BinOp { left, op, right } => Ok(SHIRExpr::BinOp {
            left: Box::new(lower_expr(left)?),
            op: op.clone(),
            right: Box::new(lower_expr(right)?),
        }),
        CHIRExpr::UnOp { op, expr } => Ok(SHIRExpr::UnOp {
            op: op.clone(),
            expr: Box::new(lower_expr(expr)?),
        }),
        CHIRExpr::Mux { cond, then_val, else_val } => Ok(SHIRExpr::Mux {
            cond: Box::new(lower_expr(cond)?),
            then_val: Box::new(lower_expr(then_val)?),
            else_val: Box::new(lower_expr(else_val)?),
        }),
        CHIRExpr::Case { scrutinee, arms, default } => {
            let default_expr = match default {
                Some(d) => lower_expr(d)?,
                None => return Err(SHIRLowerError::UnsupportedConstruct {
                    description: "match-as-expression without a wildcard/default arm".to_string(),
                    span: SourceSpan::default(),
                }),
            };
            Ok(SHIRExpr::Case {
                scrutinee: Box::new(lower_expr(scrutinee)?),
                arms: arms.iter()
                    .map(|a| Ok(SHIRCaseArm {
                        pattern: lower_pattern(&a.pattern)?,
                        guard: a.guard.as_ref().map(lower_expr).transpose()?,
                        value: lower_expr(&a.value)?,
                    }))
                    .collect::<Result<_, _>>()?,
                default: Box::new(default_expr),
            })
        }
        CHIRExpr::SignCast { signed, expr } => Ok(SHIRExpr::SignCast {
            signed: *signed,
            expr: Box::new(lower_expr(expr)?),
        }),
        CHIRExpr::Concat(exprs) => Ok(SHIRExpr::Concat(
            exprs.iter().map(lower_expr).collect::<Result<_, _>>()?
        )),
        CHIRExpr::Slice { expr, high, low } => Ok(SHIRExpr::Slice {
            expr: Box::new(lower_expr(expr)?),
            high: *high,
            low: *low,
        }),
        CHIRExpr::DynBit { base, index } => Ok(SHIRExpr::DynBit {
            base: Box::new(lower_expr(base)?),
            index: Box::new(lower_expr(index)?),
        }),
        CHIRExpr::Resize { expr, width } => Ok(SHIRExpr::Resize {
            expr: Box::new(lower_expr(expr)?),
            width: width.clone(),
        }),
        CHIRExpr::MemData { mem, port } => {
            Ok(SHIRExpr::MemData { mem: mem.clone(), port: *port })
        }
        CHIRExpr::MemValid { mem, port } => {
            Ok(SHIRExpr::MemValid { mem: mem.clone(), port: *port })
        }
    }
}

fn lower_lit(lit: &CHIRLit) -> SHIRLit {
    SHIRLit { ty: lit.ty.clone(), value: lit.value }
}

fn lower_pattern(pattern: &CHIRPattern) -> Result<SHIRPattern, SHIRLowerError> {
    match pattern {
        CHIRPattern::Lit(lit) => Ok(SHIRPattern::Lit(lower_lit(lit))),
        CHIRPattern::Wildcard => Ok(SHIRPattern::Wildcard),
        CHIRPattern::Tuple(parts) => {
            let lowered = parts.iter().map(lower_pattern).collect::<Result<_, _>>()?;
            Ok(SHIRPattern::Tuple(lowered))
        }
        CHIRPattern::Const { name } => Ok(SHIRPattern::Const(name.clone())),
        CHIRPattern::EnumVariant { name, inner } => Ok(SHIRPattern::EnumVariant {
            name: name.clone(),
            inner: inner.as_ref()
                .map(|p| lower_pattern(p).map(Box::new))
                .transpose()?,
        }),
    }
}

/// A memory preload, expression by expression.
fn lower_mem_init(init: &CHIRMemInit) -> Result<SHIRMemInit, SHIRLowerError> {
    Ok(match init {
        CHIRMemInit::Fill { var, value } => SHIRMemInit::Fill {
            var: var.clone(),
            value: lower_expr(value)?,
        },
        CHIRMemInit::Words(words) => {
            SHIRMemInit::Words(words.iter().map(lower_expr).collect::<Result<_, _>>()?)
        }
    })
}

fn lower_reg_decl(r: &CHIRRegDecl) -> Result<SHIRReg, SHIRLowerError> {
    Ok(SHIRReg {
        name: r.name.clone(),
        ty: r.ty.clone(),
        init: r.init.as_ref().map(|lit| SHIRLit { ty: lit.ty.clone(), value: lit.value }),
        init_param: r.init_param.clone(),
    })
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use copper_core::chir::{CHIRLit, CHIRType, Width};
    use copper_core::frontend_ir::SourceSpan;

    fn span() -> SourceSpan { SourceSpan::default() }

    fn no_hw() -> std::collections::HashSet<String> { Default::default() }

    fn make_chir(src: &str) -> CHIRModule {
        use crate::parser::capture_frontend_ir;
        use crate::chir_lower::{lower_to_chir, ModuleRegistry};
        let design_fn: syn::ItemFn = syn::parse_str(src).unwrap();
        let fir = capture_frontend_ir(&design_fn, &no_hw()).unwrap();
        let registry = ModuleRegistry::new();
        lower_to_chir(&fir, &no_hw(), &registry).unwrap()
    }

    // ── Phase register width ─────────────────────────────────────────────────

    #[test]
    fn test_phase_register_width_two_phases() {
        assert_eq!(phase_register_width(2), 1);
    }

    #[test]
    fn test_phase_register_width_four_phases() {
        assert_eq!(phase_register_width(4), 2);
    }

    #[test]
    fn test_phase_register_width_three_phases() {
        assert_eq!(phase_register_width(3), 2);
    }

    // ── Segment splitting ────────────────────────────────────────────────────

    #[test]
    fn test_split_single_tick_gives_two_segments() {
        let stmts = vec![
            CHIRStmt::Assign {
                target: "x".to_string(),
                value: CHIRExpr::Lit(CHIRLit { ty: CHIRType::UInt { width: Width::Concrete(8) }, value: 0 }),
                span: span(),
            },
            CHIRStmt::AwaitTick { clock: "clk".to_string(), span: span() },
            CHIRStmt::Assign {
                target: "x".to_string(),
                value: CHIRExpr::Lit(CHIRLit { ty: CHIRType::UInt { width: Width::Concrete(8) }, value: 1 }),
                span: span(),
            },
        ];
        let segments = split_at_ticks(&stmts);
        assert_eq!(segments.len(), 2);
        assert_eq!(segments[0].len(), 1);
        assert_eq!(segments[1].len(), 1);
    }

    #[test]
    fn test_split_two_ticks_gives_three_segments() {
        let stmts = vec![
            CHIRStmt::AwaitTick { clock: "clk".to_string(), span: span() },
            CHIRStmt::AwaitTick { clock: "clk".to_string(), span: span() },
        ];
        let segments = split_at_ticks(&stmts);
        assert_eq!(segments.len(), 3);
    }

    // ── Combinational body ───────────────────────────────────────────────────

    #[test]
    fn test_lower_comb_simple_adder() {
        let chir = make_chir("fn add(a: u8, b: u8) -> u8 { a + b }");
        let shir = lower_to_shir(&chir).unwrap();
        assert_eq!(shir.name, "add");
        assert!(matches!(shir.body, SHIRBody::Combinational(_)));
        if let SHIRBody::Combinational(body) = &shir.body {
            // Final expression `a + b` becomes PortDrive to "out"
            assert_eq!(body.stmts.len(), 1);
            if let SHIRStmt::PortDrive { port_name, value, .. } = &body.stmts[0] {
                assert_eq!(port_name, "out");
                assert!(matches!(value, SHIRExpr::BinOp { .. }));
            } else {
                panic!("expected PortDrive");
            }
        }
    }

    #[test]
    fn test_lower_comb_single_var_output() {
        let chir = make_chir("fn pass(a: u8) -> u8 { a }");
        let shir = lower_to_shir(&chir).unwrap();
        if let SHIRBody::Combinational(body) = &shir.body {
            assert_eq!(body.stmts.len(), 1);
            if let SHIRStmt::PortDrive { value, .. } = &body.stmts[0] {
                assert!(matches!(value, SHIRExpr::Var(_)));
            } else {
                panic!("expected PortDrive");
            }
        } else {
            panic!("expected combinational");
        }
    }

    // ── Sequential body ──────────────────────────────────────────────────────

    #[test]
    fn test_lower_seq_single_tick_register_update() {
        let chir = make_chir(
            "async fn counter(clk: Clock<MainClk>) {
                let mut count: u8 = 0u8;
                loop {
                    count = count.wrapping_add(1u8);
                    clk.tick().await;
                }
            }"
        );
        let shir = lower_to_shir(&chir).unwrap();
        if let SHIRBody::Sequential(body) = &shir.body {
            assert_eq!(body.phases.len(), 1);
            assert_eq!(body.clock, "clk");
            assert_eq!(body.registers.len(), 1);
            assert_eq!(body.registers[0].name, "count");
            // post_edge should have the count update
            let phase = &body.phases[0];
            assert!(phase.post_edge.iter().any(|u| u.target == "count"));
        } else {
            panic!("expected sequential");
        }
    }

    #[test]
    fn test_lower_seq_single_phase_has_no_phase_r() {
        let chir = make_chir(
            "async fn m(clk: Clock<MainClk>, x: u8) {
                loop { clk.tick().await; }
            }"
        );
        let shir = lower_to_shir(&chir).unwrap();
        if let SHIRBody::Sequential(body) = &shir.body {
            assert_eq!(body.phases.len(), 1);
            assert!(!body.registers.iter().any(|r| r.name == "phase_r"));
        }
    }

    #[test]
    fn test_lower_seq_two_ticks_generates_phase_r() {
        let chir = make_chir(
            "async fn m(clk: Clock<MainClk>, x: u8) {
                let mut acc: u8 = 0u8;
                loop {
                    clk.tick().await;
                    clk.tick().await;
                    acc = acc.wrapping_add(x);
                }
            }"
        );
        let shir = lower_to_shir(&chir).unwrap();
        if let SHIRBody::Sequential(body) = &shir.body {
            assert_eq!(body.phases.len(), 2);
            assert!(body.registers.iter().any(|r| r.name == "phase_r"));
            // phase_r should start at 0
            let phase_r = body.registers.iter().find(|r| r.name == "phase_r").unwrap();
            assert_eq!(phase_r.init.as_ref().unwrap().value, 0);
        } else {
            panic!("expected sequential");
        }
    }

    #[test]
    fn test_lower_seq_phase_advance_in_post_edge() {
        let chir = make_chir(
            "async fn m(clk: Clock<MainClk>, x: u8) {
                let mut acc: u8 = 0u8;
                loop {
                    clk.tick().await;
                    clk.tick().await;
                    acc = acc.wrapping_add(x);
                }
            }"
        );
        let shir = lower_to_shir(&chir).unwrap();
        if let SHIRBody::Sequential(body) = &shir.body {
            // Phase 0 post_edge should advance to phase 1
            let phase0 = &body.phases[0];
            let advance = phase0.post_edge.iter().find(|u| u.target == "phase_r");
            assert!(advance.is_some());
            match &advance.unwrap().next_value {
                SHIRExpr::Lit(lit) => assert_eq!(lit.value, 1),
                _ => panic!("expected literal 1 for phase advance"),
            }
            // Phase 1 post_edge should wrap back to phase 0
            let phase1 = &body.phases[1];
            let wrap = phase1.post_edge.iter().find(|u| u.target == "phase_r");
            assert!(wrap.is_some());
            match &wrap.unwrap().next_value {
                SHIRExpr::Lit(lit) => assert_eq!(lit.value, 0),
                _ => panic!("expected literal 0 for phase wrap"),
            }
        }
    }

    // ── Conditional register updates ─────────────────────────────────────────

    #[test]
    fn test_one_sided_if_holds_current_value() {
        // if cond { count = 0u8; }  → Mux(cond, 0, count)
        let stmts = vec![CHIRStmt::If {
            condition: CHIRExpr::Var("cond".to_string()),
            then_body: vec![CHIRStmt::Assign {
                target: "count".to_string(),
                value: CHIRExpr::Lit(CHIRLit { ty: CHIRType::UInt { width: Width::Concrete(8) }, value: 0 }),
                span: span(),
            }],
            else_body: None,
            span: span(),
        }];

        let regs = vec![CHIRRegDecl {
            name: "count".to_string(),
            ty: CHIRType::UInt { width: Width::Concrete(8) },
            init: None,
            init_param: None,
            span: span(),
        }];
        let promoted = std::collections::HashSet::new();

        let updates = extract_reg_updates_shared(&stmts, &regs, &promoted, span(), &HashMap::new(), &mut HashMap::new())
            .unwrap();
        assert_eq!(updates.len(), 1);
        assert_eq!(updates[0].target, "count");
        // next_value should be Mux(cond, 0, count)
        assert!(matches!(&updates[0].next_value, SHIRExpr::Mux { .. }));
        if let SHIRExpr::Mux { else_val, .. } = &updates[0].next_value {
            // else_val should be Var("count") — hold
            assert!(matches!(else_val.as_ref(), SHIRExpr::Var(n) if n == "count"));
        }
    }

    #[test]
    fn test_two_sided_if_uses_both_branches() {
        let stmts = vec![CHIRStmt::If {
            condition: CHIRExpr::Var("cond".to_string()),
            then_body: vec![CHIRStmt::Assign {
                target: "x".to_string(),
                value: CHIRExpr::Lit(CHIRLit { ty: CHIRType::UInt { width: Width::Concrete(8) }, value: 1 }),
                span: span(),
            }],
            else_body: Some(vec![CHIRStmt::Assign {
                target: "x".to_string(),
                value: CHIRExpr::Lit(CHIRLit { ty: CHIRType::UInt { width: Width::Concrete(8) }, value: 2 }),
                span: span(),
            }]),
            span: span(),
        }];

        let regs = vec![CHIRRegDecl {
            name: "x".to_string(),
            ty: CHIRType::UInt { width: Width::Concrete(8) },
            init: None,
            init_param: None,
            span: span(),
        }];
        let promoted = std::collections::HashSet::new();

        let updates = extract_reg_updates_shared(&stmts, &regs, &promoted, span(), &HashMap::new(), &mut HashMap::new())
            .unwrap();
        assert_eq!(updates.len(), 1);
        assert!(matches!(&updates[0].next_value, SHIRExpr::Mux { .. }));
        if let SHIRExpr::Mux { then_val, else_val, .. } = &updates[0].next_value {
            assert!(matches!(then_val.as_ref(), SHIRExpr::Lit(l) if l.value == 1));
            assert!(matches!(else_val.as_ref(), SHIRExpr::Lit(l) if l.value == 2));
        }
    }

    // ── End-to-end ───────────────────────────────────────────────────────────

    #[test]
    fn test_e2e_counter_single_tick() {
        let chir = make_chir(
            "async fn counter(clk: Clock<MainClk>) {
                let mut count: u8 = 0u8;
                loop {
                    count = count.wrapping_add(1u8);
                    clk.tick().await;
                }
            }"
        );
        let shir = lower_to_shir(&chir).unwrap();
        assert_eq!(shir.name, "counter");
        if let SHIRBody::Sequential(body) = &shir.body {
            assert_eq!(body.phases.len(), 1);
            assert!(!body.registers.iter().any(|r| r.name == "phase_r"));
            let phase = &body.phases[0];
            assert!(phase.post_edge.iter().any(|u| u.target == "count"));
        } else {
            panic!("expected sequential");
        }
    }

    #[test]
    fn test_e2e_comb_adder_ports_preserved() {
        let chir = make_chir("fn add(a: u8, b: u8) -> u8 { a + b }");
        let shir = lower_to_shir(&chir).unwrap();
        let port_names: Vec<_> = shir.ports.iter().map(|p| p.name.as_str()).collect();
        assert!(port_names.contains(&"a"));
        assert!(port_names.contains(&"b"));
        assert!(port_names.contains(&"out"));
    }

    // ── Sequential forwarding ────────────────────────────────────────────────

    #[test]
    fn test_sequential_forwarding_direct_assigns() {
        // stage1_r = input + 1;
        // stage2_r = stage1_r + stage1_r;
        // stage2_r's next_value should use the new stage1_r (input+1), not the old register.
        let stmts = vec![
            CHIRStmt::Assign {
                target: "stage1_r".to_string(),
                value: CHIRExpr::BinOp {
                    left: Box::new(CHIRExpr::Var("input".to_string())),
                    op: copper_core::chir::CHIRBinOp::Add { wrapping: true },
                    right: Box::new(CHIRExpr::Lit(CHIRLit { ty: CHIRType::UInt { width: Width::Concrete(8) }, value: 1 })),
                },
                span: span(),
            },
            CHIRStmt::Assign {
                target: "stage2_r".to_string(),
                value: CHIRExpr::BinOp {
                    left: Box::new(CHIRExpr::Var("stage1_r".to_string())),
                    op: copper_core::chir::CHIRBinOp::Add { wrapping: true },
                    right: Box::new(CHIRExpr::Var("stage1_r".to_string())),
                },
                span: span(),
            },
        ];
        let regs = vec![
            CHIRRegDecl { name: "stage1_r".to_string(), ty: CHIRType::UInt { width: Width::Concrete(8) }, init: None, init_param: None, span: span() },
            CHIRRegDecl { name: "stage2_r".to_string(), ty: CHIRType::UInt { width: Width::Concrete(8) }, init: None, init_param: None, span: span() },
        ];
        let promoted = std::collections::HashSet::new();
        let updates = extract_reg_updates_shared(&stmts, &regs, &promoted, span(), &HashMap::new(), &mut HashMap::new())
            .unwrap();

        assert_eq!(updates.len(), 2);
        let s1 = updates.iter().find(|u| u.target == "stage1_r").unwrap();
        let s2 = updates.iter().find(|u| u.target == "stage2_r").unwrap();

        // stage1_r's next_value = input + 1 (unchanged)
        assert!(matches!(&s1.next_value, SHIRExpr::BinOp { .. }));

        // stage2_r's next_value must NOT be BinOp(Var("stage1_r"), ...) — that would use the OLD register.
        // It should be BinOp(BinOp(input+1), ...) — forwarded to the new stage1_r value.
        match &s2.next_value {
            SHIRExpr::BinOp { left, right, .. } => {
                // Both sides should be the forwarded expression (input+1), not Var("stage1_r")
                assert!(!matches!(left.as_ref(), SHIRExpr::Var(n) if n == "stage1_r"),
                    "stage2_r left operand should be forwarded, not bare Var(stage1_r)");
                assert!(!matches!(right.as_ref(), SHIRExpr::Var(n) if n == "stage1_r"),
                    "stage2_r right operand should be forwarded, not bare Var(stage1_r)");
            }
            other => panic!("expected BinOp for stage2_r next_value, got {:?}", other),
        }
    }

    #[test]
    fn test_promoted_wire_renamed_in_later_phase() {
        // Two-tick module: step1 computed in segment 1 (phase 1), used in the
        // TRAILING segment, which is the next resumption's head and lowers into
        // phase 0 (Phase C step 4). The use crosses a tick, so step1 is promoted:
        // captured into step1_r at phase 1's edge, read as step1_r by the guarded
        // trailing update in phase 0. (Before step 4 the trailing segment was folded
        // into phase 1 and step1 stayed a wire.)
        let chir = make_chir(
            "async fn two_cycle_op(clk: Clock<MainClk>, input: u8) {
                let mut acc: u8 = 0u8;
                loop {
                    clk.tick().await;
                    let step1: u8 = input.wrapping_add(1u8);
                    clk.tick().await;
                    acc = acc.wrapping_add(step1);
                }
            }"
        );
        let shir = lower_to_shir(&chir).unwrap();
        let SHIRBody::Sequential(body) = &shir.body else { panic!("expected sequential") };
        assert_eq!(body.phases.len(), 2);
        fn mentions(expr: &SHIRExpr, name: &str) -> bool {
            match expr {
                SHIRExpr::Var(n) => n == name,
                SHIRExpr::BinOp { left, right, .. } => mentions(left, name) || mentions(right, name),
                _ => false,
            }
        }
        let (guard, trailing) = body.phases[0].guarded_post_edge.as_ref().expect("phase 0 runs the trailing region");
        assert!(matches!(guard, SHIRExpr::Var(n) if n == STARTED_REG));
        let acc_update = trailing.iter().find(|u| u.target == "acc").expect("acc is updated by the trailing region");
        assert!(mentions(&acc_update.next_value, "step1_r"), "{:?}", acc_update.next_value);
        assert!(!body.phases[1].post_edge.iter().any(|u| u.target == "acc"), "no fold into phase 1");
        assert!(body.phases[1].post_edge.iter().any(|u| u.target == "step1_r"), "step1 captured at phase 1's edge");
    }

    #[test]
    fn test_forwarding_with_one_sided_if() {
        // R = 5; if cond { R = R + 1; } → after if, R's next = Mux(cond, 5+1, 5)
        let stmts = vec![
            CHIRStmt::Assign {
                target: "R".to_string(),
                value: CHIRExpr::Lit(CHIRLit { ty: CHIRType::UInt { width: Width::Concrete(8) }, value: 5 }),
                span: span(),
            },
            CHIRStmt::If {
                condition: CHIRExpr::Var("cond".to_string()),
                then_body: vec![CHIRStmt::Assign {
                    target: "R".to_string(),
                    value: CHIRExpr::BinOp {
                        left: Box::new(CHIRExpr::Var("R".to_string())),
                        op: copper_core::chir::CHIRBinOp::Add { wrapping: false },
                        right: Box::new(CHIRExpr::Lit(CHIRLit { ty: CHIRType::UInt { width: Width::Concrete(8) }, value: 1 })),
                    },
                    span: span(),
                }],
                else_body: None,
                span: span(),
            },
        ];
        let regs = vec![CHIRRegDecl { name: "R".to_string(), ty: CHIRType::UInt { width: Width::Concrete(8) }, init: None, init_param: None, span: span() }];
        let promoted = std::collections::HashSet::new();
        let updates = extract_reg_updates_shared(&stmts, &regs, &promoted, span(), &HashMap::new(), &mut HashMap::new())
            .unwrap();

        // Should produce two updates for R: first the flat assign, then the Mux
        // (or deduplicated to just the Mux — either is acceptable as long as last wins)
        let last_r = updates.iter().rev().find(|u| u.target == "R").unwrap();
        // The final R update must be a Mux (the if overrides the flat assign)
        assert!(matches!(&last_r.next_value, SHIRExpr::Mux { .. }),
            "expected Mux for R after one-sided if with prior assign");
        if let SHIRExpr::Mux { then_val, else_val, .. } = &last_r.next_value {
            // then_val: R+1 where R was already forwarded to 5 → should be Lit(5)+1, not Var("R")+1
            assert!(!matches!(then_val.as_ref(), SHIRExpr::BinOp { left, .. } if matches!(left.as_ref(), SHIRExpr::Var(n) if n == "R")),
                "then_val should use forwarded R=5, not bare Var(R)");
            // else_val: hold = forwarded R = Lit(5)
            assert!(matches!(else_val.as_ref(), SHIRExpr::Lit(l) if l.value == 5),
                "else_val (hold) should be the forwarded value 5");
        }
    }
}
