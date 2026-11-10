//! Shared harness for transpiler equivalence tests.
//!
//! Every equivalence test does the same three things:
//!   1. transpile a DUT fixture to SystemVerilog,
//!   2. run the DUT in the Copper simulator, recording actual outputs per cycle
//!      alongside a reference model's expected outputs,
//!   3. Verilate the *generated* `.sv` against the simulator's own trace.
//!
//! Only steps 1 and 3 are boilerplate; the per-DUT parts (port wiring, stimulus,
//! reference model) stay in each test because DUT signatures genuinely differ.
//! This module factors out the boilerplate and the assertion.
//!
//! Each DUT fixture in `tests/fixtures/` is used two ways — `include!`d for
//! simulation and `include_str!`d for transpilation — so the simulated and
//! transpiled designs are byte-identical by construction.
//!
//! Note the division of labour when a test fails:
//!   * `verilator: FAIL` → the transpiler disagrees with the simulator (a real
//!     transpiler bug — the simulator is the semantic source of truth).
//!   * `trace: FAIL` with `verilator: PASS` → the transpiler is fine and the
//!     test's own reference model is wrong.

#![allow(dead_code)] // each test binary uses a different subset

use std::path::PathBuf;

use copper_core::types::Bits;
use copper_core::Logic;
use copper_sim::{make_cycle, CycleData, HardwareTest, SimulationTrace};

mod relation;
#[allow(unused_imports)]
pub use relation::{ledger_write, run_step, runtime_failure_kind, Pattern, SeqOutcome, LEDGER_ENV};
use relation::{one_line, ref_runtime_failure_kind, to_int};

/// Whether Verilator can be used, for tests that drive it directly rather than
/// through `EquivalenceTest`.
///
/// Returns `false` **only** when the binary is genuinely absent (the one skippable
/// case). If Verilator is installed but fails to run — classically a stale
/// `VERILATOR_ROOT`, a known hazard on this project's machines (see CLAUDE.md) —
/// this **panics** instead of returning `false`.
///
/// That distinction is the whole point. A probe that treats "broken" as "absent"
/// makes a misconfigured environment indistinguishable from an unconfigured one, and
/// the Verilator arms of a test — often the only thing anchoring it to real hardware
/// — quietly do not run while the suite reports green.
pub fn verilator_available() -> bool {
    match copper_sim::verilator_status() {
        Ok(()) => true,
        Err(e) if e.starts_with(copper_sim::VERILATOR_NOT_INSTALLED) => {
            eprintln!("skipping: {e}");
            false
        }
        Err(e) => panic!("Verilator is present but unusable, so this test cannot be skipped:\n{e}"),
    }
}

/// A `verilator` invocation with `VERILATOR_ROOT` cleared — always use this rather
/// than `Command::new("verilator")`, so a stale value cannot break the build with a
/// confusing error after `verilator_available` has already said yes.
pub fn verilator_command() -> std::process::Command {
    let mut cmd = std::process::Command::new("verilator");
    cmd.env_remove("VERILATOR_ROOT");
    cmd
}

/// Convenience: `bool` → `Logic`.
pub fn logic(b: bool) -> Logic {
    if b {
        Logic::One
    } else {
        Logic::Zero
    }
}

/// A tiny deterministic PRNG (SplitMix64) for reproducible randomized/property
/// tests. Not cryptographic — just well-spread and seedable, so a failing case is
/// reproducible from its seed. Mirrors the generator `copper-sim` uses for
/// `PollOrder::Seeded`, kept here so tests need no external `rand` dependency.
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng(seed)
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// A random `u8`.
    pub fn u8(&mut self) -> u8 {
        self.next_u64() as u8
    }

    /// A uniform value in `0..n` (n > 0). Modulo bias is irrelevant for these tests.
    pub fn below(&mut self, n: u64) -> u64 {
        self.next_u64() % n
    }

    /// A random single bit as `Logic`.
    pub fn logic(&mut self) -> Logic {
        logic(self.next_u64() & 1 == 1)
    }
}

/// A port payload that seeded random stimulus can be generated for, and that can be
/// handed to the trace recorder as bits.
///
/// The corpus is near-uniform in this respect — 175 `Bits<N>` ports and 123 `Logic`
/// ports, and no third payload shape outside the const-generic and `Vec` modules the
/// transpiler does not accept — which is what makes a mechanical sweep possible.
/// See `design_docs/CORPUS_DIFFERENTIAL_SWEEP.md`.
pub trait RandStim: Copy {
    /// The payload's width in bits: the length of `as_bits` and of the slice
    /// `from_bits` takes. What the sweep's stimulus plan sizes its vectors by.
    const WIDTH: usize;
    /// A uniformly random value of this type.
    fn rand(rng: &mut Rng) -> Self;
    /// The value whose `as_bits` is `bits` (`bits.len() == WIDTH`, LSB-first).
    fn from_bits(bits: &[Logic]) -> Self;
    /// The value as a bit slice, in the layout `record_cycle` expects.
    fn as_bits(&self) -> Vec<Logic>;
}

impl RandStim for Logic {
    const WIDTH: usize = 1;
    fn rand(rng: &mut Rng) -> Self {
        rng.logic()
    }
    fn from_bits(bits: &[Logic]) -> Self {
        assert_eq!(bits.len(), 1, "a Logic is one bit");
        bits[0]
    }
    fn as_bits(&self) -> Vec<Logic> {
        vec![*self]
    }
}

impl<const N: usize> RandStim for Bits<N> {
    const WIDTH: usize = N;
    /// Built bit by bit rather than from an integer: `from_usize` panics on a value
    /// too wide for `N`, and `from_u64` will not compile below `N = 64`. Per-bit is
    /// width-exact for every `N` the corpus uses and every one it might.
    fn rand(rng: &mut Rng) -> Self {
        let mut b = Bits::<N>::zero();
        for i in 0..N {
            b.as_array_mut()[i] = rng.logic();
        }
        b
    }
    fn from_bits(bits: &[Logic]) -> Self {
        assert_eq!(bits.len(), N, "a Bits<{N}> is {N} bits");
        let mut b = Bits::<N>::zero();
        for i in 0..N {
            b.as_array_mut()[i] = bits[i];
        }
        b
    }
    fn as_bits(&self) -> Vec<Logic> {
        self.as_array().to_vec()
    }
}

impl<T: RandStim, const N: usize> RandStim for [T; N] {
    const WIDTH: usize = T::WIDTH * N;
    fn rand(rng: &mut Rng) -> Self {
        std::array::from_fn(|_| T::rand(rng))
    }
    fn from_bits(bits: &[Logic]) -> Self {
        assert_eq!(bits.len(), Self::WIDTH, "an [T; {N}] is {} bits", Self::WIDTH);
        std::array::from_fn(|i| T::from_bits(&bits[i * T::WIDTH..(i + 1) * T::WIDTH]))
    }
    /// Element-major, each element in its own bit order — the layout the hand-written
    /// array-port tests already record (`mux_equivalence.rs` flattens
    /// `data.iter().flat_map(|b| b.as_array())`), which is the array-port ABI in
    /// `design_docs/ARRAY_PORT_ABI.md`.
    fn as_bits(&self) -> Vec<Logic> {
        self.iter().flat_map(|e| e.as_bits()).collect()
    }
}

// ─── The corpus sweep's stimulus plan ────────────────────────────────────────

/// The constants of the sweep's multi-sequence stimulus policy
/// (`design_docs/CORPUS_DIFFERENTIAL_SWEEP.md` §3.2). They live in `build.rs` and
/// reach here through the generated `BUDGET`, so the generator and the plan cannot
/// disagree about them; the policy itself is [`StimulusPlan`], one function with
/// one pin (`tests/corpus_generated.rs::the_stimulus_policy_is_what_the_design_doc_says`).
#[derive(Clone, Copy, Debug)]
pub struct StimulusBudget {
    /// Independent random sequences per module; sequence 0 is the historical seed.
    pub seeds: usize,
    /// Cycles per seed sequence (clocked module).
    pub seed_cycles: usize,
    /// Cycles per first-cycle sequence (clocked module).
    pub first_cycles: usize,
    /// A clocked module with at most this many input bits (reset excluded) starts
    /// one sequence from EVERY initial input vector; a wider one from the pattern list.
    pub exhaustive_bits: usize,
    /// A combinational module with at most this many input bits is driven by its
    /// whole truth table instead of random vectors.
    pub comb_exhaustive_bits: usize,
    /// Random vectors per seed for a wider combinational module.
    pub comb_vectors: usize,
}

/// What one stimulus sequence is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SequenceKind {
    /// Random stimulus from `seed` on every cycle.
    Seed,
    /// The first live cycle drives `first` — one sequence per initial vector, for a
    /// module narrow enough to enumerate. Never dropped.
    Exhaustive,
    /// The first live cycle drives `first`, a vector from the fixed pattern list —
    /// dropped at run time if an earlier sequence already drove that vector on its
    /// first live cycle.
    Pattern,
    /// Combinational: cycle `c` drives `vectors[c]` (a truth table, or the pattern list).
    Vectors,
}

/// One stimulus sequence of a [`StimulusPlan`].
#[derive(Clone, Debug)]
pub struct Sequence {
    pub index: usize,
    pub kind: SequenceKind,
    /// A short name for a `Pattern` or `Vectors` sequence ("all ones", "truth table").
    pub name: String,
    pub seed: u64,
    pub cycles: usize,
    /// The flat input vector to drive on the first live cycle: every `In` port in
    /// signature order, the reset port excluded, each port LSB-first.
    pub first: Option<Vec<Logic>>,
    /// `Vectors` only: the vector for each cycle, in the same flat layout.
    pub vectors: Vec<Vec<Logic>>,
}

/// The sweep's stimulus for one module — the multi-sequence policy of
/// `design_docs/CORPUS_DIFFERENTIAL_SWEEP.md` §3.2, built at run time because a
/// width written as a file-scope `const` is not known to the generator.
///
/// A generated case drives it as a cursor: `while plan.next_sequence() { … for
/// cycle in 0..plan.cycles() { port_value = plan.stim::<T>(cycle, port, &mut rng); … } }`,
/// with a fresh executor per sequence and `EquivalenceTest::begin_sequence` so the
/// Verilated model restarts too. `stim` is called once per `In` port per cycle, in
/// signature order — the plan records the vector a sequence actually drove on its
/// first live cycle from those calls, which is what the pattern dedup is checked
/// against (what ran, not what the seed was predicted to produce).
pub struct StimulusPlan {
    module: String,
    /// `In` ports in signature order: name and width.
    ports: Vec<(String, usize)>,
    /// Each port's offset in the flat vector; `usize::MAX` for the reset port.
    offsets: Vec<usize>,
    /// The reset port's index among `ports` and its ASSERTED value.
    reset: Option<(usize, Logic)>,
    clocked: bool,
    /// Total input width, reset excluded — *W* in the design doc.
    width: usize,
    sequences: Vec<Sequence>,
    cursor: Option<usize>,
    /// First-live-cycle vectors the sequences run so far actually drove (as
    /// [`bits_key`]s — `Logic` is not `Ord`).
    covered: std::collections::BTreeSet<Vec<bool>>,
    /// The current sequence's first-live-cycle vector, assembled port by port.
    pending: Vec<Logic>,
    /// Run only this sequence (`COPPER_SWEEP_SEQUENCE`), for reproducing a failure alone.
    only: Option<usize>,
}

