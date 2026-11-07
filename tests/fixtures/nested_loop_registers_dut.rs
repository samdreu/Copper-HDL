// The two nested-loop shapes of SPEC I.7.5 (Phase C step 6, MEASUREMENTS §K):
// register inference through a tick-bearing nested loop's own control flow. Until
// 2026-11-07 such a loop was ONE folded node of the inference's CFG, with no
// definitions and its out-edge a tick, which got each of these wrong in a different
// direction. Both are swept (simulator, interpreter, transpiled SV) and reconciled
// (`copper-codegen/tests/register_reconciliation.rs`).

// MISSED by the fold: `v` is assigned before the `for` and read inside it after an
// interior tick — live at that tick, a register of I.7.1. The fold saw the read at
// the node's entry only, where no tick precedes it, and inferred {i}. The
// descended graph infers {i, v}. `x` changes every cycle, so a `v` that were not
// held would show at `q` from the second iteration on. (`fold_defect` in
// `tests/ref_probes/` writes `v + Bits::from_usize(i)`; that method's width is the
// open I.14.7 (3) class, so the swept twin reads `i` in a comparison instead.)
#[hardware(sequential)]
async fn nested_loop_missed(
    clk: Clock<MainClk>,
    x: In<Bits<8>, MainClk>,
    q: RegOut<Bits<8>, MainClk>,
) {
    loop {
        let v = x.read();
        for i in 0..4 {
            if i == 3 {
                q.write(x.read());
            } else {
                q.write(v);
            }
            clk.tick().await;
        }
    }
}

// ADDED by the fold: `w` is assigned inside the nested loop and read only after it.
// The loop is left only by the `break` that follows the assignment with no tick
// between, so the value read after the loop is the one assigned in the same
// resumption, and the value assigned before the interior tick is reassigned after
// it before any read: `w` is dead at every tick — a wire (and its initializer is
// never read, because the `loop` body runs at least once). The fold carried it out
// of the node as live across a tick and inferred {w}. The descended graph infers {}.
#[hardware(sequential)]
async fn nested_loop_added(
    clk: Clock<MainClk>,
    x: In<Bits<8>, MainClk>,
    q: RegOut<Bits<8>, MainClk>,
) {
    loop {
        let mut w: Bits<8> = Bits::zero();
        loop {
            w = x.read();
            if w[0] == Logic::One {
                break;
            }
            clk.tick().await;
        }
        q.write(w + Bits::from_lit::<1>());
        clk.tick().await;
    }
}
