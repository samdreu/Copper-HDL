use copper_core::Logic;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

/// Represents a trace of signal values over time
#[derive(Debug, Clone)]
pub struct SimulationTrace {
    pub cycles: Vec<CycleData>,
    /// Stimulus-sequence boundaries, in order (see [`SimulationTrace::begin_sequence`]).
    /// Empty for a single-sequence trace, which is every trace recorded before
    /// 2026-10-28 and produces a byte-identical testbench to that date's.
    pub sequences: Vec<SequenceMark>,
}

/// The start of one stimulus sequence inside a trace. Every sequence begins from
/// the design's **initial state**: the generated testbench destroys the Verilated
/// model and constructs a fresh one at the boundary, so the sequences share one
/// compiled model and one build but nothing else — the corpus sweep's
/// multi-sequence stimulus policy (`design_docs/CORPUS_DIFFERENTIAL_SWEEP.md` §3.2)
/// rests on this.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SequenceMark {
    /// Index into `cycles` of the sequence's first cycle.
    pub start: usize,
    /// What the sequence is (a seed, a first-cycle vector, …), carried into the
    /// testbench's `FAIL:` lines so a mismatch names the stimulus that produced it.
    pub label: String,
}

/// Represents the data for a single cycle in the simulation, including the cycle number, input signal values, and output signal values. 
/// This is used to compare against Verilator's output for verification.
#[derive(Debug, Clone)]
pub struct CycleData {
    pub cycle: usize,
    pub inputs: Vec<(String, Vec<Logic>)>,
    pub outputs: Vec<(String, Vec<Logic>)>,
}

impl SimulationTrace {
    /// Create a new empty simulation trace.
    pub fn new() -> Self {
        SimulationTrace { cycles: Vec::new(), sequences: Vec::new() }
    }

    /// Create a trace from a pre-built list of cycles. Useful for constructing expected traces.
    pub fn from_cycles(cycles: Vec<CycleData>) -> Self {
        SimulationTrace { cycles, sequences: Vec::new() }
    }

    /// Begin a new stimulus sequence: the cycles recorded from here on start from
    /// the design's initial state, on a fresh instance of the Verilated model. Call
    /// it before the first cycle of every sequence, the first included (a mark at
    /// index 0 needs no restart and emits none).
    pub fn begin_sequence(&mut self, label: &str) {
        self.sequences.push(SequenceMark { start: self.cycles.len(), label: label.to_string() });
    }

    /// The sequence (its index among the marks and the mark itself) that the cycle
    /// at position `index` belongs to, if the trace has sequence marks at all.
    pub fn sequence_of(&self, index: usize) -> Option<(usize, &SequenceMark)> {
        self.sequences
            .iter()
            .enumerate()
            .rev()
            .find(|(_, m)| m.start <= index)
    }

    /// Add a cycle's data to the trace.
    pub fn add_cycle(&mut self, cycle: usize, inputs: Vec<(String, Vec<Logic>)>, outputs: Vec<(String, Vec<Logic>)>) {
        self.cycles.push(CycleData { cycle, inputs, outputs });
    }

    /// Export the trace as a VCD (Value Change Dump) file viewable in GTKWave or similar tools.
    ///
    /// Signal widths are inferred from the first cycle. One VCD time unit = one simulation cycle.
    pub fn export_vcd(&self, path: &str, module_name: &str) -> Result<(), String> {
        if self.cycles.is_empty() {
            return Err("Cannot export VCD: trace has no cycles".to_string());
        }

        // Collect signal names and widths from first cycle, inputs then outputs
        let first = &self.cycles[0];
        let mut signals: Vec<(String, usize, char)> = Vec::new(); // (name, width, symbol)
        let mut symbol = '!' as u8;

        for (name, vals) in first.inputs.iter().chain(first.outputs.iter()) {
            signals.push((name.clone(), vals.len(), symbol as char));
            symbol += 1;
        }

        let symbol_map: HashMap<String, char> = signals
            .iter()
            .map(|(name, _, sym)| (name.clone(), *sym))
            .collect();

        let mut vcd = String::new();

        // Header
        vcd.push_str("$timescale 1ps $end\n");
        vcd.push_str(&format!("$scope module {} $end\n", module_name));
        for (name, width, sym) in &signals {
            vcd.push_str(&format!("$var wire {} {} {} $end\n", width, sym, name));
        }
        vcd.push_str("$upscope $end\n");
        vcd.push_str("$enddefinitions $end\n");

        // Initial values at time 0 from the first cycle
        vcd.push_str("#0\n");
        vcd.push_str("$dumpvars\n");
        for (name, _, sym) in &signals {
            let vals = first
                .inputs
                .iter()
                .chain(first.outputs.iter())
                .find(|(n, _)| n == name)
                .map(|(_, v)| v.as_slice())
                .unwrap_or(&[]);
            vcd.push_str(&format!("{} {}\n", logic_vals_to_vcd(vals), sym));
        }
        vcd.push_str("$end\n");

        // One timestep per cycle
        for cycle_data in &self.cycles {
            vcd.push_str(&format!("#{}\n", cycle_data.cycle));
            for (name, vals) in cycle_data.inputs.iter().chain(cycle_data.outputs.iter()) {
                if let Some(&sym) = symbol_map.get(name) {
                    vcd.push_str(&format!("{} {}\n", logic_vals_to_vcd(vals), sym));
                }
            }
        }

        // Create parent directory if needed
        if let Some(parent) = Path::new(path).parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent)
                    .map_err(|e| format!("Failed to create directory '{}': {}", parent.display(), e))?;
            }
        }

        fs::write(path, vcd)
            .map_err(|e| format!("Failed to write VCD '{}': {}", path, e))?;

        Ok(())
    }
}

/// Format a `Logic` slice as a VCD binary value string (e.g. `b0`, `b1`, `b10x1`).
/// The slice is stored LSB-first; VCD format requires MSB-first.
fn logic_vals_to_vcd(vals: &[Logic]) -> String {
    let bits: String = vals
        .iter()
        .rev() // convert LSB-first storage to MSB-first VCD
        .map(|v| match v {
            Logic::Zero => '0',
            Logic::One => '1',
            Logic::X => 'x',
        })
        .collect();
    format!("b{}", bits)
}

