// SPEC II.2.2 (P22), R3's over-refusal narrowed in Phase C step 10: a nested loop is a
// clock boundary of its enclosing loop when every path to one of its `break`s has
// ticked — the walk descends `if`/`match` arm by arm (`copper_analysis::cfg::
// untick_paths`). Until step 10 it answered "exitable" at the first statement that
// held a tick without being one, and both front-ends refused these two shapes
// (`livelock_a`, `livelock_b` of D-12 §0 (iii)) though every path ticks. Swept here so
// the admission is measured on all three legs, not only parsed; the same shapes are
// `copper-macros/tests/ui/pass/livelock_{a,b}.rs`.

/// The nested `loop` has no `break`; an `if` with a tick, then the bare tick. The
/// enclosing loop's only boundary is the nested loop, which is never left.
#[allow(clippy::never_loop)]
#[hardware(sequential)]
async fn boundary_tick_in_arm(clk: Clock<MainClk>, c: In<Logic, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    let mut n: Bits<8> = Bits::zero();
    loop {
        loop {
            let t = c.read();
            if t == Logic::One {
                n = n + Bits::from_lit::<1>();
                q.write(n);
                clk.tick().await;
            }
            q.write(n);
            clk.tick().await;
        }
    }
}

/// Both arms of an `if`/`else` tick, then the `break` test, then the bare tick; the
/// enclosing loop writes `q` after the nested loop, in the resumption the `break`
/// starts (the zero-time continuation, I.8.2).
#[hardware(sequential)]
async fn boundary_both_arms_tick(
    clk: Clock<MainClk>,
    c: In<Logic, MainClk>,
    d: In<Logic, MainClk>,
    q: RegOut<Bits<8>, MainClk>,
) {
    let mut n: Bits<8> = Bits::zero();
    loop {
        loop {
            let t = c.read();
            if t == Logic::One {
                n = n + Bits::from_lit::<1>();
                clk.tick().await;
            } else {
                clk.tick().await;
            }
            let done = d.read();
            if done == Logic::One {
                break;
            }
            clk.tick().await;
        }
        q.write(n);
    }
}
