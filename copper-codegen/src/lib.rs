mod parser;
pub mod file_consts;
pub mod chir_lower;
pub mod control_extract;
pub mod shir_lower;
pub mod vlir_lower;
pub mod emit;
pub mod width_checks;
mod const_fold;

pub use parser::capture_frontend_ir;
pub use chir_lower::{lower_to_chir, ModuleRegistry};
pub use shir_lower::lower_to_shir;
pub use vlir_lower::lower_to_vlir;
pub use vlir_lower::legalized_port_name;
pub use emit::{emit_verilog, EmitConfig};

/// End-to-end transpile of a single `#[hardware]` module given as a parsed
/// `syn::ItemFn`, plus the set of hardware fn names and their FIR registry
/// (for submodule resolution). Returns SystemVerilog text.
pub fn transpile_item_fn(
    design_fn: &syn::ItemFn,
    hardware_fns: &std::collections::HashSet<String>,
    registry: &ModuleRegistry,
    config: &EmitConfig,
) -> Result<String, String> {
    let fir = capture_frontend_ir(design_fn, hardware_fns).map_err(|e| format!("{e:?}"))?;
    transpile_fir(&fir, hardware_fns, registry, config)
}

/// Transpile an already-captured FIR (CHIR → SHIR → VLIR → text). Use this when
/// the FIR needs enriching first — e.g. injecting file-scope enums, which are not
/// reachable from an `ItemFn` alone.
pub fn transpile_fir(
    fir: &copper_core::FrontendModuleIR,
    hardware_fns: &std::collections::HashSet<String>,
    registry: &ModuleRegistry,
    config: &EmitConfig,
) -> Result<String, String> {
    let vlir = lower_fir_to_vlir(fir, hardware_fns, registry)?;
    Ok(emit_verilog(&vlir, config))
}

/// [`transpile_fir`] up to the VLIR — the whole lowering pipeline (control
/// extraction, CHIR, SHIR, VLIR) without the text emission, so a caller can
/// inspect the lowered module.
pub fn lower_fir_to_vlir(
    fir: &copper_core::FrontendModuleIR,
    hardware_fns: &std::collections::HashSet<String>,
    registry: &ModuleRegistry,
) -> Result<copper_core::vlir::VLIRModule, String> {
    let shir = lower_fir_to_shir(fir, hardware_fns, registry)?;
    lower_to_vlir(&shir).map_err(|e| format!("{e}"))
}

/// [`transpile_fir`] up to the SHIR — the same passes in the same order, register
/// authority included, so a test that inspects the phases sees what the
/// transpiler lowers (tests/timing_model_derivations.rs used to rebuild this
/// sequence by hand and missed the register append; MEASUREMENTS §K.11).
pub fn lower_fir_to_shir(
    fir: &copper_core::FrontendModuleIR,
    hardware_fns: &std::collections::HashSet<String>,
    registry: &ModuleRegistry,
) -> Result<copper_core::shir::SHIRModule, String> {
    let chir = lower_fir_to_chir(fir, hardware_fns, registry)?;
    lower_to_shir(&chir).map_err(|e| format!("{e}"))
}

