//! The resumable walker: one resumption of the kernel coroutine, from a `tick`
//! site to the next, over `syn` statements with their Rust meaning (SPEC §I.2.4's
//! classification; I.8 for loops and control flow).
//!
//! # Suspension and resumption
//!
//! A [`TickSite`] is the index path of a `clk.tick().await` statement: at each
//! nesting level the statement index within its block, with one extra segment for
//! the branch of an `if` (0 then, 1 else), the arm of a `match`, or the body of a
//! loop (always 0). `exec_block` walks a block either from its start or
//! *resuming* at a path: statements before the path's index are skipped (they ran
//! in the resumption that parked here), the statement at the index is resumed with
//! the rest of the path, and the statements after it run normally. A path of
//! length one names the tick itself, which on resumption is simply stepped over.
//!
//! # Scopes
//!
//! The environment is a stack of scopes indexed by **block depth**, not by call
//! stack: a block at depth `d` owns `scopes[d]`. Entering a block pushes its scope
//! (truncating anything deeper left by an earlier sibling); leaving it pops; a
//! suspension leaves the stack as it is, so the locals of every enclosing block
//! survive the park, and resuming re-descends without pushing. That is the state
//! of I.0.1 — the locals at the suspension site — kept literally.

use crate::bits::RefBits;
use crate::types::{parse_ty, ConstEval};
use crate::value::{IntTy, Ty, Value};
use crate::RefError;
use copper_core::Logic;
use std::cell::RefCell;
use crate::memory::{MemState, Mode};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use syn::{BinOp, Expr, Pat, Stmt, UnOp};

/// The index path of a `tick` statement — the suspension site (I.0.1).
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct TickSite(pub Vec<usize>);

impl std::fmt::Display for TickSite {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s: Vec<String> = self.0.iter().map(|i| i.to_string()).collect();
        write!(f, "[{}]", s.join("."))
    }
}

/// How a statement list ended.
#[derive(Debug, Clone, PartialEq)]
pub enum Flow {
    Done,
    Break,
    Continue,
    /// Parked at a `tick`; the path is built on the way out, innermost first.
    Suspend(Vec<usize>),
}

/// The static part of a module: everything the walker needs that does not change
/// between resumptions.
pub struct Program {
    /// `const` items (file scope and body), by name: the declared type and the
    /// initializer, evaluated lazily into `const_vals`.
    pub consts: HashMap<String, (Option<syn::Type>, Expr)>,
    pub const_vals: RefCell<HashMap<String, Value>>,
    /// Enum name → unit variant names.
    pub enums: HashMap<String, Vec<String>>,
    /// Struct name → field names (declaration order).
    pub structs: HashMap<String, Vec<String>>,
    /// Struct name → each named field's declared type (declaration order): a bare literal
    /// field (`S { y: 1 }`) takes it, as rustc types it (2026-11-08, MEASUREMENTS §Z.6).
    pub struct_tys: HashMap<String, Vec<syn::Type>>,
    /// Tuple structs, by name: each field's declared type. A call of the name is the
    /// constructor (I.14.4: rustc admits it in a constant; 2026-11-08, MEASUREMENTS §V.4).
    pub tuple_structs: HashMap<String, Vec<syn::Type>>,
    /// Helper free functions, by name.
    pub fns: HashMap<String, syn::ItemFn>,
    /// `impl` methods, by (type name, method name).
    pub methods: HashMap<(String, String), syn::ImplItemFn>,
    /// Const-generic bindings.
    pub generics: HashMap<String, u64>,
    pub clocks: Vec<String>,
    pub inputs: HashMap<String, Ty>,
    pub outputs: Vec<(String, Ty)>,
}

/// The run-time environment: scopes by block depth.
#[derive(Default, Debug, Clone)]
pub struct Env {
    pub scopes: Vec<HashMap<String, Value>>,
}

impl Env {
    pub fn get(&self, name: &str) -> Option<&Value> {
        self.scopes.iter().rev().find_map(|s| s.get(name))
    }
    pub fn declare(&mut self, name: &str, v: Value) {
        self.scopes
            .last_mut()
            .expect("a scope to declare into")
            .insert(name.to_string(), v);
    }
    pub fn set(&mut self, name: &str, v: Value) -> Result<(), RefError> {
        for s in self.scopes.iter_mut().rev() {
            if let Some(slot) = s.get_mut(name) {
                *slot = v;
                return Ok(());
            }
        }
        Err(RefError::Runtime(format!("assignment to an undeclared local `{name}`")))
    }
    fn enter(&mut self, depth: usize) {
        self.scopes.truncate(depth);
        self.scopes.push(HashMap::new());
    }
    fn leave(&mut self, depth: usize) {
        self.scopes.truncate(depth);
    }
    /// The poison run: overwrite every named local, wherever it lives.
    pub fn poison(&mut self, targets: &HashSet<String>) {
        for s in &mut self.scopes {
            for (k, v) in s.iter_mut() {
                if targets.contains(k) {
                    *v = Value::Poison(k.clone());
                }
            }
        }
    }
    /// Every local name currently alive (the hidden loop counters excluded).
    pub fn live_names(&self) -> BTreeSet<String> {
        self.scopes
            .iter()
            .flat_map(|s| s.keys())
            .filter(|k| !k.starts_with("__for@"))
            .cloned()
            .collect()
    }
}

/// One resumption's execution context.
pub struct Exec<'a> {
    pub prog: &'a Program,
    pub env: &'a mut Env,
    pub inputs: &'a HashMap<String, Value>,
    pub pending: &'a mut HashMap<String, Value>,
    /// The module's memories (SPEC I.10.1(a): variables of H(M) of their own
    /// kind, beside the environment), by binding name; `None` in a constant
    /// context, which has none.
    pub mems: Option<&'a mut BTreeMap<String, MemState>>,
    /// Helper-function call stack, for the recursion refusal (R6).
    call_stack: Vec<String>,
    /// The `impl` type of each associated fn being called: what `Self` names.
    self_ty: Vec<String>,
}

fn unsup(s: impl Into<String>) -> RefError {
    RefError::Unsupported(s.into())
}
fn rt(s: impl Into<String>) -> RefError {
    RefError::Runtime(s.into())
}

fn path_name(p: &syn::Path) -> String {
    p.segments.iter().map(|s| s.ident.to_string()).collect::<Vec<_>>().join("::")
}

/// The two-segment `Type::member` form of a path, if it has exactly two segments.
fn path2(p: &syn::Path) -> Option<(String, String)> {
    if p.segments.len() == 2 {
        Some((p.segments[0].ident.to_string(), p.segments[1].ident.to_string()))
    } else {
        None
    }
}

/// The const-generic arguments of one path segment (`Bits::<8>` → [8]).
fn seg_const_args(seg: &syn::PathSegment) -> Vec<&syn::GenericArgument> {
    match &seg.arguments {
        syn::PathArguments::AngleBracketed(a) => a.args.iter().collect(),
        _ => Vec::new(),
    }
}

impl<'a> ConstEval for Exec<'a> {
    fn eval_usize(&self, e: &Expr) -> Result<usize, RefError> {
        // Width expressions are constants: evaluate them against the consts and
        // generics only (a local cannot name a width).
        let mut env = Env::default();
        env.enter(0);
        let mut pending = HashMap::new();
        let inputs = HashMap::new();
        let mut ex = Exec { prog: self.prog, env: &mut env, inputs: &inputs, pending: &mut pending, mems: None, call_stack: Vec::new(), self_ty: Vec::new() };
        let v = ex.eval_expr(e)?;
        v.as_index()
    }
}

impl<'a> Exec<'a> {
    pub fn new(
        prog: &'a Program,
        env: &'a mut Env,
        inputs: &'a HashMap<String, Value>,
        pending: &'a mut HashMap<String, Value>,
    ) -> Self {
        Exec { prog, env, inputs, pending, mems: None, call_stack: Vec::new(), self_ty: Vec::new() }
    }

    /// Give the resumption the module's memories (I.10.1(a)).
    pub fn with_mems(mut self, mems: &'a mut BTreeMap<String, MemState>) -> Self {
        self.mems = Some(mems);
        self
    }

    /// A const-evaluation context with no locals, for `Module::new`.
    pub fn eval_const(prog: &Program, e: &Expr) -> Result<Value, RefError> {
        let mut env = Env::default();
        env.enter(0);
        let mut pending = HashMap::new();
        let inputs = HashMap::new();
        let mut ex = Exec::new(prog, &mut env, &inputs, &mut pending);
        ex.eval_expr(e)
    }

    pub fn parse_ty(&self, t: &syn::Type) -> Result<Ty, RefError> {
        parse_ty(t, self)
    }

    // ── statements ─────────────────────────────────────────────────────────

    /// Run `stmts` as the block at `depth`, from the start or resuming at `resume`.
    /// `bindings` are declared into the block's fresh scope (match-arm patterns,
    /// loop variables); ignored on resumption, whose scope is still live.
    pub fn exec_block(
        &mut self,
        stmts: &[Stmt],
        depth: usize,
        resume: Option<&[usize]>,
        bindings: Vec<(String, Value)>,
    ) -> Result<Flow, RefError> {
        let (start, rest) = match resume {
            Some(path) => {
                let Some((&i, rest)) = path.split_first() else {
                    return Err(rt("resume path ended at a block"));
                };
                (i, Some(rest))
            }
            None => {
                self.env.enter(depth);
                for (n, v) in bindings {
                    self.env.declare(&n, v);
                }
                (0, None)
            }
        };
        if resume.is_some() && start >= stmts.len() {
            return Err(rt(format!("resume index {start} past the end of a {}-statement block", stmts.len())));
        }
        let mut resume_rest = rest;
        for (i, stmt) in stmts.iter().enumerate().skip(start) {
            let r = resume_rest.take();
            let flow = self.exec_stmt(stmt, depth, r)?;
            match flow {
                Flow::Done => {}
                Flow::Suspend(mut p) => {
                    p.insert(0, i);
                    return Ok(Flow::Suspend(p));
                }
                other => {
                    self.env.leave(depth);
                    return Ok(other);
                }
            }
        }
        self.env.leave(depth);
        Ok(Flow::Done)
    }

    fn exec_stmt(&mut self, stmt: &Stmt, depth: usize, resume: Option<&[usize]>) -> Result<Flow, RefError> {
        match stmt {
            Stmt::Local(l) => {
                if resume.is_some() {
                    return Err(rt("resume path into a `let`"));
                }
                self.exec_let(l)?;
                Ok(Flow::Done)
            }
            // Items were collected by `Module::new`; a body-local `const`, `enum`,
            // `struct` or `fn` has no run-time effect here (I.2.4: DESUGARED).
            Stmt::Item(_) => Ok(Flow::Done),
            Stmt::Macro(m) => {
                let name = path_name(&m.mac.path);
                Err(unsup(format!(
                    "statement macro `{name}!` (II.1 R13: the macro admits it with unrewritten reads, the transpiler refuses it; a read inside macro tokens is R14)"
                )))
            }
            Stmt::Expr(e, _) => self.exec_stmt_expr(e, depth, resume),
        }
    }

    fn exec_let(&mut self, l: &syn::Local) -> Result<(), RefError> {
        let Some(init) = &l.init else {
            // `let x;` with assignments later: Rust forbids a read before the
            // first assignment, so the placeholder is never read.
            let (pat, _) = split_pat_ty(&l.pat);
            if let Some(name) = pat_ident(pat) {
                self.env.declare(&name, Value::Poison(format!("<uninitialized {name}>")));
                return Ok(());
            }
            return Err(unsup("`let` with a pattern and no initializer"));
        };
        if init.diverge.is_some() {
            return Err(unsup("`let … else` (I.2.5: OUTSIDE)"));
        }
        if memory_ctor(&init.expr).is_some() {
            return self.exec_memory_binding(l, &init.expr);
        }
        let mut v = self.eval_expr(&init.expr)?;
        let (pat, ty) = split_pat_ty(&l.pat);
        if let Some(ty) = ty {
            let t = self.parse_ty(ty)?;
            v = v.coerce_to(&t)?;
        }
        self.bind_pat(pat, v)
    }

