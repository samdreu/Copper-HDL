//! Register power-on values — R11 (2026-10-29; rustc the judge since D-21, 2026-11-07).
//!
//! A pre-loop `let mut` the liveness inference calls a register is a flip-flop,
//! and its initializer is the flip-flop's **power-on value**: the simulator starts
//! the binding there, and the transpiler states the same value in the emitted
//! SystemVerilog as an `initial` assignment (`emit::reg_inits`). That is only
//! possible for a **constant** — or for `Bits::x()`, which says the register starts
//! *undefined*: there is nothing to state, and the design needs a reset before the
//! register is read. Refused in **both** front-ends, with no opt-out.
//!
//! **What "constant" means (D-21 A3).** Rust's constant: the macro emits every
//! pre-loop register initializer inside an inline `const { … }` block
//! ([`preloop_register_initializers`] names them), so rustc admits exactly what it
//! can evaluate at compile time — a literal, a file `const`, an associated const
//! (`T::K`), a const generic, a call to a `const fn` (every `Bits`/`Logic`
//! constructor is one, and so is a user's `const fn f`), arithmetic over those — and
//! refuses everything else (E0015 for a call to a non-`const` fn, E0435 for a
//! non-constant value). Until D-21 the rule was the syntactic [`is_constant_expr`],
//! which refused a `const fn f(3)` and admitted a non-`const` `T::bubble()`.
//!
//! What stays syntactic, in both front-ends ([`check_register_initializers`]):
//! an initializer that **reads a run-time value** — a parameter (an input port, the
//! clock) or an earlier pre-loop binding. rustc refuses that too (E0435), but its
//! message talks about `const` items; this one says why a register needs a constant.
//!
//! The transpiler cannot see rustc's verdict, so it keeps its own fold
//! (`chir_lower::lower_register_init`, over `consts::ConstEnv`) and asks one more
//! syntactic question here, [`check_register_initializer_calls`]: every call in the
//! initializer is to a `const fn` it knows — a `Bits`/`Logic` constructor
//! ([`is_core_const_ctor`]) or a `const fn` of the file. That is the half of rustc's
//! judgement the fold alone would miss: the fold inlines an associated ctor
//! structurally (`Latch::bubble()`) whether or not it is `const`.

use std::collections::BTreeSet;

use syn::spanned::Spanned;
use syn::visit::Visit;
use syn::{Expr, FnArg, ItemFn, Pat, Stmt};

pub use crate::cfg::is_constant_expr;

/// Every pre-loop `let` in the register set with an initializer, in source order:
/// `(name, init)`. These are the initializers the macro evaluates in a `const { }`
/// block (D-21 A3). A pre-loop binding the inference calls a wire is a wire of the
/// first resumption (SPEC I.2.3 / I.7.2(c), D-21 C2) and is not one of them.
pub fn preloop_register_initializers<'a>(
    f: &'a ItemFn,
    registers: &[String],
) -> Vec<(String, &'a Expr)> {
    let mut out = Vec::new();
    for s in &f.block.stmts {
        match s {
            Stmt::Expr(Expr::Loop(_), _) => break,
            Stmt::Local(l) => {
                let Some(name) = local_name(&l.pat) else { continue };
                if !registers.iter().any(|r| *r == name) {
                    continue;
                }
                if let Some(init) = &l.init {
                    out.push((name, &*init.expr));
                }
            }
            _ => {}
        }
    }
    out
}

/// The names a pre-loop register initializer cannot read: the fn's parameters and
/// the pre-loop bindings before it.
fn runtime_names(f: &ItemFn) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    for arg in &f.sig.inputs {
        if let FnArg::Typed(pt) = arg {
            pat_names(&pt.pat, &mut names);
        }
    }
    names
}

fn pat_names(p: &Pat, out: &mut BTreeSet<String>) {
    match p {
        Pat::Ident(pi) => {
            out.insert(pi.ident.to_string());
        }
        Pat::Type(pt) => pat_names(&pt.pat, out),
        Pat::Tuple(t) => t.elems.iter().for_each(|e| pat_names(e, out)),
        Pat::TupleStruct(t) => t.elems.iter().for_each(|e| pat_names(e, out)),
        Pat::Reference(r) => pat_names(&r.pat, out),
        _ => {}
    }
}

