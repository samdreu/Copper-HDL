//! Shared compile-time control/liveness analysis for Copper (c2 architecture).
//!
//! **Why this crate exists (gate G6 in `design_docs/SYNCHRONOUS_SEMANTICS_IMPL_PLAN.md`).**
//! The c2 decision is that the sim macro (`copper-macros`) and the transpiler
//! (`copper-codegen`) must consume *one* authoritative control-flow analysis —
//! register inference (T1), read-timing facts (item 3), and the FSM report all
//! need the same CFG, and two analyses that must agree is itself a correctness
//! hazard. The open feasibility question was whether the `copper-macros`
//! **proc-macro** can depend on such a shared crate without a dependency cycle or
//! a heavy compile-time cost.
//!
//! **Answer (this crate demonstrates it): yes.** The analysis keys off
//! [`syn::ItemFn`] — the representation the proc-macro already receives *and* the
//! one the transpiler already builds (`copper_codegen::…::capture_frontend_ir`
//! takes a `&syn::ItemFn`; `transpile_source` is `parse_file` → `ItemFn`). So both
//! front-ends already hold the analysis input; no front-end unification is needed
//! for the analysis itself. And because `copper-core` is a leaf crate, a crate
//! depending only on `copper-core` + `syn` (both of which `copper-macros` already
//! pulls in) introduces **no cycle** and **no new heavy transitive dependency**.
//!
//! **Scope of THIS module (item 2).**
//! [`infer_registers`] is now full **backward liveness over a real CFG** (see
//! [`cfg`]) — it generalizes the G6 slice's minimal "pre-loop binding reassigned in
//! loop" criterion to registers *born inside* the loop and live across an interior
//! `.await` (e.g. `mac_pipeline`'s pipeline registers). The same CFG also powers
//! [`check_reachability`], the well-formedness check that every path through a
//! hardware loop must reach a tick.

use std::collections::BTreeSet;

use syn::ItemFn;

mod cfg;
pub mod consts;
mod data;
pub mod front_end;
pub use front_end::{check_front_end_rules, hardware_mode, parse_hardware_args, HardwareMode};
pub use data::{
    check_memory_port_index, check_outside_methods, memory_port_index_message, CHECKED_METHOD_MESSAGE,
    IS_VALID_MESSAGE, X_COMPARISON_MESSAGE, X_PATTERN_MESSAGE,
};
mod init;
pub mod memory_rules;
mod shape;

pub use cfg::{
    constant_empty_range_assertions, constant_ticking_for_ranges, empty_range_dependencies, range_key,
    stmt_contains_tick,
    with_empty_range_oracle, Cfg, DerivationFacts, EdgeKind, PhaseFacts, NESTED_EXIT_MESSAGE,
    ZERO_TIME_FOR_ITERATION_MESSAGE,
};
pub use init::{
    call_path_text, check_held_preloop_values, check_register_initializer_calls, held_preloop_message,
    held_preloop_values,
    check_register_initializers as check_register_initializers_with, is_constant_expr,
    is_core_const_ctor, non_const_calls_in_register_initializers,
    non_constant_register_initializers, preloop_register_initializers, register_init_call_message,
    register_init_message,
};
pub use shape::{
    check_awaits, check_counted_for_nesting, check_nested_loop_shape, check_no_preloop_tick,
    NESTED_RUNTIME_EMPTY_FOR_MESSAGE, PRELOOP_TICK_MESSAGE,
};

/// SPEC II.1 **R8** (D-8, Phase C step 3, 2026-11-06): a `sequential` or
/// `synchronizer` module has no plain `Out<T, D>`. Under the one-job body every
/// output of a clocked module is a register — a `RegOut<T, D>` written in the
/// resumption that ends at edge N+1 shows from `obs_{N+1}` — and a value that must
/// follow an input within one cycle (a passthrough, a Mealy output) is a
/// `#[hardware(combinational)]` module wired to the clocked module's `RegOut`s.
///
/// Reads the mode from the first token of the `#[hardware(...)]` attribute list
/// (CLAUDE.md: a flag after the mode must not drop the module from the check);
/// `Ok` for any other mode or for a function with no such attribute, so both
/// front-ends can call it on every target. The error is spanned at the port's
/// type; both front-ends report the same message.
pub fn check_clocked_outputs(f: &ItemFn) -> Result<(), syn::Error> {
    let clocked = f.attrs.iter().any(|a| {
        a.path().segments.last().is_some_and(|s| s.ident == "hardware")
            && a
                .meta
                .require_list()
                .ok()
                .and_then(|l| l.tokens.clone().into_iter().next())
                .is_some_and(|t| {
                    let t = t.to_string();
                    t == "sequential" || t == "synchronizer"
                })
    });
    if !clocked {
        return Ok(());
    }
    check_no_plain_out(f)
}

