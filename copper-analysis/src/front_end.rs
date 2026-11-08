//! The front-end rules both `#[hardware]` front-ends run (SPEC II.1.3, P10; Phase C
//! step 10). Until step 10 every function here except the new ones was the macro's
//! own (`copper-macros/src/lib.rs`), and the transpiler either refused the same
//! shape by another arm or admitted it — the parity breaks of II.1.3, measured on
//! the macro's ui cases (`review/MEASUREMENTS.md` §P). Each is now one function with
//! one message, called by the macro and by `copper_codegen::checked_fir_for_target`
//! through [`check_front_end_rules`], in one order, so a module that fails two rules
//! gets the same first message from both.
//!
//! Every message names its clause (SPEC_OVERVIEW §6, II.1.1's message obligation).

use std::collections::HashSet;

use syn::{visit::Visit, Error, FnArg, ItemFn, Pat, ReturnType, Type};

/// The mode written in `#[hardware(<mode>)]` (I.2.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HardwareMode {
    Sequential,
    Combinational,
    /// The sanctioned domain-crossing point (I.12.3): a `sequential` kernel whose
    /// `In` ports may lie in another domain.
    Synchronizer,
    /// A name for a design (I.13.4): nets and instantiations, no coroutine.
    Structural,
}

impl HardwareMode {
    /// Modes that are clocked, async, tick-bearing loops (everything but comb and structural).
    pub fn is_sequential_like(&self) -> bool {
        matches!(self, HardwareMode::Sequential | HardwareMode::Synchronizer)
    }
}

/// Parse the text of a `#[hardware(...)]` argument list (whitespace ignored): one
/// mode, no flag. The one parser both front-ends use, so a flag the macro refuses
/// the transpiler refuses with the same message (R9: `allow_pretick_alignment`).
pub fn parse_hardware_args(text: &str) -> Result<HardwareMode, String> {
    let text = text.replace(' ', "");
    let mut parts = text.split(',').filter(|p| !p.is_empty());
    let mode = match parts.next().unwrap_or("") {
        "sequential" => HardwareMode::Sequential,
        "combinational" => HardwareMode::Combinational,
        "synchronizer" => HardwareMode::Synchronizer,
        "structural" => HardwareMode::Structural,
        _ => {
            return Err("Unsupported #[hardware(...)] argument. Supported: sequential, combinational, \
                        synchronizer, structural (SPEC II.1 S0, I.2.2)"
                .to_string())
        }
    };
    if let Some(flag) = parts.next() {
        let why = if flag == "allow_pretick_alignment" {
            " — `allow_pretick_alignment` was retired with the pre-tick alignment rules by \
             D-8 (SPEC II.3.2): a clocked module has no plain `Out`, so there is nothing to opt \
             out of"
        } else {
            ""
        };
        return Err(format!(
            "Unsupported #[hardware(...)] flag `{flag}`; the attribute takes a mode only (SPEC II.1 \
             S0, R9){why}"
        ));
    }
    Ok(mode)
}

/// The mode of a `#[hardware(...)]` fn as the transpiler and the corpus audits see
/// it (the attribute still on the fn): `None` when there is no such attribute.
pub fn hardware_mode(f: &ItemFn) -> Option<Result<HardwareMode, Error>> {
    let attr = f
        .attrs
        .iter()
        .find(|a| a.path().segments.last().is_some_and(|s| s.ident == "hardware"))?;
    let text = match &attr.meta {
        syn::Meta::List(l) => l.tokens.to_string(),
        _ => String::new(),
    };
    Some(parse_hardware_args(&text).map_err(|m| Error::new_spanned(attr, m)))
}

pub(crate) fn outer_type_name(ty: &Type) -> Option<String> {
    if let Type::Path(type_path) = ty {
        type_path.path.segments.last().map(|s| s.ident.to_string())
    } else {
        None
    }
}

// ── Clock domains (I.12.4) ──────────────────────────────────────────────────


/// The domain type name at the end of a generic type, but only when there are at
/// least `min_args` type arguments. Returns `None` for a non-`Path` domain (e.g.
/// the `()` unit domain) or too few arguments.
///
/// Used two ways: `Clock<D>` has one type arg (`min_args = 1` → the domain),
/// while `In<T, D>` / `Out<T, D>` need two (`min_args = 2` → the *last* is the
/// domain). A one-argument `In<T>` therefore carries the *default* `()` domain,
/// not an explicit one, and is treated as domain-agnostic.
fn type_domain_at(ty: &Type, min_args: usize) -> Option<String> {
    let Type::Path(tp) = ty else { return None };
    let seg = tp.path.segments.last()?;
    let syn::PathArguments::AngleBracketed(ab) = &seg.arguments else { return None };
    let type_args: Vec<&Type> = ab
        .args
        .iter()
        .filter_map(|a| match a {
            syn::GenericArgument::Type(t) => Some(t),
            _ => None,
        })
        .collect();
    if type_args.len() < min_args {
        return None;
    }
    match type_args.last() {
        Some(Type::Path(p)) => p.path.segments.last().map(|s| s.ident.to_string()),
        _ => None, // e.g. a `()` unit domain
    }
}

/// Domain of a `Clock<D>` (its single type argument).
fn clock_domain(ty: &Type) -> Option<String> {
    type_domain_at(ty, 1)
}

/// *Explicit* domain of an `In<T, D>` / `Out<T, D>` — `None` when the domain is
/// left default (`In<T>` → `()`), which is not considered a crossing.
fn port_domain(ty: &Type) -> Option<String> {
    type_domain_at(ty, 2)
}

fn param_ident(a: &FnArg) -> Option<String> {
    if let FnArg::Typed(pt) = a {
        if let Pat::Ident(pi) = &*pt.pat {
            return Some(pi.ident.to_string());
        }
    }
    None
}

/// Reject foreign-domain ports on a regular sequential module (signature-level).
///
/// Not called for `#[hardware(synchronizer)]` modules — those are the sanctioned
/// crossing points and may declare a foreign-domain input.
pub fn check_cdc(f: &ItemFn) -> Result<(), Error> {
    // Native domain = the module's clock domain.
    let clocks: Vec<String> = f
        .sig
        .inputs
        .iter()
        .filter_map(|a| match a {
            FnArg::Typed(pt) if outer_type_name(&pt.ty).as_deref() == Some("Clock") => {
                clock_domain(&pt.ty)
            }
            _ => None,
        })
        .collect();

    // Multi-clock: a module with several clocks has no single native domain;
    // richer analysis (which crossing belongs to which edge) is out of scope, so
    // skip rather than guess.
    let native = match clocks.as_slice() {
        [d] => d.clone(),
        _ => return Ok(()),
    };

    for a in &f.sig.inputs {
        let FnArg::Typed(pt) = a else { continue };
        let outer = outer_type_name(&pt.ty);
        let Some(kind) = (match outer.as_deref() {
            Some("In") => Some("input"),
            Some("Out") => Some("output"),
            Some("RegOut") => Some("output"),
            // A received `Memory<T, R, W, D, RL, WL>` is clocked on `D` — its ports
            // capture and commit on that domain's edge — so a foreign-domain memory
            // is as real a crossing as a foreign-domain port, and is caught here
            // rather than becoming a silent two-clock design. `port_domain` reads it
            // without a special case: the const arguments are not type arguments, so
            // the last TYPE argument of a `Memory<…>` is `D`, exactly as it is for
            // `In<T, D>`.
            Some("Memory") => Some("memory"),
            _ => None,
        }) else {
            continue;
        };
        if let Some(d) = port_domain(&pt.ty) {
            if d != native {
                let name = param_ident(a).unwrap_or_default();
                // A memory cannot be brought across with a synchronizer, so the
                // remedy differs from a port's — say so rather than give advice that
                // cannot be followed.
                let remedy = if kind == "memory" {
                    format!(
                        "A memory is clocked on its own domain and cannot be synchronized across \
                         one — give this module a `Clock<{d}>` and receive the memory there, or \
                         hand it a memory in `{native}` (SPEC I.12.4(b), I.10.5)."
                    )
                } else {
                    "A regular module may not cross clock domains — bring the signal across with \
                     a synchronizer (`copper::sync_2ff`, or your own `#[hardware(synchronizer)]` \
                     module), then use its output in this domain (SPEC I.12.4(a); II.1 S0)."
                        .to_string()
                };
                return Err(Error::new_spanned(a, format!(
                    "clock-domain crossing: {kind} `{name}` is in domain `{d}`, but this module is \
                     clocked on `{native}`. {remedy}",
                )));
            }
        }
    }
    Ok(())
}


