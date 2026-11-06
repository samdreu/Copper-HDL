// Probes for the agreement relation's two non-agreement verdicts (SPEC §III.2.2 and
// §III.2.4), measured by tests/agreement_relation.rs and recorded in
// design_docs/review/MEASUREMENTS.md §E. Not in tests/fixtures/: each one exists to
// LEAVE the relation (an X observation, a run-time failure) under a stimulus the
// test chooses, and the sweep's random stimulus would reach those points at random
// cycles instead.

// (ii) An X observation: `a % b` with `b == 0` is all-X (SPEC §I.14.2, the operator
// table), shown from the next observation. The SV pair leaves the relation there
// (III.2.2); sim ≡ ref still compares it exactly.
#[hardware(sequential)]
async fn xp_rem(clk: Clock<MainClk>, a: In<Bits<8>, MainClk>, b: In<Bits<8>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    loop {
        let r = a.read() % b.read();
        q.write(r);
        clk.tick().await;
    }
}

// (iii-a) An X fed to a condition (I.14.2(c)): bit 0 of `a % b` is X when `b == 0`,
// and `as_bool` on it is a run-time failure — the simulator panics
// ("Cannot convert X to bool"), the interpreter errs. (`r` is also written so every
// bit of it is used: an unused slice is fatal under Verilator's -Wall.)
#[hardware(sequential)]
async fn trunc_x_cond(clk: Clock<MainClk>, a: In<Bits<8>, MainClk>, b: In<Bits<8>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    loop {
        let r = a.read() % b.read();
        if r[0].as_bool() {
            q.write(r);
        } else {
            q.write(b.read());
        }
        clk.tick().await;
    }
}

// (iii-b) An out-of-range memory address (I.14.6(b)): a 10-entry ROM read at a 4-bit
// address; any address ≥ 10 panics at the `read()` call.
#[hardware(sequential)]
async fn trunc_oob(clk: Clock<MainClk>, addr: In<Bits<4>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    let mem = Memory::<Bits<8>, 1, 0, MainClk, 1, 1>::from_fn(clk.clone(), 10, |i| Bits::from_usize(i + 3));
    loop {
        mem.read_port::<0>().read(addr.read().as_usize());
        clk.tick().await;
        q.write(mem.read_port::<0>().data());
    }
}

// (iii-c) An undelivered `data()` (I.10, D-16): the read is issued only while `go`,
// and the data is taken after every tick, so the first iteration with `go == 0`
// takes data no read delivered.
#[hardware(sequential)]
async fn trunc_data(clk: Clock<MainClk>, go: In<Logic, MainClk>, addr: In<Bits<4>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    let rom = Memory::<Bits<8>, 1, 0, MainClk, 1, 1>::from_fn(clk.clone(), 16, |i| Bits::from_usize(i * 5 + 1));
    loop {
        if go.read() == Logic::One {
            rom.read_port::<0>().read(addr.read().as_usize());
        }
        clk.tick().await;
        q.write(rom.read_port::<0>().data());
    }
}