/// The first run-time name `e` reads, if any.
fn reads_runtime(e: &Expr, runtime: &BTreeSet<String>) -> Option<String> {
    struct V<'r> {
        runtime: &'r BTreeSet<String>,
        found: Option<String>,
    }
    impl<'ast> Visit<'ast> for V<'_> {
        fn visit_expr_path(&mut self, p: &'ast syn::ExprPath) {
            if self.found.is_none() && p.qself.is_none() && p.path.segments.len() == 1 {
                let name = p.path.segments[0].ident.to_string();
                if self.runtime.contains(&name) {
                    self.found = Some(name);
                }
            }
            syn::visit::visit_expr_path(self, p);
        }
    }
    let mut v = V { runtime, found: None };
    v.visit_expr(e);
    v.found
}

/// Pre-loop register bindings whose initializer **reads a run-time value** (a
/// parameter or an earlier pre-loop binding), as `(name, init)` in source order.
/// The syntactic half of R11 both front-ends run; the rest of "constant" is
/// rustc's (the macro's `const { }`) and the transpiler's fold.
pub fn non_constant_register_initializers<'a>(
    f: &'a ItemFn,
    registers: &[String],
) -> Vec<(String, &'a Expr)> {
    let mut runtime = runtime_names(f);
    let mut out = Vec::new();
    for s in &f.block.stmts {
        match s {
            Stmt::Expr(Expr::Loop(_), _) => break,
            Stmt::Local(l) => {
                let Some(name) = local_name(&l.pat) else { continue };
                if registers.iter().any(|r| *r == name) {
                    if let Some(init) = &l.init {
                        if reads_runtime(&init.expr, &runtime).is_some() {
                            out.push((name.clone(), &*init.expr));
                        }
                    }
                }
                pat_names(&l.pat, &mut runtime);
            }
            _ => {}
        }
    }
    out
}

fn local_name(p: &Pat) -> Option<String> {
    match p {
        Pat::Ident(pi) => Some(pi.ident.to_string()),
        Pat::Type(pt) => local_name(&pt.pat),
        _ => None,
    }
}

/// The rule's syntactic half: `Err` (spanned at the initializer) for the first
/// register whose initializer reads a run-time value, with
/// [`register_init_message`]. `registers` is the module's inferred register set.
pub fn check_register_initializers(f: &ItemFn, registers: &[String]) -> Result<(), syn::Error> {
    match non_constant_register_initializers(f, registers).first() {
        None => Ok(()),
        Some((name, init)) => Err(syn::Error::new(init.span(), register_init_message(name, init))),
    }
}

/// The message both front-ends print, verbatim, for an initializer that reads a
/// run-time value.
pub fn register_init_message(name: &str, init: &Expr) -> String {
    format!(
        "register `{name}` is initialized from {what} that reads a run-time value, so the \
         hardware has no power-on value for it: the simulator would start `{name}` at \
         whatever the expression evaluates to, and the emitted SystemVerilog cannot state \
         that value. Initialize `{name}` with a Rust constant — a literal, a `const` item or \
         associated const, a call to a `const fn` such as `Bits::zero()`, \
         `Bits::from_u32(k)` or `Bits::from_lit::<K>()` — or with `Bits::x()` to say it \
         starts undefined and must be reset before it is read",
        what = describe(init)
    )
}

/// What kind of expression the initializer is, for the message.
fn describe(e: &Expr) -> &'static str {
    match e {
        Expr::MethodCall(_) => "a method call",
        Expr::Call(_) => "a call",
        Expr::Path(_) => "a local or parameter",
        Expr::Field(_) => "a field read",
        Expr::Index(_) => "an index expression",
        Expr::Macro(_) => "a macro invocation",
        Expr::If(_) | Expr::Match(_) | Expr::Block(_) => "a block or branch expression",
        Expr::Binary(_) | Expr::Unary(_) | Expr::Cast(_) | Expr::Paren(_) => "an expression",
        Expr::Array(_) | Expr::Repeat(_) => "an array expression",
        Expr::Struct(_) => "a struct literal",
        _ => "an expression",
    }
}