// ── Clocks are provided, not constructed ────────────────────────────────────
//
// A hardware module receives its `Clock<D>` as a parameter; it may not create a
// new one. A fabricated clock is a distinct instance that the testbench never
// advances, so a module ticking it would hang — and, more importantly, a clock
// is a real signal you are handed, not something logic conjures. This closes
// "Gap 2" from the CDC audit. Enforced at the macro (the only place that knows a
// function body is a module body); testbenches, which are not `#[hardware]`,
// still create clocks normally. Cloning a clock parameter stays legal — a clone
// shares the same real clock, which is how you pass it to a submodule.

/// Flags a call whose path constructs a `Clock` — `Clock::new()`,
/// `Clock::<D>::new()`, `…::Clock::default()`.
struct ClockConstruction {
    found: Option<proc_macro2::Span>,
}

impl<'ast> Visit<'ast> for ClockConstruction {
    fn visit_expr_call(&mut self, call: &'ast syn::ExprCall) {
        if let syn::Expr::Path(p) = &*call.func {
            let segs = &p.path.segments;
            if segs.len() >= 2 {
                let last = segs[segs.len() - 1].ident.to_string();
                let owner = segs[segs.len() - 2].ident.to_string();
                if owner == "Clock" && (last == "new" || last == "default") && self.found.is_none() {
                    self.found = Some(syn::spanned::Spanned::span(call));
                }
            }
        }
        syn::visit::visit_expr_call(self, call);
    }
}

/// Reject any clock construction inside a hardware module body.
pub fn check_no_clock_construction(f: &ItemFn) -> Result<(), Error> {
    let mut v = ClockConstruction { found: None };
    v.visit_block(f.block.as_ref());
    match v.found {
        Some(span) => Err(Error::new(
            span,
            "a hardware module may not create a clock — clocks are provided as parameters. \
             A fabricated clock is never driven, so the module would hang. To clock a submodule, \
             pass a clone of a clock parameter (`clk.clone()`) (SPEC II.1 S2, R17; I.2.5(b)).",
        )),
        None => Ok(()),
    }
}

// ── Parameter shadowing ─────────────────────────────────────────────────────
//
// A hardware port is a physical wire named by its parameter. A `let` binding that
// reuses a port name hides the wire for the rest of the body: subsequent reads hit
// the local, not the port, so a stimulus never reaches the logic and an output is
// silently never driven. This is almost always a bug (and never necessary — rename
// the local), so the macro rejects it at the definition site.

/// Collect the identifiers bound by a pattern (handles the nested pattern forms a
/// `let` can use: tuples, tuple-structs, refs, `mut`, type ascription, `|`).
fn collect_pat_idents(pat: &Pat, out: &mut Vec<syn::Ident>) {
    match pat {
        Pat::Ident(pi) => {
            out.push(pi.ident.clone());
            if let Some((_, sub)) = &pi.subpat {
                collect_pat_idents(sub, out);
            }
        }
        Pat::Tuple(t) => t.elems.iter().for_each(|p| collect_pat_idents(p, out)),
        Pat::TupleStruct(t) => t.elems.iter().for_each(|p| collect_pat_idents(p, out)),
        Pat::Slice(s) => s.elems.iter().for_each(|p| collect_pat_idents(p, out)),
        Pat::Reference(r) => collect_pat_idents(&r.pat, out),
        Pat::Type(t) => collect_pat_idents(&t.pat, out),
        Pat::Paren(p) => collect_pat_idents(&p.pat, out),
        Pat::Or(o) => o.cases.iter().for_each(|p| collect_pat_idents(p, out)),
        Pat::Struct(s) => s.fields.iter().for_each(|f| collect_pat_idents(&f.pat, out)),
        _ => {}
    }
}


/// The identifiers of every value parameter (the hardware ports).
fn param_idents(sig: &syn::Signature) -> std::collections::HashSet<String> {
    sig.inputs
        .iter()
        .filter_map(|a| match a {
            FnArg::Typed(pt) => match &*pt.pat {
                Pat::Ident(pi) => Some(pi.ident.to_string()),
                _ => None,
            },
            _ => None,
        })
        .collect()
}

struct ParamShadow<'a> {
    params: &'a std::collections::HashSet<String>,
    found: Option<(String, proc_macro2::Span)>,
}

impl<'ast, 'a> Visit<'ast> for ParamShadow<'a> {
    fn visit_local(&mut self, local: &'ast syn::Local) {
        let mut idents = Vec::new();
        collect_pat_idents(&local.pat, &mut idents);
        for id in idents {
            if self.found.is_none() && self.params.contains(&id.to_string()) {
                self.found = Some((id.to_string(), id.span()));
            }
        }
        syn::visit::visit_local(self, local);
    }
}

/// Reject a `let` binding that shadows a hardware port parameter.
pub fn check_no_param_shadowing(f: &ItemFn) -> Result<(), Error> {
    let params = param_idents(&f.sig);
    if params.is_empty() {
        return Ok(());
    }
    let mut v = ParamShadow { params: &params, found: None };
    v.visit_block(f.block.as_ref());
    match v.found {
        Some((name, span)) => Err(Error::new(
            span,
            format!(
                "`{name}` shadows the hardware port parameter of the same name; the `let` binding \
                 hides the port for the rest of the body (reads and writes hit the local, not the \
                 wire). Rename the local (SPEC II.1 S0, R18; I.2.3's name classes)."
            ),
        )),
        None => Ok(()),
    }
}

// ── Unsupported software idioms: Option / `?` / panic! ──────────────────────
//
// A hardware module describes gates, not software control flow. `Option`, the
// `?` operator, and `panic!` have no honest gate-level meaning, and every way to
// lower them either duplicates logic, hides a valid-bit as implicit state, or
// silently guesses a value on an "impossible" path. Copper's premise is that
// such ambiguity should be *unrepresentable*, so these are rejected at the macro
// (the definition site) with a pointer to the explicit hardware form: a
// `{ valid: Logic, payload: T }` struct you branch on, and an explicit output
// signal (e.g. `illegal: Logic`) instead of a panic.

const OPTION_GUIDANCE: &str = "`Option` is not supported in a hardware module — \
    model maybe-valid data explicitly with a `{ valid: Logic, payload: T }` struct \
    and branch on `.valid`, rather than `Some`/`None`/`unwrap` (SPEC II.2 D3: one spelling per meaning).";

struct UnsupportedIdiom {
    found: Option<(proc_macro2::Span, String)>,
}

impl UnsupportedIdiom {
    fn flag(&mut self, span: proc_macro2::Span, msg: impl Into<String>) {
        if self.found.is_none() {
            self.found = Some((span, msg.into()));
        }
    }
}

impl<'ast> Visit<'ast> for UnsupportedIdiom {
    fn visit_expr_try(&mut self, node: &'ast syn::ExprTry) {
        self.flag(
            syn::spanned::Spanned::span(node),
            "the `?` operator is not supported in a hardware module — it is software \
             control flow with no gate-level meaning. Return a `{ valid: Logic, .. }` \
             struct and branch on `.valid` instead of propagating `None`/`Err` (SPEC II.2 D3).",
        );
        syn::visit::visit_expr_try(self, node);
    }

