// The smallest design in which `clk.tick().await` earns its keep: an 8-bit
// counter.
//
// `count` is an ordinary `let mut` — nothing marks it as state. It becomes a
// flip-flop because it is *live across the tick*: written in one iteration and
// read in the next. That is the whole language in one variable.
//
// `main()` is a real self-check: it runs the Copper simulator against an
// independent reference model written in plain `u8` arithmetic (no `Bits<N>`,
// no `Logic`), and exits non-zero on any mismatch. Simulator-vs-SystemVerilog
// equivalence is covered separately and automatically by the corpus
// differential sweep (see design_docs/CORPUS_DIFFERENTIAL_SWEEP.md), which
// picks this module up the moment the file exists.
//
// The expected trace below encodes two assumptions about the module under test:
//   1. `reset_i` wins over `en_i`, and `en_i` gates the increment;
//   2. the tick comes FIRST in the loop body, so `count_o` is the register's
//      current value (Moore) rather than this cycle's combinational next value.
// If the module settles on different semantics, `next_count` is the one place
// that has to change for (1), and the model update moves relative to
// `tick_clock` for (2).

use copper_core::port::{wire, In, Out};
use copper_core::{Bits, Clock, ClockDomain, Logic};
use copper_macros::hardware;
use copper_sim::{make_cycle, HardwareExecutor, HardwareTest, SimulationTrace};

struct MainClk;
impl ClockDomain for MainClk{}

#[hardware(sequential)]
async fn simple_counter(
    clk: Clock<MainClk>,
    reset_i: In<Logic, MainClk>,
    en_i: In<Logic, MainClk>,
    count_o: Out<Bits<8>, MainClk>,
) {
    let mut count = Bits::<8>::x();
    loop {
        if reset_i.read() == Logic::One {
            count = Bits::zero();
        } else if en_i.read() == Logic::One {   
            count = count + Bits::from_usize(1)
        }
        count_o.write(count);
        clk.tick().await;

    }
}

/// The reference model: the counter's recurrence in plain `u8`, independent of
/// `Bits<N>` and of anything the module does. `wrapping_add` is the 8-bit wrap
/// the hardware gets for free from `Bits<8>`.
fn next_count(count: u8, reset: bool, en: bool) -> u8 {
    if reset {
        0
    } else if en {
        count.wrapping_add(1)
    } else {
        count
    }
}

fn main() {
    let mut clk = Clock::<MainClk>::new();
    let mut exec = HardwareExecutor::new();

    let (reset_drv, reset_in) = wire::<Logic, MainClk>(Logic::Zero);
    let (en_drv, en_in) = wire::<Logic, MainClk>(Logic::Zero);
    let (count_drv, count_obs) = wire::<Bits<8>, MainClk>(Bits::zero());

    let dh = count_drv.dirty_handle();
    let reads = vec![reset_in.wire_id(), en_in.wire_id()];
    exec.spawn_wired(
        simple_counter(clk.clone(), reset_in, en_in, count_drv),
        vec![dh],
        reads,
    );

    let mut test = HardwareTest::new("simple_counter")
        .with_waveform("waveforms/simple_counter.vcd");

    // Stimulus, as (reset, en) pairs.
    //
    // 1. Forced cases first, so they are covered regardless of the RNG:
    //    reset, count up, hold across en=0, reset mid-count, resume.
    let forced: &[(bool, bool)] = &[
        (true, false),  // reset to 0
        (false, true),  // 1
        (false, true),  // 2
        (false, true),  // 3
        (false, false), // hold at 3
        (false, false), // hold at 3
        (false, true),  // 4
        (true, true),   // reset wins over enable -> 0
        (false, true),  // 1
        (false, false), // hold at 1
    ];

    // 2. A clean run of 260 enabled cycles, which crosses the 8-bit wrap
    //    (255 -> 0) that `Bits<8>` does and a `u8` counter in the model must
    //    agree with. Preceded by a reset so the wrap lands at a known cycle.
    let wrap = std::iter::once((true, false)).chain(std::iter::repeat((false, true)).take(260));

    // 3. Deterministic pseudo-random (LCG) stimulus: ~1-in-16 reset, ~50%
    //    enable, so resets interleave with holds in orders nobody wrote by hand.
    const N_RANDOM: usize = 256;
    let mut rng: u32 = 0x1234_5678;
    let random: Vec<(bool, bool)> = (0..N_RANDOM)
        .map(|_| {
            rng = rng.wrapping_mul(1664525).wrapping_add(1013904223);
            (((rng >> 5) & 0xF) == 0, ((rng >> 3) & 1) == 1)
        })
        .collect();

    let stimulus: Vec<(bool, bool)> = forced
        .iter()
        .copied()
        .chain(wrap)
        .chain(random)
        .collect();

    let mut model = 0u8; // matches `Bits::<8>::zero()` at power-up
    let mut expected_cycles = Vec::new();

    for (i, &(reset, en)) in stimulus.iter().enumerate() {
        reset_drv.write(Logic::from_bool(reset));
        en_drv.write(Logic::from_bool(en));
        exec.tick_clock(&mut clk);

        model = next_count(model, reset, en);

        let reset_l = Logic::from_bool(reset);
        let en_l = Logic::from_bool(en);
        let inputs: &[(&str, &[Logic])] = &[
            ("reset_i", std::slice::from_ref(&reset_l)),
            ("en_i", std::slice::from_ref(&en_l)),
        ];

        test.record_cycle(i, inputs, &[("count_o", count_obs.read().as_array())]);
        expected_cycles.push(make_cycle(
            i,
            inputs,
            &[("count_o", Bits::<8>::from_u8(model).as_array())],
        ));
    }

    let expected = SimulationTrace::from_cycles(expected_cycles);
    test.finish_with_expected(&expected).assert_passed();
    println!(
        "simple_counter: Copper sim ≡ reference model over {} cycles (reset, enable, hold, 8-bit wrap) ✓",
        stimulus.len()
    );
}
