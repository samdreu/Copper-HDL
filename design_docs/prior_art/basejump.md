# Taylor, "BaseJump STL: SystemVerilog Needs a Standard Template Library for Hardware Design" (invited, DAC 2018, 6 pp.)

Source: `paper/references/basejump_stl_dac2018.pdf`, text in `scratchpad/papers/basejump_stl_dac2018.txt`.
Read in full. Citations are the paper's section numbers.

## 0. What kind of reference this is

A library/position paper, not a semantics paper. It proposes an open-source STL of
synthesizable SystemVerilog modules (containers/plumbing/primitives), lays out design
principles (portability, latency-insensitive interfaces, pervasive parameterisation,
coding style, metaprogramming, unit tests), lists ~34 module families (§6), and gives a
wishlist for SystemVerilog (§5). Its evidence of correctness is **silicon**: "Four
distinct chips designed with BaseJump STL have been taped out, fabricated, and tested,
including a 511-core heterogeneous RISC-V core" (§1); "80% of the modules for the design
were instantiated directly from BaseJump STL" (Abstract); "used by 40+ designers"; "8
unique RTL-to-GDS tapeins ... by third-party users" (§1). For Copper, BaseJump is the
*independent reference implementation* that `examples/basejump/` checks against — a
reference by artefact (the Verilog text, under Verilator), not by specification. The paper
contains no cycle-level description of any module; the meaning of `bsg_dff_en`,
`bsg_fifo_1r1w_small`, `bsg_lfsr` etc. is only in the `.v` files.

## 1. What is given a meaning, and by what

- Modules are given meaning by their synthesizable SystemVerilog source and by use in
  taped-out chips; the paper describes interfaces and intent, not behaviour.
- The one place the paper *does* talk about semantics is the SRAM simultaneous read/write
  case (§3): "A critical correctness issue for ASIC portability are the semantics of
  simultaneous read and writes to SRAMs. In some memories, a simultaneous read and write
  destroys the contents of that memory cell. In other memories, a simultaneous read and
  write results in correct writes of the data, but incorrect read values are returned. In
  other memories, the side effects are that the write occurs and then the read occurs. The
  parameter write_then_read_same_addr_p indicates the intent of the designer as to whether
  they expect the ram to be able to successfully perform a simultaneous write and read of
  the same address, with the write appearing to go first. The use of this parameter is
  critical to avoid portability even when memories have different semantics. The
  implementation of the module thunks down onto a synthesizeable implementation (or a hard
  block with synthesized logic to adjust the semantics) if the behavior is unsupported
  directly by the hard block." Footnote 1: "We chose this approach because a hardened 1R1W
  SRAM that does not support this can be easily modified with some additional standard
  cells to do write-to-read bypassing, where as read-then-write is much more expensive."
  So the semantic choice is pinned by a *parameter* on the interface and realised by
  wrappers — a semantics-by-interface-contract device. This maps directly onto Copper's
  `Memory` read/write ordering modes (ReadFirst / WriteFirst, `RECEIVED_MEMORY_ABI.md`) and
  onto the `ram_read_first` reference.
- Reset (§3): "while FPGA flows allow registers to be initialized to a constant value via
  bitstream, ASICs require proper reset signals. In BaseJump STL, we address the reset issue
  by having explicit reset lines for logic in the interfaces of modules that may require
  reset logic." A portability stance on power-on state: never rely on initialisers. Copper's
  2026-10-29 decision (constant initializers → SV `initial`) is the FPGA-style choice BaseJump
  avoids; worth noting when anchoring a Copper module against a BaseJump module with
  `reset_i`.

## 2. Style

Principles + taxonomy + module catalogue. No semantics, no rules. Verification methodology
is one line: "(9) Testing Suite. The STL shall have unit tests for each module" (§2), and
for metaprogrammed modules: Python-generated SV "shall look like it is written by humans.
This generated code is what is used by users of BaseJump STL, although the python scripts
are available to verify correctness" (§2 item 8).

## 3. Time model / interfaces

- Single-clock synchronous modules with explicit `clk_i`/`reset_i`; CDC modules listed
  separately (§6 items 9–10: `bsg_async_fifo`, `bsg_launch_sync_sync`, `bsg_sync_sync`,
  `bsg_async_ptr_gray`, `bsg_async_credit_counter`) — the counterparts of Copper's
  `sync_2ff` and the `synchronizer` mode.