    /// An expression in statement position: the resumable forms (`if`, `match`,
    /// loops, the tick) and the effectful ones (assignment, `write`).
    fn exec_stmt_expr(&mut self, e: &Expr, depth: usize, resume: Option<&[usize]>) -> Result<Flow, RefError> {
        match e {
            Expr::Await(a) => {
                self.check_tick(&a.base)?;
                match resume {
                    // Resumed past the tick: the edge has happened (I.0.2).
                    Some([]) => Ok(Flow::Done),
                    Some(p) => Err(rt(format!("resume path {p:?} descends into a tick"))),
                    None => Ok(Flow::Suspend(Vec::new())),
                }
            }
            Expr::If(i) => self.exec_if(i, depth, resume),
            Expr::Match(m) => self.exec_match(m, depth, resume),
            Expr::Loop(l) => {
                if l.label.is_some() {
                    return Err(unsup("a labelled loop (II.1 R12: labelled `break` has no meaning stated)"));
                }
                let mut r = resume.map(|p| strip_zero(p)).transpose()?;
                loop {
                    let flow = self.exec_block(&l.body.stmts, depth + 1, r.take(), Vec::new())?;
                    match flow {
                        Flow::Done | Flow::Continue => {}
                        Flow::Break => return Ok(Flow::Done),
                        Flow::Suspend(mut p) => {
                            p.insert(0, 0);
                            return Ok(Flow::Suspend(p));
                        }
                    }
                }
            }
            Expr::While(w) => {
                if w.label.is_some() {
                    return Err(unsup("a labelled loop (II.1 R12)"));
                }
                // `while c { s }` is `loop { if !c { break }; s }` (I.2.4): a test
                // false on entry runs zero iterations in zero time (I.8.4(a)).
                let mut r = resume.map(|p| strip_zero(p)).transpose()?;
                loop {
                    if r.is_none() && !self.eval_expr(&w.cond)?.as_bool()? {
                        return Ok(Flow::Done);
                    }
                    let flow = self.exec_block(&w.body.stmts, depth + 1, r.take(), Vec::new())?;
                    match flow {
                        Flow::Done | Flow::Continue => {}
                        Flow::Break => return Ok(Flow::Done),
                        Flow::Suspend(mut p) => {
                            p.insert(0, 0);
                            return Ok(Flow::Suspend(p));
                        }
                    }
                }
            }
            Expr::ForLoop(f) => self.exec_for(f, depth, resume),
            Expr::Block(b) => {
                if b.label.is_some() {
                    return Err(unsup("a labelled block"));
                }
                self.exec_block(&b.block.stmts, depth + 1, resume, Vec::new())
            }
            Expr::Break(b) => {
                if b.label.is_some() || b.expr.is_some() {
                    return Err(unsup("a labelled or valued `break` (II.1 R12)"));
                }
                Ok(Flow::Break)
            }
            Expr::Continue(c) => {
                if c.label.is_some() {
                    return Err(unsup("a labelled `continue` (II.1 R12)"));
                }
                Ok(Flow::Continue)
            }
            Expr::Return(_) => Err(unsup("`return` in a module body (I.2.5(d): OUTSIDE)")),
            Expr::Const(_) => Ok(Flow::Done), // `const { assert!(…) }`: ERASED
            Expr::Assign(a) => {
                let v = self.eval_expr(&a.right)?;
                self.assign_place(&a.left, v)?;
                Ok(Flow::Done)
            }
            Expr::Binary(b) if is_compound_assign(&b.op) => {
                let cur = self.eval_expr(&b.left)?;
                let rhs = self.eval_expr(&b.right)?;
                let v = self.binop(&strip_assign(&b.op), cur, rhs)?;
                self.assign_place(&b.left, v)?;
                Ok(Flow::Done)
            }
            Expr::MethodCall(mc) if mc.method == "write" => {
                self.exec_write(mc)?;
                Ok(Flow::Done)
            }
            Expr::MethodCall(mc) if mc.method == "set" && mc.args.len() == 2 => {
                // `b.set(i, l)`: a mutation of the receiver place (Bits::set).
                let i = self.eval_expr(&mc.args[0])?.as_index()?;
                let l = self.eval_expr(&mc.args[1])?.as_logic()?;
                let mut cur = self.eval_expr(&mc.receiver)?.as_bits()?;
                if i >= cur.width as usize {
                    return Err(rt(format!("Bits::set index {i} out of bounds for width {}", cur.width)));
                }
                cur.set(i, l);
                self.assign_place(&mc.receiver, Value::Bits(cur))?;
                Ok(Flow::Done)
            }
            Expr::Macro(m) => {
                let name = path_name(&m.mac.path);
                if name == "matches" {
                    let _ = self.eval_expr(e)?;
                    return Ok(Flow::Done);
                }
                Err(unsup(format!("statement macro `{name}!` (II.1 R13/R14)")))
            }
            other => {
                let _ = self.eval_expr(other)?;
                Ok(Flow::Done)
            }
        }
    }

    fn check_tick(&self, base: &Expr) -> Result<(), RefError> {
        if let Expr::MethodCall(mc) = base {
            if mc.method == "tick" && mc.args.is_empty() {
                if let Expr::Path(p) = &*mc.receiver {
                    let n = path_name(&p.path);
                    if self.prog.clocks.iter().any(|c| *c == n) {
                        return Ok(());
                    }
                }
            }
        }
        Err(unsup("an `await` that is not `clk.tick().await` (II.1 R1: the only suspension is the cycle boundary)"))
    }

    fn exec_if(&mut self, i: &syn::ExprIf, depth: usize, resume: Option<&[usize]>) -> Result<Flow, RefError> {
        let (branch, rest) = match resume {
            Some([b, rest @ ..]) => (*b, Some(rest)),
            Some([]) => return Err(rt("resume path ended at an `if`")),
            None => {
                if let Expr::Let(_) = &*i.cond {
                    return Err(unsup("`if let` (I.2.5: OUTSIDE)"));
                }
                let c = self.eval_expr(&i.cond)?.as_bool()?;
                (if c { 0 } else { 1 }, None)
            }
        };
        let flow = if branch == 0 {
            self.exec_block(&i.then_branch.stmts, depth + 1, rest, Vec::new())?
        } else {
            match &i.else_branch {
                None => Flow::Done,
                Some((_, e)) => match &**e {
                    Expr::Block(b) => self.exec_block(&b.block.stmts, depth + 1, rest, Vec::new())?,
                    Expr::If(inner) => self.exec_if(inner, depth, rest)?,
                    other => return Err(rt(format!("unexpected else form {}", expr_kind(other)))),
                },
            }
        };
        Ok(match flow {
            Flow::Suspend(mut p) => {
                p.insert(0, branch);
                Flow::Suspend(p)
            }
            f => f,
        })
    }

    fn exec_match(&mut self, m: &syn::ExprMatch, depth: usize, resume: Option<&[usize]>) -> Result<Flow, RefError> {
        let (arm_idx, rest, bindings) = match resume {
            Some([a, rest @ ..]) => (*a, Some(rest), Vec::new()),
            Some([]) => return Err(rt("resume path ended at a `match`")),
            None => {
                let scrutinee = self.eval_expr(&m.expr)?;
                let mut chosen = None;
                for (k, arm) in m.arms.iter().enumerate() {
                    let mut b = Vec::new();
                    if self.match_pat(&arm.pat, &scrutinee, &mut b)? {
                        if let Some((_, guard)) = &arm.guard {
                            if !self.eval_with_bindings(guard, &b)?.as_bool()? {
                                continue;
                            }
                        }
                        chosen = Some((k, b));
                        break;
                    }
                }
                let Some((k, b)) = chosen else {
                    return Err(rt(format!("no match arm matched {scrutinee}")));
                };
                (k, None, b)
            }
        };
        let arm = m
            .arms
            .get(arm_idx)
            .ok_or_else(|| rt(format!("resume into arm {arm_idx} of a {}-arm match", m.arms.len())))?;
        let flow = self.exec_arm_body(&arm.body, depth, rest, bindings)?;
        Ok(match flow {
            Flow::Suspend(mut p) => {
                p.insert(0, arm_idx);
                Flow::Suspend(p)
            }
            f => f,
        })
    }

    /// A match arm's body: a block, or a single expression treated as a
    /// one-statement block (path segment 0).
    fn exec_arm_body(
        &mut self,
        body: &Expr,
        depth: usize,
        resume: Option<&[usize]>,
        bindings: Vec<(String, Value)>,
    ) -> Result<Flow, RefError> {
        match body {
            Expr::Block(b) => self.exec_block(&b.block.stmts, depth + 1, resume, bindings),
            other => {
                let rest = match resume {
                    Some([0, rest @ ..]) => Some(rest),
                    Some(p) => return Err(rt(format!("resume path {p:?} into a one-expression arm"))),
                    None => {
                        self.env.enter(depth + 1);
                        for (n, v) in bindings {
                            self.env.declare(&n, v);
                        }
                        None
                    }
                };
                let flow = self.exec_stmt_expr(other, depth + 1, rest)?;
                Ok(match flow {
                    Flow::Suspend(mut p) => {
                        p.insert(0, 0);
                        Flow::Suspend(p)
                    }
                    f => {
                        self.env.leave(depth + 1);
                        f
                    }
                })
            }
        }
    }

    /// `for v in a..b { s }` with I.8.5's desugaring: `let mut c = a; let e = b;
    /// loop { if c >= e { break }; let v = c; c = c + 1; s }`. The counter and the
    /// bound live as hidden locals of the enclosing block (`__for@<depth>`), so
    /// they survive a tick inside the body; `v` is a local of the body's scope.
    ///
    /// `for v in a..=b` (D-27; D-28, 2026-11-09): I.8.5's inclusive form, tick-free or
    /// tick-bearing alike — `let mut d = c > e; loop { if d { break }; let v = c;
    /// if c == e { d = true } else { c = c + 1 }; s }`. The counter is never incremented
    /// past `e`, so an end at its type's maximum — constant or reached only at run time —
    /// runs its last iteration and stops, as Rust's `RangeInclusive` does. The done flag
    /// is a hidden local beside the counter (`__for@<depth>d`), so it survives a tick.
    /// Until D-28 a tick-bearing one incremented past `e` and failed there (line `II.3
    /// found inclusive-for-runtime-max`, MEASUREMENTS §AA.5, §AB.1).
    ///
    /// The loop variable has the range's type (I.14.1, line `II.3 found loop-var-type`):
    /// a typed bound's, else an unsuffixed integer, which takes the type of the operand
    /// it meets — rustc's inference from the use. Until 2026-11-09 two unsuffixed bounds
    /// made it `usize` ("mixed integer types u8 and usize" at `a.as_u8() == i`).
    fn exec_for(&mut self, f: &syn::ExprForLoop, depth: usize, resume: Option<&[usize]>) -> Result<Flow, RefError> {
        if f.label.is_some() {
            return Err(unsup("a labelled loop (II.1 R12)"));
        }
        let Expr::Range(r) = &*f.expr else {
            return Err(unsup("a `for` over something other than a range (I.2.4: the counted loop is `a..b`)"));
        };
        let inclusive = matches!(r.limits, syn::RangeLimits::Closed(_));
        let (Some(lo), Some(hi)) = (&r.start, &r.end) else {
            return Err(unsup("a half-open range in a `for`"));
        };
        let var = match &*f.pat {
            Pat::Wild(_) => None,
            Pat::Ident(pi) => Some(pi.ident.to_string()),
            _ => return Err(unsup("a `for` pattern that is not an identifier")),
        };
        let ckey = format!("__for@{depth}c");
        let ekey = format!("__for@{depth}e");
        let dkey = format!("__for@{depth}d");
        let mut r = match resume {
            Some(p) => Some(strip_zero(p)?),
            None => {
                let a = self.eval_expr(lo)?;
                let b = self.eval_expr(hi)?;
                let (a, b) = unify_ints(a, b)?;
                let empty = if inclusive { a.as_int()? > b.as_int()? } else { a.as_int()? >= b.as_int()? };
                self.env.declare(&ckey, a);
                self.env.declare(&ekey, b);
                self.env.declare(&dkey, Value::Bool(empty));
                None
            }
        };
        loop {
            let bindings = if r.is_none() {
                let c = self.env.get(&ckey).cloned().ok_or_else(|| rt("lost for-counter"))?;
                let e = self.env.get(&ekey).cloned().ok_or_else(|| rt("lost for-bound"))?;
                let (ci, ei) = (c.as_int()?, e.as_int()?);
                let done = matches!(self.env.get(&dkey), Some(Value::Bool(true)));
                if done || (!inclusive && ci >= ei) {
                    return Ok(Flow::Done);
                }
                if inclusive && ci == ei {
                    self.env.set(&dkey, Value::Bool(true))?;
                } else {
                    let next = self.binop(&BinOp::Add(Default::default()), c.clone(), Value::Int(1, IntTy::Unsuffixed))?;
                    self.env.set(&ckey, next)?;
                }
                var.iter().map(|v| (v.clone(), c.clone())).collect()
            } else {
                Vec::new()
            };
            let flow = self.exec_block(&f.body.stmts, depth + 1, r.take(), bindings)?;
            match flow {
                Flow::Done | Flow::Continue => {}
                Flow::Break => return Ok(Flow::Done),
                Flow::Suspend(mut p) => {
                    p.insert(0, 0);
                    return Ok(Flow::Suspend(p));
                }
            }
        }
    }

    fn exec_write(&mut self, mc: &syn::ExprMethodCall) -> Result<(), RefError> {
        if self.memory_call(mc)?.is_some() {
            return Ok(());
        }
        let Expr::Path(p) = &*mc.receiver else {
            return Err(unsup("`.write()` on something other than a port parameter"));
        };
        let name = path_name(&p.path);
        let Some((_, ty)) = self.prog.outputs.iter().find(|(n, _)| *n == name) else {
            if self.env.get(&name).is_some() {
                return Err(unsup(format!("`.write()` on local `{name}`")));
            }
            return Err(unsup(format!("`{name}.write()`: `{name}` is not a `RegOut` port of this module")));
        };
        if mc.args.len() != 1 {
            return Err(rt("`write` takes one argument"));
        }
        let v = self.eval_expr(&mc.args[0])?;
        v.check_poison()?;
        let v = v.coerce_to(&ty.clone())?;
        // Last write before the edge wins (I.6.2).
        self.pending.insert(name, v);
        Ok(())
    }

