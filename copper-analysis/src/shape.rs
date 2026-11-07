//! Source-shape rules both front-ends enforce (phase 2 of the tick-dialect
//! unification, 2026-10-23): a nested loop must end at a clock boundary, and the
//! only `await` a hardware body may write is `<clock>.tick().await`.
//!
//! Both used to be refused by the transpiler alone (`chir_lower` /
//! `control_extract`), so a module could compile and simulate and then be refused
//! when transpiled — the transpiler's refusal set was the de-facto language. They
//! live here, on the `syn::ItemFn` both front-ends already hold, and the macro
//! calls them too; the transpiler keeps its own copies as defence in depth, the
//! way it keeps `check_reachability`.

use proc_macro2::Span;
use syn::spanned::Spanned;
use syn::{Expr, ItemFn, Stmt};

use crate::cfg::{stmt_contains_tick, tick_clock};

/// Every nested loop under the module's top-level `loop` must **end at a clock
/// boundary**, and a `while` or a nested `loop` must contain a tick at all.
///
/// A nested `loop`/`while`/`for` whose body contains a `clk.tick().await` is a
/// repeating wait: its body is one segment of code between two ticks, so its last
/// statement must be the bare tick — or another tick-bearing loop that itself ends
/// at one. The other ordering, `loop { tick; <test> }`, is **outside the language
/// by decision** (2026-08-24): it puts the test in the window after the entering
/// edge, where a simulator samples the value the just-past edge produced and a
/// flip-flop samples the value present before its own edge — measured a full
/// cycle apart on `loop { clk.tick().await; if go { break; } }`, and not
/// reconciled by holding the stimulus. Ticks that live only inside `if`/`match`
/// arms of the body give it no single boundary and are refused too.
///
/// A tick-free nested `loop` never terminates in hardware and a tick-free `while`
/// is a combinational loop with no trip count; both are refused (a tick-free `for`
/// over a constant range unrolls and is fine). The messages are the transpiler's
/// own (`chir_lower::nested_loop_error`, `lower_while`), so the two front-ends
/// say the same thing.
pub fn check_nested_loop_shape(f: &ItemFn) -> Result<(), syn::Error> {
    let Some(body) = top_level_loop_body(f) else { return Ok(()) };
    check_stmts(body)
}

fn top_level_loop_body(f: &ItemFn) -> Option<&[Stmt]> {
    f.block.stmts.iter().find_map(|s| match s {
        Stmt::Expr(Expr::Loop(l), _) => Some(l.body.stmts.as_slice()),
        _ => None,
    })
}

fn check_stmts(stmts: &[Stmt]) -> Result<(), syn::Error> {
    for s in stmts {
        check_stmt(s)?;
    }
    Ok(())
}

fn check_stmt(s: &Stmt) -> Result<(), syn::Error> {
    match s {
        Stmt::Expr(e, _) => check_expr(e),
        Stmt::Local(l) => match &l.init {
            Some(init) => check_expr(&init.expr),
            None => Ok(()),
        },
        _ => Ok(()),
    }
}

fn check_expr(e: &Expr) -> Result<(), syn::Error> {
    match e {
        Expr::Loop(l) => {
            check_nested(&l.body.stmts, e.span(), LoopKind::Loop)?;
            check_stmts(&l.body.stmts)
        }
        Expr::While(w) => {
            check_nested(&w.body.stmts, e.span(), LoopKind::While)?;
            check_stmts(&w.body.stmts)
        }
        Expr::ForLoop(fl) => {
            check_nested(&fl.body.stmts, e.span(), LoopKind::For)?;
            check_stmts(&fl.body.stmts)
        }
        Expr::If(i) => {
            check_stmts(&i.then_branch.stmts)?;
            if let Some((_, eb)) = &i.else_branch {
                check_expr(eb)?;
            }
            Ok(())
        }
        Expr::Match(m) => {
            for arm in &m.arms {
                check_expr(&arm.body)?;
            }
            Ok(())
        }
        Expr::Block(b) => check_stmts(&b.block.stmts),
        Expr::Unsafe(u) => check_stmts(&u.block.stmts),
        _ => Ok(()),
    }
}

#[derive(Clone, Copy, PartialEq)]
enum LoopKind {
    Loop,
    While,
    For,
}

