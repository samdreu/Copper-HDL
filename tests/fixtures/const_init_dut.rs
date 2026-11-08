// R11 under D-21 A3 (2026-11-07, `design_docs/review/MEASUREMENTS.md` §N): a register's
// power-on value is any Rust constant. The macro evaluates each pre-loop register
// initializer in a `const { }` block (rustc the judge); the transpiler folds the same
// initializer through `copper_analysis::consts::ConstEnv`. Until D-21 the syntactic
// `is_constant_expr` refused the single-segment `const fn` call `ci_seed(3)` in both
// front-ends; the file const and the associated const were admitted already. Each
// module states a power-on value only these spellings name, and shows it at obs_1.

const CI_START: u8 = 0x5A;
const CI_STEP: u8 = CI_START / 30;

const fn ci_seed(k: u8) -> u8 {
    k * 2 + 1
}

struct CiCfg;

impl CiCfg {
    const K: u8 = 9;
    const fn twice(x: u8) -> u8 {
        x * 2
    }
}

/// A free `const fn` call, in a `Bits` constructor and as an integer register's start.
#[hardware(sequential)]
async fn init_const_fn(
    clk: Clock<MainClk>,
    a: In<Bits<8>, MainClk>,
    o: RegOut<Bits<8>, MainClk>,
    p: RegOut<Bits<8>, MainClk>,
) {
    let mut n: Bits<8> = Bits::from_u8(ci_seed(3));
    let mut k: u8 = ci_seed(5);
    loop {
        o.write(n);
        p.write(Bits::from_u8(k));
        clk.tick().await;
        n = n + a.read();
        k = k.wrapping_add(1);
    }
}

/// File consts, one defined through the other.
#[hardware(sequential)]
async fn init_file_const(
    clk: Clock<MainClk>,
    a: In<Bits<8>, MainClk>,
    o: RegOut<Bits<8>, MainClk>,
    p: RegOut<Bits<8>, MainClk>,
) {
    let mut n = Bits::<8>::from_u8(CI_START);
    let mut c: u8 = CI_STEP;
    loop {
        o.write(n);
        p.write(Bits::from_u8(c));
        clk.tick().await;
        n = n ^ a.read();
        // Until 2026-11-08 `c.wrapping_add(CI_STEP)` lint-failed: a `u8` file const in a
        // body expression emitted as a 32-bit `localparam int` (§N.5 item 1); it is a
        // sized `localparam` since §U.6 — `init_file_const_in_body` below.
        c = c.wrapping_add(1);
    }
}

/// `u8` file consts in body expressions (line `II.3 found u8-const`, MEASUREMENTS §U.6): each
/// emits as a sized `localparam logic [7:0]`, so `c + CI_STEP` is an 8-bit add.
#[hardware(sequential)]
async fn init_file_const_in_body(
    clk: Clock<MainClk>,
    a: In<Bits<8>, MainClk>,
    o: RegOut<Bits<8>, MainClk>,
    p: RegOut<Bits<8>, MainClk>,
) {
    let mut c: u8 = CI_STEP;
    loop {
        p.write(Bits::from_u8(c));
        o.write(a.read() ^ Bits::from_u8(CI_START));
        clk.tick().await;
        c = c.wrapping_add(CI_STEP);
    }
}

/// An associated const (`T::K`) and an associated `const fn` over it.
#[hardware(sequential)]
async fn init_assoc_const(
    clk: Clock<MainClk>,
    a: In<Bits<8>, MainClk>,
    o: RegOut<Bits<8>, MainClk>,
    p: RegOut<Bits<8>, MainClk>,
) {
    let mut n = Bits::<8>::from_u8(CiCfg::K);
    let mut m: Bits<8> = Bits::from_u8(CiCfg::twice(CiCfg::K));
    loop {
        o.write(n);
        p.write(m);
        clk.tick().await;
        n = n + a.read();
        m = m - a.read();
    }
}
