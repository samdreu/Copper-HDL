# Publication checklist

> **Opened 2026-11-10.** The items that block a public release of this repository, and the
> ones that are done. Each open item names what closing it means. Reference Verilog is written
> by the user only, so no item here is closed by a session writing one. Sources: the provenance
> check of 2026-11-10 (git history of every reference `.sv` file) and `review/MEASUREMENTS.md`
> §AK.

## Done

- **License.** Dual MIT OR Apache-2.0 since 2026-11-10 (`LICENSE-MIT`, `LICENSE-APACHE`, the
  `license` field of every crate's `Cargo.toml`, README § License). The vendored BaseJump STL
  files keep the Solderpad Hardware License v0.51 (`LICENSE-Solderpad`), and each one has a
  header that names its upstream module and the adaptations.
- **Orphan reference files deleted** (2026-11-10, §AK): `examples/counter_enable.sv`,
  `tests/fixtures/reference_sv/accumulator_en_dut.sv` and
  `tests/fixtures/reference_sv/aggregate_locals_dut.sv` (that one was malformed). Nothing read
  any of them.
- **`bit_not_bits` demoted** (2026-11-10, §AK). Its reference was written in a session, so it
  is no longer a `REFERENCE` row. G-E went from 5 anchored to 4. The module is owed an
  independent reference (SPEC III.2.11 (j) item 9).

## Blocking: third-party references with no redistribution license

The user replaces each one with a reference they write. The replacement is a file named for the
same module, with the same ports, that passes the same check. It also needs a header that says the
user wrote it. Until then, each file stays and keeps anchoring what it anchors today.

| file | apparent source | compared against it | replacing it means |
|---|---|---|---|
| `examples/combinational/sv/priority_encode.sv` | its header says "Adapted from Greg Stitt, University of Florida" | `examples/combinational/priority_encode.rs` (`with_verilog`, the example's `main()`); one of the five combinational goldens of SPEC P24 and `TIMING_COVERAGE_MATRIX.md` row 3 | a new `priority_encode` module with the same ports and widths that passes `cargo run --example priority_encode` |
| `examples/sequential/sv/shift_register.sv` | its header cites chipverify.com | `examples/sequential/shift_register.rs` (`with_verilog`, `main()`) | a new `shift_register` module with the same ports (`rstn` included) that passes `cargo run --example shift_register` |
| `examples/memory/sv/dual_port_ram.sv` | its header ("Simple Dual-Port Block RAM with One Clock / File: dual_port_ram.v") reads like a vendor synthesis template. This is not confirmed. | `examples/memory/dual_port_ram.rs` (`with_verilog`, `main()`); SPEC D-16 names it the memory anchor, and SPEC P23 and `TIMING_COVERAGE_MATRIX.md` row 4 cite it | a new read-first, latency-1, simple dual-port RAM that passes `cargo run --example dual_port_ram`; SPEC D-16, P23 and matrix row 4 keep their citation if the path stays the same |

## Open: provenance that git does not record

**Status.** "user" means the user confirmed they wrote the file. "session" means the user
confirmed they did not write it, or the file says it was written in a session. "unknown" means
neither. "Claude co-author" means the commit that added the file carries a `Co-Authored-By: Claude`
trailer, so the commit was made in a session. A header that says "hand-written" does not say who
wrote it. Each file has exactly one commit in its history.

| file | read by | added in | status |
|---|---|---|---|
| `examples/combinational/sv/one_bit_comparator.sv` | `examples/combinational/one_bit_comparator.rs` (example self-check, one of P24's five combinational goldens) | 82f7a10e, 2026-06-26, by the user alone; no header | unknown. No source is named, and no license is recorded (README § License). |
| `examples/sequential/sv/pattern_detector.sv` | `examples/sequential/pattern_detector.rs`, `tests/pattern_detector_equivalence.rs` (det_110101) | b7ed8395, 2026-07-13, by the user alone; tutorial-style, no header | unknown. No source is named, and no license is recorded. |
| `tests/fixtures/reference_sv/fifo_mem_new.sv` | `tests/verilog_fifo_memory_new.rs` | 65a4e28f, 2026-08-27, Claude co-author. The file was moved in from an untracked `verilog/fifo_mem_new.v`, and git has no history before that. | unknown |
| `tests/fixtures/reference_sv/hybrid_pipeline.sv` | `tests/verilog_hybrid_pipeline.rs` | 65a4e28f (the same move, from `verilog/hybrid_pipeline.v`) | unknown |
| `tests/fixtures/reference_sv/bit_not_bits.sv` | nothing since 2026-11-10 | 7abfd98a, 2026-08-26, Claude co-author | session (the user confirmed 2026-11-10). It is kept and read by nothing. Deleting it has not been decided. |
| `tests/fixtures/timing_probe_sv/counter_comb.sv`, `counter_reg.sv`, `dff.sv`, `enff.sv` | `tests/timing_probe_investigation.rs` (probe goldens) | 1d13296a, 2026-07-25, Claude co-author | unknown |
| `tests/fixtures/timing_probe_sv/ram1.sv` | `tests/mem_latency_probe.rs`; `TIMING_COVERAGE_MATRIX.md` calls it a dormant golden | 1d13296a, Claude co-author | unknown |

**Written by the user, confirmed 2026-11-10.** The provenance check covered only the 16 files
without a header. The six files below have a header that calls them hand-written or independent,
and each was added in a commit with a Claude co-author. The user confirmed on 2026-11-10 that they
wrote all six; the co-author trailers are commit attribution only. Each header now has a
`PROVENANCE` line that says so. They are not blocking.

| file | anchors | added in | status |
|---|---|---|---|
| `tests/fixtures/reference_sv/ram_read_first.sv` | **G-E** (`REFERENCE` row): `ram_read_first` | 7abfd98a, 2026-08-26 | user (confirmed 2026-11-10) |
| `tests/fixtures/reference_sv/handshake.sv` | **G-E** (`REFERENCE` row): the three `handshake` copies | 89284f2, 2026-09-03 | user (confirmed 2026-11-10) |
| `examples/cdc/sv/sync_2ff_ref.sv` | `tests/cdc_synchronizer_anchor.rs`; matrix row 5 | 525a0dce, 2026-08-21 | user (confirmed 2026-11-10) |
| `examples/cdc/sv/two_domain_hierarchy.sv` | `tests/two_domain_hierarchy_cdc.rs`; SPEC P19; matrix row 5 | 0d67f9ea, 2026-07-30 | user (confirmed 2026-11-10) |
| `examples/sequential/sv/pattern_detector_010.sv` | `tests/det_010_independent_golden.rs`; matrix row 6 | d1cab795, 2026-07-29 | user (confirmed 2026-11-10) |
| `tests/fixtures/timing_probe_sv/mac_fsm.sv` | the `mac_fsm` register set (SPEC III.2.11) | 1d13296a, 2026-07-25 | user (confirmed 2026-11-10) |

## Open: other items found on a quick pass

- **`rust-version = "1.85"`** in every crate's `Cargo.toml` has not been checked. The tree builds
  only on the pinned 1.98.1 (`rust-toolchain.toml`), and no build with 1.85 has been recorded.
  `edition = "2024"` needs at least 1.85. To close it, build the library crates with 1.85, or
  set `rust-version` to the oldest version that was measured. The trybuild `.stderr` files pin
  1.98.1's diagnostics, so only the build can be checked on an older compiler, not the tests.
- **README § License** lists five references "adapted from third-party tutorial or template
  sources". Three of them are the blocking items above. The other two, `one_bit_comparator.sv`
  and `pattern_detector.sv`, have unknown status. That section must agree with this file once the
  user replaces or confirms each one.
- **Never touched by this checklist:** `paper/` is the user's.
