//! The shared constant evaluator (SPEC I.14.4: Rust's constants, one evaluator for
//! both front-ends and the interpreter). Started 2026-11-06 for Phase C step 5
//! row 2 (II.3 (f1): a counted `for` bound named by a file `const` must be known
//! statically non-empty); row 7 ((d), (g)) extends it.
//!
//! It evaluates an integer constant expression over a name environment and returns
//! `None` for anything outside the grammar it covers — never a guess. Arithmetic is
//! over `i128` with checked operations: an overflow, a division by zero or a shift
//! out of range is `None` (rustc rejects such a constant; this evaluator does not
//! invent a value for it).

use std::collections::HashMap;

/// An integer literal as Rust spells it: any base (`0x`, `0o`, `0b`), `_`
/// separators, an optional integer type suffix (`8usize`, `0xFF_u8`).
pub fn parse_int_lit(text: &str) -> Option<i128> {
    let t: String = text.chars().filter(|&c| c != '_').collect();
    let (radix, digits) = if let Some(r) = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
        (16, r)
    } else if let Some(r) = t.strip_prefix("0o") {
        (8, r)
    } else if let Some(r) = t.strip_prefix("0b") {
        (2, r)
    } else {
        (10, t.as_str())
    };
    const SUFFIXES: [&str; 12] =
        ["usize", "isize", "u128", "i128", "u64", "i64", "u32", "i32", "u16", "i16", "u8", "i8"];
    let digits = SUFFIXES
        .iter()
        .find_map(|s| digits.strip_suffix(s))
        .unwrap_or(digits);
    if digits.is_empty() {
        return None;
    }
    i128::from_str_radix(digits, radix).ok()
}

/// Apply a binary integer operator by its Rust spelling.
pub fn apply_binop(op: &str, a: i128, b: i128) -> Option<i128> {
    match op {
        "+" => a.checked_add(b),
        "-" => a.checked_sub(b),
        "*" => a.checked_mul(b),
        "/" => a.checked_div(b),
        "%" => a.checked_rem(b),
        "<<" => u32::try_from(b).ok().and_then(|s| a.checked_shl(s)),
        ">>" => u32::try_from(b).ok().and_then(|s| a.checked_shr(s)),
        "&" => Some(a & b),
        "|" => Some(a | b),
        "^" => Some(a ^ b),
        _ => None,
    }
}

/// Evaluate an integer constant expression; `env` resolves a bare name.
pub fn eval_syn(expr: &syn::Expr, env: &dyn Fn(&str) -> Option<i128>) -> Option<i128> {
    use syn::{BinOp, Expr, UnOp};
    match expr {
        Expr::Lit(l) => match &l.lit {
            syn::Lit::Int(i) => parse_int_lit(&i.to_string()),
            syn::Lit::Bool(b) => Some(b.value as i128),
            _ => None,
        },
        Expr::Path(p) => env(&p.path.get_ident()?.to_string()),
        Expr::Paren(p) => eval_syn(&p.expr, env),
        Expr::Group(g) => eval_syn(&g.expr, env),
        Expr::Unary(u) => {
            let v = eval_syn(&u.expr, env)?;
            match u.op {
                UnOp::Neg(_) => v.checked_neg(),
                _ => None,
            }
        }
        Expr::Binary(b) => {
            let op = match b.op {
                BinOp::Add(_) => "+",
                BinOp::Sub(_) => "-",
                BinOp::Mul(_) => "*",
                BinOp::Div(_) => "/",
                BinOp::Rem(_) => "%",
                BinOp::Shl(_) => "<<",
                BinOp::Shr(_) => ">>",
                BinOp::BitAnd(_) => "&",
                BinOp::BitOr(_) => "|",
                BinOp::BitXor(_) => "^",
                _ => return None,
            };
            apply_binop(op, eval_syn(&b.left, env)?, eval_syn(&b.right, env)?)
        }
        _ => None,
    }
}

