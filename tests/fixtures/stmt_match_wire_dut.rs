// A clocked body's wire local (a loop-body `let mut s`) assigned inside a statement `match`'s
// arm and read after the `match` in the same resumption (line `II.3 found stmt-match-wire`,
// MEASUREMENTS §AC.6, §AD). Until 2026-11-09 the transpiler's forwarding walk
// (`shir_lower::extract_updates_from_stmts`) merged a `match`'s REGISTER updates per arm but
// dropped each arm's WIRE values, so every edge consumer after the `match` read the wire's value
// from before it (`q <= 8'd0` for `s = 0; match .. { 1 => s = 5, _ => {} } q.write(s)`):
// `sim=ref≠SV`, Verilator-clean, on every clocked route. An `if` merged both. Each module here
// reads the wire after the statement that assigns it.

// ── The plain `match`, single-tick (linear route) ──

/// An arm assigns, the default arm does not.
#[hardware(sequential)]
async fn smw_lin(clk: Clock<MainClk>, a: In<Bits<4>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    loop {
        let v: u8 = a.read().as_u8();
        let mut s: u8 = 0;
        match v & 3 {
            1 => { s = v.wrapping_mul(3); }
            _ => {}
        }
        q.write(Bits::from_u8(s ^ (v << 4)));
        clk.tick().await;
    }
}

/// Every arm assigns, one with an arm-local `let`, one an expression arm; the scrutinee a port read.
#[hardware(sequential)]
async fn smw_all_arms(clk: Clock<MainClk>, a: In<Bits<4>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    loop {
        let mut s: u8 = 0;
        match a.read().as_u8() & 3 {
            0 => { let w = a.read().as_u8() + 1; s = w; }
            1 => s = 9,
            2 | 3 => { s = s + 2; s = s + 3; }
            _ => {}
        }
        q.write(Bits::from_u8(s));
        clk.tick().await;
    }
}

/// An arm that does not assign the wire (only another arm does), and a second wire only the
/// default arm assigns.
#[hardware(sequential)]
async fn smw_one_arm(clk: Clock<MainClk>, a: In<Bits<4>, MainClk>, q: RegOut<Bits<8>, MainClk>, r: RegOut<Bits<8>, MainClk>) {
    loop {
        let v: u8 = a.read().as_u8();
        let mut s: u8 = 1;
        let mut t: u8 = 2;
        match v >> 2 {
            0 => {}
            1 => { s = v; }
            _ => { t = v + 7; }
        }
        q.write(Bits::from_u8(s));
        r.write(Bits::from_u8(t));
        clk.tick().await;
    }
}

/// A guarded arm and a binding arm (`desugar` lowers this `match` into an `if` chain).
#[hardware(sequential)]
async fn smw_guard_bind(clk: Clock<MainClk>, a: In<Bits<4>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    loop {
        let v: u8 = a.read().as_u8();
        let mut s: u8 = 3;
        match v {
            x if x > 12 => { s = x + 1; }
            4 => {}
            n => { s = n ^ s; }
        }
        q.write(Bits::from_u8(s));
        clk.tick().await;
    }
}

/// A register update after the `match` reads the wire.
#[hardware(sequential)]
async fn smw_reg_reads(clk: Clock<MainClk>, a: In<Bits<4>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    let mut acc: u8 = 0;
    loop {
        let mut s: u8 = 1;
        match a.read().as_u8() & 1 {
            1 => { s = 4; }
            _ => {}
        }
        acc = acc.wrapping_add(s);
        q.write(Bits::from_u8(acc));
        clk.tick().await;
    }
}

// ── Nested statement forms ──

/// A `match` inside an `if`; an `if` inside a `match` arm; a `match` inside a `match` arm.
#[hardware(sequential)]
async fn smw_nested(clk: Clock<MainClk>, a: In<Bits<4>, MainClk>, q: RegOut<Bits<8>, MainClk>, r: RegOut<Bits<8>, MainClk>) {
    loop {
        let v: u8 = a.read().as_u8();
        let mut s: u8 = 0;
        let mut t: u8 = 0;
        if v & 8 == 8 {
            match v & 1 {
                1 => { s = 6; }
                _ => {}
            }
        }
        match v & 2 {
            2 => {
                if v & 4 == 4 { t = 1; } else { t = 2; }
                match v & 1 {
                    0 => { s = s + 10; }
                    _ => {}
                }
            }
            _ => {}
        }
        q.write(Bits::from_u8(s));
        r.write(Bits::from_u8(t));
        clk.tick().await;
    }
}