/// Generate a C++ testbench for Verilator.
///
/// When `tracing` is set, the testbench includes `--trace` instrumentation that
/// writes a VCD of all internal module signals to the path given as its **first
/// command-line argument**. The path is deliberately *not* baked into the C++:
/// the testbench text is part of the model-cache key (see
/// [`verify_with_verilator_report`]), and a per-invocation path inside it would make
/// every traced run a cache miss — and, worse, would let one cached binary only
/// ever write one file. Taking it at run time keeps the binary content-neutral and
/// every run's output per-invocation.
pub fn generate_testbench(
    module_name: &str,
    trace: &SimulationTrace,
    has_clock: bool,
    tracing: bool,
) -> String {
    let mut tb = String::new();

    // Includes
    tb.push_str(&format!("#include \"V{}.h\"\n", module_name));
    tb.push_str("#include \"verilated.h\"\n");
    if tracing {
        tb.push_str("#include \"verilated_vcd_c.h\"\n");
    }
    tb.push_str("#include <iostream>\n");
    if !trace.sequences.is_empty() && tracing {
        tb.push_str("#include <string>\n");
    }
    tb.push_str("\n");

    // main()
    tb.push_str("int main(int argc, char** argv) {\n");
    tb.push_str("    Verilated::commandArgs(argc, argv);\n");
    if tracing {
        tb.push_str("    Verilated::traceEverOn(true);\n");
    }
    tb.push_str(&format!("    V{} *top = new V{}();\n", module_name, module_name));

    // Trace setup — the VCD path arrives as argv[1] (see the doc comment above).
    if tracing {
        tb.push_str("    if (argc < 2) {\n");
        tb.push_str(&format!(
            "        std::cerr << \"usage: V{} <vcd-output-path>\" << std::endl;\n",
            module_name
        ));
        tb.push_str("        return 2;\n");
        tb.push_str("    }\n");
        tb.push_str("    VerilatedVcdC* tfp = new VerilatedVcdC();\n");
        tb.push_str("    top->trace(tfp, 99);\n"); // 99 = trace all hierarchy levels
        tb.push_str("    tfp->open(argv[1]);\n");
        tb.push_str("    uint64_t vcd_time = 0;\n");
    }

    tb.push_str("    int failures = 0;\n\n");

    // Test cycles
    for (index, cycle_data) in trace.cycles.iter().enumerate() {
        // A sequence boundary: the model is destroyed and constructed afresh, so the
        // sequence starts from the design's initial state exactly as the first one
        // did (and as the simulator's fresh executor does). A traced run writes one
        // VCD per sequence, `<argv[1]>.seq<k>` for every sequence after the first.
        // Marks at index 0 need no restart. The text here is part of the model-cache
        // key like everything else in the testbench, and is emitted only for a
        // trace that has marks, so a single-sequence trace is byte-identical to the
        // pre-2026-10-28 form.
        if let Some((k, mark)) = trace.sequences.iter().enumerate().find(|(_, m)| m.start == index) {
            tb.push_str(&format!("    // ── sequence {k}: {} ──\n", cpp_string(&mark.label)));
            if index > 0 {
                tb.push_str("    delete top;\n");
                tb.push_str(&format!("    top = new V{}();\n", module_name));
                if tracing {
                    tb.push_str("    tfp->close();\n");
                    tb.push_str("    delete tfp;\n");
                    tb.push_str("    tfp = new VerilatedVcdC();\n");
                    tb.push_str("    top->trace(tfp, 99);\n");
                    tb.push_str(&format!(
                        "    tfp->open((std::string(argv[1]) + \".seq{k}\").c_str());\n"
                    ));
                }
            }
        }
        // What a FAIL line says about the stimulus: nothing for a single-sequence
        // trace, the sequence and its local cycle otherwise.
        let seq_note = match trace.sequence_of(index) {
            Some((k, mark)) if !trace.sequences.is_empty() => format!(
                " (sequence {k} \\\"{}\\\", cycle {})",
                cpp_string(&mark.label),
                index - mark.start
            ),
            _ => String::new(),
        };
        tb.push_str(&format!("    // Cycle {}\n", cycle_data.cycle));

        for (name, values) in &cycle_data.inputs {
            let value = logic_vec_to_int(values);
            tb.push_str(&format!("    top->{} = {};\n", name, value));
        }

        if has_clock {
            tb.push_str("    top->clk = 0;\n");
            tb.push_str("    top->eval();\n");
            if tracing {
                tb.push_str("    tfp->dump(vcd_time++);\n");
            }
            tb.push_str("    top->clk = 1;\n");
            tb.push_str("    top->eval();\n");
            if tracing {
                tb.push_str("    tfp->dump(vcd_time++);\n");
            }
        } else {
            tb.push_str("    top->eval();\n");
            if tracing {
                tb.push_str("    tfp->dump(vcd_time++);\n");
            }
        }

        // Output checks — log failures but keep running so the full waveform is captured.
        for (name, expected) in &cycle_data.outputs {
            let expected_val = logic_vec_to_int(expected);
            tb.push_str(&format!(
                "    if (top->{name} != {exp}) {{\n\
                 \x20\x20\x20\x20    std::cout << \"FAIL: Cycle {cyc} {name} expected {exp} got \" << (int)top->{name} << \"{seq_note}\" << std::endl;\n\
                 \x20\x20\x20\x20    failures++;\n\
                 \x20\x20\x20\x20}} else {{\n\
                 \x20\x20\x20\x20    std::cout << \"PASS: Cycle {cyc} {name}\" << std::endl;\n\
                 \x20\x20\x20\x20}}\n",
                name = name,
                exp  = expected_val,
                cyc  = cycle_data.cycle,
            ));
        }

        tb.push_str("\n");
    }

    // Trace teardown
    if tracing {
        tb.push_str("    tfp->close();\n");
        tb.push_str("    delete tfp;\n");
    }

    tb.push_str("    delete top;\n");
    tb.push_str("    if (failures == 0) {\n");
    tb.push_str("        std::cout << \"All tests passed!\" << std::endl;\n");
    tb.push_str("        return 0;\n");
    tb.push_str("    } else {\n");
    tb.push_str("        std::cout << failures << \" test(s) failed.\" << std::endl;\n");
    tb.push_str("        return 1;\n");
    tb.push_str("    }\n");
    tb.push_str("}\n");

    tb
}

/// A label as it may appear inside a C++ string literal or a `//` comment: the two
/// characters that would end or escape the literal are escaped, and a newline —
/// which would end the comment and then the literal — is folded to a space.
fn cpp_string(label: &str) -> String {
    label
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace(['\n', '\r'], " ")
}

/// Convert Logic vector to integer for C++ testbench
fn logic_vec_to_int(values: &[Logic]) -> u64 {
    let mut result = 0u64;
    for (i, val) in values.iter().enumerate() {
        match val {
            Logic::One  => result |= 1 << i,
            Logic::Zero => {}
            Logic::X    => result |= 1 << i, // treat X as 1 for testbench purposes
        }
    }
    result
}

/// Unambiguous marker prefixing the *one* error that means "Verilator is not
/// installed on this machine" — the only condition a caller may legitimately treat
/// as a skip.
///
/// It exists because the alternative — matching prose like `contains("not found")`
/// — silently swallowed **real** build failures. Verilator's C++ stage emits
/// `fatal error: 'Vfoo.h' file not found` for a broken testbench, which matched that
/// substring, so a genuinely failing equivalence check reported PASS. Callers must
/// match this prefix and treat every other `Err` as a failure.
pub const VERILATOR_NOT_INSTALLED: &str = "VERILATOR_NOT_INSTALLED:";

/// Whether an error from [`verify_with_verilator`] means "Verilator is not installed"
/// — the *only* condition under which a caller may skip instead of fail.
///
/// Deliberately an exact-prefix match on [`VERILATOR_NOT_INSTALLED`], never a
/// substring search of the message. See the regression tests at the bottom of this
/// file for the real Verilator output that made the substring version unsafe.
pub fn is_missing_verilator(err: &str) -> bool {
    err.starts_with(VERILATOR_NOT_INSTALLED)
}