    // ── places and patterns ────────────────────────────────────────────────

    fn assign_place(&mut self, place: &Expr, v: Value) -> Result<(), RefError> {
        // `x.as_array_mut()[i] = l` is the element write `x[i] = l` (I.14.3(d); Phase C
        // step 8): the `&mut` view names the vector itself.
        if let Expr::Index(ix) = place {
            if let Expr::MethodCall(mc) = &*ix.expr {
                if mc.method == "as_array_mut" && mc.args.is_empty() {
                    let mut ix2 = ix.clone();
                    ix2.expr = mc.receiver.clone();
                    return self.assign_place(&Expr::Index(ix2), v);
                }
            }
        }
        match place {
            Expr::Path(p) if p.path.segments.len() == 1 => {
                let name = p.path.segments[0].ident.to_string();
                let cur = self
                    .env
                    .get(&name)
                    .cloned()
                    .ok_or_else(|| rt(format!("assignment to an undeclared local `{name}`")))?;
                let v = match cur {
                    Value::Poison(_) => v,
                    ref c => v.size_like(c),
                };
                self.env.set(&name, v)
            }
            Expr::Index(ix) => {
                let base = self.eval_place(&ix.expr)?;
                let i = self.eval_expr(&ix.index)?.as_index()?;
                let updated = match base {
                    Value::Bits(mut b) => {
                        if i >= b.width as usize {
                            return Err(rt(format!("bit index {i} out of bounds for Bits<{}>", b.width)));
                        }
                        b.set(i, v.as_logic()?);
                        Value::Bits(b)
                    }
                    Value::Array(mut els) => {
                        if i >= els.len() {
                            return Err(rt(format!("index {i} out of bounds for an array of {}", els.len())));
                        }
                        let sized = v.size_like(&els[i]);
                        els[i] = sized;
                        Value::Array(els)
                    }
                    // A `Bits::zero()` whose width rustc infers from a later use
                    // (`let mut b = Bits::zero(); b[7] = …; o.write(b)`) is still an
                    // unsized literal here; it is sized at first contact (I.14.4),
                    // and a bit set before that is the same bit of the sized value.
                    // An index past the width it is later given is caught when it is
                    // sized (the literal no longer fits, I.14.1).
                    Value::Lit(n) => {
                        if i >= 128 {
                            return Err(rt(format!("bit index {i} out of bounds for an unsized Bits literal")));
                        }
                        match v.as_logic()? {
                            Logic::One => Value::Lit(n | (1u128 << i)),
                            Logic::Zero => Value::Lit(n & !(1u128 << i)),
                            Logic::X => {
                                return Err(unsup(
                                    "an X bit set in a Bits value whose width is not yet known (I.14.4: size it first)",
                                ))
                            }
                        }
                    }
                    other => return Err(rt(format!("index assignment into {other}"))),
                };
                self.assign_place(&ix.expr, updated)
            }
            Expr::Field(fe) => {
                let base = self.eval_place(&fe.base)?;
                let updated = match (base, &fe.member) {
                    (Value::Struct(n, mut fields), syn::Member::Named(id)) => {
                        let key = id.to_string();
                        let slot = fields
                            .iter_mut()
                            .find(|(k, _)| *k == key)
                            .ok_or_else(|| rt(format!("no field `{key}` on `{n}`")))?;
                        slot.1 = v.size_like(&slot.1);
                        Value::Struct(n, fields)
                    }
                    (Value::Tuple(mut els), syn::Member::Unnamed(ix)) => {
                        let i = ix.index as usize;
                        if i >= els.len() {
                            return Err(rt(format!("tuple index {i} out of bounds")));
                        }
                        els[i] = v.size_like(&els[i]);
                        Value::Tuple(els)
                    }
                    (other, _) => return Err(rt(format!("field assignment into {other}"))),
                };
                self.assign_place(&fe.base, updated)
            }
            Expr::Tuple(t) => {
                // Simultaneous: the right-hand side is already a value (I.2.4).
                let Value::Tuple(vs) = v else {
                    return Err(rt("tuple assignment from a non-tuple"));
                };
                if vs.len() != t.elems.len() {
                    return Err(rt("tuple assignment arity mismatch"));
                }
                for (e, v) in t.elems.iter().zip(vs) {
                    if let Expr::Infer(_) = e {
                        continue;
                    }
                    self.assign_place(e, v)?;
                }
                Ok(())
            }
            Expr::Paren(p) => self.assign_place(&p.expr, v),
            Expr::Unary(u) if matches!(u.op, UnOp::Deref(_)) => self.assign_place(&u.expr, v),
            other => Err(unsup(format!("assignment to a {} expression", expr_kind(other)))),
        }
    }

    /// The current value of a place, *without* the poison check: an element or
    /// field write reads the rest of the aggregate, which liveness also counts as
    /// a use, so a poisoned aggregate is reported through `eval_expr` on the base.
    fn eval_place(&mut self, e: &Expr) -> Result<Value, RefError> {
        self.eval_expr(e)
    }

    fn bind_pat(&mut self, pat: &Pat, v: Value) -> Result<(), RefError> {
        match pat {
            Pat::Ident(pi) => {
                if pi.subpat.is_some() {
                    return Err(unsup("an `@` pattern"));
                }
                self.env.declare(&pi.ident.to_string(), v);
                Ok(())
            }
            Pat::Wild(_) => Ok(()),
            Pat::Tuple(t) => {
                let Value::Tuple(vs) = v else {
                    return Err(rt("tuple pattern against a non-tuple"));
                };
                if vs.len() != t.elems.len() {
                    return Err(rt("tuple pattern arity mismatch"));
                }
                for (p, v) in t.elems.iter().zip(vs) {
                    self.bind_pat(p, v)?;
                }
                Ok(())
            }
            Pat::Type(pt) => {
                let t = self.parse_ty(&pt.ty)?;
                self.bind_pat(&pt.pat, v.coerce_to(&t)?)
            }
            Pat::Paren(p) => self.bind_pat(&p.pat, v),
            other => Err(unsup(format!("`let` pattern {}", pat_kind(other)))),
        }
    }

