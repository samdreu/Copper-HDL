// A parallel-in, serial-out shifter — the transmit half of a UART, at one clock
// per bit. Wait for `start`, capture the byte, then put a start bit, eight data
// bits LSB first, and a stop bit on the wire, one per cycle.
//
// This is the example for MULTIPLE clock boundaries in one module, because it has
// all three shapes a cycle boundary takes in real designs:
//
//   * a conditional wait   — `while start.read() == Logic::Zero { tick }`
//   * a counted sequence   — `for i in 0..8 { …; tick }`
//   * straight-line ticks  — the start bit and the stop bit
//
// The payoff is what the transpiler declares that the source never does. The
// emitted module has three registers — `pc` (the state), `i` (the bit counter)
// and `word` (the captured byte) — and the bit counter is the interesting one: in
// Verilog you declare and increment it by hand and compare it against 8, state
// that has nothing to do with the problem and exists only because the language
// cannot say "do this eight times". Here it is a `for` loop variable, and the
// register falls out of it being live across a tick.
//
// Timing: `tx` is a `RegOut`, so each value reaches the wire on the edge that
// ends the cycle it was written in, and holds until the next write. A frame is
// ten cycles on the wire: start, eight data bits, stop. `start` is sampled every
// cycle while idle; the cycle it is seen is the cycle the start bit is driven.

use copper_core::port::{registered_wire, wire, In, RegOut};
use copper_core::{Bits, Clock, ClockDomain, Logic};
use copper_macros::hardware;
use copper_sim::HardwareExecutor;

struct MainClk;
impl ClockDomain for MainClk {}

#[hardware(sequential)]
async fn serializer(
    clk: Clock<MainClk>,
    start: In<Logic, MainClk>,
    data: In<Bits<8>, MainClk>,
    tx: RegOut<Logic, MainClk>,
) {
    loop {
        tx.write(Logic::One); // idle: the line rests high
        while start.read() == Logic::Zero {
            clk.tick().await;
        }
        let word = data.read();
        tx.write(Logic::Zero); // start bit
        clk.tick().await;
        for i in 0..8 {
            tx.write(word[i]); // data bits, LSB first
            clk.tick().await;
        }
        tx.write(Logic::One); // stop bit
        clk.tick().await;
    }
}

// ── Reference model: the source's control flow, walked by hand ────────────────
//
// `Top` drives the idle high and tests `start`; `WaitStart` is the `while`'s
// self-loop and tests the same thing. Either way, the cycle `start` is seen is the
// cycle the byte is captured and the start bit is driven — leaving a `while` costs
// no time. `Data` walks the eight bits, one per cycle. `Stop` drives the stop bit
// and returns to the top. A `RegOut` holds when it is not written, which is why
// `WaitStart` leaves `tx` alone.

#[derive(Clone, Copy, PartialEq, Debug)]
enum St {
    Top,
    WaitStart,
    Data,
    Stop,
}

struct Model {
    st: St,
    i: usize,
    word: u8,
    tx: bool,
}

impl Model {
    fn new() -> Self {
        Model { st: St::Top, i: 0, word: 0, tx: false }
    }

    /// One clock: apply `(start, data)`, return `tx` as observed after the edge.
    fn step(&mut self, start: bool, data: u8) -> bool {
        match self.st {
            St::Top | St::WaitStart => {
                if self.st == St::Top {
                    self.tx = true; // the idle drive, only at the loop top
                }
                if start {
                    self.word = data;
                    self.tx = false; // start bit
                    self.i = 0;
                    self.st = St::Data;
                } else {
                    self.st = St::WaitStart;
                }
            }
            St::Data => {
                self.tx = (self.word >> self.i) & 1 == 1;
                self.i += 1;
                if self.i == 8 {
                    self.st = St::Stop;
                }
            }
            St::Stop => {
                self.tx = true;
                self.st = St::Top;
            }
        }
        self.tx
    }
}

fn logic(b: bool) -> Logic {
    if b { Logic::One } else { Logic::Zero }
}

