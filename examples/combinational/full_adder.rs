// A one-bit full adder: the smallest combinational module with more than one
// output.
//
// `main()` is a real self-check: it drives all eight input combinations through
// the Copper simulator and compares against an independent reference model
// written in plain integer arithmetic (`a + b + cin`, no `Logic` operators),
// exiting non-zero on any mismatch. Simulator-vs-SystemVerilog equivalence is
// covered separately and automatically by the corpus differential sweep (see
// design_docs/CORPUS_DIFFERENTIAL_SWEEP.md), which picks this module up the
// moment the file exists.

use copper_core::port::*;
use copper_core::types::*;
use copper_sim::*;
use copper_macros::hardware;

#[hardware(combinational)]
fn full_adder(
    a_i: In<Logic, ()>,
    b_i: In<Logic, ()>,
    cin_i: In<Logic, ()>,
    sum_o: Out<Logic, ()>,
    cout_o: Out<Logic, ()>,
) {
    let a = a_i.read();
    let b = b_i.read();
    let cin = cin_i.read();
    sum_o.write(a ^ b ^ cin);
    cout_o.write((a & b) | (cin & (a ^ b)));
}

fn logic(bit: u8) -> Logic {
    if bit == 0 { Logic::Zero } else { Logic::One }
}

fn main() {
    let mut executor = HardwareExecutor::new();

    let (a_drv, a_in) = wire::<Logic, ()>(Logic::Zero);
    let (b_drv, b_in) = wire::<Logic, ()>(Logic::Zero);
    let (cin_drv, cin_in) = wire::<Logic, ()>(Logic::Zero);
    let (sum_out, sum_obs) = wire::<Logic, ()>(Logic::Zero);
    let (cout_out, cout_obs) = wire::<Logic, ()>(Logic::Zero);

    let dhs = vec![sum_out.dirty_handle(), cout_out.dirty_handle()];
    let reads = vec![a_in.wire_id(), b_in.wire_id(), cin_in.wire_id()];
    executor.spawn_wired(full_adder(a_in, b_in, cin_in, sum_out, cout_out), dhs, reads);

    let mut test = HardwareTest::new("full_adder")
        .with_waveform("waveforms/full_adder.vcd");

    // Exhaustive: all eight (a, b, cin) combinations.
    let cases: Vec<(u8, u8, u8)> = (0..8u8).map(|n| (n >> 2 & 1, n >> 1 & 1, n & 1)).collect();

    for (i, &(a, b, cin)) in cases.iter().enumerate() {
        a_drv.write(logic(a));
        b_drv.write(logic(b));
        cin_drv.write(logic(cin));
        executor.poll_tasks();
        test.record_cycle(
            i,
            &[("a_i", &[logic(a)]), ("b_i", &[logic(b)]), ("cin_i", &[logic(cin)])],
            &[("sum_o", &[sum_obs.read()]), ("cout_o", &[cout_obs.read()])],
        );
    }

    // Reference model: the two-bit integer sum a + b + cin.
    let expected = SimulationTrace::from_cycles(
        cases.iter().enumerate().map(|(i, &(a, b, cin))| {
            let total = a + b + cin;
            make_cycle(
                i,
                &[("a_i", &[logic(a)]), ("b_i", &[logic(b)]), ("cin_i", &[logic(cin)])],
                &[("sum_o", &[logic(total & 1)]), ("cout_o", &[logic(total >> 1)])],
            )
        }).collect(),
    );

    test.finish_with_expected(&expected).assert_passed();
}
