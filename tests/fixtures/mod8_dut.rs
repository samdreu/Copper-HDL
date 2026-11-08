// Single source of truth for the modulo-datapath equivalence test.
// `include!`d for simulation and `include_str!`d for transpilation.
// Exercises the `%` (remainder) operator, which the transpiler supports (unlike
// `/`, which it rejects — see copper-codegen/tests/unsupported_constructs.rs). The
// divisor is kept non-zero by the stimulus (a zero divisor is X in hardware).
#[hardware(combinational)]
fn mod8<D: ClockDomain>(a: In<Bits<8>, D>, b: In<Bits<8>, D>, o: Out<Bits<8>, D>) {
    o.write(a.read() % b.read());
}