/// Whether the `verilator` binary is present and runnable.
///
/// Distinguishes three states that must not be conflated:
///   * `Ok(())` — present and `--version` succeeds.
///   * `Err(msg)` starting with [`VERILATOR_NOT_INSTALLED`] — the binary could not be
///     spawned at all. The only skippable case.
///   * `Err(msg)` without that prefix — the binary *is* installed but fails to run
///     (classically a stale `VERILATOR_ROOT`). That is a broken environment, not an
///     absent tool, and must surface loudly rather than masquerade as "not installed".
///
/// `VERILATOR_ROOT` is cleared for the probe exactly as it is for the build below,
/// so the probe and the thing it is probing for see the same environment.
pub fn verilator_status() -> Result<(), String> {
    verilator_version().map(|_| ())
}

/// The first line of `verilator --version`, probed once per process. The same
/// classification as [`verilator_status`] (which is this with the text dropped);
/// the text itself is part of every model-cache key, so a Verilator upgrade can
/// never serve a model built by the previous one.
pub fn verilator_version() -> Result<String, String> {
    static VERSION: OnceLock<Result<String, String>> = OnceLock::new();
    VERSION
        .get_or_init(|| {
            match Command::new("verilator")
                .arg("--version")
                .env_remove("VERILATOR_ROOT")
                .output()
            {
                Ok(o) if o.status.success() => Ok(String::from_utf8_lossy(&o.stdout)
                    .lines()
                    .next()
                    .unwrap_or("")
                    .trim()
                    .to_string()),
                Ok(o) => Err(format!(
                    "Verilator is installed but failed to run (`verilator --version` exited {}). \
                     This is a broken environment, not a missing tool — it must not be skipped.\n{}{}",
                    o.status,
                    String::from_utf8_lossy(&o.stdout),
                    String::from_utf8_lossy(&o.stderr),
                )),
                Err(e) => Err(format!(
                    "{VERILATOR_NOT_INSTALLED} Verilator not found ({e}). \
                     Install with: brew install verilator (macOS) or apt-get install verilator (Linux)"
                )),
            }
        })
        .clone()
}

/// Whether `ccache` can be spawned, probed once per process. Used only to decide
/// whether to hand Verilator's makefiles an object cache; a missing ccache changes
/// nothing but build time.
fn ccache_available() -> bool {
    static AVAILABLE: OnceLock<bool> = OnceLock::new();
    *AVAILABLE.get_or_init(|| {
        Command::new("ccache")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    })
}

// ─── The built-model cache ───────────────────────────────────────────────────
//
// A full regression Verilates a few hundred models, and nearly all of them are
// identical from one run to the next: the same emitted SystemVerilog, the same
// generated testbench, the same flags. Building each from scratch is where the wall
// time goes. So a *built* model (the `V<module>` executable) is kept under
// `target/verilator/cache/<key>/`, where `<key>` is a SHA-256 over everything the
// executable is a function of, and an invocation whose key already exists runs the
// cached executable instead of Verilator.
//
// Correctness comes before speed here, in three ways:
//
//  1. **The key is the whole input, not a summary of it.** It covers the full
//     SystemVerilog text (plus any file it `include`s), the full testbench text —
//     which is the stimulus and the expected outputs, compiled in — the module name,
//     the sorted `-G` parameters, the tracing flag, every Verilator argument, and the
//     Verilator version. A changed module cannot hit. A hit additionally compares the
//     stored key *material* byte-for-byte against the computed one, so a hash
//     collision degrades to a miss rather than to a wrong model.
//
//  2. **Nothing is ever built in, or written into, a shared directory.** A miss
//     builds exactly as before, in a work directory unique per invocation, runs the
//     model it built, and only then *publishes*: the executable, the key material and
//     a `COMPLETE` marker (written last) go into a private staging directory that is
//     atomically `rename`d to the cache path. An entry therefore either does not exist
//     or is complete. If the rename loses a race to another invocation of the same
//     model, this one's copy is discarded — an existing entry is never overwritten.
//     The shared-directory clobbering that made two tests run against each other's
//     builds (see the `WORK_DIR_SEQ` comment below) cannot recur, because the cache
//     directory is only ever read.
//
//  3. **A run never writes inside the cache.** Stimulus and expected values are
//     compiled into the executable; the only run-time output, the optional VCD, goes
//     to the per-invocation path the caller named, passed as `argv[1]`. Two tests
//     running the same cached model in parallel share nothing writable.
//
// `COPPER_VERILATOR_CACHE=0` turns the cache off (every invocation builds and runs
// in its own directory, exactly the pre-cache behaviour). `cargo clean` reclaims
// the cache along with the other `target/verilator` artifacts.

/// Whether [`verify_with_verilator_report`] may read and populate the built-model
/// cache. Read from the environment by [`VerilatorCachePolicy::from_env`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VerilatorCachePolicy {
    /// Hit the cache when an identical model was built before; publish on a miss.
    Enabled,
    /// Build every model from scratch in its own directory and never touch the cache.
    Disabled,
}

impl VerilatorCachePolicy {
    /// The environment variable that disables the cache: `COPPER_VERILATOR_CACHE=0`.
    pub const ENV: &'static str = "COPPER_VERILATOR_CACHE";

    /// The policy the environment asks for: `Disabled` iff [`Self::ENV`] is set to
    /// `0` (or `off`/`false`/`no`), `Enabled` otherwise — including when unset.
    pub fn from_env() -> Self {
        Self::from_setting(std::env::var(Self::ENV).ok().as_deref())
    }

    /// [`Self::from_env`] on an explicit value, so the parse is testable without
    /// mutating process-global environment under a parallel test harness.
    pub fn from_setting(value: Option<&str>) -> Self {
        match value.map(|v| v.trim().to_ascii_lowercase()).as_deref() {
            Some("0") | Some("off") | Some("false") | Some("no") => Self::Disabled,
            _ => Self::Enabled,
        }
    }
}

/// What the Verilated model's registers hold at power-on, before any `initial`
/// block runs. Read from the environment by [`VerilatorPowerOn::from_env`].
///
/// Verilator is a two-state simulator: by default every uninitialized variable
/// starts at zero, so a register whose emitted SystemVerilog states no power-on
/// value and whose Copper initializer is `Bits::zero()` *agrees by accident* — the
/// SV says nothing, the simulator says 0, Verilator says 0. `Randomized` closes
/// that accident: the model is built with `--x-initial unique` (and
/// `--x-assign unique`) and run with `+verilator+rand+reset+2`, so every
/// register the SV does not initialize starts at a random value, and the only
/// registers that can agree with the simulator are the ones whose `initial`
/// assignment the transpiler emitted — or ones a reset defines before the first
/// observation (the sweep's `RESET` rows). The seed is fixed so a run reproduces;
/// `COPPER_VERILATOR_X_INITIAL=<n>` picks it. A randomized build is a different
/// executable from a zero-fill build, and its flags are in the model-cache key
/// like every other Verilator argument, so the two never alias.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VerilatorPowerOn {
    /// Verilator's default: every uninitialized register starts at zero.
    ZeroFill,
    /// Every uninitialized register starts at a seeded random value.
    Randomized { seed: u32 },
}

