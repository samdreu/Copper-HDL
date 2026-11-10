//! D-28 at 128 bits (MEASUREMENTS §AB.1): a ticking `for` over a `u128` inclusive range whose
//! run-time end reaches `u128::MAX` runs its last iteration and stops, in the simulator (Rust's
//! `RangeInclusive`) and in the transpiled SV (I.8.5's done flag — no counter type holds `e + 1`).
//! Checked simulator against Verilator on every value of the 2-bit input in turn; also swept on three
//! legs (`tests/fixtures/wide_int_dut.rs`, since MEASUREMENTS §AG gave the reference interpreter its
//! `u128`).

mod common;

use common::{EquivalenceTest, RandStim};
use copper_core::port::{registered_wire, wire, In, RegOut};
use copper_core::{Bits, Clock, ClockDomain};
use copper_macros::hardware;
use copper_sim::HardwareExecutor;

struct MainClk;
impl ClockDomain for MainClk {}

include!("fixtures/wide_int_dut.rs");
const SRC: &str = include_str!("fixtures/wide_int_dut.rs");

#[test]
fn a_u128_inclusive_for_at_its_maximum_exits_and_matches_verilog() {
    let mut eq = EquivalenceTest::differential_only("imx_u128_done", SRC, Some("imx_u128_done"));
    let mut clk = Clock::<MainClk>::new();
    let mut exec = HardwareExecutor::new();
    let (a_drv, a_in) = wire::<Bits<2>, MainClk>(Bits::zero());
    let (q_out, q_obs) = registered_wire::<Bits<8>, MainClk>(&clk, Bits::zero());
    let dirty = vec![q_out.dirty_handle()];
    exec.spawn_wired(imx_u128_done(clk.clone(), a_in, q_out), dirty, vec![]);
    let mut seen_max = 0;
    let mut seen_entry = 0;
    for k in 0..64u32 {
        let a = Bits::<2>::from_usize(((k / 5) % 4) as usize);
        a_drv.write(a);
        exec.tick_clock(&mut clk);
        let q = q_obs.read();
        seen_max += (q.as_u8() == 0xff) as u32;
        seen_entry += (q.as_u8() == 0x33) as u32;
        eq.record_differential(&[("a", &a.as_bits()[..])], &[("q", &q.as_bits()[..])]);
    }
    assert!(seen_max > 0, "the stimulus never took the loop to u128::MAX");
    assert!(seen_entry > 1, "the loop never exited back to the module loop's head");
    eq.finish();
}