/// FNV-1a of `s`, forced odd — the same function as `build.rs`'s `seed_of`, which
/// derives sequence 0's seed; the later sequences' seeds are derived here from the
/// module name and the sequence's position.
fn fnv1a_seed(s: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in s.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x1000_0000_01b3);
    }
    h | 1
}

/// The environment variable that restricts a generated case to one sequence.
pub const SWEEP_SEQUENCE_ENV: &str = "COPPER_SWEEP_SEQUENCE";

/// A bit vector as a set key (`Logic` is not `Ord`; `X` counts as set).
pub fn bits_key(v: &[Logic]) -> Vec<bool> {
    v.iter().map(|b| *b != Logic::Zero).collect()
}

impl StimulusPlan {
    /// Build the plan for `module`. `base_seed` is the historical per-module seed
    /// (sequence 0 reproduces the single-sequence stimulus verbatim); `ports` are
    /// the `In` ports in signature order with their widths; `reset` is the index of
    /// the `RESET` port among them and whether it is active-low.
    pub fn new(
        module: &str,
        base_seed: u64,
        ports: &[(&str, usize)],
        clocked: bool,
        reset: Option<(usize, bool)>,
        budget: StimulusBudget,
    ) -> Self {
        let reset = reset.map(|(i, active_low)| {
            assert_eq!(ports[i].1, 1, "the reset port `{}` must be one bit wide", ports[i].0);
            (i, if active_low { Logic::Zero } else { Logic::One })
        });
        let mut offsets = Vec::with_capacity(ports.len());
        let mut width = 0;
        for (i, (_, w)) in ports.iter().enumerate() {
            if reset.is_some_and(|(r, _)| r == i) {
                offsets.push(usize::MAX);
            } else {
                offsets.push(width);
                width += w;
            }
        }
        let only = std::env::var(SWEEP_SEQUENCE_ENV).ok().map(|v| {
            v.trim().parse::<usize>().unwrap_or_else(|_| {
                panic!("{SWEEP_SEQUENCE_ENV}={v:?} is not a sequence index")
            })
        });
        let mut plan = StimulusPlan {
            module: module.to_string(),
            ports: ports.iter().map(|(n, w)| (n.to_string(), *w)).collect(),
            offsets,
            reset,
            clocked,
            width,
            sequences: Vec::new(),
            cursor: None,
            covered: Default::default(),
            pending: Vec::new(),
            only,
        };
        plan.sequences = plan.build(base_seed, budget);
        plan
    }

    /// Run only sequence `index` (`None`: all), as `COPPER_SWEEP_SEQUENCE` does.
    pub fn with_only(mut self, index: Option<usize>) -> Self {
        self.only = index;
        self
    }

    /// The policy, as sequences. Clocked: `seeds` random sequences, then one
    /// first-cycle sequence per initial vector (W ≤ `exhaustive_bits`) or per
    /// pattern-list entry; a module with no inputs has no first-cycle sequences.
    /// Combinational: the whole truth table (W ≤ `comb_exhaustive_bits`), else
    /// `seeds` random sequences of `comb_vectors` and the pattern list as vectors.
    fn build(&self, base_seed: u64, b: StimulusBudget) -> Vec<Sequence> {
        let w = self.width;
        let seed_k = |k: usize| if k == 0 { base_seed } else { fnv1a_seed(&format!("{}#{k}", self.module)) };
        let mut out: Vec<Sequence> = Vec::new();
        let seq = |index: usize, kind, name: &str, seed, cycles, first, vectors| Sequence {
            index,
            kind,
            name: name.to_string(),
            seed,
            cycles,
            first,
            vectors,
        };
        if self.clocked {
            for k in 0..b.seeds {
                out.push(seq(out.len(), SequenceKind::Seed, "seed", seed_k(k), b.seed_cycles, None, vec![]));
            }
            if w == 0 {
                return out;
            }
            if w <= b.exhaustive_bits {
                for v in 0..(1usize << w) {
                    let first = (0..w).map(|j| logic((v >> j) & 1 == 1)).collect::<Vec<_>>();
                    let seed = fnv1a_seed(&format!("{}#first{}", self.module, out.len()));
                    out.push(seq(out.len(), SequenceKind::Exhaustive, "first-cycle vector", seed, b.first_cycles, Some(first), vec![]));
                }
            } else {
                for (name, first) in self.pattern_list() {
                    let seed = fnv1a_seed(&format!("{}#first{}", self.module, out.len()));
                    out.push(seq(out.len(), SequenceKind::Pattern, &name, seed, b.first_cycles, Some(first), vec![]));
                }
            }
        } else if w <= b.comb_exhaustive_bits {
            let vectors: Vec<Vec<Logic>> = (0..(1usize << w))
                .map(|v| (0..w).map(|j| logic((v >> j) & 1 == 1)).collect())
                .collect();
            let n = vectors.len();
            out.push(seq(0, SequenceKind::Vectors, &format!("truth table (2^{w} vectors)"), base_seed, n, None, vectors));
        } else {
            for k in 0..b.seeds {
                out.push(seq(out.len(), SequenceKind::Seed, "seed", seed_k(k), b.comb_vectors, None, vec![]));
            }
            let vectors: Vec<Vec<Logic>> = self.pattern_list().into_iter().map(|(_, v)| v).collect();
            let n = vectors.len();
            out.push(seq(out.len(), SequenceKind::Vectors, "pattern list", base_seed, n, None, vectors));
        }
        out
    }

    /// The fixed first-cycle pattern list for a wide module: all zeros; all ones;
    /// per `In` port, that port all ones with the rest zero, and that port zero with
    /// the rest all ones. Deduplicated, order kept. Reset port excluded throughout.
    fn pattern_list(&self) -> Vec<(String, Vec<Logic>)> {
        let w = self.width;
        let mut out: Vec<(String, Vec<Logic>)> = Vec::new();
        let mut seen = std::collections::BTreeSet::new();
        let mut push = |name: String, v: Vec<Logic>| {
            if seen.insert(bits_key(&v)) {
                out.push((name, v));
            }
        };
        push("all zeros".into(), vec![Logic::Zero; w]);
        push("all ones".into(), vec![Logic::One; w]);
        for (i, (name, pw)) in self.ports.iter().enumerate() {
            let off = self.offsets[i];
            if off == usize::MAX {
                continue;
            }
            let mut only = vec![Logic::Zero; w];
            only[off..off + pw].fill(Logic::One);
            push(format!("only {name} ones"), only);
            let mut rest = vec![Logic::One; w];
            rest[off..off + pw].fill(Logic::Zero);
            push(format!("only {name} zeros"), rest);
        }
        out
    }

    /// The planned sequences (before run-time dedup).
    pub fn sequences(&self) -> &[Sequence] {
        &self.sequences
    }

    /// *W*: the total input width, reset port excluded.
    pub fn width(&self) -> usize {
        self.width
    }

    /// Cycles across every planned sequence (before run-time dedup).
    pub fn cycles_total(&self) -> usize {
        self.sequences.iter().map(|s| s.cycles).sum()
    }

    /// The cycle a first-cycle vector is driven on: 0, or 1 for a module whose cycle
    /// 0 is the asserted reset.
    pub fn first_live_cycle(&self) -> usize {
        usize::from(self.reset.is_some())
    }

    /// One line for the case's output, before any sequence runs.
    pub fn summary(&self) -> String {
        let tier = if !self.clocked {
            if self.sequences.len() == 1 { "whole truth table" } else { "random vectors + pattern list" }
        } else if self.width == 0 {
            "no inputs: seeds only"
        } else if self.sequences.iter().any(|s| s.kind == SequenceKind::Exhaustive) {
            "every initial vector"
        } else {
            "first-cycle pattern list"
        };
        format!(
            "stimulus plan for {}: {} sequences planned, {} input bits ({tier}); \
             re-run one alone with {SWEEP_SEQUENCE_ENV}=<index>",
            self.module,
            self.sequences.len(),
            self.width,
        )
    }

    /// Advance to the next sequence to run, printing its description; `false` when
    /// there is none. A `Pattern` sequence whose first-cycle vector an earlier
    /// sequence already drove is skipped, and says so.
    pub fn next_sequence(&mut self) -> bool {
        loop {
            let i = self.cursor.map_or(0, |c| c + 1);
            if i >= self.sequences.len() {
                self.cursor = Some(i);
                return false;
            }
            self.cursor = Some(i);
            self.pending.clear();
            if self.only.is_some_and(|o| o != i) {
                continue;
            }
            let s = &self.sequences[i];
            if s.kind == SequenceKind::Pattern
                && s.first.as_ref().is_some_and(|f| self.covered.contains(&bits_key(f)))
            {
                println!(
                    "sequence {i}/{}: first-cycle pattern {} ({}) skipped — an earlier sequence \
                     already drove that vector on its first live cycle",
                    self.sequences.len(),
                    s.name,
                    self.describe_vector(s.first.as_ref().unwrap())
                );
                continue;
            }
            println!("{}", self.describe(i));
            return true;
        }
    }

    fn describe(&self, i: usize) -> String {
        let s = &self.sequences[i];
        let n = self.sequences.len();
        match s.kind {
            SequenceKind::Seed => format!("sequence {i}/{n}: seed {:#018x}, {} cycles", s.seed, s.cycles),
            SequenceKind::Exhaustive | SequenceKind::Pattern => format!(
                "sequence {i}/{n}: {} {} on cycle {}, then seed {:#018x}, {} cycles",
                s.name,
                self.describe_vector(s.first.as_ref().unwrap()),
                self.first_live_cycle(),
                s.seed,
                s.cycles
            ),
            SequenceKind::Vectors => format!("sequence {i}/{n}: {}, {} vectors", s.name, s.cycles),
        }
    }

