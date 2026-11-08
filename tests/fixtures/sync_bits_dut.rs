// SPEC I.12.4(e) (D-17 A4b): a `synchronizer`'s foreign port may be `Bits<N>`, meaning N
// independent per-bit synchronizers — each bit read is the last-committed value of that
// bit, and nothing relates the bits. A witness fixture, not an anchor (2026-11-08,
// MEASUREMENTS §V.6): the corpus had no multi-bit crossing, so the per-bit reading had
// no module the three legs (simulator, interpreter, emitted SV) were compared on. The
// sweep drives `d` with random words every destination cycle, so every bit changes
// independently of the others.
//
// Domains `SyncSrc` / `SyncDst` are declared by the sweep's wrapper.

#[hardware(synchronizer)]
async fn sync_bits_2ff(clk: Clock<SyncDst>, d: In<Bits<4>, SyncSrc>, q: RegOut<Bits<4>, SyncDst>) {
    let mut ff1: Bits<4> = Bits::zero();
    loop {
        let ff2 = ff1;
        ff1 = d.read();
        q.write(ff2);
        clk.tick().await;
    }
}
