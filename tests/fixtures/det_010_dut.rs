// Single source of truth for the det_010 equivalence test.
// A 4-state Moore "010" detector, single-tick, in the one-job spelling (D-8): the
// output is a `RegOut` of the next state, written before the tick. Also exercises
// `matches!`.
enum State { A, B, C, D }

#[hardware(sequential)]
async fn det_010(
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
        // The Moore output of the NEXT state, written before the tick: the `RegOut`
        // shows it from the edge that commits `state` (D-8; Q1_TRADEOFFS §4 (a)).
        if matches!(state, State::D) {
            out_o.write(Logic::One);
        } else {
            out_o.write(Logic::Zero);
        }
        clk.tick().await;
    }
}
