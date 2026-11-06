// Concrete (transpilable) specialization of the standard-library synchronizer
// `copper::sync_2ff` (`src/sync.rs`). Body kept character-for-character identical
// to the library module — only the generic domain parameters are pinned, because
// `copper-transpile` only handles concrete modules.
//
// `tests/cdc_synchronizer_anchor.rs` asserts this specialization is behaviourally
// identical to the library generic in the simulator, so anchoring this concrete
// module anchors the library primitive.
//
// Domains `SrcClk` / `DstClk` are declared by the including test file.

#[hardware(synchronizer)]
async fn sync_2ff_concrete(clk: Clock<DstClk>, d: In<Logic, SrcClk>, q: RegOut<Logic, DstClk>) {
    let mut ff1 = Logic::Zero;
    loop {
        let ff2 = ff1;
        ff1 = d.read();
        q.write(ff2);
        clk.tick().await;
    }
}