    // `visit_macro` fires for a macro invocation in any position — expression
    // (`x = panic!()`) or statement (`panic!();`, which is a `Stmt::Macro`, not
    // an `Expr::Macro`).
    fn visit_macro(&mut self, node: &'ast syn::Macro) {
        if let Some(id) = node.path.get_ident() {
            let n = id.to_string();
            if matches!(n.as_str(), "panic" | "unreachable" | "todo" | "unimplemented") {
                self.flag(
                    syn::spanned::Spanned::span(node),
                    format!(
                        "`{n}!` is not synthesizable in a hardware module — an \
                         unreachable/error condition must be an explicit output signal \
                         (e.g. an `illegal: Logic` port), not a panic (SPEC II.2 D3; I.2.5(a))."
                    ),
                );
            }
        }
        syn::visit::visit_macro(self, node);
    }

    fn visit_expr_call(&mut self, node: &'ast syn::ExprCall) {
        if let syn::Expr::Path(p) = &*node.func {
            if p.path.segments.last().is_some_and(|s| s.ident == "Some") {
                self.flag(syn::spanned::Spanned::span(node), OPTION_GUIDANCE);
            }
        }
        syn::visit::visit_expr_call(self, node);
    }

    fn visit_expr_path(&mut self, node: &'ast syn::ExprPath) {
        if node.path.is_ident("None") {
            self.flag(syn::spanned::Spanned::span(node), OPTION_GUIDANCE);
        }
        syn::visit::visit_expr_path(self, node);
    }

    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        if matches!(node.method.to_string().as_str(), "unwrap" | "unwrap_or" | "expect") {
            self.flag(syn::spanned::Spanned::span(node), OPTION_GUIDANCE);
        }
        syn::visit::visit_expr_method_call(self, node);
    }

    fn visit_pat(&mut self, node: &'ast syn::Pat) {
        let is_option_pat = match node {
            syn::Pat::TupleStruct(ts) => ts.path.segments.last().is_some_and(|s| s.ident == "Some"),
            syn::Pat::Ident(pi) => pi.ident == "None" && pi.subpat.is_none(),
            syn::Pat::Path(pp) => pp.path.segments.last().is_some_and(|s| s.ident == "None"),
            _ => false,
        };
        if is_option_pat {
            self.flag(syn::spanned::Spanned::span(node), OPTION_GUIDANCE);
        }
        syn::visit::visit_pat(self, node);
    }

    fn visit_type_path(&mut self, node: &'ast syn::TypePath) {
        if node.path.segments.last().is_some_and(|s| s.ident == "Option") {
            self.flag(syn::spanned::Spanned::span(node), OPTION_GUIDANCE);
        }
        syn::visit::visit_type_path(self, node);
    }
}

/// Reject `Option` / `?` / `panic!` (and friends) inside a hardware module body.
pub fn check_no_unsupported_idioms(f: &ItemFn) -> Result<(), Error> {
    let mut v = UnsupportedIdiom { found: None };
    v.visit_block(f.block.as_ref());
    match v.found {
        Some((span, msg)) => Err(Error::new(span, msg)),
        None => Ok(()),
    }
}

/// Visits an async fn body and records which clock parameter names have a `.tick().await` call.
struct TickAwaitVisitor<'a> {
    // NOTE: `visit_expr_await` currently records *every* `.tick().await` receiver
    // into `found` without checking it against `clock_names`. Either the visit
    // should filter to actual clock params (`clock_names.contains(..)`) — a latent
    // correctness gap — or this field is vestigial. Noted in the 2026-08-26
    // audit (now design_docs/OUTDATED/AUDIT_2026-08-26.md §4); left in place
    // pending that decision.
    #[allow(dead_code)]
    clock_names: &'a HashSet<String>,
    found: HashSet<String>,
}

impl<'ast, 'a> Visit<'ast> for TickAwaitVisitor<'a> {
    fn visit_expr_await(&mut self, node: &'ast syn::ExprAwait) {
        if let syn::Expr::MethodCall(method) = &*node.base {
            if method.method == "tick" && method.args.is_empty() {
                if let syn::Expr::Path(path) = &*method.receiver {
                    if let Some(ident) = path.path.get_ident() {
                        self.found.insert(ident.to_string());
                    }
                }
            }
        }
        syn::visit::visit_expr_await(self, node);
    }
}

/// True if the function body has a top-level `loop { … }`. A sequential module
/// runs forever, so its body must be an infinite loop (optionally preceded by
/// `let` declarations for registers/constants). A labelled loop counts too.
fn has_top_level_loop(block: &syn::Block) -> bool {
    block
        .stmts
        .iter()
        .any(|s| matches!(s, syn::Stmt::Expr(syn::Expr::Loop(_), _)))
}

/// Enforce the canonical sequential shape: **exactly one** top-level `loop`, and
/// it is the **final** statement. Pre-loop `let`/const declarations (registers and
/// constants) are fine; a *second* top-level loop or any statement *after* the
/// infinite loop is unreachable — either two independent state machines crammed
/// into one module or dead code. The "no loop at all" case is rejected separately
/// by the `has_top_level_loop` gate, so this runs only when at least one exists.
fn check_single_top_level_loop(block: &syn::Block) -> Result<(), Error> {
    let loops: Vec<usize> = block
        .stmts
        .iter()
        .enumerate()
        .filter(|(_, s)| matches!(s, syn::Stmt::Expr(syn::Expr::Loop(_), _)))
        .map(|(i, _)| i)
        .collect();

    if loops.len() > 1 {
        return Err(Error::new(
            syn::spanned::Spanned::span(&block.stmts[loops[1]]),
            "#[hardware(sequential)] body must have exactly one top-level `loop { … }` — a \
             hardware module is a single state machine. Merge the loops, or split this into \
             separate modules (SPEC II.1 S0, R20; I.2.2).",
        ));
    }

    if let Some(&pos) = loops.first() {
        if pos != block.stmts.len() - 1 {
            return Err(Error::new(
                syn::spanned::Spanned::span(&block.stmts[pos + 1]),
                "#[hardware(sequential)] `loop { … }` must be the final statement; code after \
                 an infinite loop is unreachable. Move it inside the loop, or before it (SPEC II.1 S0, R20; I.2.2).",
            ));
        }
    }

    Ok(())
}

/// Returns the names of every `In<T, D>`-typed parameter, in declaration order.
fn in_param_names(sig: &syn::Signature) -> Vec<String> {
    sig.inputs
        .iter()
        .filter_map(|arg| {
            if let FnArg::Typed(pat_ty) = arg {
                if outer_type_name(&pat_ty.ty).as_deref() == Some("In") {
                    if let Pat::Ident(pi) = &*pat_ty.pat {
                        return Some(pi.ident.to_string());
                    }
                }
            }
            None
        })
        .collect()
}

