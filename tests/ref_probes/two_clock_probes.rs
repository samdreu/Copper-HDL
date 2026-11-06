// Two-clock probes for `tests/two_clock_pins.rs` (Phase C step 2; SPEC §I.12.2,
// §III.2.6). Domains `ClkA` / `ClkB` are declared by the including test file.
// Outside `tests/fixtures/` on purpose: the sweep drives one clock, and a module
// whose meaning is about a second domain is checked by the hand-written pins.

// D-17 probe (i), the never-ticked domain: a module in B whose first resumption
// must run in B's first settle, not in the first settle of whichever domain ticks
// first. One job: the read sees the input standing before the edge it ends at.
#[hardware(sequential)]
async fn b_reg(clk: Clock<ClkB>, x: In<Bits<8>, ClkB>, q: RegOut<Bits<8>, ClkB>) {
    loop {
        let t = x.read();
        q.write(t);
        clk.tick().await;
    }
}
