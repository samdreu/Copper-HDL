// A `let` that shadows a binding of an OUTER scope ends at its block's end; after it the outer
// binding is visible again, unchanged (Rust's block scoping; SPEC I.14; lines `II.3 found
// for-var-shadow-nested` and `II.3 found scoped-shadow`, MEASUREMENTS §AC). Until 2026-11-09 the
// transpiler lowered every binding of one name onto one SV signal, so an inner shadow overwrote
// the outer binding: a `for`'s loop variable (tick-free, the SV `for`'s own variable,
// lint-rejected; tick-bearing, the counter, which changed the trip count — `sim=ref≠SV`,
// Verilator-clean when the widths matched), an ordinary local (`sim=ref≠SV`), a register (a
// duplicate SV declaration). `desugar::rename_scoped_shadows` now gives the shadow a name of its
// own for its scope. Each module reads the shadowed binding after the shadow's block.

// ── A `for`'s variable shadowed in a nested block: tick-free, combinational ──

#[hardware(combinational)]
fn ssh_comb_if<D: ClockDomain>(a: In<Bits<8>, D>, o: Out<Bits<8>, D>) {
    let mut s: u8 = a.read().as_u8();
    for i in 0u8..4 {
        if a.read().as_u8() & 1 == 1 {
            let i = i + 1;
            s = s ^ i;
        }
        s = s.wrapping_add(i);
    }
    o.write(Bits::from_u8(s));
}

#[hardware(combinational)]
fn ssh_comb_match<D: ClockDomain>(a: In<Bits<8>, D>, o: Out<Bits<8>, D>) {
    let mut s: u8 = a.read().as_u8();
    for i in 0u8..4 {
        match a.read().as_u8() & 3 {
            0 => {
                let i = i * 2;
                s = s.wrapping_add(i);
            }
            _ => {}
        }
        s = s ^ i;
    }
    o.write(Bits::from_u8(s));
}

/// A bare block, a shadow of the shadow two blocks in, and a same-block re-shadow.
#[hardware(combinational)]
fn ssh_comb_block<D: ClockDomain>(a: In<Bits<8>, D>, o: Out<Bits<8>, D>) {
    let mut s: u8 = a.read().as_u8();
    for i in 0u8..4 {
        {
            let i = i ^ 5;
            s = s.wrapping_add(i);
            {
                let i = i.wrapping_mul(3);
                let i = i + 1;
                s = s ^ i;
            }
            s = s.wrapping_add(i);
        }
        s = s ^ i;
    }
    o.write(Bits::from_u8(s));
}

// ── tick-free, clocked, linear route (two top-level ticks) ──

#[hardware(sequential)]
async fn ssh_lin_if(clk: Clock<MainClk>, a: In<Bits<4>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    loop {
        let mut s: u8 = a.read().as_u8();
        for i in 0u8..4 {
            if a.read().as_u8() & 1 == 1 {
                let i = i + 1;
                s = s ^ i;
            } else {
                let i = i + 2;
                s = s.wrapping_add(i);
            }
            s = s.wrapping_add(i);
        }
        q.write(Bits::from_u8(s));
        clk.tick().await;
        q.write(Bits::from_u8(a.read().as_u8()));
        clk.tick().await;
    }
}

#[hardware(sequential)]
async fn ssh_lin_match(clk: Clock<MainClk>, a: In<Bits<4>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    loop {
        let mut s: u8 = a.read().as_u8();
        for i in 0u8..4 {
            match a.read().as_u8() & 3 {
                0 => {
                    let i = i * 2;
                    s = s.wrapping_add(i);
                }
                _ => {}
            }
            {
                let i = i ^ 5;
                s = s.wrapping_add(i);
            }
            s = s ^ i;
        }
        q.write(Bits::from_u8(s));
        clk.tick().await;
        q.write(Bits::from_u8(a.read().as_u8()));
        clk.tick().await;
    }
}

// ── tick-free, clocked, extraction route (a branch-nested tick) ──

#[hardware(sequential)]
async fn ssh_ext_if(clk: Clock<MainClk>, a: In<Bits<4>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    loop {
        let mut s: u8 = a.read().as_u8();
        for i in 0u8..4 {
            if a.read().as_u8() & 1 == 1 {
                let i = i + 1;
                s = s ^ i;
            }
            s = s.wrapping_add(i);
        }
        q.write(Bits::from_u8(s));
        if a.read().as_u8() & 2 == 2 {
            clk.tick().await;
        }
        clk.tick().await;
    }
}

