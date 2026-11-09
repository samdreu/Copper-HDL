// A `RegOut` written inside a tick-free `for` of a clocked body (line `II.3 found
// port-write-in-for`, MEASUREMENTS §Y.6, §Z.7). SPEC I.2.4 unrolls the loop, so each iteration's
// write is a write of the resumption and the last one is the drive (I.6). Until 2026-11-08 the
// transpiler refused it with the latch rule's message; it now accumulates the writes into a
// combinational temp and a written-flag, and drives the port once after the loop when the flag
// is set.

const PWF_TRIPS: usize = 4;

/// The ledger line's probe: every iteration writes the same input.
#[hardware(sequential)]
async fn pwf_probe(clk: Clock<MainClk>, a: In<Bits<8>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    loop {
        for _ in 0..3 {
            q.write(a.read());
        }
        clk.tick().await;
    }
}

/// A const-generic bound (`PARAMS`: N = 3), the written value depending on the iteration.
#[hardware(sequential)]
async fn pwf_generic_bound<const N: usize>(clk: Clock<MainClk>, a: In<Bits<8>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    let mut n: Bits<8> = Bits::zero();
    loop {
        for _ in 0..N {
            n = n + a.read();
            q.write(n);
        }
        clk.tick().await;
    }
}

/// A conditional write: the last iteration whose mask bit is set drives; none holds `q`.
#[hardware(sequential)]
async fn pwf_conditional(clk: Clock<MainClk>, a: In<Bits<8>, MainClk>, m: In<Bits<4>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    let mut k: Bits<8> = Bits::zero();
    loop {
        for i in 0..PWF_TRIPS {
            k = k + Bits::from_u8(1);
            if m.read()[i] == Logic::One {
                q.write(a.read() ^ k);
            }
        }
        clk.tick().await;
    }
}

/// A write before the loop that the loop may overwrite, and a second port written only in it.
#[hardware(sequential)]
async fn pwf_before_and_two_ports(
    clk: Clock<MainClk>,
    a: In<Bits<8>, MainClk>,
    c: In<Logic, MainClk>,
    q: RegOut<Bits<8>, MainClk>,
    r: RegOut<Bits<8>, MainClk>,
) {
    loop {
        q.write(a.read());
        for _ in 0..2 {
            if c.read() == Logic::One {
                q.write(a.read() + Bits::from_u8(7));
                r.write(a.read());
            }
        }
        clk.tick().await;
    }
}

/// Nested `for`s, and the extraction route (a tick in each arm).
#[hardware(sequential)]
async fn pwf_nested_phases(clk: Clock<MainClk>, a: In<Bits<8>, MainClk>, c: In<Logic, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    let mut s: Bits<8> = Bits::zero();
    loop {
        if c.read() == Logic::One {
            for _ in 0..2 {
                for _ in 0..3 {
                    s = s ^ a.read();
                    q.write(s + Bits::from_u8(1));
                }
            }
            clk.tick().await;
        } else {
            for _ in 0..2 {
                q.write(s);
            }
            clk.tick().await;
        }
    }
}