    /// `sel=1, data=0x3f` — the flat vector split back into its ports.
    pub fn describe_vector(&self, v: &[Logic]) -> String {
        let mut parts = Vec::new();
        for (i, (name, w)) in self.ports.iter().enumerate() {
            let off = self.offsets[i];
            if off == usize::MAX {
                continue;
            }
            let bits = &v[off..off + w];
            let text = if *w == 1 {
                (if bits[0] == Logic::One { "1" } else { "0" }).to_string()
            } else if *w <= 64 {
                let mut x = 0u64;
                for (j, b) in bits.iter().enumerate() {
                    if *b == Logic::One {
                        x |= 1 << j;
                    }
                }
                format!("{x:#x}")
            } else {
                format!("0b{}", bits.iter().rev().map(|b| if *b == Logic::One { '1' } else { '0' }).collect::<String>())
            };
            parts.push(format!("{name}={text}"));
        }
        parts.join(", ")
    }

    fn current(&self) -> &Sequence {
        &self.sequences[self.cursor.expect("next_sequence() first")]
    }

    /// The short label that names the current sequence in the testbench's FAIL lines.
    pub fn label(&self) -> String {
        let s = self.current();
        match s.kind {
            SequenceKind::Seed => format!("seed {:#018x}", s.seed),
            SequenceKind::Exhaustive => format!("first {}", self.describe_vector(s.first.as_ref().unwrap())),
            SequenceKind::Pattern => format!("pattern {}: {}", s.name, self.describe_vector(s.first.as_ref().unwrap())),
            SequenceKind::Vectors => s.name.clone(),
        }
    }

    pub fn index(&self) -> usize {
        self.current().index
    }

    pub fn seed(&self) -> u64 {
        self.current().seed
    }

    pub fn cycles(&self) -> usize {
        self.current().cycles
    }

    /// The value to drive on `port` (its index among the `In` ports) on `cycle` of
    /// the current sequence. The reset port is asserted on cycle 0, deasserted on
    /// the first live cycle of a first-cycle sequence, random otherwise (as before
    /// the policy); every other port takes the sequence's vector on its designated
    /// cycle and a random value otherwise.
    pub fn stim<T: RandStim>(&mut self, cycle: usize, port: usize, rng: &mut Rng) -> T {
        let (name, w) = &self.ports[port];
        assert_eq!(
            T::WIDTH, *w,
            "port `{name}` was declared {w} bits wide to the plan and is driven as {} bits",
            T::WIDTH
        );
        let first_live = self.first_live_cycle();
        let seq = self.current();
        if let Some((r, asserted)) = self.reset {
            if r == port {
                let bit = if cycle == 0 {
                    asserted
                } else if cycle == first_live && seq.first.is_some() {
                    !asserted
                } else {
                    rng.logic()
                };
                return T::from_bits(&[bit]);
            }
        }
        let off = self.offsets[port];
        let value: T = match (seq.kind, &seq.first) {
            (SequenceKind::Vectors, _) => T::from_bits(&seq.vectors[cycle][off..off + w]),
            (_, Some(first)) if cycle == first_live => T::from_bits(&first[off..off + w]),
            _ => T::rand(rng),
        };
        // Record what this sequence actually drove on its first live cycle, port by
        // port; the vector is complete after the last `In` port's call.
        if cycle == first_live && seq.kind != SequenceKind::Vectors && self.width > 0 {
            self.pending.extend(value.as_bits());
            if self.pending.len() == self.width {
                let v = std::mem::take(&mut self.pending);
                self.covered.insert(bits_key(&v));
            }
        }
        value
    }

    /// The first-live-cycle vectors driven so far, as [`bits_key`]s (for the policy's pin).
    pub fn covered(&self) -> &std::collections::BTreeSet<Vec<bool>> {
        &self.covered
    }
}

/// Transpile a DUT fixture and write the SystemVerilog to a temp file.
///
/// `module` may be `None` when the fixture declares exactly one hardware module.
///
/// The file is keyed on `(module, pid, counter)`, never on the module name alone —
/// the same rule as the Verilator work dirs. Thirteen module names occur twice in
/// the swept corpus (an `examples/` copy and a `tests/fixtures/` copy), both run in
/// the same test binary in parallel threads, and two of those pairs (`mac_pipeline`,
/// `ripple_carry_adder`) emit DIFFERENT SystemVerilog. A name-keyed path let one
/// case overwrite the other's SV between `for_module` and `finish`, so a test could
/// Verilate its twin's design against its own stimulus — a false-PASS mechanism as
/// well as a false-failure one.
pub fn transpile_fixture(module_name: &str, src: &str, module: Option<&str>) -> PathBuf {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static COUNTER: AtomicUsize = AtomicUsize::new(0);

    let sv = copper_codegen::transpile_source(src, module, &copper_codegen::EmitConfig::default())
        .unwrap_or_else(|e| panic!("transpiling '{module_name}' failed: {e}"));
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "copper_{module_name}_{}_{n}.sv",
        std::process::id()
    ));
    std::fs::write(&path, sv).expect("write generated sv");
    path
}

/// An equivalence check in progress: accumulates the simulator's actual trace
/// and the reference model's expected trace, then compares both and Verilates
/// the generated SystemVerilog against the actual trace.
pub struct EquivalenceTest {
    test: HardwareTest,
    expected: Vec<CycleData>,
    cycle: usize,
    /// The DUT source + selected module, retained so `finish` can run the G2
    /// structural register-inference check (`None` for `sim_only`, which has no src).
    src: Option<String>,
    module: Option<String>,
    /// A G2 reg-match request set by `with_reference_registers`: the path to an
    /// *independent hand-written* reference SV and the match mode.
    reg_check: Option<(String, copper_analysis::RegMatch)>,
    /// The module name, mirrored because `HardwareTest` keeps its own private and
    /// the reference leg needs to build a second one with the same Verilator top.
    name: String,
    /// A BEHAVIOURAL reference leg set by `with_hand_written_reference`: an
    /// independent Verilog implementation of the same behaviour, run over the same
    /// stimulus. `reg_check` above is the STRUCTURAL counterpart — it compares
    /// register sets, not traces — and the two are deliberately separate: a design
    /// can have the right flip-flops and the wrong behaviour, or the reverse.
    reference_sv: Option<String>,
    /// Mirrored because `with_params` folds them into `self.test` and the reference
    /// leg has to be Verilated at the same widths.
    params: Vec<(String, i64)>,
    /// Set by [`EquivalenceTest::differential_only`]: this run has no reference
    /// model, and `record_differential` feeds the simulator's own outputs in as the
    /// expected trace. Kept as a flag rather than left implicit so the two recording
    /// modes cannot be mixed — see that constructor for why.
    differential: bool,
    /// Stimulus sequences begun with [`begin_sequence`](Self::begin_sequence): the
    /// cycle each starts at and its label. Mirrored from `HardwareTest` because the
    /// reference leg replays the same boundaries, and the failure report names the
    /// sequence a mismatch fell in.
    sequences: Vec<(usize, String)>,
    /// Set by [`with_reference_interpreter`](Self::with_reference_interpreter): also
    /// replay the recorded stimulus through `copper_ref` — the THIRD leg
    /// (`design_docs/SPEC_OVERVIEW.md` §7), the spec made executable.
    reference_interpreter: bool,
    /// The transpiled SystemVerilog, kept so a differential run can Verilate the
    /// trace the agreement relation admits rather than the trace recorded.
    sv_path: Option<PathBuf>,
    /// Per sequence (parallel to `sequences`): observation 0, if recorded
    /// ([`record_observation_zero`](Self::record_observation_zero), SPEC §III.2.1).
    seq_obs0: Vec<Option<Vec<(String, Vec<Logic>)>>>,
    /// Per sequence: the simulator's run-time failure that ended it, if any
    /// ([`record_failure`](Self::record_failure), SPEC §III.2.4).
    seq_failure: Vec<Option<SimFailure>>,
    /// The module has a `RESET` row: its observation 0 is declared outside the SV
    /// pair (SPEC §III.2.2, "from reset").
    reset_row: bool,
    /// The sweep case's name, for the verdict ledger; `None` outside the sweep.
    case: Option<String>,
}

/// The simulator's run-time failure at one resumption (SPEC §III.2.4).
#[derive(Clone, Debug)]
pub struct SimFailure {
    /// Index into the recorded cycles at which the failing resumption's observation
    /// would have been recorded (the sequence's cycle count so far, from its start).
    pub at: usize,
    /// The inputs driven for that resumption — the reference replays it with them.
    pub inputs: Vec<(String, Vec<Logic>)>,
    pub message: String,
    pub kind: &'static str,
}

impl EquivalenceTest {
    /// Transpile `src` and begin an equivalence check for `module_name`.
    /// `module_name` must match the DUT's function name (Verilator's top module).
    pub fn new(module_name: &str, src: &str) -> Self {
        Self::for_module(module_name, src, None)
    }

    /// Like `new`, but selects one module from a multi-module fixture.
    pub fn for_module(module_name: &str, src: &str, module: Option<&str>) -> Self {
        let sv_path = transpile_fixture(module_name, src, module);
        EquivalenceTest {
            test: HardwareTest::new(module_name).with_verilog(
                sv_path.to_str().expect("temp path is valid UTF-8"),
            ),
            sv_path: Some(sv_path.clone()),
            seq_obs0: Vec::new(),
            seq_failure: Vec::new(),
            reset_row: false,
            case: None,
            expected: Vec::new(),
            cycle: 0,
            src: Some(src.to_string()),
            module: module.map(str::to_string),
            reg_check: None,
            name: module_name.to_string(),
            reference_sv: None,
            params: Vec::new(),
            differential: false,
            sequences: Vec::new(),
            reference_interpreter: false,
        }
    }

    /// Also structurally reg-match the DUT's **inferred** register set (from the
    /// shared control/liveness analysis) against an *independent hand-written*
    /// reference SV's sequential flip-flops (G2 of the impl plan). `mode` is
    /// `NameExact` for a faithful translation that mirrors the design's names,
    /// `StorageEquivalent` (count) for a genuinely independent reference. Asserted
    /// in `finish`; the reference must NOT be the transpiler's own output (circular).
    pub fn with_reference_registers(
        mut self,
        reference_sv_path: &str,
        mode: copper_analysis::RegMatch,
    ) -> Self {
        self.reg_check = Some((reference_sv_path.to_string(), mode));
        self
    }

    /// Override the transpiled module's SystemVerilog parameters for the Verilator
    /// cross-check (e.g. `&[("N", 8), ("N_1", 7)]`) — the same widths the
    /// simulator ran the parametric DUT at.
    pub fn with_params(mut self, params: &[(&str, i64)]) -> Self {
        self.test = self.test.with_params(params);
        self.params = params.iter().map(|(k, v)| (k.to_string(), *v)).collect();
        self
    }

