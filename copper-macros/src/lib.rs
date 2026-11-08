use proc_macro::TokenStream;
use quote::quote;
use std::collections::HashSet;
use syn::{parse_macro_input, Error, FnArg, ItemFn, Pat, Type};

use copper_analysis::HardwareMode;

struct HardwareArgs {
    mode: HardwareMode,
}

/// The attribute's arguments, through the parser both front-ends share (R9).
fn parse_hardware_mode(args: TokenStream) -> Result<HardwareArgs, Error> {
    copper_analysis::parse_hardware_args(&args.to_string())
        .map(|mode| HardwareArgs { mode })
        .map_err(|m| Error::new(proc_macro2::Span::call_site(), m))
}

fn outer_type_name(ty: &Type) -> Option<String> {
    if let Type::Path(type_path) = ty {
        type_path.path.segments.last().map(|s| s.ident.to_string())
    } else {
        None
    }
}

/// The `D` in a `Clock<D>` parameter's type (its one type argument).
fn clock_domain_type(ty: &Type) -> Option<Type> {
    let Type::Path(tp) = ty else { return None };
    let seg = tp.path.segments.last()?;
    let syn::PathArguments::AngleBracketed(ab) = &seg.arguments else { return None };
    ab.args.iter().find_map(|a| match a {
        syn::GenericArgument::Type(t) => Some(t.clone()),
        _ => None,
    })
}


// ── Clock-domain-crossing (CDC) enforcement ─────────────────────────────────
//
// A regular `#[hardware(sequential)]` module is single-domain: every port must be
// in the module's own clock domain. A signal from another domain may only be
// brought in through a *synchronizer* — a `#[hardware(synchronizer)]` module such
// as the library `sync_2ff`, which is exempt from this rule. Combined with the
// phantom-domain types (which already reject wiring a foreign wire into a native
// port), this makes every clock-domain crossing a real, honest synchronizer
// module and forbids crossing domains implicitly inside ordinary logic. See the
// audit in `copper-core/src/cdc.rs`.
fn wrap_combinational(mut f: ItemFn) -> ItemFn {
    let body = &f.block;
    let new_body: syn::Block = syn::parse_quote! {
        {
            loop {
                #body
                ::copper_sim::delta_yield().await;
            }
        }
    };
    f.sig.asyncness = Some(syn::token::Async {
        span: proc_macro2::Span::call_site(),
    });
    f.block = Box::new(new_body);
    f
}