/// R8's core, for a caller that already knows `f` is a clocked module — the
/// macro, whose input fn no longer carries the `#[hardware]` attribute being
/// expanded. [`check_clocked_outputs`] is the attribute-reading wrapper the
/// transpiler and the corpus audit call.
pub fn check_no_plain_out(f: &ItemFn) -> Result<(), syn::Error> {
    for arg in &f.sig.inputs {
        let syn::FnArg::Typed(pt) = arg else { continue };
        let syn::Type::Path(tp) = &*pt.ty else { continue };
        let Some(last) = tp.path.segments.last() else { continue };
        if last.ident == "Out" {
            let name = match &*pt.pat {
                syn::Pat::Ident(i) => i.ident.to_string(),
                _ => "?".to_string(),
            };
            return Err(syn::Error::new_spanned(&pt.ty, clocked_out_message(&name)));
        }
    }
    Ok(())
}

/// The one message of R8, shared by both front-ends.
pub fn clocked_out_message(port: &str) -> String {
    format!(
        "output port `{port}` is a plain `Out` on a clocked module: under the one-job body \
         (D-8, SPEC II.1 R8) every output of a `sequential` or `synchronizer` module is a \
         register. Declare it `RegOut<T, D>` (written before the tick, it shows from the next \
         edge), or compute it in a `#[hardware(combinational)]` module wired to this module's \
         `RegOut`s if it must follow an input within the cycle"
    )
}

/// The register power-on rule R11 (`init`, 2026-10-29; D-21 A3, 2026-11-07): every
/// pre-loop binding the liveness inference calls a register must be initialized from
/// a Rust constant or from `Bits::x()`. The initializer is the flip-flop's power-on
/// value, which the transpiler states as an `initial` assignment. This is the
/// syntactic half both front-ends run — an initializer that reads a run-time value
/// (a parameter, an earlier pre-loop binding) is refused with R11's message; the
/// rest of "constant" is rustc's (the macro evaluates the initializer in a
/// `const { }` block) and, in the transpiler, [`check_register_initializer_calls_in`]
/// plus its fold. `Ok` for a module with no top-level loop.
/// [`check_register_initializers_with`] takes an already-inferred register set.
pub fn check_register_initializers(f: &ItemFn) -> Result<(), syn::Error> {
    match Cfg::build(f) {
        Some(cfg) => init::check_register_initializers(f, &cfg.registers()),
        None => Ok(()),
    }
}

/// The transpiler's stand-in for rustc's E0015 (D-21 A3): every call in a pre-loop
/// register initializer is to a core `Bits`/`Logic` constructor or to a fn
/// `is_const_fn` names (the file's `const fn`s). `Ok` for a module with no loop.
pub fn check_register_initializer_calls_in(
    f: &ItemFn,
    is_const_fn: &dyn Fn(&str) -> bool,
) -> Result<(), syn::Error> {
    match Cfg::build(f) {
        Some(cfg) => init::check_register_initializer_calls(f, &cfg.registers(), is_const_fn),
        None => Ok(()),
    }
}


/// Per-phase facts for the cycle-dataflow derivation table
/// (`design_docs/CYCLE_DATAFLOW_SEMANTICS.md` phase 1). Reporting only — no rule
/// keys on this. `None` for a module with no top-level loop (nothing sequential to
/// classify). See [`Cfg::derivation_facts`] for the phase notion and the two
/// documented first-cut approximations.
pub fn derivation_facts(f: &ItemFn) -> Option<DerivationFacts> {
    Cfg::build(f).map(|c| c.derivation_facts())
}

/// Infer the synthesizable **register set** of a sequential hardware module from
/// its control flow, via full backward liveness over the module's CFG ([`Cfg`]).
///
/// Criterion: a local is a flip-flop iff it is (a) *defined inside* the top-level
/// loop (a `let` binding or an assignment target) and (b) *live across a clock
/// edge* — its pre-tick value is read post-tick. This is the T1 answer: the
/// synthesizable register set computed from control flow, not read off rustc's
/// over-capturing `Future` layout.
///
/// A pre-loop binding only *read* in the loop is a constant/wire, not a register
/// (e.g. `lfsr`'s `xor_mask`) — excluded by (a). A same-cycle combinational temp is
/// excluded by (b). Returns register names sorted (stable structural output).
pub fn infer_registers(f: &ItemFn) -> Vec<String> {
    Cfg::build(f).map(|c| c.registers()).unwrap_or_default()
}








/// How many **clock phases** `f`'s top-level loop occupies — the number of distinct
/// cycles one iteration runs in. `None` for a module with no loop.
///
/// The transpiler re-derives this downstream, by splitting the LOWERED body at its
/// ticks, and the two disagree wherever a pass between them rewrites the tick
/// structure. Exposed so the disagreement can be measured.
pub fn clock_phase_count(f: &ItemFn) -> Option<usize> {
    Cfg::build(f).map(|c| c.clock_phase_count())
}

pub fn check_reachability(f: &ItemFn) -> Result<(), syn::Error> {
    match Cfg::build(f) {
        Some(cfg) => cfg
            .check_reachability()
            .map_err(|(span, msg)| syn::Error::new(span, msg)),
        None => Ok(()),
    }
}


