// A tick-bearing `for` inside a tick-bearing nested loop, followed in that loop's body by more
// statements and the body's own tick (line `II.3 found nested-ticking-for`, MEASUREMENTS §AC.7,
// §AD.2). SPEC I.8.2 makes every tick of a nested loop a suspension site and I.8.5's `D` composes,
// so the shape has its Rust meaning; the macro admitted it (I.8.2(b): the body's only bare tick is
// its last statement). Until 2026-11-09 the transpiler refused it: the inner `for`'s desugar left
// its hoisted tick as a bare statement in the middle of the enclosing body, which the flattener's
// copy of the shape rule then read as `W; tick; C; tick`. Such a `for` now takes I.8.5's form
// without the hoisting (`control_extract::expand_counted_for`), its continuation reaching the
// enclosing body's tick.

/// Two constant ranges, distinct variables.
#[hardware(sequential)]
async fn ntf_distinct(clk: Clock<MainClk>, a: In<Bits<4>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    loop {
        for i in 0u8..2 {
            for j in 5u8..7 {
                q.write(Bits::from_u8(j ^ a.read().as_u8()));
                clk.tick().await;
            }
            q.write(Bits::from_u8(i << 4));
            clk.tick().await;
        }
    }
}

/// The counters interact: the inner range is `0..i`, empty on the outer loop's first iteration
/// (zero iterations in zero time), and the inner body reads both counters.
#[hardware(sequential)]
async fn ntf_interact(clk: Clock<MainClk>, a: In<Bits<4>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    loop {
        for i in 0u8..4 {
            for j in 0..i {
                q.write(Bits::from_u8((i << 4) | j | (a.read().as_u8() & 8)));
                clk.tick().await;
            }
            q.write(Bits::from_u8(0x80 | i));
            clk.tick().await;
        }
    }
}

/// An inner `break` on an input, before the inner tick; the outer variable read after the inner
/// loop (live across its ticks: a register).
#[hardware(sequential)]
async fn ntf_break(clk: Clock<MainClk>, a: In<Bits<4>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    loop {
        for i in 0u8..3 {
            for j in 0u8..4 {
                if a.read().as_u8() & 1 == 1 {
                    break;
                }
                q.write(Bits::from_u8(j + 10));
                clk.tick().await;
            }
            q.write(Bits::from_u8(i + 100));
            clk.tick().await;
        }
    }
}

/// An inner range empty at run time (the bound read from an input): zero iterations, zero time.
#[hardware(sequential)]
async fn ntf_runtime_empty(clk: Clock<MainClk>, a: In<Bits<4>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    loop {
        for i in 0u8..2 {
            let n: u8 = a.read().as_u8() & 3;
            for _ in 0..n {
                q.write(Bits::from_u8(n + 40));
                clk.tick().await;
            }
            q.write(Bits::from_u8(i + 50));
            clk.tick().await;
        }
    }
}

/// An inclusive inner range, three levels deep, an anonymous middle counter.
#[hardware(sequential)]
async fn ntf_three(clk: Clock<MainClk>, a: In<Bits<4>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    loop {
        for i in 0u8..2 {
            for _ in 0u8..2 {
                for k in 1u8..=2 {
                    q.write(Bits::from_u8((i << 4) | k | (a.read().as_u8() & 4)));
                    clk.tick().await;
                }
                q.write(Bits::from_u8(0x60 | i));
                clk.tick().await;
            }
            q.write(Bits::from_u8(0x70));
            clk.tick().await;
        }
    }
}

/// The inner `for` in a nested `loop` and in a `while`, each followed by statements and a tick.
#[hardware(sequential)]
async fn ntf_in_loop_while(clk: Clock<MainClk>, a: In<Bits<4>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    loop {
        loop {
            for j in 5u8..7 {
                q.write(Bits::from_u8(j ^ a.read().as_u8()));
                clk.tick().await;
            }
            if a.read().as_u8() & 2 == 2 {
                break;
            }
            q.write(Bits::from_u8(1));
            clk.tick().await;
        }
        while a.read().as_u8() & 4 == 0 {
            for j in 0u8..2 {
                q.write(Bits::from_u8(j + 20));
                clk.tick().await;
            }
            q.write(Bits::from_u8(2));
            clk.tick().await;
        }
        q.write(Bits::from_u8(3));
        clk.tick().await;
    }
}

/// The inner `for` inside an `if` of the outer body.
#[hardware(sequential)]
async fn ntf_if_inner(clk: Clock<MainClk>, a: In<Bits<4>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    loop {
        for i in 0u8..3 {
            if a.read().as_u8() & 1 == 1 {
                for j in 0u8..2 {
                    q.write(Bits::from_u8(j + 30));
                    clk.tick().await;
                }
            }
            q.write(Bits::from_u8(i));
            clk.tick().await;
        }
    }
}

/// The inner bound names the outer variable and the inner body reads the inner variable with
/// no type of its own: rustc types `j` by `i` (`u8`), and the transpiler, which typed it `i32`
/// until 2026-11-09, now does too (line `II.3 found for-bound-loop-var`, MEASUREMENTS §AD.3).
#[hardware(sequential)]
async fn ntf_bound_var(clk: Clock<MainClk>, a: In<Bits<4>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    loop {
        for i in 10u8..13 {
            for j in 8..i {
                q.write(Bits::from_u8(((j.wrapping_add(250) >> 1) as u8) ^ a.read().as_u8()));
                clk.tick().await;
            }
            q.write(Bits::from_u8(i));
            clk.tick().await;
        }
    }
}

/// An inner `for` followed by another tick-bearing `for` (the body ends in the second, whose
/// tick is hoisted as before), and an inner `for` followed by a wait loop and a tick.
#[hardware(sequential)]
async fn ntf_then_for_then_wait(clk: Clock<MainClk>, a: In<Bits<4>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    loop {
        for i in 0u8..2 {
            for j in 0u8..2 {
                q.write(Bits::from_u8(j + 10));
                clk.tick().await;
            }
            for k in 0u8..3 {
                q.write(Bits::from_u8(k + i + 20));
                clk.tick().await;
            }
        }
        for i in 0u8..2 {
            for j in 0u8..2 {
                q.write(Bits::from_u8(j + 40));
                clk.tick().await;
            }
            loop {
                if a.read().as_u8() & 1 == 1 {
                    break;
                }
                clk.tick().await;
            }
            q.write(Bits::from_u8(i + 50));
            clk.tick().await;
        }
    }
}
