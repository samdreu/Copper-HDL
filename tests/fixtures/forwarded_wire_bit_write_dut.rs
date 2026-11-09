// A bit write to a wire of a clocked segment whose value the forwarding walk records — its
// initializer reads a register assigned earlier in the segment (line `II.3 found
// forwarded-wire-bit-write`, MEASUREMENTS §Y.4, §Z.2). Until 2026-11-08 the walk had no arm
// for the bit write, so an edge consumer read the value from before it (`q <= (f ^ a)` for
// `fwb_probe`), and the wire, left unread, failed Verilator's lint. The walk now updates the
// recorded value with the whole-value form `(v & ~(W'(1) << i)) | (W'(e) << i)`, and a wire
// read nowhere loses its bit writes with its assignments.

/// The ledger line's probe.
#[hardware(sequential)]
async fn fwb_probe(clk: Clock<MainClk>, a: In<Bits<4>, MainClk>, d: In<Logic, MainClk>, q: RegOut<Bits<4>, MainClk>) {
    let mut f: Bits<4> = Bits::zero();
    loop {
        f = f ^ a.read();
        let mut s: Bits<4> = f;
        s[0] = d.read();
        q.write(s);
        clk.tick().await;
    }
}

/// The same wire read after the tick (promoted) — it agreed before; pinned with the rest.
#[hardware(sequential)]
async fn fwb_promoted(clk: Clock<MainClk>, a: In<Bits<4>, MainClk>, d: In<Logic, MainClk>, q: RegOut<Bits<4>, MainClk>) {
    let mut f: Bits<4> = Bits::zero();
    loop {
        f = f ^ a.read();
        let mut s: Bits<4> = f;
        s[0] = d.read();
        clk.tick().await;
        q.write(s);
    }
}

/// A run-time index, a bit write under a branch, and the wire fed back into the register.
#[hardware(sequential)]
async fn fwb_branch_dyn(
    clk: Clock<MainClk>,
    a: In<Bits<8>, MainClk>,
    i: In<Bits<3>, MainClk>,
    d: In<Logic, MainClk>,
    c: In<Logic, MainClk>,
    q: RegOut<Bits<8>, MainClk>,
) {
    let mut f: Bits<8> = Bits::zero();
    loop {
        f = f + a.read();
        let mut s: Bits<8> = f;
        if c.read().as_bool() {
            s[i.read().as_usize()] = d.read();
        } else {
            s[7] = d.read();
        }
        s[1] = c.read();
        f = s ^ f;
        q.write(s);
        clk.tick().await;
    }
}

/// On the extraction route (a tick in each arm).
#[hardware(sequential)]
async fn fwb_phases(
    clk: Clock<MainClk>,
    a: In<Bits<4>, MainClk>,
    d: In<Logic, MainClk>,
    c: In<Logic, MainClk>,
    q: RegOut<Bits<4>, MainClk>,
) {
    let mut f: Bits<4> = Bits::zero();
    loop {
        f = f ^ a.read();
        let mut s: Bits<4> = f;
        if c.read().as_bool() {
            s[2] = d.read();
            q.write(s);
            clk.tick().await;
        } else {
            s[3] = d.read();
            q.write(s + f);
            clk.tick().await;
        }
    }
}