    /// Does `pat` match `v`? Bindings are collected, not declared.
    fn match_pat(&mut self, pat: &Pat, v: &Value, b: &mut Vec<(String, Value)>) -> Result<bool, RefError> {
        v.check_poison()?;
        match pat {
            Pat::Wild(_) => Ok(true),
            Pat::Ident(pi) => {
                // `n @ p`: matches what `p` matches, and binds the whole value to `n`.
                if let Some((_, sub)) = &pi.subpat {
                    if !self.match_pat(sub, v, b)? {
                        return Ok(false);
                    }
                    b.push((pi.ident.to_string(), v.clone()));
                    return Ok(true);
                }
                let name = pi.ident.to_string();
                // A bare identifier that names a unit variant of a known enum, or
                // a const, is a constant pattern; otherwise it binds.
                if let Some(c) = self.lookup_const(&name)? {
                    return Ok(values_equal(&c, v)?);
                }
                b.push((name, v.clone()));
                Ok(true)
            }
            Pat::Lit(l) => {
                let lit = self.eval_lit(&l.lit)?;
                let lit = lit.size_like(v);
                values_equal(&lit, v)
            }
            Pat::Path(p) => {
                let c = self.eval_path_value(&p.path, &[])?;
                let c = c.size_like(v);
                values_equal(&c, v)
            }
            Pat::Tuple(t) => {
                let Value::Tuple(vs) = v else {
                    return Err(rt(format!("tuple pattern against {v}")));
                };
                if t.elems.iter().any(|p| matches!(p, Pat::Rest(_))) {
                    return Err(unsup("`..` in a tuple pattern"));
                }
                if vs.len() != t.elems.len() {
                    return Err(rt("tuple pattern arity mismatch"));
                }
                for (p, v) in t.elems.iter().zip(vs) {
                    if !self.match_pat(p, v, b)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            Pat::Range(r) => {
                let x = v.as_int()?;
                let lo = match &r.start {
                    Some(e) => Some(self.eval_expr(e)?.as_int()?),
                    None => None,
                };
                let hi = match &r.end {
                    Some(e) => Some(self.eval_expr(e)?.as_int()?),
                    None => None,
                };
                let ok_lo = lo.is_none_or(|l| x >= l);
                let ok_hi = match (hi, &r.limits) {
                    (None, _) => true,
                    (Some(h), syn::RangeLimits::Closed(_)) => x <= h,
                    (Some(h), syn::RangeLimits::HalfOpen(_)) => x < h,
                };
                Ok(ok_lo && ok_hi)
            }
            Pat::Or(o) => {
                for alt in &o.cases {
                    let mut ab = Vec::new();
                    if self.match_pat(alt, v, &mut ab)? {
                        if !ab.is_empty() {
                            return Err(unsup("an or-pattern that binds (I.2.4: OUTSIDE)"));
                        }
                        return Ok(true);
                    }
                }
                Ok(false)
            }
            Pat::Paren(p) => self.match_pat(&p.pat, v, b),
            Pat::Reference(r) => self.match_pat(&r.pat, v, b),
            Pat::TupleStruct(ts) => Err(unsup(format!(
                "a tuple-variant pattern `{}(…)` (enums are value sets of unit variants, I.2.4)",
                path_name(&ts.path)
            ))),
            other => Err(unsup(format!("pattern {}", pat_kind(other)))),
        }
    }

    fn eval_with_bindings(&mut self, e: &Expr, b: &[(String, Value)]) -> Result<Value, RefError> {
        let depth = self.env.scopes.len();
        self.env.enter(depth);
        for (n, v) in b {
            self.env.declare(n, v.clone());
        }
        let r = self.eval_expr(e);
        self.env.leave(depth);
        r
    }

    // ── expressions ────────────────────────────────────────────────────────

    pub fn eval_expr(&mut self, e: &Expr) -> Result<Value, RefError> {
        match e {
            Expr::Lit(l) => self.eval_lit(&l.lit),
            Expr::Path(p) => {
                if p.qself.is_some() {
                    return Err(unsup("a qualified path"));
                }
                self.eval_path_value(&p.path, &[])
            }
            Expr::Paren(p) => self.eval_expr(&p.expr),
            Expr::Group(g) => self.eval_expr(&g.expr),
            Expr::Reference(r) => self.eval_expr(&r.expr),
            Expr::Unary(u) => {
                let v = self.eval_expr(&u.expr)?;
                v.check_poison()?;
                match u.op {
                    UnOp::Deref(_) => Ok(v),
                    UnOp::Not(_) => match v {
                        Value::Bool(b) => Ok(Value::Bool(!b)),
                        Value::Bits(b) => Ok(Value::Bits(b.not())),
                        Value::Int(i, t) => int_result(!i, t),
                        Value::Lit(_) | Value::LitX => Err(rt("`!` on a Bits literal of unknown width")),
                        other => Err(rt(format!("`!` on {other}"))),
                    },
                    UnOp::Neg(_) => match v {
                        Value::Bits(b) => Ok(Value::Bits(b.neg())),
                        Value::Int(i, t) => int_result(-i, t),
                        other => Err(rt(format!("`-` on {other}"))),
                    },
                    _ => Err(unsup("unknown unary operator")),
                }
            }
            Expr::Binary(b) => {
                if is_compound_assign(&b.op) {
                    return Err(rt("compound assignment in expression position"));
                }
                // Short-circuit like Rust.
                if let BinOp::And(_) = b.op {
                    let l = self.eval_expr(&b.left)?.as_bool()?;
                    return Ok(Value::Bool(l && self.eval_expr(&b.right)?.as_bool()?));
                }
                if let BinOp::Or(_) = b.op {
                    let l = self.eval_expr(&b.left)?.as_bool()?;
                    return Ok(Value::Bool(l || self.eval_expr(&b.right)?.as_bool()?));
                }
                let l = self.eval_expr(&b.left)?;
                let r = self.eval_expr(&b.right)?;
                self.binop(&b.op, l, r)
            }
            Expr::Cast(c) => {
                let v = self.eval_expr(&c.expr)?;
                let t = self.parse_ty(&c.ty)?;
                match (v, t) {
                    (Value::Int(i, _), Ty::Int(t)) => Ok(Value::Int(wrap_int(i, t), t)),
                    (Value::Bool(b), Ty::Int(t)) => Ok(Value::Int(b as i128, t)),
                    (v, t) => Err(unsup(format!("cast of {v} to {t:?}"))),
                }
            }
            Expr::Index(ix) => {
                let base = self.eval_expr(&ix.expr)?;
                base.check_poison()?;
                if let Expr::Range(_) = &*ix.index {
                    return Err(unsup("a range index (slice)"));
                }
                let i = self.eval_expr(&ix.index)?.as_index()?;
                match base {
                    Value::Bits(b) => {
                        if i >= b.width as usize {
                            return Err(rt(format!("bit index {i} out of bounds for Bits<{}>", b.width)));
                        }
                        Ok(Value::logic(b.get(i)))
                    }
                    Value::Lit(v) => {
                        if i >= 128 {
                            return Err(rt("bit index out of range"));
                        }
                        Ok(Value::logic(if (v >> i) & 1 == 1 { Logic::One } else { Logic::Zero }))
                    }
                    Value::LitX => Ok(Value::logic(Logic::X)),
                    Value::Array(els) => els
                        .get(i)
                        .cloned()
                        .ok_or_else(|| rt(format!("index {i} out of bounds for an array of {}", els.len()))),
                    other => Err(rt(format!("index into {other}"))),
                }
            }
            Expr::Field(fe) => {
                let base = self.eval_expr(&fe.base)?;
                base.check_poison()?;
                match (base, &fe.member) {
                    (Value::Struct(n, fields), syn::Member::Named(id)) => {
                        let key = id.to_string();
                        fields
                            .into_iter()
                            .find(|(k, _)| *k == key)
                            .map(|(_, v)| v)
                            .ok_or_else(|| rt(format!("no field `{key}` on `{n}`")))
                    }
                    (Value::Tuple(els), syn::Member::Unnamed(ix)) => els
                        .get(ix.index as usize)
                        .cloned()
                        .ok_or_else(|| rt("tuple index out of bounds")),
                    // A tuple struct's field (2026-11-08, MEASUREMENTS §V.4).
                    (Value::Struct(n, fields), syn::Member::Unnamed(ix)) => {
                        let key = ix.index.to_string();
                        fields
                            .into_iter()
                            .find(|(k, _)| *k == key)
                            .map(|(_, v)| v)
                            .ok_or_else(|| rt(format!("no field `{key}` on `{n}`")))
                    }
                    (other, _) => Err(rt(format!("field access on {other}"))),
                }
            }
            Expr::Tuple(t) => Ok(Value::Tuple(
                t.elems.iter().map(|e| self.eval_expr(e)).collect::<Result<_, _>>()?,
            )),
            Expr::Array(a) => Ok(Value::Array(
                a.elems.iter().map(|e| self.eval_expr(e)).collect::<Result<_, _>>()?,
            )),
            Expr::Repeat(r) => {
                let v = self.eval_expr(&r.expr)?;
                let n = self.eval_expr(&r.len)?.as_index()?;
                Ok(Value::Array(vec![v; n]))
            }
            Expr::Struct(s) => {
                if s.rest.is_some() {
                    return Err(unsup("`..rest` in a struct literal (I.2.4: OUTSIDE)"));
                }
                let mut name = s.path.segments.last().map(|x| x.ident.to_string()).unwrap_or_default();
                // `Self { .. }` inside an associated fn names the impl's type.
                if name == "Self" {
                    name = self.self_ty.last().cloned().ok_or_else(|| rt("`Self { .. }` outside an `impl`"))?;
                }
                let decl = self
                    .prog
                    .structs
                    .get(&name)
                    .cloned()
                    .ok_or_else(|| rt(format!("unknown struct `{name}`")))?;
                let mut fields = Vec::new();
                for f in &s.fields {
                    let syn::Member::Named(id) = &f.member else {
                        return Err(unsup("a tuple struct literal"));
                    };
                    let mut v = self.eval_expr(&f.expr)?;
                    // An unsuffixed integer field is the field's declared type (rustc infers it).
                    if matches!(v, Value::Int(_, IntTy::Unsuffixed)) {
                        let pos = decl.iter().position(|k| *k == id.to_string());
                        let ty = pos.and_then(|i| self.prog.struct_tys.get(&name).and_then(|t| t.get(i)).cloned());
                        if let Some(ty) = ty {
                            if let Ok(t) = self.parse_ty(&ty) {
                                v = v.coerce_to(&t)?;
                            }
                        }
                    }
                    fields.push((id.to_string(), v));
                }
                // Declaration order, so two values of one struct compare by field.
                let mut ordered = Vec::new();
                for k in &decl {
                    let v = fields
                        .iter()
                        .find(|(n, _)| n == k)
                        .map(|(_, v)| v.clone())
                        .ok_or_else(|| rt(format!("struct literal `{name}` misses field `{k}`")))?;
                    ordered.push((k.clone(), v));
                }
                Ok(Value::Struct(name, ordered))
            }
            Expr::Call(c) => self.eval_call(c),
            Expr::MethodCall(mc) => self.eval_method_call(mc),
            Expr::If(i) => {
                if let Expr::Let(_) = &*i.cond {
                    return Err(unsup("`if let` (I.2.5: OUTSIDE)"));
                }
                let c = self.eval_expr(&i.cond)?.as_bool()?;
                if c {
                    self.eval_block_value(&i.then_branch.stmts, Vec::new())
                } else {
                    match &i.else_branch {
                        None => Ok(Value::Tuple(Vec::new())),
                        Some((_, e)) => match &**e {
                            Expr::Block(b) => self.eval_block_value(&b.block.stmts, Vec::new()),
                            other => self.eval_expr(other),
                        },
                    }
                }
            }
            Expr::Match(m) => {
                let scrutinee = self.eval_expr(&m.expr)?;
                for arm in &m.arms {
                    let mut b = Vec::new();
                    if self.match_pat(&arm.pat, &scrutinee, &mut b)? {
                        if let Some((_, guard)) = &arm.guard {
                            if !self.eval_with_bindings(guard, &b)?.as_bool()? {
                                continue;
                            }
                        }
                        return match &*arm.body {
                            Expr::Block(bl) => self.eval_block_value(&bl.block.stmts, b),
                            other => self.eval_with_bindings(other, &b),
                        };
                    }
                }
                Err(rt(format!("no match arm matched {scrutinee}")))
            }
            Expr::Block(b) => self.eval_block_value(&b.block.stmts, Vec::new()),
            Expr::Macro(m) => self.eval_macro(m),
            Expr::Await(_) => Err(unsup("a tick inside an expression (I.2.3: `tick` is a statement)")),
            Expr::Const(_) => Ok(Value::Tuple(Vec::new())),
            Expr::Closure(_) => Err(unsup("a closure (I.2.4: OUTSIDE outside a memory preload)")),
            Expr::Try(_) => Err(unsup("`?` (I.2.5(a): OUTSIDE)")),
            Expr::Range(_) => Err(unsup("a range value outside a `for`")),
            Expr::Assign(_) => Err(rt("assignment in expression position")),
            other => Err(unsup(format!("expression form `{}`", expr_kind(other)))),
        }
    }

    /// A block in expression position: its statements, then its tail value. Must
    /// not tick (a tick is a statement of the kernel, never inside an expression).
    fn eval_block_value(&mut self, stmts: &[Stmt], bindings: Vec<(String, Value)>) -> Result<Value, RefError> {
        let depth = self.env.scopes.len();
        self.env.enter(depth);
        for (n, v) in bindings {
            self.env.declare(&n, v);
        }
        let mut result = Value::Tuple(Vec::new());
        let n = stmts.len();
        for (i, s) in stmts.iter().enumerate() {
            if i + 1 == n {
                if let Stmt::Expr(e, None) = s {
                    result = self.eval_expr(e)?;
                    break;
                }
            }
            match self.exec_stmt(s, depth + 1, None)? {
                Flow::Done => {}
                Flow::Suspend(_) => return Err(unsup("a tick inside an expression block")),
                Flow::Break | Flow::Continue => {
                    return Err(unsup("`break`/`continue` inside an expression block"));
                }
            }
        }
        self.env.leave(depth);
        Ok(result)
    }

    fn eval_lit(&self, l: &syn::Lit) -> Result<Value, RefError> {
        match l {
            syn::Lit::Int(i) => {
                let v: i128 = i
                    .base10_parse()
                    .map_err(|e| rt(format!("integer literal: {e}")))?;
                let t = match i.suffix() {
                    "" => IntTy::Unsuffixed,
                    s => IntTy::parse(s).ok_or_else(|| unsup(format!("integer suffix `{s}`")))?,
                };
                Ok(Value::Int(v, t))
            }
            syn::Lit::Bool(b) => Ok(Value::Bool(b.value)),
            other => Err(unsup(format!("literal `{}`", quote::ToTokens::to_token_stream(other)))),
        }
    }

    fn lookup_const(&mut self, name: &str) -> Result<Option<Value>, RefError> {
        if let Some(v) = self.prog.const_vals.borrow().get(name) {
            return Ok(Some(v.clone()));
        }
        let Some((ty, init)) = self.prog.consts.get(name) else {
            return Ok(None);
        };
        if self.call_stack.iter().any(|c| c == &format!("const {name}")) {
            return Err(rt(format!("`const {name}` refers to itself")));
        }
        self.call_stack.push(format!("const {name}"));
        let v = Exec::eval_const(self.prog, init);
        self.call_stack.pop();
        let mut v = v?;
        let tuple_struct_ty = matches!(ty, Some(syn::Type::Path(tp))
            if tp.path.get_ident().is_some_and(|i| self.prog.tuple_structs.contains_key(&i.to_string())));
        if let (Some(ty), false) = (ty, tuple_struct_ty) {
            v = v.coerce_to(&self.parse_ty(ty)?)?;
        }
        self.prog.const_vals.borrow_mut().insert(name.to_string(), v.clone());
        Ok(Some(v))
    }

    /// A path in value position: a local, a const, a const generic, `Logic::One`,
    /// an enum unit variant, `u8::MAX`.
    fn eval_path_value(&mut self, path: &syn::Path, _args: &[Expr]) -> Result<Value, RefError> {
        if path.segments.len() == 1 {
            let name = path.segments[0].ident.to_string();
            if let Some(v) = self.env.get(&name) {
                let v = v.clone();
                v.check_poison()?;
                return Ok(v);
            }
            if let Some(v) = self.lookup_const(&name)? {
                return Ok(v);
            }
            if let Some(g) = self.prog.generics.get(&name) {
                return Ok(Value::Int(*g as i128, IntTy::Usize));
            }
            if self.prog.inputs.contains_key(&name) {
                return Err(unsup(format!(
                    "`In` port `{name}` used other than as the receiver of `.read()` (II.1 R19)"
                )));
            }
            return Err(rt(format!("unknown name `{name}`")));
        }
        if let Some((a, b)) = path2(path) {
            match (a.as_str(), b.as_str()) {
                ("Logic", "One") => return Ok(Value::logic(Logic::One)),
                ("Logic", "Zero") => return Ok(Value::logic(Logic::Zero)),
                ("Logic", "X") => return Ok(Value::logic(Logic::X)),
                _ => {}
            }
            if let Some(variants) = self.prog.enums.get(&a) {
                if variants.iter().any(|v| *v == b) {
                    return Ok(Value::Enum(a, b));
                }
                return Err(rt(format!("`{a}::{b}` is not a unit variant of `{a}`")));
            }
            if let Some(t) = IntTy::parse(&a) {
                let (lo, hi) = t.range();
                return match b.as_str() {
                    "MAX" => Ok(Value::Int(hi, t)),
                    "MIN" => Ok(Value::Int(lo, t)),
                    "BITS" => Ok(Value::Int(int_bits(t) as i128, IntTy::U32)),
                    _ => Err(unsup(format!("`{a}::{b}`"))),
                };
            }
            if let Some(v) = self.lookup_const(&format!("{a}::{b}"))? {
                return Ok(v);
            }
            if self.prog.structs.contains_key(&a) || self.prog.enums.contains_key(&a) {
                return Err(unsup(format!("associated item `{a}::{b}` (II.3 row (d): an associated const)")));
            }
        }
        Err(unsup(format!("path `{}`", path_name(path))))
    }

    fn eval_macro(&mut self, m: &syn::ExprMacro) -> Result<Value, RefError> {
        let name = path_name(&m.mac.path);
        if name != "matches" {
            return Err(unsup(format!(
                "macro `{name}!` in an expression (II.1 R13/R14: a read inside macro tokens is neither classified nor rewritten)"
            )));
        }
        // `matches!(e, p [if guard])` is DESUGARED to a match with a bool in each arm.
        let args: MatchesArgs = syn::parse2(m.mac.tokens.clone())
            .map_err(|e| RefError::Parse(format!("matches!: {e}")))?;
        let v = self.eval_expr(&args.expr)?;
        let mut b = Vec::new();
        if !self.match_pat(&args.pat, &v, &mut b)? {
            return Ok(Value::Bool(false));
        }
        if let Some(g) = &args.guard {
            return Ok(Value::Bool(self.eval_with_bindings(g, &b)?.as_bool()?));
        }
        Ok(Value::Bool(true))
    }

    fn eval_call(&mut self, c: &syn::ExprCall) -> Result<Value, RefError> {
        let Expr::Path(fp) = &*c.func else {
            return Err(unsup("a call through a non-path callee"));
        };
        let path = &fp.path;
        let segs: Vec<String> = path.segments.iter().map(|s| s.ident.to_string()).collect();
        let args: Vec<Value> = c.args.iter().map(|a| self.eval_expr(a)).collect::<Result<_, _>>()?;
        for a in &args {
            a.check_poison()?;
        }
        if segs.len() == 1 {
            let name = &segs[0];
            if let Some(f) = self.prog.fns.get(name).cloned() {
                return self.call_fn(name, &f.sig, &f.block, args);
            }
            if let Some(tys) = self.prog.tuple_structs.get(name).cloned() {
                if tys.len() != args.len() {
                    return Err(rt(format!("`{name}` has {} fields, {} given", tys.len(), args.len())));
                }
                let mut fields = Vec::with_capacity(args.len());
                for (i, (a, t)) in args.into_iter().zip(&tys).enumerate() {
                    let v = match self.parse_ty(t) {
                        Ok(ty) => a.coerce_to(&ty)?,
                        Err(_) => a,
                    };
                    fields.push((i.to_string(), v));
                }
                return Ok(Value::Struct(name.clone(), fields));
            }
            return Err(unsup(format!("call of unknown function `{name}`")));
        }
        if segs.len() == 2 {
            let (ty, m) = (&segs[0], &segs[1]);
            if ty == "Bits" {
                // `Bits::<W>::…` fixes the width; `Bits::…` leaves it to the context.
                let width = match seg_const_args(&path.segments[0]).first() {
                    Some(a) => Some(self.generic_arg_usize(a)? as u32),
                    None => None,
                };
                let margs = seg_const_args(&path.segments[1]);
                return self.bits_ctor(m, width, &margs, args);
            }
            if ty == "Logic" {
                return match m.as_str() {
                    "from_bool" => Ok(Value::logic(Logic::from_bool(arg1(&args)?.as_bool()?))),
                    // `From<Bits<1>> for Logic`, `From<bool> for Logic` (I.14.3(d)).
                    "from" => match arg1(&args)? {
                        Value::Bool(b) => Ok(Value::logic(Logic::from_bool(b))),
                        v => v.clone().coerce_to(&Ty::Logic),
                    },
                    _ => Err(unsup(format!("`Logic::{m}`"))),
                };
            }
            if let Some(f) = self.prog.methods.get(&(ty.clone(), m.clone())).cloned() {
                // An associated function (`Model::new()`), or a method called in
                // path form.
                return self.call_fn(&format!("{ty}::{m}"), &f.sig, &f.block, args);
            }
            if self.prog.enums.contains_key(ty) {
                return Err(unsup(format!("a tuple variant `{ty}::{m}(…)` (I.2.4: enums are unit-variant value sets)")));
            }
        }
        Err(unsup(format!("call of `{}`", segs.join("::"))))
    }

    fn generic_arg_usize(&mut self, a: &syn::GenericArgument) -> Result<usize, RefError> {
        match a {
            syn::GenericArgument::Const(e) => self.eval_expr(e)?.as_index(),
            syn::GenericArgument::Type(syn::Type::Path(p)) if p.qself.is_none() => {
                self.eval_path_value(&p.path, &[])?.as_index()
            }
            other => Err(RefError::Parse(format!(
                "unreadable const generic argument `{}`",
                quote::ToTokens::to_token_stream(other)
            ))),
        }
    }

    fn bits_ctor(
        &mut self,
        m: &str,
        width: Option<u32>,
        margs: &[&syn::GenericArgument],
        args: Vec<Value>,
    ) -> Result<Value, RefError> {
        let sized = |v: Value| -> Result<Value, RefError> {
            match width {
                Some(w) => v.coerce_to(&Ty::Bits(w)),
                None => Ok(v),
            }
        };
        match m {
            // `From<Logic> for Bits<1>`, `From<[Logic; N]>`, `From<u128>` (truncating,
            // `from_uint`) — I.14.3(d), Phase C step 8.
            "from" => match arg1(&args)? {
                Value::Int(v, _) => sized(Value::Lit(v as u128)),
                Value::Array(_) => self.bits_ctor("from_array", width, margs, args),
                v => sized(v.clone()),
            },
            "zero" => sized(Value::Lit(0)),
            "x" => sized(Value::LitX),
            // Unsized: rustc infers the width; sized at first contact (I.14.4,
            // Phase C step 8 — refused until then, a parity item of D-19 (iv)).
            "all_ones" => sized(Value::LitOnes),
            "from_lit" => {
                let Some(a) = margs.first() else {
                    return Err(rt("`from_lit` without a const generic"));
                };
                let v = self.generic_arg_usize(a)? as u128;
                sized(Value::Lit(v))
            }
            "from_u8" | "from_u16" | "from_u32" | "from_u64" | "from_u128" | "from_usize" => {
                let a = arg1(&args)?;
                let v = a.as_int()?;
                let bits = match m {
                    "from_u8" => 8,
                    "from_u16" => 16,
                    "from_u32" => 32,
                    "from_u64" | "from_usize" => 64,
                    _ => 128,
                };
                if v < 0 || (bits < 128 && v >= (1i128 << bits)) {
                    return Err(rt(format!("`Bits::{m}({v})`: the argument is not a {m}-sized integer")));
                }
                sized(Value::Lit(v as u128))
            }
            "from_array" | "from_slice" => {
                let Value::Array(els) = arg1(&args)? else {
                    return Err(rt(format!("`Bits::{m}` of a non-array")));
                };
                let ls: Vec<Logic> = els.iter().map(Value::as_logic).collect::<Result<_, _>>()?;
                Ok(Value::Bits(RefBits::from_logic_slice(&ls)))
            }
            "mux" => {
                if args.len() != 3 {
                    return Err(rt("`Bits::mux` takes three arguments"));
                }
                let sel = args[0].as_logic()?;
                let a = args[1].clone().size_like(&args[2]);
                let b = args[2].clone().size_like(&a);
                Ok(Value::Bits(RefBits::mux(sel, &a.as_bits()?, &b.as_bits()?)))
            }
            other => Err(unsup(format!("`Bits::{other}`"))),
        }
    }

    fn eval_method_call(&mut self, mc: &syn::ExprMethodCall) -> Result<Value, RefError> {
        if let Some(v) = self.memory_call(mc)? {
            return Ok(v);
        }
        let m = mc.method.to_string();
        // An `In` port read: the one form a read takes (I.2.3), yielding this
        // resumption's input (I.4.1).
        if let Expr::Path(p) = &*mc.receiver {
            if p.path.segments.len() == 1 {
                let name = p.path.segments[0].ident.to_string();
                if self.env.get(&name).is_none() {
                    if self.prog.inputs.contains_key(&name) {
                        if m != "read" || !mc.args.is_empty() {
                            return Err(unsup(format!("`In` port `{name}` used other than as `{name}.read()` (II.1 R19)")));
                        }
                        return self
                            .inputs
                            .get(&name)
                            .cloned()
                            .ok_or_else(|| rt(format!("no input value given for port `{name}` this resumption")));
                    }
                    if self.prog.outputs.iter().any(|(n, _)| *n == name) {
                        return match m.as_str() {
                            "write" => {
                                self.exec_write(mc)?;
                                Ok(Value::Tuple(Vec::new()))
                            }
                            _ => Err(unsup(format!("`{name}.{m}()` on a `RegOut` port"))),
                        };
                    }
                    if self.prog.clocks.iter().any(|c| *c == name) {
                        return Err(unsup(format!(
                            "`{name}.{m}()` on the clock outside `clk.tick().await` (II.1 R1/R17)"
                        )));
                    }
                }
            }
        }
        let recv = self.eval_expr(&mc.receiver)?;
        recv.check_poison()?;
        let args: Vec<Value> = mc.args.iter().map(|a| self.eval_expr(a)).collect::<Result<_, _>>()?;
        for a in &args {
            a.check_poison()?;
        }
        let targs = seg_const_args(&syn::PathSegment {
            ident: mc.method.clone(),
            arguments: match &mc.turbofish {
                Some(t) => syn::PathArguments::AngleBracketed(t.clone()),
                None => syn::PathArguments::None,
            },
        })
        .into_iter()
        .map(|a| self.generic_arg_usize(a))
        .collect::<Result<Vec<usize>, _>>()?;

        match (&recv, m.as_str()) {
            (_, "clone") => return Ok(recv),
            // `From`/`Into` between `Logic` and `Bits<1>` (I.14.3(d); Phase C step 8):
            // the identity on the bit — this interpreter does not distinguish the two.
            (Value::Bits(_) | Value::Lit(_) | Value::LitX | Value::LitOnes, "into") if args.is_empty() => {
                return Ok(recv);
            }
            (Value::LitOnes, _) => {
                return Err(rt(format!("`.{m}()` on a Bits::all_ones() whose width is not yet fixed")));
            }
            (Value::Bits(_) | Value::Lit(_) | Value::LitX, _) => {
                return self.bits_method(recv, &m, &targs, &args);
            }
            (Value::Int(v, t), _) => {
                let (v, t) = (*v, *t);
                return self.int_method(v, t, &m, &args);
            }
            (Value::Bool(b), "then_some") | (Value::Bool(b), "then") => {
                let _ = b;
                return Err(unsup("`Option` (I.2.5(a): OUTSIDE)"));
            }
            (Value::Struct(name, _), _) | (Value::Enum(name, _), _) => {
                let name = name.clone();
                if let Some(f) = self.prog.methods.get(&(name.clone(), m.clone())).cloned() {
                    let takes_self = matches!(f.sig.inputs.first(), Some(syn::FnArg::Receiver(_)));
                    if !takes_self {
                        return Err(rt(format!("`{name}::{m}` has no `self`")));
                    }
                    let mut_self = matches!(
                        f.sig.inputs.first(),
                        Some(syn::FnArg::Receiver(r)) if r.mutability.is_some() || matches!(&*r.ty, syn::Type::Reference(rf) if rf.mutability.is_some())
                    );
                    let mut all = vec![recv.clone()];
                    all.extend(args);
                    let (result, new_self) = self.call_fn_with_self(&format!("{name}::{m}"), &f.sig, &f.block, all)?;
                    if mut_self {
                        self.assign_place(&mc.receiver, new_self)?;
                    }
                    return Ok(result);
                }
                return Err(unsup(format!("method `{m}` on `{name}`")));
            }
            (Value::Array(_), "len") => {
                let Value::Array(e) = recv else { unreachable!() };
                return Ok(Value::Int(e.len() as i128, IntTy::Usize));
            }
            (Value::Array(_), "to_vec") | (Value::Array(_), "iter") => {
                return Err(unsup(format!("`[T; N]::{m}` (I.14 states no array methods)")));
            }
            _ => {}
        }
        Err(unsup(format!("method `{m}` on {recv}")))
    }

    fn bits_method(&mut self, recv: Value, m: &str, targs: &[usize], args: &[Value]) -> Result<Value, RefError> {
        // Methods that only make sense on a sized value size the literal first
        // where the context can (an `as_u8` on a literal is the literal).
        let lit = match recv {
            Value::Lit(v) => Some(v),
            _ => None,
        };
        if let Some(v) = lit {
            match m {
                "as_u8" | "as_u16" | "as_u32" | "as_u64" | "as_u128" | "as_usize" => {
                    return Ok(Value::Int(v as i128, as_int_ty(m)));
                }
                "as_bool" => return Ok(Value::Bool(v != 0)),
                "is_valid" => return Ok(Value::Bool(true)),
                _ => return Err(rt(format!("`.{m}()` on a Bits literal whose width is not yet fixed"))),
            }
        }
        if matches!(recv, Value::LitX) {
            return match m {
                "is_valid" => Ok(Value::Bool(false)),
                _ => Err(rt(format!("`.{m}()` on a Bits::x() whose width is not yet fixed"))),
            };
        }
        let b = recv.as_bits()?;
        let w = b.width;
        let x_err = |what: &str| rt(format!("{what} of an X value (the simulator panics: `Cannot convert X`)"));
        Ok(match m {
            "as_u8" | "as_u16" | "as_u32" | "as_u64" | "as_u128" | "as_usize" => {
                let need = match m {
                    "as_u8" => 8,
                    "as_u16" => 16,
                    "as_u32" => 32,
                    "as_u64" | "as_usize" => 64,
                    _ => 128,
                };
                if w > need {
                    return Err(rt(format!("`.{m}()` on Bits<{w}>: too wide (a compile error in the source)")));
                }
                let v = b.as_uint().ok_or_else(|| x_err(m))?;
                Value::Int(v as i128, as_int_ty(m))
            }
            "as_bool" => {
                if w != 1 {
                    return Err(rt("`.as_bool()` on a Bits value"));
                }
                match b.as_logic() {
                    Logic::X => return Err(x_err("as_bool")),
                    l => Value::Bool(l == Logic::One),
                }
            }
            "is_valid" => Value::Bool(b.is_valid()),
            "get" => {
                let i = arg1(args)?.as_index()?;
                if i >= w as usize {
                    return Err(rt(format!("`get({i})` out of bounds for Bits<{w}>")));
                }
                Value::logic(b.get(i))
            }
            "shift_left" => Value::Bits(b.shl(arg1(args)?.as_index()?)),
            "shift_right" => Value::Bits(b.shr(arg1(args)?.as_index()?)),
            "arithmetic_shift_right" => Value::Bits(b.arithmetic_shift_right(arg1(args)?.as_index()?)),
            "with_lsb" => Value::Bits(b.with_lsb(arg1(args)?.as_logic()?)),
            "with_msb" => Value::Bits(b.with_msb(arg1(args)?.as_logic()?)),
            "and_reduce" => Value::logic(b.and_reduce()),
            "or_reduce" => Value::logic(b.or_reduce()),
            "xor_reduce" => Value::logic(b.xor_reduce()),
            "nand_reduce" => Value::logic(!b.and_reduce()),
            "nor_reduce" => Value::logic(!b.or_reduce()),
            "xnor_reduce" => Value::logic(!b.xor_reduce()),
            "eq_logic" | "ne_logic" | "lt_logic" | "le_logic" | "gt_logic" | "ge_logic" => {
                let o = arg1(args)?.size_like(&recv).as_bits()?;
                let l = match m {
                    "eq_logic" => b.eq_logic(&o),
                    "ne_logic" => !b.eq_logic(&o),
                    "lt_logic" => b.lt_logic(&o),
                    "le_logic" => b.le_logic(&o),
                    "gt_logic" => b.gt_logic(&o),
                    _ => b.ge_logic(&o),
                };
                Value::logic(l)
            }
            "zero_extend" | "sign_extend" | "truncate" | "replicate" => {
                let Some(&mw) = targs.first() else {
                    return Err(rt(format!("`.{m}()` without its const generic width")));
                };
                let mw = mw as u32;
                Value::Bits(match m {
                    "zero_extend" => {
                        if mw < w {
                            return Err(rt(format!("zero_extend::<{mw}> on Bits<{w}>")));
                        }
                        b.zero_extend(mw)
                    }
                    "sign_extend" => {
                        if mw < w {
                            return Err(rt(format!("sign_extend::<{mw}> on Bits<{w}>")));
                        }
                        b.sign_extend(mw)
                    }
                    "truncate" => {
                        if mw > w {
                            return Err(rt(format!("truncate::<{mw}> on Bits<{w}>")));
                        }
                        b.truncate(mw)
                    }
                    _ => {
                        if mw % w != 0 {
                            return Err(rt(format!("replicate::<{mw}> on Bits<{w}>")));
                        }
                        b.replicate(mw)
                    }
                })
            }
            "part_select" => {
                let Some(&len) = targs.first() else {
                    return Err(rt("`.part_select()` without its const generic length"));
                };
                let lo = arg1(args)?.as_index()?;
                if lo + len > w as usize {
                    return Err(rt(format!("part_select::<{len}>({lo}) out of bounds for Bits<{w}>")));
                }
                Value::Bits(b.part_select(lo, len as u32))
            }
            "concat" => {
                let o = arg1(args)?.as_bits()?;
                let r = b.concat(&o);
                if let Some(&out) = targs.get(1) {
                    if out as u32 != r.width {
                        return Err(rt(format!("concat::<_, {out}> of Bits<{w}> and Bits<{}>", o.width)));
                    }
                }
                Value::Bits(r)
            }
            "as_array" => Value::Array(b.to_logic_vec().into_iter().map(Value::logic).collect()),
            "wrapping_add" => Value::Bits(b.add(&arg1(args)?.size_like(&recv).as_bits()?)),
            "wrapping_sub" => Value::Bits(b.sub(&arg1(args)?.size_like(&recv).as_bits()?)),
            "wrapping_mul" => Value::Bits(b.mul(&arg1(args)?.size_like(&recv).as_bits()?)),
            "saturating_add" | "saturating_sub" | "checked_add" | "checked_sub" => {
                return Err(unsup(format!(
                    "`Bits::{m}` (not a method of copper-core's `Bits`; the integer one is I.14.3(c))"
                )));
            }
            "len" => Value::Int(w as i128, IntTy::Usize),
            other => return Err(unsup(format!("`Bits::{other}` (not a method of copper-core's `Bits`)"))),
        })
    }

    fn int_method(&mut self, v: i128, t: IntTy, m: &str, args: &[Value]) -> Result<Value, RefError> {
        let t = if t == IntTy::Unsuffixed { IntTy::Usize } else { t };
        match m {
            "wrapping_add" | "wrapping_sub" | "wrapping_mul" | "saturating_add" | "saturating_sub"
            | "saturating_mul" | "pow" => {
                let o = arg1(args)?.as_int()?;
                let (lo, hi) = t.range();
                let r = match m {
                    "wrapping_add" => wrap_int(v + o, t),
                    "wrapping_sub" => wrap_int(v - o, t),
                    "wrapping_mul" => wrap_int(v.wrapping_mul(o), t),
                    "saturating_add" => (v + o).clamp(lo, hi),
                    "saturating_sub" => (v - o).clamp(lo, hi),
                    // I.14.3(c): clamped at the type's bounds (Phase C step 8).
                    "saturating_mul" => v.checked_mul(o).map_or(if (v < 0) != (o < 0) { lo } else { hi }, |p| p.clamp(lo, hi)),
                    _ => {
                        let r = v.checked_pow(o as u32).ok_or_else(|| rt("integer overflow in `pow`"))?;
                        if r > hi { return Err(rt("integer overflow in `pow`")); }
                        r
                    }
                };
                Ok(Value::Int(r, t))
            }
            // The rest of the `const` integer methods the shared evaluator runs
            // (2026-11-08, MEASUREMENTS §V.4), each with Rust's meaning at `t`
            // (`min`/`max` are `Ord`'s, not `const fn`s: outside, as in rustc).
            "wrapping_neg" => Ok(Value::Int(wrap_int(-v, t), t)),
            "wrapping_shl" | "wrapping_shr" => {
                let s = (arg1(args)?.as_int()? as u32) % int_bits(t);
                let bits = int_bits(t);
                let pat = if bits >= 128 { v as u128 } else { (v as u128) & ((1u128 << bits) - 1) };
                let r = match (m, signed_int(t)) {
                    ("wrapping_shl", _) => wrap_int((pat << s) as i128, t),
                    (_, true) => v >> s,
                    _ => (pat >> s) as i128,
                };
                Ok(Value::Int(r, t))
            }
            "count_zeros" | "leading_zeros" => {
                let bits = int_bits(t);
                let pat = if bits >= 128 { v as u128 } else { (v as u128) & ((1u128 << bits) - 1) };
                let r = if m == "count_zeros" { bits - pat.count_ones() } else { pat.leading_zeros() - (128 - bits) };
                Ok(Value::Int(r as i128, IntTy::U32))
            }
            "next_power_of_two" if !signed_int(t) => {
                let p = if v <= 1 { 1 } else { 1i128.checked_shl(128 - ((v - 1) as u128).leading_zeros()).ok_or_else(|| rt("integer overflow in `next_power_of_two`"))? };
                if p > t.range().1 {
                    return Err(rt("integer overflow in `next_power_of_two`"));
                }
                Ok(Value::Int(p, t))
            }
            "abs" if signed_int(t) => {
                let r = v.checked_abs().filter(|r| *r <= t.range().1).ok_or_else(|| rt("integer overflow in `abs`"))?;
                Ok(Value::Int(r, t))
            }
            "is_power_of_two" => Ok(Value::Bool(v > 0 && (v & (v - 1)) == 0)),
            "trailing_zeros" => Ok(Value::Int(if v == 0 { int_bits(t) as i128 } else { v.trailing_zeros() as i128 }, IntTy::U32)),
            "count_ones" => Ok(Value::Int(wrap_int(v, t).count_ones() as i128, IntTy::U32)),
            other => Err(unsup(format!("integer method `{other}`"))),
        }
    }

    /// Call a helper `fn`: a fresh environment, parameters bound by their declared
    /// types, the body evaluated as a block value (I.2.4: inlined). Ticks, loops
    /// that tick, and recursion are outside it.
    fn call_fn(&mut self, name: &str, sig: &syn::Signature, block: &syn::Block, args: Vec<Value>) -> Result<Value, RefError> {
        let (v, _) = self.call_fn_with_self(name, sig, block, args)?;
        Ok(v)
    }

    fn call_fn_with_self(
        &mut self,
        name: &str,
        sig: &syn::Signature,
        block: &syn::Block,
        args: Vec<Value>,
    ) -> Result<(Value, Value), RefError> {
        if self.call_stack.iter().any(|c| c == name) {
            return Err(unsup(format!("recursion through `{name}` (II.1 R6 / I.2.5(c): no unbounded work inside an instant)")));
        }
        if sig.asyncness.is_some() {
            return Err(unsup(format!("a call of the `async fn {name}` (I.2.5(b): nested `async` is outside)")));
        }
        if sig.inputs.len() != args.len() {
            return Err(rt(format!("`{name}` takes {} arguments, {} given", sig.inputs.len(), args.len())));
        }
        let mut callee = Env::default();
        callee.enter(0);
        let mut self_slot = Value::Tuple(Vec::new());
        for (p, v) in sig.inputs.iter().zip(args) {
            match p {
                syn::FnArg::Receiver(_) => {
                    callee.declare("self", v.clone());
                    self_slot = v;
                }
                syn::FnArg::Typed(pt) => {
                    let t = self.parse_ty(&pt.ty)?;
                    let v = v.coerce_to(&t)?;
                    let Some(n) = pat_ident(&pt.pat) else {
                        return Err(unsup("a pattern parameter in a helper fn"));
                    };
                    callee.declare(&n, v);
                }
            }
        }
        let saved = std::mem::replace(self.env, callee);
        self.call_stack.push(name.to_string());
        let impl_ty = name.split_once("::").map(|(t, _)| t.to_string());
        if let Some(t) = &impl_ty {
            self.self_ty.push(t.clone());
        }
        let result = self.eval_fn_body(block);
        if impl_ty.is_some() {
            self.self_ty.pop();
        }
        self.call_stack.pop();
        let callee = std::mem::replace(self.env, saved);
        let mut v = result?;
        if let syn::ReturnType::Type(_, t) = &sig.output {
            // `-> Self` names the impl's struct, whose value is already shaped.
            let is_self = matches!(&**t, syn::Type::Path(tp) if tp.path.is_ident("Self"));
            if !is_self {
                v = v.coerce_to(&self.parse_ty(t)?)?;
            }
        }
        let new_self = callee.get("self").cloned().unwrap_or(self_slot);
        Ok((v, new_self))
    }

    /// A helper's body: statements then the tail, with `return e` honoured.
    fn eval_fn_body(&mut self, block: &syn::Block) -> Result<Value, RefError> {
        let n = block.stmts.len();
        for (i, s) in block.stmts.iter().enumerate() {
            if i + 1 == n {
                if let Stmt::Expr(e, None) = s {
                    return self.eval_expr(e);
                }
            }
            if let Stmt::Expr(Expr::Return(r), _) = s {
                return match &r.expr {
                    Some(e) => self.eval_expr(e),
                    None => Ok(Value::Tuple(Vec::new())),
                };
            }
            match self.exec_stmt(s, 1, None)? {
                Flow::Done => {}
                Flow::Suspend(_) => return Err(unsup("a tick inside a helper fn (I.2.5(c))")),
                _ => return Err(unsup("`break`/`continue` at the top of a helper fn")),
            }
        }
        Ok(Value::Tuple(Vec::new()))
    }

    // ── operators ──────────────────────────────────────────────────────────

    pub fn binop(&mut self, op: &BinOp, l: Value, r: Value) -> Result<Value, RefError> {
        l.check_poison()?;
        r.check_poison()?;
        // Shifts: the amount is an integer whatever the left side is.
        if matches!(op, BinOp::Shl(_) | BinOp::Shr(_)) {
            let n = r.as_int()?;
            if n < 0 {
                return Err(rt("negative shift amount"));
            }
            return match l {
                Value::Bits(b) => Ok(Value::Bits(if matches!(op, BinOp::Shl(_)) { b.shl(n as usize) } else { b.shr(n as usize) })),
                Value::Int(v, t) => {
                    let bits = int_bits(t);
                    if t != IntTy::Unsuffixed && n as u32 >= bits {
                        return Err(rt(format!("shift by {n} overflows {}", t.name())));
                    }
                    let r = if matches!(op, BinOp::Shl(_)) {
                        if t == IntTy::Unsuffixed { v.checked_shl(n as u32).ok_or_else(|| rt("shift overflow"))? } else { wrap_int(v << n, t) }
                    } else {
                        v >> n
                    };
                    int_result(r, t)
                }
                Value::Lit(v) => Ok(Value::Lit(if matches!(op, BinOp::Shl(_)) { v << n } else { v >> n })),
                other => Err(rt(format!("shift of {other}"))),
            };
        }
        let l = l.size_like(&r);
        let r = r.size_like(&l);
        match (&l, &r) {
            (Value::Bits(a), Value::Bits(b)) => {
                if a.width != b.width {
                    return Err(rt(format!("width mismatch: Bits<{}> {} Bits<{}>", a.width, binop_text(op), b.width)));
                }
                Ok(match op {
                    BinOp::Add(_) => Value::Bits(a.add(b)),
                    BinOp::Sub(_) => Value::Bits(a.sub(b)),
                    BinOp::Mul(_) => Value::Bits(a.mul(b)),
                    // I.14.3(b) (D-19 3A): truncating unsigned division, X on X or a
                    // zero divisor; lowered by the transpiler since Phase C step 8.
                    BinOp::Div(_) => Value::Bits(a.div(b)),
                    BinOp::Rem(_) => Value::Bits(a.rem(b)),
                    BinOp::BitAnd(_) => Value::Bits(a.and(b)),
                    BinOp::BitOr(_) => Value::Bits(a.or(b)),
                    BinOp::BitXor(_) => Value::Bits(a.xor(b)),
                    // The derived `PartialEq`: X == X.
                    BinOp::Eq(_) => Value::Bool(a == b),
                    BinOp::Ne(_) => Value::Bool(a != b),
                    BinOp::Lt(_) | BinOp::Le(_) | BinOp::Gt(_) | BinOp::Ge(_) => {
                        return Err(rt("`Bits` has no `PartialOrd`; use `lt_logic` and friends"));
                    }
                    other => return Err(unsup(format!("operator {} on Bits", binop_text(other)))),
                })
            }
            (Value::Lit(a), Value::Lit(b)) => {
                let (a, b) = (*a, *b);
                Ok(match op {
                    BinOp::Add(_) => Value::Lit(a.checked_add(b).ok_or_else(|| rt("literal overflow"))?),
                    BinOp::Sub(_) => Value::Lit(a.checked_sub(b).ok_or_else(|| rt("literal underflow of unknown width"))?),
                    BinOp::Mul(_) => Value::Lit(a.checked_mul(b).ok_or_else(|| rt("literal overflow"))?),
                    BinOp::BitAnd(_) => Value::Lit(a & b),
                    BinOp::BitOr(_) => Value::Lit(a | b),
                    BinOp::BitXor(_) => Value::Lit(a ^ b),
                    BinOp::Eq(_) => Value::Bool(a == b),
                    BinOp::Ne(_) => Value::Bool(a != b),
                    other => return Err(unsup(format!("operator {} on two unsized literals", binop_text(other)))),
                })
            }
            (Value::LitX, _) | (_, Value::LitX) => Err(rt("an operator on a Bits::x() whose width is not yet fixed")),
            (Value::LitOnes, _) | (_, Value::LitOnes) => Err(rt("an operator on a Bits::all_ones() whose width is not yet fixed")),
            (Value::Int(a, ta), Value::Int(b, tb)) => {
                let t = if *ta == IntTy::Unsuffixed { *tb } else { *ta };
                if *ta != IntTy::Unsuffixed && *tb != IntTy::Unsuffixed && ta != tb {
                    return Err(rt(format!("mixed integer types {} and {}", ta.name(), tb.name())));
                }
                let (a, b) = (*a, *b);
                Ok(match op {
                    BinOp::Add(_) => int_result(a + b, t)?,
                    BinOp::Sub(_) => int_result(a - b, t)?,
                    BinOp::Mul(_) => int_result(a.checked_mul(b).ok_or_else(|| rt("integer overflow"))?, t)?,
                    BinOp::Div(_) => {
                        if b == 0 { return Err(rt("integer division by zero")); }
                        int_result(a / b, t)?
                    }
                    BinOp::Rem(_) => {
                        if b == 0 { return Err(rt("integer remainder by zero")); }
                        int_result(a % b, t)?
                    }
                    BinOp::BitAnd(_) => int_result(a & b, t)?,
                    BinOp::BitOr(_) => int_result(a | b, t)?,
                    BinOp::BitXor(_) => int_result(a ^ b, t)?,
                    BinOp::Eq(_) => Value::Bool(a == b),
                    BinOp::Ne(_) => Value::Bool(a != b),
                    BinOp::Lt(_) => Value::Bool(a < b),
                    BinOp::Le(_) => Value::Bool(a <= b),
                    BinOp::Gt(_) => Value::Bool(a > b),
                    BinOp::Ge(_) => Value::Bool(a >= b),
                    other => return Err(unsup(format!("operator {} on integers", binop_text(other)))),
                })
            }
            (Value::Bool(a), Value::Bool(b)) => Ok(match op {
                BinOp::BitAnd(_) => Value::Bool(a & b),
                BinOp::BitOr(_) => Value::Bool(a | b),
                BinOp::BitXor(_) => Value::Bool(a ^ b),
                BinOp::Eq(_) => Value::Bool(a == b),
                BinOp::Ne(_) => Value::Bool(a != b),
                other => return Err(unsup(format!("operator {} on bools", binop_text(other)))),
            }),
            _ => match op {
                BinOp::Eq(_) => Ok(Value::Bool(values_equal(&l, &r)?)),
                BinOp::Ne(_) => Ok(Value::Bool(!values_equal(&l, &r)?)),
                other => Err(rt(format!("operator {} on {l} and {r}", binop_text(other)))),
            },
        }
    }
}

/// Structural equality with the derived `PartialEq` of every value kind.
fn values_equal(a: &Value, b: &Value) -> Result<bool, RefError> {
    a.check_poison()?;
    b.check_poison()?;
    Ok(match (a, b) {
        (Value::Bits(x), Value::Bits(y)) => x == y,
        (Value::Lit(x), Value::Lit(y)) => x == y,
        (Value::Lit(x), Value::Bits(y)) | (Value::Bits(y), Value::Lit(x)) => {
            y.is_valid() && y.val == *x && (y.width >= 128 || x >> y.width == 0)
        }
        (Value::Int(x, _), Value::Int(y, _)) => x == y,
        (Value::Bool(x), Value::Bool(y)) => x == y,
        (Value::Enum(e1, v1), Value::Enum(e2, v2)) => e1 == e2 && v1 == v2,
        (Value::Tuple(xs), Value::Tuple(ys)) | (Value::Array(xs), Value::Array(ys)) => {
            if xs.len() != ys.len() {
                return Err(rt("`==` on aggregates of different length"));
            }
            for (x, y) in xs.iter().zip(ys) {
                if !values_equal(x, y)? {
                    return Ok(false);
                }
            }
            true
        }
        (Value::Struct(n1, f1), Value::Struct(n2, f2)) => {
            if n1 != n2 {
                return Err(rt(format!("`==` on `{n1}` and `{n2}`")));
            }
            for ((_, x), (_, y)) in f1.iter().zip(f2) {
                if !values_equal(x, y)? {
                    return Ok(false);
                }
            }
            true
        }
        (x, y) => return Err(rt(format!("`==` on {x} and {y}"))),
    })
}

fn is_compound_assign(op: &BinOp) -> bool {
    matches!(
        op,
        BinOp::AddAssign(_)
            | BinOp::SubAssign(_)
            | BinOp::MulAssign(_)
            | BinOp::DivAssign(_)
            | BinOp::RemAssign(_)
            | BinOp::BitXorAssign(_)
            | BinOp::BitAndAssign(_)
            | BinOp::BitOrAssign(_)
            | BinOp::ShlAssign(_)
            | BinOp::ShrAssign(_)
    )
}

fn strip_assign(op: &BinOp) -> BinOp {
    match op {
        BinOp::AddAssign(_) => BinOp::Add(Default::default()),
        BinOp::SubAssign(_) => BinOp::Sub(Default::default()),
        BinOp::MulAssign(_) => BinOp::Mul(Default::default()),
        BinOp::DivAssign(_) => BinOp::Div(Default::default()),
        BinOp::RemAssign(_) => BinOp::Rem(Default::default()),
        BinOp::BitXorAssign(_) => BinOp::BitXor(Default::default()),
        BinOp::BitAndAssign(_) => BinOp::BitAnd(Default::default()),
        BinOp::BitOrAssign(_) => BinOp::BitOr(Default::default()),
        BinOp::ShlAssign(_) => BinOp::Shl(Default::default()),
        BinOp::ShrAssign(_) => BinOp::Shr(Default::default()),
        other => other.clone(),
    }
}

fn binop_text(op: &BinOp) -> String {
    quote::ToTokens::to_token_stream(op).to_string()
}

fn expr_kind(e: &Expr) -> &'static str {
    match e {
        Expr::Array(_) => "array",
        Expr::Assign(_) => "assign",
        Expr::Async(_) => "async",
        Expr::Await(_) => "await",
        Expr::Binary(_) => "binary",
        Expr::Block(_) => "block",
        Expr::Break(_) => "break",
        Expr::Call(_) => "call",
        Expr::Cast(_) => "cast",
        Expr::Closure(_) => "closure",
        Expr::Const(_) => "const",
        Expr::Continue(_) => "continue",
        Expr::Field(_) => "field",
        Expr::ForLoop(_) => "for",
        Expr::Group(_) => "group",
        Expr::If(_) => "if",
        Expr::Index(_) => "index",
        Expr::Infer(_) => "infer",
        Expr::Let(_) => "let",
        Expr::Lit(_) => "literal",
        Expr::Loop(_) => "loop",
        Expr::Macro(_) => "macro",
        Expr::Match(_) => "match",
        Expr::MethodCall(_) => "method call",
        Expr::Paren(_) => "paren",
        Expr::Path(_) => "path",
        Expr::Range(_) => "range",
        Expr::Reference(_) => "reference",
        Expr::Repeat(_) => "repeat",
        Expr::Return(_) => "return",
        Expr::Struct(_) => "struct",
        Expr::Try(_) => "try",
        Expr::TryBlock(_) => "try block",
        Expr::Tuple(_) => "tuple",
        Expr::Unary(_) => "unary",
        Expr::Unsafe(_) => "unsafe",
        Expr::While(_) => "while",
        Expr::Yield(_) => "yield",
        _ => "other",
    }
}

fn pat_kind(p: &Pat) -> &'static str {
    match p {
        Pat::Const(_) => "const",
        Pat::Ident(_) => "ident",
        Pat::Lit(_) => "literal",
        Pat::Macro(_) => "macro",
        Pat::Or(_) => "or",
        Pat::Paren(_) => "paren",
        Pat::Path(_) => "path",
        Pat::Range(_) => "range",
        Pat::Reference(_) => "reference",
        Pat::Rest(_) => "rest",
        Pat::Slice(_) => "slice",
        Pat::Struct(_) => "struct",
        Pat::Tuple(_) => "tuple",
        Pat::TupleStruct(_) => "tuple struct",
        Pat::Type(_) => "typed",
        Pat::Wild(_) => "wild",
        _ => "other",
    }
}

