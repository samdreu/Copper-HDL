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

/// SPEC II.1.2 R23 (D-27): before each tick-bearing `for` over a constant, typed
/// inclusive range (`copper_analysis::constant_ticking_inclusive_ranges`), a body item
/// `const _: () = assert!({ let r = (a)..=(b); (*r.end()).checked_add(1).is_some() }, MSG);`
/// — rustc types the range as the `for` does and refuses (E0080) an end at its type's
/// maximum. The transpiler folds the same ranges through `ConstEnv`.
fn place_inclusive_max_assertions(f: &mut ItemFn) {
    use syn::visit_mut::VisitMut;
    let ranges = copper_analysis::constant_ticking_inclusive_ranges(f);
    if ranges.is_empty() {
        return;
    }
    let msg = copper_analysis::INCLUSIVE_MAX_MESSAGE;
    struct Place<'a> {
        ranges: &'a [syn::ExprRange],
        msg: &'a str,
    }
    impl VisitMut for Place<'_> {
        fn visit_block_mut(&mut self, b: &mut syn::Block) {
            syn::visit_mut::visit_block_mut(self, b);
            let mut out = Vec::with_capacity(b.stmts.len());
            for st in std::mem::take(&mut b.stmts) {
                if let syn::Stmt::Expr(syn::Expr::ForLoop(fl), _) = &st {
                    if let syn::Expr::Range(r) = &*fl.expr {
                        let k = copper_analysis::range_key(r);
                        if let Some(s) = self.ranges.iter().find(|s| copper_analysis::range_key(s) == k) {
                            let (a, e) = (s.start.as_deref().unwrap(), s.end.as_deref().unwrap());
                            let msg = self.msg;
                            let span = syn::spanned::Spanned::span(s);
                            out.push(syn::parse_quote_spanned!(span=>
                                const _: () = ::core::assert!(
                                    { let r = (#a)..=(#e); (*r.end()).checked_add(1).is_some() },
                                    #msg
                                );
                            ));
                        }
                    }
                }
                out.push(st);
            }
            b.stmts = out;
        }
    }
    Place { ranges: &ranges, msg }.visit_block_mut(&mut f.block);
}

