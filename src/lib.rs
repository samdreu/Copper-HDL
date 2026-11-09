//! Copper HDL — top-level crate.
//!
//! Hosts the hardware standard library: reusable `#[hardware]` modules that live
//! *downstream* of `copper-sim` (the `#[hardware]` macro expands to
//! `::copper_sim::…` paths, so these cannot live inside `copper-sim` itself).
//!
//! # A const-generic range R3 depends on is checked per instance (D-30)
//!
//! A ticking `for` over a range bounded by a const generic that is the module loop's
//! only boundary on some path is taken as non-empty (SPEC I.8.4(b), D-30, MEASUREMENTS
//! §AF). The macro asserts it at monomorphization, so an instance whose range is empty
//! does not build — rustc's E0080 (a `cargo check` or trybuild never instantiates, so
//! only a build sees it). The transpiler's twin is an elaboration-time `$error`
//! (`tests/generic_range_instance_d30.rs`); the swept instance, `W = 3`, builds
//! (`tests/fixtures/generic_range_dut.rs::grn_only_boundary`).
//!
//! ```compile_fail,E0080
//! use copper_core::port::{registered_wire, wire, In, RegOut};
//! use copper_core::{Bits, Clock, ClockDomain};
//! use copper_macros::hardware;
//! use copper_sim::HardwareExecutor;
//! struct MainClk; impl ClockDomain for MainClk {}
//!
//! #[hardware(sequential)]
//! async fn grn_only_boundary<const W: usize>(clk: Clock<MainClk>, a: In<Bits<4>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
//!     loop {
//!         q.write(Bits::from_u8(a.read().as_u8() + 16));
//!         for _ in 0..W {
//!             clk.tick().await;
//!         }
//!     }
//! }
//!
//! let mut clk = Clock::<MainClk>::new();
//! let mut exec = HardwareExecutor::new();
//! let (_d, a) = wire::<Bits<4>, MainClk>(Bits::zero());
//! let (q, _o) = registered_wire::<Bits<8>, MainClk>(&clk, Bits::zero());
//! let dh = q.dirty_handle();
//! let reads = vec![a.wire_id()];
//! exec.spawn_wired(grn_only_boundary::<0>(clk.clone(), a, q), vec![dh], reads); // W = 0: refused
//! exec.tick_clock(&mut clk);
//! ```
//!
//! # Clocks are provided, not constructed
//!
//! A hardware module receives its clock as a parameter and may not create one —
//! a fabricated clock is never driven, so the module would hang (CDC audit,
//! "Gap 2"). This is rejected at compile time:
//!
//! ```compile_fail
//! use copper_core::port::RegOut;
//! use copper_core::{Clock, ClockDomain, Logic};
//! use copper_macros::hardware;
//! struct Slow; impl ClockDomain for Slow {}
//! struct Fast; impl ClockDomain for Fast {}
//!
//! #[hardware(sequential)]
//! async fn m(clk: Clock<Slow>, q: RegOut<Logic, Slow>) {
//!     let f = Clock::<Fast>::new();   // rejected: a module may not create a clock
//!     loop { q.write(Logic::Zero); clk.tick().await; let _ = &f; }
//! }
//! ```
//!
//! Cloning a clock parameter to pass to a submodule stays legal — a clone shares
//! the same real clock:
//!
//! ```
//! use copper_core::port::RegOut;
//! use copper_core::{Clock, ClockDomain, Logic};
//! use copper_macros::hardware;
//! struct Slow; impl ClockDomain for Slow {}
//!
//! #[hardware(sequential)]
//! async fn m(clk: Clock<Slow>, q: RegOut<Logic, Slow>) {
//!     let child_clk = clk.clone();
//!     loop { q.write(Logic::Zero); clk.tick().await; let _ = &child_clk; }
//! }
//! ```

pub mod sync;
pub use sync::sync_2ff;
