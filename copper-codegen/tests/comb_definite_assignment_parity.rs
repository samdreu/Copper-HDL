//! Combinational definite assignment: the macro and the transpiler refuse the
//! same set (`CYCLE_DATAFLOW_SEMANTICS.md` §10.11, 2026-11-02).
//!
//! The macro refuses a combinational `Out` written on some paths but not all
//! (`copper_analysis::check_definite_assignment`, a MUST dataflow over the CFG).
//! Until 2026-11-02 the transpiler had only its own `check_no_latches` over the
//! lowered `always_comb`, and an unconditional write went to a module-level
//! `assign` outside it — so `o.write(3); if c { o.write(x) }`, written on every
//! path and admitted by the macro, was refused as a latch. The unconditional
//! write of a port also driven inside an `if`/`match`/`for` now stays in
//! `always_comb` in program order, and the transpiler runs the macro's rule too.
//!
//! Each row is checked against both front ends. The admitted rows are swept
//! under `-Wall` as `tests/fixtures/comb_default_override_dut.rs`; the refused
//! rows are `copper-macros/tests/ui/fail/combinational_latch*.rs`.

fn transpile(src: &str) -> Result<String, String> {
    copper_codegen::transpile_source(src, None, &copper_codegen::EmitConfig::default())
}

fn macro_admits(src: &str) -> bool {
    let file: syn::File = syn::parse_str(src).expect("probe parses");
    let f = file
        .items
        .iter()
        .find_map(|i| if let syn::Item::Fn(f) = i { Some(f) } else { None })
        .expect("one fn");
    copper_analysis::check_definite_assignment(f).is_ok()
}

/// `(name, source, admitted, port the refusal names)`.
const ROWS: &[(&str, &str, bool, &str)] = &[
    (
        "default_then_override",
        r#"#[hardware(combinational)]
        fn m(c: In<Logic, ()>, x: In<Bits<8>, ()>, o: Out<Bits<8>, ()>) {
            o.write(Bits::from_lit::<3>());
            if c.read() == Logic::One { o.write(x.read()); }
        }"#,
        true,
        "",
    ),
    (
        "override_then_default",
        r#"#[hardware(combinational)]
        fn m(c: In<Logic, ()>, x: In<Bits<8>, ()>, o: Out<Bits<8>, ()>) {
            if c.read() == Logic::One { o.write(x.read()); }
            o.write(Bits::from_lit::<3>());
        }"#,
        true,
        "",
    ),
    (
        "nested_overrides",
        r#"#[hardware(combinational)]
        fn m(c: In<Logic, ()>, x: In<Bits<8>, ()>, y: In<Bits<8>, ()>, o: Out<Bits<8>, ()>) {
            o.write(Bits::from_lit::<3>());
            if c.read() == Logic::One {
                let xv = x.read();
                o.write(xv);
                if xv[0] == Logic::One { o.write(y.read()); }
            }
        }"#,
        true,
        "",
    ),
    (
        "match_default_arm",
        r#"#[hardware(combinational)]
        fn m(s: In<Bits<2>, ()>, x: In<Bits<8>, ()>, o: Out<Bits<8>, ()>) {
            o.write(Bits::from_lit::<3>());
            match s.read().as_usize() {
                0 => o.write(x.read()),
                _ => {}
            }
        }"#,
        true,
        "",
    ),
    (
        "for_override",
        r#"#[hardware(combinational)]
        fn m(x: In<Bits<8>, ()>, o: Out<Bits<8>, ()>) {
            let xv = x.read();
            o.write(Bits::zero());
            for i in 0..8 { if xv[i] == Logic::One { o.write(Bits::from_usize(i)); } }
        }"#,
        true,
        "",
    ),
    (
        "partial",
        r#"#[hardware(combinational)]
        fn m(c: In<Logic, ()>, x: In<Bits<8>, ()>, o: Out<Bits<8>, ()>) {
            if c.read() == Logic::One { o.write(x.read()); }
        }"#,
        false,
        "o",
    ),
    (
        "partial_match",
        r#"#[hardware(combinational)]
        fn m(s: In<Bits<2>, ()>, x: In<Bits<8>, ()>, o: Out<Bits<8>, ()>) {
            match s.read().as_usize() {
                0 => o.write(x.read()),
                _ => {}
            }
        }"#,
        false,
        "o",
    ),
    (
        // `o`'s default does not cover `p`.
        "partial_second_port",
        r#"#[hardware(combinational)]
        fn m(c: In<Logic, ()>, x: In<Bits<8>, ()>, o: Out<Bits<8>, ()>, p: Out<Bits<8>, ()>) {
            o.write(Bits::from_lit::<3>());
            if c.read() == Logic::One { o.write(x.read()); p.write(x.read()); }
        }"#,
        false,
        "p",
    ),
    (
        // Two partial writes are not a default: neither runs on every path.
        "partial_two_ifs",
        r#"#[hardware(combinational)]
        fn m(c: In<Logic, ()>, d: In<Logic, ()>, x: In<Bits<8>, ()>, o: Out<Bits<8>, ()>) {
            if c.read() == Logic::One { o.write(Bits::from_lit::<3>()); }
            if d.read() == Logic::One { o.write(x.read()); }
        }"#,
        false,
        "o",
    ),
];

#[test]
fn both_front_ends_admit_and_refuse_the_same_rows() {
    for &(name, src, admitted, port) in ROWS {
        assert_eq!(macro_admits(src), admitted, "{name}: the macro's verdict");
        match transpile(src) {
            Ok(sv) => {
                assert!(admitted, "{name}: transpiled a partial write:\n{sv}");
                assert!(sv.contains("always_comb"), "{name}: the port is one always_comb:\n{sv}");
                assert!(!sv.contains("assign o"), "{name}: no second driver beside always_comb:\n{sv}");
            }
            Err(e) => {
                assert!(!admitted, "{name}: the transpiler refused a shape the macro admits: {e}");
                assert!(
                    e.contains("would infer a latch") && e.contains(&format!("`{port}`")),
                    "{name}: the refusal names `{port}` as a latch: {e}"
                );
            }
        }
    }
}

/// The `for` row's loop index is 32 bits; the port write is width-cast, as a
/// `let` binding's value is, so `-Wall` does not refuse it as WIDTHTRUNC.
#[test]
fn a_port_written_from_a_loop_index_is_width_cast() {
    let sv = transpile(ROWS.iter().find(|r| r.0 == "for_override").unwrap().1).unwrap();
    assert!(sv.contains("o = 8'(i);"), "{sv}");
}
