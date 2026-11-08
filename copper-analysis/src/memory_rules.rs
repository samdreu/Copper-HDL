//! The memory **shape** rules of SPEC §I.10 that both front-ends run (D-25, 2026-11-08).
//!
//! Each is a rule on the source text alone — a parameter type or a turbofish — so it
//! lives here and is called once from [`crate::front_end::check_front_end_rules`]; the
//! macro and the transpiler therefore refuse with one message (II.1.3). Rule 6, the
//! memory rule that reasons about clock edges, is [`crate::check_memory_staging`].
//!
//! * **Placement** (I.10.5, I.10.6, I.2.2): a `Memory` is a parameter of a `sequential`
//!   or `synchronizer` module only. A `combinational` module neither stages nor observes
//!   one (I.10.6, OUTSIDE); a `structural` module receives none — one receiver per memory
//!   (I.10.5, D-25 (1)), the owner being the instantiating context outside the language.
//! * **Latency** (I.10.1(a)): a literal `READ_LAT` or `WRITE_LAT` of 0 — in a parameter
//!   type or a `Memory::<…>` turbofish — is OUTSIDE (D-25 (3)). The simulator's type
//!   asserts the same at construction (`copper_core::memory::latency_message`).
//! * **The received write latency** (I.10.5, II.3.3 row (i)): a `Memory` parameter's
//!   `WRITE_LAT` must be 1 — the bus carries the freshly staged write nets (REFUSED-both,
//!   fix direction: expose the committing stage on the bus).

use syn::spanned::Spanned;
use syn::visit::Visit;
use syn::{Error, Expr, FnArg, GenericArgument, ItemFn, Lit, PathArguments, Type};

use crate::front_end::HardwareMode;

/// The message for a `Memory` parameter on a `combinational` module (I.10.6).
pub const COMBINATIONAL_MEMORY_MESSAGE: &str = "a `combinational` module cannot receive a \
    `Memory`: an evaluation has no resumption to stage in, and a port nothing stages is never \
    ready. Stage the read in a `sequential` module and write `data()` to a `RegOut` that the \
    combinational module reads (SPEC I.10.6: OUTSIDE; I.2.2)";

/// The message for a `Memory` parameter on a `structural` module (I.10.5).
pub const STRUCTURAL_MEMORY_MESSAGE: &str = "a `structural` module cannot receive a `Memory`: \
    a memory is a parameter of a `sequential` or `synchronizer` module only, and is received by \
    exactly one module (SPEC I.10.5: one receiver per memory; I.2.2)";

/// The message for a received memory whose `WRITE_LAT` is not 1 (I.10.5, II.3.3 row (i)).
pub fn received_write_latency_message(name: &str, write_lat: u64) -> String {
    format!(
        "received `Memory` parameter `{name}` has WRITE_LAT = {write_lat}; the bus ABI \
         supports WRITE_LAT = 1 only — declare the memory inside the module, or use \
         WRITE_LAT = 1 (SPEC I.10.5; II.3.3 row (i), REFUSED-both)"
    )
}

/// Run the memory shape rules of SPEC §I.10 on `f`, written in `mode`.
pub fn check_memory_shapes(f: &ItemFn, mode: HardwareMode) -> Result<(), Error> {
    for arg in &f.sig.inputs {
        let FnArg::Typed(pt) = arg else { continue };
        let Some(args) = memory_generics(&pt.ty) else { continue };
        match mode {
            HardwareMode::Combinational => {
                return Err(Error::new_spanned(&pt.ty, COMBINATIONAL_MEMORY_MESSAGE));
            }
            HardwareMode::Structural => {
                return Err(Error::new_spanned(&pt.ty, STRUCTURAL_MEMORY_MESSAGE));
            }
            HardwareMode::Sequential | HardwareMode::Synchronizer => {}
        }
        check_latencies(&args, pt.ty.span())?;
        if let Some(m) = args.get(5).and_then(literal_usize) {
            if m != 1 {
                let name = match &*pt.pat {
                    syn::Pat::Ident(i) => i.ident.to_string(),
                    _ => "_".to_string(),
                };
                return Err(Error::new_spanned(&pt.ty, received_write_latency_message(&name, m)));
            }
        }
    }

    // Every `Memory::<…>` turbofish in the body (the declarations of I.10.3).
    struct V {
        err: Option<Error>,
    }
    impl<'ast> Visit<'ast> for V {
        fn visit_expr_path(&mut self, p: &'ast syn::ExprPath) {
            if self.err.is_none() {
                for seg in &p.path.segments {
                    if seg.ident == "Memory" {
                        if let PathArguments::AngleBracketed(ab) = &seg.arguments {
                            let args: Vec<GenericArgument> = ab.args.iter().cloned().collect();
                            if let Err(e) = check_latencies(&args, seg.span()) {
                                self.err = Some(e);
                            }
                        }
                    }
                }
            }
            syn::visit::visit_expr_path(self, p);
        }
    }
    let mut v = V { err: None };
    v.visit_block(&f.block);
    match v.err {
        Some(e) => Err(e),
        None => Ok(()),
    }
}