/// Simulate on `stream` (one `(start, data)` per cycle) and return `tx` per cycle.
fn simulate(stream: &[(bool, u8)]) -> Vec<bool> {
    let mut clk = Clock::<MainClk>::new();
    let mut exec = HardwareExecutor::new();
    let (start_drv, start_in) = wire::<Logic, MainClk>(Logic::Zero);
    let (data_drv, data_in) = wire::<Bits<8>, MainClk>(Bits::zero());
    let (tx_drv, tx_obs) = registered_wire::<Logic, MainClk>(&clk, Logic::Zero);
    let dh = tx_drv.dirty_handle();
    let reads = vec![start_in.wire_id(), data_in.wire_id()];
    exec.spawn_wired(serializer(clk.clone(), start_in, data_in, tx_drv), vec![dh], reads);

    let mut out = Vec::with_capacity(stream.len());
    for &(start, data) in stream {
        start_drv.write(logic(start));
        data_drv.write(Bits::from_u8(data));
        exec.tick_clock(&mut clk);
        out.push(tx_obs.read() == Logic::One);
    }
    out
}

/// One frame: `start` high for the cycle that launches it, then `pad` idle cycles
/// with `start` low, which is long enough for the ten-cycle frame to finish.
fn frame(byte: u8, pad: usize) -> Vec<(bool, u8)> {
    std::iter::once((true, byte))
        .chain(std::iter::repeat((false, 0x00)).take(pad))
        .collect()
}

/// Idle, a frame with a mixed byte, all-zeros and all-ones (so every bit position
/// is exercised in both directions), and a frame launched from a longer idle wait.
fn coverage_stream() -> Vec<(bool, u8)> {
    let mut s: Vec<(bool, u8)> = vec![(false, 0x00), (false, 0x00)];
    s.extend(frame(0xA5, 11));
    s.extend(frame(0x00, 11));
    s.extend(frame(0xFF, 11));
    s.extend(vec![(false, 0x00); 3]);
    s.extend(frame(0x01, 11));
    s.extend(frame(0x80, 11));
    s
}

fn check(stream: &[(bool, u8)], verbose: bool) -> usize {
    let sim = simulate(stream);
    let mut model = Model::new();
    let mut mismatches = 0;
    for (c, &(start, data)) in stream.iter().enumerate() {
        let expected = model.step(start, data);
        if verbose {
            println!(
                "cycle {c:3}: start={} data=0x{data:02x} -> tx={}",
                start as u8, sim[c] as u8
            );
        }
        if sim[c] != expected {
            mismatches += 1;
            println!(
                "cycle {c}: simulator tx={} but the model says {}",
                sim[c] as u8, expected as u8
            );
        }
    }
    mismatches
}

/// The ten bits a receiver would see for `byte`: start low, eight data bits LSB
/// first, stop high. Written from the UART frame format, not from the module.
fn expected_frame(byte: u8) -> Vec<bool> {
    let mut f = vec![false];
    for i in 0..8 {
        f.push((byte >> i) & 1 == 1);
    }
    f.push(true);
    f
}

fn main() {
    // A demo AND a self-check: the simulator against the hand model of the source.
    let m = check(&coverage_stream(), true);
    if m != 0 {
        eprintln!("FAIL: {m} cycle(s) disagree with the model");
        std::process::exit(1);
    }
    println!("serializer: simulator == hand model ✓");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializer_matches_the_hand_model_on_the_coverage_stream() {
        assert_eq!(check(&coverage_stream(), false), 0);
    }

    #[test]
    fn a_frame_is_start_eight_data_bits_lsb_first_then_stop() {
        // Checked against the UART frame format rather than against the module:
        // one idle cycle, then the ten bits on the wire.
        for byte in [0xA5u8, 0x00, 0xFF, 0x01, 0x80, 0x3C] {
            let sim = simulate(&frame(byte, 11));
            assert_eq!(&sim[..10], &expected_frame(byte)[..], "byte 0x{byte:02x}");
            assert!(sim[10], "byte 0x{byte:02x}: the line must return to idle high");
        }
    }

    #[test]
    fn the_line_rests_high_while_no_frame_is_requested() {
        let idle: Vec<(bool, u8)> = vec![(false, 0xFF); 20];
        assert!(simulate(&idle).iter().all(|&b| b));
    }
}
