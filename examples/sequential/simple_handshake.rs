// A request/acknowledge handshake counter in its shortest form: wait for `req`,
// wait for `ack`, count the completed handshake.
//
// Each `while` is a state with a self-loop and each `clk.tick().await` is a clock
// edge, so the control flow IS the state machine an HDL would make the designer
// enumerate: three ticks, three states (the loop top where `done` is published,
// wait for `req`, wait for `ack`), and the transpiler emits exactly that `pc`.
// `n` is live across the ticks and becomes a register; `done` is a `RegOut`
// because it is written before a tick and must hold for the cycles in between.
//
// The difference from examples/sequential/handshake.rs is one `clk.tick().await`,
// and it is a protocol decision rather than a saving. That module has a bare tick
// between the two waits, so `ack` is never sampled in the cycle `req` was seen and
// a handshake always costs at least two cycles. This one samples both in the same
// cycle, so `req` and `ack` high together complete a handshake in ONE cycle. Use
// that module when an `ack` already asserted must not count as a response to this
// request; use this one when it may.
//
// Timing: `done` shows the count of completed handshakes and updates the cycle
// after the loop returns to its top, which is the cycle after the handshake
// completed. `req` is sampled every cycle until seen; `ack` from the same cycle.

use copper_core::port::{registered_wire, wire, In, RegOut};
use copper_core::{Bits, Clock, ClockDomain, Logic};
use copper_macros::hardware;
use copper_sim::HardwareExecutor;

struct MainClk;
impl ClockDomain for MainClk {}

#[hardware(sequential)]
async fn simple_handshake(
    clk: Clock<MainClk>,
    req: In<Logic, MainClk>,
    ack: In<Logic, MainClk>,
    done: RegOut<Bits<8>, MainClk>,
) {
    let mut n: Bits<8> = Bits::zero();
    loop {
        done.write(n);
        while req.read() == Logic::Zero {
            clk.tick().await;
        }
        while ack.read() == Logic::Zero {
            clk.tick().await;
        }
        n = n + Bits::from_lit::<1>();
        clk.tick().await;
    }
}

// ── Reference model: the source's control flow, walked by hand ────────────────
//
// `Top`: publish `done = n`, then test `req` and, if it is high, `ack` in the same
// cycle. `WaitReq`: the first `while`'s self-loop, which tests the same pair.
// `WaitAck`: the second `while`'s self-loop; on `ack` count and return to `Top`.
// The observed `done` after the edge is the value written this cycle (only at
// `Top`) or the previous value (a RegOut holds). `done` takes the count from
// BEFORE the increment, because both are non-blocking writes in the same cycle.

#[derive(Clone, Copy, PartialEq, Debug)]
enum St {
    Top,
    WaitReq,
    WaitAck,
}

struct Model {
    st: St,
    n: u8,
    done: u8,
}

impl Model {
    fn new() -> Self {
        Model { st: St::Top, n: 0, done: 0 }
    }

    /// Leaving the `req` wait falls straight into the `ack` test with no tick in
    /// between, so both conditions are read in one cycle. Shared by `Top` and
    /// `WaitReq`, which differ only in that `Top` also publishes `done`.
    fn advance(&mut self, req: bool, ack: bool) {
        if !req {
            self.st = St::WaitReq;
        } else if ack {
            self.n = self.n.wrapping_add(1);
            self.st = St::Top;
        } else {
            self.st = St::WaitAck;
        }
    }

    /// One clock: apply `(req, ack)`, return `done` as observed after the edge.
    fn step(&mut self, req: bool, ack: bool) -> u8 {
        match self.st {
            St::Top => {
                self.done = self.n;
                self.advance(req, ack);
            }
            St::WaitReq => self.advance(req, ack),
            St::WaitAck => {
                if ack {
                    self.n = self.n.wrapping_add(1);
                    self.st = St::Top;
                }
            }
        }
        self.done
    }
}

fn logic(b: bool) -> Logic {
    if b { Logic::One } else { Logic::Zero }
}