/// `Memory<T, R, W, D, READ_LAT, WRITE_LAT>`'s generic arguments, if `ty` names a `Memory`.
fn memory_generics(ty: &Type) -> Option<Vec<GenericArgument>> {
    let Type::Path(tp) = ty else { return None };
    let seg = tp.path.segments.last()?;
    if seg.ident != "Memory" {
        return None;
    }
    match &seg.arguments {
        PathArguments::AngleBracketed(ab) => Some(ab.args.iter().cloned().collect()),
        _ => Some(Vec::new()),
    }
}

/// Refuse a literal 0 at the `READ_LAT` (4) or `WRITE_LAT` (5) position.
fn check_latencies(args: &[GenericArgument], span: proc_macro2::Span) -> Result<(), Error> {
    let l = args.get(4).and_then(literal_usize);
    let m = args.get(5).and_then(literal_usize);
    if l == Some(0) || m == Some(0) {
        // An omitted latency is the type's default, 1. One message with the simulator's
        // construction-time assert.
        return Err(Error::new(
            span,
            copper_core::memory::latency_message(l.unwrap_or(1) as usize, m.unwrap_or(1) as usize),
        ));
    }
    Ok(())
}

/// A generic argument that is an integer literal (`0`, `1`, or `{ 2 }`).
fn literal_usize(a: &GenericArgument) -> Option<u64> {
    let e = match a {
        GenericArgument::Const(e) => e,
        // `Memory<T, 1, 0, D, 0, 1>` parses its integer arguments as consts; a bare
        // identifier is a type to syn and is not a literal.
        _ => return None,
    };
    lit_of(e)
}

fn lit_of(e: &Expr) -> Option<u64> {
    match e {
        Expr::Lit(l) => match &l.lit {
            Lit::Int(i) => i.base10_parse().ok(),
            _ => None,
        },
        Expr::Block(b) if b.block.stmts.len() == 1 => match &b.block.stmts[0] {
            syn::Stmt::Expr(e, None) => lit_of(e),
            _ => None,
        },
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check(src: &str, mode: HardwareMode) -> Result<(), String> {
        let f: ItemFn = syn::parse_str(src).unwrap();
        check_memory_shapes(&f, mode).map_err(|e| e.to_string())
    }

    #[test]
    fn a_combinational_memory_parameter_is_refused() {
        let e = check("fn m(x: Memory<Bits<8>, 1, 0, C, 1, 1>, o: Out<Bits<8>, C>) {}", HardwareMode::Combinational)
            .unwrap_err();
        assert!(e.contains("SPEC I.10.6"), "{e}");
    }

    #[test]
    fn a_structural_memory_parameter_is_refused() {
        let e = check("async fn m(clk: Clock<C>, x: Memory<Bits<8>, 1, 0, C, 1, 1>, o: RegOut<Bits<8>, C>) {}", HardwareMode::Structural)
            .unwrap_err();
        assert!(e.contains("one receiver per memory"), "{e}");
    }

    #[test]
    fn a_zero_latency_is_refused_in_a_parameter_and_a_declaration() {
        let e = check("async fn m(clk: Clock<C>, x: Memory<Bits<8>, 1, 0, C, 0, 1>) {}", HardwareMode::Sequential)
            .unwrap_err();
        assert!(e.contains("READ_LAT = 0, WRITE_LAT = 1; both must be at least 1"), "{e}");
        let e = check(
            "async fn m(clk: Clock<C>) { let r = Memory::<Bits<8>, 1, 1, C, 1, 0>::new(clk.clone(), 4); loop { clk.tick().await; } }",
            HardwareMode::Sequential,
        )
        .unwrap_err();
        assert!(e.contains("READ_LAT = 1, WRITE_LAT = 0; both must be at least 1"), "{e}");
    }

    #[test]
    fn a_received_write_latency_above_one_is_refused() {
        let e = check("async fn m(clk: Clock<C>, x: Memory<Bits<8>, 1, 1, C, 1, 2>) {}", HardwareMode::Sequential)
            .unwrap_err();
        assert!(e.contains("WRITE_LAT = 2; the bus ABI supports WRITE_LAT = 1 only"), "{e}");
    }

    #[test]
    fn the_admitted_shapes_pass() {
        for (src, mode) in [
            ("async fn m(clk: Clock<C>, x: Memory<Bits<8>, 1, 1, C, 3, 1>) {}", HardwareMode::Sequential),
            ("async fn m(clk: Clock<C>, x: Memory<Bits<8>, 1, 1, C>) {}", HardwareMode::Sequential),
            ("async fn m(clk: Clock<C>, x: Memory<Bits<8>, 1, 1, C, 1, 1>) {}", HardwareMode::Synchronizer),
            (
                "async fn m(clk: Clock<C>) { let r = Memory::<Bits<8>, 1, 1, C, 2, 2>::new(clk.clone(), 4); loop { clk.tick().await; } }",
                HardwareMode::Sequential,
            ),
        ] {
            assert_eq!(check(src, mode), Ok(()), "{src}");
        }
    }
}
