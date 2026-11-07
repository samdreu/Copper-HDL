// A register assigned in a resumption is read by that resumption as the value it
// was assigned (SPEC I.3.4, I.7.3) — however the read reaches the output.
//
// II.3 row (f5), ADMITTED-AND-DIVERGENT until Phase C step 5 (2026-11-06,
// `design_docs/review/MEASUREMENTS.md` §J.3): `shir_lower` forwarded a register's
// new value into every EDGE-sampled consumer (a `RegOut` drive, a memory staging,
// the `always_ff` copy of a branch test) but not into what the `always_comb` block
// computes — an array element assignment, a `let` wire's value, the combinational
// copy of an `if` test. A drive inlines a wire it reads, which hid the gap until the
// value travelled through something a drive cannot inline. `f5_sipo` is
// `q1_c_sipo_block` (`review/Q1_TRADEOFFS.md` §4 (ii)); the other three are the
// family the row's site turned out to be.

/// q1_c_sipo_block (Q1_TRADEOFFS §4 (ii)): `w3` carried across the back edge,
/// assigned in the last segment, read through `bits[12 + k] = w3[k]`.
#[hardware(sequential)]
async fn f5_sipo(
    clk: Clock<MainClk>,
    data_i: In<Bits<4>, MainClk>,
    data_o: RegOut<Bits<16>, MainClk>,
    w3_dbg: RegOut<Bits<4>, MainClk>,
) {
    let mut w3: Bits<4> = Bits::zero();
    loop {
        let w0 = data_i.read();
        clk.tick().await;
        let w1 = data_i.read();
        clk.tick().await;
        let w2 = data_i.read();
        clk.tick().await;
        w3 = data_i.read();
        let mut bits = [Logic::Zero; 16];
        for k in 0..4 {
            bits[k] = w0[k];
            bits[4 + k] = w1[k];
            bits[8 + k] = w2[k];
            bits[12 + k] = w3[k];
        }
        data_o.write(Bits::from_slice(&bits));
        clk.tick().await;
        w3_dbg.write(w3);
    }
}

/// The element write's GUARD reads the register assigned before it.
#[hardware(sequential)]
async fn f5_guard(clk: Clock<MainClk>, x: In<Bits<4>, MainClk>, q: RegOut<Bits<4>, MainClk>) {
    let mut r: Bits<4> = Bits::zero();
    loop {
        r = r + x.read();
        let mut bits = [Logic::Zero; 4];
        if r[0] == Logic::One {
            bits[1] = Logic::One;
        }
        bits[0] = r[3];
        q.write(Bits::from_slice(&bits));
        clk.tick().await;
    }
}

/// Through a `let` wire: `let y = r + 1; bits[k] = y[k]`.
#[hardware(sequential)]
async fn f5_wire(clk: Clock<MainClk>, x: In<Bits<8>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    let mut r: Bits<8> = Bits::zero();
    loop {
        r = r + x.read();
        let y = r + Bits::from_u8(1);
        let mut bits = [Logic::Zero; 8];
        for k in 0..8 {
            bits[k] = y[k];
        }
        q.write(Bits::from_slice(&bits));
        clk.tick().await;
    }
}

/// A wire assigned in a branch whose condition reads the register.
#[hardware(sequential)]
async fn f5_branch_wire(clk: Clock<MainClk>, x: In<Bits<8>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    let mut r: Bits<8> = Bits::zero();
    loop {
        r = r + x.read();
        let mut t: Bits<8> = Bits::zero();
        if r[0] == Logic::One {
            t = Bits::from_u8(5);
        }
        q.write(t);
        clk.tick().await;
    }
}