/// Simulate on `stream` (one `(req, ack)` per cycle) and return `done` per cycle.
fn simulate(stream: &[(bool, bool)]) -> Vec<u8> {
    let mut clk = Clock::<MainClk>::new();
    let mut exec = HardwareExecutor::new();
    let (req_drv, req_in) = wire::<Logic, MainClk>(Logic::Zero);
    let (ack_drv, ack_in) = wire::<Logic, MainClk>(Logic::Zero);
    let (done_drv, done_obs) = registered_wire::<Bits<8>, MainClk>(&clk, Bits::zero());
    let dh = done_drv.dirty_handle();
    let reads = vec![req_in.wire_id(), ack_in.wire_id()];
    exec.spawn_wired(simple_handshake(clk.clone(), req_in, ack_in, done_drv), vec![dh], reads);

    let mut out = Vec::with_capacity(stream.len());
    for &(req, ack) in stream {
        req_drv.write(logic(req));
        ack_drv.write(logic(ack));
        exec.tick_clock(&mut clk);
        out.push(done_obs.read().as_usize() as u8);
    }
    out
}

/// Every wait length from zero up, plus the case this module exists for:
/// `req` and `ack` high in the same cycle, which completes in one.
fn coverage_stream() -> Vec<(bool, bool)> {
    let s: &[(u8, u8)] = &[
        (0, 0), (0, 0),         // idle: waiting for req
        (1, 0),                 // req seen at Top, ack low -> WaitAck
        (0, 0), (0, 0),         // waiting for ack
        (0, 1),                 // ack seen                  -> done = 1 next cycle
        (1, 1),                 // Top with BOTH high: completes in this one cycle
        (0, 0),                 // Top again                 -> done = 2
        (1, 1),                 // and again, back to back
        (0, 0),                 // Top                       -> done = 3
        (0, 0), (1, 0),         // req after one wait cycle
        (1, 0), (1, 0), (1, 1), // ack after two wait cycles -> done = 4
        (0, 0), (0, 0),         // idle
        (1, 1),                 // one-cycle handshake from WaitReq, not Top
        (0, 0), (0, 0),         // idle                      -> done = 5
    ];
    s.iter().map(|&(r, a)| (r == 1, a == 1)).collect()
}

fn check(stream: &[(bool, bool)], verbose: bool) -> usize {
    let sim = simulate(stream);
    let mut model = Model::new();
    let mut mismatches = 0;
    for (c, &(req, ack)) in stream.iter().enumerate() {
        let expected = model.step(req, ack);
        if verbose {
            println!("cycle {c:2}: req={} ack={} -> done={}", req as u8, ack as u8, sim[c]);
        }
        if sim[c] != expected {
            mismatches += 1;
            println!("cycle {c}: simulator done={} but the model says {expected}", sim[c]);
        }
    }
    mismatches
}

fn main() {
    // A demo AND a self-check: the simulator against the hand model of the source.
    let m = check(&coverage_stream(), true);
    if m != 0 {
        eprintln!("FAIL: {m} cycle(s) disagree with the model");
        std::process::exit(1);
    }
    println!("simple_handshake: simulator == hand model ✓");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_handshake_matches_the_hand_model_on_the_coverage_stream() {
        assert_eq!(check(&coverage_stream(), false), 0);
    }

    #[test]
    fn req_and_ack_together_complete_in_one_cycle() {
        // The whole point of dropping the middle tick. Two cycles of both high
        // from reset: the first completes a handshake, and `done` shows it on the
        // cycle after — which the four-tick `handshake` cannot do.
        let stream: Vec<(bool, bool)> = std::iter::repeat((true, true)).take(4).collect();
        assert_eq!(simulate(&stream), vec![0, 1, 2, 3]);
    }

    #[test]
    fn a_request_never_acknowledged_never_counts() {
        let stream: Vec<(bool, bool)> = std::iter::repeat((true, false)).take(40).collect();
        assert_eq!(simulate(&stream).last(), Some(&0));
    }
}