    /// Also run the recorded stimulus against an **independent hand-written Verilog
    /// implementation of the same behaviour** and require it to agree.
    ///
    /// # What this adds over the differential check
    ///
    /// The sweep's ordinary check is simulator vs the SystemVerilog the transpiler
    /// emitted — two implementations of one source, which is an oracle for *"the two
    /// agree"* and nothing more. Both can be wrong together, and a shared
    /// misconception between the executor and the lowering is exactly the failure it
    /// cannot see. A third implementation nobody derived from the other two closes
    /// that: the reference is what a hardware engineer would have written for this
    /// behaviour, so agreement means the semantics are right, not merely consistent.
    ///
    /// # What it does NOT add
    ///
    /// A reference written by whoever wrote the Copper module shares that person's
    /// model of the design. It catches lowering and transcription errors; it does
    /// not catch a misconception both files share. Only a genuinely third-party
    /// source (BaseJump STL and the like) buys that, which is why each reference
    /// file states its provenance in its header and why third-party is preferred
    /// wherever one exists.
    ///
    /// The reference's SystemVerilog module must be named the same as the Copper
    /// module — it is Verilated as the top, exactly as the transpiler's output is.
    pub fn with_hand_written_reference(mut self, reference_sv_path: &str) -> Self {
        self.reference_sv = Some(reference_sv_path.to_string());
        self
    }

    /// Also replay the recorded stimulus through the **reference interpreter**
    /// (`copper_ref`): the kernel run under SPEC §I.0's one-resumption protocol
    /// without rustc's lowering or the executor. The simulator's recorded trace is
    /// compared cycle for cycle with what the interpreter observes after each edge.
    ///
    /// How a disagreement reads (`SPEC_OVERVIEW.md` §7, `copper-ref/README.md`):
    /// the interpreter is the spec, so sim ≠ ref with SV = ref is a simulator or
    /// macro bug; sim = ref ≠ SV is a transpiler bug or a II.3 row; all three
    /// different is a spec gap or an interpreter bug. All legs run before any
    /// fails, so the report names which.
    ///
    /// A module the interpreter defers (a plain `Out`, a memory, a second clock)
    /// is a FAILURE here, not a skip: `build.rs`'s `REF_SKIP` table is where such a
    /// module is excused, with a sentence, and the generated case then does not
    /// call this.
    pub fn with_reference_interpreter(mut self) -> Self {
        self.reference_interpreter = true;
        self
    }

    /// Like `new`, but checks the simulator against the reference model only —
    /// no transpilation, no Verilator cross-check. Use this for DUTs where the
    /// transpiler is known not to agree with the simulator yet (see
    /// TRANSPILATION_TODO.md, "`pre_edge_barrier` invisible to phase
    /// extraction"), so the frontend/simulation semantics can still be
    /// verified independently of that gap.
    pub fn sim_only(module_name: &str) -> Self {
        EquivalenceTest {
            test: HardwareTest::new(module_name),
            sv_path: None,
            seq_obs0: Vec::new(),
            seq_failure: Vec::new(),
            reset_row: false,
            case: None,
            expected: Vec::new(),
            cycle: 0,
            src: None,
            module: None,
            reg_check: None,
            name: module_name.to_string(),
            reference_sv: None,
            params: Vec::new(),
            differential: false,
            sequences: Vec::new(),
            reference_interpreter: false,
        }
    }

    /// Begin a **differential-only** check: simulator vs the transpiled
    /// SystemVerilog under Verilator, with **no reference model**.
    ///
    /// The two are independent implementations of the same source, so comparing
    /// them is already an oracle — a divergence means one of them is wrong. What it
    /// cannot see is the case where both are wrong the same way, which is what a
    /// reference model (and, better, the independent Verilog in
    /// `examples/basejump/`) is for. Use this where no model exists; do not use it
    /// to retire one that does.
    ///
    /// Recording is a separate method ([`record_differential`](Self::record_differential))
    /// and the two are mutually exclusive by assertion. Feeding the simulator's own
    /// outputs back as the "expected" trace is exactly right here and a silent
    /// disaster anywhere else — a model that has quietly become a copy of the thing
    /// it is meant to check always passes. It must not be reachable by accident.
    pub fn differential_only(module_name: &str, src: &str, module: Option<&str>) -> Self {
        let mut eq = Self::for_module(module_name, src, module);
        eq.differential = true;
        eq
    }

    /// Begin a new stimulus sequence (see `HardwareTest::begin_sequence`): the cycles
    /// recorded from here on start from the design's initial state on both sides.
    /// The caller restarts the simulator (a fresh executor and wires); the Verilated
    /// model and the reference leg restart at the same cycle.
    pub fn begin_sequence(&mut self, label: &str) {
        self.test.begin_sequence(label);
        self.sequences.push((self.cycle, label.to_string()));
        self.seq_obs0.push(None);
        self.seq_failure.push(None);
    }

    /// Name this run as the sweep case `case` (`<wrapper>::<module>`): its verdicts
    /// go to the ledger `tools/regression.sh` counts (G-G).
    pub fn sweep_case(mut self, case: &str) -> Self {
        self.case = Some(case.to_string());
        self
    }

    /// The module has a `RESET` row (build.rs): observation 0 is outside the SV pair
    /// by the row's declaration (SPEC §III.2.2 — the relation covers `obs_k`, k ≥ 1),
    /// and is still compared between the simulator and the reference.
    pub fn with_reset_row(mut self) -> Self {
        self.reset_row = true;
        self
    }

    /// Record observation 0 of the current sequence (SPEC §I.1.4, §III.2.1): every
    /// output's value after the first resumption's settle (`pre_edge_settle`) and
    /// before the first edge. Call it once, before the sequence's first `record_*`.
    pub fn record_observation_zero(&mut self, outputs: &[(&str, &[Logic])]) {
        assert!(self.differential, "observation 0 is recorded by the differential harness only");
        if self.sequences.is_empty() && self.cycle == 0 {
            // A single-sequence run (a hand-driven measurement): its one sequence.
            self.begin_sequence("");
        }
        let k = self.sequences.len().checked_sub(1).expect("begin_sequence before record_observation_zero");
        assert_eq!(self.sequences[k].0, self.cycle, "observation 0 precedes the sequence's first recorded cycle");
        assert!(self.seq_obs0[k].is_none(), "observation 0 recorded twice");
        self.seq_obs0[k] = Some(outputs.iter().map(|(n, v)| (n.to_string(), v.to_vec())).collect());
    }

    /// The simulator failed at this cycle's resumption (a panic caught by
    /// [`run_step`]): the sequence's trace ends here (SPEC §III.2.4). `inputs` are
    /// the values driven for the failing resumption. A panic that is NOT one of
    /// III.2.4's run-time failures is not truncation — it fails the case here.
    pub fn record_failure(&mut self, inputs: &[(&str, &[Logic])], message: &str) {
        let Some(kind) = runtime_failure_kind(message) else {
            panic!(
                "the simulator panicked at recorded cycle {} of '{}', and not with a run-time \
                 failure of SPEC §III.2.4 (an undelivered data(), an out-of-range address, an X \
                 fed to a condition or an as_* view, integer overflow) — this is a failure, not a \
                 truncation:\n{message}",
                self.cycle, self.name
            );
        };
        let k = self.sequences.len().checked_sub(1).expect("begin_sequence before record_failure");
        assert!(self.seq_failure[k].is_none(), "a sequence fails at most once");
        self.seq_failure[k] = Some(SimFailure {
            at: self.cycle,
            inputs: inputs.iter().map(|(n, v)| (n.to_string(), v.to_vec())).collect(),
            message: message.to_string(),
            kind,
        });
    }

    /// Record one cycle of a [`differential_only`](Self::differential_only) run: the
    /// inputs driven and the outputs the simulator produced. The simulator's own
    /// outputs become the expected trace, so the trace comparison is trivially
    /// satisfied and the check that carries the weight is the Verilator one.
    pub fn record_differential(&mut self, inputs: &[(&str, &[Logic])], actual: &[(&str, &[Logic])]) {
        assert!(
            self.differential,
            "record_differential on a modelled EquivalenceTest — it would replace the reference \
             model with a copy of the simulator, and the test would pass by construction"
        );
        self.record(inputs, actual, actual);
    }

    /// Record one cycle: the inputs driven, the outputs the simulator actually
    /// produced, and the outputs the reference model predicts. Cycle numbering
    /// is automatic.
    pub fn record(
        &mut self,
        inputs: &[(&str, &[Logic])],
        actual: &[(&str, &[Logic])],
        expected: &[(&str, &[Logic])],
    ) {
        assert!(
            !self.differential || std::ptr::eq(actual, expected),
            "record with a reference model on a differential_only run — pick one: a model, or \
             the simulator standing in for one"
        );
        let i = self.cycle;
        self.test.record_cycle(i, inputs, actual);
        self.expected.push(make_cycle(i, inputs, expected));
        self.cycle += 1;
    }

    /// Assert the simulator matched the reference model, that the generated
    /// SystemVerilog matches the simulator under Verilator, and — if
    /// `with_reference_registers` was set — that the inferred register set
    /// structurally matches the independent reference SV (G2).
    pub fn finish(self) {
        let v = self.finish_report();
        if !v.all_ok() {
            panic!("{}", v.report);
        }
    }

