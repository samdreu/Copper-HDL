// SPEC I.14 (D-19) probes, swept since Phase C step 8 (2026-11-07,
// `design_docs/review/MEASUREMENTS.md` §M) — I.14.7 (1)-(10), each probe the fixture of
// its obligation. The D-19 measurements (session spec-packet-i14): `w1_index`/`w1_comb`
// transpiled and Verilator refused them ("Illegal bit or array select", width 1);
// `w1_from`, `xp_div`, `xw_port` and the `k_*` six were refused by the transpiler;
// `usize_width`/`usize_annot` were ADMITTED-AND-DIVERGENT (`usize` 32 bits, the `as_*`
// views passthroughs at the receiver's width); `c_narrow`/`c_narrow2` failed -Wall.
// The probes are verbatim; the twins after them exercise the lowerings step 8 added.
// Integer-port probes (`int_wrapping`, `int_overflow`) are not here: the sweep's
// generator stimulates `Logic`/`Bits` ports only — they are pinned on the reference
// interpreter in `copper-ref/tests/interp_data.rs`.

/// (iii) A `Bits<1>` port indexed `[0]` (I.11.7's width-1 defect, sequential spelling).
#[hardware(sequential)]
async fn w1_index(clk: Clock<MainClk>, a: In<Bits<1>, MainClk>, q: RegOut<Logic, MainClk>) {
    loop {
        let v = a.read();
        q.write(v[0]);
        clk.tick().await;
    }
}
/// (iii) control: a `Logic` port and a `Bits<1>` port as one-bit values, `From` both ways.
#[hardware(sequential)]
async fn w1_from(
    clk: Clock<MainClk>,
    a: In<Bits<1>, MainClk>,
    l: In<Logic, MainClk>,
    q: RegOut<Logic, MainClk>,
    qb: RegOut<Bits<1>, MainClk>,
) {
    loop {
        let v = a.read();
        let s = l.read();
        q.write(Logic::from(v));
        qb.write(Bits::from(s));
        clk.tick().await;
    }
}
/// (iii) the combinational spelling of I.11.7's draft.
#[hardware(combinational)]
fn w1_comb(a: In<Bits<1>, MainClk>, q: Out<Logic, MainClk>) {
    q.write(a.read()[0]);
}
/// (iv) a hex literal in a `from_lit` turbofish, in the body.
#[hardware(sequential)]
async fn k_hex_body(clk: Clock<MainClk>, a: In<Bits<8>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    loop {
        let x = a.read();
        q.write(x ^ Bits::<8>::from_lit::<0xA5>());
        clk.tick().await;
    }
}
/// (iv) a hex literal as a register initializer.
#[hardware(sequential)]
async fn k_hex_init(clk: Clock<MainClk>, a: In<Bits<8>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    let mut r = Bits::<8>::from_lit::<0xA5>();
    loop {
        q.write(r);
        r = a.read();
        clk.tick().await;
    }
}
/// (iv) an underscore-separated decimal literal.
#[hardware(sequential)]
async fn k_underscore(clk: Clock<MainClk>, a: In<Bits<16>, MainClk>, q: RegOut<Bits<16>, MainClk>) {
    loop {
        let x = a.read();
        q.write(x + Bits::<16>::from_lit::<1_000>());
        clk.tick().await;
    }
}
/// (iv) `all_ones()` without a width (rustc infers it from the port).
#[hardware(sequential)]
async fn k_ones_nowidth(clk: Clock<MainClk>, a: In<Bits<8>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    loop {
        let x = a.read();
        q.write(x ^ Bits::all_ones());
        clk.tick().await;
    }
}
/// (iv) `all_ones()` with a width.
#[hardware(sequential)]
async fn k_ones_width(clk: Clock<MainClk>, a: In<Bits<8>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    loop {
        let x = a.read();
        q.write(x ^ Bits::<8>::all_ones());
        clk.tick().await;
    }
}
/// (iv) `all_ones()` as an initializer.
#[hardware(sequential)]
async fn k_ones_init(clk: Clock<MainClk>, a: In<Bits<8>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    let mut r = Bits::<8>::all_ones();
    loop {
        q.write(r);
        r = a.read();
        clk.tick().await;
    }
}
/// (v) narrowing casts on Rust integers, through the passthrough `as_u32`.
#[hardware(sequential)]
async fn c_narrow(clk: Clock<MainClk>, a: In<Bits<16>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    loop {
        let v = a.read().as_u32();
        let n = v as u8;
        q.write(Bits::<8>::from_u8(n));
        clk.tick().await;
    }
}
/// (v) narrowing through a passthrough `as_u32` that reads the high byte too.
#[hardware(sequential)]
async fn c_narrow2(clk: Clock<MainClk>, a: In<Bits<16>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    loop {
        let v = a.read().as_u32();
        let lo = v as u8;
        let hi = (v >> 8) as u8;
        q.write(Bits::<8>::from_u8(lo ^ hi));
        clk.tick().await;
    }
}
/// (v) a signed reinterpretation and back.
#[hardware(sequential)]
async fn c_signed(clk: Clock<MainClk>, a: In<Bits<8>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    loop {
        let v = a.read().as_u8();
        let s = (v as i8) >> 2;
        q.write(Bits::<8>::from_u8(s as u8));
        clk.tick().await;
    }
}
/// `usize` arithmetic past 2^32: the transpiler resolves `usize` as 32 bits.
#[hardware(sequential)]
async fn usize_width(clk: Clock<MainClk>, a: In<Bits<8>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    loop {
        let v = a.read().as_usize();
        let big = v << 30;
        let back = big >> 30;
        q.write(Bits::<8>::from_usize(back & 0xff));
        clk.tick().await;
    }
}
/// `usize` annotated: the transpiler resolves `usize` as 32 bits, the host as 64.
#[hardware(sequential)]
async fn usize_annot(clk: Clock<MainClk>, a: In<Bits<8>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    loop {
        let v: usize = a.read().as_usize();
        let big: usize = v << 40;
        let back: usize = big >> 40;
        q.write(Bits::<8>::from_usize(back & 0xff));
        clk.tick().await;
    }
}
#[hardware(sequential)]
async fn xp_div(
    clk: Clock<MainClk>,
    a: In<Bits<8>, MainClk>,
    b: In<Bits<8>, MainClk>,
    q: RegOut<Bits<8>, MainClk>,
) {
    loop {
        let x = a.read();
        let y = b.read();
        q.write(x / y);
        clk.tick().await;
    }
}
/// A write of `Bits::x()` / `Logic::X` to a RegOut.
#[hardware(sequential)]
async fn xw_port(
    clk: Clock<MainClk>,
    a: In<Bits<8>, MainClk>,
    c: In<Logic, MainClk>,
    q: RegOut<Bits<8>, MainClk>,
    ql: RegOut<Logic, MainClk>,
) {
    loop {
        let x = a.read();
        if c.read() == Logic::One {
            q.write(Bits::x());
        } else {
            q.write(x);
        }
        ql.write(Logic::X);
        clk.tick().await;
    }
}
/// `/` with a divisor that is never zero, so every SV pair stays in the relation.
#[hardware(sequential)]
async fn xp_div_nz(
    clk: Clock<MainClk>,
    a: In<Bits<8>, MainClk>,
    b: In<Bits<8>, MainClk>,
    q: RegOut<Bits<8>, MainClk>,
    r: RegOut<Bits<8>, MainClk>,
) {
    loop {
        let x = a.read();
        let y = b.read() | Bits::from_u8(1);
        q.write(x / y);
        r.write(x % y);
        clk.tick().await;
    }
}