/// [`transpile_fir`] up to the CHIR — control extraction, the register append and
/// `chir_lower`, the passes [`lower_fir_to_shir`] runs before `lower_to_shir`. The
/// last IR that carries source spans per statement, which is what the projection
/// test (`tests/projection_relation.rs`, SPEC III.1 P1) reads the emitted state
/// machine off.
pub fn lower_fir_to_chir(
    fir: &copper_core::FrontendModuleIR,
    hardware_fns: &std::collections::HashSet<String>,
    registry: &ModuleRegistry,
) -> Result<copper_core::chir::CHIRModule, String> {
    // Control extraction (FIR→FIR): flatten async control-flow loops whose ticks
    // live inside branches into an explicit single-tick `match pc` FSM — the shape
    // the pipeline below already lowers correctly. No-op for linear modules.
    let source = fir;
    let mut fir = fir.clone();
    // `while <cond> { … tick; }` is sugar for the repeating wait extraction
    // already handles; rewrite it before the gate looks at the body.
    control_extract::desugar_tick_waits(&mut fir);
    // …and `for <var> in <a>..<b> { … tick; }` is sugar for a counted one. It runs
    // AFTER the `while` rewrite so a `for` nested inside a tick-bearing `while`
    // is reached: that rewrite moves the body into a fresh `loop`, which this pass
    // then walks.
    control_extract::desugar_counted_loops_in(&mut fir);
    // Why extraction is about to decline, if the reason is a construct the linear
    // path downstream cannot name. Computed BEFORE the pass runs, since a declined
    // module is left untouched and there is nothing to inspect afterwards.
    let declined = control_extract::unflattenable_reason(&fir);
    // When the reason is a MALFORMED NESTED LOOP, `unflattenable_reason` returns
    // None on purpose, so `chir_lower::nested_loop_error` can split it three ways.
    // But that function is handed one loop at a time and cannot look sideways: for
    // a body of `<well-formed wait>` then `<malformed wait>` it was called on the
    // FIRST loop and quoted the ordering rule at a loop that already obeys it.
    // Find the loop actually at fault here, where the whole body is visible, and
    // build the message from THAT one. Rendered eagerly because `extract_control`
    // takes `&mut fir` next, which ends the borrow.
    let malformed_loop = if declined.is_none() {
        control_extract::first_malformed_nested_loop(&fir)
            .map(|(l, sp)| format!("{}", chir_lower::nested_loop_error(l, sp)))
    } else {
        None
    };
    let preloop_muts_before = preloop_mut_names(&fir);
    control_extract::extract_control(&mut fir);
    // Register state SYNTHESIZED by the FIR→FIR passes — `pc`, the
    // `__copper_ctr*` counters, and hoisted cross-state locals — appears as new
    // pre-loop `let mut` declarations. Append the diff to the FIR's register
    // authority so `chir_lower` classifies them without a second heuristic; a
    // future pass that introduces register state is covered automatically.
    //
    // A hoisted cross-state local is appended only when the inference names it
    // (Phase C step 6, P13: hoist by liveness, MEASUREMENTS §K.6). The hoist moves a
    // local mentioned in two states by NAME; one the inference does not name is dead
    // at every tick (I.7.1) — its states pass it within a resumption, never across an
    // edge — so `chir_lower` gives it a wire, not a flop nobody reads
    // (`nested_loop_added`'s `w` was one).
    // Only a SOURCE local is the inference's to judge: the passes' own state
    // (`__copper_ctr*`, which the counted-`for` desugar declares in the loop and the
    // hoist then moves out) is not in its domain and stays a register.
    let hoisted: std::collections::BTreeSet<String> = hoisted_names(&fir)
        .into_iter()
        .filter(|n| control_extract::stmts_mention(&source.raw_statements, n))
        .collect();
    for name in preloop_mut_names(&fir) {
        if !preloop_muts_before.contains(&name)
            && !fir.registers.contains(&name)
            && !hoisted.contains(&name)
        {
            fir.registers.push(name);
        }
    }
    let fir = &fir;

    // A declined module cannot be flattened, so whatever the linear lowering
    // reports is downstream of that decline — and it blames the first unsupported
    // thing it REACHES, which is routinely not the thing at fault. `uart/rx`
    // reported its well-formed repeating wait (line 55) for a `continue` further
    // down the body. Prefer the construct that actually stopped the flattening,
    // with its own span.
    lower_to_chir(fir, hardware_fns, registry).map_err(|e| {
        match (&declined, &malformed_loop) {
            (Some(reason), _) => format!("{reason}"),
            (None, Some(m)) => m.clone(),
            _ => format!("{e}"),
        }
    })
}

