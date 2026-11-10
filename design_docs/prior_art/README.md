# Prior-art reading notes — how each HDL paper specifies its semantics

One file per document in `paper/references/` (gitignored, see that directory's
README for the citations), written 2026-11-02 for `../FORMAL_SEMANTICS_SURVEY.md`.
Each answers the same nine questions (object formalized, style, time model, state
rule, intra-cycle ordering, properties and how established, relation to generated
hardware, exact rule locations with transcriptions, transfer to Copper). The
transcribed inference rules of `esterel.md`, `filament.md` and `anvil.md` were
checked against the typeset PDF pages, not the OCR text.

**Dated 2026-11-02, before D-8.** The survey content on each HDL stands. The *Transfer to Copper*
sections of the seventeen 2026-11-02 notes describe Copper as it then was: a plain `Out` on a
clocked body, the `<port>_hold` form, immediate and deferred reads behind a barrier, opening
and closing anchors, and the pre-tick rules. D-8 and Phase C removed all of these (2026-11-06).
Copper's semantics are `../SPEC.md`'s (normative since 2026-11-09, D-31); notes that call
`SYNCHRONOUS_SEMANTICS.md` or `CYCLE_DATAFLOW_SEMANTICS.md` normative predate that. `koika.md`
and `kami.md` (2026-11-05) were written against SPEC.

| file | document |
|---|---|
| `esterel.md` | Berry & Gonthier, Esterel, SCP 1992 |
| `filament.md` | Nigam et al., Filament, PLDI 2023 |
| `anvil.md` | Yu et al., Anvil, ASPLOS 2026 |
| `arch.md` | Arch, arXiv 2026 |
| `clash.md` | Baaij et al., CλaSH, DSD 2010 |
| `bluespec.md` | Nikhil, Bluespec SystemVerilog tutorial, MEMOCODE 2004 |
| `chisel.md` | Bachrach et al., Chisel, DAC 2012 |
| `spade.md` | Skarman & Gustafsson, Spade, FPL 2022 |
| `pymtl3.md` | Jiang et al., PyMTL3, IEEE Micro 2020 |
| `golden_age.md` | Truong & Hanrahan, A Golden Age of HDLs, SNAPL 2019 |
| `myhdl.md` | MyHDL manual 0.11 |
| `prost.md` | Riedl et al., Prost!, LATTE 2026 |
| `rhdl.md` | Basu, RHDL (LATTE 2025) and RustHDL (LATTE 2024) |
| `cummings.md` | Cummings, Nonblocking Assignments, SNUG 2000 |
| `systemc.md` | Accellera SystemC Synthesizable Subset 1.4.7 |
| `verismith.md` | Herklotz & Wickerson, Verismith, FPGA 2020 |
| `basejump.md` | Taylor, BaseJump STL, DAC 2018 |
| `koika.md` | Bourgeat et al., Kôika, PLDI 2020 (short note, 2026-11-05, SPEC III.1; not in `paper/references/`) |
| `kami.md` | Choi et al., Kami, ICFP 2017 (short note, 2026-11-05, SPEC III.1; not in `paper/references/`) |