/// I.14.7 (7): `saturating_*` on the integer views (I.14.3(c), IN), `Logic::from_bool`,
/// an element write through `as_array_mut`, `Bits::from(Logic)`.
#[hardware(sequential)]
async fn data_methods_seq(
    clk: Clock<MainClk>,
    a: In<Bits<8>, MainClk>,
    b: In<Bits<8>, MainClk>,
    l: In<Logic, MainClk>,
    sat: RegOut<Bits<8>, MainClk>,
    sub: RegOut<Bits<8>, MainClk>,
    mul: RegOut<Bits<8>, MainClk>,
    eq: RegOut<Logic, MainClk>,
    put: RegOut<Bits<8>, MainClk>,
    one: RegOut<Bits<1>, MainClk>,
) {
    loop {
        let x = a.read().as_u8();
        let y = b.read().as_u8();
        sat.write(Bits::from_u8(x.saturating_add(y)));
        sub.write(Bits::from_u8(x.saturating_sub(y)));
        mul.write(Bits::from_u8((x & 0x1F).saturating_mul(y & 0x1F)));
        eq.write(Logic::from_bool(x == y));
        let mut v = a.read();
        v.as_array_mut()[3] = l.read();
        put.write(v);
        one.write(Bits::from(l.read()));
        clk.tick().await;
    }
}

/// I.14.7 (4), combinational: a width-1 select and `From` in a combinational body.
#[hardware(combinational)]
fn w1_from_comb(a: In<Bits<1>, MainClk>, l: In<Logic, MainClk>, q: Out<Logic, MainClk>, qb: Out<Bits<1>, MainClk>) {
    q.write(Logic::from(a.read()) ^ a.read()[0] ^ l.read());
    qb.write(Bits::from(l.read()));
}


/// I.14.7 (2): a constant bit index through the shared evaluator — a file const,
/// a const expression, a `const fn` — a static select (a dynamic one before step 8).
const DI_K: usize = 0x3;
const DI_HI: usize = DI_K * 2 + 1;
const fn di_half(n: usize) -> usize {
    n / 2
}
#[hardware(sequential)]
async fn const_index(
    clk: Clock<MainClk>,
    a: In<Bits<8>, MainClk>,
    q: RegOut<Bits<4>, MainClk>,
    p: RegOut<Bits<8>, MainClk>,
) {
    loop {
        let x = a.read();
        p.write(x);
        let mut r = [Logic::Zero; 4];
        r[0] = x[DI_K];
        r[1] = x[DI_HI];
        r[2] = x[di_half(DI_HI) - 1];
        r[3] = x.get(1_0 / 5);
        q.write(Bits::from_slice(&r));
        clk.tick().await;
    }
}
