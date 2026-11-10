//! Shape rules over the data of SPEC §I.14 that both front-ends enforce with one
//! message (Phase C step 8, `design_docs/review/MEASUREMENTS.md` §M):
//!
//! - [`check_outside_methods`] — `checked_*` (an `Option`, I.14.3(c)) and
//!   `is_valid()` (a question about the simulator's third value, which no 2-state
//!   realization holds, I.14.3(d)) are OUTSIDE (D-19; I.14.7 (7)); so is `==`/`!=`
//!   with an X-literal operand, the same question spelled with an operator (D-23 (3)),
//!   and a `Logic::X` pattern (or one naming a body `const` whose value is X), the
//!   same question spelled as a pattern (D-24 (2)).
//! - [`check_target_byte_order`] — `to_be` / `to_le` mean the simulating host's byte order,
//!   which SystemVerilog has not: refused (D-32 (2), I.14.3(c)).
//! - [`check_memory_port_index`] — a memory port index `k ≥ R` (`≥ W`) is a shape
//!   rule decided at the instance (D-19 6b-A, I.14.6(c); I.14.7 (9)).

use syn::spanned::Spanned;
use syn::visit::Visit;
use syn::{Expr, ItemFn};

/// The message for a `checked_*` call (I.14.3(c)).
pub const CHECKED_METHOD_MESSAGE: &str = "`checked_*` arithmetic returns an `Option`, which \
    is not a hardware value: it is OUTSIDE (SPEC I.14.3(c), D-19). Use `wrapping_*` (modular) \
    or `saturating_*` (clamped), or test the operands before the operation";

/// The message for `==`/`!=` with an X literal operand (I.14.3(d), D-23 (3)).
pub const X_COMPARISON_MESSAGE: &str = "a comparison with the X literal (`Logic::X`, `Bits::x()`) \
    asks whether a value holds the simulator's third value X, which no 2-state realization holds: \
    it is `is_valid()` spelled with an operator, and OUTSIDE (SPEC I.14.3(d), D-19, D-23)";

/// The message for a pattern that matches the X literal (I.14.3(d)–(e), D-24 (2)).
pub const X_PATTERN_MESSAGE: &str = "a pattern matching the X literal (`Logic::X`, or a `const` \
    whose value is X) asks whether a value holds the simulator's third value X, which no 2-state \
    realization holds: it is `is_valid()` spelled as a pattern, and OUTSIDE (SPEC I.14.3(d), D-19, \
    D-24). A `Logic` scrutinee holding X matches `_` only";

/// The message for `is_valid()` (I.14.3(d)).
pub const IS_VALID_MESSAGE: &str = "`is_valid()` asks whether a value holds the simulator's \
    third value X, which no 2-state realization holds: it is OUTSIDE (SPEC I.14.3(d), D-19)";

/// The message for `to_be` / `to_le` (SPEC I.14.3(c), D-32 (2)): refused in both front-ends.
pub const TARGET_BYTE_ORDER_MESSAGE: &str = "`to_be` / `to_le` convert to the byte order of the \
    machine the program runs on (the identity on one, `swap_bytes` on the other), and SystemVerilog has \
    no byte order: a design whose meaning changes with the simulating host is not one hardware can state, \
    so they are refused (SPEC I.14.3(c), D-32). Write `swap_bytes()` or the value itself";

/// The tail of the transpiler's message for a method it does not lower (SPEC II.1.3's fourth
/// named exception, D-32 (1)); the transpiler writes ``method `<m>` `` ahead of it, one
/// message for every such method, in the `.`-form and the path-call form (`u16::pow(v, 2)`).
pub const UNLOWERED_METHOD_MESSAGE: &str = "is not lowered by the transpiler, which emits no text \
    for it. A Rust integer method the transpiler does not lower compiles under `#[hardware]` (rustc \
    type-checks it; the macro sees no types) and simulates with Rust's meaning: the fourth named \
    exception of SPEC II.1.3 (D-32, I.14.3(c)). The integer methods it lowers are \
    `copper_codegen::LOWERED_INT_METHODS`";

/// The integer types whose path-call form (`u32::to_be(v)`) is a method call (MEASUREMENTS §AI.2).
const PRIMITIVE_INTS: &[&str] =
    &["u8", "u16", "u32", "u64", "u128", "usize", "i8", "i16", "i32", "i64", "i128", "isize"];

