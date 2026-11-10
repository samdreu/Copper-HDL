//! `copper-ref` — the **reference interpreter**: the third leg of the corpus sweep
//! (`design_docs/SPEC_OVERVIEW.md` §7).
//!
//! It interprets a `#[hardware(sequential)]` (or `synchronizer`) module's
//! `syn::ItemFn` **directly** — no proc macro, no rustc lowering, no executor —
//! under the one-resumption protocol of `SPEC.md` §I.0 as decided by D-8:
//!
//! * the module is the coroutine `async move { s_pre; loop { s } }` (I.0.1);
//! * [`Module::resume`] runs ONE resumption: from the entry on the first call
//!   (`s_pre`, then the loop head to the first `tick`, I.9.1), afterwards from the
//!   parked `tick` site to the next one it reaches, through plain Rust control
//!   flow; every `In::read()` in it yields the inputs the caller passed —
//!   `x_{N+1}`, the value present before the edge the resumption ends at (I.4.1);
//!   a `RegOut::write` sets a pending value, last write wins (I.6.1, I.6.2);
//! * [`Module::edge`] commits the pending values (I.6.1); the locals are the
//!   registers and need no commit of their own — one instant per cycle means the
//!   environment *is* the state (I.7.3);
//! * [`Module::observe`] is every `RegOut`'s committed value — before the first
//!   edge its initial value, the zero of its type (I.6.6, I.9.4).
//!
//! A suspension site is a [`TickSite`]: the statement/arm/loop index path of the
//! `tick` in the kernel (the residue of I.3.3). `pc` states of the extraction
//! route are many-to-one onto these (I.0.4), which `tests/` measures.
//!
//! [`memory`] is §I.10's memory model: a `Memory` bound in `s_pre` is a variable
//! of H(M) beside the environment, its stagings effects of the resumption, its
//! edge the read and write steps in the mode's order.
//!
//! A `combinational` module is evaluated once per input vector ([`Module::evaluate`],
//! §I.11), and a `structural` module is the design it names ([`Design`], §I.13).
//!
//! **What it refuses** — every refusal names its SPEC row:
//! a plain `Out` on a clocked module, a `Memory` parameter (I.10.5), a second
//! clock, a `structural` module given to [`Module::new`] (all *deferred*,
//! [`RefError::Deferred`], the classes `build.rs`'s `ref_skip_reason` skips); a read
//! inside macro tokens, a statement macro, a labelled or valued `break`, `return`, a
//! non-`tick` `await`, a tick before the loop, recursion ([`RefError::Unsupported`],
//! naming the II.1 / II.3 row). It has no opinion about what the transpiler can lower: a shape in
//! II.3's defect rows is interpreted by its Part I meaning, which is the point.
//!
//! **Reading a disagreement** (`SPEC_OVERVIEW.md` §7): the interpreter is the
//! spec made executable, so sim ≠ ref with SV = ref is a simulator or macro bug;
//! sim = ref ≠ SV is a transpiler bug or a II.3 row; all three different is a
//! spec gap or an interpreter bug. See `README.md`.

pub mod bits;
pub mod design;
pub mod interp;
pub mod memory;
pub mod module;
pub mod types;
pub mod value;

pub use bits::RefBits;
pub use design::Design;
pub use interp::{Flow, TickSite};

/// The static `tick` sites of any `#[hardware]` fn's body — the states of H(M)
/// (SPEC I.0.4) — without constructing a [`Module`], so a module the interpreter
/// defers (a plain `Out`) can still be counted against the transpiler's `pc` states.
pub fn static_tick_sites(f: &syn::ItemFn) -> std::collections::BTreeSet<TickSite> {
    let mut out = std::collections::BTreeSet::new();
    interp::collect_tick_sites(&f.block.stmts, &mut Vec::new(), &mut out);
    out
}
pub use module::{ConstScope, Module, Port, PortKind};
pub use value::{IntTy, Ty, Value};

/// Why the interpreter could not run, or could not continue.
#[derive(Debug, Clone, PartialEq)]
pub enum RefError {
    /// The source does not parse, or the named module is not in it.
    Parse(String),
    /// A module class the interpreter does not cover yet, with the SPEC clause
    /// that owes it — the `REF_SKIP` classes: plain `Out` (D-8), a `Memory`
    /// parameter (I.10.5),
    /// a second clock (I.12), `combinational` (I.11), `structural` (I.13).
    Deferred(String),
    /// A construct outside the kernel or in a dated refusal row, named by its
    /// II.1 / II.3 row.
    Unsupported(String),
    /// A run-time fault the Rust program would panic on (an X read as an integer,
    /// an index out of bounds, integer overflow) or an interpreter invariant.
    Runtime(String),
    /// The poison run read a local the register inference did not name before it
    /// was reassigned (SPEC §I.0.5(a)): an under-inference, by the local's name.
    Poisoned(String),
}

impl std::fmt::Display for RefError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RefError::Parse(s) => write!(f, "parse: {s}"),
            RefError::Deferred(s) => write!(f, "deferred: {s}"),
            RefError::Unsupported(s) => write!(f, "unsupported: {s}"),
            RefError::Runtime(s) => write!(f, "runtime: {s}"),
            RefError::Poisoned(n) => write!(
                f,
                "poisoned read of `{n}`: a local the register inference did not name was \
                 read after a tick before being reassigned (SPEC I.0.5(a) under-inference)"
            ),
        }
    }
}

impl std::error::Error for RefError {}
