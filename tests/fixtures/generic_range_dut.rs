// A tick-bearing counted `for` over a range bounded by a const generic, where R3's verdict depends on
// its being non-empty (line `II.3 found ticking-for-generic-range`, D-30, MEASUREMENTS §AE.5, §AF).
// R3 takes such a range as non-empty in both front-ends (II.2.2); since D-30 both hold the hypothesis
// per instance — the macro with an inline `const { assert!(..) }` (rustc refuses an empty instance
// at monomorphization, `src/lib.rs`'s D-30 doctest), the transpiler by lowering the `for` as
// non-empty and emitting an elaboration-time `$error` (`tests/generic_range_instance_d30.rs`).
// Until D-30 the macro admitted these with no check and the transpiler refused them (its
// zero-time cycle test saw the range's empty exit). Swept at the widths of build.rs's `PARAMS`.

const GRN_ONE: usize = 1;

/// The parametric delay: a `for` over `0..W` is the module loop's only boundary (MEASUREMENTS §AE.5's `zzi_29`).
#[hardware(sequential)]
async fn grn_only_boundary<const W: usize>(clk: Clock<MainClk>, a: In<Bits<4>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    loop {
        q.write(Bits::from_u8(a.read().as_u8() + 16));
        for _ in 0..W {
            clk.tick().await;
        }
    }
}

/// A generic counted delay followed by a skippable wait (`zzi_30`).
#[hardware(sequential)]
async fn grn_then_wait<const W: usize>(clk: Clock<MainClk>, a: In<Bits<4>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    loop {
        for j in 0..W {
            q.write(Bits::from_u8(j as u8));
            clk.tick().await;
        }
        q.write(Bits::from_u8(9));
        loop {
            if a.read().as_u8() & 1 == 1 {
                break;
            }
            clk.tick().await;
        }
    }
}

/// A hand-written nested `loop` whose boundary is a generic inner `for`, ending in a wait (`zzi_31`).
#[hardware(sequential)]
async fn grn_handwritten<const W: usize>(clk: Clock<MainClk>, a: In<Bits<4>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    let mut c: u8 = 0;
    loop {
        loop {
            if c >= 2 {
                break;
            }
            c = c + 1;
            for _ in 0..W {
                clk.tick().await;
            }
            q.write(Bits::from_u8(c));
            loop {
                if a.read().as_u8() & 1 == 1 {
                    break;
                }
                clk.tick().await;
            }
        }
        c = 0;
        clk.tick().await;
    }
}

/// `ticking-for-ends-in-loop`'s shape, the inner range generic (`zzi_04`) and inclusive through a file `const`.
#[hardware(sequential)]
async fn grn_inner_ends_wait<const W: usize>(clk: Clock<MainClk>, a: In<Bits<4>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    loop {
        for i in 0u8..2 {
            for _ in 0..W {
                clk.tick().await;
            }
            for k in GRN_ONE..=(W + GRN_ONE) {
                q.write(Bits::from_u8(k as u8 + i * 16));
                clk.tick().await;
            }
            q.write(Bits::from_u8(i));
            loop {
                if a.read().as_u8() & 1 == 1 {
                    break;
                }
                clk.tick().await;
            }
        }
    }
}

/// The outer `for` over `0..W` ends in the wait (`zzi_32`).
#[hardware(sequential)]
async fn grn_outer_ends_wait<const W: usize>(clk: Clock<MainClk>, a: In<Bits<4>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    loop {
        for i in 0..W {
            for _ in 0u8..2 {
                clk.tick().await;
            }
            q.write(Bits::from_u8(i as u8));
            loop {
                if a.read().as_u8() & 1 == 1 {
                    break;
                }
                clk.tick().await;
            }
        }
    }
}

/// Two generic delays in a row, each the boundary of a path — a set of two, each range checked on its own (recorded over-check).
#[hardware(sequential)]
async fn grn_two_generic<const W: usize, const V: usize>(clk: Clock<MainClk>, a: In<Bits<4>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    loop {
        q.write(Bits::from_u8(1));
        for _ in 0..W {
            clk.tick().await;
        }
        q.write(Bits::from_u8(a.read().as_u8()));
        for _ in 0..V {
            clk.tick().await;
        }
    }
}