/// Enforce the **memory-port staging rules** (rule 6, SPEC I.10.4) — a read result
/// observed only after the clock edge that produces it (d), and never observed on a port
/// nothing stages, in the loop or before it (c). Several stagings of one port in one
/// resumption are admitted, the last winning (I.10.1(b); clauses (a) and (b) retired by
/// D-16 B, removed by D-25). Rejected with a spanned [`syn::Error`].
///
/// Lives here, on the source, because both are questions about clock **edges**,
/// and the transpiler's own copy asked them of the loop's tick-delimited *segments*
/// — which `control_extract` legitimately erases by rewriting a branch- or
/// loop-nested tick into a single-tick `match pc` FSM. That made every memory design
/// needing extraction unwritable. See [`Cfg::check_memory_staging`] for the measured
/// false positive and the reachability formulation that replaces segment order.
///
/// A module with no top-level loop (combinational) has no clock edge to order
/// anything against and is `Ok`.
pub fn check_memory_staging(f: &ItemFn) -> Result<(), syn::Error> {
    match Cfg::build(f) {
        Some(cfg) => cfg
            .check_memory_staging()
            .map_err(|(span, msg)| syn::Error::new(span, msg)),
        None => Ok(()),
    }
}

/// Enforce **definite assignment** for a **combinational** module (`#[hardware(
/// combinational)]`): every `Out` port must be driven on all control paths or none
/// — a some-but-not-all (conditional) assignment infers a **latch**. Rejected with
/// a spanned [`syn::Error`]; the fix is to drive it on every path (add the missing
/// branch / `_` arm). This is the shared (c2) version of the transpiler's own
/// `check_no_latches`, so the sim macro rejects the latch at compile time too — not
/// only at transpile. See [`Cfg::check_definite_assignment`].
///
/// A **sequential** module (has a top-level clocked loop) is *not* checked: a
/// sequential `Out` legitimately holds when unwritten (an enabled register, verified
/// `sim ≡ BaseJump` on `bsg_dff_en`), so imposing "assign on all paths" there would
/// reject valid hardware.
pub fn check_definite_assignment(f: &ItemFn) -> Result<(), syn::Error> {
    // D2 is the combinational module's rule (I.11.3). The mode decides when the
    // attribute is still on the fn (the transpiler, the audits): a `structural`
    // body has no loop either, and its `Out`s are driven by its children (Phase C
    // step 10, found when the never-written clause landed). The macro, whose fn no
    // longer carries the attribute, calls this from its combinational arm only.
    if let Some(Ok(mode)) = crate::hardware_mode(f) {
        if mode != HardwareMode::Combinational {
            return Ok(());
        }
    }
    // A top-level loop ⇒ sequential ⇒ the enabled-register idiom is valid; skip.
    if Cfg::build(f).is_some() {
        return Ok(());
    }
    Cfg::build_combinational(f)
        .check_definite_assignment()
        .map_err(|(span, msg)| syn::Error::new(span, msg))
}

/// The **sequential register set** of an independent hand-written reference
/// SystemVerilog module — the reference side of G2's structural
/// register-inference correctness check
/// (`design_docs/SYNCHRONOUS_SEMANTICS_IMPL_PLAN.md`).
///
/// Returns the flip-flops: identifiers that are the target of a **nonblocking
/// assignment** (`x <= …`), minus **output ports** (`output [reg] … x`). This
/// rests on the universal RTL convention that `<=` is used only in sequential
/// (`always @(posedge …)`) blocks and blocking `=` in combinational ones — so it
/// correctly **excludes the `next_*` combinational regs** of the two-process
/// Moore idiom (which use `=`), and it excludes Copper's `RegOut` output-port axis
/// (`output reg`), leaving only genuine internal state.
///
/// This is deliberately a small, convention-based extractor for *test references*,
/// not a full Verilog parser.
pub fn reference_sv_registers(sv: &str) -> BTreeSet<String> {
    let text = strip_sv_noise(sv);
    let outputs = output_port_names(&text);
    let arrays = memory_array_names(sv);
    nonblocking_assign_targets(&text)
        .difference(&outputs)
        .filter(|n| !arrays.contains(*n))
        .cloned()
        .collect()
}

