// A `continue` of a NESTED loop is a zero-time return to that loop's head, in the
// same resumption (SPEC I.8.3, D-10 A2-i).
//
// II.3 row (a), REFUSED-one (the transpiler; the macro admits) until Phase C step 5
// (2026-11-06, `design_docs/review/MEASUREMENTS.md` §J.4). The transpiler refused
// every nested `continue` because a nested loop `W ; tick ; C`'s head state holds the
// ROTATED `C ; W`. The nested-loop shape rule admits only a body whose one top-level
// tick is LAST (`C` empty), so the head holds `W` as written and the `continue` is a
// zero-time goto to it; a non-trivial rotation, if the rule ever admits one, runs the
// loop's entry copy instead. The shape the macro admits (D-10's parity probe): a
// `continue` after a tick inside a branch of `W`.

#[hardware(sequential)]
async fn nested_continue(
    clk: Clock<MainClk>,
    a: In<Logic, MainClk>,
    go: In<Logic, MainClk>,
    stop: In<Logic, MainClk>,
    q: RegOut<Bits<8>, MainClk>,
) {
    let mut n: Bits<8> = Bits::zero();
    loop {
        q.write(n);
        loop {
            if a.read() == Logic::One {
                clk.tick().await;
                if go.read() == Logic::One {
                    continue;
                }
                n = n + Bits::from_u8(1);
                q.write(n);
                if stop.read() == Logic::One {
                    break;
                }
            }
            clk.tick().await;
        }
        n = n + Bits::from_u8(16);
        clk.tick().await;
    }
}

/// The `continue` in a `match` arm after a branch tick, with a register update before
/// it in the same resumption (kept: the `continue` leaves after it).
#[hardware(sequential)]
async fn nested_continue_match(
    clk: Clock<MainClk>,
    a: In<Logic, MainClk>,
    sel: In<Bits<2>, MainClk>,
    q: RegOut<Bits<8>, MainClk>,
) {
    let mut n: Bits<8> = Bits::zero();
    loop {
        loop {
            if a.read() == Logic::One {
                clk.tick().await;
                match sel.read().as_u8() {
                    0 => {
                        n = n + Bits::from_u8(1);
                        q.write(n);
                        continue;
                    }
                    1 => break,
                    _ => q.write(Bits::from_u8(200)),
                }
                n = n + Bits::from_u8(4);
            }
            clk.tick().await;
        }
        q.write(n);
        clk.tick().await;
    }
}