impl VerilatorPowerOn {
    /// The environment variable: `COPPER_VERILATOR_X_INITIAL=1` randomizes with
    /// seed 1, `=<n>` with seed `n`; unset, empty, `0`, `off`, `false` or `no` is
    /// the zero-fill default.
    pub const ENV: &'static str = "COPPER_VERILATOR_X_INITIAL";

    /// The policy the environment asks for.
    pub fn from_env() -> Self {
        Self::from_setting(std::env::var(Self::ENV).ok().as_deref())
    }

    /// [`Self::from_env`] on an explicit value, testable without touching the
    /// process environment.
    pub fn from_setting(value: Option<&str>) -> Self {
        let Some(v) = value.map(|v| v.trim().to_ascii_lowercase()) else { return Self::ZeroFill };
        match v.as_str() {
            "" | "0" | "off" | "false" | "no" => Self::ZeroFill,
            "on" | "true" | "yes" => Self::Randomized { seed: 1 },
            other => match other.parse::<u32>() {
                Ok(0) => Self::ZeroFill,
                Ok(seed) => Self::Randomized { seed },
                Err(_) => Self::Randomized { seed: 1 },
            },
        }
    }

    /// Verilator arguments at build time. Part of the model-cache key.
    pub fn build_args(&self) -> &'static [&'static str] {
        match self {
            Self::ZeroFill => &[],
            Self::Randomized { .. } => &["--x-initial", "unique", "--x-assign", "unique"],
        }
    }

    /// Plusargs for the built executable (`Verilated::commandArgs` reads them).
    /// `rand+reset+2` is "randomize", and the seed makes the run reproducible;
    /// without it Verilator seeds from the clock.
    pub fn run_args(&self) -> Vec<String> {
        match self {
            Self::ZeroFill => Vec::new(),
            Self::Randomized { seed } => {
                vec!["+verilator+rand+reset+2".to_string(), format!("+verilator+seed+{seed}")]
            }
        }
    }
}

/// What the built-model cache did for one invocation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CacheOutcome {
    /// The cache was off ([`VerilatorCachePolicy::Disabled`]) or could not be used for
    /// this model (an unresolvable `` `include ``); the model was built and run in its
    /// own directory. `reason` says which.
    Bypassed { reason: String },
    /// A complete entry with byte-identical key material existed; Verilator did not run.
    Hit { key: String },
    /// No usable entry existed; the model was built, run, and then published.
    Miss { key: String },
}

impl CacheOutcome {
    /// The content key, when the cache was consulted at all.
    pub fn key(&self) -> Option<&str> {
        match self {
            CacheOutcome::Hit { key } | CacheOutcome::Miss { key } => Some(key),
            CacheOutcome::Bypassed { .. } => None,
        }
    }
}

/// The result of one [`verify_with_verilator_report`] call whose model built and ran.
#[derive(Clone, Debug)]
pub struct VerilatorRun {
    /// The testbench reported every cycle's outputs equal to the simulator's.
    pub passed: bool,
    /// What the built-model cache did.
    pub cache: CacheOutcome,
    /// The simulation's stdout (the per-cycle PASS/FAIL lines).
    pub output: String,
}

/// The directory that holds built models: `target/verilator/cache`, relative to the
/// working directory like every other Verilator artifact of this harness.
pub fn verilator_cache_root() -> PathBuf {
    PathBuf::from("target/verilator/cache")
}

/// Written last into an entry's staging directory, so its presence after the atomic
/// rename means the entry is complete.
const CACHE_COMPLETE_MARKER: &str = "COMPLETE";
/// The exact key material the entry's key was computed from, kept for the
/// byte-for-byte check on a hit.
const CACHE_MANIFEST: &str = "manifest.txt";

/// The `` `include "file" `` lines of a SystemVerilog source, outside `//` comments.
/// Good enough for the key: it must find every include Verilator would act on, and
/// over-approximating (a quoted name inside a block comment) only costs a bypass.
fn sv_includes(sv: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in sv.lines() {
        let code = line.split("//").next().unwrap_or("");
        let mut rest = code;
        while let Some(pos) = rest.find("`include") {
            let after = &rest[pos + "`include".len()..];
            let trimmed = after.trim_start();
            if let Some(stripped) = trimmed.strip_prefix('"')
                && let Some(close) = stripped.find('"')
            {
                out.push(stripped[..close].to_string());
            }
            rest = after;
        }
    }
    out
}

/// The text of every file the source `` `include ``s, transitively, each resolved
/// relative to the directory of the file that includes it (Verilator's first search
/// location). `Err` names an include that could not be read, in which case the
/// caller must not cache: a key that omits part of the model is exactly the stale
/// hit the cache exists to make impossible.
fn resolve_sv_includes(sv_path: &Path, sv: &str) -> Result<Vec<(String, String)>, String> {
    let mut resolved: Vec<(String, String)> = Vec::new();
    let mut pending: Vec<(PathBuf, String)> = sv_includes(sv)
        .into_iter()
        .map(|name| (sv_path.parent().map(Path::to_path_buf).unwrap_or_default(), name))
        .collect();
    while let Some((dir, name)) = pending.pop() {
        let path = dir.join(&name);
        let display = path.display().to_string();
        if resolved.iter().any(|(p, _)| *p == display) {
            continue;
        }
        let text = fs::read_to_string(&path)
            .map_err(|e| format!("`include \"{name}\" (resolved to {display}) is unreadable: {e}"))?;
        for nested in sv_includes(&text) {
            pending.push((path.parent().map(Path::to_path_buf).unwrap_or_default(), nested));
        }
        resolved.push((display, text));
    }
    resolved.sort();
    Ok(resolved)
}

/// The key material: a single text that every input to the built executable is
/// embedded in, length-prefixed so no two inputs can run together. The key is its
/// SHA-256; the text itself is stored beside the executable and compared on a hit.
#[allow(clippy::too_many_arguments)] // one argument per keyed input, on purpose
fn cache_manifest(
    verilator_version: &str,
    module_name: &str,
    params: &[(String, i64)],
    tracing: bool,
    key_args: &[String],
    sv: &str,
    includes: &[(String, String)],
    testbench: &str,
) -> String {
    let mut sorted: Vec<&(String, i64)> = params.iter().collect();
    sorted.sort();
    let mut m = String::new();
    m.push_str("copper verilator model cache, format 1\n");
    m.push_str(&format!("verilator: {verilator_version}\n"));
    m.push_str(&format!("module: {module_name}\n"));
    m.push_str(&format!("tracing: {tracing}\n"));
    m.push_str(&format!("params ({}):\n", sorted.len()));
    for (name, value) in sorted {
        m.push_str(&format!("  {name}={value}\n"));
    }
    m.push_str(&format!("args ({}):\n", key_args.len()));
    for arg in key_args {
        m.push_str(&format!("  {} {arg}\n", arg.len()));
    }
    m.push_str(&format!("sv ({} bytes):\n{sv}\n", sv.len()));
    m.push_str(&format!("includes ({}):\n", includes.len()));
    for (path, text) in includes {
        m.push_str(&format!("include {} ({} bytes):\n{text}\n", path, text.len()));
    }
    m.push_str(&format!("testbench ({} bytes):\n{testbench}\n", testbench.len()));
    m
}

