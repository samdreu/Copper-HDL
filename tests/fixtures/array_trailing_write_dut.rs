// An array register written AFTER its tick and read before it in the next iteration (line `II.3
// found array-register-trailing-write`, MEASUREMENTS §Z.9, §AA.1). The trailing write and the
// next head's read are one resumption (SPEC I.0.2): the head reads the word the trailing write
// just staged, through the write-through mux, as it reads an earlier write of its own segment.
// Until 2026-11-09 the head muxed only the previous resumption's staged write (`r_wr0_s1_*`) and
// the probe disagreed on 8/8 sequences, `sim=ref≠SV`. Each module is swept on three legs.

/// The probe of §Z.9: read before the tick, write after it.
#[hardware(sequential)]
async fn atw_probe(clk: Clock<MainClk>, a: In<Bits<2>, MainClk>, d: In<Bits<8>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    let mut r: [u8; 4] = [0u8; 4];
    loop {
        q.write(Bits::from_u8(r[a.read().as_usize()]));
        clk.tick().await;
        r[a.read().as_usize()] = d.read().as_u8();
    }
}

/// A head write AND a trailing write to one array: the head's write takes the port after the
/// trailing region's, a read before it sees the trailing write, a read after it sees both.
#[hardware(sequential)]
async fn atw_head_and_trailing(
    clk: Clock<MainClk>,
    a: In<Bits<2>, MainClk>,
    b: In<Bits<2>, MainClk>,
    d: In<Bits<8>, MainClk>,
    q: RegOut<Bits<8>, MainClk>,
    p: RegOut<Bits<8>, MainClk>,
) {
    let mut r: [u8; 4] = [1, 2, 3, 4];
    loop {
        q.write(Bits::from_u8(r[a.read().as_usize()]));
        r[b.read().as_usize()] = d.read().as_u8() ^ 0x5a;
        p.write(Bits::from_u8(r[a.read().as_usize()]));
        clk.tick().await;
        r[a.read().as_usize()] = d.read().as_u8();
    }
}

/// Two trailing writes, the second under a condition: the head reads the later one where both
/// hit its address.
#[hardware(sequential)]
async fn atw_two_trailing(
    clk: Clock<MainClk>,
    a: In<Bits<2>, MainClk>,
    c: In<Logic, MainClk>,
    d: In<Bits<8>, MainClk>,
    q: RegOut<Bits<8>, MainClk>,
) {
    let mut r: [u8; 4] = [9, 8, 7, 6];
    loop {
        q.write(Bits::from_u8(r[a.read().as_usize()].wrapping_add(r[0])));
        clk.tick().await;
        r[a.read().as_usize()] = d.read().as_u8();
        if c.read() == Logic::One {
            r[(a.read().as_usize() + 1) & 3] = d.read().as_u8().wrapping_add(1);
        }
    }
}

/// Two phases: the trailing region joins phase 0 ahead of the head; phase 1 reads through the
/// staged write of phase 0's resumption.
#[hardware(sequential)]
async fn atw_two_phases(clk: Clock<MainClk>, a: In<Bits<2>, MainClk>, d: In<Bits<8>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    let mut r: [u8; 4] = [0u8; 4];
    loop {
        q.write(Bits::from_u8(r[a.read().as_usize()]));
        clk.tick().await;
        q.write(Bits::from_u8(r[a.read().as_usize()] ^ 0xff));
        clk.tick().await;
        r[a.read().as_usize()] = d.read().as_u8();
    }
}

/// A pre-loop write (the first resumption's, SPEC I.9.3) and a trailing write share the low port;
/// they never run in one resumption.
#[hardware(sequential)]
async fn atw_preloop_and_trailing(clk: Clock<MainClk>, a: In<Bits<2>, MainClk>, d: In<Bits<8>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    let mut r: [u8; 4] = [0u8; 4];
    r[1] = 0x11;
    loop {
        q.write(Bits::from_u8(r[a.read().as_usize()]));
        clk.tick().await;
        r[a.read().as_usize()] = d.read().as_u8();
    }
}

/// A branch-nested tick: the extraction route (`match pc`), where the write after the last tick
/// and the head's read are arms of one FSM.
#[hardware(sequential)]
async fn atw_extraction(
    clk: Clock<MainClk>,
    a: In<Bits<2>, MainClk>,
    c: In<Logic, MainClk>,
    d: In<Bits<8>, MainClk>,
    q: RegOut<Bits<8>, MainClk>,
) {
    let mut r: [u8; 4] = [5, 6, 7, 8];
    loop {
        q.write(Bits::from_u8(r[a.read().as_usize()]));
        if c.read() == Logic::One {
            clk.tick().await;
        }
        clk.tick().await;
        r[a.read().as_usize()] = d.read().as_u8();
    }
}