/// SPEC I.8.4(b), II.2.2 R3 (MEASUREMENTS §V.3): emit each of
/// `copper_analysis::empty_range_dependencies` as a body item
/// `const _: () = assert!(<not all of these ranges empty>, NESTED_EXIT_MESSAGE);`,
/// which rustc evaluates under `cargo check` (an inline `const { }` in a fn body is
/// evaluated only at monomorphization, measured). A set of one range goes right
/// before its `for` statement, so a `const` declared in that block is in scope; a set
/// of several goes at the top of the body.
fn place_empty_range_assertions(f: &mut ItemFn) {
    use syn::visit_mut::VisitMut;
    let msg = copper_analysis::NESTED_EXIT_MESSAGE;
    let cond = |set: &[syn::ExprRange]| -> proc_macro2::TokenStream {
        let parts: Vec<proc_macro2::TokenStream> = set
            .iter()
            .map(|r| {
                let (a, b) = (r.start.as_deref().unwrap(), r.end.as_deref().unwrap());
                match r.limits {
                    syn::RangeLimits::HalfOpen(_) => quote! { ((#a) >= (#b)) },
                    syn::RangeLimits::Closed(_) => quote! { ((#a) > (#b)) },
                }
            })
            .collect();
        quote! { !(#(#parts)&&*) }
    };
    let item = |set: &[syn::ExprRange]| -> syn::Stmt {
        let c = cond(set);
        let span = syn::spanned::Spanned::span(&set[0]);
        syn::parse_quote_spanned!(span=> const _: () = ::core::assert!(#c, #msg);)
    };
    let sets = copper_analysis::empty_range_dependencies(f);
    let mut singles: Vec<syn::ExprRange> = Vec::new();
    let mut top: Vec<syn::Stmt> = Vec::new();
    for set in &sets {
        if set.len() == 1 {
            singles.push(set[0].clone());
        } else {
            top.push(item(set));
        }
    }
    struct Place<'a> {
        singles: &'a [syn::ExprRange],
        item: &'a dyn Fn(&[syn::ExprRange]) -> syn::Stmt,
    }
    impl VisitMut for Place<'_> {
        fn visit_block_mut(&mut self, b: &mut syn::Block) {
            syn::visit_mut::visit_block_mut(self, b);
            let mut out = Vec::with_capacity(b.stmts.len());
            for st in std::mem::take(&mut b.stmts) {
                if let syn::Stmt::Expr(syn::Expr::ForLoop(fl), _) = &st {
                    if let syn::Expr::Range(r) = &*fl.expr {
                        let k = copper_analysis::range_key(r);
                        if let Some(s) = self.singles.iter().find(|s| copper_analysis::range_key(s) == k) {
                            out.push((self.item)(std::slice::from_ref(s)));
                        }
                    }
                }
                out.push(st);
            }
            b.stmts = out;
        }
    }
    Place { singles: &singles, item: &item }.visit_block_mut(&mut f.block);
    for (i, st) in top.into_iter().enumerate() {
        f.block.stmts.insert(i, st);
    }
}

/// The structural arm's two rewrites (see the arm).
fn structural_rewrite(f: &mut ItemFn) {
    use syn::spanned::Spanned;
    for st in f.block.stmts.iter_mut() {
        match st {
            syn::Stmt::Local(l) => {
                if let Some(init) = &mut l.init {
                    if let syn::Expr::Call(call) = &mut *init.expr {
                        if let syn::Expr::Path(p) = &mut *call.func {
                            if let Some(seg) = p.path.segments.last_mut() {
                                if let syn::PathArguments::AngleBracketed(ab) = &mut seg.arguments {
                                    ab.args.push(syn::parse_quote!(_));
                                }
                            }
                        }
                    }
                }
            }
            syn::Stmt::Expr(e @ syn::Expr::Call(_), Some(_)) => {
                let call = e.clone();
                let span = call.span();
                *e = syn::parse_quote_spanned!(span=> ::copper_sim::__instance(#call));
            }
            _ => {}
        }
    }
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
    // SPEC I.13.5 (D-18 A2, D-23 (1), Phase C step 11b): instantiation is a form of
    // the structural body only. The module's `HardwareModule` is consumed by a
    // testbench spawn or by `copper_sim::__instance` (the structural wrapper); a
    // call of it as a statement of a behavioural body would drop the child, which
    // then never runs. rustc decides: the generated fn's return is `#[must_use]`
    // with I.13.5's message, and every behavioural body denies `unused_must_use`,
    // so `child(a, b);` in a body is a compile error naming the clause. (The
    // transpiler refuses the same call from its registry with the same message.)
    let must_use = syn::LitStr::new(
        copper_analysis::front_end::BEHAVIOURAL_INSTANCE_MESSAGE,
        proc_macro2::Span::call_site(),
    );
    f.attrs.push(syn::parse_quote!(#[must_use = #must_use]));
    f.attrs.push(syn::parse_quote!(#[deny(unused_must_use)]));
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
            // A run-time-empty counted `for` ending an enclosing ticking `for`
            // (MEASUREMENTS §J.15): same function, same message, in `transpile_target`.
            // Before reachability (§X.2): R3 counts a range that may be empty at run
            // time as an exit (I.8.4(b)) and refuses that shape too, but its message
            // speaks of a livelock, which a bounded enclosing `for` is not.
            if let Err(err) = copper_analysis::check_counted_for_nesting(&input_fn) {
                return err.to_compile_error().into();
            }
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
            // SPEC I.8.4(b), II.2.2 R3 (2026-11-08, MEASUREMENTS §V.3): a ticking `for`
            // over a range empty only through a constant (`0..EMPTY`). The macro
            // cannot fold a file `const`; the shared analysis names the hypotheses R3's
            // verdict depends on and rustc checks each, as a body item (evaluated
            // under `cargo check`, unlike an inline `const { }`), with R3's message.
            // The transpiler folds the same ranges through `ConstEnv`.
            place_empty_range_assertions(&mut f);
            // SPEC II.1.2 R23 (D-27, 2026-11-09): a ticking `for` over `a..=b` whose
            // constant end is its type's maximum — the counter would overflow. rustc
            // checks it the same way, a body item right before the `for`.
            place_inclusive_max_assertions(&mut f);
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
            // A structural module names a design (SPEC I.13.4(a), D-18 A1): it is
            // not a coroutine and is never spawned — the simulator hand-wires the
            // same instances in a testbench, the transpiler emits the hierarchy. It
            // is NOT wrapped as a `HardwareModule`; it is emitted so it compiles
            // alongside its children, which makes rustc check the wiring (port
            // count E0061, domains E0308, one driver E0382).
            // Phase C step 10 (SPEC I.13.4(b)–(d), D-18): the shape rules ran in
            // `check_front_end_rules`. Two rewrites let rustc judge the rest: each
            // `net::<T, D>()` gets the `_` for its driver kind (inferred from the
            // child port its driver end reaches, so an `Out` and a `RegOut` net are
            // one spelling), and each instantiation is passed to
            // `copper_sim::__instance`, so a call of a plain `fn` is a type error.
            let mut f = input_fn;
            structural_rewrite(&mut f);
            quote! { #f }.into()
        }
    }
}