/// A `match` inside a tick-free `for`, and a bare block assigning the wire.
#[hardware(sequential)]
async fn smw_in_for(clk: Clock<MainClk>, a: In<Bits<4>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    loop {
        let v: u8 = a.read().as_u8();
        let mut s: u8 = 0;
        for i in 0u8..3 {
            match (v >> i) & 1 {
                1 => { s = s + i + 1; }
                _ => {}
            }
        }
        {
            s = s ^ 0x40;
        }
        q.write(Bits::from_u8(s));
        clk.tick().await;
    }
}

/// The wire's initializer reads a register assigned earlier in the segment (a recorded wire),
/// then a `match` arm reassigns it.
#[hardware(sequential)]
async fn smw_recorded(clk: Clock<MainClk>, a: In<Bits<4>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    let mut c: u8 = 0;
    loop {
        c = c.wrapping_add(1);
        let mut s: u8 = c;
        match a.read().as_u8() & 1 {
            1 => { s = s.wrapping_add(100); }
            _ => {}
        }
        q.write(Bits::from_u8(s));
        clk.tick().await;
    }
}

// ── Two-phase linear and extraction routes ──

#[hardware(sequential)]
async fn smw_lin2(clk: Clock<MainClk>, a: In<Bits<4>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    loop {
        let mut s: u8 = 0;
        match a.read().as_u8() & 3 {
            1 => { s = 5; }
            2 => {}
            _ => { s = 7; }
        }
        q.write(Bits::from_u8(s));
        clk.tick().await;
        let mut t: u8 = 1;
        match a.read().as_u8() >> 2 {
            0 => { t = 11; }
            _ => {}
        }
        q.write(Bits::from_u8(t));
        clk.tick().await;
    }
}

#[hardware(sequential)]
async fn smw_ext(clk: Clock<MainClk>, a: In<Bits<4>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    loop {
        let mut s: u8 = 0;
        match a.read().as_u8() & 3 {
            1 => { s = 5; }
            _ => {}
        }
        q.write(Bits::from_u8(s));
        if a.read().as_u8() & 4 == 4 {
            let mut u: u8 = 2;
            match a.read().as_u8() & 1 {
                0 => { u = 13; }
                _ => {}
            }
            q.write(Bits::from_u8(u));
            clk.tick().await;
        }
        clk.tick().await;
    }
}

/// The trailing region (after the last tick) holds the `match`.
#[hardware(sequential)]
async fn smw_trailing(clk: Clock<MainClk>, a: In<Bits<4>, MainClk>, q: RegOut<Bits<8>, MainClk>, r: RegOut<Bits<8>, MainClk>) {
    loop {
        q.write(Bits::from_u8(a.read().as_u8()));
        clk.tick().await;
        let mut s: u8 = 0;
        match a.read().as_u8() & 1 {
            1 => { s = 3; }
            _ => {}
        }
        r.write(Bits::from_u8(s + 20));
    }
}

// ── Consumers inside the arm, and a condition after the `match` ──

/// A port write inside the arm reads the arm's assignment; inside another arm an `if` after a
/// nested `match` reads the nested `match`'s assignment.
#[hardware(sequential)]
async fn smw_write_in_arm(clk: Clock<MainClk>, a: In<Bits<4>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    loop {
        let v: u8 = a.read().as_u8();
        let mut s: u8 = 0;
        match v & 1 {
            1 => { s = v + 1; q.write(Bits::from_u8(s)); }
            _ => {
                match v & 2 {
                    2 => { s = 50; }
                    _ => {}
                }
                if s == 50 { q.write(Bits::from_u8(s + 1)); }
            }
        }
        clk.tick().await;
    }
}

/// An `if` after the `match` tests the wire and updates a register; a `Bits` wire gets a bit
/// write in an arm.
#[hardware(sequential)]
async fn smw_cond_after(clk: Clock<MainClk>, a: In<Bits<4>, MainClk>, q: RegOut<Bits<8>, MainClk>, r: RegOut<Bits<4>, MainClk>) {
    let mut n: u8 = 0;
    loop {
        let v: u8 = a.read().as_u8();
        let mut s: u8 = 0;
        let mut b: Bits<4> = Bits::zero();
        match v & 3 {
            3 => { s = 1; b[2] = Logic::One; }
            _ => {}
        }
        if s == 1 { n = n.wrapping_add(1); }
        q.write(Bits::from_u8(n));
        r.write(b);
        clk.tick().await;
    }
}
