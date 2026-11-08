// The two shapes `copper-macros/tests/ui/pass/register_init_constants_ok.rs` uses that the
// transpiler refused until 2026-11-08 (line `II.3 found preload-ok`, MEASUREMENTS §N.5 item 2,
// §U.3): a `match` used as an expression with no wildcard arm (rustc proved it exhaustive; the
// last arm is the `default`), and a bit write on a register that is a packed vector of one-bit
// elements (`[Logic; N]`, `Bits<N>`), now a whole-value update of the register.

#[derive(Clone, Copy, PartialEq, Eq)]
enum PbPhase {
    Idle,
    Load,
    Run,
}

/// A three-state cycle by an exhaustive `match` expression, and a `[Logic; 4]` shift register
/// written element by element.
#[hardware(sequential)]
async fn pb_shift_and_phase(
    clk: Clock<MainClk>,
    a: In<Logic, MainClk>,
    q: RegOut<Bits<4>, MainClk>,
    p: RegOut<Bits<2>, MainClk>,
) {
    let mut bits = [Logic::Zero; 4];
    let mut phase = PbPhase::Idle;
    loop {
        let mut v: Bits<4> = Bits::zero();
        for i in 0..4 {
            v[i] = bits[i];
        }
        q.write(v);
        p.write(match phase {
            PbPhase::Idle => Bits::from_lit::<0>(),
            PbPhase::Load => Bits::from_lit::<1>(),
            PbPhase::Run => Bits::from_lit::<2>(),
        });
        clk.tick().await;
        bits[3] = bits[2];
        bits[2] = bits[1];
        bits[1] = bits[0];
        bits[0] = a.read();
        phase = match phase {
            PbPhase::Idle => PbPhase::Load,
            PbPhase::Load => PbPhase::Run,
            PbPhase::Run => PbPhase::Idle,
        };
    }
}

/// A `Bits<4>` register's bit written at a run-time index, and read back in the same
/// resumption (the forwarded value).
#[hardware(sequential)]
async fn pb_dynamic_bit(
    clk: Clock<MainClk>,
    k: In<Bits<2>, MainClk>,
    a: In<Logic, MainClk>,
    q: RegOut<Bits<4>, MainClk>,
    r: RegOut<Logic, MainClk>,
) {
    let mut m: Bits<4> = Bits::from_lit::<5>();
    loop {
        let i = k.read().as_usize();
        m[i] = a.read();
        r.write(m[3]);
        q.write(m);
        clk.tick().await;
    }
}