fn check_nested(body: &[Stmt], span: Span, kind: LoopKind) -> Result<(), syn::Error> {
    let has_tick = body.iter().any(stmt_contains_tick);
    if !has_tick {
        return match kind {
            LoopKind::For => Ok(()),
            LoopKind::While => Err(syn::Error::new(
                span,
                "a `while` loop with no `clk.tick().await` is combinational, so it must \
                 unroll — which needs a trip count known at compile time. Write it as a \
                 `for` over a constant range",
            )),
            LoopKind::Loop => Err(syn::Error::new(
                span,
                "a nested `loop` with no `clk.tick().await` would never terminate in \
                 hardware. Give the loop a `clk.tick().await`, or write it as a `for` over \
                 a constant range if it is meant to unroll",
            )),
        };
    }
    if ends_at_clock_boundary(body) {
        return Ok(());
    }
    if kind == LoopKind::For {
        // The transpiler's own wording (`control_extract::unflattenable_in_expr`):
        // the repeating-wait advice below does not apply to a counted `for`.
        return Err(syn::Error::new(
            span,
            "a `clk.tick().await` inside a `for` whose repetition cannot be counted — \
             the header must be an exclusive `start..end` range and the tick must be \
             the LAST statement of the body. Write it as \
             `for <var> in <start>..<end> { <work>; clk.tick().await; }`",
        ));
    }
    let has_statement_tick = body.iter().any(is_tick_stmt);
    // Ticks only inside `if`/`match` arms of a nested `loop`: each is a suspension
    // site like any other (SPEC I.8.2); the extraction lowers the loop with a
    // zero-time return to its head. Refused here ("clause 4") until Phase C step 5
    // (2026-11-06, II.3 (b), MEASUREMENTS §J.5).
    if !has_statement_tick && kind == LoopKind::Loop && ticks_only_in_arms(body) {
        return Ok(());
    }
    if has_statement_tick {
        Err(syn::Error::new(
            span,
            "a repeating wait must be written as `loop { <test>; clk.tick().await; }` — \
             the tick has to be the LAST statement of the loop body. Testing AFTER the \
             tick reads an input in the window where a simulator samples the value the \
             just-past edge produced and a flip-flop samples the value present before \
             its own edge; the two are a cycle apart under any testbench that changes \
             inputs between edges. Copper does not choose between them — the ordering is \
             outside the language, the same way a pre-tick alignment hazard is. Move the \
             test before the tick: `loop { if ready { break; } clk.tick().await; }`",
        ))
    } else {
        Err(syn::Error::new(
            span,
            "this nested loop's body has no clock boundary of its own: every \
             `clk.tick().await` in it sits inside an `if` or `match` arm, so the body is \
             not one segment of code between two ticks and there is no state to build \
             from it. A nested loop's body must END at its boundary — either \
             `clk.tick().await` as the last statement, or another tick-bearing loop as \
             the last statement. Lift the tick out of the branches so it is the body's \
             last statement",
        ))
    }
}

/// `control_extract::body_ends_at_a_clock_boundary`, on the source: the last
/// statement is a bare tick, or — with no bare tick anywhere in the body — a
/// nested tick-bearing loop that itself ends at a boundary.
fn ends_at_clock_boundary(body: &[Stmt]) -> bool {
    match body.iter().position(is_tick_stmt) {
        Some(t) => t + 1 == body.len(),
        None => body.last().is_some_and(|last| match last {
            Stmt::Expr(Expr::Loop(l), _) => ends_at_clock_boundary(&l.body.stmts),
            Stmt::Expr(Expr::While(w), _) => ends_at_clock_boundary(&w.body.stmts),
            Stmt::Expr(Expr::ForLoop(fl), _) => ends_at_clock_boundary(&fl.body.stmts),
            _ => false,
        }),
    }
}

/// Every top-level statement that contains a tick is an `if` or a `match`.
fn ticks_only_in_arms(body: &[Stmt]) -> bool {
    body.iter().filter(|s| stmt_contains_tick(s)).all(|s| {
        matches!(s, Stmt::Expr(Expr::If(_), _) | Stmt::Expr(Expr::Match(_), _))
    })
}

fn is_tick_stmt(s: &Stmt) -> bool {
    matches!(s, Stmt::Expr(e, _) if tick_clock(e).is_some())
}

