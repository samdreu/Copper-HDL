// SPEC I.10.1(b), I.10.4 (D-16 B, D-25): several stagings of one memory port in one
// resumption are ADMITTED, the last winning, and rule 6 (c) counts the stagings before
// the loop. Until D-25 every module here but the two pre-loop ones was refused by rule 6
// (a) or (b) in both front-ends ("accessed inside a nested loop with no tick", "accessed 2
// times in one cycle"), and the pre-loop ones by (c) ("nothing stages a `read()`"). Measured
// three ways before the rules were removed (`review/MEASUREMENTS.md` §W.1 #8, a temporary
// patch): sim = ref = SV on all eight.
//
// - `stage_unrolled_for` — a tick-free counted `for` staging twice (rule 6 (a)); the last
//   iteration's address wins.
// - `stage_for_once`, `stage_for_two_ports` — (a)'s two over-refusals: one iteration, one
//   staging; one iteration touching two distinct ports.
// - `stage_twice_read`, `stage_twice_write` — two unconditional stagings of one read / write
//   port (rule 6 (b); the old `two_accesses_*` shape).
// - `stage_dominated` — II.2.4's dominated staging: a later conditional staging of the port.
// - `stage_preloop_only` — a port staged only in `s_pre`, observed after the loop's tick:
//   valid once, in the resumption ending at edge 2 (I.9.3).
// - `stage_preloop_head` — the same staging observed at the loop head, before the tick:
//   rule 6 (d) does not order against a pre-loop staging (D-25 (4)).
#[hardware(sequential)]
async fn stage_unrolled_for(clk: Clock<MainClk>, a: In<Bits<4>, MainClk>, q: RegOut<Bits<16>, MainClk>) {
    let rom = Memory::<Bits<16>, 1, 0, MainClk, 1, 1>::from_fn(clk.clone(), 16, |i| Bits::from_usize(i * 3 + 7));
    loop {
        for i in 0..2 {
            rom.read_port::<0>().read(a.read().as_usize() / 2 + i);
        }
        clk.tick().await;
        q.write(rom.read_port::<0>().data());
    }
}

#[hardware(sequential)]
async fn stage_for_once(clk: Clock<MainClk>, a: In<Bits<4>, MainClk>, q: RegOut<Bits<16>, MainClk>) {
    let rom = Memory::<Bits<16>, 2, 0, MainClk, 1, 1>::from_fn(clk.clone(), 16, |i| Bits::from_usize(i * 3 + 7));
    loop {
        for _i in 0..1 {
            rom.read_port::<0>().read(a.read().as_usize());
        }
        clk.tick().await;
        q.write(rom.read_port::<0>().data());
    }
}

#[hardware(sequential)]
async fn stage_for_two_ports(clk: Clock<MainClk>, a: In<Bits<4>, MainClk>, q: RegOut<Bits<16>, MainClk>, r: RegOut<Bits<16>, MainClk>) {
    let rom = Memory::<Bits<16>, 2, 0, MainClk, 1, 1>::from_fn(clk.clone(), 16, |i| Bits::from_usize(i * 3 + 7));
    loop {
        for _i in 0..1 {
            rom.read_port::<0>().read(a.read().as_usize());
            rom.read_port::<1>().read(15 - a.read().as_usize());
        }
        clk.tick().await;
        q.write(rom.read_port::<0>().data());
        r.write(rom.read_port::<1>().data());
    }
}

#[hardware(sequential)]
async fn stage_twice_read(clk: Clock<MainClk>, a: In<Bits<4>, MainClk>, b: In<Bits<4>, MainClk>, q: RegOut<Bits<16>, MainClk>) {
    let rom = Memory::<Bits<16>, 1, 0, MainClk, 1, 1>::from_fn(clk.clone(), 16, |i| Bits::from_usize(i * 3 + 7));
    loop {
        rom.read_port::<0>().read(a.read().as_usize());
        rom.read_port::<0>().read(b.read().as_usize());
        clk.tick().await;
        q.write(rom.read_port::<0>().data());
    }
}

#[hardware(sequential)]
async fn stage_twice_write(clk: Clock<MainClk>, a: In<Bits<4>, MainClk>, d: In<Bits<8>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    let ram = Memory::<Bits<8>, 1, 1, MainClk, 1, 1>::new(clk.clone(), 16);
    loop {
        ram.write_port::<0>().write(a.read().as_usize(), d.read());
        ram.write_port::<0>().write(0, d.read() ^ Bits::from_u8(0x5a));
        ram.read_port::<0>().read(a.read().as_usize());
        clk.tick().await;
        if ram.read_port::<0>().is_ready() { q.write(ram.read_port::<0>().data()); }
    }
}

#[hardware(sequential)]
async fn stage_dominated(clk: Clock<MainClk>, sel: In<Logic, MainClk>, a: In<Bits<4>, MainClk>, b: In<Bits<4>, MainClk>, q: RegOut<Bits<16>, MainClk>) {
    let rom = Memory::<Bits<16>, 1, 0, MainClk, 1, 1>::from_fn(clk.clone(), 16, |i| Bits::from_usize(i * 3 + 7));
    loop {
        rom.read_port::<0>().read(a.read().as_usize());
        if sel.read() == Logic::One {
            rom.read_port::<0>().read(b.read().as_usize());
        }
        clk.tick().await;
        q.write(rom.read_port::<0>().data());
    }
}

#[hardware(sequential)]
async fn stage_preloop_only(clk: Clock<MainClk>, a: In<Bits<4>, MainClk>, q: RegOut<Bits<16>, MainClk>, r: RegOut<Bits<4>, MainClk>) {
    let rom = Memory::<Bits<16>, 1, 0, MainClk, 1, 1>::from_fn(clk.clone(), 16, |i| Bits::from_usize(i * 3 + 7));
    rom.read_port::<0>().read(5);
    loop {
        r.write(a.read());
        clk.tick().await;
        if rom.read_port::<0>().is_ready() {
            q.write(rom.read_port::<0>().data());
        }
    }
}

#[hardware(sequential)]
async fn stage_preloop_head(clk: Clock<MainClk>, a: In<Bits<4>, MainClk>, q: RegOut<Bits<16>, MainClk>, r: RegOut<Bits<4>, MainClk>, v: RegOut<Logic, MainClk>) {
    let rom = Memory::<Bits<16>, 1, 0, MainClk, 1, 1>::from_fn(clk.clone(), 16, |i| Bits::from_usize(i * 3 + 7));
    rom.read_port::<0>().read(5);
    loop {
        v.write(Logic::from_bool(rom.read_port::<0>().is_ready()));
        if rom.read_port::<0>().is_ready() { q.write(rom.read_port::<0>().data()); }
        r.write(a.read());
        clk.tick().await;
    }
}
