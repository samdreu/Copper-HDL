//! Register power-on values (2026-10-29).
//!
//! A pre-loop `let mut` the liveness inference calls a register is a flip-flop,
//! and its initializer is the flip-flop's **power-on value**: the simulator starts
//! the binding there, and the transpiler states the same value in the emitted
//! SystemVerilog as an `initial` assignment (`emit::reg_inits`). That is only
//! possible for a value the transpiler can state without running Rust — a
//! **constant** ([`is_constant_expr`]: a literal, `Bits::from_u32(k)`,
//! `Bits::<N>::from_u8(k)`, `Bits::from_lit::<K>()`, `Bits::zero()`, an enum
//! variant, arithmetic over those) — or for `Bits::x()`, which says the register
//! starts *undefined*: there is nothing to state, and the design needs a reset
//! before the register is read. Anything else — an input read, a local, a call
//! through a free function — is a value the hardware does not have at power-on.
//! Until this rule the transpiler dropped such an initializer silently, and so,
//! worse, every constructor-call constant: `lfsr`'s `Bits::from_u32(1)` never
//! reached its SV (found by the multi-sequence sweep 2026-10-28), and the ~120
//! `Bits::zero()` registers in the corpus agreed with Verilator only because its
//! two-state model zero-fills. Refused in **both** front-ends, with no opt-out.
//!
//! **One definition of "constant".** The predicate here is the syntactic one
//! `Cfg::out_first_driven_late` already applies to a write's operand, exported so
//! the transpiler asks the same question. Codegen then *evaluates* the constant
//! (`chir_lower::lower_register_init`) to a literal of the register's width. The
//! evaluator folds every spelling the corpus uses; a shape this predicate accepts
//! but the evaluator cannot fold — a `const` item whose value is not an integer
//! literal, an associated const, a const generic as a value, a constant free
//! function — is a lowering limitation the transpiler reports as an unsupported
//! construct, never a dropped value. The corpus audit
//! (`copper-analysis/tests/register_initializer_corpus.rs`) pins that every
//! swept module's initializers pass here, and the sweep itself proves they fold
//! there (a refused module fails its case).

use syn::spanned::Spanned;
use syn::{Expr, ItemFn, Pat, Stmt};

pub use crate::cfg::is_constant_expr;

/// Pre-loop register bindings whose initializer is **not** a constant: every
/// `let mut <name> = <init>;` before the module's top-level `loop` with `name` in
/// `registers` (the liveness inference's set, [`crate::Cfg::registers`]) and
/// `!is_constant_expr(init)`, as `(name, init)` in source order. A binding the
/// inference calls a wire (never reassigned in the loop) is a combinational
/// constant/wire and not asked about; a register born *inside* the loop is
/// assigned before it is read on every path and has no power-on value to state.
pub fn non_constant_register_initializers<'a>(
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
                let Some(init) = &l.init else { continue };
                if !is_constant_expr(&init.expr) {
                    out.push((name, &*init.expr));
                }
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

/// The rule: `Err` (spanned at the initializer) for the first register whose
/// initializer is not a constant, with [`register_init_message`]. `registers` is
/// the module's inferred register set, so a caller holding a built
/// [`crate::Cfg`] does not rebuild it (the macro builds one graph per module).
pub fn check_register_initializers(f: &ItemFn, registers: &[String]) -> Result<(), syn::Error> {
    match non_constant_register_initializers(f, registers).first() {
        None => Ok(()),
        Some((name, init)) => Err(syn::Error::new(init.span(), register_init_message(name, init))),
    }
}

/// The message both front-ends print, verbatim.
pub fn register_init_message(name: &str, init: &Expr) -> String {
    format!(
        "register `{name}` is initialized from {what}, which is not a constant, so the \
         hardware has no power-on value for it: the simulator would start `{name}` at \
         whatever the expression evaluates to, and the emitted SystemVerilog cannot state \
         that value. Initialize `{name}` with a constant — an integer literal, \
         `Bits::from_u32(k)`, `Bits::<N>::from_u8(k)`, `Bits::from_lit::<K>()`, \
         `Bits::zero()`, an enum variant — or with `Bits::x()` to say it starts undefined \
         and must be reset before it is read",
        what = describe(init)
    )
}

/// What kind of non-constant the initializer is, for the message.
fn describe(e: &Expr) -> &'static str {
    match e {
        Expr::MethodCall(_) => "a method call",
        Expr::Call(_) => "a call through a single-segment path (a free function)",
        Expr::Path(_) => "a local or parameter",
        Expr::Field(_) => "a field read",
        Expr::Index(_) => "an index expression",
        Expr::Macro(_) => "a macro invocation",
        Expr::If(_) | Expr::Match(_) | Expr::Block(_) => "a block or branch expression",
        Expr::Binary(_) | Expr::Unary(_) | Expr::Cast(_) | Expr::Paren(_) => {
            "an expression over non-constants"
        }
        Expr::Array(_) | Expr::Repeat(_) => "an array expression over non-constants",
        Expr::Struct(_) => "a struct literal",
        _ => "a non-constant expression",
    }
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

    #[test]
    fn every_constant_spelling_is_accepted() {
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
        ] {
            let body = format!("let mut r = {init}; loop {{ clk.tick().await; r = r + a.read(); o.write(r); }}");
            assert_eq!(flagged(&body), Vec::<String>::new(), "{init}");
        }
    }

    #[test]
    fn a_non_constant_initializer_on_a_register_is_flagged() {
        for init in ["a.read()", "seed()", "k", "a.read() + 1", "if x { 1 } else { 0 }"] {
            let body = format!("let mut r = {init}; loop {{ clk.tick().await; r = r + a.read(); o.write(r); }}");
            assert_eq!(flagged(&body), vec!["r".to_string()], "{init}");
        }
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
        assert!(msg.starts_with("register `r` is initialized from a method call"), "{msg}");
        assert!(msg.contains("`Bits::x()`"), "{msg}");
        let f = fn_of("let mut r = Bits::zero(); loop { clk.tick().await; r = r + a.read(); o.write(r); }");
        assert!(check_register_initializers(&f, &crate::infer_registers(&f)).is_ok());
    }
}