/// Checks every `In<T, D>` parameter is used *only* as the direct receiver of
/// a zero-arg `.read()` call — e.g. `step.read()` — anywhere else it appears
/// (through a `.clone()`, a reassignment to a new binding, passed to a helper
/// function, stored in a collection, etc.) is a pattern the per-read freshness
/// check (`SyncedReadRewriter`, below) cannot place correctly, since it can
/// only rewrite call sites it can see directly in this function's own body.
/// Rather than silently leaving such a read unprotected, this is a hard
/// compile error: `#[hardware(sequential)]`'s guarantee (a read only ever
/// blocks when it would otherwise race a testbench-driven update from a
/// premature loop iteration) requires knowing about every read.
///
/// Returns every offending identifier so the caller can report all of them at
/// once, not just the first.
fn find_unprotectable_in_uses(block: &syn::Block, in_params: &HashSet<String>) -> Vec<syn::Ident> {
    struct Finder<'a> {
        in_params: &'a HashSet<String>,
        found: Vec<syn::Ident>,
    }

    impl<'a, 'ast> Visit<'ast> for Finder<'a> {
        fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
            let is_recognized_read = node.method == "read"
                && node.args.is_empty()
                && matches!(&*node.receiver, syn::Expr::Path(p)
                    if p.path.get_ident().is_some_and(|id| self.in_params.contains(&id.to_string())));

            if !is_recognized_read {
                // Not the pattern we rewrite — its receiver is fair game for
                // being an unprotectable use (e.g. `a.clone().read()`: this
                // outer call isn't recognized, so we walk into `a.clone()`,
                // whose *own* receiver `a` is a bare path and gets caught by
                // visit_expr_path below).
                self.visit_expr(&node.receiver);
            }
            for arg in &node.args {
                self.visit_expr(arg);
            }
        }

        fn visit_expr_path(&mut self, node: &'ast syn::ExprPath) {
            if let Some(ident) = node.path.get_ident() {
                if self.in_params.contains(&ident.to_string()) {
                    self.found.push(ident.clone());
                    return;
                }
            }
            syn::visit::visit_expr_path(self, node);
        }
    }

    let mut finder = Finder { in_params, found: Vec::new() };
    finder.visit_block(block);
    finder.found
}

/// SPEC II.1 R19's remaining reason (D-12, II.3.4): an `In<T, D>` parameter is used
/// only as the receiver of `.read()` — `let t = x.read()` is the one read form; a
/// clone, a rebinding or a helper taking `&In` is refused. Until Phase C step 3
/// this was the precondition for the per-read barrier injection (`ReadRewriter`,
/// which rewrote a deferred read to `{ pre_edge_barrier::<D>().await; x.read() }`);
/// the injection and the barrier were deleted with the two-job executor (D-8), so
/// the body now runs as written.
/// R19's one message.
pub const IN_USE_MESSAGE: &str = "this `In<T, D>` parameter is used other than as the receiver of \
    `.read()`: the one read form is `let t = x.read()`, called on the parameter itself — a \
    `.clone()`, a rebinding or a helper taking the port is no read of the kernel (SPEC II.1 S0, \
    R19; I.2.3). Read it directly at each place it is needed.";

pub fn check_in_param_uses(f: &ItemFn) -> Result<(), Error> {
    let in_params: HashSet<String> = in_param_names(&f.sig).into_iter().collect();
    if in_params.is_empty() {
        return Ok(());
    }

    let bad = find_unprotectable_in_uses(&f.block, &in_params);
    if !bad.is_empty() {
        let mut iter = bad.into_iter();
        let mut err = Error::new_spanned(
            &iter.next().unwrap(),
            IN_USE_MESSAGE,
        );
        for ident in iter {
            err.combine(Error::new_spanned(
                &ident,
                "...and here (same parameter, same restriction)",
            ));
        }
        return Err(err);
    }
    Ok(())
}


/// Validates a hardware function based on its signature and hardware mode.
pub fn check_header(input_fn: &ItemFn, hardware_mode: HardwareMode) -> Result<(), Error> {
    // Sequential must be async; combinational must not be async (the macro adds async)
    if input_fn.sig.asyncness.is_none() && hardware_mode.is_sequential_like() {
        return Err(Error::new_spanned(
            &input_fn.sig,
            "#[hardware(sequential)] can only be applied to async functions (SPEC II.1 S0, R20; I.2.2)",
        ));
    }
    if input_fn.sig.asyncness.is_some() && matches!(hardware_mode, HardwareMode::Combinational) {
        return Err(Error::new_spanned(
            &input_fn.sig.asyncness,
            "#[hardware(combinational)] must not be async — the macro adds the async wrapper (SPEC II.1 S0, R20; I.2.2)",
        ));
    }

    // Must have no return value
    if let ReturnType::Type(_, ty) = &input_fn.sig.output {
        if !matches!(&**ty, Type::Tuple(t) if t.elems.is_empty()) {
            return Err(Error::new_spanned(
                &input_fn.sig.output,
                "hardware functions must not have a return value — outputs go through Out<T> parameters (SPEC II.1 S0, R20; I.2.2)",
            ));
        }
    }

    // Walk parameters: enforce types and track presence of Clock / Out
    let mut has_clock = false;
    let mut has_out = false;

    for arg in &input_fn.sig.inputs {
        match arg {
            FnArg::Receiver(_) => {
                return Err(Error::new_spanned(
                    arg,
                    "hardware functions cannot have a self parameter (SPEC II.1 S0, R20; I.2.2)",
                ));
            }
            FnArg::Typed(pat_ty) => {
                // Parameters must be named (no destructuring)
                if !matches!(&*pat_ty.pat, Pat::Ident(_)) {
                    return Err(Error::new_spanned(
                        &pat_ty.pat,
                        "hardware function parameters must be named (SPEC II.1 S0, R20; I.2.2)",
                    ));
                }

                // All parameters must be Clock<D>, In<T>, Out<T>, or RegOut<T>.
                // `RegOut<T,D>` is a registered output port (an enabled flip-flop):
                // same body surface as `Out` (`.write`), but its value commits at the
                // clock edge, so a held/conditional output is a proper register rather
                // than a latch. It counts as an output. See copper-core RegOut and
                // design_docs/REGISTERED_OUTPUTS.md.
                // `Memory<T, R, W, D, RL, WL>` is a *received resource*, not a port:
                // the same disposition as `Clock<D>` one section above ("a hardware
                // module receives its clock as a parameter; it may not create a new
                // one"). A memory is a real array you are handed — by a testbench, or
                // by a parent that owns it — so receiving one is how a module shares
                // storage instead of privately conjuring its own. It is neither an
                // input nor an output, so it satisfies neither `has_clock` nor
                // `has_out`: a module whose only non-clock parameter is a memory still
                // needs an `Out` to say anything.
                //
                // Constructing a memory in the body stays legal (that is what every
                // existing single-owner design does); this only adds the second
                // spelling. `copper_analysis::Cfg` collects BOTH spellings, so
                // `check_memory_staging` covers a received memory exactly as it covers
                // a declared one — see `memory_locals`.
                match outer_type_name(&pat_ty.ty).as_deref() {
                    Some("Clock")  => has_clock = true,
                    Some("In")     => {}
                    Some("Out")    => has_out = true,
                    Some("RegOut") => has_out = true,
                    Some("Memory") => {}
                    _ => {
                        return Err(Error::new_spanned(
                            &pat_ty.ty,
                            "hardware function parameters must be Clock<D>, In<T>, Out<T>, \
                             RegOut<T>, or Memory<T, R, W, D, RL, WL> (SPEC II.1 S0, R20; I.2.2)",
                        ));
                    }
                }
            }
        }
    }

    // Sequential must have at least one Clock parameter
    if hardware_mode.is_sequential_like() && !has_clock {
        return Err(Error::new_spanned(
            &input_fn.sig,
            "#[hardware(sequential)] must have at least one Clock<D> parameter (SPEC II.1 S0, R20; I.2.2)",
        ));
    }

    // A structural parent must have at least one Clock<D> to thread to its
    // children — that is the whole point (the multi-clock hierarchy enabler).
    if matches!(hardware_mode, HardwareMode::Structural) && !has_clock {
        return Err(Error::new_spanned(
            &input_fn.sig,
            "#[hardware(structural)] must have at least one Clock<D> parameter to thread to its children (SPEC II.1 S0, R20; I.2.2)",
        ));
    }

    // Combinational cannot have a Clock parameter
    if matches!(hardware_mode, HardwareMode::Combinational) && has_clock {
        return Err(Error::new_spanned(
            &input_fn.sig,
            "#[hardware(combinational)] cannot have a Clock parameter (SPEC II.1 S0, R20; I.2.2)",
        ));
    }

    // Both modes must have at least one Out<T>. In<T> is not required — a
    // free-running module (e.g. a counter with no reset/enable) legitimately
    // has no inputs at all.
    if !has_out {
        return Err(Error::new_spanned(
            &input_fn.sig,
            "hardware functions must have at least one Out<T> parameter (SPEC II.1 S0, R20; I.2.2)",
        ));
    }

    // Sequential-like: every Clock<D> parameter must have at least one `name.tick().await` in the body.
    if hardware_mode.is_sequential_like() {
        let clock_names: HashSet<String> = input_fn.sig.inputs.iter()
            .filter_map(|arg| {
                if let FnArg::Typed(pat_ty) = arg {
                    if outer_type_name(&pat_ty.ty).as_deref() == Some("Clock") {
                        if let Pat::Ident(pi) = &*pat_ty.pat {
                            return Some(pi.ident.to_string());
                        }
                    }
                }
                None
            })
            .collect();

        let mut visitor = TickAwaitVisitor { clock_names: &clock_names, found: HashSet::new() };
        visitor.visit_block(&input_fn.block);

        for arg in &input_fn.sig.inputs {
            if let FnArg::Typed(pat_ty) = arg {
                if outer_type_name(&pat_ty.ty).as_deref() == Some("Clock") {
                    if let Pat::Ident(pi) = &*pat_ty.pat {
                        let name = pi.ident.to_string();
                        if !visitor.found.contains(&name) {
                            return Err(Error::new_spanned(
                                arg,
                                format!("#[hardware(sequential)] body must contain `{name}.tick().await` (SPEC II.1 S0, R20; I.2.2)"),
                            ));
                        }
                    }
                }
            }
        }
    }

    // Sequential-like: the body must be a top-level infinite `loop` (the user
    // writes it — for combinational, the macro adds the loop). Without it, CHIR
    // rejects the module later; catch it here with a clearer, earlier message.
    if hardware_mode.is_sequential_like() && !has_top_level_loop(&input_fn.block) {
        return Err(Error::new_spanned(
            &input_fn.sig,
            "#[hardware(sequential)] body must be a top-level `loop { … clk.tick().await; }` \
             — a hardware module runs forever, so its body is an infinite loop (SPEC II.1 S0, R20; I.2.2)",
        ));
    }

    // Sequential-like: that single top-level loop must be the *only* one, and the
    // last statement (a second loop or trailing code is unreachable). Runs after
    // the gate above guaranteed at least one top-level loop.
    if hardware_mode.is_sequential_like() {
        check_single_top_level_loop(&input_fn.block)?;
    }

    Ok(())
}