/// Identifiers declared as *unpacked arrays* — `logic [15:0] mem [0:255];`,
/// `reg [15:0] ram [1023:0];`. These are memory arrays, a different storage class
/// from the flip-flops this module reasons about.
///
/// They have to be excluded explicitly. `strip_sv_noise` removes bracket groups,
/// so a memory write `mem[addr] <= data;` reduces to `mem <= data;` and would
/// otherwise be counted as a flip-flop named `mem` — which no source-level
/// register inference will ever produce, since `infer_registers` names locals that
/// live across a tick and a `Memory<..>` binding is not one. The same applies to a
/// hand-written reference SV: `examples/memory/sv/dual_port_ram.sv` declares
/// `reg [15:0] ram [1023:0]` and writes `ram[addra] <= dia`.
///
/// Nothing is lost by excluding them: Copper has no unpacked-array registers —
/// `Bits<N>` is packed — so an excluded name could never have been a real match.
fn memory_array_names(sv: &str) -> BTreeSet<String> {
    const DECL_KW: [&str; 4] = ["reg", "logic", "wire", "bit"];
    let mut names = BTreeSet::new();
    for line in sv.lines() {
        let line = match line.find("//") {
            Some(i) => &line[..i],
            None => line,
        };
        let line = line.trim();
        // A declaration, not a use: `assign d = mem[a];` also ends in `];`.
        let Some(first) = line.split_whitespace().next() else { continue };
        if !DECL_KW.contains(&first) {
            continue;
        }
        let Some(body) = line.strip_suffix(';') else { continue };
        let body = body.trim_end();
        // An unpacked dimension trails the name: `… mem [0:255]`.
        if !body.ends_with(']') {
            continue;
        }
        let Some(open) = body.rfind('[') else { continue };
        let before = body[..open].trim_end();
        let name: String = before
            .chars()
            .rev()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        if !name.is_empty() && !DECL_KW.contains(&name.as_str()) {
            names.insert(name);
        }
    }
    names
}

/// The two ways G2 compares an inferred register set to a reference SV's registers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RegMatch {
    /// Names match exactly — valid only when the reference SV is a *faithful
    /// translation* that mirrors the design's own names (e.g. `mac_fsm.sv`).
    NameExact,
    /// Only the flip-flop *count* matches — the honest bar for a truly independent
    /// reference whose author chose different names/encoding (e.g. a two-process
    /// Moore `cur_state` vs Copper's `state`). Item 2 strengthens this to
    /// storage-equivalence (count + per-register bit-width) once inference carries
    /// widths from resolved Rust types.
    StorageEquivalent,
}

/// Assert that the register set inferred from `dut_src` (a **single** hardware fn)
/// matches the sequential registers of reference SystemVerilog `sv` under `mode`.
/// Convenience for hand-written single-fn snippets (the unit tests).
pub fn assert_registers_match_reference_sv(dut_src: &str, sv: &str, mode: RegMatch) {
    let f: ItemFn = syn::parse_str(dut_src).expect("DUT source parses as a single hardware fn");
    assert_fn_registers_match_reference_sv(&f, sv, mode);
}

/// Assert the registers of a hardware **module located in a source file** match a
/// reference SV under `mode`. This is the G2 harness entry point:
/// `tests/common::EquivalenceTest` calls it with the fixture source (which carries
/// enums/`use`s alongside the fn) and an *independent hand-written* reference SV,
/// making register-inference correctness a checked part of the equivalence suite.
///
/// `module` selects the `#[hardware]` fn by name; `None` requires the file to hold
/// exactly one.
pub fn assert_source_registers_match_reference_sv(
    src: &str,
    module: Option<&str>,
    sv: &str,
    mode: RegMatch,
) {
    let f = find_hardware_fn(src, module);
    assert_fn_registers_match_reference_sv(&f, sv, mode);
}

/// The [`ItemFn`]-level assert both convenience wrappers delegate to.
pub fn assert_fn_registers_match_reference_sv(f: &ItemFn, sv: &str, mode: RegMatch) {
    let inferred: BTreeSet<String> = infer_registers(f).into_iter().collect();
    let reference = reference_sv_registers(sv);
    match mode {
        RegMatch::NameExact => assert_eq!(
            inferred, reference,
            "inferred register set does not name-match the reference SV's sequential registers"
        ),
        RegMatch::StorageEquivalent => assert_eq!(
            inferred.len(),
            reference.len(),
            "inferred flip-flop count {} != reference count {} (inferred {inferred:?} vs \
             reference {reference:?})",
            inferred.len(),
            reference.len()
        ),
    }
}

/// Locate a `#[hardware(...)]` function in a source file — by `module` name, or
/// the sole one when `module` is `None`. Panics with a clear message otherwise
/// (this is a test-harness helper, so a missing/ambiguous module is a test bug).
fn find_hardware_fn(src: &str, module: Option<&str>) -> ItemFn {
    let file = syn::parse_file(src).expect("DUT source parses as a Rust file");
    let hw: Vec<&syn::ItemFn> = file
        .items
        .iter()
        .filter_map(|i| match i {
            syn::Item::Fn(f) if is_hardware_fn(f) => Some(f),
            _ => None,
        })
        .collect();
    match module {
        Some(name) => hw
            .into_iter()
            .find(|f| f.sig.ident == name)
            .unwrap_or_else(|| panic!("no #[hardware] fn named `{name}` in source"))
            .clone(),
        None => match hw.as_slice() {
            [only] => (*only).clone(),
            other => panic!(
                "expected exactly one #[hardware] fn (found {}); pass a module name",
                other.len()
            ),
        },
    }
}

/// Whether `f` carries a `#[hardware(...)]` attribute.
fn is_hardware_fn(f: &ItemFn) -> bool {
    f.attrs
        .iter()
        .any(|a| a.path().segments.last().is_some_and(|s| s.ident == "hardware"))
}