    /// `finish` without the panic: every leg's verdict, for a measurement that
    /// predicts a disagreement and wants to read it rather than fail on it.
    pub fn finish_report(self) -> Verdicts {
        if self.differential {
            return self.finish_relation();
        }
        if let Some((ref_sv_path, mode)) = &self.reg_check {
            let src = self
                .src
                .as_deref()
                .expect("with_reference_registers requires a DUT source (not sim_only)");
            let sv = std::fs::read_to_string(ref_sv_path)
                .unwrap_or_else(|e| panic!("read reference SV {ref_sv_path}: {e}"));
            copper_analysis::assert_source_registers_match_reference_sv(
                src,
                self.module.as_deref(),
                &sv,
                *mode,
            );
        }
        // BOTH LEGS RUN BEFORE EITHER IS ALLOWED TO FAIL. Asserting the transpiled
        // leg first would hide the reference result whenever both are wrong, which is
        // exactly the case a new fixture-plus-reference pair is most likely to be in —
        // two round trips to learn what one run already knew.
        //
        // Keep a copy: the transpiled leg consumes the trace, and the reference leg
        // must be driven with the SAME stimulus and held to the SAME expectations.
        let replay = self.expected.clone();
        let sequences = self.sequences.clone();

        // The reference-interpreter leg FIRST: it needs no Verilator and its
        // verdict is what the other two are read against. Never asserted here.
        let interpreter = if self.reference_interpreter {
            let src = self
                .src
                .as_deref()
                .expect("with_reference_interpreter requires a DUT source (not sim_only)");
            let module = self.module.clone().unwrap_or_else(|| self.name.clone());
            let ps: Vec<(&str, i64)> = self.params.iter().map(|(k, v)| (k.as_str(), *v)).collect();
            let mut leg = ReferenceLeg::new(&self.name, src, &module, &ps);
            for (i, c) in replay.iter().enumerate() {
                for (_, label) in sequences.iter().filter(|(start, _)| *start == i) {
                    leg.begin_sequence(label);
                }
                let ins: Vec<(&str, &[Logic])> =
                    c.inputs.iter().map(|(n, v)| (n.as_str(), &v[..])).collect();
                let outs: Vec<(&str, &[Logic])> =
                    c.outputs.iter().map(|(n, v)| (n.as_str(), &v[..])).collect();
                leg.record(&ins, &outs);
            }
            let v = leg.run();
            println!("{}", v.summary_line());
            Some(v)
        } else {
            None
        };

        let expected = SimulationTrace::from_cycles(self.expected);
        let transpiled = self.test.finish_with_expected(&expected);
        transpiled.print_summary();

        let reference = self.reference_sv.as_ref().map(|ref_path| {
            println!("=== {} vs INDEPENDENT reference {ref_path} ===", self.name);
            let mut ref_test = HardwareTest::new(&self.name).with_verilog(ref_path);
            if !self.params.is_empty() {
                let ps: Vec<(&str, i64)> =
                    self.params.iter().map(|(k, v)| (k.as_str(), *v)).collect();
                ref_test = ref_test.with_params(&ps);
            }
            for (i, c) in replay.iter().enumerate() {
                // The same sequence boundaries as the transpiled leg: the reference
                // must restart where the simulator did, or it is held to another
                // sequence's expectations from its own continuing state.
                for (_, label) in sequences.iter().filter(|(start, _)| *start == i) {
                    ref_test.begin_sequence(label);
                }
                let ins: Vec<(&str, &[Logic])> =
                    c.inputs.iter().map(|(n, v)| (n.as_str(), &v[..])).collect();
                let outs: Vec<(&str, &[Logic])> =
                    c.outputs.iter().map(|(n, v)| (n.as_str(), &v[..])).collect();
                ref_test.record_cycle(c.cycle, &ins, &outs);
            }
            let expected_ref = SimulationTrace::from_cycles(replay);
            let r = ref_test.finish_with_expected(&expected_ref);
            r.print_summary();
            r
        });

        let ref_failed = reference.as_ref().is_some_and(|r| !r.passed());
        let interp_failed = interpreter.as_ref().is_some_and(|v| !v.passed());
        let mut verdicts = Verdicts {
            transpiled: transpiled.passed(),
            reference_sv: reference.as_ref().map(|r| r.passed()),
            interpreter: interpreter.as_ref().map(|v| v.verdict()),
            report: String::new(),
            pattern: None,
            sequences: Vec::new(),
            name: self.name.clone(),
            failed_hard: false,
        };
        if transpiled.passed() && !ref_failed && !interp_failed {
            return verdicts;
        }

        // WHICH legs failed is the diagnosis, so say it rather than making the reader
        // infer it from two summaries. The transpiled leg alone means the transpiler
        // disagrees with the simulator. The REFERENCE leg alone is the finding this
        // whole mechanism exists for: the simulator and the transpiler agree with each
        // other and are both wrong, which no amount of differential testing can see.
        let mut report = String::new();
        let verdict = match (transpiled.passed(), ref_failed) {
            (false, false) => "the TRANSPILED SystemVerilog disagrees with the simulator",
            (true, true) => {
                "the simulator and the transpiled SystemVerilog AGREE WITH EACH OTHER and \
                 both disagree with the independent reference — a shared misconception, \
                 which is the case the differential check alone cannot see"
            }
            (false, true) => "both legs failed: the transpiler disagrees with the simulator, \
                              AND both disagree with the independent reference",
            // Reachable only through the interpreter leg (the two Verilator legs
            // passed): its reading follows.
            (true, false) => "the transpiled SystemVerilog agrees with the simulator and the \
                              REFERENCE INTERPRETER disagrees",
        };
        report.push_str(&format!("equivalence failed for '{}': {verdict}\n", self.name));
        if let Some(v) = interpreter.as_ref() {
            // The three-way reading, with the interpreter as the spec (§7).
            let reading = match (transpiled.passed(), v.passed()) {
                (true, false) => {
                    "the simulator and the transpiled SystemVerilog AGREE WITH EACH OTHER and the \
                     REFERENCE INTERPRETER disagrees: both implementations share a reading the \
                     spec does not give — a simulator/macro bug (SPEC_OVERVIEW.md §7), or a Phase C \
                     obligation not yet landed (the two-job residue), or a spec gap"
                }
                (false, true) => {
                    "the simulator and the REFERENCE INTERPRETER agree and the transpiled \
                     SystemVerilog disagrees: a transpiler bug, or a II.3 row"
                }
                (false, false) => {
                    "all three differ: a spec gap or an interpreter bug (read the interpreter's \
                     first disagreement first)"
                }
                (true, true) => "the interpreter leg passed",
            };
            report.push_str(&format!("reference interpreter: {reading}\n"));
        }
        if !transpiled.passed() {
            report.push_str("\n-- simulator vs transpiled SystemVerilog --\n");
            report.push_str(&errors_by_sequence(&sequences, &transpiled.errors));
        }
        if let Some(r) = reference.filter(|r| !r.passed()) {
            report.push_str("\n-- simulator vs independent reference --\n");
            report.push_str(&errors_by_sequence(&sequences, &r.errors));
        }
        if let Some(v) = interpreter.filter(|v| !v.passed()) {
            report.push_str("\n-- simulator vs REFERENCE INTERPRETER (copper-ref) --\n");
            report.push_str(&v.detail());
        }
        verdicts.report = report;
        verdicts
    }
}

// ── The agreement relation (SPEC §III.2) on a differential run ──────────────

/// A sequence's span in the recorded cycles.
struct Span {
    start: usize,
    end: usize,
    label: String,
}

/// Parse a relation testbench's FAIL lines: `(cycle, port, at obs_0) → the value
/// Verilator showed`.
/// The value is decimal, or `0x` and hex for a port wider than 64 bits (`wide_check`).
fn sv_values(output: &str) -> std::collections::HashMap<(usize, String, bool), u128> {
    let mut m = std::collections::HashMap::new();
    for l in output.lines().filter(|l| l.starts_with("FAIL: Cycle ")) {
        let t: Vec<&str> = l.split_whitespace().collect();
        // FAIL: Cycle <c> <port>[@obs0] expected <e> got <g> …
        if t.len() < 8 || t[4] != "expected" || t[6] != "got" {
            continue;
        }
        let g = match t[7].strip_prefix("0x") {
            Some(h) => u128::from_str_radix(h, 16),
            None => t[7].parse::<u128>(),
        };
        let (Ok(c), Ok(g)) = (t[2].parse::<usize>(), g) else { continue };
        let (port, obs0) = match t[3].strip_suffix("@obs0") {
            Some(p) => (p.to_string(), true),
            None => (t[3].to_string(), false),
        };
        m.insert((c, port, obs0), g);
    }
    m
}

impl EquivalenceTest {
    fn spans(&self) -> Vec<Span> {
        if self.sequences.is_empty() {
            return vec![Span { start: 0, end: self.expected.len(), label: String::new() }];
        }
        self.sequences
            .iter()
            .enumerate()
            .map(|(k, (start, label))| Span {
                start: *start,
                end: self.sequences.get(k + 1).map_or(self.expected.len(), |(s, _)| *s),
                label: label.clone(),
            })
            .collect()
    }