/// The pre-loop `let mut` names of a FIR body — the declarations the FIR→FIR
/// passes add register state through (see the diff in [`transpile_fir`]).
/// The pre-loop `let mut`s control extraction hoisted out of a state
/// (`HOISTED_CROSS_STATE_ATTR`).
fn hoisted_names(fir: &copper_core::FrontendModuleIR) -> std::collections::BTreeSet<String> {
    use copper_core::frontend_ir::RawStmtKind;
    fir.raw_statements
        .iter()
        .filter_map(|s| match &s.kind {
            RawStmtKind::Local(l) if l.attrs.iter().any(|a| a == control_extract::HOISTED_CROSS_STATE_ATTR) => {
                Some(l.name.clone())
            }
            _ => None,
        })
        .collect()
}

fn preloop_mut_names(fir: &copper_core::FrontendModuleIR) -> std::collections::BTreeSet<String> {
    use copper_core::frontend_ir::RawStmtKind;
    fir.raw_statements
        .iter()
        .filter_map(|s| match &s.kind {
            RawStmtKind::Local(l) if l.is_mut => Some(l.name.clone()),
            _ => None,
        })
        .collect()
}

/// Transpile one hardware module out of a Rust *source string*. Finds hardware
/// modules (functions carrying `#[hardware(...)]`), builds a FIR registry
/// across all of them for submodule resolution, and transpiles the selected
/// module. `module` may be `None` when the source has exactly one hardware
/// module. Used by the `copper-transpile` CLI and the test harness.
///
/// A function whose signature has a `Clock`/`In`/`Out` parameter but no
/// `#[hardware(...)]` attribute is rejected rather than silently treated as
/// a hardware module — attributed and non-attributed functions simulate with
/// different timing (the macro injects a pre-edge barrier), so module
/// detection must not paper over the difference.
pub fn transpile_source(
    src: &str,
    module: Option<&str>,
    config: &EmitConfig,
) -> Result<String, String> {
    let prepared = prepare_source(src)?;
    let target = prepared.select_target(module)?;
    transpile_target(target, &prepared, config)
}

/// Like [`transpile_source`], but emits the selected module **plus every
/// `#[hardware]` submodule it transitively instantiates**, deepest-first, into
/// one self-contained SystemVerilog string. This is what a hierarchical design
/// (item 4's structural parent + its clocked children) needs for a standalone
/// Verilator run — a `module` that instantiates `fast_counter`/`sync_2ff`/… would
/// otherwise reference undefined modules. For a leaf module (no submodules) the
/// output equals `transpile_source`'s (just the one module).
pub fn transpile_source_hierarchy(
    src: &str,
    module: Option<&str>,
    config: &EmitConfig,
) -> Result<String, String> {
    let prepared = prepare_source(src)?;
    let target = prepared.select_target(module)?;
    let order = prepared.hierarchy_emit_order(&target.sig.ident.to_string());

    let mut out = String::new();
    for (i, name) in order.iter().enumerate() {
        let f = prepared
            .modules
            .iter()
            .find(|m| &m.sig.ident.to_string() == name)
            .ok_or_else(|| format!("internal error: module '{name}' vanished from registry"))?;
        if i > 0 {
            out.push('\n');
        }
        out.push_str(&transpile_target(f, &prepared, config)?);
    }
    Ok(out)
}

/// Shared setup for the source-string entry points: parse, reject unattributed
/// hardware-signature functions, collect the hardware modules, and build the FIR
/// registry (with file-scope enums/items injected) used for submodule resolution.
struct PreparedSource {
    modules: Vec<syn::ItemFn>,
    names: Vec<String>,
    hardware_fns: std::collections::HashSet<String>,
    registry: ModuleRegistry,
    file_enums: Vec<copper_core::frontend_ir::ItemEnum>,
    file_scope: parser::FileScope,
}