/// Turn a transformed `async fn m(params) { body }` into
/// `fn m(params) -> HardwareModule<impl Future<Output = ()>>
///  { HardwareModule::__new(async move { body }) }`.
///
/// This is what makes `#[hardware]` mandatory: a module's value is now a
/// `HardwareModule`, the only thing the executor's spawn APIs accept. A bare
/// `async fn` produces a plain `Future` and cannot be spawned — you can't forget
/// the attribute.
/// Wrap each pre-loop register initializer (`copper_analysis::preloop_register_initializers`)
/// in an inline `const { … }` block, spanned at the initializer so rustc's verdict
/// points at the user's expression (D-21 A3).
fn const_wrap_register_initializers(f: &mut ItemFn, registers: &[String]) {
    use syn::spanned::Spanned;
    let names: HashSet<String> = copper_analysis::preloop_register_initializers(f, registers)
        .into_iter()
        .map(|(n, _)| n)
        .collect();
    for s in f.block.stmts.iter_mut() {
        match s {
            syn::Stmt::Expr(syn::Expr::Loop(_), _) => break,
            syn::Stmt::Local(l) => {
                let name = match &l.pat {
                    Pat::Ident(pi) => Some(pi.ident.to_string()),
                    Pat::Type(pt) => match &*pt.pat {
                        Pat::Ident(pi) => Some(pi.ident.to_string()),
                        _ => None,
                    },
                    _ => None,
                };
                if !name.is_some_and(|n| names.contains(&n)) {
                    continue;
                }
                if let Some(init) = &mut l.init {
                    let e = &init.expr;
                    let span = e.span();
                    *init.expr = syn::parse_quote_spanned!(span=> const { #e });
                }
            }
            _ => {}
        }
    }
}

/// R5, the macro's half (D-21 A3's mechanism): every bound of a tick-free counted
/// `for` is evaluated in an inline `const { … }` block, spanned at the bound, so
/// rustc judges that it is a Rust constant (`copper_analysis::front_end::
/// check_tick_free_for_bounds` has already refused a run-time read with R5's
/// message).
fn const_wrap_for_bounds(f: &mut ItemFn) {
    use syn::spanned::Spanned;
    use syn::visit_mut::VisitMut;
    struct W;
    impl VisitMut for W {
        fn visit_expr_for_loop_mut(&mut self, fl: &mut syn::ExprForLoop) {
            syn::visit_mut::visit_expr_for_loop_mut(self, fl);
            let tick_free = !fl.body.stmts.iter().any(copper_analysis::stmt_contains_tick);
            if let (true, syn::Expr::Range(r)) = (tick_free, &mut *fl.expr) {
                for b in [r.start.as_mut(), r.end.as_mut()].into_iter().flatten() {
                    if matches!(**b, syn::Expr::Const(_)) {
                        continue;
                    }
                    let e = b.clone();
                    let span = e.span();
                    **b = syn::parse_quote_spanned!(span=> const { #e });
                }
            }
        }
    }
    W.visit_block_mut(&mut f.block);
}

fn wrap_in_module(f: &mut ItemFn) {
    let body = f.block.clone();
    // A clocked module (one `Clock<D>` parameter: `sequential`, `synchronizer`) is
    // tagged with `D`, the domain whose edges resume it (SPEC §I.12.1). The tag is
    // read only by the one-resumption executor (Phase C step 2); the body is
    // wrapped unchanged either way. A combinational module has no clock and no tag.
    let clock_domains: Vec<Type> = f
        .sig
        .inputs
        .iter()
        .filter_map(|a| match a {
            FnArg::Typed(pt) if outer_type_name(&pt.ty).as_deref() == Some("Clock") => {
                clock_domain_type(&pt.ty)
            }
            _ => None,
        })
        .collect();
    let name = f.sig.ident.to_string();
    f.sig.asyncness = None;
    f.sig.output = syn::parse_quote! {
        -> ::copper_sim::HardwareModule<impl ::core::future::Future<Output = ()>>
    };
    f.block = Box::new(match clock_domains.as_slice() {
        [d] => syn::parse_quote! {
            {
                ::copper_sim::HardwareModule::__new_in_domain::<#d>(async move #body).__named(#name)
            }
        },
        _ => syn::parse_quote! {
            {
                ::copper_sim::HardwareModule::__new(async move #body).__named(#name)
            }
        },
    });
}

/// #[hardware] macro for defining hardware modules
///
/// Marks a function as a hardware module.
/// - `#[hardware(sequential)]` — async fn, must have `Clock<D>`, `In<T>`, `Out<T>` params, no return value
/// - `#[hardware(combinational)]` — non-async fn, must have `In<T>` and `Out<T>` params, no Clock, no return value.
///   The macro wraps the body in `loop { <body>; delta_yield().await; }` automatically for combinational modules.
#[proc_macro_attribute]
pub fn hardware(args: TokenStream, input: TokenStream) -> TokenStream {
    let input_fn = parse_macro_input!(input as ItemFn);
    let HardwareArgs { mode: hardware_mode } =
        match parse_hardware_mode(args) {
            Ok(parsed) => parsed,
            Err(err) => return err.to_compile_error().into(),
        };

    // SPEC II.1.3 (P10, Phase C step 10): the syntactic rules of II.1 and II.2 —
    // the header (R20, R10), R17, I.14.3's OUTSIDE methods, the memory port index,
    // D3, R16, R18, R7, R12, R13/R14, the CDC port rule and R19 — are one function
    // of `copper-analysis` with one message each, run by `transpile_target` too, in
    // the same order.
    if let Err(err) = copper_analysis::check_front_end_rules(&input_fn, hardware_mode) {
        return err.to_compile_error().into();
    }

    match hardware_mode {
        HardwareMode::Sequential | HardwareMode::Synchronizer => {
            // c2 (item 2): reachability well-formedness — every path through the
            // module's top-level loop must reach a `clk.tick().await`. A tickless
            // path is a zero-time combinational cycle (it would hang the sim); it is
            // now a hard, spanned compile error rather than an accident silently
            // masked by the single-trailing-tick construction. Checked on the
            // pristine ItemFn, before any rewrite.
            // One control-flow graph per module. Every check below is a method on
            // it, and the register set it caches is what each of them would
            // otherwise recompute (the liveness fixed point dominated the
            // attribute's cost on the transpilable CPU: four rebuilds of it,
            // measured 2026-09-03). `None` only for a body with no top-level
            // loop, which `validate_hardware_fn` has already refused.
            let cfg = copper_analysis::Cfg::build(&input_fn);
            if let Err(err) = cfg
                .as_ref()
                .map_or(Ok(()), |c| c.check_reachability().map_err(|(s, m)| Error::new(s, m)))
            {
                return err.to_compile_error().into();
            }
            // Front-end parity (2026-10-23): the shapes the transpiler refuses are
            // refused here too, so a module that simulates also transpiles. A nested
            // loop must end at a clock boundary (the repeating wait is
            // `loop { <test>; tick }`, never `loop { tick; <test> }`), and the only
            // `await` a body may write is `<clock>.tick().await`. Both on the pristine
            // fn, before any rewrite, and after reachability so a module that never
            // ticks gets that message rather than one about a nested loop.
            // SPEC II.1 R8 (D-8, Phase C step 3): a clocked module has no plain
            // `Out`. Same function, same message, in `transpile_target`.
            if let Err(err) = copper_analysis::check_no_plain_out(&input_fn) {
                return err.to_compile_error().into();
            }
            if let Err(err) = copper_analysis::check_nested_loop_shape(&input_fn) {
                return err.to_compile_error().into();
            }
            if let Err(err) = copper_analysis::check_awaits(&input_fn) {
                return err.to_compile_error().into();
            }
            // SPEC II.1.2 R21: no tick before the module loop. Same function, same
            // message, in `transpile_target` (Phase C step 5, 2026-11-07).
            if let Err(err) = copper_analysis::check_no_preloop_tick(&input_fn) {
                return err.to_compile_error().into();
            }
            // A run-time-empty counted `for` ending an enclosing ticking `for`
            // (MEASUREMENTS §J.15): same function, same message, in `transpile_target`.
            if let Err(err) = copper_analysis::check_counted_for_nesting(&input_fn) {
                return err.to_compile_error().into();
            }
            // A pre-loop register must start from a constant or from `Bits::x()`
            // (2026-10-29): its initializer is the flip-flop's power-on value, which
            // the transpiler states as an `initial` assignment — and a value it
            // cannot state it used to drop silently (`lfsr`'s `Bits::from_u32(1)`,
            // found by the multi-sequence sweep). Same rule, same message, in
            // `transpile_target`; no opt-out in either front-end.
            if let Err(err) = cfg.as_ref().map_or(Ok(()), |c| {
                copper_analysis::check_register_initializers_with(&input_fn, &c.registers())
            }) {
                return err.to_compile_error().into();
            }
            // D-21 H1 (decided 2026-11-07): a held pre-loop value is refused. Same
            // function, same message, in `transpile_target`.
            if let Err(err) = copper_analysis::check_held_preloop_values(&input_fn) {
                return err.to_compile_error().into();
            }
            // The memory-port staging rules and the plain-`Out`-from-a-read-result
            // rule — transpiler-only until 2026-10-23, now enforced here too (front-end
            // parity): one access per bus per cycle, a read result observed only after
            // the edge that produces it and only on a staged port, and no plain `Out`
            // driven from a read result across clock phases.
            if let Err((span, msg)) = cfg.as_ref().map_or(Ok(()), |c| c.check_memory_staging()) {
                return Error::new(span, msg).to_compile_error().into();
            }
            // The pre-tick alignment family (rules 9–15), `multi_write_collapse`
            // (rule 8) and `memory_result_drives_plain_out` (rule 7) were checked
            // here. All of them reason about a plain `Out` on a clocked body, which
            // R8 (above) refuses: retired by D-8 in Phase C step 3 (SPEC II.3.2).
            // c2 (gate G6): the sim macro consumes the SHARED analysis crate — the
            // same one the transpiler consumes — proving a proc-macro can depend on
            // it with no dependency cycle. The inferred register set is logged for
            // the FSM report. Computed on the pristine ItemFn.
            let inferred_registers = cfg.as_ref().map(|c| c.registers()).unwrap_or_default();
            log::debug!(
                "copper-analysis inferred registers for `{}`: {:?}",
                input_fn.sig.ident,
                inferred_registers
            );

            let mut f = input_fn;
            // R11, D-21 A3 (2026-11-07): rustc judges a register's power-on value.
            // Every pre-loop register initializer is evaluated in an inline
            // `const { }` block, so "constant" is Rust's constant — a file `const`,
            // `T::K`, a const generic, any `const fn` call — and a call to a
            // non-`const` fn is rustc's E0015. A wire's initializer is untouched.
            const_wrap_register_initializers(&mut f, &inferred_registers);
            const_wrap_for_bounds(&mut f);
            wrap_in_module(&mut f);
            quote! { #f }.into()
        }
        HardwareMode::Combinational => {
            // c2 (item 2): definite assignment — a combinational `Out` must be
            // driven on all control paths or none; a partial (conditional) write
            // infers a latch. The transpiler already rejects this late
            // (`check_no_latches`); enforcing it here means the *sim* rejects it at
            // compile time too, from the same shared analysis. Checked on the
            // pristine fn, before the body is wrapped in the delta loop.
            if let Err(err) = copper_analysis::check_definite_assignment(&input_fn) {
                return err.to_compile_error().into();
            }
            let mut input_fn = input_fn;
            const_wrap_for_bounds(&mut input_fn);
            let mut f = wrap_combinational(input_fn);
            wrap_in_module(&mut f);
            quote! { #f }.into()
        }
        HardwareMode::Structural => {
            // Pure-hierarchy parent (item 4). Transpile-only for now: the parent
            // instantiates clocked children and has no loop/tick/registers of its
            // own, so there is nothing to rewrite for the sim. It is NOT wrapped as
            // a `HardwareModule` — a structural parent is not spawned (its children
            // are hand-wired in the testbench); the transpiler reads the parent
            // from source and lowers it via `#[hardware(structural)]`. The CDC
            // discipline at its instantiation call sites is enforced there (the
            // transpiler is text-based and re-derives it), and for compiled code
            // the phantom domain types already reject a mismatched wire. Emit the
            // function unchanged so it compiles alongside its children.
            let f = input_fn;
            quote! { #f }.into()
        }
    }
}

