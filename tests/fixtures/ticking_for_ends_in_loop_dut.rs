// A nested tick-bearing loop whose body has no bare tick and ends in a skippable wait, R3 holding
// through an earlier inner `for` over a range non-empty by its constants (line `II.3 found
// ticking-for-ends-in-loop`, D-29, MEASUREMENTS §AD.5, §AE). The macro admitted every module here
// (I.8.2(b): the body ends in a loop that ends at a tick; R3 knows the inner `for` always ticks);
// until 2026-11-09 the transpiler refused them: `expand_counted_for` had no tick to hoist from a body
// ending in a loop, and the inner `for`'s exit test at entry — never taken over a constant non-empty
// range — gave `splice_zero_time_gotos` a structural zero-time cycle. D-29: such a `for` takes I.8.5's
// `D` with no hoisted tick (the exit test after the body when the range is non-empty by `ConstEnv`),
// and the unhoisted form drops its dead entry test. Every module is on the extraction route (a nested
// tick-bearing loop is a branch-nested tick).

const TFL_N: u8 = 3;
const TFL_K: u8 = 2;
const TFL_LO: u32 = 7;
const TFL_HI: u32 = TFL_LO + 2;

/// §AD.5's probe `zzh_05`: the outer `for`'s body ends in the wait; the inner `for` ticks on every path.
#[hardware(sequential)]
async fn tfl_outer_ends_wait(clk: Clock<MainClk>, a: In<Bits<4>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    loop {
        for i in 0u8..2 {
            for j in 0u8..2 {
                q.write(Bits::from_u8(j ^ (a.read().as_u8() & 2)));
                clk.tick().await;
            }
            q.write(Bits::from_u8(i + 16));
            loop {
                if a.read().as_u8() & 1 == 1 {
                    break;
                }
                clk.tick().await;
            }
        }
    }
}

/// §AD.5's hand-written twin `h1`: a nested `loop` with its own counter, no bare tick, ending in a wait.
#[hardware(sequential)]
async fn tfl_handwritten(clk: Clock<MainClk>, a: In<Bits<4>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    loop {
        let mut c: u32 = 0;
        loop {
            if c >= 2 {
                break;
            }
            c = c + 1;
            for j in 0u8..2 {
                q.write(Bits::from_u8(j + 4));
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
        clk.tick().await;
    }
}

/// The outer range through a file `const` (`ConstEnv` folds it), the inner through a body-local one.
#[hardware(sequential)]
async fn tfl_const_items(clk: Clock<MainClk>, a: In<Bits<4>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    loop {
        const K: u8 = 2;
        for i in 0..TFL_N {
            for j in 0..K {
                q.write(Bits::from_u8(j + 32));
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

/// A hand-written `loop` whose inner `for` is bounded by a file `const`; a `while` of the same shape.
#[hardware(sequential)]
async fn tfl_loop_while_const(clk: Clock<MainClk>, a: In<Bits<4>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    let mut c: u8 = 0;
    loop {
        loop {
            if c >= 3 {
                break;
            }
            c = c + 1;
            for j in 0..TFL_K {
                q.write(Bits::from_u8(j + c));
                clk.tick().await;
            }
            loop {
                if a.read().as_u8() & 1 == 1 {
                    break;
                }
                clk.tick().await;
            }
        }
        c = 0;
        while c < 2 {
            c = c + 1;
            for j in 0u8..2 {
                q.write(Bits::from_u8(j + c + 64));
                clk.tick().await;
            }
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

/// An outer range known only at run time (zero iterations possible), followed by a tick: `D` with its exit test at the top.
#[hardware(sequential)]
async fn tfl_outer_runtime(clk: Clock<MainClk>, a: In<Bits<4>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    loop {
        let n: u8 = a.read().as_u8() & 3;
        for i in 0..n {
            for _ in 0u8..2 {
                clk.tick().await;
            }
            q.write(Bits::from_u8(i + 48));
            loop {
                if a.read().as_u8() & 1 == 1 {
                    break;
                }
                clk.tick().await;
            }
        }
        q.write(Bits::from_u8(77));
        clk.tick().await;
    }
}

/// An author `break` before the inner `for`; inclusive and arithmetic-literal inner ranges; two inner `for`s.
#[hardware(sequential)]
async fn tfl_break_inclusive_two(clk: Clock<MainClk>, a: In<Bits<4>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    loop {
        for i in (1u8 + 1)..4 {
            if a.read().as_u8() == 9 {
                break;
            }
            for j in 0u8..=1 {
                q.write(Bits::from_u8(j + 2));
                clk.tick().await;
            }
            for k in (3u8 - 2)..(1 + 2) {
                q.write(Bits::from_u8(k + i * 8));
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
        q.write(Bits::from_u8(66));
        clk.tick().await;
    }
}

/// Three levels: the middle `for` ends in the wait; the inner `for` sits in one arm, a tick in the other.
#[hardware(sequential)]
async fn tfl_three_arms(clk: Clock<MainClk>, a: In<Bits<4>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    loop {
        for i in 0u8..2 {
            for k in 0u8..2 {
                if a.read().as_u8() & 4 == 4 {
                    for _ in 0u8..2 {
                        clk.tick().await;
                    }
                } else {
                    clk.tick().await;
                }
                q.write(Bits::from_u8(k + i * 4));
                loop {
                if a.read().as_u8() & 1 == 1 {
                    break;
                }
                clk.tick().await;
            }
            }
            q.write(Bits::from_u8(i + 8));
            clk.tick().await;
        }
    }
}

/// A wait, the inner `for`, a wait; the loop variable read after the first wait; `u32` const bounds; an end at `u8::MAX`.
#[hardware(sequential)]
async fn tfl_waits_wide_max(clk: Clock<MainClk>, a: In<Bits<4>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    loop {
        for i in 250u8..u8::MAX {
            loop {
                if a.read().as_u8() & 1 == 1 {
                    break;
                }
                clk.tick().await;
            }
            for j in TFL_LO..TFL_HI {
                q.write(Bits::from_u8(j as u8));
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
        clk.tick().await;
    }
}