/// `let x: T = …` carries its type on the pattern; split it off.
fn split_pat_ty(p: &Pat) -> (&Pat, Option<&syn::Type>) {
    match p {
        Pat::Type(pt) => (&pt.pat, Some(&pt.ty)),
        other => (other, None),
    }
}

pub fn pat_ident(p: &Pat) -> Option<String> {
    match p {
        Pat::Ident(pi) => Some(pi.ident.to_string()),
        Pat::Type(pt) => pat_ident(&pt.pat),
        Pat::Paren(pp) => pat_ident(&pp.pat),
        _ => None,
    }
}

/// A loop's resume path must start with the body segment 0.
fn strip_zero(p: &[usize]) -> Result<&[usize], RefError> {
    match p {
        [0, rest @ ..] => Ok(rest),
        other => Err(rt(format!("resume path {other:?} into a loop does not start with its body"))),
    }
}

fn arg1(args: &[Value]) -> Result<Value, RefError> {
    match args {
        [a] => Ok(a.clone()),
        _ => Err(rt(format!("expected one argument, found {}", args.len()))),
    }
}

fn as_int_ty(m: &str) -> IntTy {
    match m {
        "as_u8" => IntTy::U8,
        "as_u16" => IntTy::U16,
        "as_u32" => IntTy::U32,
        "as_u64" => IntTy::U64,
        "as_u128" => IntTy::U128,
        _ => IntTy::Usize,
    }
}

