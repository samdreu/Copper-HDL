//! The `continue` of a tick-free counted `for` (SPEC I.2.4; `design_docs/review/MEASUREMENTS.md`
//! §U.1, line `II.3 found tickfree-continue` of SPEC §III.3.4).
//!
//! A tick-free `for` over a constant range is an unrolling (I.2.4), and a `continue` in its
//! body is Rust's: the rest of that iteration is skipped. The pipeline downstream lowers the
//! `for` to an SV `for` (Verilator unrolls it) and has no `continue`, so this FIR→FIR pass
//! removes it first, by placing the rest of the iteration under the negated condition:
//!
//! ```text
//! for i in a..b { A; if c { B; continue; } C; }   ⇒   for i in a..b { A; if c { B; } else { C; } }
//! ```
//!
//! In general, for the first statement `s` of a body that holds a `continue` of this loop
//! (not one of a nested loop), with `rest` the statements after it:
//!
//! * `continue;` → the statements before it (the rest is dropped);
//! * `if c { T } else { E }` → `if c { elim(T ++ rest) } else { elim(E ++ rest) }` (an absent
//!   `else` is empty);
//! * `match v { p => e, … }` → every arm's body becomes `{ elim(e ++ rest) }`;
//! * `{ S }` → `elim(S ++ rest)` in a block.
//!
//! This is the same program: every path through `s` either reaches a `continue` (and skips
//! `rest`) or falls out of `s` (and runs `rest`), and `rest` runs after exactly those paths. The
//! rest is copied into each branch, so a binding made inside a branch would come into scope for
//! the copied statements; a `rest` that names a name bound inside `s` is left untouched (the
//! `continue` stays, and `chir_lower` refuses it with a message that says why), as are a labelled
//! `continue` and one inside a `let` initializer or another expression.
//!
//! A tick-bearing `for`'s `continue` is refused by both front-ends since D-23 (II.3.3 (h)), so
//! the pass touches only tick-free `for` bodies.

use copper_core::frontend_ir::{
    ExprBlock, ExprIf, ExprStmt, ExprType, FrontendModuleIR, RawStmt, RawStmtKind,
};

/// Rewrite every tick-free `for` of the module body (at any depth).
pub fn eliminate_tickfree_continue(fir: &mut FrontendModuleIR) {
    let mut body = std::mem::take(&mut fir.raw_statements);
    walk_stmts(&mut body);
    fir.raw_statements = body;
}

fn walk_stmts(stmts: &mut [RawStmt]) {
    for s in stmts.iter_mut() {
        match &mut s.kind {
            RawStmtKind::Expr(es) => walk_expr(&mut es.expr),
            RawStmtKind::Local(l) => {
                if let Some(init) = l.init.as_mut() {
                    walk_expr(init);
                }
            }
            RawStmtKind::Item(_) => {}
        }
    }
}

fn walk_expr(e: &mut ExprType) {
    match e {
        ExprType::ForLoop(f) => {
            // Inner loops first: their `continue`s are theirs, and once rewritten they hold none.
            walk_stmts(&mut f.body);
            if !stmts_contain_tick(&f.body) {
                f.body = elim(&f.body);
            }
        }
        ExprType::Loop(l) => walk_stmts(&mut l.body),
        ExprType::While(w) => walk_stmts(&mut w.body),
        ExprType::Block(b) => walk_stmts(&mut b.stmts),
        ExprType::If(f) => {
            walk_stmts(&mut f.then_block);
            if let Some(eb) = f.else_branch.as_mut() {
                walk_expr(eb);
            }
        }
        ExprType::Match(m) => {
            for a in m.arms.iter_mut() {
                walk_expr(&mut a.body);
            }
        }
        _ => {}
    }
}

/// The body with this loop's `continue`s removed, or the body unchanged when a `continue` sits
/// where the rewrite does not reach (see the module docs).
fn elim(stmts: &[RawStmt]) -> Vec<RawStmt> {
    try_elim(stmts).unwrap_or_else(|| stmts.to_vec())
}