fn prepare_source(src: &str) -> Result<PreparedSource, String> {
    use std::collections::{HashMap, HashSet};

    let file = syn::parse_file(src).map_err(|e| format!("parse error: {e}"))?;

    let unattributed: Vec<String> = file
        .items
        .iter()
        .filter_map(|it| match it {
            syn::Item::Fn(f) if has_hardware_signature(f) && !is_hardware_fn(f) => {
                Some(f.sig.ident.to_string())
            }
            _ => None,
        })
        .collect();
    if !unattributed.is_empty() {
        return Err(format!(
            "missing #[hardware(...)] attribute on: {} — functions with Clock/In/Out \
             parameters must be marked #[hardware(sequential)] or #[hardware(combinational)]",
            unattributed.join(", ")
        ));
    }

    let modules: Vec<syn::ItemFn> = file
        .items
        .iter()
        .filter_map(|it| match it {
            syn::Item::Fn(f) if is_hardware_fn(f) => Some(f.clone()),
            _ => None,
        })
        .collect();

    if modules.is_empty() {
        return Err("no hardware modules found in source".to_string());
    }
    let names: Vec<String> = modules.iter().map(|f| f.sig.ident.to_string()).collect();
    let hardware_fns: HashSet<String> = names.iter().cloned().collect();

    // Enums and other file-scope items are visible to every module in the file
    // but are not reachable from an `ItemFn`, so inject them into each module's
    // FIR. (Nothing consumes the non-enum items yet — capture only, #7a.)
    let file_enums = parser::capture_file_enums(&file);
    let file_scope = parser::capture_file_scope(&file, &hardware_fns);

    let mut registry: ModuleRegistry = HashMap::new();
    for f in &modules {
        if let Ok(mut fir) = capture_frontend_ir(f, &hardware_fns) {
            fir.enums.extend(file_enums.iter().cloned());
            inject_file_scope(&mut fir, &file_scope);
            registry.insert(f.sig.ident.to_string(), fir);
        }
    }

    Ok(PreparedSource { modules, names, hardware_fns, registry, file_enums, file_scope })
}

impl PreparedSource {
    fn select_target(&self, module: Option<&str>) -> Result<&syn::ItemFn, String> {
        match module {
            Some(name) => self
                .modules
                .iter()
                .find(|f| f.sig.ident == name)
                .ok_or_else(|| format!("module '{name}' not found; available: {}", self.names.join(", "))),
            None if self.modules.len() == 1 => Ok(&self.modules[0]),
            None => Err(format!(
                "{} modules found; specify one. Available: {}",
                self.modules.len(),
                self.names.join(", ")
            )),
        }
    }

    /// The `#[hardware]` submodules `name` directly instantiates, read off its
    /// lowered CHIR. Only a structural body instantiates (SPEC I.13.5, D-23).
    fn direct_deps(&self, name: &str) -> Vec<String> {
        use copper_core::chir::CHIRBody;
        let Some(fir) = self.registry.get(name) else { return Vec::new() };
        let Ok(chir) = lower_to_chir(fir, &self.hardware_fns, &self.registry) else {
            return Vec::new();
        };
        let subs = match &chir.body {
            CHIRBody::Combinational(_) | CHIRBody::Sequential(_) => return Vec::new(),
            CHIRBody::Structural(b) => &b.submodules,
        };
        let mut names: Vec<String> = subs.iter().map(|s| s.module_name.clone()).collect();
        names.dedup();
        names
    }

    /// Post-order DFS from `target`: every transitively-instantiated child
    /// appears before its parent, `target` last, each module once. A dependency
    /// not present in the file (an external module) is skipped — emit only what
    /// this file defines.
    fn hierarchy_emit_order(&self, target: &str) -> Vec<String> {
        let mut order = Vec::new();
        let mut visited = std::collections::HashSet::new();
        self.visit_deps(target, &mut visited, &mut order);
        order
    }

    fn visit_deps(&self, name: &str, visited: &mut std::collections::HashSet<String>, order: &mut Vec<String>) {
        if !self.hardware_fns.contains(name) || !visited.insert(name.to_string()) {
            return;
        }
        for dep in self.direct_deps(name) {
            self.visit_deps(&dep, visited, order);
        }
        order.push(name.to_string());
    }
}

