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
