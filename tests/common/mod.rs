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

    /// Like `new`, but checks the simulator against the reference model only —
    /// no transpilation, no Verilator cross-check. Use this for DUTs where the
    /// transpiler is known not to agree with the simulator yet (see
    /// TRANSPILATION_TODO.md, "`pre_edge_barrier` invisible to phase
    /// extraction"), so the frontend/simulation semantics can still be
    /// verified independently of that gap.
    pub fn sim_only(module_name: &str) -> Self {
        EquivalenceTest {
            test: HardwareTest::new(module_name),
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
        if transpiled.passed() && !ref_failed {
            return;
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
            (true, false) => unreachable!("early-returned above"),
        };
        report.push_str(&format!("equivalence failed for '{}': {verdict}\n", self.name));
        if !transpiled.passed() {
            report.push_str("\n-- simulator vs transpiled SystemVerilog --\n");
            report.push_str(&errors_by_sequence(&sequences, &transpiled.errors));
        }
        if let Some(r) = reference.filter(|r| !r.passed()) {
            report.push_str("\n-- simulator vs independent reference --\n");
            report.push_str(&errors_by_sequence(&sequences, &r.errors));
        }
        panic!("{report}");
    }
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