/// The only `await` a hardware body may write is a direct `<clock>.tick().await`.
///
/// Any other awaited value has no hardware meaning; the simulator would run it
/// with whatever the executor happens to do, and the transpiler refuses it
/// (`chir_lower`: "await on non-clock value"). Checked on the pristine function,
/// before the macro injects its own `pre_edge_barrier::<D>().await`.
/// SPEC II.1.2 **R21**: a `tick` before the module's top-level `loop` is OUTSIDE —
/// the statements before the loop are the first resumption's and are tick-free
/// (I.2.2's `s_pre`, I.9.3). One message for both front-ends (the transpiler's
/// `chir_lower` keeps a copy as defence in depth). Before Phase C step 5
/// (2026-11-07) the macro admitted it and the linear route dropped the tick (II.3
/// (f6)); MEASUREMENTS §J.12.
pub fn check_no_preloop_tick(f: &ItemFn) -> Result<(), syn::Error> {
    for s in &f.block.stmts {
        if matches!(s, Stmt::Expr(Expr::Loop(_), _)) {
            return Ok(());
        }
        if stmt_contains_tick(s) {
            return Err(syn::Error::new(s.span(), PRELOOP_TICK_MESSAGE));
        }
    }
    Ok(())
}

/// A tick-bearing counted `for` whose body ENDS in a tick-bearing counted `for`
/// with a bound that is not static (an integer literal, or constant-shaped names —
/// `is_constant_expr` — none of them a const generic of the module): with the inner
/// range empty, each enclosing iteration takes zero time, which the emitted state
/// machine cannot run without unrolling the enclosing loop. A dated II.3 limitation,
/// REFUSED in both front-ends with one message (Phase C step 5, 2026-11-07,
/// MEASUREMENTS §J.15).
pub fn check_counted_for_nesting(f: &ItemFn) -> Result<(), syn::Error> {
    let generics: Vec<String> = f
        .sig
        .generics
        .params
        .iter()
        .filter_map(|p| match p {
            syn::GenericParam::Const(c) => Some(c.ident.to_string()),
            _ => None,
        })
        .collect();
    struct V<'g> {
        generics: &'g [String],
        err: Option<syn::Error>,
    }
    impl<'ast> syn::visit::Visit<'ast> for V<'_> {
        fn visit_expr_for_loop(&mut self, fl: &'ast syn::ExprForLoop) {
            if self.err.is_none() && fl.body.stmts.iter().any(stmt_contains_tick) {
                if let Some(Stmt::Expr(Expr::ForLoop(inner), _)) = fl.body.stmts.last() {
                    if inner.body.stmts.iter().any(stmt_contains_tick) && !range_is_static(&inner.expr, self.generics) {
                        self.err = Some(syn::Error::new(inner.span(), NESTED_RUNTIME_EMPTY_FOR_MESSAGE));
                    }
                }
            }
            syn::visit::visit_expr_for_loop(self, fl);
        }
    }
    let mut v = V { generics: &generics, err: None };
    syn::visit::Visit::visit_block(&mut v, &f.block);
    v.err.map_or(Ok(()), Err)
}

/// Both bounds of an exclusive range are static (see `check_counted_for_nesting`).
fn range_is_static(e: &Expr, generics: &[String]) -> bool {
    let Expr::Range(r) = e else { return false };
    let names_generic = |x: &Expr| {
        let mut hit = false;
        struct P<'a>(&'a [String], &'a mut bool);
        impl<'ast> syn::visit::Visit<'ast> for P<'_> {
            fn visit_path(&mut self, p: &'ast syn::Path) {
                if p.get_ident().is_some_and(|i| self.0.iter().any(|g| i == g)) {
                    *self.1 = true;
                }
                syn::visit::visit_path(self, p);
            }
        }
        syn::visit::Visit::visit_expr(&mut P(generics, &mut hit), x);
        hit
    };
    [r.start.as_deref(), r.end.as_deref()].into_iter().flatten().all(|b| {
        crate::cfg::is_constant_expr(b) && !names_generic(b)
    })
}

/// The nesting rule's message, shared by both front-ends.
pub const NESTED_RUNTIME_EMPTY_FOR_MESSAGE: &str = "a counted `for` whose range may be empty at \
    run time is the last statement of an enclosing tick-bearing `for`: with an empty inner range \
    the enclosing loop's iterations take no time, which the emitted state machine cannot realize. \
    End the enclosing `for` body with its own `clk.tick().await`, or make the inner bound a \
    constant (a literal or a `const`)";

/// R21's message, shared by both front-ends.
pub const PRELOOP_TICK_MESSAGE: &str = "`clk.tick().await` before the module's `loop`: the \
    statements before the loop are the first resumption's and run in no time (SPEC I.2.2, \
    I.9.3). Move the tick into the loop";