/// Drop `//` line comments and `[..]` width specs so the register extractors see
/// bare declarations/assignments.
fn strip_sv_noise(sv: &str) -> String {
    let mut out = String::with_capacity(sv.len());
    for line in sv.lines() {
        let line = match line.find("//") {
            Some(i) => &line[..i],
            None => line,
        };
        let mut depth = 0u32; // strip [ .. ] (bit widths / indices)
        for ch in line.chars() {
            match ch {
                '[' => depth += 1,
                ']' => depth = depth.saturating_sub(1),
                _ if depth == 0 => out.push(ch),
                _ => {}
            }
        }
        out.push('\n');
    }
    out
}

/// Identifiers that are the LHS of a nonblocking assignment `x <= …`.
fn nonblocking_assign_targets(text: &str) -> BTreeSet<String> {
    let bytes = text.as_bytes();
    let mut names = BTreeSet::new();
    let mut i = 0;
    while let Some(rel) = text[i..].find("<=") {
        let pos = i + rel;
        // Walk back over whitespace, then capture the identifier.
        let mut j = pos;
        while j > 0 && bytes[j - 1].is_ascii_whitespace() {
            j -= 1;
        }
        let end = j;
        while j > 0 && (bytes[j - 1].is_ascii_alphanumeric() || bytes[j - 1] == b'_') {
            j -= 1;
        }
        // A comparison `(v <= 8'd99)` is no assignment: a nonblocking target starts a
        // statement, never follows an open parenthesis (the emitter parenthesizes every
        // comparison; the range-pattern guards of 2026-11-08 put one in `always_ff`,
        // MEASUREMENTS §Z.5).
        let mut k = j;
        while k > 0 && bytes[k - 1].is_ascii_whitespace() {
            k -= 1;
        }
        let after_paren = k > 0 && bytes[k - 1] == b'(';
        if j < end && !after_paren {
            names.insert(text[j..end].to_string());
        }
        i = pos + 2;
    }
    names
}

/// Identifiers declared as module output ports (`output [reg|wire|logic] … name`),
/// handling both ANSI header ports and separate `output` declarations. Each
/// `output` occurrence is scanned up to the next `;` or `)`, skipping type
/// keywords, and stopping at the next direction keyword (for comma-joined headers
/// like `… output out )`).
fn output_port_names(text: &str) -> BTreeSet<String> {
    const TYPE_KW: [&str; 6] = ["reg", "wire", "logic", "signed", "output", "bit"];
    let bytes = text.as_bytes();
    let mut names = BTreeSet::new();
    let mut k = 0;
    while let Some(rel) = text[k..].find("output") {
        let start = k + rel;
        let next = start + "output".len();
        let prev_ok = start == 0 || !bytes[start - 1].is_ascii_alphanumeric();
        let word_ok = next >= text.len() || !bytes[next].is_ascii_alphanumeric();
        if prev_ok && word_ok {
            let stop = text[next..]
                .find([';', ')'])
                .map(|p| next + p)
                .unwrap_or(text.len());
            for tok in text[next..stop].split(|c: char| !(c.is_ascii_alphanumeric() || c == '_')) {
                if matches!(tok, "input" | "inout") {
                    break; // next port in an ANSI header
                }
                if !tok.is_empty() && !TYPE_KW.contains(&tok) {
                    names.insert(tok.to_string());
                }
            }
        }
        k = next;
    }
    names
}

/// Production entry point (item 2): infer registers from the shared frontend IR
/// via full backward liveness over the CFG. Stubbed for the G6 slice — present to
/// pin the intended type dependency on `copper-core`'s IR, confirming the shared
/// crate compiles against the same FIR both front-ends build.
pub fn registers_from_fir(_ir: &copper_core::frontend_ir::FrontendModuleIR) -> Vec<String> {
    unimplemented!("item 2 follow-on: backward liveness over a CFG built from the shared FIR")
}

#[cfg(test)]
mod tests {
    /// A memory array is storage, but not a flip-flop: `strip_sv_noise` turns
    /// `mem[a] <= d;` into `mem <= d;`, which would otherwise be reported as a
    /// register named `mem` that no source-level inference can ever produce.
    #[test]
    fn memory_arrays_are_not_reference_registers() {
        let sv = r#"
module m (input logic clk, input logic [7:0] a, output logic [15:0] o);
    logic [15:0] mem [0:255];
    logic [15:0] q;
    always_ff @(posedge clk) begin
        q <= mem[a];
        mem[a] <= 16'd7;
    end
    assign o = q;
endmodule
"#;
        let regs = reference_sv_registers(sv);
        assert!(regs.contains("q"), "a real flip-flop must still be reported: {regs:?}");
        assert!(
            !regs.contains("mem"),
            "an unpacked array is a memory, not a register: {regs:?}"
        );
    }

    /// The exclusion must key on the DECLARATION, not on the name appearing with
    /// brackets anywhere — a plain register indexed on the left (`q[3] <= x`) is
    /// still a register.
    #[test]
    fn bit_assigned_register_is_still_a_reference_register() {
        let sv = r#"
module m (input logic clk, output logic o);
    logic [7:0] q;
    always_ff @(posedge clk) begin
        q[3] <= 1'b1;
    end
    assign o = q[0];
endmodule
"#;
        assert!(reference_sv_registers(sv).contains("q"));
    }