- **Latency-insensitive handshakes** (§4): three interfaces. *ready-valid-and* (`rv->&`):
  "At the beginning of the cycle, both Producer (P) and Consumer (C) send their willingness
  to send data and to receive it, respectively. During the cycle, the signals propagate
  across the chip. At the end of the cycle, the Producer and Consumer independently AND the
  signals together to determine whether the exchange of data has occurred. ... First, there
  are no combinational loops in the system. Second, the ready and valid signals have almost
  an entire cycle to transfer." *ready-then-valid* (`r->v`, producer "demanding", computes
  valid from the consumer's ready) and *valid-then-ready* (`v->r`, consumer demanding). "But
  what if both the producer and the consumer are demanding? In this case, they are not
  directly compatible. We solve this by inserting a FIFO". Rule: "all else being equal, we
  should chose interfaces that are helpful over those that are demanding."
  This is a precise informal statement of the Moore (registered, "helpful") vs Mealy
  (combinational-through, "demanding") handshake distinction and of why combinational
  ready/valid paths are legitimate but *local*. Copper's decision "handshakes stay
  registered/Moore (RegOut); Mealy ready refused" (valid/ready option 1, 2026-10-21;
  witnesses `vr_sink_mealy`, `vr_skid`) is the choice of `rv->&` only; BaseJump's catalogue
  (FIFOs that are "helpful on both ends", the `r->v`/`v->r` converters) is the independent
  hardware that shows what the refused shapes look like when they are built deliberately.
- Memories: synchronous 1R1W/1RW/2R1W with the `write_then_read_same_addr_p` contract (§3,
  §6 items 15–20).

## 4. State / initial values

Explicit reset lines; constant-initialised registers avoided for ASIC portability (§3).
`harden_p` flag to detect un-substituted hard macros. Nothing on power-on values beyond the
FPGA-vs-ASIC remark.

## 5. Intra-cycle ordering

Only §4's "no combinational loops" property of `rv->&` and the observation that `r->v`/`v->r`
require a round trip within the cycle ("the modules can be at most half a cycle of wire
latency away"). Nothing on blocking/nonblocking, multiple drivers, or simulation.

## 6. Properties claimed and how established

- Productivity and reuse (80% of Celerity from the STL; 5-week RTL-to-GDS class chips);
  portability ("emergency port of a 250nm chip design to a 180nm tapeout without changing
  any of the design's source code", §1) via the "hard shadow directory" of
  process-specific drop-in replacements with identical interfaces (§3).
- Established by tape-outs and use; no formal properties, no equivalence checking, unit
  tests asserted to exist.

## 7. Relation between simulation and synthesized hardware

Not discussed. Non-synthesizable modules are named `bsg_nonsynth_*` by convention (§2 item
7) — a naming discipline for the sim/synth boundary rather than a semantics.

## 8. Transfer to Copper

- **Role**: the independent oracle for "which behavior is correct" (`examples/basejump/`,
  G-E anchoring, `design_docs/ANCHORING_A_MODULE.md`). Its authority is empirical (silicon,
  wide use), not normative; the formal-esque definitions should describe it as "a reference
  implementation whose Verilog is the specification", and note the Solderpad attribution
  obligation.
- **Devices that fit**: the parameter-pinned memory semantics (`write_then_read_same_addr_p`)
  as the model for Copper's `Memory` mode selection; the `rv->&` handshake as Copper's
  registered `RegOut` valid/ready; CDC modules as the counterpart to `synchronizer`; explicit
  reset as a portability constraint Copper's `initial`-based power-on does not satisfy for
  ASIC targets (a limitation to state).
- **Devices that do not fit**: BaseJump's `r->v`/`v->r` are combinational-through handshakes
  that Copper refuses (Mealy ready); BaseJump shows they are a real design space and that
  the standard remedy (insert a FIFO) is structural, not a language rule.
- **Critical take**: BaseJump gives Copper its most credible correctness evidence (3 of 6
  anchored designs match the reference cell count exactly, per memory) precisely because it
  offers no semantics of its own — agreement with an independently written, silicon-proven
  artefact is a different kind of evidence from agreement between two halves of one tool.
  The paper itself should be cited for the handshake taxonomy and the memory-contract
  parameter, and the `.v` sources for everything else.