// ── R7, R12, R13/R14: the kernel's control and expression forms (S0, S4) ─────

/// SPEC II.1 **R7** (S4, I.2.5(d), I.8.1): the coroutine never returns — a `break`
/// of the module loop, and a `return` anywhere in the body, end it, and a
/// coroutine has no meaning after I.1.1's infinite edge sequence. On a
/// `combinational` body (the macro wraps it in a loop of its own) the same two
/// forms are outside I.11.1's function (S0).
pub fn check_module_loop_exit(f: &ItemFn) -> Result<(), Error> {
    struct V {
        depth: usize,
        found: Option<(proc_macro2::Span, &'static str)>,
    }
    impl<'ast> Visit<'ast> for V {
        fn visit_expr_return(&mut self, n: &'ast syn::ExprReturn) {
            if self.found.is_none() {
                self.found = Some((syn::spanned::Spanned::span(n), "return"));
            }
        }
        fn visit_expr_break(&mut self, n: &'ast syn::ExprBreak) {
            if self.depth == 0 && n.label.is_none() && self.found.is_none() {
                self.found = Some((syn::spanned::Spanned::span(n), "break"));
            }
            syn::visit::visit_expr_break(self, n);
        }
        fn visit_expr_loop(&mut self, n: &'ast syn::ExprLoop) {
            self.depth += 1;
            syn::visit::visit_expr_loop(self, n);
            self.depth -= 1;
        }
        fn visit_expr_while(&mut self, n: &'ast syn::ExprWhile) {
            self.depth += 1;
            syn::visit::visit_expr_while(self, n);
            self.depth -= 1;
        }
        fn visit_expr_for_loop(&mut self, n: &'ast syn::ExprForLoop) {
            self.depth += 1;
            syn::visit::visit_expr_for_loop(self, n);
            self.depth -= 1;
        }
        fn visit_expr_closure(&mut self, _: &'ast syn::ExprClosure) {}
        fn visit_item(&mut self, _: &'ast syn::Item) {}
    }
    // The module loop itself is depth 0: its body is visited without entering
    // `visit_expr_loop` for it.
    let mut v = V { depth: 0, found: None };
    for s in &f.block.stmts {
        match s {
            syn::Stmt::Expr(syn::Expr::Loop(l), _) => {
                for st in &l.body.stmts {
                    v.visit_stmt(st);
                }
            }
            other => v.visit_stmt(other),
        }
    }
    match v.found {
        Some((span, "return")) => Err(Error::new(span, RETURN_MESSAGE)),
        Some((span, _)) => Err(Error::new(span, MODULE_LOOP_BREAK_MESSAGE)),
        None => Ok(()),
    }
}

/// R7's message for `return` (both front-ends).
pub const RETURN_MESSAGE: &str = "`return` ends the module's coroutine, which never ends — it is \
    resumed at every edge of its clock forever (SPEC II.1 S4, R7; I.2.5(d)). Write the rest of the \
    cycle under an `if` instead, and let the loop come round.";
/// R7's message for a `break` of the module loop (both front-ends).
pub const MODULE_LOOP_BREAK_MESSAGE: &str = "this `break` leaves the module's own `loop`, which ends \
    the coroutine — a hardware module runs forever (SPEC II.1 S4, R7; I.8.1). A `break` may leave \
    only a nested loop; to stop doing something, hold a state register and test it.";

/// SPEC II.1 **R12** (S0, I.2.3; D-10 A5, I.8.3): `break` and `continue` are
/// unlabelled and leave or restart the innermost loop; a `break` carries no value.
pub fn check_break_forms(f: &ItemFn) -> Result<(), Error> {
    struct V {
        found: Option<(proc_macro2::Span, String)>,
    }
    impl<'ast> Visit<'ast> for V {
        fn visit_expr_break(&mut self, n: &'ast syn::ExprBreak) {
            if self.found.is_none() {
                if let Some(l) = &n.label {
                    self.found = Some((syn::spanned::Spanned::span(n), format!("a labelled `break {l}`")));
                } else if n.expr.is_some() {
                    self.found = Some((syn::spanned::Spanned::span(n), "a `break` with a value".into()));
                }
            }
            syn::visit::visit_expr_break(self, n);
        }
        fn visit_expr_continue(&mut self, n: &'ast syn::ExprContinue) {
            if self.found.is_none() {
                if let Some(l) = &n.label {
                    self.found = Some((syn::spanned::Spanned::span(n), format!("a labelled `continue {l}`")));
                }
            }
        }
        fn visit_expr_closure(&mut self, _: &'ast syn::ExprClosure) {}
        fn visit_item(&mut self, _: &'ast syn::Item) {}
    }
    let mut v = V { found: None };
    v.visit_block(&f.block);
    match v.found {
        Some((span, what)) => Err(Error::new(span, break_form_message(&what))),
        None => Ok(()),
    }
}

/// R12's one message.
pub fn break_form_message(what: &str) -> String {
    format!(
        "{what} is outside the kernel's grammar: `break` and `continue` are unlabelled and act on \
         the innermost loop, and a `break` carries no value (SPEC II.1 S0, R12; I.2.3, I.8.3). \
         Leave an outer loop through a state register the inner one sets, and carry a value out \
         in a local assigned before the `break`."
    )
}

/// SPEC II.1 **R13/R14** (S0, I.2.3: `e` pure, an input read only as the hoisted
/// `let t = x.read()`): no macro in statement or expression position. Admitted:
/// anything inside `const { }` (rustc evaluates it), `matches!` (DESUGARED, I.2.4)
/// and `vec![…]` as the argument of `from_contents` (a memory's listed preload,
/// DESUGARED, I.2.4, I.10.3). The panic family has D3's own message, which
/// [`check_front_end_rules`] runs first. R14 — an input read inside the tokens —
/// is refused with the macro that holds it: the tokens are not an expression of
/// the kernel, and the read in them is no `let t = x.read()`.
pub fn check_no_body_macros(f: &ItemFn) -> Result<(), Error> {
    struct V {
        found: Option<(proc_macro2::Span, String)>,
    }
    impl V {
        fn check(&mut self, m: &syn::Macro) {
            let name = m.path.segments.last().map(|s| s.ident.to_string()).unwrap_or_default();
            if name != "matches" && self.found.is_none() {
                self.found = Some((syn::spanned::Spanned::span(m), name));
            }
        }
    }
    impl<'ast> Visit<'ast> for V {
        fn visit_macro(&mut self, m: &'ast syn::Macro) {
            self.check(m);
        }
        fn visit_expr_const(&mut self, _: &'ast syn::ExprConst) {}
        fn visit_expr_closure(&mut self, _: &'ast syn::ExprClosure) {}
        fn visit_item(&mut self, _: &'ast syn::Item) {}
        fn visit_expr_call(&mut self, n: &'ast syn::ExprCall) {
            let preload = matches!(&*n.func, syn::Expr::Path(p)
                if p.path.segments.last().is_some_and(|s| s.ident == "from_contents"));
            self.visit_expr(&n.func);
            for a in &n.args {
                match a {
                    syn::Expr::Macro(m) if preload && m.mac.path.is_ident("vec") => {}
                    other => self.visit_expr(other),
                }
            }
        }
    }
    let mut v = V { found: None };
    v.visit_block(&f.block);
    match v.found {
        Some((span, name)) => Err(Error::new(span, body_macro_message(&name))),
        None => Ok(()),
    }
}

/// R13/R14's one message.
pub fn body_macro_message(name: &str) -> String {
    format!(
        "`{name}!` in a hardware module body is outside the kernel's grammar: an expression is \
         pure, over the operators of I.14, and an input is read only as `let t = x.read()` \
         (SPEC II.1 S0, R13/R14; I.2.3). A macro's tokens are not an expression of the kernel. \
         A compile-time check belongs in `const {{ … }}`; a run-time one is an output port."
    )
}

/// SPEC II.1 **R10** (S0, I.2.2: exactly one; S2: a second resumption protocol;
/// D-17, I.12.5): a `sequential` or `synchronizer` module has exactly one `Clock`
/// parameter. A design that needs two clocks is a `structural` parent over
/// single-domain modules and synchronizers (I.13.4).
pub fn check_one_clock(f: &ItemFn, mode: HardwareMode) -> Result<(), Error> {
    if !mode.is_sequential_like() {
        return Ok(());
    }
    let clocks: Vec<&syn::PatType> = f
        .sig
        .inputs
        .iter()
        .filter_map(|a| match a {
            FnArg::Typed(pt) if outer_type_name(&pt.ty).as_deref() == Some("Clock") => Some(pt),
            _ => None,
        })
        .collect();
    if let Some(second) = clocks.get(1) {
        return Err(Error::new_spanned(second, TWO_CLOCKS_MESSAGE));
    }
    Ok(())
}

/// R10's one message.
pub const TWO_CLOCKS_MESSAGE: &str = "a second `Clock` parameter on a clocked module: a \
    `sequential` or `synchronizer` module is resumed by the edges of exactly one clock, and a \
    `tick` on a second clock would be a second resumption protocol the coroutine does not have \
    (SPEC II.1 S0/S2, R10; I.2.2, I.12.5). Write the design as a `#[hardware(structural)]` parent \
    that gives each single-clock child its own clock, with a `#[hardware(synchronizer)]` child \
    at every crossing.";


// ── R5: a tick-free `for` has a constant bound (S3) ──────────────────────────

/// The bounds of every tick-free counted `for` (one whose body has no `tick`) in
/// `f`, in source order: the expressions R5 requires to be constants of I.14.
pub fn tick_free_for_bounds(f: &ItemFn) -> Vec<&syn::Expr> {
    struct V<'a>(Vec<&'a syn::Expr>);
    impl<'a> Visit<'a> for V<'a> {
        fn visit_expr_for_loop(&mut self, fl: &'a syn::ExprForLoop) {
            if !fl.body.stmts.iter().any(crate::cfg::stmt_contains_tick) {
                if let syn::Expr::Range(r) = &*fl.expr {
                    self.0.extend(r.start.as_deref());
                    self.0.extend(r.end.as_deref());
                }
            }
            syn::visit::visit_expr_for_loop(self, fl);
        }
    }
    let mut v = V(Vec::new());
    v.visit_block(&f.block);
    v.0
}

/// SPEC II.1 **R5** (S3, I.2.5(c), I.8.5): a tick-free counted `for` is unrolled
/// (I.2.4), so its bound is a constant of I.14 (I.14.4: Rust's constants). The
/// syntactic half both front-ends share: each bound is constant-shaped
/// (`is_constant_expr`, a const generic of the module, or arithmetic over them)
/// and reads no run-time value. The macro then has rustc judge it (the bound in a
/// `const { }` block, as R11's initializers, D-21 A3); the transpiler folds it
/// through `ConstEnv` when it unrolls. A ticking `for` is a state machine and its
/// bound may be read at run time (I.8.4, snapshotted at entry).
pub fn check_tick_free_for_bounds(f: &ItemFn) -> Result<(), Error> {
    let generics: HashSet<String> = f
        .sig
        .generics
        .params
        .iter()
        .filter_map(|p| match p {
            syn::GenericParam::Const(c) => Some(c.ident.to_string()),
            _ => None,
        })
        .collect();
    fn constant(e: &syn::Expr, g: &HashSet<String>) -> bool {
        match e {
            syn::Expr::Path(p) if p.qself.is_none() && p.path.get_ident().is_some_and(|i| g.contains(&i.to_string())) => true,
            syn::Expr::Paren(p) => constant(&p.expr, g),
            syn::Expr::Group(x) => constant(&x.expr, g),
            syn::Expr::Cast(c) => constant(&c.expr, g),
            syn::Expr::Unary(u) => constant(&u.expr, g),
            syn::Expr::Binary(b) => constant(&b.left, g) && constant(&b.right, g),
            syn::Expr::Const(_) => true,
            other => crate::is_constant_expr(other),
        }
    }
    for b in tick_free_for_bounds(f) {
        if !constant(b, &generics) {
            return Err(Error::new_spanned(b, RUNTIME_FOR_BOUND_MESSAGE));
        }
    }
    Ok(())
}

/// R5's one message.
pub const RUNTIME_FOR_BOUND_MESSAGE: &str = "this bound of a tick-free `for` is not a constant: \
    a `for` with no `clk.tick().await` in its body runs inside one cycle, so it is unrolled, and \
    its bound must be a Rust constant — a literal, a `const`, a const generic (SPEC II.1 S3, R5; \
    I.2.5(c), I.8.5, I.14.4). Loop to a constant bound and test the run-time limit inside, or put \
    a tick in the body to make the loop a state machine.";

/// The syntactic front-end rules of II.1 and II.2, in the one order both
/// front-ends run them (II.1.3): the header first (R20, R9, R10), then the body
/// rules. The CFG rules (R2/R3 reachability, R11, the memory rules) and the
/// mode-specific rules each front-end runs after this, from the same functions.
pub fn check_front_end_rules(f: &ItemFn, mode: HardwareMode) -> Result<(), Error> {
    check_header(f, mode)?;
    check_one_clock(f, mode)?;
    check_no_clock_construction(f)?;
    crate::check_outside_methods(f)?;
    crate::check_memory_port_index(f)?;
    check_no_unsupported_idioms(f)?;
    crate::check_admissible(f)?;
    check_no_param_shadowing(f)?;
    if mode == HardwareMode::Structural {
        return Ok(());
    }
    check_module_loop_exit(f)?;
    check_break_forms(f)?;
    check_tick_free_for_bounds(f)?;
    check_no_body_macros(f)?;
    if mode.is_sequential_like() {
        if mode == HardwareMode::Sequential {
            check_cdc(f)?;
        }
        check_in_param_uses(f)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn validate_hardware_fn(f: &ItemFn, m: &HardwareMode) -> Result<(), Error> {
        check_header(f, *m)
    }
    use syn::parse_quote;

    // ── Option / `?` / panic! are rejected ──────────────────────────────────

    #[test]
    fn question_mark_operator_rejected() {
        let f: syn::ItemFn = parse_quote! {
            async fn m(clk: Clock<C>, i: In<Bits<8>, C>, o: Out<Bits<8>, C>) {
                loop { let x = decode(i.read())?; o.write(x); clk.tick().await; }
            }
        };
        assert!(check_no_unsupported_idioms(&f).is_err());
    }

    #[test]
    fn panic_family_macros_rejected() {
        for body in [
            "panic!(\"bad\")",
            "unreachable!()",
            "todo!()",
            "unimplemented!()",
        ] {
            let stmt: syn::Stmt = syn::parse_str(&format!("{body};")).unwrap();
            let f: syn::ItemFn = parse_quote! {
                async fn m(clk: Clock<C>, o: Out<Bits<8>, C>) {
                    loop { #stmt o.write(Bits::zero()); clk.tick().await; }
                }
            };
            assert!(check_no_unsupported_idioms(&f).is_err(), "{body} should be rejected");
        }
    }

    #[test]
    fn option_some_none_and_unwrap_rejected() {
        // Some(..) expression
        let f: syn::ItemFn = parse_quote! {
            async fn m(clk: Clock<C>, o: Out<Bits<8>, C>) {
                loop { let _x = Some(Bits::zero()); o.write(Bits::zero()); clk.tick().await; }
            }
        };
        assert!(check_no_unsupported_idioms(&f).is_err());

        // `match … { Some(d) => .., None => .. }` patterns + unwrap_or method
        let f: syn::ItemFn = parse_quote! {
            async fn m(clk: Clock<C>, i: In<Bits<8>, C>, o: Out<Bits<8>, C>) {
                loop {
                    let c = classify(i.read()).unwrap_or(Bits::zero());
                    o.write(c);
                    clk.tick().await;
                }
            }
        };
        assert!(check_no_unsupported_idioms(&f).is_err());

        // `Option<T>` in a type annotation
        let f: syn::ItemFn = parse_quote! {
            async fn m(clk: Clock<C>, o: Out<Bits<8>, C>) {
                loop { let _x: Option<Bits<8>> = None; o.write(Bits::zero()); clk.tick().await; }
            }
        };
        assert!(check_no_unsupported_idioms(&f).is_err());
    }

    #[test]
    fn ordinary_hardware_body_is_accepted() {
        // No Option / ? / panic — a plain FSM body passes.
        let f: syn::ItemFn = parse_quote! {
            async fn m(clk: Clock<C>, i: In<Bits<8>, C>, o: Out<Bits<8>, C>) {
                let mut acc: Bits<8> = Bits::zero();
                loop {
                    acc = acc + i.read();
                    o.write(acc);
                    clk.tick().await;
                }
            }
        };
        assert!(check_no_unsupported_idioms(&f).is_ok());
    }

    // ── Clocks are provided, not constructed (Gap 2) ────────────────────────

    #[test]
    fn clock_construction_rejected_new() {
        let f: syn::ItemFn = parse_quote! {
            async fn m(clk: Clock<Slow>, q: Out<Logic, Slow>) {
                let sneaky = Clock::<Fast>::new();
                loop { q.write(Logic::Zero); sneaky.tick().await; clk.tick().await; }
            }
        };
        assert!(check_no_clock_construction(&f).is_err());
    }

    #[test]
    fn clock_construction_rejected_default() {
        let f: syn::ItemFn = parse_quote! {
            async fn m(clk: Clock<Slow>, q: Out<Logic, Slow>) {
                let sneaky: Clock<Fast> = Clock::default();
                loop { q.write(Logic::Zero); clk.tick().await; let _ = &sneaky; }
            }
        };
        assert!(check_no_clock_construction(&f).is_err());
    }

    #[test]
    fn clock_clone_is_allowed() {
        // Cloning a clock parameter (to pass to a submodule) shares the same real
        // clock and must stay legal.
        let f: syn::ItemFn = parse_quote! {
            async fn m(clk: Clock<Slow>, q: Out<Logic, Slow>) {
                let child_clk = clk.clone();
                loop { q.write(Logic::Zero); clk.tick().await; let _ = &child_clk; }
            }
        };
        assert!(check_no_clock_construction(&f).is_ok());
    }

    #[test]
    fn no_clock_construction_in_normal_module() {
        let f: syn::ItemFn = parse_quote! {
            async fn m(clk: Clock<Slow>, step: In<Bits<8>, Slow>, q: Out<Bits<8>, Slow>) {
                let mut c = Bits::zero();
                loop { q.write(c); clk.tick().await; c = c + step.read(); }
            }
        };
        assert!(check_no_clock_construction(&f).is_ok());
    }

    // ── CDC enforcement (check_cdc — signature-level) ───────────────────────
    //
    // A regular sequential module must be single-domain; foreign-domain crossings
    // go through a `#[hardware(synchronizer)]` module (which is not passed to
    // `check_cdc` at all). So the check is purely: reject any foreign-domain port.

    #[test]
    fn cdc_accepts_native_only_module() {
        let f: syn::ItemFn = parse_quote! {
            async fn m(clk: Clock<Slow>, step: In<Bits<8>, Slow>, q: Out<Bits<8>, Slow>) {
                let mut c = Bits::zero();
                loop { q.write(c); clk.tick().await; c = c + step.read(); }
            }
        };
        assert!(check_cdc(&f).is_ok());
    }

    #[test]
    fn cdc_rejects_foreign_input() {
        let f: syn::ItemFn = parse_quote! {
            async fn m(clk: Clock<Slow>, fast: In<Logic, Fast>, q: Out<Logic, Slow>) {
                loop { q.write(fast.read()); clk.tick().await; }
            }
        };
        assert!(check_cdc(&f).is_err());
    }

    #[test]
    fn cdc_rejects_foreign_output() {
        let f: syn::ItemFn = parse_quote! {
            async fn m(clk: Clock<Slow>, d: In<Logic, Slow>, fq: Out<Logic, Fast>) {
                loop { fq.write(d.read()); clk.tick().await; }
            }
        };
        assert!(check_cdc(&f).is_err());
    }

    #[test]
    fn cdc_accepts_foreign_generic_domain_input_is_still_foreign() {
        // A generic source domain `Src` differs from the concrete clock domain, so
        // it is foreign — a regular module still may not take it directly. (Only a
        // synchronizer may, and synchronizers are never passed to `check_cdc`.)
        let f: syn::ItemFn = parse_quote! {
            async fn m<Src>(clk: Clock<Slow>, d: In<Logic, Src>, q: Out<Logic, Slow>) {
                loop { q.write(d.read()); clk.tick().await; }
            }
        };
        assert!(check_cdc(&f).is_err());
    }

    #[test]
    fn cdc_skips_multiclock_modules() {
        // Two clocks → no unambiguous native domain → not checked here.
        let f: syn::ItemFn = parse_quote! {
            async fn m(cf: Clock<Fast>, cs: Clock<Slow>, fast: In<Logic, Fast>, q: Out<Logic, Slow>) {
                loop { q.write(fast.read()); cf.tick().await; cs.tick().await; }
            }
        };
        assert!(check_cdc(&f).is_ok());
    }

    #[test]
    fn sequential_must_be_async() {
        let f: syn::ItemFn = parse_quote! {
            fn counter(clk: Clock<MainClk>, input: In<u8>, out: Out<u8>) {}
        };
        assert!(validate_hardware_fn(&f, &HardwareMode::Sequential).is_err());
    }

    #[test]
    fn combinational_must_not_be_async() {
        let f: syn::ItemFn = parse_quote! {
            async fn gate(a: In<Logic>, out: Out<Logic>) {}
        };
        assert!(validate_hardware_fn(&f, &HardwareMode::Combinational).is_err());
    }

    #[test]
    fn rejects_return_value() {
        let f: syn::ItemFn = parse_quote! {
            async fn counter(clk: Clock<MainClk>, input: In<u8>, out: Out<u8>) -> u8 { loop {} }
        };
        assert!(validate_hardware_fn(&f, &HardwareMode::Sequential).is_err());
    }

    #[test]
    fn rejects_raw_parameter_type() {
        let f: syn::ItemFn = parse_quote! {
            async fn counter(clk: Clock<MainClk>, input: u8, out: Out<u8>) { loop {} }
        };
        assert!(validate_hardware_fn(&f, &HardwareMode::Sequential).is_err());
    }

    #[test]
    fn sequential_requires_clock() {
        let f: syn::ItemFn = parse_quote! {
            async fn counter(input: In<u8>, out: Out<u8>) { loop {} }
        };
        assert!(validate_hardware_fn(&f, &HardwareMode::Sequential).is_err());
    }

    #[test]
    fn combinational_rejects_clock() {
        let f: syn::ItemFn = parse_quote! {
            fn gate(clk: Clock<MainClk>, a: In<Logic>, out: Out<Logic>) {}
        };
        assert!(validate_hardware_fn(&f, &HardwareMode::Combinational).is_err());
    }

    #[test]
    fn allows_missing_in() {
        // A free-running module (no reset/enable) legitimately has no In<T>.
        let f: syn::ItemFn = parse_quote! {
            async fn counter(clk: Clock<MainClk>, out: Out<u8>) { loop { clk.tick().await; } }
        };
        assert!(validate_hardware_fn(&f, &HardwareMode::Sequential).is_ok());
    }

    #[test]
    fn requires_at_least_one_out() {
        let f: syn::ItemFn = parse_quote! {
            async fn counter(clk: Clock<MainClk>, input: In<u8>) { loop {} }
        };
        assert!(validate_hardware_fn(&f, &HardwareMode::Sequential).is_err());
    }

    #[test]
    fn valid_sequential() {
        let f: syn::ItemFn = parse_quote! {
            async fn counter(clk: Clock<MainClk>, input: In<u8>, out: Out<u8>) {
                loop { clk.tick().await; }
            }
        };
        assert!(validate_hardware_fn(&f, &HardwareMode::Sequential).is_ok());
    }

    #[test]
    fn sequential_missing_tick_await() {
        let f: syn::ItemFn = parse_quote! {
            async fn counter(clk: Clock<MainClk>, input: In<u8>, out: Out<u8>) { loop {} }
        };
        assert!(validate_hardware_fn(&f, &HardwareMode::Sequential).is_err());
    }

    #[test]
    fn sequential_missing_top_level_loop() {
        // Has a tick but no `loop` — a straight-line sequential body is rejected.
        let f: syn::ItemFn = parse_quote! {
            async fn counter(clk: Clock<MainClk>, input: In<u8>, out: Out<u8>) {
                clk.tick().await;
            }
        };
        assert!(validate_hardware_fn(&f, &HardwareMode::Sequential).is_err());
    }

    #[test]
    fn sequential_loop_after_lets_is_ok() {
        // Pre-loop `let` declarations before the top-level loop are fine.
        let f: syn::ItemFn = parse_quote! {
            async fn counter(clk: Clock<MainClk>, input: In<u8>, out: Out<u8>) {
                let mut c = 0u8;
                loop { out.write(c); clk.tick().await; c = c + 1; }
            }
        };
        assert!(validate_hardware_fn(&f, &HardwareMode::Sequential).is_ok());
    }

    #[test]
    fn param_shadowing_rejected() {
        // `let out` shadows the output port `out`.
        let f: syn::ItemFn = parse_quote! {
            async fn m(clk: Clock<MainClk>, out: Out<u8>) {
                loop { let out = 0u8; clk.tick().await; }
            }
        };
        assert!(check_no_param_shadowing(&f).is_err());
    }

    #[test]
    fn param_shadowing_in_tuple_pattern_rejected() {
        // Shadowing hidden inside a tuple `let` is still caught.
        let f: syn::ItemFn = parse_quote! {
            async fn m(clk: Clock<MainClk>, step: In<u8>, out: Out<u8>) {
                loop { let (step, _c) = (0u8, 1u8); out.write(step); clk.tick().await; }
            }
        };
        assert!(check_no_param_shadowing(&f).is_err());
    }

    #[test]
    fn non_shadowing_local_is_ok() {
        // A local whose name differs from every port is fine.
        let f: syn::ItemFn = parse_quote! {
            async fn m(clk: Clock<MainClk>, step: In<u8>, out: Out<u8>) {
                loop { let next = step.read(); out.write(next); clk.tick().await; }
            }
        };
        assert!(check_no_param_shadowing(&f).is_ok());
    }

    #[test]
    fn multiple_top_level_loops_rejected() {
        let f: syn::ItemFn = parse_quote! {
            async fn m(clk: Clock<MainClk>, out: Out<u8>) {
                loop { out.write(0u8); clk.tick().await; }
                loop { clk.tick().await; }
            }
        };
        assert!(validate_hardware_fn(&f, &HardwareMode::Sequential).is_err());
    }

    #[test]
    fn code_after_top_level_loop_rejected() {
        let f: syn::ItemFn = parse_quote! {
            async fn m(clk: Clock<MainClk>, out: Out<u8>) {
                loop { out.write(0u8); clk.tick().await; }
                out.write(1u8);
            }
        };
        assert!(validate_hardware_fn(&f, &HardwareMode::Sequential).is_err());
    }

    #[test]
    fn synchronizer_requires_top_level_loop() {
        let f: syn::ItemFn = parse_quote! {
            async fn s(clk: Clock<Slow>, d: In<Logic, Fast>, q: Out<Logic, Slow>) {
                q.write(d.read()); clk.tick().await;
            }
        };
        assert!(validate_hardware_fn(&f, &HardwareMode::Synchronizer).is_err());
    }

    #[test]
    fn combinational_needs_no_loop() {
        // Combinational modules have no user-written loop (the macro adds it), so
        // the loop check must not apply to them.
        let f: syn::ItemFn = parse_quote! {
            fn gate(a: In<Logic>, b: In<Logic>, out: Out<Logic>) {
                out.write(a.read());
            }
        };
        assert!(validate_hardware_fn(&f, &HardwareMode::Combinational).is_ok());
    }

    #[test]
    fn valid_combinational() {
        let f: syn::ItemFn = parse_quote! {
            fn gate(a: In<Logic>, b: In<Logic>, out: Out<Logic>) {}
        };
        assert!(validate_hardware_fn(&f, &HardwareMode::Combinational).is_ok());
    }
}
