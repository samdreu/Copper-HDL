// A register assigned inside a tick-free `for` of a clocked body (line `II.3 found
// register-in-tickfree-for`, MEASUREMENTS §X.3, §Y). SPEC I.2.4: a tick-free `for` over a
// constant range is DESUGARED — unrolled — so each iteration's assignment is a statement of
// the resumption and a later read sees it (I.3.4). Until 2026-11-08 the transpiler emitted an
// SV `for` whose body the segment's forwarding walk could not see: a register's writes in it
// were dropped (`f <= (f ^ a)` for `tfr_reg_accumulate`), on both routes and for a constant
// or a const-generic bound, and so were a wire accumulator's whose initializer read a register
// assigned earlier in the segment. The transpiler now gives every outer name a clocked `for`
// assigns a combinational temp the SV `for` accumulates by name, written back after the loop.
//
// The last two modules pin the second defect §Y found on the way (§Y.2): a `let mut` wire
// reassigned from itself in a clocked segment (no `for`) counted its earlier assignments twice.

const TFR_TRIPS: usize = 3;

#[derive(Clone, Copy)]
struct TfrPair {
    x: Bits<8>,
    y: Bits<8>,
}

/// The ledger line's probe: four increments of a register, then an input folded in.
#[hardware(sequential)]
async fn tfr_reg_accumulate(clk: Clock<MainClk>, a: In<Bits<8>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    let mut f: Bits<8> = Bits::zero();
    loop {
        q.write(f);
        for _ in 0..4 {
            f = f + Bits::from_lit::<1>();
        }
        f = f ^ a.read();
        clk.tick().await;
    }
}

/// The same over a const-generic range: an SV `for` the elaborator unrolls (`PARAMS`: N = 5).
#[hardware(sequential)]
async fn tfr_generic_bound<const N: usize>(
    clk: Clock<MainClk>,
    a: In<Bits<8>, MainClk>,
    q: RegOut<Bits<8>, MainClk>,
) {
    let mut f: Bits<8> = Bits::zero();
    loop {
        f = f ^ a.read();
        for _ in 0..N {
            f = f + Bits::from_lit::<3>();
        }
        q.write(f);
        clk.tick().await;
    }
}

/// A file-const bound, a `u8` register, a body-local `let` and a branch on the loop variable.
#[hardware(sequential)]
async fn tfr_const_bound_branch(
    clk: Clock<MainClk>,
    a: In<Bits<8>, MainClk>,
    k: In<Bits<2>, MainClk>,
    q: RegOut<Bits<8>, MainClk>,
) {
    let mut acc: u8 = 7;
    loop {
        for i in 0..TFR_TRIPS {
            let step: u8 = a.read().as_u8() ^ acc;
            if k.read().as_usize() == i {
                acc = acc.wrapping_add(step);
            } else {
                acc = acc ^ 0x5a;
            }
        }
        q.write(Bits::from_u8(acc));
        clk.tick().await;
    }
}

/// Nested tick-free `for`s writing one register, read by the drive after them.
#[hardware(sequential)]
async fn tfr_nested(
    clk: Clock<MainClk>,
    a: In<Bits<8>, MainClk>,
    k: In<Bits<2>, MainClk>,
    q: RegOut<Bits<8>, MainClk>,
) {
    let mut f: Bits<8> = Bits::from_lit::<1>();
    loop {
        for i in 0..2 {
            for j in 0..3 {
                if k.read().as_usize() == i + j {
                    f = f + a.read();
                }
            }
        }
        q.write(f);
        clk.tick().await;
    }
}

/// The extraction route: a tick in each arm, a register written in a `for` in each.
#[hardware(sequential)]
async fn tfr_extracted(
    clk: Clock<MainClk>,
    a: In<Bits<8>, MainClk>,
    s: In<Logic, MainClk>,
    q: RegOut<Bits<8>, MainClk>,
) {
    let mut f: Bits<8> = Bits::zero();
    loop {
        q.write(f);
        if s.read() == Logic::One {
            for _ in 0..3 {
                f = f + a.read();
            }
            clk.tick().await;
        } else {
            for _ in 0..2 {
                f = f ^ Bits::from_lit::<5>();
                f = f + Bits::from_lit::<1>();
            }
            clk.tick().await;
            f = f + a.read();
        }
    }
}

/// A wire accumulator whose initializer reads a register assigned earlier in the segment.
#[hardware(sequential)]
async fn tfr_wire_from_assigned(clk: Clock<MainClk>, a: In<Bits<8>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    let mut f: Bits<8> = Bits::zero();
    loop {
        f = f ^ a.read();
        let mut g: Bits<8> = f;
        for _ in 0..4 {
            g = g + Bits::from_lit::<1>();
        }
        q.write(g);
        clk.tick().await;
    }
}

/// §Y.2: a wire reassigned from itself, read in its segment (no `for`).
#[hardware(sequential)]
async fn tfr_wire_self_reassign(
    clk: Clock<MainClk>,
    a: In<Bits<8>, MainClk>,
    q: RegOut<Bits<8>, MainClk>,
    r: RegOut<Bits<8>, MainClk>,
) {
    let mut f: Bits<8> = Bits::zero();
    loop {
        let mut g: Bits<8> = a.read();
        g = g + Bits::from_lit::<1>();
        g = g + Bits::from_lit::<3>();
        f = f ^ g;
        q.write(g);
        r.write(f);
        clk.tick().await;
    }
}

/// §Y.2: the same wire read after the tick (a promoted wire), and an unrolled accumulation.
#[hardware(sequential)]
async fn tfr_wire_promoted(clk: Clock<MainClk>, a: In<Bits<8>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    loop {
        let mut g: Bits<8> = a.read();
        g = g + Bits::from_lit::<1>();
        for _ in 0..3 {
            g = g + g;
        }
        clk.tick().await;
        q.write(g);
    }
}

/// A wire bit-written before the loop and in it, then copied into a register: the temp must
/// start from the bit-written value (`shift_register`'s shape on a concrete width). It agrees on
/// `b159abb` too — it pins this change's own first draft, which forwarded `s` and lost `s[0]`
/// (MEASUREMENTS §Y.2).
#[hardware(sequential)]
async fn tfr_bit_write(clk: Clock<MainClk>, d: In<Logic, MainClk>, q: RegOut<Bits<4>, MainClk>) {
    let mut r: Bits<4> = Bits::zero();
    loop {
        let mut s: Bits<4> = Bits::zero();
        s[0] = d.read();
        for i in 1..4 {
            s[i] = r[i - 1];
        }
        r = s;
        q.write(r);
        clk.tick().await;
    }
}

/// A struct register assigned whole in the loop (its temp takes one wire per field). Each
/// field reads only itself: a later field reading an earlier one of the same literal is a
/// separate defect (MEASUREMENTS §Y.5).
#[hardware(sequential)]
async fn tfr_struct(clk: Clock<MainClk>, a: In<Bits<8>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    let mut p = TfrPair { x: Bits::zero(), y: Bits::from_lit::<9>() };
    loop {
        q.write(p.x ^ p.y);
        for _ in 0..3 {
            p = TfrPair { x: p.x + a.read(), y: p.y + Bits::from_lit::<1>() };
        }
        clk.tick().await;
    }
}