/// Where a complete, byte-identical entry for `manifest` keeps its executable, if
/// one exists. Anything less than all three — marker present, manifest identical,
/// executable present — is a miss, never a partial hit.
fn cached_executable(entry: &Path, exe_name: &str, manifest: &str) -> Option<PathBuf> {
    if !entry.join(CACHE_COMPLETE_MARKER).is_file() {
        return None;
    }
    let stored = fs::read_to_string(entry.join(CACHE_MANIFEST)).ok()?;
    if stored != manifest {
        println!(
            "Verilator model: cache entry {} has DIFFERENT key material (hash collision or \
             corruption) — ignoring it and building from scratch",
            entry.display()
        );
        return None;
    }
    let exe = entry.join(exe_name);
    exe.is_file().then_some(exe)
}

/// Publish a freshly built executable as cache entry `entry`, atomically.
///
/// The executable, the key material and (last) the `COMPLETE` marker are assembled
/// in a staging directory private to this invocation, which is then `rename`d into
/// place: the entry is never visible half-written. A rename that fails because the
/// entry now exists means a parallel invocation of the same model got there first;
/// their entry is kept and ours is dropped. Any other failure is reported and the
/// staging directory removed — the verification result does not depend on it.
fn publish_cache_entry(entry: &Path, staging: &Path, built_exe: &Path, exe_name: &str, manifest: &str) {
    let result = (|| -> Result<(), String> {
        fs::create_dir_all(staging).map_err(|e| format!("create {}: {e}", staging.display()))?;
        let staged_exe = staging.join(exe_name);
        // Same filesystem (the work dir is also under target/verilator), so a rename
        // is a metadata move; fall back to a copy if it is not.
        if fs::rename(built_exe, &staged_exe).is_err() {
            fs::copy(built_exe, &staged_exe)
                .map_err(|e| format!("copy {} into staging: {e}", built_exe.display()))?;
        }
        fs::write(staging.join(CACHE_MANIFEST), manifest).map_err(|e| format!("write manifest: {e}"))?;
        fs::write(staging.join(CACHE_COMPLETE_MARKER), "").map_err(|e| format!("write marker: {e}"))?;
        match fs::rename(staging, entry) {
            Ok(()) => Ok(()),
            Err(_) if entry.join(CACHE_COMPLETE_MARKER).is_file() => {
                println!(
                    "Verilator model: cache entry {} was published by a parallel invocation; \
                     keeping theirs",
                    entry.display()
                );
                Ok(())
            }
            Err(e) => Err(format!("rename staging into {}: {e}", entry.display())),
        }
    })();
    if let Err(e) = result {
        println!("Verilator model: not cached ({e})");
    }
    // Whether we won, lost, or failed, nothing of ours may linger beside the entries.
    let _ = fs::remove_dir_all(staging);
}

/// Run Verilator simulation and compare with expected trace.
/// The original public API — no waveform output.
pub fn verify_with_verilator(
    verilog_file: &str,
    module_name: &str,
    trace: &SimulationTrace,
) -> Result<bool, String> {
    verify_with_verilator_traced(verilog_file, module_name, trace, None, &[])
}

/// Run Verilator simulation, compare with expected trace, and optionally dump
/// an internal-signal VCD to `vcd_output_path`.
///
/// When `vcd_output_path` is `Some(path)`, Verilator is compiled with `--trace`
/// and the simulation writes a full VCD (including all internal signals, registers,
/// and hierarchy) to `path`. Open it in GTKWave to inspect internal state.
///
/// A mismatch is an `Err`, as it always was for this entry point; see
/// [`verify_with_verilator_report`] for the form that also says what the cache did.
pub(crate) fn verify_with_verilator_traced(
    verilog_file: &str,
    module_name: &str,
    trace: &SimulationTrace,
    vcd_output_path: Option<&str>,
    params: &[(String, i64)],
) -> Result<bool, String> {
    let run = verify_with_verilator_report(
        verilog_file,
        module_name,
        trace,
        vcd_output_path,
        params,
        VerilatorCachePolicy::from_env(),
    )?;
    if run.passed {
        Ok(true)
    } else {
        Err(format!("Simulation failed:\n{}", run.output))
    }
}