pub fn check_awaits(f: &ItemFn) -> Result<(), syn::Error> {
    struct Awaits(Option<syn::Error>);
    impl<'ast> syn::visit::Visit<'ast> for Awaits {
        fn visit_expr_await(&mut self, node: &'ast syn::ExprAwait) {
            if self.0.is_none() && tick_clock(&Expr::Await(node.clone())).is_none() {
                self.0 = Some(syn::Error::new(
                    node.span(),
                    "await on a non-clock value: the only `await` a hardware module may \
                     write is `clk.tick().await`, which waits for a clock edge. Nothing \
                     else has a hardware meaning",
                ));
            }
            syn::visit::visit_expr_await(self, node);
        }
    }
    let mut v = Awaits(None);
    syn::visit::Visit::visit_block(&mut v, &f.block);
    match v.0 {
        Some(e) => Err(e),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(src: &str) -> ItemFn {
        syn::parse_str(src).expect("parse ItemFn")
    }

    #[test]
    fn test_first_wait_is_accepted() {
        let f = parse(
            "async fn m(clk: Clock<C>, go: In<Logic, C>) { loop { loop { if go.read() == Logic::One { break; } clk.tick().await; } clk.tick().await; } }",
        );
        assert!(check_nested_loop_shape(&f).is_ok());
    }

    #[test]
    fn tick_first_wait_is_refused_with_the_ordering_rule() {
        let f = parse(
            "async fn m(clk: Clock<C>, go: In<Logic, C>) { loop { loop { clk.tick().await; if go.read() == Logic::One { break; } } clk.tick().await; } }",
        );
        let msg = check_nested_loop_shape(&f).unwrap_err().to_string();
        assert!(msg.contains("LAST statement"), "{msg}");
    }

    #[test]
    fn counted_delay_and_while_wait_are_accepted() {
        let f = parse(
            "async fn m(clk: Clock<C>, rx: In<Logic, C>) { loop { while rx.read() == Logic::One { clk.tick().await; } for _ in 0..8 { clk.tick().await; } clk.tick().await; } }",
        );
        assert!(check_nested_loop_shape(&f).is_ok());
    }

    #[test]
    fn body_ending_in_a_nested_wait_is_accepted() {
        let f = parse(
            "async fn m(clk: Clock<C>, go: In<Logic, C>) { loop { loop { let x = 1; while go.read() == Logic::Zero { clk.tick().await; } } } }",
        );
        assert!(check_nested_loop_shape(&f).is_ok());
    }

    /// Clause 4 lifted (Phase C step 5, II.3 (b)): a nested `loop` whose ticks lie only
    /// in arms is admitted; a `while` of that shape keeps the clause's message.
    #[test]
    fn ticks_only_in_arms_of_a_loop_are_accepted_and_of_a_while_refused() {
        let f = parse(
            "async fn m(clk: Clock<C>, c: In<Logic, C>) { loop { loop { if c.read() == Logic::One { clk.tick().await; } else { clk.tick().await; } } } }",
        );
        assert!(check_nested_loop_shape(&f).is_ok());
        let f = parse(
            "async fn m(clk: Clock<C>, c: In<Logic, C>) { loop { while c.read() == Logic::One { if c.read() == Logic::One { clk.tick().await; } else { clk.tick().await; } } clk.tick().await; } }",
        );
        let msg = check_nested_loop_shape(&f).unwrap_err().to_string();
        assert!(msg.contains("no clock boundary of its own"), "{msg}");
    }

    #[test]
    fn tickless_nested_loop_and_while_are_refused_but_for_unrolls() {
        let f = parse("async fn m(clk: Clock<C>) { loop { loop { break; } clk.tick().await; } }");
        assert!(check_nested_loop_shape(&f).unwrap_err().to_string().contains("never terminate"));
        let f = parse("async fn m(clk: Clock<C>, c: In<Logic, C>) { loop { while c.read() == Logic::One { } clk.tick().await; } }");
        assert!(check_nested_loop_shape(&f).unwrap_err().to_string().contains("must unroll"));
        let f = parse("async fn m(clk: Clock<C>) { loop { for i in 0..4 { let _ = i; } clk.tick().await; } }");
        assert!(check_nested_loop_shape(&f).is_ok());
    }

    #[test]
    fn foreign_await_is_refused_and_ticks_are_not() {
        let f = parse("async fn m(clk: Clock<C>) { loop { clk.tick().await; } }");
        assert!(check_awaits(&f).is_ok());
        let f = parse("async fn m(clk: Clock<C>, fut: F) { loop { fut.await; clk.tick().await; } }");
        assert!(check_awaits(&f).unwrap_err().to_string().contains("non-clock"));
        let f = parse("async fn m(clk: Clock<C>) { loop { delta_yield().await; clk.tick().await; } }");
        assert!(check_awaits(&f).is_err());
    }
}
