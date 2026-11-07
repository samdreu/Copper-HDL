//! Memory across phases, and a read result consumed with no mediating register:
//! sim ≡ transpiled SystemVerilog.
//!
//! ## What a read result actually is
//!
//! The address buses are combinational and phase-gated, so outside its staging
//! phase a read port's address reads as `'0`. A result observed in a LATER phase
//! therefore cannot be a continuous read of the array — it has to be captured.
//! The lowering emits a read pipeline stage for exactly that:
//!
//! ```systemverilog
//! always_ff @(posedge clk) begin
//!     rom_rd0_q0 <= rom_rd0_data;  // unguarded: the simulator's pipeline
//!     rom_rd0_v0 <= rom_rd0_en;    // advances on EVERY edge
//! end
//! ```
//!
//! Unguarded is the faithful choice, not a shortcut: `advance_read_pipelines` is a
//! plain clock listener in `copper-core/src/memory.rs` — the memory knows nothing
//! about phases. Outside its staging phase the enable is 0, so `_v` goes false
//! there, which is precisely `is_ready()` after a cycle with no staged address.
//! The captured word survives one cycle and no longer, in both.
//!
//! ## The two forms, and the bug that came from having only one
//!
//! A consumer that latches at the SAME edge as the capture — a register update in
//! the staging phase — must read the *combinational* value, since the pipeline
//! register has not updated yet at that edge. Everything else must read the
//! register. `rom_direct` below is why this matters beyond multi-phase: it is a
//! single-tick loop whose post-tick segment drives an output straight from
//! `data()`. That segment runs AFTER the capture edge, so it must read the
//! register — and until 2026-08-24 it read the continuous array net instead,
//! tracking the address being staged for the *next* edge. A full cycle early, and
//! shipped: no test drove a read result without a register in between.
//!
//! ## Phase C step 3 (D-8, 2026-11-06)
//!
//! Every output here is a `RegOut` now. A resumption observes what the previous
//! edge delivered (SPEC I.10.1 (d)) and a `RegOut` shows from the next edge, so each
//! model below is the two-job model DELAYED one observation (`delayed`). The refused
//! plain-`Out` shape `mp_rom` (retired rule 7, `memory_result_drives_plain_out`) is
//! unexpressible and its test is gone; `dual_port_ram`, the same-edge case, is an
//! array register since step 3 (MEASUREMENTS §G.2), so the shape pin no longer
//! covers it.

mod common;

use common::EquivalenceTest;
use copper_core::port::{registered_wire, wire, In, Out, RegOut};
use copper_core::types::Bits;
use copper_core::{Clock, ClockDomain, Memory};
use copper_macros::hardware;
use copper_sim::HardwareExecutor;

struct MainClk;
impl ClockDomain for MainClk {}

include!("fixtures/multiphase_rom_dut.rs");
const SRC: &str = include_str!("fixtures/multiphase_rom_dut.rs");

/// The ROM's contents, stated once — the same rule the fixture's `from_fn` states.
fn rom(i: usize) -> u16 {
    (i * 3 + 7) as u16
}

/// Even indices are the staging phase, so these are the addresses that matter for
/// the two-phase DUTs; they are all distinct so a wrong pipeline depth shows up.
const ADDRS: [usize; 8] = [0, 5, 2, 9, 15, 1, 7, 3];

/// Reference model for both two-phase DUTs: on a staging cycle the output shows
/// the word captured on the PREVIOUS staging cycle, then this cycle's address is
/// captured; the intervening cycle holds.
fn delayed(v: Vec<u16>) -> Vec<u16> {
    std::iter::once(0).chain(v.iter().copied()).take(v.len()).collect()
}

fn two_phase_expected() -> Vec<u16> {
    let mut q = 0u16;
    ADDRS
        .iter()
        .enumerate()
        .map(|(i, &a)| {
            let out = q;
            if i % 2 == 0 {
                q = rom(a);
            }
            out
        })
        .collect()
}