#[hardware(sequential)]
async fn ssh_ext_match(clk: Clock<MainClk>, a: In<Bits<4>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    loop {
        let mut s: u8 = a.read().as_u8();
        for i in 0u8..4 {
            match a.read().as_u8() & 3 {
                0 => {
                    let i = i * 2;
                    s = s.wrapping_add(i);
                }
                _ => {}
            }
            {
                let i = i ^ 5;
                s = s.wrapping_add(i);
            }
            s = s ^ i;
        }
        q.write(Bits::from_u8(s));
        if a.read().as_u8() & 4 == 4 {
            clk.tick().await;
        }
        clk.tick().await;
    }
}

// ── tick-bearing `for` (the extraction route): the shadow is not the counter ──

#[hardware(sequential)]
async fn ssh_tick_if(clk: Clock<MainClk>, a: In<Bits<4>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    let mut t: u8 = 0;
    loop {
        for i in 0u8..4 {
            if a.read().as_u8() & 1 == 1 {
                let i = i + 1;
                t = t.wrapping_add(i);
            }
            q.write(Bits::from_u8(t ^ (i << 4)));
            clk.tick().await;
        }
        t = t.wrapping_add(1);
    }
}

#[hardware(sequential)]
async fn ssh_tick_match(clk: Clock<MainClk>, a: In<Bits<4>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    let mut t: u8 = 0;
    loop {
        for i in 0u8..4 {
            match a.read().as_u8() & 3 {
                0 => {
                    let i = i + 2;
                    t = t.wrapping_add(i);
                }
                _ => {}
            }
            {
                let i = i + 1;
                t = t ^ i;
            }
            q.write(Bits::from_u8(t ^ (i << 4)));
            clk.tick().await;
        }
        t = t.wrapping_add(1);
    }
}

/// A `u32` range: the shadow has the counter's width, so before the fix no lint warning flagged
/// it — the silent case (`sim=ref≠SV` on 20 of 20 sequences).
#[hardware(sequential)]
async fn ssh_tick32_if(clk: Clock<MainClk>, a: In<Bits<4>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    let mut t: u32 = 0;
    loop {
        for i in 0u32..4 {
            if a.read().as_u8() & 1 == 1 {
                let i = i + 1;
                t = t.wrapping_add(i);
            }
            q.write(Bits::from_u8((t ^ (i << 4)) as u8));
            clk.tick().await;
        }
        t = t.wrapping_add(1);
    }
}

#[hardware(sequential)]
async fn ssh_tick32_block(clk: Clock<MainClk>, a: In<Bits<4>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    let mut t: u32 = 3;
    loop {
        for i in 0u32..4 {
            {
                let i = i + 1;
                t = t.wrapping_add(i);
            }
            q.write(Bits::from_u8((t ^ (i << 4)) as u8 ^ a.read().as_u8()));
            clk.tick().await;
        }
        t = t.wrapping_add(1);
    }
}

/// The shadow live across the tick of its tick-bearing `for`: a register of its own.
#[hardware(sequential)]
async fn ssh_tick_live(clk: Clock<MainClk>, a: In<Bits<4>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    loop {
        for i in 0u8..3 {
            if a.read().as_u8() & 1 == 1 {
                let i = i.wrapping_add(a.read().as_u8());
                clk.tick().await;
                q.write(Bits::from_u8(i));
            }
            q.write(Bits::from_u8(i << 4));
            clk.tick().await;
        }
    }
}

// ── beyond a `for`'s own variable ──

/// An ordinary local, and a tuple `let` shadowing two.
#[hardware(combinational)]
fn ssh_comb_local<D: ClockDomain>(a: In<Bits<8>, D>, o: Out<Bits<8>, D>) {
    let x: u8 = a.read().as_u8();
    let y: u8 = x ^ 0x5a;
    let mut s: u8 = 0;
    if x & 1 == 1 {
        let x = x.wrapping_add(7);
        s = x;
    }
    if x & 2 == 2 {
        let (x, y) = (y, x);
        s = s.wrapping_add(x ^ (y << 1));
    }
    o.write(Bits::from_u8(s ^ x ^ y));
}

/// A `match` arm's binder that hides an outer local of its name.
#[hardware(combinational)]
fn ssh_arm_binder<D: ClockDomain>(a: In<Bits<8>, D>, o: Out<Bits<8>, D>) {
    let x: u8 = a.read().as_u8();
    let mut s: u8 = x;
    if x & 1 == 1 {
        let x = x.wrapping_mul(5);
        s = match x & 3 {
            0 => s,
            x => s.wrapping_add(x),
        };
        s = s ^ x;
    }
    o.write(Bits::from_u8(s ^ x));
}

