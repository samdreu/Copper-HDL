// A tick-bearing counted `for` over a range that is EMPTY ONLY AT RUN TIME runs
// zero iterations in zero time (SPEC I.8.4(b), D-10 A4-i).
//
// II.3 row (f1), ADMITTED-AND-DIVERGENT until Phase C step 5 (2026-11-06,
// `design_docs/review/MEASUREMENTS.md` §J.2): the extraction route hoists the
// body's last tick out of the desugared loop (it stands for the LAST iteration's
// tick) and took it even when the first exit test broke before any iteration —
// one extra cycle per zero-count pass (D-10 probe (ii): Verilator FAIL at cycles
// 0, 15, 16 of 24). The hoisted tick is now guarded on the counter having
// advanced from its start.

/// D-10's probe (ii): the count is an input, 0 on a quarter of the cycles.
#[hardware(sequential)]
async fn for_empty(clk: Clock<MainClk>, c: In<Bits<2>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    let mut n: Bits<8> = Bits::zero();
    loop {
        let cnt: u32 = c.read().as_u32();
        for _ in 0..cnt {
            n = n + Bits::from_u8(1);
            q.write(n);
            clk.tick().await;
        }
        q.write(Bits::from_u8(255));
        clk.tick().await;
    }
}

/// A NON-LITERAL start: `s..e`, both inputs read at entry. The guard compares the
/// counter against a snapshot of `s` taken at entry, not a re-read.
#[hardware(sequential)]
async fn for_empty_start(
    clk: Clock<MainClk>,
    s: In<Bits<2>, MainClk>,
    e: In<Bits<2>, MainClk>,
    q: RegOut<Bits<8>, MainClk>,
) {
    let mut n: Bits<8> = Bits::zero();
    loop {
        let lo: u32 = s.read().as_u32();
        let hi: u32 = e.read().as_u32();
        for _ in lo..hi {
            n = n + Bits::from_u8(1);
            q.write(n);
            clk.tick().await;
        }
        q.write(Bits::from_u8(255));
        clk.tick().await;
    }
}