/// Run the per-module transpile: the shared reachability well-formedness check,
/// register inference (logged), then FIR → SV.
fn transpile_target(
    target: &syn::ItemFn,
    prepared: &PreparedSource,
    config: &EmitConfig,
) -> Result<String, String> {
    let vlir = vlir_for_target(target, prepared)?;
    Ok(emit_verilog(&vlir, config))
}

/// [`transpile_target`] up to the VLIR: every front-end rule, then the lowering.
fn vlir_for_target(
    target: &syn::ItemFn,
    prepared: &PreparedSource,
) -> Result<copper_core::vlir::VLIRModule, String> {
    let fir = checked_fir_for_target(target, prepared)?;
    let mut vlir = lower_fir_to_vlir(&fir, &prepared.hardware_fns, &prepared.registry)?;
    vlir.width_checks = width_checks::width_checks(target, &fir.file_const_source).0;
    Ok(vlir)
}

/// One module's lowered forms, for a test that inspects what the transpiler does
/// with it rather than only what it emits: the captured FIR (file scope injected,
/// before any FIR→FIR pass), the CHIR, the SHIR and the SystemVerilog text — each
/// produced by the same passes, in the same order, as [`transpile_source`].
pub struct LoweredModule {
    pub fir: copper_core::FrontendModuleIR,
    pub chir: copper_core::chir::CHIRModule,
    pub shir: copper_core::shir::SHIRModule,
    pub sv: String,
}

/// [`transpile_source`] for one named module, keeping every intermediate form (see
/// [`LoweredModule`]). Every front-end rule runs first, so a module this returns
/// is one `transpile_source` emits.
pub fn lower_source_module(src: &str, module: &str) -> Result<LoweredModule, String> {
    let prepared = prepare_source(src)?;
    let target = prepared.select_target(Some(module))?;
    let fir = checked_fir_for_target(target, &prepared)?;
    let chir = lower_fir_to_chir(&fir, &prepared.hardware_fns, &prepared.registry)?;
    let shir = lower_to_shir(&chir).map_err(|e| format!("{e}"))?;
    let mut vlir = lower_to_vlir(&shir).map_err(|e| format!("{e}"))?;
    vlir.width_checks = width_checks::width_checks(target, &fir.file_const_source).0;
    let sv = emit_verilog(&vlir, &EmitConfig::default());
    Ok(LoweredModule { fir, chir, shir, sv })
}

/// R3's refusal of a ticking `for` whose emptiness its verdict depends on and which
/// `ConstEnv` cannot fold (SPEC I.8.4(b), MEASUREMENTS §V.3).
pub const UNFOLDED_BOUNDARY_RANGE_MESSAGE: &str = "this `for` may be the enclosing loop's only clock \
    boundary, and whether its range is empty is a constant the transpiler's evaluator cannot fold \
    (SPEC I.8.4(b), II.2.2 R3; I.14.4): an empty range runs no iteration and the enclosing loop \
    would return to its top in zero time. Spell the bounds with literals, `const` items or \
    `const fn` calls the evaluator reads, or give the enclosing loop a tick of its own";

/// The `const` items declared inside `f`'s body (any depth), as source text.
fn body_const_items(f: &syn::ItemFn) -> Vec<String> {
    struct V(Vec<String>);
    impl<'a> syn::visit::Visit<'a> for V {
        fn visit_item_const(&mut self, c: &'a syn::ItemConst) {
            self.0.push(quote::ToTokens::to_token_stream(c).to_string());
        }
    }
    let mut v = V(Vec::new());
    syn::visit::Visit::visit_block(&mut v, &f.block);
    v.0
}

