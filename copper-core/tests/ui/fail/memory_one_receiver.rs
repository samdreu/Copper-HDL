// SPEC I.10.5 (D-16 A7, D-25 (1)): one receiver per memory. A `Memory` is not `Clone`
// and nothing yields a second handle to its storage, so a memory handed to one module
// cannot be handed to another — rustc's move is the one-owner rule.
//
// Count 1 — the memory is already moved into its first receiver:

use copper_core::{Bits, Clock, ClockDomain, Memory};

struct Main; impl ClockDomain for Main {}
fn receiver_a(_c: Clock<Main>, _m: Memory<Bits<8>, 1, 1, Main, 1, 1>) {}
fn receiver_b(_c: Clock<Main>, _m: Memory<Bits<8>, 1, 1, Main, 1, 1>) {}

fn main() {
    let clk = Clock::<Main>::new();
    let m = Memory::<Bits<8>, 1, 1, Main, 1, 1>::new(clk.clone(), 16);
    receiver_a(clk.clone(), m);
    // E0382: use of moved value `m` — a memory has one receiver, by move semantics
    receiver_b(clk, m);
}