    use super::*;

    /// The `mac_fsm` coding (mirrors `tests/fixtures/mac_fsm_dut.rs`): a single-tick
    /// FSM whose persistent state is four pre-loop locals.
    const MAC_FSM_SRC: &str = r#"
        #[hardware(sequential)]
        async fn mac_fsm(
            clk: Clock<MainClk>,
            a: In<Bits<8>, MainClk>,
            b: In<Bits<8>, MainClk>,
            c: In<Bits<8>, MainClk>,
            out: RegOut<Bits<8>, MainClk>,
        ) {
            let mut stage = Stage::Load;
            let mut product: Bits<8> = Bits::from_lit::<0>();
            let mut c_latch: Bits<8> = Bits::from_lit::<0>();
            let mut result: Bits<8> = Bits::from_lit::<0>();
            loop {
                match stage {
                    Stage::Load => {
                        product = a.read() * b.read();
                        c_latch = c.read();
                        stage = Stage::Mul;
                    }
                    Stage::Mul => {
                        result = product.clone() + c_latch.clone();
                        stage = Stage::Out;
                    }
                    Stage::Out => {
                        out.write(result.clone());
                        stage = Stage::Load;
                    }
                }
                clk.tick().await;
            }
        }
    "#;

    fn parse(src: &str) -> ItemFn {
        syn::parse_str(src).expect("parse ItemFn")
    }

    #[test]
    fn infers_mac_fsm_register_set() {
        let regs = infer_registers(&parse(MAC_FSM_SRC));
        assert_eq!(
            regs,
            vec![
                "c_latch".to_string(),
                "product".to_string(),
                "result".to_string(),
                "stage".to_string()
            ],
            "inferred register set does not match the mac_fsm state"
        );
    }

    /// A pre-loop constant that is only read in the loop is a wire, not a register.
    #[test]
    fn constant_not_counted_as_register() {
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
        let regs = infer_registers(&parse(src));
        assert_eq!(regs, vec!["state".to_string()], "mask is a constant, not a register");
    }

    /// Structural reg-for-reg check against the INDEPENDENT hand-written reference
    /// SystemVerilog (`tests/fixtures/timing_probe_sv/mac_fsm.sv`) — this is gate
    /// G2's definition of register-inference correctness, on the G6 slice. Parses
    /// the reference's internal `reg` declarations (excluding the `output reg`,
    /// which is Copper's `RegOut` output-port axis, not internal state) and asserts
    /// they equal the inferred set.
    fn read_ref_sv(rel: &str) -> String {
        let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(rel);
        std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()))
    }

    /// G2 structural correctness, NAME-EXACT form: `mac_fsm.sv` is a faithful
    /// translation mirroring the design's names, so the inferred set matches the
    /// reference's sequential registers name-for-name.
    #[test]
    fn inferred_set_matches_independent_reference_sv() {
        let sv = read_ref_sv("../tests/fixtures/timing_probe_sv/mac_fsm.sv");
        assert_registers_match_reference_sv(MAC_FSM_SRC, &sv, RegMatch::NameExact);
    }

    /// The reference extractor returns the flip-flops only: the `output reg out`
    /// (Copper's RegOut axis) is excluded despite being nonblocking-assigned.
    #[test]
    fn reference_extractor_excludes_output_port() {
        let sv = read_ref_sv("../tests/fixtures/timing_probe_sv/mac_fsm.sv");
        let regs = reference_sv_registers(&sv);
        assert!(!regs.contains("out"), "output reg must be excluded, got {regs:?}");
        assert_eq!(
            regs,
            ["c_latch", "product", "result", "stage"]
                .iter()
                .map(|s| s.to_string())
                .collect::<BTreeSet<_>>()
        );
    }

    /// G2 structural correctness, STORAGE-EQUIVALENT form + the naming nuance: the
    /// independent `pattern_detector_010.sv` is a two-process Moore machine whose
    /// flip-flop is `cur_state` (its `next_state` is combinational — blocking `=`,
    /// correctly excluded) — a *different name* from Copper's `state`. Names cannot
    /// match, but the flip-flop COUNT does. This is why G2's independent-reference
    /// bar is storage-equivalence, not name-equality.
    #[test]
    fn det_010_reference_is_storage_equivalent_not_name_exact() {
        let det_010_src = r#"
            #[hardware(sequential)]
            async fn det_010(clk: Clock<C>, rstn: In<Logic, C>, in_i: In<Logic, C>, out_o: RegOut<Logic, C>) {
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
        let sv = read_ref_sv("../examples/sequential/sv/pattern_detector_010.sv");

        // Flip-flop set: only cur_state (next_state is combinational, excluded).
        let reference = reference_sv_registers(&sv);
        assert_eq!(reference, ["cur_state"].iter().map(|s| s.to_string()).collect::<BTreeSet<_>>());

        // Names differ (state vs cur_state) — so NameExact would fail, but
        // StorageEquivalent (count) holds.
        assert_registers_match_reference_sv(det_010_src, &sv, RegMatch::StorageEquivalent);
        let inferred: BTreeSet<String> = infer_registers(&parse(det_010_src)).into_iter().collect();
        assert_ne!(inferred, reference, "names are expected to differ for an independent reference");
    }
}