/// The full-control form of [`verify_with_verilator`]: parameters, optional VCD,
/// and an explicit cache policy, reporting what the cache did alongside the verdict.
///
/// `Ok(run)` means a model was built (or found cached) and ran to completion;
/// `run.passed` is its verdict. `Err` is an infrastructure failure — Verilator absent
/// (see [`is_missing_verilator`]) or broken, a build error, or a model that could not
/// be executed — and is never a disagreement between the two simulations.
pub fn verify_with_verilator_report(
    verilog_file: &str,
    module_name: &str,
    trace: &SimulationTrace,
    vcd_output_path: Option<&str>,
    params: &[(String, i64)],
    policy: VerilatorCachePolicy,
) -> Result<VerilatorRun, String> {
    // Read the source ONCE: the text that is keyed is the text Verilator compiles.
    let verilog_text = fs::read_to_string(verilog_file)
        .map_err(|e| format!("Failed to read Verilog file '{}': {}", verilog_file, e))?;
    let has_clock = verilog_has_clock_port(&verilog_text);
    let tracing = vcd_output_path.is_some();

    // Ensure the VCD output directory exists before running the simulation
    if let Some(vcd_path) = vcd_output_path {
        if let Some(parent) = Path::new(vcd_path).parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent)
                    .map_err(|e| format!("Failed to create VCD directory: {}", e))?;
            }
        }
    }

    // Generate the testbench. Its text carries the stimulus and expected outputs and
    // nothing per-invocation (the VCD path is a run-time argument), so it is keyed.
    let testbench = generate_testbench(module_name, trace, has_clock, tracing);
    let param_suffix: String = params
        .iter()
        .map(|(name, value)| format!("_{name}{value}"))
        .collect();

    // ...and on a per-INVOCATION counter. Neither the module name nor the process is
    // enough:
    //
    //  * `det_010_independent_golden.rs` runs TWO tests against the same golden top
    //    module (`det_010`) in parallel threads of ONE binary. Same module, same
    //    params, same pid — they shared a directory and clobbered each other's build,
    //    failing intermittently with "no such file or directory: Vdet_010__ALL.a".
    //  * Two `cargo test` runs in one checkout collide the same way.
    //
    // A counter plus the pid is unique per call, which is the only thing that
    // actually holds. Reproduced and verified both ways before landing.
    static WORK_DIR_SEQ: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let unique = format!(
        "{}_{}",
        std::process::id(),
        WORK_DIR_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    );
    // Under `target/verilator/`, not the repo root: these are per-invocation and
    // never cleaned (uniqueness is what makes them safe, see above), so in the
    // root they accumulated by the dozen. `target/` already means "generated,
    // safe to delete" and `cargo clean` reclaims them.
    let work_root = Path::new("target/verilator");
    fs::create_dir_all(work_root)
        .map_err(|e| format!("Failed to create {}: {e}", work_root.display()))?;
    let obj_dir = format!("target/verilator/obj_dir_{module_name}{param_suffix}_{unique}");
    let tb_file = format!("target/verilator/tb_{module_name}{param_suffix}_{unique}.cpp");

    // Check Verilator is installed AND runnable. `verilator_version` separates
    // "absent" (skippable, marked with VERILATOR_NOT_INSTALLED) from "present but
    // broken" (must fail) — the old check used `.is_err()`, which only catches a
    // spawn failure, and reported prose the caller had to substring-match.
    let verilator_version = verilator_version()?;

    // Module parameter overrides (`-GN=8`), so the Verilated design matches the
    // widths the simulator ran with. Built here so the strings outlive the args.
    let param_args: Vec<String> = params
        .iter()
        .map(|(name, value)| format!("-G{name}={value}"))
        .collect();

    // Verilated output directory, keyed on module name AND parameters.
    //
    // Two things shared a single `obj_dir` before this, and both let a test run
    // against the WRONG Verilated model:
    //
    //  * Verilator defaults to `obj_dir` in the CWD, which *every* equivalence test
    //    shared. Tests inside one binary run in parallel threads, so two Verilating at
    //    once clobbered each other.
    //  * Worse, the same module built at *different parameters* also collided —
    //    `wide_alu` is checked at `N=32` and then `N=64`. Same name, same directory,
    //    and `--build`'s make step decides by mtime, so the second build could be
    //    skipped as "up to date" and the N=64 stimulus then ran against the N=32
    //    model. That is the observed symptom: a Bits<64> DUT reading back 32-bit
    //    values, intermittently (mtime has 1-second granularity), and always clean
    //    under `--test-threads=1`.
    //
    // Nondeterministic *failure* is how this surfaced, but the same collision can
    // equally produce a false PASS — which is why the directory is now unique per
    // (module, parameters) rather than per module.

    // Build compile arguments.
    //
    // The C++ is compiled at `-O0`: this harness exists to compare a few hundred
    // cycles against the simulator, so the model's run time is noise next to its
    // compile time, and Verilator's default `-Os` roughly doubles the latter. The
    // throughput benchmark (`tests/sim_throughput.rs`) builds its own models and is
    // unaffected. `OPT_FAST`/`OPT_GLOBAL` are the make variables `verilated.mk`
    // applies to the model and runtime objects respectively.
    //
    // Two copies of the list: the real one, and the one that goes into the cache key
    // with the three per-invocation paths replaced by placeholders (their *content*
    // is keyed separately). Every flag that shapes the executable is in both.
    //
    // The power-on policy's flags (`--x-initial unique`, see [`VerilatorPowerOn`])
    // are in this list too, so a randomized build and a zero-fill build of the
    // same SV are two cache entries, never one.
    let power_on = VerilatorPowerOn::from_env();
    let mut fixed_args: Vec<&str> = vec![
        "--cc", "--exe", "--build",
        "--top-module", module_name,
        "-Wall", "-Wno-DECLFILENAME",
        "-CFLAGS", "-std=c++14",
        "-MAKEFLAGS", "OPT_FAST=-O0",
        "-MAKEFLAGS", "OPT_GLOBAL=-O0",
    ];
    fixed_args.extend_from_slice(power_on.build_args());
    if let VerilatorPowerOn::Randomized { seed } = power_on {
        println!("Verilator power-on: randomized ({}={seed}, --x-initial unique, +verilator+rand+reset+2)", VerilatorPowerOn::ENV);
    }
    let mut verilator_args: Vec<&str> = fixed_args.clone();
    verilator_args.push("--Mdir");
    verilator_args.push(&obj_dir);
    for p in &param_args {
        verilator_args.push(p);
    }
    if tracing {
        verilator_args.push("--trace");
    }
    verilator_args.push(verilog_file);
    verilator_args.push(&tb_file);

    let mut key_args: Vec<String> = fixed_args.iter().map(|a| a.to_string()).collect();
    key_args.extend(["--Mdir", "<obj_dir>"].map(String::from));
    let mut sorted_param_args = param_args.clone();
    sorted_param_args.sort();
    key_args.extend(sorted_param_args);
    if tracing {
        key_args.push("--trace".to_string());
    }
    key_args.extend(["<verilog_file>", "<testbench>"].map(String::from));

    // ── Cache lookup ─────────────────────────────────────────────────────────
    let exe_name = format!("V{module_name}");
    let cache_root = verilator_cache_root();
    // `Some((entry, manifest))` when the cache may be consulted for this model.
    let cache_plan: Result<(PathBuf, String, String), String> = match policy {
        VerilatorCachePolicy::Disabled => Err(format!("{}=0", VerilatorCachePolicy::ENV)),
        VerilatorCachePolicy::Enabled => resolve_sv_includes(Path::new(verilog_file), &verilog_text)
            .map(|includes| {
                let manifest = cache_manifest(
                    &verilator_version,
                    module_name,
                    params,
                    tracing,
                    &key_args,
                    &verilog_text,
                    &includes,
                    &testbench,
                );
                let key = sha256_hex(manifest.as_bytes());
                (cache_root.join(&key), key, manifest)
            }),
    };

    let (sim_exe, outcome, cleanup): (PathBuf, CacheOutcome, Option<(String, String)>) =
        match &cache_plan {
            Ok((entry, key, manifest)) if cached_executable(entry, &exe_name, manifest).is_some() => {
                println!("Verilator model: cache hit {key}");
                (entry.join(&exe_name), CacheOutcome::Hit { key: key.clone() }, None)
            }
            _ => {
                match &cache_plan {
                    Ok((_, key, _)) => println!("Verilator model: cache miss {key} (building)"),
                    Err(reason) => println!("Verilator model: cache bypassed ({reason})"),
                }
                // ── Build, exactly as before, in the per-invocation directory ──
                fs::write(&tb_file, &testbench)
                    .map_err(|e| format!("Failed to write testbench: {}", e))?;
                println!("Generated testbench: {}", tb_file);
                println!("Running Verilator{}...", if tracing { " (with trace)" } else { "" });
                let mut verilator_cmd = Command::new("verilator");
                verilator_cmd
                    .args(&verilator_args)
                    .env_remove("VERILATOR_ROOT");
                // Every model recompiles the Verilator runtime from scratch. `verilated.mk`
                // prefixes each compile with `$(OBJCACHE)`, so when ccache is on the PATH and
                // the caller has not chosen otherwise, hand it over: the runtime objects then
                // compile once per machine. `tools/regression.sh` exports the same variable;
                // this covers a bare `cargo test`. A build-speed knob only — the model is
                // identical, which is why it is not part of the cache key.
                if std::env::var_os("OBJCACHE").is_none() && ccache_available() {
                    verilator_cmd.env("OBJCACHE", "ccache");
                }
                let verilator_output = verilator_cmd
                    .output()
                    .map_err(|e| format!("Failed to run Verilator: {}", e))?;

                if !verilator_output.status.success() {
                    let _ = fs::remove_dir_all(&obj_dir);
                    let _ = fs::remove_file(&tb_file);
                    return Err(format!(
                        "Verilator compilation failed:\n{}",
                        String::from_utf8_lossy(&verilator_output.stderr)
                    ));
                }
                println!("Verilator compilation successful");

                let built = Path::new(&obj_dir).join(&exe_name);
                if !built.is_file() {
                    return Err(format!("Simulation executable not found: {}", built.display()));
                }
                let outcome = match &cache_plan {
                    Ok((_, key, _)) => CacheOutcome::Miss { key: key.clone() },
                    Err(reason) => CacheOutcome::Bypassed { reason: reason.clone() },
                };
                (built, outcome, Some((obj_dir.clone(), tb_file.clone())))
            }
        };

    // ── Run ──────────────────────────────────────────────────────────────────
    println!("Running Verilator simulation...");
    let mut sim_cmd = Command::new(&sim_exe);
    if let Some(vcd_path) = vcd_output_path {
        sim_cmd.arg(vcd_path);
    }
    // After the VCD path: the testbench reads `argv[1]` positionally when tracing,
    // and `Verilated::commandArgs` finds plusargs anywhere.
    sim_cmd.args(power_on.run_args());
    let sim_output = sim_cmd
        .output()
        .map_err(|e| format!("Failed to run simulation {}: {}", sim_exe.display(), e))?;

    // ── Publish (a miss only), then drop the per-invocation scratch ──────────
    if let Some((obj_dir, tb_file)) = cleanup {
        if let (Ok((entry, _, manifest)), CacheOutcome::Miss { .. }) = (&cache_plan, &outcome) {
            let staging = cache_root.join(format!(".staging_{module_name}{param_suffix}_{unique}"));
            publish_cache_entry(entry, &staging, &sim_exe, &exe_name, manifest);
        }
        // The per-process work dir is scratch; drop it so runs do not accumulate
        // hundreds of directories in the repo root.
        let _ = fs::remove_dir_all(&obj_dir);
        let _ = fs::remove_file(&tb_file);
    }

    let stdout = String::from_utf8_lossy(&sim_output.stdout).into_owned();
    println!("Verilator output:\n{}", stdout);

    if !sim_output.status.success() {
        // Exit 1 is the testbench's own "some cycle mismatched" (it prints the FAIL
        // lines and the count); anything else is the model failing to run at all.
        if sim_output.status.code() == Some(1) && stdout.contains("test(s) failed.") {
            return Ok(VerilatorRun { passed: false, cache: outcome, output: stdout });
        }
        return Err(format!(
            "Simulation failed ({}):\n{}{}",
            sim_output.status,
            stdout,
            String::from_utf8_lossy(&sim_output.stderr)
        ));
    }

    if stdout.contains("All tests passed!") {
        Ok(VerilatorRun { passed: true, cache: outcome, output: stdout })
    } else {
        Err("Verilator simulation did not pass all tests".to_string())
    }
}

