//! **P6 — Copper's X algebra against IEEE 1800 4-state, as a recorded table**
//! (SPEC I.14.2(b), (h); I.14.7 (1); I.14.8's "the IEEE comparison of D-19 (i-b) as a
//! standing check — Icarus is optional, so a recorded table").
//!
//! `tests/data/ieee1800_4state.txt` holds, per row, an operator, its operands (8-bit
//! `a`, `b` with X bits sprinkled at 0/10/25/50 %, a `Logic` `l`, a shift amount `n`) and
//! the result **Icarus Verilog** (an executable IEEE 1800 4-state simulator, independent
//! of Copper) printed for the operator's SV spelling. Two tests:
//!
//! - `copper_matches_the_recorded_ieee_table` (always runs, no tool needed): every row's
//!   Copper result (`copper_core::Bits<8>` / `Logic`) equals the recorded IEEE result, bit
//!   for bit. This is the standing pin: a change to `types.rs`'s X rules that departs from
//!   IEEE fails here on every machine.
//! - `the_recorded_table_is_what_icarus_computes` (needs `iverilog`; absent is a printed
//!   skip, present-but-broken a failure): re-runs every row under Icarus and requires the
//!   recorded column — so the file cannot drift from the tool it claims to record.
//!
//! The derived `==` is Rust's structural equality (X == X is true), IEEE's `===`; it is
//! compared against `===` only (I.14.2(b)). `eq_logic`/`ne_logic` are IEEE's `==`/`!=`
//! since Phase C step 8 (D-19 2a-B; before, 209 of D-19's 400 rows were more X).
//!
//! `COPPER_IEEE_TABLE_REGENERATE=1` rewrites the file from the generator below via Icarus
//! (a reviewed change: the diff of the data file is the measurement).

use copper_core::{Bits, Logic};
use std::fmt::Write as _;
use std::path::PathBuf;

const ROWS: usize = 400;

fn table_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/ieee1800_4state.txt")
}

fn lcg(s: &mut u64) -> u64 {
    *s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    *s >> 33
}

fn rand_bits(s: &mut u64, xp: u64) -> Bits<8> {
    let mut b = Bits::<8>::zero();
    for i in 0..8 {
        let r = lcg(s);
        b.set(i, if r % 100 < xp { Logic::X } else if (r >> 8) & 1 == 1 { Logic::One } else { Logic::Zero });
    }
    b
}

fn ch(l: Logic) -> char {
    match l {
        Logic::One => '1',
        Logic::Zero => '0',
        Logic::X => 'x',
    }
}

/// MSB first, like a Verilog literal body.
fn s(b: &[Logic]) -> String {
    b.iter().rev().map(|l| ch(*l)).collect()
}

fn parse_bits(t: &str) -> Bits<8> {
    assert_eq!(t.len(), 8, "operand {t}");
    let mut b = Bits::<8>::zero();
    for (k, c) in t.chars().rev().enumerate() {
        b.set(k, parse_logic(c));
    }
    b
}

fn parse_logic(c: char) -> Logic {
    match c {
        '0' => Logic::Zero,
        '1' => Logic::One,
        'x' => Logic::X,
        _ => panic!("bad bit {c}"),
    }
}

