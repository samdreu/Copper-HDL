// A combinational `Out` written unconditionally (a default) AND inside an
// `if`/`match`/`for` (an override) is written on every path, so it is not a latch.
//
// Until 2026-11-02 the macro admitted these (its definite-assignment check is a
// MUST dataflow over the CFG) and the transpiler refused them as latches: the
// unconditional write became a module-level `assign`, the nested one stayed in
// `always_comb`, and the latch check saw only the nested one — a front-end parity
// break. The default now stays in `always_comb`, in program order
// (`CYCLE_DATAFLOW_SEMANTICS.md` §10.12). A genuinely partial write is still
// refused by both front ends (`copper-macros/tests/ui/fail/combinational_latch*.rs`).

/// The shape the task reported: a constant default, then a conditional override.
#[hardware(combinational)]
pub fn cdo_default_then_override<D: ClockDomain>(c: In<Logic, D>, x: In<Bits<8>, D>, o: Out<Bits<8>, D>) {
    o.write(Bits::from_lit::<3>());
    if c.read() == Logic::One {
        o.write(x.read());
    }
}

/// The reverse: the trailing unconditional write wins on every path (always 3).
#[hardware(combinational)]
pub fn cdo_override_then_default<D: ClockDomain>(c: In<Logic, D>, x: In<Bits<8>, D>, o: Out<Bits<8>, D>) {
    if c.read() == Logic::One {
        o.write(x.read());
    }
    o.write(Bits::from_lit::<3>());
}

/// Nested overrides: the inner one only under the outer one.
#[hardware(combinational)]
pub fn cdo_nested_overrides<D: ClockDomain>(
    c: In<Logic, D>,
    x: In<Bits<8>, D>,
    y: In<Bits<8>, D>,
    o: Out<Bits<8>, D>,
) {
    o.write(Bits::from_lit::<3>());
    if c.read() == Logic::One {
        let xv = x.read();
        o.write(xv);
        if xv[0] == Logic::One {
            o.write(y.read());
        }
    }
}

/// A default, then a `match` whose `_` arm writes nothing.
#[hardware(combinational)]
pub fn cdo_match_default_arm<D: ClockDomain>(
    s: In<Bits<2>, D>,
    x: In<Bits<8>, D>,
    y: In<Bits<8>, D>,
    o: Out<Bits<8>, D>,
) {
    o.write(Bits::from_lit::<3>());
    match s.read().as_usize() {
        0 => o.write(x.read()),
        1 => o.write(y.read()),
        _ => {}
    }
}

/// A default, then overrides inside a `for` (the last set bit wins).
#[hardware(combinational)]
pub fn cdo_for_override<D: ClockDomain>(x: In<Bits<8>, D>, o: Out<Bits<8>, D>) {
    let xv = x.read();
    o.write(Bits::zero());
    for i in 0..8 {
        if xv[i] == Logic::One {
            o.write(Bits::from_usize(i));
        }
    }
}

/// Control: every arm writes, no default — transpiled before the fix too, and
/// its SystemVerilog is unchanged by it.
#[hardware(combinational)]
pub fn cdo_match_every_arm<D: ClockDomain>(
    s: In<Bits<2>, D>,
    x: In<Bits<8>, D>,
    y: In<Bits<8>, D>,
    o: Out<Bits<8>, D>,
) {
    match s.read().as_usize() {
        0 => o.write(x.read()),
        1 => o.write(y.read()),
        _ => o.write(Bits::from_lit::<3>()),
    }
}