    /// The differential run under the relation: observation 0 compared (III.2.1),
    /// an X observation taking the SV pair OUTSIDE (III.2.2), a run-time failure
    /// TRUNCATING the trace with the simulator and the reference required to fail
    /// at the same resumption (III.2.4), and every disagreement read by its pattern
    /// (III.2.8). All legs run before any verdict is drawn.
    fn finish_relation(self) -> Verdicts {
        let spans = self.spans();
        let marked = !self.sequences.is_empty();
        let obs0_of = |k: usize| self.seq_obs0.get(k).cloned().flatten();
        let failure_of = |k: usize| self.seq_failure.get(k).cloned().flatten();

        // 1. The SV trace the relation admits, and each sequence's OUTSIDE cut.
        let mut outcomes: Vec<SeqOutcome> = Vec::new();
        let mut cuts: Vec<usize> = Vec::new();
        let mut obs0_compared: Vec<bool> = Vec::new();
        let mut sv_trace = SimulationTrace::new();
        sv_trace.relation = true;
        for (k, sp) in spans.iter().enumerate() {
            let mut o = SeqOutcome { index: k, label: sp.label.clone(), ..Default::default() };
            let mut cut = sp.end;
            let mut obs0_sv = None;
            if let Some(ob) = obs0_of(k) {
                if self.reset_row {
                    o.obs0_declared_out = true;
                } else if let Some((port, _)) = ob.iter().find(|(_, v)| v.contains(&Logic::X)) {
                    o.outside = Some((sp.start, None, port.clone()));
                    cut = sp.start;
                } else {
                    obs0_sv = Some(ob);
                }
            }
            if o.outside.is_none() {
                for p in sp.start..sp.end {
                    if let Some((port, _)) = self.expected[p].outputs.iter().find(|(_, v)| v.contains(&Logic::X)) {
                        o.outside = Some((p, Some(p - sp.start), port.clone()));
                        cut = p;
                        break;
                    }
                }
            }
            let mut cycles: Vec<CycleData> = self.expected[sp.start..cut].to_vec();
            if cycles.is_empty() && obs0_sv.is_some() {
                // Observation 0 alone: the testbench checks it after the first
                // drive and settle, so it needs that cycle's inputs and nothing else.
                let inputs = if sp.start < sp.end {
                    Some(self.expected[sp.start].inputs.clone())
                } else {
                    failure_of(k).map(|f| f.inputs)
                };
                match inputs {
                    Some(inputs) => cycles.push(CycleData { cycle: sp.start, inputs, outputs: Vec::new() }),
                    None => obs0_sv = None,
                }
            }
            obs0_compared.push(obs0_sv.is_some());
            if !cycles.is_empty() {
                if marked || obs0_sv.is_some() {
                    sv_trace.begin_sequence(&sp.label);
                    sv_trace.sequences.last_mut().expect("just pushed").obs0 = obs0_sv;
                }
                sv_trace.cycles.extend(cycles);
            }
            cuts.push(cut);
            outcomes.push(o);
        }

        // 2. The reference interpreter (no Verilator), over the full recorded trace.
        let interpreter = if self.reference_interpreter {
            let src = self.src.as_deref().expect("with_reference_interpreter requires a DUT source");
            let module = self.module.clone().unwrap_or_else(|| self.name.clone());
            let ps: Vec<(&str, i64)> = self.params.iter().map(|(k, v)| (k.as_str(), *v)).collect();
            let mut leg = ReferenceLeg::new(&self.name, src, &module, &ps);
            for (k, sp) in spans.iter().enumerate() {
                leg.begin_sequence(&sp.label);
                if let Some(ob) = obs0_of(k) {
                    let outs: Vec<(&str, &[Logic])> = ob.iter().map(|(n, v)| (n.as_str(), &v[..])).collect();
                    leg.record_observation_zero(&outs);
                }
                for c in &self.expected[sp.start..sp.end] {
                    let ins: Vec<(&str, &[Logic])> = c.inputs.iter().map(|(n, v)| (n.as_str(), &v[..])).collect();
                    let outs: Vec<(&str, &[Logic])> = c.outputs.iter().map(|(n, v)| (n.as_str(), &v[..])).collect();
                    leg.record(&ins, &outs);
                }
                if let Some(f) = failure_of(k) {
                    let ins: Vec<(&str, &[Logic])> = f.inputs.iter().map(|(n, v)| (n.as_str(), &v[..])).collect();
                    leg.record_failure(&ins, &f.message);
                }
            }
            let v = leg.run();
            println!("{}", v.summary_line());
            Some(v)
        } else {
            None
        };

        // 3. The transpiled SV (and the hand-written reference) under Verilator.
        let policy = copper_sim::VerilatorCachePolicy::from_env();
        let run_sv = |path: &str| -> Result<Option<copper_sim::verification::VerilatorRun>, String> {
            if sv_trace.cycles.is_empty() {
                println!("  [verilator] no observation of '{}' is inside the SV pair — not run", self.name);
                return Ok(None);
            }
            match copper_sim::verify_with_verilator_report(path, &self.name, &sv_trace, None, &self.params, policy) {
                Ok(r) => Ok(Some(r)),
                Err(e) if copper_sim::is_missing_verilator(&e) => {
                    println!("  [verilator] not available: {e}");
                    Ok(None)
                }
                Err(e) => Err(e),
            }
        };
        let mut hard: Vec<String> = Vec::new();
        let sv_run = match self.sv_path.as_ref().map(|p| run_sv(p.to_str().expect("utf-8 temp path"))) {
            Some(Ok(r)) => r,
            Some(Err(e)) => {
                hard.push(format!("Verilator error (transpiled SV): {e}"));
                None
            }
            None => None,
        };
        let sv_runs = sv_run.is_some();
        let got = sv_run.as_ref().map(|r| sv_values(&r.output)).unwrap_or_default();
        let reference = self.reference_sv.as_ref().map(|path| {
            println!("=== {} vs INDEPENDENT reference {path} ===", self.name);
            match run_sv(path) {
                Ok(Some(r)) => (r.passed, r.output),
                Ok(None) => (true, String::new()),
                Err(e) => (false, format!("Verilator error (independent reference): {e}")),
            }
        });

        // 4. Each sequence's verdict.
        let mut sv_unexcused = false;
        let excused_any = false;
        for (k, sp) in spans.iter().enumerate() {
            let o = &mut outcomes[k];
            let cut = cuts[k];
            let rseq = interpreter.as_ref().and_then(|v| v.seqs.get(k));
            let mut first: Option<(Pattern, String, bool)> = None; // (pattern, where, at obs_0)
            // SV mismatches, wherever they fall: (the ports at observation 0, later).
            let mut sv_bad: (Vec<String>, bool) = (Vec::new(), false);
            let observe = |obs: &[(String, Vec<Logic>)], at: Option<usize>, compared: bool, first: &mut Option<(Pattern, String, bool)>, sv_bad: &mut (Vec<String>, bool)| {
                for (port, s) in obs {
                    let r = match at {
                        None => rseq.and_then(|q| q.obs0.get(port)),
                        Some(p) => interpreter.as_ref().and_then(|v| v.values.get(&(p, port.clone()))),
                    };
                    let v = (sv_runs && compared).then(|| {
                        let key = match at {
                            None => (sp.start, port.clone(), true),
                            Some(p) => (self.expected[p].cycle, port.clone(), false),
                        };
                        got.get(&key).copied().or_else(|| to_int(s)).unwrap_or(0)
                    });
                    if v.is_some_and(|v| Some(v) != to_int(s)) {
                        if at.is_none() {
                            sv_bad.0.push(port.clone());
                        } else {
                            sv_bad.1 = true;
                        }
                    }
                    if let Some(pat) = Pattern::of(s, r.map(|r| &r[..]), v, !compared, sv_runs) {
                        if first.is_none() {
                            let wher = match at {
                                None => "observation 0".to_string(),
                                Some(p) => format!("cycle {} (recorded cycle {p})", p - sp.start),
                            };
                            let show = |x: Option<String>| x.unwrap_or_else(|| "—".to_string());
                            *first = Some((
                                pat,
                                format!(
                                    "at {wher}, port {port}: sim {} ref {} SV {}",
                                    fmt_bits(s),
                                    show(r.map(|r| fmt_bits(r))),
                                    show(v.map(|v| v.to_string()))
                                ),
                                at.is_none(),
                            ));
                        }
                    }
                }
            };
            if let Some(ob) = obs0_of(k) {
                observe(&ob, None, obs0_compared[k], &mut first, &mut sv_bad);
            }
            for p in sp.start..sp.end {
                let outs = self.expected[p].outputs.clone();
                observe(&outs, Some(p), p < cut, &mut first, &mut sv_bad);
            }
            // Run-time failures: the simulator's, and whether the reference agreed.
            let sim_fail = failure_of(k);
            match (rseq, &sim_fail) {
                (Some(q), _) if q.failure_mismatch.is_some() && first.is_none() => {
                    first = Some((Pattern::SimNeRefSvOutside, q.failure_mismatch.clone().unwrap(), false));
                }
                (Some(q), Some(f)) if q.truncated_agree => {
                    o.truncated = Some((f.at - sp.start + 1, format!("{} — {}", f.kind, f.message)));
                }
                (None, Some(f)) => {
                    o.truncated = Some((
                        f.at - sp.start + 1,
                        format!("{} — {} (reference leg not run)", f.kind, f.message),
                    ));
                }
                _ => {}
            }
            // (The observation-0 plain-`Out` excuse class of Phase C step 1 is gone:
            // it emptied with the D-8 rewrites and was deleted at the flip, step 3.)
            if let Some((pat, wher, _at0)) = first {
                o.disagree = Some((pat, wher));
            }
            if sv_bad.1 || !sv_bad.0.is_empty() {
                sv_unexcused = true;
            }
        }
        // An SV mismatch the per-observation walk did not attribute (it cannot
        // happen if the walk and the testbench agree on what was compared) is a
        // harness bug, never a pass.
        let sv_failed_lines = sv_run.as_ref().is_some_and(|r| !r.passed);
        if sv_failed_lines != (sv_unexcused || excused_any) {
            hard.push("the testbench's verdict and the relation walk's disagree about whether the SV matched — a harness bug".into());
        }

        let pattern = outcomes.iter().find_map(|o| o.disagree.as_ref().map(|(p, _)| *p));
        let interp_ok = interpreter.as_ref().map(|v| v.verdict());
        let interp_failed = interpreter.as_ref().is_some_and(|v| !v.passed());
        let mut verdicts = Verdicts {
            transpiled: !sv_unexcused && hard.iter().all(|h| !h.contains("transpiled SV")),
            reference_sv: reference.as_ref().map(|(ok, _)| *ok),
            interpreter: interp_ok,
            report: String::new(),
            pattern,
            sequences: outcomes,
            name: self.name.clone(),
            failed_hard: !hard.is_empty(),
        };
        let summary = format!("agreement relation: {} sequence(s): {} [{}]", verdicts.sequences.len(), verdicts.counts(), self.name);
        println!("{summary}");
        for o in verdicts.sequences.iter().filter(|o| o.verdict() != "PASS" || o.obs0_declared_out) {
            println!("  {}", o.line(&self.name));
        }
        if let Some(case) = &self.case {
            let mut lines: Vec<String> = verdicts.sequences.iter().map(|o| o.line(case)).collect();
            lines.push(format!(
                "CASE\t{case}\t{}\tpattern={}",
                verdicts.counts(),
                pattern.map_or("none", Pattern::label)
            ));
            ledger_write("sweep-differential", case, &lines);
        }
        if verdicts.all_ok() {
            return verdicts;
        }

        let mut report = String::new();
        report.push_str(&format!("equivalence failed for '{}': {summary}\n", self.name));
        for h in &hard {
            report.push_str(&format!("{h}\n"));
        }
        if let Some(p) = pattern {
            report.push_str(&format!("first disagreement: pattern {} — {}\n", p.label(), p.reading()));
        }
        for o in verdicts.sequences.iter().filter(|o| o.disagree.is_some()) {
            report.push_str(&format!("  {}\n", o.line(&self.name)));
        }
        let seqs = self.sequences.clone();
        if let Some(r) = sv_run.as_ref().filter(|r| !r.passed) {
            report.push_str("\n-- simulator vs transpiled SystemVerilog --\n");
            report.push_str(&errors_by_sequence(&seqs, &[format!("Simulation failed:\n{}", r.output)]));
        }
        if let Some((false, out)) = &reference {
            report.push_str("\n-- simulator vs independent reference --\n");
            report.push_str(&errors_by_sequence(&seqs, &[out.clone()]));
            report.push_str(
                "(the hand-written reference is anchoring, not a leg of the relation: a \
                 disagreement with it is a shared misconception or a reference to fix)\n",
            );
        }
        if let Some(v) = interpreter.filter(|_| interp_failed) {
            report.push_str("\n-- simulator vs REFERENCE INTERPRETER (copper-ref) --\n");
            report.push_str(&v.detail());
        }
        verdicts.report = report;
        verdicts
    }
}

/// Every leg's verdict from [`EquivalenceTest::finish_report`].
pub struct Verdicts {
    /// The transpiled SystemVerilog under Verilator agreed with the simulator.
    pub transpiled: bool,
    /// The independent hand-written reference agreed, if one was set.
    pub reference_sv: Option<bool>,
    /// The reference interpreter agreed (`Ok(cycles)`), if it was set; `Err` is the
    /// first disagreement or refusal.
    pub interpreter: Option<Result<usize, String>>,
    /// The failure report `finish` would have panicked with (empty when all ok).
    pub report: String,
    /// The first disagreement's pattern (SPEC §III.2.8), on a differential run.
    pub pattern: Option<Pattern>,
    /// Every sequence's outcome under the relation, on a differential run.
    pub sequences: Vec<SeqOutcome>,
    pub name: String,
    /// A failure that is not a disagreement of values: Verilator broken, the
    /// interpreter refusing the module, a stale excuse row.
    pub failed_hard: bool,
}

