//! The register fold's delegation to the shared constant evaluator for the two
//! spellings it does not fold itself (2026-11-08, `design_docs/review/MEASUREMENTS.md`
//! §V.4, line `II.3 found const-call`): a `const` integer method call
//! (`K.wrapping_add(1)`) and a tuple-struct field (`K.0`, `W(7).0`). rustc admits
//! both in the macro's `const { }`; `chir_lower::fold_const_init` calls this from one
//! arm, passing itself as `fold` for the sub-expressions it does fold.

use copper_analysis::consts::{int_method, ConstEnv, IntTy};
use copper_core::frontend_ir::ExprType;

fn compact(s: &str) -> String {
    s.chars().filter(|c| !c.is_whitespace()).collect()
}

/// The integer type a FIR expression shows on its own (a typed const, `static` or
/// tuple-struct const field, a suffixed literal, a cast, a method's receiver).
fn ty_hint(e: &ExprType, env: &ConstEnv) -> Option<IntTy> {
    match e {
        ExprType::Path(p) => env.type_of(&compact(&p.path_text)),
        ExprType::Lit(l) => {
            let t = compact(&l.text);
            let at = t.find(['u', 'i'])?;
            IntTy::of_name(&t[at..])
        }
        ExprType::Cast(c) => IntTy::of_name(&compact(&c.target_ty.ty_text)),
        ExprType::Field(f) => match &*f.base {
            ExprType::Path(p) => env.type_of(&format!("{}.{}", compact(&p.path_text), f.member)),
            _ => None,
        },
        ExprType::MethodCall(m) => match m.method.as_str() {
            "count_ones" | "count_zeros" | "leading_zeros" | "trailing_zeros" => IntTy::of_name("u32"),
            _ => ty_hint(&m.receiver, env).or_else(|| m.args.iter().find_map(|a| ty_hint(a, env))),
        },
        _ => None,
    }
}

/// Fold a method call or a field access of a register initializer to its value's bit
/// pattern (masked by the caller). `width` is the register's: the type a receiver
/// with none of its own takes (rustc infers it from the annotation), unsigned.
pub fn method_or_field(
    e: &ExprType,
    width: Option<usize>,
    env: &ConstEnv,
    fold: &dyn Fn(&ExprType, Option<usize>) -> Result<u128, String>,
) -> Result<u128, String> {
    let int = |v: u128| i128::try_from(v).map_err(|_| "folds to a value wider than 127 bits".to_string());
    match e {
        ExprType::MethodCall(m) => {
            let recv = int(fold(&m.receiver, None)?)?;
            let args = m.args.iter().map(|a| int(fold(a, None)?)).collect::<Result<Vec<_>, _>>()?;
            let ty = ty_hint(&m.receiver, env)
                .or_else(|| m.args.iter().find_map(|a| ty_hint(a, env)))
                .or_else(|| width.and_then(|w| IntTy::of_name(&format!("u{w}"))));
            // A typed signed receiver folds to a negative value; the receiver's own
            // type reads it back (fold returns the bit pattern at its width).
            let recv = match ty {
                Some(t) if t.signed => t.wrap(recv).ok_or("a signed receiver out of range")?,
                _ => recv,
            };
            int_method(&m.method, recv, &args, ty).map(|v| v as u128).ok_or_else(|| {
                format!(
                    "calls `{}`, which the shared evaluator does not run as a constant here (not a \
                     `const` integer method, no integer type it can read, or an overflow)",
                    m.method
                )
            })
        }
        ExprType::Field(f) => {
            let i: usize = f.member.parse().map_err(|_| format!("reads the named field `{}`", f.member))?;
            match &*f.base {
                ExprType::Path(p) => {
                    let name = format!("{}.{i}", compact(&p.path_text));
                    env.get(&name)
                        .map(|v| v as u128)
                        .ok_or_else(|| format!("reads `{name}`, which is not a tuple-struct constant field"))
                }
                ExprType::Call(c) => {
                    let ExprType::Path(p) = &*c.func else {
                        return Err("reads a field of a call through something other than a path".to_string());
                    };
                    let name = compact(&p.path_text);
                    if !env.is_tuple_struct(&name) {
                        return Err(format!("reads a field of a call to `{name}`, which is not a tuple struct"));
                    }
                    let arg = c.args.get(i).ok_or_else(|| format!("reads field {i} of `{name}`, which it has not"))?;
                    fold(arg, width)
                }
                _ => Err("reads a field of an expression that is not a constant".to_string()),
            }
        }
        _ => Err("is not a method call or a field".to_string()),
    }
}