/// Every front-end rule on `target`, then its FIR with the file scope injected.
fn checked_fir_for_target(
    target: &syn::ItemFn,
    prepared: &PreparedSource,
) -> Result<copper_core::FrontendModuleIR, String> {
    // SPEC II.1.3 (P10, Phase C step 10): the macro's syntactic rules — the header
    // (R20, R9's flag, R10), R17, I.14.3's OUTSIDE methods, the memory port index,
    // D3, R16, R18, R7, R12, R13/R14, the CDC port rule, R19 — one function, one
    // message each, run first by both front-ends in the same order.
    if let Some(mode) = copper_analysis::hardware_mode(target) {
        let mode = mode.map_err(|e| e.to_string())?;
        copper_analysis::check_front_end_rules(target, mode).map_err(|e| e.to_string())?;
    }
    // c2 (item 2): the transpiler enforces the SAME reachability well-formedness
    // the sim macro does, from the SAME shared analysis on the SAME `&syn::ItemFn`
    // — a tickless loop path is rejected here too (one authoritative check, both
    // front-ends). In practice a module reaching the transpiler already compiled
    // through the macro's check; this keeps the standalone CLI honest.
    //
    // SPEC I.8.4(b), II.2.2 R3 (2026-11-08, MEASUREMENTS §V.3): a ticking `for` over
    // a range empty only through a constant (`0..EMPTY`) is no boundary. The ranges
    // both front-ends decide (`constant_ticking_for_ranges`) are folded through
    // `ConstEnv` — the file's consts and the body's own — and R3 runs with what the
    // fold says; the macro has rustc check the same ranges. A range R3's verdict
    // depends on that the fold cannot evaluate is refused (the fold is narrower than
    // rustc there, as for R5 and R11), never assumed non-empty.
    {
        let mut items: Vec<String> = prepared.file_scope.const_source.clone();
        items.extend(body_const_items(target));
        let env = copper_analysis::consts::ConstEnv::from_items(items.iter().map(String::as_str));
        let ranges = copper_analysis::constant_ticking_for_ranges(target);
        let mut known = std::collections::HashMap::new();
        for r in &ranges {
            let none = std::collections::HashMap::new();
            let bound = |e: &Option<Box<syn::Expr>>| e.as_deref().and_then(|e| env.eval_expr(e, &none));
            if let (Some(a), Some(b)) = (bound(&r.start), bound(&r.end)) {
                let empty = match r.limits {
                    syn::RangeLimits::HalfOpen(_) => a >= b,
                    syn::RangeLimits::Closed(_) => a > b,
                };
                known.insert(copper_analysis::range_key(r), empty);
            }
        }
        if ranges.iter().any(|r| !known.contains_key(&copper_analysis::range_key(r))) {
            for set in copper_analysis::empty_range_dependencies(target) {
                if let Some(r) = set.iter().find(|r| !known.contains_key(&copper_analysis::range_key(r))) {
                    return Err(syn::Error::new_spanned(r, UNFOLDED_BOUNDARY_RANGE_MESSAGE).to_string());
                }
            }
        }
        copper_analysis::with_empty_range_oracle(
            Box::new(move |r| known.get(&copper_analysis::range_key(r)).copied()),
            || copper_analysis::check_reachability(target),
        )
        .map_err(|e| e.to_string())?;
    }
    // Front-end parity (2026-10-23): the two shape rules the macro now enforces
    // too, from the shared crate, AFTER reachability so a module that never ticks
    // at all gets the reachability message (the better of the two) rather than a
    // complaint about one tick-free nested loop. `chir_lower` / `control_extract`
    // keep their own copies as defence in depth, the way `check_reachability` is.
    copper_analysis::check_nested_loop_shape(target).map_err(|e| e.to_string())?;
    copper_analysis::check_awaits(target).map_err(|e| e.to_string())?;
    // SPEC II.1.2 R21 (Phase C step 5): the macro runs the same function.
    copper_analysis::check_no_preloop_tick(target).map_err(|e| e.to_string())?;
    copper_analysis::check_counted_for_nesting(target).map_err(|e| e.to_string())?;
    // A combinational `Out` written on some paths but not all is a latch — the
    // macro's rule, from the same analysis (2026-11-02, §10.12). Until then the
    // transpiler had only `check_no_latches` over its own lowering, which refused
    // a default-then-override the macro admitted; that check stays as defence in
    // depth (it also covers the wires and memory nets this rule does not see).
    copper_analysis::check_definite_assignment(target).map_err(|e| e.to_string())?;
    // R11: a pre-loop register's initializer is its power-on value and must be a
    // Rust constant or `Bits::x()` (2026-10-29; D-21 A3, 2026-11-07). The macro has
    // rustc judge it (a `const { }` block); the transpiler cannot see that verdict,
    // so it runs the syntactic half both share (no run-time read, same message),
    // then refuses a call to a fn that is not `const` (rustc's E0015 — the fold
    // alone would inline a non-`const` associated ctor structurally), and
    // `chir_lower::lower_register_init` folds the rest; what it cannot fold is an
    // error there, never a dropped value.
    copper_analysis::check_register_initializers(target).map_err(|e| e.to_string())?;
    // D-21 H1 (decided 2026-11-07): the macro's rule, same function, same message.
    copper_analysis::check_held_preloop_values(target).map_err(|e| e.to_string())?;
    {
        let env = copper_analysis::consts::ConstEnv::from_items(
            prepared.file_scope.const_source.iter().map(String::as_str),
        );
        // A tuple-struct constructor is a `const` call too (rustc's E0015 note: "calls
        // in constants are limited to constant functions, tuple structs and tuple
        // variants"; 2026-11-08, MEASUREMENTS §V.4).
        copper_analysis::check_register_initializer_calls_in(target, &|p| env.is_const_fn(p) || env.is_tuple_struct(p))
            .map_err(|e| e.to_string())?;
        // R5, the transpiler's half (the macro has rustc judge each bound in a
        // `const { }` block): a tick-free `for` bound that names no const generic
        // must fold through the same evaluator, or it is not a constant the
        // transpiler can unroll — refused with R5's message, never emitted as a
        // run-time `for` limit.
        let generics: std::collections::HashSet<String> = target
            .sig
            .generics
            .params
            .iter()
            .filter_map(|p| match p {
                syn::GenericParam::Const(c) => Some(c.ident.to_string()),
                _ => None,
            })
            .collect();
        for b in copper_analysis::front_end::tick_free_for_bounds(target) {
            let text = quote::ToTokens::to_token_stream(b).to_string();
            let names_generic = text
                .split(|c: char| !(c.is_alphanumeric() || c == '_'))
                .any(|w| generics.contains(w));
            if !names_generic && env.eval_expr(b, &Default::default()).is_none() {
                return Err(copper_analysis::front_end::RUNTIME_FOR_BOUND_MESSAGE.to_string());
            }
        }
    }
    // SPEC II.1 R8 (D-8, Phase C step 3, 2026-11-06): a clocked module has no
    // plain `Out` — the macro's rule, same function, same message. The pre-tick
    // alignment family (rules 9–15) and `multi_write_collapse` (rule 8), which
    // reasoned about the plain `Out` this refuses, are retired (SPEC II.3.2).
    copper_analysis::check_clocked_outputs(target).map_err(|e| e.to_string())?;

    // The memory-port staging rules, for the same reason and from the same place:
    // they are questions about clock EDGES, and control extraction rewrites the
    // ticks that delimit them into `pc` states. `chir_lower` used to ask them of
    // the lowered body's tick-delimited segments, which made an ordinary memory
    // design fail the moment anything else in the module needed extraction.
    copper_analysis::check_memory_staging(target).map_err(|e| e.to_string())?;
    // SPEC I.13.5 (D-18 A2, D-23 (1), Phase C step 11b): a behavioural body
    // instantiates nothing. The macro has rustc refuse `child(a, b);` (the generated
    // fn's denied `#[must_use]`, whose reason is this message); the transpiler
    // refuses every call its registry resolves to a `#[hardware]` fn. Late, as the
    // macro's refusal is rustc's, after every macro rule. (`lower_hardware_call`, the
    // legacy expression form that instantiated such a call, is deleted.)
    if !matches!(copper_analysis::hardware_mode(target), Some(Ok(copper_analysis::HardwareMode::Structural))) {
        copper_analysis::front_end::check_no_behavioural_instance(target, &|n| {
            prepared.registry.contains_key(n) || prepared.hardware_fns.contains(n)
        })
        .map_err(|e| e.to_string())?;
    }
    // (`memory_result_drives_plain_out`, rule 7, was checked here: retired as
    // vacuous under D-8 — SPEC II.3.2.)

    // c2 (gate G6): the transpiler consumes the SAME shared analysis, on the SAME
    // input (`&syn::ItemFn`), that the sim macro consumes — one authoritative
    // register/CFG analysis, not two that must agree. Register inference is also
    // consumed by the parser (`parser.rs`) and shir_lowering; the result here is
    // only logged.
    let inferred_registers = copper_analysis::infer_registers(target);
    log::debug!(
        "copper-analysis inferred registers for `{}`: {:?}",
        target.sig.ident,
        inferred_registers
    );

    let mut fir = capture_frontend_ir(target, &prepared.hardware_fns).map_err(|e| format!("{e:?}"))?;
    fir.enums.extend(prepared.file_enums.iter().cloned());
    inject_file_scope(&mut fir, &prepared.file_scope);
    Ok(fir)
}