fn try_elim(stmts: &[RawStmt]) -> Option<Vec<RawStmt>> {
    let Some(i) = stmts.iter().position(stmt_has_continue) else {
        return Some(stmts.to_vec());
    };
    let s = &stmts[i];
    let rest = &stmts[i + 1..];
    let mut out: Vec<RawStmt> = stmts[..i].to_vec();
    let RawStmtKind::Expr(es) = &s.kind else {
        return None; // a `continue` inside a `let` initializer
    };
    let bound = bound_names(std::slice::from_ref(s));
    if bound.iter().any(|n| stmts_name(rest, n)) {
        return None; // the copied rest would see a branch's binding
    }
    let with_rest = |head: Vec<RawStmt>| -> Option<Vec<RawStmt>> {
        let mut v = head;
        v.extend(rest.iter().cloned());
        try_elim(&v)
    };
    match &es.expr {
        ExprType::Continue(c) => {
            if c.label.is_some() {
                return None;
            }
            Some(out)
        }
        ExprType::If(f) => {
            let then_block = with_rest(f.then_block.clone())?;
            let else_stmts = match f.else_branch.as_deref() {
                None => Vec::new(),
                Some(ExprType::Block(b)) => b.stmts.clone(),
                Some(other) => vec![stmt_of(other.clone(), s)],
            };
            let else_stmts = with_rest(else_stmts)?;
            let span = f.span;
            out.push(stmt_of(
                ExprType::If(ExprIf {
                    condition: f.condition.clone(),
                    then_block,
                    else_branch: Some(Box::new(ExprType::Block(ExprBlock { stmts: else_stmts, span }))),
                    span,
                }),
                s,
            ));
            Some(out)
        }
        ExprType::Match(m) => {
            let mut m = m.clone();
            for a in m.arms.iter_mut() {
                let head = match &*a.body {
                    ExprType::Block(b) => b.stmts.clone(),
                    other => vec![stmt_of(other.clone(), s)],
                };
                let span = a.span;
                a.body = Box::new(ExprType::Block(ExprBlock { stmts: with_rest(head)?, span }));
            }
            out.push(stmt_of(ExprType::Match(m), s));
            Some(out)
        }
        ExprType::Block(b) => {
            let span = b.span;
            let stmts = with_rest(b.stmts.clone())?;
            out.push(stmt_of(ExprType::Block(ExprBlock { stmts, span }), s));
            Some(out)
        }
        _ => None, // a `continue` inside another expression
    }
}

fn stmt_of(expr: ExprType, like: &RawStmt) -> RawStmt {
    RawStmt {
        order: like.order,
        kind: RawStmtKind::Expr(ExprStmt { expr, has_semi: true, span: like.span }),
        text: like.text.clone(),
        span: like.span,
    }
}

/// Whether the statement holds a `continue` of the loop it is in (not of a nested loop).
fn stmt_has_continue(s: &RawStmt) -> bool {
    match &s.kind {
        RawStmtKind::Expr(es) => expr_has_continue(&es.expr),
        RawStmtKind::Local(l) => l.init.as_ref().is_some_and(expr_has_continue),
        RawStmtKind::Item(_) => false,
    }
}

