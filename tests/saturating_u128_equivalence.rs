//! `saturating_add` / `saturating_sub` on `u128` (SPEC II.3.3 (e), MEASUREMENTS §U.7): the
//! simulator against the transpiled SystemVerilog under Verilator, on the operand pairs that
//! reach each clamp and a few that do not — the boundary pairs the sweep's random stimulus need not
//! hit. The module is also swept on three legs (`tests/fixtures/wide_int_dut.rs`, since MEASUREMENTS
//! §AG gave the reference interpreter its `u128`).

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
fn u128_saturating_add_and_sub_clamp_and_match_verilog() {
    const MAX: u128 = u128::MAX;
    const TOP: u128 = 1 << 127;
    let pairs: &[(u128, u128)] = &[
        (0, 0),
        (MAX, 1),
        (TOP, TOP),
        (TOP - 1, TOP),
        (MAX - 5, 5),
        (MAX - 5, 6),
        (5, 7),
        (7, 5),
        (0, MAX),
        (MAX, MAX),
        (0x0123_4567_89AB_CDEF_0123_4567_89AB_CDEF, 0xFEDC_BA98_7654_3210_FEDC_BA98_7654_3210),
        (0xFEDC_BA98_7654_3210_FEDC_BA98_7654_3211, 0x0123_4567_89AB_CDEF_0123_4567_89AB_CDEF),
    ];
    // `differential_only` (the sweep's harness, which carries ports wider than 64 bits;
    // `EquivalenceTest::new`'s testbench compares through a 64-bit integer).
    let mut eq = EquivalenceTest::differential_only("sat_u128", SRC, Some("sat_u128"));
    let mut clk = Clock::<MainClk>::new();
    let mut exec = HardwareExecutor::new();
    let half = |v: u128| (Bits::<64>::from_u64((v >> 64) as u64), Bits::<64>::from_u64(v as u64));
    let join = |hi: Bits<64>, lo: Bits<64>| ((hi.as_u64() as u128) << 64) | lo.as_u64() as u128;
    let (ah_drv, ah_in) = wire::<Bits<64>, MainClk>(Bits::zero());
    let (al_drv, al_in) = wire::<Bits<64>, MainClk>(Bits::zero());
    let (bh_drv, bh_in) = wire::<Bits<64>, MainClk>(Bits::zero());
    let (bl_drv, bl_in) = wire::<Bits<64>, MainClk>(Bits::zero());
    let (sh_out, sh_obs) = registered_wire::<Bits<64>, MainClk>(&clk, Bits::zero());
    let (sl_out, sl_obs) = registered_wire::<Bits<64>, MainClk>(&clk, Bits::zero());
    let (th_out, th_obs) = registered_wire::<Bits<64>, MainClk>(&clk, Bits::zero());
    let (tl_out, tl_obs) = registered_wire::<Bits<64>, MainClk>(&clk, Bits::zero());
    let dirty = vec![sh_out.dirty_handle(), sl_out.dirty_handle(), th_out.dirty_handle(), tl_out.dirty_handle()];
    exec.spawn_wired(
        sat_u128(clk.clone(), ah_in, al_in, bh_in, bl_in, sh_out, sl_out, th_out, tl_out),
        dirty,
        vec![],
    );
    for &(x, y) in pairs {
        let ((xh, xl), (yh, yl)) = (half(x), half(y));
        ah_drv.write(xh);
        al_drv.write(xl);
        bh_drv.write(yh);
        bl_drv.write(yl);
        exec.tick_clock(&mut clk);
        let (sh, sl, th, tl) = (sh_obs.read(), sl_obs.read(), th_obs.read(), tl_obs.read());
        assert_eq!(join(sh, sl), x.saturating_add(y), "the simulator's clamp at {x:#x} + {y:#x}");
        assert_eq!(join(th, tl), x.saturating_sub(y), "the simulator's clamp at {x:#x} - {y:#x}");
        eq.record_differential(
            &[
                ("a_hi", &xh.as_bits()[..]),
                ("a_lo", &xl.as_bits()[..]),
                ("b_hi", &yh.as_bits()[..]),
                ("b_lo", &yl.as_bits()[..]),
            ],
            &[
                ("s_hi", &sh.as_bits()[..]),
                ("s_lo", &sl.as_bits()[..]),
                ("t_hi", &th.as_bits()[..]),
                ("t_lo", &tl.as_bits()[..]),
            ],
        );
    }
    eq.finish();
}