fn signed_int(t: IntTy) -> bool {
    matches!(t, IntTy::I8 | IntTy::I16 | IntTy::I32 | IntTy::I64 | IntTy::Isize)
}

fn int_bits(t: IntTy) -> u32 {
    match t {
        IntTy::U8 | IntTy::I8 => 8,
        IntTy::U16 | IntTy::I16 => 16,
        IntTy::U32 | IntTy::I32 => 32,
        IntTy::U64 | IntTy::I64 | IntTy::Usize | IntTy::Isize => 64,
        IntTy::U128 | IntTy::Unsuffixed => 128,
    }
}

/// Rust's `as`: wrap into the target's bit width.
fn wrap_int(v: i128, t: IntTy) -> i128 {
    let bits = int_bits(t);
    if bits >= 128 {
        return v;
    }
    let m = (1i128 << bits) - 1;
    let u = v & m;
    match t {
        IntTy::I8 | IntTy::I16 | IntTy::I32 | IntTy::I64 | IntTy::Isize => {
            if u >> (bits - 1) & 1 == 1 { u - (1i128 << bits) } else { u }
        }
        _ => u,
    }
}

/// An arithmetic result checked against the type's range — the debug-build panic
/// of the simulator, surfaced as a fault.
fn int_result(v: i128, t: IntTy) -> Result<Value, RefError> {
    let (lo, hi) = t.range();
    if v < lo || v > hi {
        return Err(rt(format!("integer overflow: {v} is outside {}", t.name())));
    }
    Ok(Value::Int(v, t))
}

