// A `continue` of a tick-free counted `for` (SPEC I.2.4: the `for` is an unrolling and the
// `continue` is Rust's — the rest of that iteration is skipped). Until 2026-11-08 the macro
// admitted it and the transpiler refused it with a message about the module loop (MEASUREMENTS
// §S.4, line `II.3 found tickfree-continue`); the transpiler now places the rest of the
// iteration under the negated condition (§U.1).

/// The number of set bits, skipping index 2 and every clear bit; a ticking body.
#[hardware(sequential)]
async fn tfc_sum_set_bits(
    clk: Clock<MainClk>,
    a: In<Bits<8>, MainClk>,
    o: RegOut<Bits<8>, MainClk>,
) {
    loop {
        let v = a.read();
        let mut s: Bits<8> = Bits::zero();
        for i in 0..8 {
            if i == 2 {
                continue;
            }
            if v[i] == Logic::Zero {
                continue;
            }
            s = s + Bits::from_lit::<1>();
        }
        o.write(s);
        clk.tick().await;
    }
}

/// `continue` in one arm of an `if`/`else` and in a `match` arm, with work after it.
#[hardware(combinational)]
pub fn tfc_branches<D: ClockDomain>(a: In<Bits<4>, D>, b: In<Bits<4>, D>, o: Out<Bits<4>, D>) {
    let av = a.read();
    let bv = b.read();
    let mut acc: Bits<4> = Bits::zero();
    for i in 0..4 {
        if av[i] == Logic::One {
            acc = acc ^ bv;
            continue;
        } else {
            acc = acc + Bits::from_lit::<1>();
        }
        match bv[i] {
            Logic::One => continue,
            _ => {}
        }
        acc = acc + Bits::from_lit::<2>();
    }
    o.write(acc);
}
