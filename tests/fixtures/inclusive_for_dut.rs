// A `for` over an inclusive range, `for v in a..=b` (D-27, 2026-11-09; line `II.3 found
// inclusive-for`, MEASUREMENTS §Z.10, §AA.3): IN with Rust's meaning. A tick-free one is unrolled
// `b − a + 1` times (SPEC I.2.4), emitted as an SV `for (… ; i <= b; …)`, so an end at its type's
// maximum does not overflow; a tick-bearing one is I.8.5's counted loop exiting on `c > e`, and a
// constant end at its type's maximum is refused by both front-ends (II.1.2 R23; ui/fail
// `inclusive_ticking_for_at_max`). Each module is swept on three legs.

const IFR_LAST: u8 = 3;
const IFR_HI: u8 = 254;

/// The battery's tick-free probe (`zzq_16`): a population count of the low four bits.
#[hardware(combinational)]
fn ifr_comb<D: ClockDomain>(a: In<Bits<8>, D>, o: Out<Bits<8>, D>) {
    let mut s: u8 = 0;
    for i in 0..=3 {
        if a.read()[i] == Logic::One {
            s = s.wrapping_add(1);
        }
    }
    o.write(Bits::from_u8(s));
}

/// Tick-free in a clocked body: a const end, a loop variable read as a value, a nested pair.
#[hardware(sequential)]
async fn ifr_tickfree_seq(clk: Clock<MainClk>, a: In<Bits<8>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    let mut acc: u8 = 0;
    loop {
        let mut s: u8 = 0;
        for i in 1..=IFR_LAST {
            s = s.wrapping_add(a.read().as_u8() >> i);
        }
        for j in 0..=1 {
            for k in 0..=2 {
                s = s ^ ((j + 2 * k) as u8);
            }
        }
        acc = acc.wrapping_add(s);
        q.write(Bits::from_u8(acc));
        clk.tick().await;
    }
}

/// Tick-free with the end at its type's maximum: six iterations, 250 through 255 — Rust's
/// `RangeInclusive` does not overflow, and neither does `i <= 255`. (`i as u8`: a loop variable
/// read as a narrower operand without a cast is line `II.3 found loop-var-type`.)
#[hardware(sequential)]
async fn ifr_tickfree_max(clk: Clock<MainClk>, a: In<Bits<8>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    loop {
        let mut s: u8 = 0;
        for i in 250u8..=255u8 {
            s = s.wrapping_add((i as u8) ^ a.read().as_u8());
        }
        q.write(Bits::from_u8(s));
        clk.tick().await;
    }
}

/// Tick-free, writing an array register: the unrolled form, one write port per iteration.
#[hardware(sequential)]
async fn ifr_tickfree_array(clk: Clock<MainClk>, a: In<Bits<2>, MainClk>, d: In<Bits<8>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    let mut r: [u8; 4] = [0u8; 4];
    loop {
        q.write(Bits::from_u8(r[a.read().as_usize()]));
        for i in 1..=2 {
            r[i] = d.read().as_u8().wrapping_add(i as u8);
        }
        clk.tick().await;
    }
}

/// The battery's tick-bearing probe (`zzq_25`): three cycles per pass.
#[hardware(sequential)]
async fn ifr_ticking_probe(clk: Clock<MainClk>, a: In<Bits<8>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    let mut n: u8 = 0;
    loop {
        for _ in 0..=2 {
            q.write(Bits::from_u8(n));
            clk.tick().await;
        }
        n = n ^ a.read().as_u8();
    }
}

/// Tick-bearing over a const end and a typed end one below its maximum (passes R23).
#[hardware(sequential)]
async fn ifr_ticking_const(clk: Clock<MainClk>, a: In<Bits<8>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    loop {
        for i in 1..=IFR_LAST {
            q.write(Bits::from_u8(i ^ a.read().as_u8()));
            clk.tick().await;
        }
        for i in 252u8..=IFR_HI {
            q.write(Bits::from_u8(i));
            clk.tick().await;
        }
    }
}

/// Tick-bearing over a run-time end: empty when `n < 2` (zero iterations, zero time, I.8.4(b)),
/// so the module loop has a tick of its own.
#[hardware(sequential)]
async fn ifr_ticking_runtime(clk: Clock<MainClk>, a: In<Bits<2>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    loop {
        let n = a.read().as_u8();
        q.write(Bits::from_u8(0xf0 | n));
        clk.tick().await;
        for i in 2..=n {
            q.write(Bits::from_u8(i));
            clk.tick().await;
        }
    }
}

/// Tick-bearing with a `break` the author wrote: it leaves before the body's tick.
#[hardware(sequential)]
async fn ifr_ticking_break(clk: Clock<MainClk>, a: In<Bits<8>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    let mut t: u8 = 0;
    loop {
        for i in 0..=4 {
            if a.read().as_u8() == i as u8 {
                break;
            }
            t = t.wrapping_add(1);
            q.write(Bits::from_u8(t));
            clk.tick().await;
        }
        q.write(Bits::from_u8(0x80 | t));
        clk.tick().await;
    }
}