impl Verdicts {
    pub fn all_ok(&self) -> bool {
        self.transpiled
            && self.reference_sv != Some(false)
            && !matches!(self.interpreter, Some(Err(_)))
            && !self.failed_hard
    }

    /// Counts of each sequence verdict, in a fixed order, for the summary line.
    pub fn counts(&self) -> String {
        let mut c = std::collections::BTreeMap::new();
        for s in &self.sequences {
            *c.entry(s.verdict()).or_insert(0usize) += 1;
        }
        ["PASS", "OUTSIDE", "TRUNCATED", "EXCUSED", "DISAGREE"]
            .iter()
            .map(|v| format!("{} {v}", c.get(v).copied().unwrap_or(0)))
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// A measurement test that PINS this run's disagreement records it in the
    /// ledger as pinned, with its pattern, so G-G can match it against the row
    /// excusing the module (or see that a disagreement is pinned at all).
    pub fn record_pin(&self, pin: &str) {
        let pattern = self.pattern.map_or("none", Pattern::label);
        let mut lines = vec![format!("PINNED\t{}\t{pin}\tpattern={pattern}", self.name)];
        lines.extend(self.sequences.iter().map(|s| s.line(&self.name)));
        ledger_write("pin", &self.name, &lines);
        println!("pinned disagreement [{}]: pattern {pattern} — {pin}", self.name);
    }
}

// ── The reference-interpreter leg ────────────────────────────────────────────

/// The simulator's recorded trace replayed through `copper_ref::Module` — the
/// third leg of the sweep (`design_docs/SPEC_OVERVIEW.md` §7). Driven exactly like
/// `EquivalenceTest`: `begin_sequence`, `record_observation_zero`, `record` per
/// cycle, `record_failure` at a run-time failure, `finish`. Needs no Verilator, so
/// the generated `*_reference_leg` cases run under `--no-examples` and `--quick` too.
///
/// The harness convention is the protocol's: the inputs recorded for cycle `i`
/// are `x_{i+1}`, driven before edge `i+1`, so the leg calls `resume` with them,
/// then `edge`, then compares `observe` — `obs_{i+1}` — with what the simulator
/// read after its `tick_clock`. Observation 0 (SPEC §III.2.1) is `observe` after the
/// first `resume` and before the first `edge`, compared exactly (X included,
/// III.2.0) with what the simulator read between `pre_edge_settle` and
/// `post_edge_settle`. A run-time failure (III.2.4) must happen at the same
/// resumption on both sides: the interpreter is resumed once more, with the failing
/// resumption's inputs, and must fail there.
pub struct ReferenceLeg {
    name: String,
    src: String,
    module: String,
    params: Vec<(String, u64)>,
    cycles: Vec<CycleData>,
    sequences: Vec<(usize, String)>,
    obs0: Vec<Option<Vec<(String, Vec<Logic>)>>>,
    failures: Vec<Option<SimFailure>>,
    case: Option<String>,
}

/// One sequence of the replay.
#[derive(Debug, Clone, Default)]
pub struct RefSeq {
    /// The interpreter's observation 0, by (legalized) port.
    pub obs0: std::collections::HashMap<String, Vec<Logic>>,
    /// The simulator and the interpreter failed at the same resumption.
    pub truncated_agree: bool,
    /// One failed where the other did not, in words.
    pub failure_mismatch: Option<String>,
}

/// What the replay found.
#[derive(Debug, Clone)]
pub struct ReferenceVerdict {
    pub name: String,
    /// Cycles compared and agreeing.
    pub agreed: usize,
    /// Cycles compared and disagreeing.
    pub disagreed: usize,
    /// The first disagreement or refusal, in full.
    pub first_failure: Option<String>,
    /// Up to 40 `FAIL:` lines.
    pub fails: Vec<String>,
    /// Static tick sites of the kernel and how many of them some resumption reached.
    pub sites_static: usize,
    pub sites_reached: usize,
    /// Per sequence.
    pub seqs: Vec<RefSeq>,
    /// The interpreter's observation after each recorded cycle, by (recorded cycle,
    /// legalized port) — what the pattern of a disagreement is read from.
    pub values: std::collections::HashMap<(usize, String), Vec<Logic>>,
    /// Observations 0 compared, and how many agreed.
    pub obs0_compared: usize,
    pub obs0_agreed: usize,
    /// Sequences truncated with both sides failing at the same resumption.
    pub truncated: usize,
}

impl ReferenceVerdict {
    pub fn passed(&self) -> bool {
        self.first_failure.is_none()
    }
    pub fn verdict(&self) -> Result<usize, String> {
        match &self.first_failure {
            None => Ok(self.agreed),
            Some(f) => Err(f.clone()),
        }
    }
    /// The one line `tools/regression.sh` and a reader look for.
    pub fn summary_line(&self) -> String {
        let extra = format!(
            "; observation 0 agreed on {} of {}; {} sequence(s) truncated at an agreed run-time failure",
            self.obs0_agreed, self.obs0_compared, self.truncated
        );
        match &self.first_failure {
            None => format!(
                "reference leg: {} cycles agreed ({} of {} tick sites reached){extra} [{}]",
                self.agreed, self.sites_reached, self.sites_static, self.name
            ),
            Some(f) => format!(
                "reference leg: FAIL — {} of {} cycles disagree{extra}; first: {f} [{}]",
                self.disagreed,
                self.agreed + self.disagreed,
                self.name
            ),
        }
    }
    pub fn detail(&self) -> String {
        let mut s = String::new();
        s.push_str(&self.summary_line());
        s.push('\n');
        for l in &self.fails {
            s.push_str(l);
            s.push('\n');
        }
        s
    }
}

impl ReferenceLeg {
    pub fn new(name: &str, src: &str, module: &str, params: &[(&str, i64)]) -> Self {
        ReferenceLeg {
            name: name.to_string(),
            src: src.to_string(),
            module: module.to_string(),
            params: params
                .iter()
                .map(|(k, v)| (k.to_string(), u64::try_from(*v).expect("a non-negative parameter")))
                .collect(),
            cycles: Vec::new(),
            sequences: Vec::new(),
            obs0: Vec::new(),
            failures: Vec::new(),
            case: None,
        }
    }

    /// Name this run as the sweep case `case`, for the verdict ledger (G-G).
    pub fn sweep_case(mut self, case: &str) -> Self {
        self.case = Some(case.to_string());
        self
    }

    /// Accepted for symmetry with [`EquivalenceTest::with_reset_row`]: the reset
    /// row's declaration concerns the SV pair only; `sim ≡ ref` compares
    /// observation 0 exactly, X included (SPEC §III.2.0).
    pub fn with_reset_row(self) -> Self {
        self
    }

    pub fn begin_sequence(&mut self, label: &str) {
        self.sequences.push((self.cycles.len(), label.to_string()));
        self.obs0.push(None);
        self.failures.push(None);
    }

    fn ensure_sequence(&mut self) {
        if self.sequences.is_empty() {
            self.begin_sequence("");
        }
    }

    /// Observation 0 of the current sequence, as the simulator recorded it.
    pub fn record_observation_zero(&mut self, outputs: &[(&str, &[Logic])]) {
        self.ensure_sequence();
        let k = self.sequences.len() - 1;
        assert_eq!(self.sequences[k].0, self.cycles.len(), "observation 0 precedes the sequence's first cycle");
        self.obs0[k] = Some(outputs.iter().map(|(n, v)| (n.to_string(), v.to_vec())).collect());
    }

    /// Port names may be the source names or the emitted (legalized) ones.
    pub fn record(&mut self, inputs: &[(&str, &[Logic])], outputs: &[(&str, &[Logic])]) {
        self.ensure_sequence();
        let i = self.cycles.len();
        self.cycles.push(make_cycle(i, inputs, outputs));
    }

    /// The simulator failed at this resumption (SPEC §III.2.4); see
    /// [`EquivalenceTest::record_failure`].
    pub fn record_failure(&mut self, inputs: &[(&str, &[Logic])], message: &str) {
        self.ensure_sequence();
        let Some(kind) = runtime_failure_kind(message) else {
            panic!(
                "the simulator panicked at recorded cycle {} of '{}', and not with a run-time \
                 failure of SPEC §III.2.4 — this is a failure, not a truncation:\n{message}",
                self.cycles.len(),
                self.name
            );
        };
        let k = self.sequences.len() - 1;
        self.failures[k] = Some(SimFailure {
            at: self.cycles.len(),
            inputs: inputs.iter().map(|(n, v)| (n.to_string(), v.to_vec())).collect(),
            message: message.to_string(),
            kind,
        });
    }