/// A register shadowed in a nested block and at the top of the loop body.
#[hardware(sequential)]
async fn ssh_reg(clk: Clock<MainClk>, a: In<Bits<4>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    let mut r: u8 = 3;
    loop {
        let mut s: u8 = 0;
        if a.read().as_u8() & 1 == 1 {
            let r = r.wrapping_add(5);
            s = r;
        }
        q.write(Bits::from_u8(s ^ r));
        r = r.wrapping_add(1);
        clk.tick().await;
    }
}

#[hardware(sequential)]
async fn ssh_reg_top(clk: Clock<MainClk>, a: In<Bits<4>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    let mut r: u8 = 3;
    loop {
        let r = r.wrapping_add(a.read().as_u8());
        q.write(Bits::from_u8(r));
        clk.tick().await;
    }
}

/// A port-read local shadowed in a branch.
#[hardware(sequential)]
async fn ssh_port_local(clk: Clock<MainClk>, a: In<Bits<4>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    loop {
        let v: u8 = a.read().as_u8();
        let mut s: u8 = 0;
        if v & 3 == 1 {
            let v = v.wrapping_mul(3);
            s = v;
        }
        q.write(Bits::from_u8(s ^ (v << 4)));
        clk.tick().await;
    }
}

/// An outer loop's variable shadowed inside an inner loop's branch, and an inner `for`
/// rebinding the outer one's variable.
#[hardware(combinational)]
fn ssh_outer_var<D: ClockDomain>(a: In<Bits<8>, D>, o: Out<Bits<8>, D>) {
    let mut s: u8 = a.read().as_u8();
    for i in 0u8..3 {
        for j in 0u8..2 {
            if s & 1 == 1 {
                let i = i + j;
                s = s.wrapping_add(i);
            }
            s = s ^ i;
        }
        for i in 4u8..6 {
            s = s.wrapping_add(i);
        }
        s = s ^ (i << 2);
    }
    o.write(Bits::from_u8(s));
}

/// A tick-free inner `for` rebinding a tick-bearing outer one's variable (the counter).
#[hardware(sequential)]
async fn ssh_inner_rebinds_counter(clk: Clock<MainClk>, a: In<Bits<4>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    loop {
        for i in 0u8..3 {
            let mut s: u8 = a.read().as_u8();
            for i in 4u8..6 {
                s = s.wrapping_add(i);
            }
            q.write(Bits::from_u8(s ^ (i << 4)));
            clk.tick().await;
        }
    }
}

/// A tick-bearing `for` whose variable shadows an outer local: the renamed counter is a
/// register, and the local is read again after the loop.
#[hardware(sequential)]
async fn ssh_for_shadows_local(clk: Clock<MainClk>, a: In<Bits<4>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    loop {
        let i: u8 = a.read().as_u8();
        for i in 0u8..3 {
            q.write(Bits::from_u8(i ^ 0x80));
            clk.tick().await;
        }
        q.write(Bits::from_u8(i));
        clk.tick().await;
    }
}

/// A local shadowed in a branch, live across a tick (the extraction route).
#[hardware(sequential)]
async fn ssh_across_tick(clk: Clock<MainClk>, a: In<Bits<4>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    loop {
        let x: u8 = a.read().as_u8();
        if x & 1 == 1 {
            let x = x.wrapping_add(9);
            clk.tick().await;
            q.write(Bits::from_u8(x));
        }
        q.write(Bits::from_u8(x << 4));
        clk.tick().await;
    }
}

/// A tick-free `for`'s `continue` whose rest names a local shadowed inside the branch: the rest,
/// copied into the branch by the `continue` rewrite, reads the OUTER `t` (until §AC the rewrite
/// declined it, since the copy would have captured the shadow).
#[hardware(combinational)]
fn ssh_continue_rest<D: ClockDomain>(a: In<Bits<4>, D>, o: Out<Bits<4>, D>) {
    let av = a.read();
    let t: Bits<4> = Bits::from_lit::<1>();
    let mut acc: Bits<4> = Bits::zero();
    for i in 0..4 {
        if av[i] == Logic::One {
            let t: Bits<4> = Bits::from_lit::<2>();
            acc = acc ^ t;
            if acc[0] == Logic::One { continue; }
        }
        acc = acc + t;
    }
    o.write(acc);
}