fn expr_has_continue(e: &ExprType) -> bool {
    match e {
        ExprType::Continue(_) => true,
        ExprType::ForLoop(_) | ExprType::Loop(_) | ExprType::While(_) | ExprType::Closure(_) => false,
        ExprType::Block(b) => b.stmts.iter().any(stmt_has_continue),
        ExprType::If(f) => {
            expr_has_continue(&f.condition)
                || f.then_block.iter().any(stmt_has_continue)
                || f.else_branch.as_deref().is_some_and(expr_has_continue)
        }
        ExprType::Match(m) => {
            expr_has_continue(&m.scrutinee) || m.arms.iter().any(|a| expr_has_continue(&a.body))
        }
        ExprType::Assign(a) => expr_has_continue(&a.left) || expr_has_continue(&a.right),
        ExprType::Binary(b) => expr_has_continue(&b.left) || expr_has_continue(&b.right),
        ExprType::Unary(u) => expr_has_continue(&u.expr),
        ExprType::MethodCall(mc) => {
            expr_has_continue(&mc.receiver) || mc.args.iter().any(expr_has_continue)
        }
        ExprType::Call(c) => c.args.iter().any(expr_has_continue),
        ExprType::Tuple(t) => t.elements.iter().any(expr_has_continue),
        ExprType::Array(a) => a.elements.iter().any(expr_has_continue),
        _ => false,
    }
}

/// Every name a `let` inside these statements binds (at any depth).
fn bound_names(stmts: &[RawStmt]) -> Vec<String> {
    let mut out = Vec::new();
    for s in stmts {
        collect_bound(s, &mut out);
    }
    out
}

fn collect_bound(s: &RawStmt, out: &mut Vec<String>) {
    match &s.kind {
        RawStmtKind::Local(l) => {
            out.push(l.name.clone());
            if let Some(i) = &l.init {
                collect_bound_expr(i, out);
            }
        }
        RawStmtKind::Expr(es) => collect_bound_expr(&es.expr, out),
        RawStmtKind::Item(_) => {}
    }
}

fn collect_bound_expr(e: &ExprType, out: &mut Vec<String>) {
    match e {
        ExprType::Block(b) => b.stmts.iter().for_each(|s| collect_bound(s, out)),
        ExprType::If(f) => {
            f.then_block.iter().for_each(|s| collect_bound(s, out));
            if let Some(eb) = f.else_branch.as_deref() {
                collect_bound_expr(eb, out);
            }
        }
        ExprType::Match(m) => {
            for a in &m.arms {
                // A binding pattern's names: every identifier-like token of the pattern.
                out.extend(words(&a.pattern_text));
                collect_bound_expr(&a.body, out);
            }
        }
        _ => {}
    }
}

/// Whether a statement's source text names `name` as a whole token (conservative: a field or
/// method of that spelling counts too, which only ever leaves a `continue` unrewritten).
fn stmts_name(stmts: &[RawStmt], name: &str) -> bool {
    stmts.iter().any(|s| words(&s.text).any(|w| w == name))
}

fn words(text: &str) -> impl Iterator<Item = String> + '_ {
    text.split(|c: char| !(c.is_alphanumeric() || c == '_'))
        .filter(|w| !w.is_empty() && *w != "_" && w.chars().next().is_some_and(|c| c.is_lowercase() || c == '_'))
        .map(str::to_string)
}

fn stmts_contain_tick(stmts: &[RawStmt]) -> bool {
    stmts.iter().any(|s| match &s.kind {
        RawStmtKind::Expr(es) => expr_contains_tick(&es.expr),
        RawStmtKind::Local(l) => l.init.as_ref().is_some_and(expr_contains_tick),
        RawStmtKind::Item(_) => false,
    })
}

fn expr_contains_tick(e: &ExprType) -> bool {
    match e {
        ExprType::Await(a) => {
            matches!(&*a.base, ExprType::MethodCall(mc) if mc.method == "tick" && mc.args.is_empty())
        }
        ExprType::ForLoop(f) => stmts_contain_tick(&f.body),
        ExprType::Loop(l) => stmts_contain_tick(&l.body),
        ExprType::While(w) => stmts_contain_tick(&w.body),
        ExprType::Block(b) => stmts_contain_tick(&b.stmts),
        ExprType::If(f) => {
            stmts_contain_tick(&f.then_block) || f.else_branch.as_deref().is_some_and(expr_contains_tick)
        }
        ExprType::Match(m) => m.arms.iter().any(|a| expr_contains_tick(&a.body)),
        _ => false,
    }
}