// ── The admissible surface ───────────────────────────────────────────────────
//
// The transpiler says no in 108 places across four stages, through 21 typed
// variants, 76 of them sharing one `UnsupportedConstruct` carrying free text — so
// two unrelated blockers are indistinguishable to anything but a human reading the
// string, and 60 of the sites have never fired. That is the shape of a language
// defined by SUBTRACTION: everything Rust can say, minus whatever each pass happens
// to reject when it gets there.
//
// This is the other direction — a POSITIVE grammar, stated once, checked early, on
// the representation both front-ends already hold. What it accepts is the language;
// anything else is refused at the declaration with a span, rather than after the
// whole pipeline has run.
//
// # It is built one rule at a time, and calibrated
//
// The grammar must NEVER reject a module the transpiler lowers today — that would
// break working designs to tidy up an error message. So it starts permissive and
// gains rules only with evidence, and the criterion is asymmetric and testable:
//
//   admissible(m) == Err  ⟹  transpile(m) == Err     (no false rejection)
//
// `copper-codegen/tests/admissible_calibration.rs` asserts that over the whole
// corpus. The converse is the GOAL, not yet the rule: as rules land, more of what
// the transpiler refuses late is caught here early, and the corresponding downstream
// refusal becomes unreachable and deletable. The calibration test prints how far
// along that is on every run.

/// Types a `#[hardware]` fn may name — rule 1 of the admissible grammar.
///
/// Every port payload and every annotated `let` must be one of these. It is the
/// rule with the most evidence behind it: `Vec<Bits<32>>` as a port and a
/// `Memory<…>` parameter are two of the nine refusals in the corpus today, and
/// struct- and tuple-typed locals are what stop `rv32i_cpu_pipelined`. Measured
/// before writing it: NO module that transpiles names a type outside this set, so
/// the rule rejects only designs that already fail — just earlier, and with a span
/// pointing at the declaration instead of a width error three passes downstream.
fn admissible_type(ty: &syn::Type) -> bool {
    match ty {
        // `[T; N]` — an array of an admissible element. The length is not checked
        // here: it may be a literal, a const generic, or a file-scope `const`, and
        // deciding which is `chir_lower`'s job, not the grammar's.
        syn::Type::Array(a) => admissible_type(&a.elem),
        // A reference is transparent: `&Clock<D>` is a clock. Whether a reference is
        // itself admissible in a given position is a separate rule.
        syn::Type::Reference(r) => admissible_type(&r.elem),
        syn::Type::Paren(p) => admissible_type(&p.elem),
        syn::Type::Path(tp) => {
            let Some(seg) = tp.path.segments.last() else { return false };
            let name = seg.ident.to_string();
            matches!(
                name.as_str(),
                "Logic"
                    | "Bits"
                    | "bool"
                    | "u8" | "u16" | "u32" | "u64" | "u128" | "usize"
                    | "i8" | "i16" | "i32" | "i64" | "i128" | "isize"
                    // Port and resource wrappers. Their payloads are checked by the
                    // caller, which knows which argument position carries one.
                    | "Clock" | "In" | "Out" | "RegOut" | "Memory"
            )
        }
        _ => false,
    }
}

/// The payload type of a port wrapper — the `T` in `In<T, D>` / `Out<T, D>` /
/// `RegOut<T, D>`. `None` for anything without one (a `Clock<D>` has a domain, not
/// a payload; a `Memory<…>`'s element type is checked by its own rule).
fn port_payload(ty: &syn::Type) -> Option<&syn::Type> {
    let syn::Type::Path(tp) = ty else { return None };
    let seg = tp.path.segments.last()?;
    if !matches!(seg.ident.to_string().as_str(), "In" | "Out" | "RegOut") {
        return None;
    }
    let syn::PathArguments::AngleBracketed(ab) = &seg.arguments else { return None };
    ab.args.iter().find_map(|a| match a {
        syn::GenericArgument::Type(t) => Some(t),
        _ => None,
    })
}