/// The path of a call, generic arguments dropped: `Bits::<8>::from_u8` →
/// `Bits::from_u8`, `Latch::bubble` → `Latch::bubble`. `None` for a call through
/// something other than a plain path.
pub fn call_path_text(c: &syn::ExprCall) -> Option<String> {
    let Expr::Path(p) = &*c.func else { return None };
    if p.qself.is_some() {
        return None;
    }
    Some(p.path.segments.iter().map(|s| s.ident.to_string()).collect::<Vec<_>>().join("::"))
}

/// `copper_core`'s value constructors, every one a `const fn` since D-21:
/// `Bits::{zero, all_ones, x, from_array, from_u8 … from_u128, from_usize, from_lit}`
/// and `Logic::from_bool`. `path` as [`call_path_text`] gives it.
pub fn is_core_const_ctor(path: &str) -> bool {
    let mut segs = path.rsplit("::");
    let (Some(f), Some(ty)) = (segs.next(), segs.next()) else { return false };
    match ty {
        "Bits" => matches!(
            f,
            "zero"
                | "all_ones"
                | "x"
                | "from_array"
                | "from_u8"
                | "from_u16"
                | "from_u32"
                | "from_u64"
                | "from_u128"
                | "from_usize"
                | "from_lit"
        ),
        "Logic" => f == "from_bool",
        _ => false,
    }
}

/// Pre-loop register initializers that call something that is not a `const fn` the
/// caller knows: `(register, callee)` in source order. A callee is known when it is
/// a core constructor ([`is_core_const_ctor`]) or `is_const_fn(path)` (the
/// transpiler passes `ConstEnv::is_const_fn`, the file's `const fn`s).
pub fn non_const_calls_in_register_initializers(
    f: &ItemFn,
    registers: &[String],
    is_const_fn: &dyn Fn(&str) -> bool,
) -> Vec<(String, String, proc_macro2::Span)> {
    struct V<'a> {
        is_const_fn: &'a dyn Fn(&str) -> bool,
        found: Option<(String, proc_macro2::Span)>,
    }
    impl<'ast> Visit<'ast> for V<'_> {
        fn visit_expr_call(&mut self, c: &'ast syn::ExprCall) {
            if self.found.is_none() {
                let path = call_path_text(c).unwrap_or_else(|| "<a non-path callee>".to_string());
                if !is_core_const_ctor(&path) && !(self.is_const_fn)(&path) {
                    self.found = Some((path, c.span()));
                }
            }
            syn::visit::visit_expr_call(self, c);
        }
    }
    let mut out = Vec::new();
    for (name, init) in preloop_register_initializers(f, registers) {
        let mut v = V { is_const_fn, found: None };
        v.visit_expr(init);
        if let Some((callee, span)) = v.found {
            out.push((name, callee, span));
        }
    }
    out
}

/// The transpiler's half of rustc's judgement: `Err` for the first register whose
/// initializer calls a fn that is not `const` (rustc refuses that in the macro's
/// `const { }` block with E0015), with [`register_init_call_message`].
pub fn check_register_initializer_calls(
    f: &ItemFn,
    registers: &[String],
    is_const_fn: &dyn Fn(&str) -> bool,
) -> Result<(), syn::Error> {
    match non_const_calls_in_register_initializers(f, registers, is_const_fn).first() {
        None => Ok(()),
        Some((name, callee, span)) => Err(syn::Error::new(*span, register_init_call_message(name, callee))),
    }
}