/// Refuse `x.to_be()` / `x.to_le()` and their path-call form `<int>::to_be(v)` in every module
/// mode (SPEC I.14.3(c), D-32 (2)). By name, as `checked_*` is: the two names mean nothing
/// else in Rust's integer API. One function for the macro and `transpile_target`.
pub fn check_target_byte_order(f: &ItemFn) -> Result<(), syn::Error> {
    struct V(Option<syn::Error>);
    impl<'ast> Visit<'ast> for V {
        fn visit_expr_method_call(&mut self, mc: &'ast syn::ExprMethodCall) {
            if self.0.is_none() && mc.args.is_empty() && (mc.method == "to_be" || mc.method == "to_le") {
                self.0 = Some(syn::Error::new(mc.method.span(), TARGET_BYTE_ORDER_MESSAGE));
            }
            syn::visit::visit_expr_method_call(self, mc);
        }
        fn visit_expr_call(&mut self, c: &'ast syn::ExprCall) {
            if self.0.is_none() {
                if let Expr::Path(p) = &*c.func {
                    let segs: Vec<String> = p.path.segments.iter().map(|s| s.ident.to_string()).collect();
                    if let [.., ty, m] = segs.as_slice() {
                        if (m == "to_be" || m == "to_le") && PRIMITIVE_INTS.contains(&ty.as_str()) {
                            self.0 = Some(syn::Error::new(p.span(), TARGET_BYTE_ORDER_MESSAGE));
                        }
                    }
                }
            }
            syn::visit::visit_expr_call(self, c);
        }
    }
    let mut v = V(None);
    v.visit_block(&f.block);
    v.0.map_or(Ok(()), Err)
}

const CHECKED: &[&str] = &[
    "checked_add",
    "checked_sub",
    "checked_mul",
    "checked_div",
    "checked_rem",
    "checked_neg",
    "checked_shl",
    "checked_shr",
    "checked_pow",
];

/// Refuse the OUTSIDE methods of I.14.3: `checked_*` and `is_valid()`, `==`/`!=`
/// with an X-literal operand (D-23 (3)), and a pattern matching the X literal — a
/// `Logic::X` pattern anywhere in a pattern (`match`, `if let`, `let`, `matches!`),
/// or an identifier pattern naming a body `const` whose initializer is an X literal
/// (D-24 (2)) — in every module mode. One function for the macro and `transpile_target`.
pub fn check_outside_methods(f: &ItemFn) -> Result<(), syn::Error> {
    struct V {
        err: Option<syn::Error>,
        x_consts: std::collections::BTreeSet<String>,
    }
    impl<'ast> Visit<'ast> for V {
        fn visit_expr_method_call(&mut self, mc: &'ast syn::ExprMethodCall) {
            if self.err.is_none() {
                let m = mc.method.to_string();
                if CHECKED.contains(&m.as_str()) {
                    self.err = Some(syn::Error::new(mc.method.span(), CHECKED_METHOD_MESSAGE));
                } else if m == "is_valid" && mc.args.is_empty() {
                    self.err = Some(syn::Error::new(mc.method.span(), IS_VALID_MESSAGE));
                }
            }
            syn::visit::visit_expr_method_call(self, mc);
        }
        fn visit_expr_binary(&mut self, b: &'ast syn::ExprBinary) {
            if self.err.is_none() && matches!(b.op, syn::BinOp::Eq(_) | syn::BinOp::Ne(_)) {
                let x = if is_x_literal(&b.right) { Some(&b.right) } else if is_x_literal(&b.left) { Some(&b.left) } else { None };
                if let Some(x) = x {
                    self.err = Some(syn::Error::new(x.span(), X_COMPARISON_MESSAGE));
                }
            }
            syn::visit::visit_expr_binary(self, b);
        }
        fn visit_pat(&mut self, p: &'ast syn::Pat) {
            if self.err.is_none() {
                let hit = match p {
                    syn::Pat::Path(pp) => path_is_logic_x(&pp.path),
                    syn::Pat::Ident(pi) if pi.subpat.is_none() => self.x_consts.contains(&pi.ident.to_string()),
                    _ => false,
                };
                if hit {
                    self.err = Some(syn::Error::new(p.span(), X_PATTERN_MESSAGE));
                }
            }
            syn::visit::visit_pat(self, p);
        }
        fn visit_macro(&mut self, m: &'ast syn::Macro) {
            // `matches!(e, pat)` is admitted (DESUGARED, I.2.4): its pattern is a pattern.
            if m.path.segments.last().is_some_and(|s| s.ident == "matches") {
                use syn::parse::Parser;
                let parse = |input: syn::parse::ParseStream| -> syn::Result<(Expr, syn::Pat)> {
                    let e: Expr = input.parse()?;
                    input.parse::<syn::Token![,]>()?;
                    Ok((e, syn::Pat::parse_multi_with_leading_vert(input)?))
                };
                if let Ok((e, pat)) = parse.parse2(m.tokens.clone()) {
                    self.visit_expr(&e);
                    self.visit_pat(&pat);
                }
            }
            syn::visit::visit_macro(self, m);
        }
    }
    // A `const` item of the body whose initializer is an X literal: a pattern naming
    // it is the X pattern (D-24 (2)). A file-level `const` is not in the `ItemFn`
    // either front-end's rule holds (MEASUREMENTS §T).
    struct Consts(std::collections::BTreeSet<String>);
    impl<'ast> Visit<'ast> for Consts {
        fn visit_item_const(&mut self, c: &'ast syn::ItemConst) {
            if is_x_literal(&c.expr) {
                self.0.insert(c.ident.to_string());
            }
        }
    }
    let mut consts = Consts(Default::default());
    consts.visit_block(&f.block);
    let mut v = V { err: None, x_consts: consts.0 };
    v.visit_block(&f.block);
    v.err.map_or(Ok(()), Err)
}

