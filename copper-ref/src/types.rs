//! Reading a `syn::Type` into the interpreter's [`Ty`]: port payloads, `let`
//! annotations, helper-function signatures. Widths are constant expressions —
//! a literal, a `const` item, a const generic, `{N + 1}` — evaluated by the same
//! evaluator the body runs under (`interp::Exec::eval_const`).

use crate::value::{IntTy, Ty};
use crate::RefError;
use syn::Type;

/// The trait object the type reader asks to evaluate a width expression.
pub trait ConstEval {
    fn eval_usize(&self, e: &syn::Expr) -> Result<usize, RefError>;
}

pub fn parse_ty(ty: &Type, ev: &dyn ConstEval) -> Result<Ty, RefError> {
    match ty {
        Type::Path(tp) => {
            let seg = tp
                .path
                .segments
                .last()
                .ok_or_else(|| RefError::Parse("empty type path".into()))?;
            let name = seg.ident.to_string();
            match name.as_str() {
                "Logic" => Ok(Ty::Logic),
                "bool" => Ok(Ty::Bool),
                "Bits" => {
                    let syn::PathArguments::AngleBracketed(args) = &seg.arguments else {
                        return Err(RefError::Parse("`Bits` without a width".into()));
                    };
                    let arg = args
                        .args
                        .first()
                        .ok_or_else(|| RefError::Parse("`Bits<>` with no argument".into()))?;
                    let w = match arg {
                        syn::GenericArgument::Const(e) => ev.eval_usize(e)?,
                        // `Bits<N>` parses the width as a *type* path when it is a bare
                        // identifier; `Bits<8>` as a const.
                        syn::GenericArgument::Type(Type::Path(p)) if p.qself.is_none() => {
                            let e: syn::Expr = syn::Expr::Path(syn::ExprPath {
                                attrs: Vec::new(),
                                qself: None,
                                path: p.path.clone(),
                            });
                            ev.eval_usize(&e)?
                        }
                        other => {
                            return Err(RefError::Parse(format!(
                                "unreadable `Bits` width `{}`",
                                quote_ty(other)
                            )))
                        }
                    };
                    if w == 0 || w > 128 {
                        return Err(RefError::Runtime(format!("Bits<{w}> is outside 1..=128")));
                    }
                    Ok(Ty::Bits(w as u32))
                }
                other => {
                    if let Some(it) = IntTy::parse(other) {
                        return Ok(Ty::Int(it));
                    }
                    Ok(Ty::Named(other.to_string()))
                }
            }
        }
        Type::Array(a) => {
            let elem = parse_ty(&a.elem, ev)?;
            let n = ev.eval_usize(&a.len)?;
            Ok(Ty::Array(Box::new(elem), n))
        }
        Type::Tuple(t) => Ok(Ty::Tuple(t.elems.iter().map(|e| parse_ty(e, ev)).collect::<Result<_, _>>()?)),
        Type::Reference(r) => parse_ty(&r.elem, ev),
        Type::Paren(p) => parse_ty(&p.elem, ev),
        Type::Group(g) => parse_ty(&g.elem, ev),
        other => Err(RefError::Unsupported(format!(
            "type `{}` is not a type of I.14",
            quote_ty_(other)
        ))),
    }
}

fn quote_ty(a: &syn::GenericArgument) -> String {
    use quote::ToTokens;
    a.to_token_stream().to_string()
}
fn quote_ty_(t: &Type) -> String {
    use quote::ToTokens;
    t.to_token_stream().to_string()
}