/// Check a `#[hardware]` fn against the admissible grammar.
///
/// Returns the first violation as a spanned error, so the diagnostic lands on the
/// declaration that is out of bounds rather than wherever a later pass tripped over
/// it. See the module note above for why this is a positive grammar and how it is
/// calibrated against the transpiler.
pub fn check_admissible(f: &ItemFn) -> Result<(), syn::Error> {
    use syn::spanned::Spanned;

    // Rule 1c — `Bits<N>`'s width bound (SPEC I.14.1(c), II.1.2 R16; 2026-11-08,
    // MEASUREMENTS §V.2): a width written as a constant (a literal, arithmetic over
    // literals, a `{ … }` block of one) must be 1 ≤ N ≤ 128, wherever a `Bits` path is
    // named — a port, an annotated `let`, a turbofish (`Bits::<0>::zero()`). A width
    // naming a const generic or a `const` item is outside what the source shows; a
    // generic module's width is chosen where it is instantiated.
    if let Some((span, n)) = bits_width_outside_bound(f) {
        return Err(syn::Error::new(span, bits_width_message(n)));
    }

    // Rule 1a — port payloads.
    for arg in &f.sig.inputs {
        let syn::FnArg::Typed(pt) = arg else { continue };
        if let Some(payload) = port_payload(&pt.ty) {
            if !admissible_type(payload) {
                return Err(syn::Error::new(
                    payload.span(),
                    format!(
                        "`{}` is not a hardware type. A port carries `Logic`, `Bits<N>`, \
                         `bool`, an integer type, or a fixed-size array of those — a value \
                         with a width the synthesized wire can have. A `Vec`, a struct or a \
                         tuple has no such width.",
                        quote_ty(payload)
                    ),
                ));
            }
        }
    }

    // Rule 1b — annotated locals. An UNANNOTATED `let` is left alone: its type comes
    // from inference, and duplicating that here is the second-analysis-that-must-agree
    // mistake this crate exists to avoid.
    struct V(Option<syn::Error>);
    impl<'ast> syn::visit::Visit<'ast> for V {
        fn visit_local(&mut self, l: &'ast syn::Local) {
            if self.0.is_none() {
                if let syn::Pat::Type(pt) = &l.pat {
                    if !admissible_type(&pt.ty) {
                        self.0 = Some(syn::Error::new(
                            pt.ty.span(),
                            format!(
                                "`{}` is not a hardware type. A local holds `Logic`, \
                                 `Bits<N>`, `bool`, an integer type, or a fixed-size array \
                                 of those. A struct or tuple of several values is written as \
                                 one local per field.",
                                quote_ty(&pt.ty)
                            ),
                        ));
                    }
                }
            }
            syn::visit::visit_local(self, l);
        }
    }
    let mut v = V(None);
    syn::visit::Visit::visit_block(&mut v, &f.block);
    match v.0 {
        Some(e) => Err(e),
        None => Ok(()),
    }
}

/// R16's width message (SPEC I.14.1(c)).
pub fn bits_width_message(n: i128) -> String {
    format!(
        "`Bits<{n}>` is not a hardware type: a `Bits<N>` has 1 ≤ N ≤ 128 bits (SPEC I.14.1(c), \
         II.1.2 R16). `Bits<0>` has no value to observe, and a wider vector's arithmetic does not \
         monomorphize. Use a width in 1..=128, or an array of `Bits` for a wider value."
    )
}

/// The first `Bits<N>` in `f` (signature or body) whose width is written as a
/// constant outside 1..=128, with its span and value.
fn bits_width_outside_bound(f: &ItemFn) -> Option<(proc_macro2::Span, i128)> {
    use syn::spanned::Spanned;
    fn width(e: &syn::Expr) -> Option<i128> {
        match e {
            syn::Expr::Block(b) if b.block.stmts.len() == 1 => match &b.block.stmts[0] {
                syn::Stmt::Expr(inner, None) => width(inner),
                _ => None,
            },
            other => consts::eval_syn(other, &|_| None),
        }
    }
    struct V(Option<(proc_macro2::Span, i128)>);
    impl<'ast> syn::visit::Visit<'ast> for V {
        fn visit_path_segment(&mut self, s: &'ast syn::PathSegment) {
            if self.0.is_none() && s.ident == "Bits" {
                if let syn::PathArguments::AngleBracketed(ab) = &s.arguments {
                    if ab.args.len() == 1 {
                        let n = match &ab.args[0] {
                            syn::GenericArgument::Const(e) => width(e),
                            _ => None,
                        };
                        if let Some(n) = n.filter(|n| !(1..=128).contains(n)) {
                            self.0 = Some((s.span(), n));
                        }
                    }
                }
            }
            syn::visit::visit_path_segment(self, s);
        }
    }
    let mut v = V(None);
    syn::visit::Visit::visit_item_fn(&mut v, f);
    v.0
}

/// Render a type for a diagnostic. Deliberately does NOT pull in `quote` — this
/// crate has no proc-macro dependencies and a message is not a reason to gain one.
fn quote_ty(ty: &syn::Type) -> String {
    match ty {
        syn::Type::Path(tp) => tp
            .path
            .segments
            .last()
            .map(|s| s.ident.to_string())
            .unwrap_or_else(|| "?".to_string()),
        syn::Type::Array(a) => format!("[{}; _]", quote_ty(&a.elem)),
        syn::Type::Reference(r) => format!("&{}", quote_ty(&r.elem)),
        syn::Type::Tuple(t) => format!(
            "({})",
            t.elems.iter().map(quote_ty).collect::<Vec<_>>().join(", ")
        ),
        _ => "this type".to_string(),
    }
}
