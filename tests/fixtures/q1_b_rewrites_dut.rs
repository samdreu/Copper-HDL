// The five one-job ("B", Prost's model, D-8) rewrites measured in
// design_docs/review/Q1_TRADEOFFS.md §4 — every output a `RegOut`, every write
// before the tick — reproduced here verbatim from that session's sources so the
// corpus sweep AND the reference-interpreter leg (copper-ref, SPEC_OVERVIEW.md §7)
// run them every time. Each was measured trace-identical to its independent anchor
// (§4: det_010 vs pattern_detector_010.sv, bsg_dff_en and bsg_counter_up_down and
// sipo_block vs BaseJump, vr_sink vs the refused Mealy module's own emitted SV).
// The shipped two-job spellings are untouched: these are the Phase C targets, not
// their replacements. Under the one-job model these are the first modules the
// interpreter must agree with the simulator on.

enum State {
    A,
    B,
    C,
    D,
}

// (a) det_010 → B: a RegOut of the next state, written before the tick.
#[hardware(sequential)]
async fn q1_b_det_010(
    clk: Clock<MainClk>,
    rstn: In<Logic, MainClk>,
    in_i: In<Logic, MainClk>,
    out_o: RegOut<Logic, MainClk>,
) {
    let mut state = State::A;
    loop {
        if rstn.read() == Logic::Zero {
            state = State::A;
        } else {
            state = match (state, in_i.read()) {
                (State::A, Logic::Zero) => State::B,
                (State::B, Logic::One) => State::C,
                (State::B, Logic::Zero) => State::B,
                (State::C, Logic::Zero) => State::D,
                (State::D, Logic::Zero) => State::B,
                _ => State::A,
            };
        }
        if matches!(state, State::D) {
            out_o.write(Logic::One);
        } else {
            out_o.write(Logic::Zero);
        }
        clk.tick().await;
    }
}

// (b) bsg_dff_en → B: the enabled register as a held RegOut (D-6's anchor).
#[hardware(sequential)]
async fn q1_b_bsg_dff_en(
    clk: Clock<MainClk>,
    data_i: In<Bits<8>, MainClk>,
    en_i: In<Logic, MainClk>,
    data_o: RegOut<Bits<8>, MainClk>,
) {
    loop {
        if en_i.read() == Logic::One {
            data_o.write(data_i.read());
        }
        clk.tick().await;
    }
}

const PTR_W: usize = 3;

// (c) bsg_counter_up_down → B.
#[hardware(sequential)]
async fn q1_b_bsg_counter_up_down(
    clk: Clock<MainClk>,
    reset_i: In<Logic, MainClk>,
    up_i: In<Logic, MainClk>,
    down_i: In<Logic, MainClk>,
    count_o: RegOut<Bits<PTR_W>, MainClk>,
) {
    let mut count = Bits::<3>::zero();
    loop {
        if reset_i.read() == Logic::One {
            count = Bits::zero();
        } else {
            let step: Bits<PTR_W> = Bits::from_lit::<1>();
            let up: Bits<PTR_W> = if up_i.read() == Logic::One { step } else { Bits::zero() };
            let down: Bits<PTR_W> = if down_i.read() == Logic::One { step } else { Bits::zero() };
            count = count - down + up;
        }
        count_o.write(count);
        clk.tick().await;
    }
}

// (c'') sipo_block → B: the wN_dbg taps as RegOuts.
#[hardware(sequential)]
async fn q1_b_sipo_block(
    clk: Clock<MainClk>,
    data_i: In<Bits<4>, MainClk>,
    data_o: RegOut<Bits<16>, MainClk>,
    w0_dbg: RegOut<Bits<4>, MainClk>,
    w1_dbg: RegOut<Bits<4>, MainClk>,
    w2_dbg: RegOut<Bits<4>, MainClk>,
    w3_dbg: RegOut<Bits<4>, MainClk>,
) {
    loop {
        let w0 = data_i.read();
        w0_dbg.write(w0);
        clk.tick().await;
        let w1 = data_i.read();
        w1_dbg.write(w1);
        clk.tick().await;
        let w2 = data_i.read();
        w2_dbg.write(w2);
        clk.tick().await;
        let w3 = data_i.read();
        w3_dbg.write(w3);
        let mut bits = [Logic::Zero; 16];
        for k in 0..4 {
            bits[k] = w0[k];
            bits[4 + k] = w1[k];
            bits[8 + k] = w2[k];
            bits[12 + k] = w3[k];
        }
        data_o.write(Bits::from_slice(&bits));
        clk.tick().await;
    }
}

// (d) vr_sink_mealy → B: the Mealy ready realized as a flop of (next state, this
// edge's input), trace-identical to the refused module's `assign ready`.
#[hardware(sequential)]
async fn q1_b_vr_sink(
    clk: Clock<MainClk>,
    valid: In<Logic, MainClk>,
    data: In<Bits<8>, MainClk>,
    drain: In<Logic, MainClk>,
    ready: RegOut<Logic, MainClk>,
    sum: RegOut<Bits<8>, MainClk>,
) {
    let mut full: Logic = Logic::Zero;
    let mut held: Bits<8> = Bits::zero();
    let mut acc: Bits<8> = Bits::zero();
    loop {
        let can_take: Logic = if full == Logic::Zero || drain.read() == Logic::One {
            Logic::One
        } else {
            Logic::Zero
        };
        if full == Logic::One && drain.read() == Logic::One {
            acc = acc + held;
            full = Logic::Zero;
        }
        if can_take == Logic::One && valid.read() == Logic::One {
            held = data.read();
            full = Logic::One;
        }
        if full == Logic::Zero || drain.read() == Logic::One {
            ready.write(Logic::One);
        } else {
            ready.write(Logic::Zero);
        }
        sum.write(acc);
        clk.tick().await;
    }
}
