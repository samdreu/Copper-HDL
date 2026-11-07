// A nested loop whose ticks lie only inside `if`/`match` arms: its tick sites are
// suspension sites like any other (SPEC I.8.2).
//
// II.3 row (b), REFUSED-both until Phase C step 5 (2026-11-06,
// `design_docs/review/MEASUREMENTS.md` §J.5): `check_nested_loop_shape` clause 4 refused
// the shape in both front-ends, because the lowering arm behind it diverged on `b2`
// (D-13 §0 (ii)): each arm's post-tick continuation fell through to the loop head with
// `pc = head`, a state transition costing a cycle, where the coroutine runs the
// continuation, the head test and the next arm's work in one resumption. The
// fall-through of such a loop is now a zero-time return to the head (its state holds
// the body as written), and the clause is lifted in both front-ends. `b1` and `b2` are
// D-13's two measured shapes.

/// `b1` — the wait shape: agreed even on the gated arm (its continuation is empty).
#[hardware(sequential)]
async fn arms_only_b1(clk: Clock<MainClk>, c: In<Logic, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    let mut n: Bits<8> = Bits::zero();
    loop {
        loop {
            if c.read() == Logic::One {
                clk.tick().await;
            } else {
                break;
            }
        }
        n = n + Bits::from_u8(1);
        q.write(n);
        clk.tick().await;
    }
}

/// `b2` — work after the arms' ticks and a head test: diverged on the gated arm at 41
/// of 48 cycles.
#[hardware(sequential)]
async fn arms_only_b2(clk: Clock<MainClk>, c: In<Logic, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    let mut n: Bits<8> = Bits::zero();
    loop {
        loop {
            if c.read() == Logic::One {
                n = n + Bits::from_u8(1);
                clk.tick().await;
            } else {
                n = n + Bits::from_u8(2);
                clk.tick().await;
            }
            q.write(n);
            if n[3] == Logic::One {
                break;
            }
        }
        n = Bits::zero();
        q.write(Bits::from_u8(99));
        clk.tick().await;
    }
}

/// A `match` with a tick in every arm and a post-tick continuation in each.
#[hardware(sequential)]
async fn arms_only_match(clk: Clock<MainClk>, sel: In<Bits<2>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    let mut n: Bits<8> = Bits::zero();
    loop {
        loop {
            match sel.read().as_u8() {
                0 => {
                    clk.tick().await;
                    n = n + Bits::from_u8(1);
                }
                1 => {
                    n = n + Bits::from_u8(3);
                    clk.tick().await;
                    q.write(n);
                }
                _ => {
                    clk.tick().await;
                    clk.tick().await;
                    n = n + Bits::from_u8(5);
                }
            }
            if n[4] == Logic::One {
                break;
            }
        }
        q.write(n);
        n = Bits::zero();
        clk.tick().await;
    }
}