/// (operator name, its IEEE 1800 spelling over `a`, `b`, `l`, `n`).
const OPS: &[(&str, &str)] = &[
    ("and", "a & b"),
    ("or", "a | b"),
    ("xor", "a ^ b"),
    ("not", "~a"),
    ("add", "a + b"),
    ("sub", "a - b"),
    ("mul", "a * b"),
    ("div", "a / b"),
    ("rem", "a % b"),
    ("neg", "-a"),
    ("shl", "a << n"),
    ("shr", "a >> n"),
    ("asr", "$unsigned($signed(a) >>> n)"),
    ("derived_eq", "a === b"),
    ("derived_ne", "a !== b"),
    ("eq_logic", "a == b"),
    ("ne_logic", "a != b"),
    ("lt_logic", "a < b"),
    ("le_logic", "a <= b"),
    ("gt_logic", "a > b"),
    ("ge_logic", "a >= b"),
    ("and_reduce", "&a"),
    ("or_reduce", "|a"),
    ("xor_reduce", "^a"),
    ("nand_reduce", "~&a"),
    ("nor_reduce", "~|a"),
    ("xnor_reduce", "~^a"),
    ("mux", "l ? a : b"),
    ("logic_and", "l & a[0]"),
    ("logic_or", "l | a[0]"),
    ("logic_xor", "l ^ a[0]"),
    ("logic_not", "!l"),
    ("sign_extend", "{{4{a[7]}}, a[7:4]}"),
    ("zero_extend", "{4'b0000, a[7:4]}"),
    ("concat", "{a[3:0], b[7:4]}"),
    ("if_eq_one", "IF"),
];

fn bo(b: bool) -> String {
    if b { "1".into() } else { "0".into() }
}

/// Copper's result for one operator, as an MSB-first string.
fn copper(op: &str, a: Bits<8>, b: Bits<8>, l: Logic, n: usize) -> String {
    let lg = |x: Logic| ch(x).to_string();
    match op {
        "and" => s((a & b).as_array()),
        "or" => s((a | b).as_array()),
        "xor" => s((a ^ b).as_array()),
        "not" => s((!a).as_array()),
        "add" => s((a + b).as_array()),
        "sub" => s((a - b).as_array()),
        "mul" => s((a * b).as_array()),
        "div" => s((a / b).as_array()),
        "rem" => s((a % b).as_array()),
        "neg" => s((-a).as_array()),
        "shl" => s((a << n).as_array()),
        "shr" => s((a >> n).as_array()),
        "asr" => s(a.arithmetic_shift_right(n).as_array()),
        "derived_eq" => bo(a == b),
        "derived_ne" => bo(a != b),
        "eq_logic" => lg(a.eq_logic(&b)),
        "ne_logic" => lg(a.ne_logic(&b)),
        "lt_logic" => lg(a.lt_logic(&b)),
        "le_logic" => lg(a.le_logic(&b)),
        "gt_logic" => lg(a.gt_logic(&b)),
        "ge_logic" => lg(a.ge_logic(&b)),
        "and_reduce" => lg(a.and_reduce()),
        "or_reduce" => lg(a.or_reduce()),
        "xor_reduce" => lg(a.xor_reduce()),
        "nand_reduce" => lg(a.nand_reduce()),
        "nor_reduce" => lg(a.nor_reduce()),
        "xnor_reduce" => lg(a.xnor_reduce()),
        "mux" => s(Bits::mux(l, &a, &b).as_array()),
        "logic_and" => lg(l & a.get(0)),
        "logic_or" => lg(l | a.get(0)),
        "logic_xor" => lg(l ^ a.get(0)),
        "logic_not" => lg(!l),
        "sign_extend" => s(a.part_select::<4>(4).sign_extend::<8>().as_array()),
        "zero_extend" => s(a.part_select::<4>(4).zero_extend::<8>().as_array()),
        "concat" => s(a.part_select::<4>(0).concat::<4, 8>(&b.part_select::<4>(4)).as_array()),
        // Copper `if l == Logic::One { 1 } else { 0 }` against SV `if (l) 1 else 0`.
        "if_eq_one" => bo(l == Logic::One),
        _ => panic!("unknown operator {op}"),
    }
}

struct Row {
    op: String,
    a: String,
    b: String,
    l: char,
    n: usize,
    ieee: String,
}

