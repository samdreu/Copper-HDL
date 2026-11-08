// Single source of truth for the wide-datapath (32/64-bit) equivalence test.
// `include!`d for simulation and `include_str!`d for transpilation.
// A width-generic combinational ALU: three results (sum/prod/diff) from shared
// inputs — the same shape as datapath16 but parametric, so it can be Verilated at
// N = 32 and N = 64 (via `.with_params`) to exercise the wide-datapath boundary
// past 16 bits, including a full-width `*` multiply.
#[hardware(combinational)]
fn wide_alu<D: ClockDomain, const N: usize>(
    a: In<Bits<N>, D>,
    b: In<Bits<N>, D>,
    sum: Out<Bits<N>, D>,
    prod: Out<Bits<N>, D>,
    diff: Out<Bits<N>, D>,
) {
    let av = a.read();
    let bv = b.read();
    sum.write(av + bv);
    prod.write(av * bv);
    diff.write(av - bv);
}