    /// Replay. A refusal by the interpreter (a construct it does not cover, or a
    /// module class it defers) is a failure, never a skip: the `REF_SKIP` table in
    /// `build.rs` is the one place a module is excused, with a sentence.
    pub fn run(&self) -> ReferenceVerdict {
        use copper_ref::RefError;
        let mut v = ReferenceVerdict {
            name: self.name.clone(),
            agreed: 0,
            disagreed: 0,
            first_failure: None,
            fails: Vec::new(),
            sites_static: 0,
            sites_reached: 0,
            seqs: Vec::new(),
            values: Default::default(),
            obs0_compared: 0,
            obs0_agreed: 0,
            truncated: 0,
        };
        let ps: Vec<(&str, u64)> = self.params.iter().map(|(k, x)| (k.as_str(), *x)).collect();
        let fresh = || copper_ref::Module::new(&self.src, &self.module, &ps);
        let probe = match fresh() {
            Ok(m) => m,
            Err(e) => {
                v.first_failure = Some(format!(
                    "copper_ref refused `{}` at construction: {e} — if this is a module class the \
                     interpreter defers, name it in build.rs's REF_SKIP with that sentence",
                    self.module
                ));
                return v;
            }
        };
        v.sites_static = probe.tick_sites().len();
        let mut reached = std::collections::BTreeSet::new();
        let fail = |v: &mut ReferenceVerdict, line: String| {
            if v.first_failure.is_none() {
                v.first_failure = Some(line.clone());
            }
            if v.fails.len() < 40 {
                v.fails.push(format!("FAIL: {line}"));
            }
            v.disagreed += 1;
        };
        let lp = |p: &str| copper_codegen::legalized_port_name(p);
        let spans: Vec<(usize, usize, String)> = if self.sequences.is_empty() {
            vec![(0, self.cycles.len(), String::new())]
        } else {
            self.sequences
                .iter()
                .enumerate()
                .map(|(k, (s, l))| (*s, self.sequences.get(k + 1).map_or(self.cycles.len(), |(n, _)| *n), l.clone()))
                .collect()
        };
        for (k, (start, end, label)) in spans.iter().enumerate() {
            let seq_name = format!("sequence {k} \"{label}\"");
            let mut rs = RefSeq::default();
            let mut m = fresh().expect("constructed once already");
            let sim_fail = self.failures.get(k).cloned().flatten();
            let obs0 = self.obs0.get(k).cloned().flatten();
            let mut ref_failed: Option<(usize, String)> = None;
            let mut pos = *start;
            loop {
                let inputs: &[(String, Vec<Logic>)] = if pos < *end {
                    &self.cycles[pos].inputs
                } else if let Some(f) = &sim_fail {
                    &f.inputs
                } else {
                    break;
                };
                let in_ports = m.input_ports();
                let mut ins: Vec<(&str, &[Logic])> = Vec::new();
                for p in &in_ports {
                    let legal = lp(&p.name);
                    let Some((_, bits)) = inputs.iter().find(|(n, _)| *n == p.name || *n == legal) else {
                        fail(&mut v, format!("Cycle {pos}: no recorded value for input `{}`", p.name));
                        return v;
                    };
                    ins.push((p.name.as_str(), &bits[..]));
                }
                let resumption = pos - start + 1;
                // A `combinational` module is a function evaluated at every settle
                // (SPEC §I.11.1); the harness's vector is one settle. A clocked one
                // is resumed (I.0.2).
                let step = if m.is_combinational() {
                    m.evaluate_bits(&ins)
                } else {
                    m.resume_bits(&ins).map(|_| ())
                };
                match step {
                    Ok(()) => {}
                    Err(RefError::Runtime(e)) if ref_runtime_failure_kind(&e).is_some() => {
                        ref_failed = Some((pos, e));
                        break;
                    }
                    Err(e) => {
                        fail(&mut v, format!("Cycle {pos} ({seq_name}): the interpreter stopped: {e}"));
                        return v;
                    }
                }
                if pos == *start {
                    if let Some(ob) = &obs0 {
                        v.obs0_compared += 1;
                        let mut ok = true;
                        for (port, got) in m.observe_bits() {
                            let Some((_, want)) = ob.iter().find(|(n, _)| *n == port || *n == lp(&port)) else {
                                fail(&mut v, format!("observation 0: the simulator recorded no output `{port}`"));
                                return v;
                            };
                            if *want != got {
                                ok = false;
                                fail(
                                    &mut v,
                                    format!(
                                        "Cycle {pos} ({seq_name}, observation 0): {port} simulator {} interpreter {}",
                                        fmt_bits(want),
                                        fmt_bits(&got)
                                    ),
                                );
                            }
                            rs.obs0.insert(lp(&port), got);
                        }
                        if ok {
                            v.obs0_agreed += 1;
                        }
                    }
                }
                if pos == *end {
                    // The simulator failed at this resumption; the interpreter did not.
                    let f = sim_fail.as_ref().expect("past the end only at a failure");
                    let line = format!(
                        "Cycle {pos} ({seq_name}): the simulator failed at resumption {resumption} \
                         ({}: {}) and the interpreter completed it — a run-time failure must end both \
                         traces at the same resumption (SPEC §III.2.4)",
                        f.kind,
                        one_line(&f.message)
                    );
                    rs.failure_mismatch = Some(line.clone());
                    fail(&mut v, line);
                    break;
                }
                m.edge();
                let c = &self.cycles[pos];
                let mut cycle_ok = true;
                for (port, got) in m.observe_bits() {
                    let Some((_, want)) = c.outputs.iter().find(|(n, _)| *n == port || *n == lp(&port)) else {
                        fail(&mut v, format!("Cycle {pos}: the simulator recorded no output `{port}`"));
                        return v;
                    };
                    if *want != got {
                        cycle_ok = false;
                        fail(
                            &mut v,
                            format!(
                                "Cycle {pos} ({seq_name}): {port} simulator {} interpreter {}",
                                fmt_bits(want),
                                fmt_bits(&got)
                            ),
                        );
                    }
                    v.values.insert((pos, lp(&port)), got);
                }
                if cycle_ok {
                    v.agreed += 1;
                }
                pos += 1;
            }
            match (&ref_failed, &sim_fail) {
                (Some((p, _)), Some(f)) if *p == f.at => {
                    rs.truncated_agree = true;
                    v.truncated += 1;
                }
                (Some((p, e)), _) => {
                    let line = format!(
                        "Cycle {p} ({seq_name}): the interpreter failed at resumption {} ({}) and the \
                         simulator did not — a run-time failure must end both traces at the same \
                         resumption (SPEC §III.2.4)",
                        p - start + 1,
                        one_line(e)
                    );
                    rs.failure_mismatch = Some(line.clone());
                    fail(&mut v, line);
                }
                _ => {}
            }
            reached.extend(m.reached_sites().iter().cloned());
            v.seqs.push(rs);
        }
        v.sites_reached = reached.len();
        v
    }

    /// The per-sequence outcomes of a reference-leg case (no SV: the pattern of a
    /// disagreement is `sim≠ref,no-SV`).
    fn outcomes(&self, v: &ReferenceVerdict) -> Vec<SeqOutcome> {
        let labels: Vec<String> = if self.sequences.is_empty() {
            vec![String::new()]
        } else {
            self.sequences.iter().map(|(_, l)| l.clone()).collect()
        };
        labels
            .iter()
            .enumerate()
            .map(|(k, label)| {
                let mut o = SeqOutcome { index: k, label: label.clone(), ..Default::default() };
                let tag = format!("sequence {k} \"{label}\"");
                if let Some(f) = v.fails.iter().find(|l| l.contains(&tag)) {
                    o.disagree = Some((Pattern::SimNeRefNoSv, f.clone()));
                }
                if v.seqs.get(k).is_some_and(|q| q.truncated_agree) {
                    let f = self.failures[k].as_ref().expect("truncated at a recorded failure");
                    o.truncated = Some((f.at - self.sequences[k].0 + 1, format!("{} — {}", f.kind, f.message)));
                }
                o
            })
            .collect()
    }

    /// Print the summary line and panic on a disagreement.
    pub fn finish(self) {
        let v = self.run();
        println!("{}", v.summary_line());
        let outcomes = self.outcomes(&v);
        for o in outcomes.iter().filter(|o| o.verdict() != "PASS") {
            println!("  {}", o.line(&self.name));
        }
        if let Some(case) = &self.case {
            let mut lines: Vec<String> = outcomes.iter().map(|o| o.line(case)).collect();
            let mut c = std::collections::BTreeMap::new();
            for o in &outcomes {
                *c.entry(o.verdict()).or_insert(0usize) += 1;
            }
            let counts = ["PASS", "OUTSIDE", "TRUNCATED", "EXCUSED", "DISAGREE"]
                .iter()
                .map(|k| format!("{} {k}", c.get(k).copied().unwrap_or(0)))
                .collect::<Vec<_>>()
                .join(", ");
            let pattern = if v.passed() { "none" } else { Pattern::SimNeRefNoSv.label() };
            lines.push(format!("CASE\t{case}\t{counts}\tpattern={pattern}"));
            ledger_write("sweep-reference", case, &lines);
        }
        if !v.passed() {
            panic!(
                "reference-interpreter leg failed for '{}': the simulator and the spec's \
                 interpreter disagree (pattern {} — {})\n{}",
                self.name,
                Pattern::SimNeRefNoSv.label(),
                Pattern::SimNeRefNoSv.reading(),
                v.detail()
            );
        }
    }
}

/// A bit vector as the integer it spells (MSB first as `b…`), X shown as such.
fn fmt_bits(v: &[Logic]) -> String {
    if v.iter().any(|b| *b == Logic::X) {
        let s: String = v.iter().rev().map(|b| match b { Logic::One => '1', Logic::Zero => '0', Logic::X => 'x' }).collect();
        return format!("b{s}");
    }
    let n = v.iter().enumerate().fold(0u128, |a, (i, b)| a | ((*b == Logic::One) as u128) << i);
    format!("{n}")
}

/// A leg's errors, as recorded — or, for a multi-sequence run, a digest: which
/// sequences failed (by index and label, with their mismatch count and first
/// `FAIL:` line), how to re-run one alone, and the `FAIL:` lines themselves
/// without the `PASS:` lines around them (a multi-sequence run has thousands).
fn errors_by_sequence(sequences: &[(usize, String)], errors: &[String]) -> String {
    {
        if sequences.is_empty() {
            return errors.join("\n");
        }
        let text = errors.join("\n");
        let fails: Vec<&str> = text.lines().filter(|l| l.starts_with("FAIL: Cycle ")).collect();
        if fails.is_empty() {
            return text;
        }
        let sequence_of = |cycle: usize| {
            sequences
                .iter()
                .enumerate()
                .rev()
                .find(|(_, (start, _))| *start <= cycle)
                .map(|(i, (start, label))| (i, *start, label.as_str()))
        };
        let mut per_seq: Vec<(usize, usize, &str, usize, &str)> = Vec::new();
        for line in &fails {
            let cycle: usize = line["FAIL: Cycle ".len()..]
                .split_whitespace()
                .next()
                .and_then(|c| c.parse().ok())
                .unwrap_or(usize::MAX);
            let Some((i, start, label)) = sequence_of(cycle) else { continue };
            match per_seq.iter_mut().find(|e| e.0 == i) {
                Some(e) => e.3 += 1,
                None => per_seq.push((i, start, label, 1, line)),
            }
        }
        let mut out = String::new();
        out.push_str(&format!(
            "{} mismatching cycle(s) in {} of {} stimulus sequence(s); re-run one alone with \
             {SWEEP_SEQUENCE_ENV}=<index>:\n",
            fails.len(),
            per_seq.len(),
            sequences.len()
        ));
        for (i, start, label, n, first) in &per_seq {
            out.push_str(&format!(
                "  sequence {i} \"{label}\" (cycles from {start}): {n} mismatch(es), first: {first}\n"
            ));
        }
        out.push('\n');
        let shown = fails.len().min(80);
        out.push_str(&fails[..shown].join("\n"));
        if fails.len() > shown {
            out.push_str(&format!("\n… {} more FAIL line(s)", fails.len() - shown));
        }
        out.push('\n');
        for l in text.lines().filter(|l| !l.starts_with("FAIL: ") && !l.starts_with("PASS: ")) {
            out.push_str(l);
            out.push('\n');
        }
        out
    }
}