/// The transpiler's message for a call to a non-`const` fn in a register
/// initializer (the simulator's is rustc's E0015 on the macro's `const { }`).
pub fn register_init_call_message(name: &str, callee: &str) -> String {
    format!(
        "register `{name}` is initialized by a call to `{callee}`, which is not a `const fn`, \
         so the initializer is not a constant and the hardware has no power-on value for it \
         (rustc refuses the same initializer in the simulator: E0015). Make `{callee}` a \
         `const fn`, initialize `{name}` with another Rust constant, or with `Bits::x()` to \
         say it starts undefined and must be reset before it is read"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fn_of(body: &str) -> ItemFn {
        syn::parse_str(&format!(
            "async fn m(clk: Clock<D>, a: In<Bits<8>, D>, o: RegOut<Bits<8>, D>) {{ {body} }}"
        ))
        .expect("parse ItemFn")
    }

    fn flagged(body: &str) -> Vec<String> {
        let f = fn_of(body);
        let regs = crate::infer_registers(&f);
        non_constant_register_initializers(&f, &regs)
            .into_iter()
            .map(|(n, _)| n)
            .collect()
    }

    fn reg_body(init: &str) -> String {
        format!("let mut r = {init}; loop {{ clk.tick().await; r = r + a.read(); o.write(r); }}")
    }

    #[test]
    fn no_constant_spelling_reads_a_runtime_value() {
        for init in [
            "0",
            "0u8",
            "true",
            "Logic::Zero",
            "State::IDLE",
            "Bits::zero()",
            "Bits::<8>::zero()",
            "Bits::from_u32(1)",
            "Bits::<8>::from_u8(3)",
            "Bits::from_lit::<0>()",
            "Bits::x()",
            "Bits::<8>::x()",
            "[Logic::Zero; 4]",
            "SEED",
            "Bits::from_u32(1 << 31)",
            "Latch::bubble()",
            // D-21 A3: R11 no longer refuses these syntactically; rustc judges them.
            "f(3)",
            "Cfg::K",
            "seed()",
        ] {
            assert_eq!(flagged(&reg_body(init)), Vec::<String>::new(), "{init}");
        }
    }

    #[test]
    fn an_initializer_reading_a_runtime_value_is_flagged() {
        for init in ["a.read()", "a.read() + 1", "clk.cycle()", "Bits::from_u32(a.read().as_u32())"] {
            assert_eq!(flagged(&reg_body(init)), vec!["r".to_string()], "{init}");
        }
        // An earlier pre-loop binding is a run-time value too.
        let body = "let k = a.read(); let mut r = k; loop { clk.tick().await; r = r + a.read(); o.write(r); }";
        assert_eq!(flagged(body), vec!["r".to_string()]);
    }

    #[test]
    fn a_pre_loop_wire_and_a_loop_born_register_are_not_asked() {
        // `w` is never reassigned: a wire, whatever its initializer.
        let body = "let w = a.read(); let mut r = Bits::zero(); loop { clk.tick().await; r = r + w; o.write(r); }";
        assert_eq!(flagged(body), Vec::<String>::new());
        // `t` is born inside the loop from a read and lives across the tick.
        let body = "loop { let mut t = a.read(); clk.tick().await; o.write(t); clk.tick().await; }";
        assert_eq!(flagged(body), Vec::<String>::new());
    }

    #[test]
    fn the_error_names_the_register_and_points_at_the_initializer() {
        let f = fn_of("let mut r = a.read(); loop { clk.tick().await; r = r + a.read(); o.write(r); }");
        let err = check_register_initializers(&f, &crate::infer_registers(&f)).unwrap_err();
        let msg = err.to_string();
        assert!(msg.starts_with("register `r` is initialized from a method call that reads a run-time value"), "{msg}");
        assert!(msg.contains("`Bits::x()`"), "{msg}");
        let f = fn_of("let mut r = Bits::zero(); loop { clk.tick().await; r = r + a.read(); o.write(r); }");
        assert!(check_register_initializers(&f, &crate::infer_registers(&f)).is_ok());
    }

    #[test]
    fn calls_must_be_const_fns_the_transpiler_knows() {
        let known = |p: &str| p == "f" || p == "Latch::bubble";
        let calls = |init: &str| {
            let f = fn_of(&reg_body(init));
            let regs = crate::infer_registers(&f);
            non_const_calls_in_register_initializers(&f, &regs, &known)
                .into_iter()
                .map(|(_, c, _)| c)
                .collect::<Vec<_>>()
        };
        for ok in ["Bits::zero()", "Bits::<8>::from_u8(f(3))", "Latch::bubble()", "Logic::from_bool(true)", "7"] {
            assert_eq!(calls(ok), Vec::<String>::new(), "{ok}");
        }
        assert_eq!(calls("seed()"), vec!["seed".to_string()]);
        assert_eq!(calls("Other::bubble()"), vec!["Other::bubble".to_string()]);
        assert_eq!(calls("Bits::from_u32(g(1))"), vec!["g".to_string()]);
    }
}