fn generated_operands() -> Vec<(String, String, char, usize)> {
    let mut seed = 0xF00Du64;
    let mut v = Vec::new();
    for _ in 0..ROWS {
        let xp = [0u64, 10, 25, 50][(lcg(&mut seed) % 4) as usize];
        let a = rand_bits(&mut seed, xp);
        let mut b = rand_bits(&mut seed, xp);
        if lcg(&mut seed) % 6 == 0 {
            b = a;
        }
        if lcg(&mut seed) % 9 == 0 {
            b = Bits::zero();
        }
        let l = [Logic::Zero, Logic::One, Logic::X][(lcg(&mut seed) % 3) as usize];
        let n = (lcg(&mut seed) % 11) as usize;
        v.push((s(a.as_array()), s(b.as_array()), ch(l), n));
    }
    v
}

fn read_table() -> Vec<Row> {
    let text = std::fs::read_to_string(table_path()).expect("tests/data/ieee1800_4state.txt");
    text.lines()
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
        .map(|line| {
            let f: Vec<&str> = line.split_whitespace().collect();
            assert_eq!(f.len(), 6, "malformed row: {line}");
            Row {
                op: f[0].into(),
                a: f[1].into(),
                b: f[2].into(),
                l: f[3].chars().next().unwrap(),
                n: f[4].parse().unwrap(),
                ieee: f[5].into(),
            }
        })
        .collect()
}

fn iverilog_available() -> bool {
    match std::process::Command::new("iverilog").arg("-V").output() {
        Ok(o) if o.status.success() => true,
        Ok(o) => panic!(
            "iverilog is installed but `iverilog -V` failed — a broken environment, not a \
             missing tool, and must not be skipped:\n{}{}",
            String::from_utf8_lossy(&o.stdout),
            String::from_utf8_lossy(&o.stderr)
        ),
        Err(_) => false,
    }
}

/// Run `(op, a, b, l, n)` rows under Icarus and return its printed result per row.
fn run_icarus(rows: &[(String, String, String, char, usize)]) -> Vec<String> {
    let nonce = format!(
        "{}_{}",
        std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
    );
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target/ieee1800_table")
        .join(nonce);
    std::fs::create_dir_all(&dir).unwrap();
    let mut tb = String::from(
        "module tb;\n  reg [7:0] a, b; reg l; integer n;\n  reg [7:0] r8; reg r1;\n  initial begin\n",
    );
    for (op, a, b, l, n) in rows {
        writeln!(tb, "    a = 8'b{a}; b = 8'b{b}; l = 1'b{l}; n = {n};").unwrap();
        let expr = OPS.iter().find(|(o, _)| o == op).unwrap().1;
        let one_bit = copper(op, parse_bits(a), parse_bits(b), parse_logic(*l), *n).len() == 1;
        if expr == "IF" {
            writeln!(tb, "    if (l) r1 = 1'b1; else r1 = 1'b0; $display(\"%b\", r1);").unwrap();
        } else if one_bit {
            writeln!(tb, "    r1 = {expr}; $display(\"%b\", r1);").unwrap();
        } else {
            writeln!(tb, "    r8 = {expr}; $display(\"%b\", r8);").unwrap();
        }
    }
    tb.push_str("  end\nendmodule\n");
    let v = dir.join("tb.v");
    let vvp = dir.join("tb.vvp");
    std::fs::write(&v, &tb).unwrap();
    let st = std::process::Command::new("iverilog")
        .args(["-g2012", "-o"])
        .arg(&vvp)
        .arg(&v)
        .output()
        .unwrap();
    assert!(st.status.success(), "iverilog failed:\n{}", String::from_utf8_lossy(&st.stderr));
    let out = std::process::Command::new("vvp").arg("-n").arg(&vvp).output().unwrap();
    assert!(out.status.success(), "vvp failed:\n{}", String::from_utf8_lossy(&out.stderr));
    let lines: Vec<String> = String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(|l| l.trim().to_lowercase())
        .filter(|l| !l.is_empty())
        .collect();
    assert_eq!(lines.len(), rows.len(), "one printed line per row");
    let _ = std::fs::remove_dir_all(&dir);
    // IEEE z never arises from these operators; fold it to x defensively.
    lines.into_iter().map(|l| l.replace('z', "x")).collect()
}