/// Two range bounds take one integer type (an unsuffixed pair is `usize`, the
/// type a loop counter used as an index gets).
fn unify_ints(a: Value, b: Value) -> Result<(Value, Value), RefError> {
    let (av, at) = match a {
        Value::Int(v, t) => (v, t),
        other => return Err(rt(format!("range bound {other} is not an integer"))),
    };
    let (bv, bt) = match b {
        Value::Int(v, t) => (v, t),
        other => return Err(rt(format!("range bound {other} is not an integer"))),
    };
    let t = match (at, bt) {
        // Two unsuffixed bounds: the loop variable stays unsuffixed and takes its type
        // from the operand it meets (rustc infers it from the use; `i32` when nothing
        // fixes it, which an unbounded integer agrees with on every in-range value).
        (IntTy::Unsuffixed, IntTy::Unsuffixed) => IntTy::Unsuffixed,
        (IntTy::Unsuffixed, t) | (t, IntTy::Unsuffixed) => t,
        (x, y) if x == y => x,
        (x, y) => return Err(rt(format!("range bounds of different types {} and {}", x.name(), y.name()))),
    };
    Ok((Value::Int(av, t), Value::Int(bv, t)))
}

/// `matches!(expr, pat [if guard])`.
struct MatchesArgs {
    expr: Expr,
    pat: Pat,
    guard: Option<Expr>,
}