/// `Logic::X` as a path (`Logic::X`, `copper_core::Logic::X`).
fn path_is_logic_x(p: &syn::Path) -> bool {
    let segs: Vec<String> = p.segments.iter().map(|s| s.ident.to_string()).collect();
    segs.len() >= 2 && segs[segs.len() - 1] == "X" && segs[segs.len() - 2] == "Logic"
}

/// `Logic::X`, `Bits::x()` or `Bits::<N>::x()` (through parentheses and `&`): the
/// X literal of I.14.4.
fn is_x_literal(e: &Expr) -> bool {
    match e {
        Expr::Paren(p) => is_x_literal(&p.expr),
        Expr::Group(g) => is_x_literal(&g.expr),
        Expr::Reference(r) => is_x_literal(&r.expr),
        Expr::Path(p) => path_is_logic_x(&p.path),
        Expr::Call(c) if c.args.is_empty() => match &*c.func {
            Expr::Path(p) => {
                let segs: Vec<String> = p.path.segments.iter().map(|s| s.ident.to_string()).collect();
                segs.len() >= 2 && segs[segs.len() - 1] == "x" && segs[segs.len() - 2] == "Bits"
            }
            _ => false,
        },
        _ => false,
    }
}

/// The message for a memory port index out of range (I.14.6(c)).
pub fn memory_port_index_message(mem: &str, kind: &str, k: u128, n: u128) -> String {
    let what = if kind == "read_port" { "read port(s), R" } else { "write port(s), W" };
    format!(
        "`{mem}.{kind}::<{k}>()`: the memory has {n} {what} = {n}, so port index {k} does not exist — \
         a port index must be below the port count (SPEC I.14.6(c), D-19 6b-A; decided at the \
         instance, refused in both front-ends)"
    )
}

/// `(R, W)` of a `Memory<T, R, W, D, …>` type's generic arguments, each `None` when
/// it is not an integer literal (a const generic: the instance decides, and rustc's
/// monomorphization of `read_port::<K>` then indexes out of range at run time).
fn port_counts(args: &syn::AngleBracketedGenericArguments) -> (Option<u128>, Option<u128>) {
    let lit = |a: Option<&syn::GenericArgument>| -> Option<u128> { generic_int(a?) };
    (lit(args.args.iter().nth(1)), lit(args.args.iter().nth(2)))
}

/// An integer-literal generic argument (`1`, `{ 1 }`), else `None`.
fn generic_int(a: &syn::GenericArgument) -> Option<u128> {
    fn of(e: &Expr) -> Option<u128> {
        match e {
            Expr::Lit(syn::ExprLit { lit: syn::Lit::Int(i), .. }) => i.base10_parse::<u128>().ok(),
            Expr::Block(b) if b.block.stmts.len() == 1 => match &b.block.stmts[0] {
                syn::Stmt::Expr(e, None) => of(e),
                _ => None,
            },
            _ => None,
        }
    }
    match a {
        syn::GenericArgument::Const(e) => of(e),
        _ => None,
    }
}