/// The values of a list of `(name, initializer text)` integer consts, each
/// evaluated over the ones before it and over each other in any order (rustc
/// resolves consts by name, not position). A const whose body is outside the
/// grammar is simply absent.
pub fn eval_const_items<'a>(items: impl IntoIterator<Item = (&'a str, &'a str)>) -> HashMap<String, i128> {
    let parsed: Vec<(String, syn::Expr)> = items
        .into_iter()
        .filter_map(|(n, t)| Some((n.to_string(), syn::parse_str::<syn::Expr>(t).ok()?)))
        .collect();
    let mut env: HashMap<String, i128> = HashMap::new();
    // A fixpoint over the list: at most `len` passes, each resolving at least one
    // const or stopping.
    loop {
        let mut progressed = false;
        for (name, e) in &parsed {
            if env.contains_key(name) {
                continue;
            }
            if let Some(v) = eval_syn(e, &|n| env.get(n).copied()) {
                env.insert(name.clone(), v);
                progressed = true;
            }
        }
        if !progressed {
            return env;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn literals_in_every_base() {
        assert_eq!(parse_int_lit("434"), Some(434));
        assert_eq!(parse_int_lit("0xFF"), Some(255));
        assert_eq!(parse_int_lit("0xFF_u8"), Some(255));
        assert_eq!(parse_int_lit("1_000usize"), Some(1000));
        assert_eq!(parse_int_lit("0b1010"), Some(10));
        assert_eq!(parse_int_lit("0o17"), Some(15));
        assert_eq!(parse_int_lit("u8"), None);
    }

    #[test]
    fn consts_refer_to_each_other_in_any_order() {
        let env = eval_const_items([
            ("HALF", "CLKS_PER_BIT / 2"),
            ("CLKS_PER_BIT", "434"),
            ("BAD", "foo()"),
            ("DIV0", "1 / 0"),
            ("YES", "true"),
            ("START", "1 + 2"),
        ]);
        assert_eq!(env.get("YES"), Some(&1));
        assert_eq!(env.get("START"), Some(&3));
        assert_eq!(env.get("CLKS_PER_BIT"), Some(&434));
        assert_eq!(env.get("HALF"), Some(&217));
        assert!(!env.contains_key("BAD"));
        assert!(!env.contains_key("DIV0"));
    }
}

/// The constants of a file (SPEC I.14.4): its `const` items, the consts of its
/// `impl` blocks (`T::K`), and its `const fn`s, built from each item's source text.
/// Phase C step 5 (2026-11-07, II.3 (d), MEASUREMENTS §J.13). Evaluation is over
/// `i128`, checked, fuel-bounded; anything outside the grammar is `None`.
#[derive(Default, Clone)]
pub struct ConstEnv {
    values: HashMap<String, i128>,
    fns: HashMap<String, syn::ItemFn>,
}

/// Statement-level control flow of a `const fn` body.
enum Flow {
    Next,
    Value(i128),
    Return(i128),
}

const FUEL: usize = 100_000;

impl ConstEnv {
    /// From item source texts (`const K: T = e;`, `impl T { const K: U = e; … }`,
    /// `const fn f(..) -> T { … }`); any other item is ignored.
    pub fn from_items<'a>(items: impl IntoIterator<Item = &'a str>) -> Self {
        let mut consts: Vec<(String, syn::Expr, Option<String>)> = Vec::new();
        let mut env = ConstEnv::default();
        for text in items {
            let Ok(item) = syn::parse_str::<syn::Item>(text) else { continue };
            match item {
                syn::Item::Const(c) => consts.push((c.ident.to_string(), *c.expr, None)),
                syn::Item::Fn(f) if f.sig.constness.is_some() => {
                    env.fns.insert(f.sig.ident.to_string(), f);
                }
                syn::Item::Impl(imp) if imp.trait_.is_none() => {
                    let syn::Type::Path(tp) = &*imp.self_ty else { continue };
                    let Some(seg) = tp.path.segments.last() else { continue };
                    let ty = seg.ident.to_string();
                    for it in imp.items {
                        match it {
                            syn::ImplItem::Const(c) => {
                                consts.push((format!("{ty}::{}", c.ident), c.expr, Some(ty.clone())))
                            }
                            syn::ImplItem::Fn(f) if f.sig.constness.is_some() => {
                                let item_fn = syn::ItemFn {
                                    attrs: f.attrs,
                                    vis: f.vis,
                                    sig: f.sig,
                                    block: Box::new(f.block),
                                };
                                env.fns.insert(format!("{ty}::{}", item_fn.sig.ident), item_fn);
                            }
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
        }
        // Consts may refer to each other in any order: a fixpoint.
        loop {
            let mut progressed = false;
            for (name, e, self_ty) in &consts {
                if env.values.contains_key(name) {
                    continue;
                }
                let mut locals = HashMap::new();
                let mut fuel = FUEL;
                if let Some(v) = env.eval(e, &mut locals, self_ty.as_deref(), &mut fuel) {
                    env.values.insert(name.clone(), v);
                    progressed = true;
                }
            }
            if !progressed {
                return env;
            }
        }
    }

    /// The value of a const (`K` or `T::K`).
    pub fn get(&self, name: &str) -> Option<i128> {
        self.values.get(name).copied()
    }

    /// Whether `name` is a `const fn` of the file (`f` or `T::f`).
    pub fn is_const_fn(&self, name: &str) -> bool {
        self.fns.contains_key(name)
    }

    /// Call a `const fn` with integer arguments.
    pub fn call(&self, name: &str, args: &[i128]) -> Option<i128> {
        let mut fuel = FUEL;
        self.call_fuel(name, args, &mut fuel)
    }

    /// Every const by name, for callers that keep a plain map.
    pub fn values(&self) -> &HashMap<String, i128> {
        &self.values
    }

    /// Evaluate an expression over this environment, `locals` bound first (a
    /// module's const generics): consts, `T::K`, `const fn` calls, the grammar of
    /// [`Self::from_items`]. `None` outside it. (The B2 differential test's entry,
    /// D-21; the transpiler's own callers go through `get` / `call`.)
    pub fn eval_expr(&self, e: &syn::Expr, locals: &HashMap<String, i128>) -> Option<i128> {
        let mut locals = locals.clone();
        let mut fuel = FUEL;
        self.eval(e, &mut locals, None, &mut fuel)
    }

    fn call_fuel(&self, name: &str, args: &[i128], fuel: &mut usize) -> Option<i128> {
        let f = self.fns.get(name)?;
        if f.sig.inputs.len() != args.len() {
            return None;
        }
        let mut locals = HashMap::new();
        for (p, a) in f.sig.inputs.iter().zip(args) {
            let syn::FnArg::Typed(pt) = p else { return None };
            let syn::Pat::Ident(id) = &*pt.pat else { return None };
            locals.insert(id.ident.to_string(), *a);
        }
        let self_ty = name.rsplit_once("::").map(|(t, _)| t.to_string());
        match self.exec_block(&f.block.stmts, &mut locals, self_ty.as_deref(), fuel)? {
            Flow::Value(v) | Flow::Return(v) => Some(v),
            Flow::Next => None,
        }
    }

    fn exec_block(
        &self,
        stmts: &[syn::Stmt],
        locals: &mut HashMap<String, i128>,
        self_ty: Option<&str>,
        fuel: &mut usize,
    ) -> Option<Flow> {
        let mut last = Flow::Next;
        for (i, s) in stmts.iter().enumerate() {
            *fuel = fuel.checked_sub(1)?;
            last = Flow::Next;
            match s {
                syn::Stmt::Local(l) => {
                    let name = match &l.pat {
                        syn::Pat::Ident(id) => id.ident.to_string(),
                        syn::Pat::Type(pt) => match &*pt.pat {
                            syn::Pat::Ident(id) => id.ident.to_string(),
                            _ => return None,
                        },
                        _ => return None,
                    };
                    let init = l.init.as_ref()?;
                    let v = self.eval(&init.expr, locals, self_ty, fuel)?;
                    locals.insert(name, v);
                }
                syn::Stmt::Expr(e, semi) => match self.eval_flow(e, locals, self_ty, fuel)? {
                    Flow::Return(v) => return Some(Flow::Return(v)),
                    Flow::Value(v) if semi.is_none() && i + 1 == stmts.len() => last = Flow::Value(v),
                    _ => {}
                },
                _ => return None,
            }
        }
        Some(last)
    }

    /// Evaluate an expression that may be a statement (`while`, an assignment, a
    /// `return`); its value when it has one.
    fn eval_flow(
        &self,
        e: &syn::Expr,
        locals: &mut HashMap<String, i128>,
        self_ty: Option<&str>,
        fuel: &mut usize,
    ) -> Option<Flow> {
        use syn::{BinOp, Expr};
        match e {
            Expr::Return(r) => {
                let v = self.eval(r.expr.as_deref()?, locals, self_ty, fuel)?;
                Some(Flow::Return(v))
            }
            Expr::While(w) => {
                loop {
                    *fuel = fuel.checked_sub(1)?;
                    if self.eval(&w.cond, locals, self_ty, fuel)? == 0 {
                        return Some(Flow::Next);
                    }
                    if let Flow::Return(v) = self.exec_block(&w.body.stmts, locals, self_ty, fuel)? {
                        return Some(Flow::Return(v));
                    }
                }
            }
            Expr::Assign(a) => {
                let Expr::Path(p) = &*a.left else { return None };
                let name = p.path.get_ident()?.to_string();
                if !locals.contains_key(&name) {
                    return None;
                }
                let v = self.eval(&a.right, locals, self_ty, fuel)?;
                locals.insert(name, v);
                Some(Flow::Next)
            }
            Expr::Binary(b)
                if matches!(
                    b.op,
                    BinOp::AddAssign(_) | BinOp::SubAssign(_) | BinOp::MulAssign(_) | BinOp::DivAssign(_)
                        | BinOp::RemAssign(_) | BinOp::ShlAssign(_) | BinOp::ShrAssign(_)
                        | BinOp::BitAndAssign(_) | BinOp::BitOrAssign(_) | BinOp::BitXorAssign(_)
                ) =>
            {
                let Expr::Path(p) = &*b.left else { return None };
                let name = p.path.get_ident()?.to_string();
                let cur = *locals.get(&name)?;
                let r = self.eval(&b.right, locals, self_ty, fuel)?;
                let op = match b.op {
                    BinOp::AddAssign(_) => "+",
                    BinOp::SubAssign(_) => "-",
                    BinOp::MulAssign(_) => "*",
                    BinOp::DivAssign(_) => "/",
                    BinOp::RemAssign(_) => "%",
                    BinOp::ShlAssign(_) => "<<",
                    BinOp::ShrAssign(_) => ">>",
                    BinOp::BitAndAssign(_) => "&",
                    BinOp::BitOrAssign(_) => "|",
                    _ => "^",
                };
                locals.insert(name, apply_binop(op, cur, r)?);
                Some(Flow::Next)
            }
            Expr::If(i) => {
                let c = self.eval(&i.cond, locals, self_ty, fuel)?;
                if c != 0 {
                    self.exec_block(&i.then_branch.stmts, locals, self_ty, fuel)
                } else if let Some((_, eb)) = &i.else_branch {
                    self.eval_flow(eb, locals, self_ty, fuel)
                } else {
                    Some(Flow::Next)
                }
            }
            Expr::Block(b) => self.exec_block(&b.block.stmts, locals, self_ty, fuel),
            Expr::Match(m) => {
                let v = self.eval(&m.expr, locals, self_ty, fuel)?;
                for arm in &m.arms {
                    if arm.guard.is_some() {
                        return None;
                    }
                    if pat_matches(&arm.pat, v)? {
                        return self.eval_flow(&arm.body, locals, self_ty, fuel);
                    }
                }
                None
            }
            _ => self.eval(e, locals, self_ty, fuel).map(Flow::Value),
        }
    }

    fn eval(
        &self,
        e: &syn::Expr,
        locals: &mut HashMap<String, i128>,
        self_ty: Option<&str>,
        fuel: &mut usize,
    ) -> Option<i128> {
        use syn::{BinOp, Expr, UnOp};
        *fuel = fuel.checked_sub(1)?;
        match e {
            Expr::Lit(l) => match &l.lit {
                syn::Lit::Int(i) => parse_int_lit(&i.to_string()),
                syn::Lit::Bool(b) => Some(b.value as i128),
                _ => None,
            },
            Expr::Path(p) => {
                let segs: Vec<String> = p.path.segments.iter().map(|s| s.ident.to_string()).collect();
                if segs.len() == 1 {
                    if let Some(v) = locals.get(&segs[0]) {
                        return Some(*v);
                    }
                    return self.values.get(&segs[0]).copied();
                }
                let mut name = segs.join("::");
                if segs[0] == "Self" {
                    name = format!("{}::{}", self_ty?, segs[1..].join("::"));
                }
                self.values.get(&name).copied()
            }
            Expr::Paren(p) => self.eval(&p.expr, locals, self_ty, fuel),
            Expr::Group(g) => self.eval(&g.expr, locals, self_ty, fuel),
            Expr::Unary(u) => {
                let v = self.eval(&u.expr, locals, self_ty, fuel)?;
                match u.op {
                    UnOp::Neg(_) => v.checked_neg(),
                    UnOp::Not(_) if v == 0 || v == 1 => Some(1 - v), // a `bool`'s `!`
                    _ => None,
                }
            }
            Expr::Binary(b) => {
                let op = match b.op {
                    BinOp::And(_) => {
                        return Some(
                            (self.eval(&b.left, locals, self_ty, fuel)? != 0
                                && self.eval(&b.right, locals, self_ty, fuel)? != 0) as i128,
                        )
                    }
                    BinOp::Or(_) => {
                        return Some(
                            (self.eval(&b.left, locals, self_ty, fuel)? != 0
                                || self.eval(&b.right, locals, self_ty, fuel)? != 0) as i128,
                        )
                    }
                    BinOp::Add(_) => "+",
                    BinOp::Sub(_) => "-",
                    BinOp::Mul(_) => "*",
                    BinOp::Div(_) => "/",
                    BinOp::Rem(_) => "%",
                    BinOp::Shl(_) => "<<",
                    BinOp::Shr(_) => ">>",
                    BinOp::BitAnd(_) => "&",
                    BinOp::BitOr(_) => "|",
                    BinOp::BitXor(_) => "^",
                    BinOp::Eq(_) | BinOp::Ne(_) | BinOp::Lt(_) | BinOp::Le(_) | BinOp::Gt(_) | BinOp::Ge(_) => {
                        let l = self.eval(&b.left, locals, self_ty, fuel)?;
                        let r = self.eval(&b.right, locals, self_ty, fuel)?;
                        return Some(match b.op {
                            BinOp::Eq(_) => l == r,
                            BinOp::Ne(_) => l != r,
                            BinOp::Lt(_) => l < r,
                            BinOp::Le(_) => l <= r,
                            BinOp::Gt(_) => l > r,
                            _ => l >= r,
                        } as i128);
                    }
                    _ => return None,
                };
                apply_binop(
                    op,
                    self.eval(&b.left, locals, self_ty, fuel)?,
                    self.eval(&b.right, locals, self_ty, fuel)?,
                )
            }
            // An integer cast keeps a value that fits an unsigned 32-bit or a
            // non-negative one; anything a truncation would change is unknown.
            Expr::Cast(c) => {
                let v = self.eval(&c.expr, locals, self_ty, fuel)?;
                (0..=u32::MAX as i128).contains(&v).then_some(v)
            }
            Expr::Call(c) => {
                let Expr::Path(p) = &*c.func else { return None };
                let segs: Vec<String> = p.path.segments.iter().map(|s| s.ident.to_string()).collect();
                let mut name = segs.join("::");
                if segs.first().is_some_and(|s| s == "Self") {
                    name = format!("{}::{}", self_ty?, segs[1..].join("::"));
                }
                let args: Vec<i128> =
                    c.args.iter().map(|a| self.eval(a, locals, self_ty, fuel)).collect::<Option<_>>()?;
                self.call_fuel(&name, &args, fuel)
            }
            Expr::If(_) | Expr::Block(_) | Expr::Match(_) => {
                let mut inner = locals.clone();
                match self.eval_flow(e, &mut inner, self_ty, fuel)? {
                    Flow::Value(v) => Some(v),
                    _ => None,
                }
            }
            _ => None,
        }
    }
}

/// A `match` arm pattern over an integer: a literal, `a | b`, `_`, an inclusive or
/// exclusive literal range. `None` for any other pattern.
fn pat_matches(p: &syn::Pat, v: i128) -> Option<bool> {
    match p {
        syn::Pat::Wild(_) => Some(true),
        syn::Pat::Lit(l) => match &l.lit {
            syn::Lit::Int(i) => Some(parse_int_lit(&i.to_string())? == v),
            syn::Lit::Bool(b) => Some(b.value as i128 == v),
            _ => None,
        },
        syn::Pat::Or(o) => {
            for c in &o.cases {
                if pat_matches(c, v)? {
                    return Some(true);
                }
            }
            Some(false)
        }
        _ => None,
    }
}

#[cfg(test)]
mod env_tests {
    use super::*;

    #[test]
    fn const_fns_assoc_consts_and_the_corpus_clog2() {
        let env = ConstEnv::from_items([
            "const fn safe_clog2(n: usize) -> usize { match n { 0 | 1 => 0, _ => { let mut bits = 0; let mut v = n - 1; while v > 0 { v >>= 1; bits += 1; } bits } } }",
            "struct Cfg;",
            "impl Cfg { const DEPTH: usize = 12; const W: usize = safe_clog2(Self::DEPTH); }",
            "const TWICE: usize = Cfg::DEPTH * 2;",
            "const fn pick(c: bool, a: u32, b: u32) -> u32 { if c { return a; } b }",
            "fn not_const(x: u32) -> u32 { x }",
        ]);
        assert_eq!(env.call("safe_clog2", &[1]), Some(0));
        assert_eq!(env.call("safe_clog2", &[8]), Some(3));
        assert_eq!(env.call("safe_clog2", &[9]), Some(4));
        assert_eq!(env.get("Cfg::DEPTH"), Some(12));
        assert_eq!(env.get("Cfg::W"), Some(4));
        assert_eq!(env.get("TWICE"), Some(24));
        assert_eq!(env.call("pick", &[1, 5, 7]), Some(5));
        assert_eq!(env.call("pick", &[0, 5, 7]), Some(7));
        assert!(!env.is_const_fn("not_const"));
        assert_eq!(env.call("not_const", &[1]), None);
    }
}