impl syn::parse::Parse for MatchesArgs {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let expr: Expr = input.parse()?;
        input.parse::<syn::Token![,]>()?;
        let pat = Pat::parse_multi_with_leading_vert(input)?;
        let guard = if input.peek(syn::Token![if]) {
            input.parse::<syn::Token![if]>()?;
            Some(input.parse()?)
        } else {
            None
        };
        let _ = input.parse::<Option<syn::Token![,]>>();
        Ok(MatchesArgs { expr, pat, guard })
    }
}

/// The static tick sites of a statement list, with the same path convention the
/// walker's `Flow::Suspend` builds — pinned against the reached set in tests.
pub fn collect_tick_sites(stmts: &[Stmt], prefix: &mut Vec<usize>, out: &mut BTreeSet<TickSite>) {
    for (i, s) in stmts.iter().enumerate() {
        prefix.push(i);
        if let Stmt::Expr(e, _) = s {
            collect_sites_expr(e, prefix, out);
        }
        prefix.pop();
    }
}

fn collect_sites_expr(e: &Expr, prefix: &mut Vec<usize>, out: &mut BTreeSet<TickSite>) {
    match e {
        Expr::Await(_) => {
            out.insert(TickSite(prefix.clone()));
        }
        Expr::If(i) => {
            prefix.push(0);
            collect_tick_sites(&i.then_branch.stmts, prefix, out);
            prefix.pop();
            if let Some((_, el)) = &i.else_branch {
                prefix.push(1);
                match &**el {
                    Expr::Block(b) => collect_tick_sites(&b.block.stmts, prefix, out),
                    Expr::If(inner) => collect_sites_expr(&Expr::If(inner.clone()), prefix, out),
                    _ => {}
                }
                prefix.pop();
            }
        }
        Expr::Match(m) => {
            for (k, arm) in m.arms.iter().enumerate() {
                prefix.push(k);
                match &*arm.body {
                    Expr::Block(b) => collect_tick_sites(&b.block.stmts, prefix, out),
                    other => {
                        prefix.push(0);
                        collect_sites_expr(other, prefix, out);
                        prefix.pop();
                    }
                }
                prefix.pop();
            }
        }
        Expr::Loop(l) => {
            prefix.push(0);
            collect_tick_sites(&l.body.stmts, prefix, out);
            prefix.pop();
        }
        Expr::While(w) => {
            prefix.push(0);
            collect_tick_sites(&w.body.stmts, prefix, out);
            prefix.pop();
        }
        Expr::ForLoop(f) => {
            prefix.push(0);
            collect_tick_sites(&f.body.stmts, prefix, out);
            prefix.pop();
        }
        Expr::Block(b) => collect_tick_sites(&b.block.stmts, prefix, out),
        _ => {}
    }
}

// ── memories (SPEC §I.10) ──────────────────────────────────────────────────

/// A `Memory` builder expression: `Memory::<T, R, W, D, L, M>::{new, from_fn,
/// from_contents}(clk, …)`, optionally followed by `.write_first()` /
/// `.read_first()` (I.10.2: the mode is the builder's). Returns the constructor
/// call and the mode the chain sets.
fn memory_ctor(e: &Expr) -> Option<(&syn::ExprCall, Option<Mode>)> {
    match e {
        Expr::Call(c) => match &*c.func {
            Expr::Path(p) if p.path.segments.first().is_some_and(|s| s.ident == "Memory") => Some((c, None)),
            _ => None,
        },
        Expr::MethodCall(mc) if mc.args.is_empty() => {
            let mode = match mc.method.to_string().as_str() {
                "write_first" => Mode::WriteFirst,
                "read_first" => Mode::ReadFirst,
                _ => return None,
            };
            // The outermost setter is the last call on the builder: it wins.
            memory_ctor(&mc.receiver).map(|(c, _)| (c, Some(mode)))
        }
        Expr::Paren(p) => memory_ctor(&p.expr),
        _ => None,
    }
}

/// `x.read_port::<K>()` / `x.write_port::<K>()`: the memory's name, read or
/// write, and the port's const-generic argument.
fn memory_port(e: &Expr) -> Option<(String, bool, &syn::GenericArgument)> {
    let Expr::MethodCall(mc) = e else { return None };
    let is_read = match mc.method.to_string().as_str() {
        "read_port" => true,
        "write_port" => false,
        _ => return None,
    };
    let Expr::Path(p) = &*mc.receiver else { return None };
    if p.path.segments.len() != 1 || !mc.args.is_empty() {
        return None;
    }
    let arg = mc.turbofish.as_ref()?.args.first()?;
    Some((p.path.segments[0].ident.to_string(), is_read, arg))
}

impl<'a> Exec<'a> {
    /// `let m = Memory(init)` in `s_pre` (I.10.1(a), I.10.3): the instance with its
    /// initial array, registered beside the environment — a variable of H(M) that
    /// is not a local.
    fn exec_memory_binding(&mut self, l: &syn::Local, init: &Expr) -> Result<(), RefError> {
        let (call, mode) = memory_ctor(init).expect("checked by the caller");
        let (pat, ann) = split_pat_ty(&l.pat);
        let Some(name) = pat_ident(pat) else {
            return Err(unsup("a `Memory` bound by a pattern"));
        };
        let Expr::Path(fp) = &*call.func else { unreachable!() };
        let segs = &fp.path.segments;
        let ctor = segs.last().map(|s| s.ident.to_string()).unwrap_or_default();
        // The instance parameters: the turbofish on `Memory`, else the annotation.
        let gargs: Vec<syn::GenericArgument> = match &segs[0].arguments {
            syn::PathArguments::AngleBracketed(a) if segs.len() >= 2 => a.args.iter().cloned().collect(),
            _ => match ann {
                Some(syn::Type::Path(tp)) => match &tp.path.segments.last().map(|s| s.arguments.clone()) {
                    Some(syn::PathArguments::AngleBracketed(a)) => a.args.iter().cloned().collect(),
                    _ => Vec::new(),
                },
                _ => Vec::new(),
            },
        };
        if gargs.len() != 6 {
            return Err(unsup(format!(
                "memory `{name}`: `Memory<T, R, W, D, READ_LAT, WRITE_LAT>` with its six parameters spelled (I.10.1(a))"
            )));
        }
        let syn::GenericArgument::Type(word_ty) = &gargs[0] else {
            return Err(RefError::Parse(format!("memory `{name}`: the word type")));
        };
        let word = self.parse_ty(word_ty)?;
        let reads = self.generic_arg_usize(&gargs[1])?;
        let writes = self.generic_arg_usize(&gargs[2])?;
        let read_lat = self.generic_arg_usize(&gargs[4])?;
        let write_lat = self.generic_arg_usize(&gargs[5])?;
        // Argument 0 is the clock the memory advances on (I.10.1(c)): the module's.
        let args: Vec<&Expr> = call.args.iter().collect();
        let init_words: Vec<Value> = match ctor.as_str() {
            // I.10.3: the all-zero array, the zero of `T`.
            "new" => {
                let depth = self.eval_expr(args.get(1).ok_or_else(|| rt("`Memory::new(clk, depth)`"))?)?.as_index()?;
                vec![Value::zero_of(&word)?; depth]
            }
            // I.10.3: the listed words.
            "from_contents" => {
                let a = args.get(1).ok_or_else(|| rt("`Memory::from_contents(clk, vec![…])`"))?;
                let Expr::Macro(m) = a else {
                    return Err(unsup("`from_contents` of something other than `vec![…]`"));
                };
                if path_name(&m.mac.path) != "vec" {
                    return Err(unsup("`from_contents` of something other than `vec![…]`"));
                }
                let arr: Expr = syn::parse_str(&format!("[{}]", m.mac.tokens))
                    .map_err(|e| RefError::Parse(format!("vec!: {e}")))?;
                match self.eval_expr(&arr)? {
                    Value::Array(v) => v,
                    other => return Err(rt(format!("`vec!` evaluated to {other}"))),
                }
            }
            // I.10.3: `A[i] = ⟦e⟧(i)`, `e` an expression in the index alone.
            "from_fn" => {
                let depth = self.eval_expr(args.get(1).ok_or_else(|| rt("`Memory::from_fn(clk, depth, |i| e)`"))?)?.as_index()?;
                let Some(Expr::Closure(cl)) = args.get(2) else {
                    return Err(unsup("`from_fn` with a third argument that is not a closure `|i| e`"));
                };
                if cl.inputs.len() != 1 {
                    return Err(unsup("a `from_fn` closure of more than one argument"));
                }
                let (ip, _) = split_pat_ty(&cl.inputs[0]);
                let iname = pat_ident(ip).ok_or_else(|| unsup("a `from_fn` closure parameter that is a pattern"))?;
                let mut words = Vec::with_capacity(depth);
                for i in 0..depth {
                    let v = self.eval_with_bindings(&cl.body, &[(iname.clone(), Value::Int(i as i128, IntTy::Usize))])?;
                    words.push(v);
                }
                words
            }
            other => return Err(unsup(format!("`Memory::{other}` (I.10.3 names `new`, `from_contents`, `from_fn`)"))),
        };
        // I.10.2: `new` / `from_contents` / `from_fn` are ReadFirst; the setter decides.
        let st = MemState::new(
            &name,
            word,
            reads,
            writes,
            read_lat,
            write_lat,
            mode.unwrap_or(Mode::ReadFirst),
            init_words,
        )?;
        let Some(mems) = self.mems.as_deref_mut() else {
            return Err(unsup("a `Memory` in a constant context"));
        };
        if mems.insert(name.clone(), st).is_some() {
            return Err(unsup(format!("a second `Memory` bound to `{name}`")));
        }
        Ok(())
    }

    /// A port access `m.read_port::<k>().{read(a), is_ready(), data()}` or
    /// `m.write_port::<j>().write(a, e)` on a memory of this module, or `None` if
    /// `mc` is not one.
    fn memory_call(&mut self, mc: &syn::ExprMethodCall) -> Result<Option<Value>, RefError> {
        let Some((name, is_read, karg)) = memory_port(&mc.receiver) else {
            return Ok(None);
        };
        if !self.mems.as_deref().is_some_and(|m| m.contains_key(&name)) {
            return Ok(None);
        }
        let k = self.generic_arg_usize(karg)?;
        let m = mc.method.to_string();
        let args: Vec<Value> = mc.args.iter().map(|a| self.eval_expr(a)).collect::<Result<_, _>>()?;
        for a in &args {
            a.check_poison()?;
        }
        let mem = self.mems.as_deref_mut().and_then(|ms| ms.get_mut(&name)).expect("checked above");
        let unit = Value::Tuple(Vec::new());
        match (is_read, m.as_str(), args.as_slice()) {
            // I.10.1(b): an effect of the resumption, consumed at the edge.
            (true, "read", [a]) => {
                mem.stage_read(k, a.as_index()?)?;
                Ok(Some(unit))
            }
            // I.10.1(d): committed state, what the last edge delivered.
            (true, "is_ready", []) => Ok(Some(Value::Bool(mem.is_ready(k)?))),
            (true, "data", []) => Ok(Some(mem.data(k)?)),
            (false, "write", [a, e]) => {
                mem.stage_write(k, a.as_index()?, e.clone())?;
                Ok(Some(unit))
            }
            _ => Err(unsup(format!(
                "`{name}.{}_port::<{k}>().{m}(…)` with {} argument(s): not a memory access of I.10.1",
                if is_read { "read" } else { "write" },
                args.len()
            ))),
        }
    }
}