/// The `Memory` generic arguments in a path (`Memory::<…>::new`, `Memory<…>`).
fn memory_args(p: &syn::Path) -> Option<&syn::AngleBracketedGenericArguments> {
    p.segments.iter().find(|s| s.ident == "Memory").and_then(|s| match &s.arguments {
        syn::PathArguments::AngleBracketed(a) => Some(a),
        _ => None,
    })
}

/// Refuse `mem.read_port::<k>()` with `k ≥ R` and `mem.write_port::<k>()` with
/// `k ≥ W`, where `R`/`W` are literal generic arguments of the memory's declared
/// (`let mem = Memory::<T, R, W, …>::new(…)`, or an annotated `let mem: Memory<…>`)
/// or received (`mem: Memory<T, R, W, …>`) type and `k` is an integer literal.
/// A count or an index that is a const generic is the instance's (rustc's) to
/// decide.
pub fn check_memory_port_index(f: &ItemFn) -> Result<(), syn::Error> {
    use std::collections::BTreeMap;
    let mut mems: BTreeMap<String, (Option<u128>, Option<u128>)> = BTreeMap::new();
    struct Decls<'m> {
        mems: &'m mut BTreeMap<String, (Option<u128>, Option<u128>)>,
    }
    fn find_path_args(e: &Expr) -> Option<(Option<u128>, Option<u128>)> {
        struct P {
            found: Option<(Option<u128>, Option<u128>)>,
        }
        impl<'ast> Visit<'ast> for P {
            fn visit_path(&mut self, p: &'ast syn::Path) {
                if self.found.is_none() {
                    if let Some(a) = memory_args(p) {
                        self.found = Some(port_counts(a));
                    }
                }
                syn::visit::visit_path(self, p);
            }
        }
        let mut v = P { found: None };
        v.visit_expr(e);
        v.found
    }
    impl<'ast> Visit<'ast> for Decls<'_> {
        fn visit_local(&mut self, l: &'ast syn::Local) {
            let (name, ty) = match &l.pat {
                syn::Pat::Ident(pi) => (Some(pi.ident.to_string()), None),
                syn::Pat::Type(pt) => match &*pt.pat {
                    syn::Pat::Ident(pi) => (Some(pi.ident.to_string()), Some(&*pt.ty)),
                    _ => (None, None),
                },
                _ => (None, None),
            };
            if let Some(name) = name {
                let from_ty = ty.and_then(|t| match t {
                    syn::Type::Path(tp) => memory_args(&tp.path).map(port_counts),
                    _ => None,
                });
                let from_init = l.init.as_ref().and_then(|i| find_path_args(&i.expr));
                if let Some(c) = from_ty.or(from_init) {
                    self.mems.insert(name, c);
                }
            }
            syn::visit::visit_local(self, l);
        }
    }
    Decls { mems: &mut mems }.visit_block(&f.block);
    for arg in &f.sig.inputs {
        let syn::FnArg::Typed(pt) = arg else { continue };
        let syn::Type::Path(tp) = &*pt.ty else { continue };
        let syn::Pat::Ident(pi) = &*pt.pat else { continue };
        if let Some(a) = memory_args(&tp.path) {
            mems.insert(pi.ident.to_string(), port_counts(a));
        }
    }
    if mems.is_empty() {
        return Ok(());
    }
    struct Uses<'m> {
        mems: &'m BTreeMap<String, (Option<u128>, Option<u128>)>,
        err: Option<syn::Error>,
    }
    impl<'ast> Visit<'ast> for Uses<'_> {
        fn visit_expr_method_call(&mut self, mc: &'ast syn::ExprMethodCall) {
            let kind = mc.method.to_string();
            if self.err.is_none() && (kind == "read_port" || kind == "write_port") {
                let recv = match &*mc.receiver {
                    Expr::Path(p) => p.path.get_ident().map(|i| i.to_string()),
                    _ => None,
                };
                let k = mc.turbofish.as_ref().and_then(|t| t.args.first()).and_then(generic_int);
                if let (Some(name), Some(k)) = (recv, k) {
                    if let Some((r, w)) = self.mems.get(&name) {
                        let n = if kind == "read_port" { *r } else { *w };
                        if let Some(n) = n {
                            if k >= n {
                                self.err = Some(syn::Error::new(
                                    mc.span(),
                                    memory_port_index_message(&name, &kind, k, n),
                                ));
                            }
                        }
                    }
                }
            }
            syn::visit::visit_expr_method_call(self, mc);
        }
    }
    let mut u = Uses { mems: &mems, err: None };
    u.visit_block(&f.block);
    u.err.map_or(Ok(()), Err)
}