#[test]
fn copper_matches_the_recorded_ieee_table() {
    if std::env::var("COPPER_IEEE_TABLE_REGENERATE").as_deref() == Ok("1") {
        assert!(iverilog_available(), "regenerating the table needs iverilog");
        let mut rows = Vec::new();
        for (a, b, l, n) in generated_operands() {
            for (op, _) in OPS {
                rows.push((op.to_string(), a.clone(), b.clone(), l, n));
            }
        }
        let ieee = run_icarus(&rows);
        let mut f = String::from(
            "# IEEE 1800 4-state results as Icarus Verilog computes them (tests/ieee1800_table.rs).\n\
             # op a b l n result   — operands and result MSB first; regenerate with\n\
             # COPPER_IEEE_TABLE_REGENERATE=1 cargo test --test ieee1800_table\n",
        );
        let ver = std::process::Command::new("iverilog").arg("-V").output().unwrap();
        let first = String::from_utf8_lossy(&ver.stdout).lines().next().unwrap_or("").to_string();
        writeln!(f, "# recorded with: {first}").unwrap();
        for ((op, a, b, l, n), r) in rows.iter().zip(ieee.iter()) {
            writeln!(f, "{op} {a} {b} {l} {n} {r}").unwrap();
        }
        std::fs::create_dir_all(table_path().parent().unwrap()).unwrap();
        std::fs::write(table_path(), f).unwrap();
    }
    let rows = read_table();
    assert_eq!(rows.len(), ROWS * OPS.len(), "the table has every operator on every row");
    let mut per_op: std::collections::BTreeMap<&str, (usize, usize)> = Default::default();
    let mut failures = Vec::new();
    for r in &rows {
        let c = copper(&r.op, parse_bits(&r.a), parse_bits(&r.b), parse_logic(r.l), r.n);
        let e = per_op.entry(OPS.iter().find(|(o, _)| *o == r.op).unwrap().0).or_default();
        if c == r.ieee {
            e.0 += 1;
        } else {
            e.1 += 1;
            if failures.len() < 20 {
                failures.push(format!(
                    "{} a={} b={} l={} n={}: copper {c}, IEEE {}",
                    r.op, r.a, r.b, r.l, r.n, r.ieee
                ));
            }
        }
    }
    for (op, (eq, ne)) in &per_op {
        eprintln!("ieee1800 {op:<12} EQUAL {eq:>4}  DIFFER {ne:>4}");
    }
    assert!(failures.is_empty(), "Copper departs from IEEE 1800 4-state:\n{}", failures.join("\n"));
}

#[test]
fn the_recorded_table_is_what_icarus_computes() {
    if !iverilog_available() {
        eprintln!("skipping: iverilog not installed (the recorded table is still checked)");
        return;
    }
    let rows = read_table();
    let input: Vec<_> = rows.iter().map(|r| (r.op.clone(), r.a.clone(), r.b.clone(), r.l, r.n)).collect();
    let ieee = run_icarus(&input);
    for (r, i) in rows.iter().zip(ieee.iter()) {
        assert_eq!(&r.ieee, i, "recorded row {} {} {} {} {} differs from Icarus", r.op, r.a, r.b, r.l, r.n);
    }
}

#[test]
fn the_table_rows_are_the_generator_s() {
    // The recorded operands are the seeded generator's, so the table is reproducible.
    let rows = read_table();
    let ops_in = generated_operands();
    for (k, r) in rows.iter().enumerate() {
        let (a, b, l, n) = &ops_in[k / OPS.len()];
        assert_eq!((&r.a, &r.b, r.l, r.n), (a, b, *l, *n), "row {k}");
        assert_eq!(r.op, OPS[k % OPS.len()].0, "row {k}");
    }
}