/// Copy captured file-scope items into a module's FIR, mirroring the `enums`
/// injection. A clone per module (multi-module files like `uart/system` have a
/// few) — negligible, and matches how enums already work.
fn inject_file_scope(fir: &mut copper_core::FrontendModuleIR, scope: &parser::FileScope) {
    fir.file_fns.extend(scope.fns.iter().cloned());
    fir.file_structs.extend(scope.structs.iter().cloned());
    fir.file_consts.extend(scope.consts.iter().cloned());
    fir.file_const_source.extend(scope.const_source.iter().cloned());
    fir.file_impls.extend(scope.impls.iter().cloned());
    fir.file_traits.extend(scope.traits.iter().cloned());
}

/// A function is a hardware module only if it carries `#[hardware(...)]`.
/// A `Clock`/`In`/`Out` signature alone is not enough — see
/// `has_hardware_signature` and `transpile_source`.
pub fn is_hardware_fn(f: &syn::ItemFn) -> bool {
    f.attrs
        .iter()
        .any(|a| a.path().segments.last().map(|s| s.ident == "hardware").unwrap_or(false))
}

/// True if `f` COULD be a hardware module missing its `#[hardware(...)]`
/// attribute: at least one port-typed parameter, EVERY parameter port-typed
/// (`Clock` / `In` / `Out` / `RegOut` / `Memory` — the macro accepts nothing
/// else), and no return type (a module returns `()`).
///
/// The old rule — ANY port-typed parameter — condemned the whole FILE when a
/// harness function merely handled ports: `spawn_uart(exec: &mut
/// HardwareExecutor, clk: Clock<MainClk>) -> UartPorts` composes two modules
/// for the SIMULATOR and can never be one itself (the executor parameter and
/// the return type both prove it), yet its presence made `uart_tx`/`uart_rx`
/// untranspilable from their own file (cause H). A genuinely forgotten
/// attribute — all ports, no return — is still caught.
fn has_hardware_signature(f: &syn::ItemFn) -> bool {
    let mut saw_port = false;
    let all_ports = f.sig.inputs.iter().all(|arg| match arg {
        syn::FnArg::Typed(pt) => match outer_type_name(&pt.ty).as_deref() {
            Some("Clock" | "In" | "Out" | "RegOut" | "Memory") => {
                saw_port = true;
                true
            }
            _ => false,
        },
        // A `self` receiver is a method, never a module.
        syn::FnArg::Receiver(_) => false,
    });
    saw_port && all_ports && matches!(f.sig.output, syn::ReturnType::Default)
}

fn outer_type_name(ty: &syn::Type) -> Option<String> {
    if let syn::Type::Path(tp) = ty {
        tp.path.segments.last().map(|s| s.ident.to_string())
    } else {
        None
    }
}
