//! Width constraints as elaboration-time checks (SPEC I.14.5(d), D-19 5A; I.14.7 (8),
//! Phase C step 8, `design_docs/review/MEASUREMENTS.md` §M).
//!
//! A generic module's `const { assert!(N_LOG == safe_clog2(N), "…") }` makes an
//! instance that violates it not exist on the Rust side (rustc refuses it at
//! monomorphization). The transpiler emits ONE parametric SV module for the family,
//! which until step 8 stated nothing, so an SV instance at a violating parameter
//! elaborated silently (`rotate_right` at `N = 8, N_LOG = 2`). Each assertion is now
//! emitted as a generate-time check in the module body:
//!
//! ```text
//! if (!((N_LOG == $clog2(N)))) begin : __copper_width_check_0
//!     $error("N_LOG must equal safe_clog2(N)");
//! end
//! ```
//!
//! The condition is translated from the Rust expression: parameters and file
//! constants by name, integer literals, `+ - * / %`, comparisons, `&& || !`,
//! parentheses, and a call to a file `const fn` that the shared evaluator measures
//! equal to `$clog2` on `0..=4096` (the corpus's `safe_clog2`). An assertion outside
//! that grammar is reported by [`width_checks`]'s second component and emits no
//! check — the III.2 precondition (the SV instantiated only where the Rust instance
//! compiles) still covers it.

use copper_core::vlir::VLIRWidthCheck;
use syn::{Expr, ItemFn};

/// The width checks of `f`'s `const { assert!(…) }` statements, and the source text
/// of each assertion that could not be translated.
pub fn width_checks(f: &ItemFn, file_const_source: &[String]) -> (Vec<VLIRWidthCheck>, Vec<String>) {
    let env = copper_analysis::consts::ConstEnv::from_items(file_const_source.iter().map(String::as_str));
    let mut checks = Vec::new();
    let mut untranslated = Vec::new();
    for stmt in &f.block.stmts {
        let e = match stmt {
            syn::Stmt::Expr(e, _) => e,
            _ => continue,
        };
        let Expr::Const(c) = e else { continue };
        for s in &c.block.stmts {
            let mac = match s {
                syn::Stmt::Macro(m) => &m.mac,
                syn::Stmt::Expr(Expr::Macro(m), _) => &m.mac,
                _ => continue,
            };
            if !mac.path.is_ident("assert") {
                continue;
            }
            let parser = syn::punctuated::Punctuated::<Expr, syn::Token![,]>::parse_terminated;
            let Ok(args) = syn::parse::Parser::parse2(parser, mac.tokens.clone()) else {
                untranslated.push(mac.tokens.to_string());
                continue;
            };
            let Some(cond) = args.first() else { continue };
            let message = match args.iter().nth(1) {
                Some(Expr::Lit(syn::ExprLit { lit: syn::Lit::Str(s), .. })) => s.value(),
                _ => format!("width constraint `{}` violated", quote_expr(cond)),
            };
            match to_sv(cond, &env) {
                Some(cond_sv) => checks.push(VLIRWidthCheck { cond: cond_sv, message }),
                None => untranslated.push(quote_expr(cond)),
            }
        }
    }
    (checks, untranslated)
}

fn quote_expr(e: &Expr) -> String {
    quote::ToTokens::to_token_stream(e).to_string()
}

/// Is the file `const fn` `name` the ceiling log2 that `$clog2` computes, measured on
/// `0..=4096` with the shared evaluator (I.14.4)? `$clog2(0) = $clog2(1) = 0`.
fn is_clog2(name: &str, env: &copper_analysis::consts::ConstEnv) -> bool {
    if !env.is_const_fn(name) {
        return false;
    }
    (0i128..=4096).all(|n| {
        let want = if n <= 1 { 0 } else { 128 - ((n - 1) as u128).leading_zeros() as i128 };
        env.call(name, &[n]) == Some(want)
    })
}

fn to_sv(e: &Expr, env: &copper_analysis::consts::ConstEnv) -> Option<String> {
    Some(match e {
        Expr::Paren(p) => format!("({})", to_sv(&p.expr, env)?),
        Expr::Lit(syn::ExprLit { lit: syn::Lit::Int(i), .. }) => i.base10_parse::<u128>().ok()?.to_string(),
        Expr::Lit(syn::ExprLit { lit: syn::Lit::Bool(b), .. }) => (if b.value { "1" } else { "0" }).to_string(),
        Expr::Path(p) => p.path.get_ident()?.to_string(),
        Expr::Unary(u) if matches!(u.op, syn::UnOp::Not(_)) => format!("(!{})", to_sv(&u.expr, env)?),
        Expr::Binary(b) => {
            use syn::BinOp::*;
            let op = match b.op {
                Add(_) => "+",
                Sub(_) => "-",
                Mul(_) => "*",
                Div(_) => "/",
                Rem(_) => "%",
                Eq(_) => "==",
                Ne(_) => "!=",
                Lt(_) => "<",
                Le(_) => "<=",
                Gt(_) => ">",
                Ge(_) => ">=",
                And(_) => "&&",
                Or(_) => "||",
                _ => return None,
            };
            format!("({} {op} {})", to_sv(&b.left, env)?, to_sv(&b.right, env)?)
        }
        Expr::Call(c) if c.args.len() == 1 => {
            let Expr::Path(p) = &*c.func else { return None };
            let name = p.path.get_ident()?.to_string();
            if !is_clog2(&name, env) {
                return None;
            }
            format!("$clog2({})", to_sv(&c.args[0], env)?)
        }
        _ => return None,
    })
}