fn verilog_has_clock_port(content: &str) -> bool {
    let lowered = content.to_lowercase();
    lowered.contains("input") && lowered.contains("clk")
}

// ─── SHA-256 ─────────────────────────────────────────────────────────────────
//
// FIPS 180-4, written out here rather than pulled in as a dependency: the cache key
// is the only consumer, the function is forty lines, and `copper-sim` deliberately
// depends on almost nothing. Checked against the published test vectors below. The
// key's job is to be a stable name for the key material; a collision is caught by
// the byte-for-byte manifest comparison on a hit, not relied on never to happen.

/// SHA-256 of `data`, as 64 lowercase hex digits.
pub fn sha256_hex(data: &[u8]) -> String {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
        0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
        0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
        0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
        0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
        0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
        0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
    ];
    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
    ];

    // Padding: 0x80, zeros to 56 mod 64, then the bit length as a big-endian u64.
    let mut msg = data.to_vec();
    let bit_len = (data.len() as u64).wrapping_mul(8);
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bit_len.to_be_bytes());

    for chunk in msg.chunks_exact(64) {
        let mut w = [0u32; 64];
        for (i, word) in chunk.chunks_exact(4).enumerate() {
            w[i] = u32::from_be_bytes([word[0], word[1], word[2], word[3]]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16].wrapping_add(s0).wrapping_add(w[i - 7]).wrapping_add(s1);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh] = h;
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ (!e & g);
            let t1 = hh.wrapping_add(s1).wrapping_add(ch).wrapping_add(K[i]).wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(maj);
            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }
        for (slot, v) in h.iter_mut().zip([a, b, c, d, e, f, g, hh]) {
            *slot = slot.wrapping_add(v);
        }
    }

    let mut hex = String::with_capacity(64);
    for word in h {
        hex.push_str(&format!("{word:08x}"));
    }
    hex
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_logic_vec_to_int() {
        assert_eq!(logic_vec_to_int(&[Logic::Zero]), 0);
        assert_eq!(logic_vec_to_int(&[Logic::One]), 1);
        assert_eq!(logic_vec_to_int(&[Logic::Zero, Logic::One]), 2);
        assert_eq!(logic_vec_to_int(&[Logic::One, Logic::One]), 3);
    }

    /// FIPS 180-4 / NIST test vectors. The cache key is this function's output, so
    /// a wrong SHA-256 here would still be a *consistent* key — these pin that it is
    /// also the well-known one, so a key can be recomputed with `shasum -a 256`.
    #[test]
    fn sha256_matches_the_published_vectors() {
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            sha256_hex(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"),
            "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
        );
        // Two blocks plus padding spill-over (length exactly 56 mod 64 forces an
        // extra block), the branch a one-block vector never exercises.
        assert_eq!(
            sha256_hex(&[b'a'; 120]),
            "2f3d335432c70b580af0e8e1b3674a7c020d683aa5f73aaaedfdc55af904c21c"
        );
    }

    #[test]
    fn cache_policy_parses_only_an_explicit_off_as_disabled() {
        use VerilatorCachePolicy::*;
        assert_eq!(VerilatorCachePolicy::from_setting(None), Enabled);
        assert_eq!(VerilatorCachePolicy::from_setting(Some("")), Enabled);
        assert_eq!(VerilatorCachePolicy::from_setting(Some("1")), Enabled);
        assert_eq!(VerilatorCachePolicy::from_setting(Some("0")), Disabled);
        assert_eq!(VerilatorCachePolicy::from_setting(Some(" off ")), Disabled);
        assert_eq!(VerilatorCachePolicy::from_setting(Some("FALSE")), Disabled);
    }

    #[test]
    fn include_scan_sees_code_and_ignores_line_comments() {
        let sv = "`include \"defs.svh\"\n// removed `include \"bsg_defines.sv\"\n\
                  module m; `include \"a.sv\" `include \"b.sv\" endmodule\n";
        assert_eq!(sv_includes(sv), vec!["defs.svh", "a.sv", "b.sv"]);
        assert!(sv_includes("module m; endmodule").is_empty());
    }

    /// A model whose `include` cannot be read must never be cached under a key that
    /// omits it — `resolve_sv_includes` refuses, and the caller bypasses the cache.
    #[test]
    fn unresolvable_include_is_an_error_not_a_partial_key() {
        let err = resolve_sv_includes(Path::new("/nonexistent/dir/top.sv"), "`include \"gone.svh\"")
            .unwrap_err();
        assert!(err.contains("gone.svh"), "{err}");
        assert_eq!(resolve_sv_includes(Path::new("x.sv"), "module m; endmodule").unwrap(), vec![]);
    }

    /// Every input that shapes the executable moves the key, and nothing else does.
    #[test]
    fn manifest_moves_with_every_keyed_input() {
        let base = |sv: &str, tb: &str, tracing: bool, params: &[(String, i64)], args: &[String]| {
            sha256_hex(cache_manifest("Verilator 5.0", "m", params, tracing, args, sv, &[], tb).as_bytes())
        };
        let args = vec!["--cc".to_string()];
        let k0 = base("module m; endmodule", "int main(){}", false, &[], &args);
        assert_eq!(k0, base("module m; endmodule", "int main(){}", false, &[], &args), "stable");
        assert_ne!(k0, base("module m; endmodule ", "int main(){}", false, &[], &args), "sv byte");
        assert_ne!(k0, base("module m; endmodule", "int main(){ }", false, &[], &args), "tb byte");
        assert_ne!(k0, base("module m; endmodule", "int main(){}", true, &[], &args), "tracing");
        assert_ne!(k0, base("module m; endmodule", "int main(){}", false, &[("N".into(), 8)], &args), "param");
        assert_ne!(k0, base("module m; endmodule", "int main(){}", false, &[], &["--cc".into(), "--trace".into()]), "args");
        // Parameter ORDER is not an input: the same widths in any order are one model.
        let ab = [("A".to_string(), 1), ("B".to_string(), 2)];
        let ba = [("B".to_string(), 2), ("A".to_string(), 1)];
        assert_eq!(base("", "", false, &ab, &args), base("", "", false, &ba, &args));
        // The Verilator version is an input.
        assert_ne!(
            sha256_hex(cache_manifest("Verilator 5.0", "m", &[], false, &args, "", &[], "").as_bytes()),
            sha256_hex(cache_manifest("Verilator 5.1", "m", &[], false, &args, "", &[], "").as_bytes()),
        );
        // Length-prefixing: moving a byte across the sv/testbench boundary is a change.
        assert_ne!(base("ab", "c", false, &[], &args), base("a", "bc", false, &[], &args));
    }
}


