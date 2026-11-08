// An element write to an array register inside a combinational `for` (line `II.3 found
// array-for`, MEASUREMENTS §J.10 item 6 (i), §U.4). Until 2026-11-08 the transpiler refused it
// (one write site, one write port, its nets reassigned per iteration — before that refusal only
// the last iteration's write committed); the `for` is now unrolled (I.2.4) and each iteration's
// write takes its own port, row (c)'s write-port form.

/// Every word accumulates the input; a word is read back at a run-time index.
#[hardware(sequential)]
async fn afw_accumulate(
    clk: Clock<MainClk>,
    a: In<Bits<8>, MainClk>,
    k: In<Bits<2>, MainClk>,
    q: RegOut<Bits<8>, MainClk>,
) {
    let mut arr = [Bits::<8>::zero(); 4];
    loop {
        q.write(arr[k.read().as_usize()]);
        for j in 0..4 {
            arr[j] = a.read() + arr[j];
        }
        clk.tick().await;
    }
}

/// A shift through the array (each iteration reads the word the previous one wrote, Rust's
/// order), a conditional write, and a read after the loop in the same resumption.
#[hardware(sequential)]
async fn afw_shift_cond(
    clk: Clock<MainClk>,
    a: In<Bits<4>, MainClk>,
    en: In<Logic, MainClk>,
    q: RegOut<Bits<4>, MainClk>,
    r: RegOut<Bits<4>, MainClk>,
) {
    let mut arr = [Bits::<4>::zero(); 4];
    loop {
        for j in 1..4 {
            arr[j] = arr[j - 1];
        }
        arr[0] = a.read();
        for j in 0..2 {
            if en.read() == Logic::One {
                arr[j + 2] = arr[j] ^ arr[3];
            }
        }
        q.write(arr[3]);
        r.write(arr[1]);
        clk.tick().await;
    }
}