#[test]
fn single_tick_direct_read_sim_matches_transpiled_verilog() {
    let mut eq = EquivalenceTest::for_module("rom_direct", SRC, Some("rom_direct"));

    let mut clk = Clock::<MainClk>::new();
    let mut exec = HardwareExecutor::new();
    let (a_drv, a_in) = wire::<Bits<4>, MainClk>(Bits::zero());
    let (d_out, d_obs) = registered_wire::<Bits<16>, MainClk>(&clk, Bits::zero());
    let dh = d_out.dirty_handle();
    exec.spawn_wired(rom_direct(clk.clone(), a_in, d_out), vec![dh], vec![]);

    let direct: Vec<u16> = delayed(ADDRS.iter().map(|&a| rom(a)).collect());
    for (i, &a) in ADDRS.iter().enumerate() {
        a_drv.write(Bits::<4>::from_usize(a));
        exec.tick_clock(&mut clk);

        // The word this edge captured is written to the `RegOut` by the next
        // resumption and shows one observation later (D-8).
        let exp = Bits::<16>::from_u16(direct[i]);
        let ab = Bits::<4>::from_usize(a);
        let db = d_obs.read();
        eq.record(
            &[("addr", &ab.as_array()[..])],
            &[("data", &db.as_array()[..])],
            &[("data", &exp.as_array()[..])],
        );
    }

    eq.finish();
}

#[test]
fn cross_phase_read_into_register_sim_matches_transpiled_verilog() {
    let mut eq = EquivalenceTest::for_module("mp_reg", SRC, Some("mp_reg"));

    let mut clk = Clock::<MainClk>::new();
    let mut exec = HardwareExecutor::new();
    let (a_drv, a_in) = wire::<Bits<4>, MainClk>(Bits::zero());
    let (d_out, d_obs) = registered_wire::<Bits<16>, MainClk>(&clk, Bits::zero());
    let dh = d_out.dirty_handle();
    exec.spawn_wired(mp_reg(clk.clone(), a_in, d_out), vec![dh], vec![]);

    for (i, &a) in ADDRS.iter().enumerate() {
        a_drv.write(Bits::<4>::from_usize(a));
        exec.tick_clock(&mut clk);

        let exp = Bits::<16>::from_u16(delayed(two_phase_expected())[i]);
        let ab = Bits::<4>::from_usize(a);
        let db = d_obs.read();
        eq.record(
            &[("addr", &ab.as_array()[..])],
            &[("data", &db.as_array()[..])],
            &[("data", &exp.as_array()[..])],
        );
    }

    eq.finish();
}

#[test]
fn cross_phase_read_into_regout_sim_matches_transpiled_verilog() {
    let mut eq = EquivalenceTest::for_module("mp_regout", SRC, Some("mp_regout"));

    let mut clk = Clock::<MainClk>::new();
    let mut exec = HardwareExecutor::new();
    let (a_drv, a_in) = wire::<Bits<4>, MainClk>(Bits::zero());
    let (d_out, d_obs) = registered_wire::<Bits<16>, MainClk>(&clk, Bits::zero());
    let dh = d_out.dirty_handle();
    exec.spawn_wired(mp_regout(clk.clone(), a_in, d_out), vec![dh], vec![]);

    for (i, &a) in ADDRS.iter().enumerate() {
        a_drv.write(Bits::<4>::from_usize(a));
        exec.tick_clock(&mut clk);

        let exp = Bits::<16>::from_u16(two_phase_expected()[i]);
        let ab = Bits::<4>::from_usize(a);
        let db = d_obs.read();
        eq.record(
            &[("addr", &ab.as_array()[..])],
            &[("data", &db.as_array()[..])],
            &[("data", &exp.as_array()[..])],
        );
    }

    eq.finish();
}

/// Shape pins for the two forms, so a future change cannot quietly collapse them
/// back into one (which is the bug this file was written for).
#[test]
fn read_result_uses_the_register_after_the_capture_edge() {
    let emit = |m: &str| {
        copper_codegen::transpile_source(SRC, Some(m), &copper_codegen::EmitConfig::default())
            .unwrap_or_else(|e| panic!("{m} should transpile: {e}"))
    };

    let direct = emit("rom_direct");
    assert!(
        direct.contains("rom_rd0_q0 <= rom_rd0_data;") && direct.contains("data <= rom_rd0_q0;"),
        "a post-tick consumer must read the CAPTURED word, not the live array read — \
         reading `rom_rd0_data` here is the one-cycle-early bug, got:\n{direct}"
    );

    let mp = emit("mp_reg");
    assert!(
        mp.contains("rom_rd0_q0 <= rom_rd0_data;"),
        "a cross-phase read needs the pipeline register, got:\n{mp}"
    );
    assert!(
        mp.contains("rom_rd0_v0 <= rom_rd0_en;"),
        "`is_ready()` across a phase needs the valid register too, got:\n{mp}"
    );
}