#[cfg(test)]
mod verilator_classification_tests {
    use super::*;

    /// The regression that matters. These are **real** messages Verilator produces
    /// on a genuine build failure, captured from the tool. The classifier used to be
    /// `err.contains("not found") || err.contains("not installed")`, which matched
    /// the first one — so a broken C++ testbench was reported as "Verilator not
    /// available", the equivalence check was skipped, and the test PASSED.
    #[test]
    fn real_build_failures_are_never_classified_as_missing() {
        let build_failures = [
            // clang, when the generated testbench includes the wrong header.
            "Verilator compilation failed:\n../tb_good.cpp:1:10: fatal error: \
             'Vwrong_name.h' file not found",
            // A module the transpiler failed to emit.
            "Verilator compilation failed:\n%Error-MODMISSING: bad.sv:2:5: \
             Cannot find file containing module: 'missing_child'",
            "Verilator compilation failed:\n%Error: bad2.sv:1:10: \
             Cannot find include file: 'nope.svh'",
            "Verilator compilation failed:\n%Error: bad3.sv:2:16: \
             Can't find definition of variable: 'undefined_sig'",
            // A stale VERILATOR_ROOT: installed, but unusable. Not a skip either.
            "Verilator is installed but failed to run (`verilator --version` exited \
             exit status: 1).\n%Error: verilator: VERILATOR_ROOT is set to inconsistent path.",
        ];
        for err in build_failures {
            assert!(
                !is_missing_verilator(err),
                "a real build failure would be silently skipped:\n{err}"
            );
        }
    }

    /// The one message that *is* a legitimate skip carries the marker.
    #[test]
    fn the_absent_binary_message_is_classified_as_missing() {
        let err = format!(
            "{VERILATOR_NOT_INSTALLED} Verilator not found (No such file or directory). \
             Install with: brew install verilator"
        );
        assert!(is_missing_verilator(&err));
    }

    /// `verilator_status` must not report a *working* install as missing. Vacuous
    /// where Verilator is absent, which is the honest thing for it to be.
    #[test]
    fn a_working_install_is_not_reported_missing() {
        if let Err(e) = verilator_status() {
            assert!(
                is_missing_verilator(&e),
                "verilator_status failed for a reason other than absence: {e}"
            );
        }
    }

    /// The power-on switch: unset and the "off" spellings are zero-fill, `1`/`on`
    /// randomize with seed 1, a number is the seed.
    #[test]
    fn power_on_policy_parses_the_switch() {
        use VerilatorPowerOn::*;
        assert_eq!(VerilatorPowerOn::from_setting(None), ZeroFill);
        for off in ["", "0", "off", "false", "no", " OFF "] {
            assert_eq!(VerilatorPowerOn::from_setting(Some(off)), ZeroFill, "{off:?}");
        }
        for on in ["1", "on", "true", "yes", "random"] {
            assert_eq!(VerilatorPowerOn::from_setting(Some(on)), Randomized { seed: 1 }, "{on:?}");
        }
        assert_eq!(VerilatorPowerOn::from_setting(Some("42")), Randomized { seed: 42 });
        assert_eq!(ZeroFill.build_args(), &[] as &[&str]);
        assert!(ZeroFill.run_args().is_empty());
        assert_eq!(
            Randomized { seed: 7 }.build_args(),
            &["--x-initial", "unique", "--x-assign", "unique"]
        );
        assert_eq!(
            Randomized { seed: 7 }.run_args(),
            vec!["+verilator+rand+reset+2".to_string(), "+verilator+seed+7".to_string()]
        );
    }

    /// A randomized build must never be served from a zero-fill entry: its flags
    /// are Verilator arguments, and every argument is in the manifest.
    #[test]
    fn power_on_flags_move_the_cache_key() {
        let mut zero: Vec<String> = vec!["--cc".into()];
        let mut rand = zero.clone();
        rand.extend(VerilatorPowerOn::Randomized { seed: 1 }.build_args().iter().map(|s| s.to_string()));
        zero.extend(VerilatorPowerOn::ZeroFill.build_args().iter().map(|s| s.to_string()));
        let key = |args: &[String]| {
            sha256_hex(cache_manifest("Verilator 5.0", "m", &[], false, args, "module m; endmodule", &[], "tb").as_bytes())
        };
        assert_ne!(key(&zero), key(&rand));
    }
}
