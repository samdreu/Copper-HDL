// A value the forwarding walk substitutes for a register or wire name under a WIDENING cast in
// the same resumption (line `II.3 found forwarded-narrow-widening`, MEASUREMENTS §AD.4). The cast
// of a name was `32'(r)` (a name is self-sized), and `shir_lower::subst_vars` put `r`'s assigned
// value under it: `32'(8'(a) + 8'd250)`, which IEEE 1800 evaluates at the cast's 32 bits — 256
// where Rust's `u8` wrapped to 0. `sim=ref≠SV`, Verilator-clean (a narrower operand inside is
// lint-rejected instead). Until 2026-11-09; the substitute now keeps the name's width,
// `32'(8'(8'(a) + 8'd250))`. Each module widens a value that wraps at its own width.

/// A register assigned a wrapping sum and compared at 32 bits, two phases.
#[hardware(sequential)]
async fn fnw_reg_cmp(clk: Clock<MainClk>, a: In<Bits<4>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    let mut r: u8 = 0;
    loop {
        r = a.read().as_u8().wrapping_add(250);
        let big: u32 = r as u32;
        q.write(Bits::from_u8(if big > 100 { 1 } else { 0 }));
        clk.tick().await;
        q.write(Bits::from_u8(r));
        clk.tick().await;
    }
}

/// The same, single-tick, the widening in an `if` condition.
#[hardware(sequential)]
async fn fnw_reg_cmp1(clk: Clock<MainClk>, a: In<Bits<4>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    let mut r: u8 = 0;
    loop {
        q.write(Bits::from_u8(r));
        r = a.read().as_u8().wrapping_add(250);
        if (r as u32) > 100 {
            q.write(Bits::from_u8(1));
        }
        clk.tick().await;
    }
}

/// A signed widening (`as i32`) of a wrapping product, and of a wrapping difference.
#[hardware(sequential)]
async fn fnw_signed(clk: Clock<MainClk>, a: In<Bits<4>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    let mut r: u8 = 0;
    let mut s: i8 = 0;
    loop {
        r = a.read().as_u8().wrapping_mul(40);
        s = (a.read().as_u8() as i8).wrapping_sub(5);
        let x: i32 = (r as i32) - 100;
        let y: i32 = (s as i32) * 3;
        q.write(Bits::from_u8(((x > 0) as u8) | (((y < 0) as u8) << 1)));
        clk.tick().await;
    }
}

/// A wire merged by a `match` (a `Case` value) widened after it.
#[hardware(sequential)]
async fn fnw_match_value(clk: Clock<MainClk>, a: In<Bits<4>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    loop {
        let v: u8 = a.read().as_u8();
        let mut s: u8 = 0;
        match v & 1 {
            1 => { s = v.wrapping_add(250); }
            _ => {}
        }
        q.write(Bits::from_u8(if (s as u16) > 100 { 9 } else { s }));
        clk.tick().await;
    }
}

/// A tick-bearing `for` whose run-time bound is a wrapping `u8` sum (the counter compares at 32
/// bits), and one whose bound is a narrower sum Verilator's lint caught.
#[hardware(sequential)]
async fn fnw_for_bound_wrap(clk: Clock<MainClk>, a: In<Bits<4>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    loop {
        let n: u8 = a.read().as_u8().wrapping_add(250);
        for _ in 250u8..n {
            q.write(Bits::from_u8(7));
            clk.tick().await;
        }
        q.write(Bits::from_u8(n));
        clk.tick().await;
    }
}

#[hardware(sequential)]
async fn fnw_for_bound_narrow(clk: Clock<MainClk>, a: In<Bits<4>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    loop {
        let n: u8 = (a.read().as_u8() & 3) + 9;
        for j in 8..n {
            q.write(Bits::from_u8((j.wrapping_add(250) >> 1) as u8));
            clk.tick().await;
        }
        q.write(Bits::from_u8(n));
        clk.tick().await;
    }
}

/// Controls: a port-read bound and a bound expression in the header (no substitution under the
/// cast) — these agreed before.
#[hardware(sequential)]
async fn fnw_for_bound_port(clk: Clock<MainClk>, a: In<Bits<4>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    loop {
        let n: u8 = a.read().as_u8();
        for j in 0..n {
            q.write(Bits::from_u8(j));
            clk.tick().await;
        }
        q.write(Bits::from_u8(n + 100));
        clk.tick().await;
    }
}

#[hardware(sequential)]
async fn fnw_for_bound_header(clk: Clock<MainClk>, a: In<Bits<4>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    loop {
        for j in 0..(a.read().as_u8() & 3) {
            q.write(Bits::from_u8(j));
            clk.tick().await;
        }
        q.write(Bits::from_u8(100));
        clk.tick().await;
    }
}
