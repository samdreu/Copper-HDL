// Two element writes on an array register in one resumption are two statements of
// it, the last write to an element winning (SPEC I.2.4's `arr[i] = e` row, I.7.3).
//
// II.3 row (c), REFUSED-one (the transpiler; the macro admits) until Phase C step 5
// (2026-11-06, `design_docs/review/MEASUREMENTS.md` §J.6): an array register lowers as
// an internal memory with ONE write port, and a second element write in a cycle was
// refused ("written twice in one cycle"). Each element write of a segment now stages
// its own write port, in program order; the memory machinery gives the highest port
// priority at the commit and in every forwarding mux, so the last write wins.

/// Two writes to different elements, a read before them and one after (the
/// write-through), on dynamic and constant indices.
#[hardware(sequential)]
async fn arr_two_writes(
    clk: Clock<MainClk>,
    a: In<Bits<8>, MainClk>,
    b: In<Bits<8>, MainClk>,
    i: In<Bits<2>, MainClk>,
    k: In<Bits<2>, MainClk>,
    q: RegOut<Bits<8>, MainClk>,
    r: RegOut<Bits<8>, MainClk>,
) {
    let mut arr = [Bits::<8>::zero(); 4];
    loop {
        let before = arr[k.read().as_usize()];
        arr[0] = a.read();
        arr[i.read().as_usize()] = b.read();
        let after = arr[k.read().as_usize()];
        q.write(before);
        r.write(after);
        clk.tick().await;
    }
}

/// Two writes to the SAME dynamic element (last wins), and writes in exclusive arms.
#[hardware(sequential)]
async fn arr_same_element(
    clk: Clock<MainClk>,
    a: In<Bits<8>, MainClk>,
    c: In<Logic, MainClk>,
    i: In<Bits<2>, MainClk>,
    q: RegOut<Bits<8>, MainClk>,
) {
    let mut arr = [Bits::<8>::zero(); 4];
    loop {
        let j = i.read().as_usize();
        arr[j] = a.read();
        arr[j] = a.read() + Bits::from_u8(1);
        if c.read() == Logic::One {
            arr[3] = Bits::from_u8(7);
        } else {
            arr[0] = arr[1] + arr[2];
        }
        q.write(arr[0] ^ arr[3]);
        clk.tick().await;
    }
}

/// One write per cycle whose DATA reads the array combinationally: the element
/// written in the previous cycle is still in the write pipeline (an array register
/// commits a stage later, write latency 2), so the combinational read must forward
/// it. Admitted and divergent until Phase C step 5 — only `always_ff` reads were
/// forwarded (MEASUREMENTS §J.6; found by `arr_same_element`).
#[hardware(sequential)]
async fn arr_comb_read(
    clk: Clock<MainClk>,
    a: In<Bits<8>, MainClk>,
    i: In<Bits<2>, MainClk>,
    k: In<Bits<2>, MainClk>,
    q: RegOut<Bits<8>, MainClk>,
) {
    let mut arr = [Bits::<8>::zero(); 4];
    loop {
        arr[i.read().as_usize()] = arr[k.read().as_usize()] + a.read();
        q.write(arr[0]);
        clk.tick().await;
    }
}
