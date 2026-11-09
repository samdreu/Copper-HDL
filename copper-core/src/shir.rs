use crate::chir::{CHIRBinOp, CHIRType, CHIRUnOp, ModuleLocalParam, ModuleParam, Width};
use crate::frontend_ir::SourceSpan;

// ── Module ────────────────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct SHIRModule {
    pub name: String,
    /// Module-level parameters (const generics), carried through from CHIR to
    /// VLIR emission (M2).
    pub params: Vec<ModuleParam>,
    /// Module-level constants (`localparam`s) carried through from CHIR.
    pub localparams: Vec<ModuleLocalParam>,
    pub ports: Vec<SHIRPort>,
    pub body: SHIRBody,
    pub span: SourceSpan,
}

#[derive(Debug)]
pub struct SHIRPort {
    pub name: String,
    pub direction: SHIRPortDir,
    pub kind: SHIRPortKind,
    /// Registered output port (`RegOut<T,D>`) — driven from `always_ff`. See
    /// `CHIRPort::registered`.
    pub registered: bool,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SHIRPortDir {
    Input,
    Output,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SHIRPortKind {
    Clock,
    Data { ty: CHIRType },
}

// ── Body ──────────────────────────────────────────────────────────────────────

#[derive(Debug)]
pub enum SHIRBody {
    Combinational(SHIRCombBody),
    Sequential(SHIRSeqBody),
    Structural(SHIRStructuralBody),
}

/// Pure-hierarchy parent body — internal nets + clocked submodule instances.
/// 1:1 with `CHIRStructuralBody` (no timing regions to lower).
#[derive(Debug)]
pub struct SHIRStructuralBody {
    pub nets: Vec<(String, CHIRType)>,
    pub submodules: Vec<SHIRSubmoduleInst>,
}

// ── Combinational body ────────────────────────────────────────────────────────

/// Combinational body as a flat list of statements.
/// Statements are either `Wire` (intermediate value) or `PortDrive` (output),
/// preserving their source order and conditional structure.
#[derive(Debug)]
pub struct SHIRCombBody {
    pub stmts: Vec<SHIRStmt>,
}

// ── Sequential body ───────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct SHIRSeqBody {
    pub clock: String,
    pub registers: Vec<SHIRReg>,
    /// `Memory<..>` instances — 1:1 with `CHIRSeqBody::memories`.
    pub memories: Vec<SHIRMemory>,
    /// One entry per phase (tick boundary).
    /// Single-tick modules have exactly one phase (phase_idx = 0).
    pub phases: Vec<SHIRPhase>,
    // Output port drives are embedded in each phase's pre_edge as
    // SHIRStmt::PortDrive, possibly inside If/Match for conditional drives.
}

#[derive(Debug)]
pub struct SHIRReg {
    pub name: String,
    pub ty: CHIRType,
    pub init: Option<SHIRLit>,
    /// See `CHIRRegDecl::init_param`.
    pub init_param: Option<String>,
}

/// A memory array instance. Mirrors `CHIRMemoryDecl` minus the span.
#[derive(Debug)]
pub struct SHIRMemory {
    pub name: String,
    pub elem_ty: CHIRType,
    pub depth: usize,
    pub read_ports: usize,
    pub write_ports: usize,
    /// Port latencies — see `CHIRMemoryDecl`.
    pub read_lat: usize,
    pub write_lat: usize,
    /// Preloaded contents — see `CHIRMemInit`.
    pub init: Option<SHIRMemInit>,
    /// Read-during-write ordering — see `CHIRMemoryDecl::write_mode`.
    pub write_mode: crate::memory::WriteMode,
    /// See `CHIRMemoryDecl::received` — a `Memory<…>` PARAMETER, lowered to bus
    /// ports with the array on the owner's side.
    pub received: bool,
}

/// 1:1 with `CHIRMemInit`, over `SHIRExpr`.
#[derive(Debug)]
pub enum SHIRMemInit {
    Fill { var: String, value: SHIRExpr },
    Words(Vec<SHIRExpr>),
}

/// A `#[hardware]` submodule instance. The output_wire is a combinational
/// wire available in all timing regions.
#[derive(Debug)]
pub struct SHIRSubmoduleInst {
    pub inst_name: String,
    pub module_name: String,
    /// `(child_clock_port, parent_clock_signal)` — see `CHIRSubmoduleInst::clocks`.
    pub clocks: Vec<(String, String)>,
    /// `(child_port, net_name)` — see `CHIRSubmoduleInst::port_nets`.
    pub port_nets: Vec<(String, String)>,
}

// ── Phase ─────────────────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct SHIRPhase {
    pub phase_idx: usize,
    /// Combinational statements before the clock edge.
    /// Includes Wire (intermediate values) and PortDrive (output port drives).
    pub pre_edge: Vec<SHIRStmt>,
    /// The trailing region's register updates (phase 0 only): emitted in
    /// `always_ff` under `if (<guard>)` AHEAD of `post_edge`, the guard being the
    /// one-bit register that is 0 until the first edge — the first resumption runs
    /// the head only (SPEC I.0.2). `post_edge` reads these registers through the
    /// guarded value (`guard ? update : register`), so a later update of the same
    /// register overrides as the program order says.
    pub guarded_post_edge: Option<(SHIRExpr, Vec<SHIRRegUpdate>)>,
    /// Register next-value updates — take effect at the clock edge.
    pub post_edge: Vec<SHIRRegUpdate>,
}

#[derive(Debug)]
pub struct SHIRRegUpdate {
    pub target: String,
    pub next_value: SHIRExpr,
}

// ── Statements ────────────────────────────────────────────────────────────────

#[derive(Debug)]
pub enum SHIRStmt {
    /// Combinational wire declaration (intermediate value).
    Wire {
        name: String,
        ty: CHIRType,
        value: SHIRExpr,
    },
    /// Drive an output port. May appear flat or inside If/Match for
    /// conditional drives (e.g. a Moore output decoded from a state `match`).
    ///
    /// One value. On a clocked body every output is a `RegOut` (SPEC II.1 R8),
    /// emitted as a non-blocking assignment in `always_ff`, so the value is the
    /// drive in PRE-edge terms: registers the resumption has already assigned
    /// stand for the values it assigned them, and its `let` wires are inlined
    /// where that matters (`shir_lower::Forwarding`; without it
    /// `n = n + step; acc.write(n)` and its reverse emit the same SystemVerilog —
    /// `TODO` cause L). On the combinational path the forwarding is the
    /// identity, so the value is the drive as written. Until Phase C step 4
    /// (2026-11-06) a drive carried a second, unforwarded form and an
    /// opening/closing tag for the plain-`Out` emission forms of a clocked body,
    /// which R8 made unreachable (`design_docs/review/MEASUREMENTS.md` §H).
    PortDrive {
        port_name: String,
        value: SHIRExpr,
    },
    /// A branch carries its test in BOTH forms: the statements inside it are
    /// split between `always_comb` (its wires, read after the edge) and
    /// `always_ff` (its port drives, sampled before it), and the two copies read
    /// the segment's registers in different windows. `vlir_lower::split_output_reg`
    /// makes that split and picks the matching test for each half.
    ///
    /// A test that reads a register this segment has ALREADY assigned is the case
    /// that separates them — `c = c + 1; if c == 3 { … }` means the incremented
    /// `c` in the simulator, and an `always_ff` copy testing the unforwarded `c`
    /// silently fires a cycle late (or, with `c` reset inside the branch, never at
    /// all). See `TODO` cause N.
    If {
        condition: SHIRExpr,
        /// `condition` in pre-edge terms — see the note above.
        edge_condition: SHIRExpr,
        then_stmts: Vec<SHIRStmt>,
        else_stmts: Option<Vec<SHIRStmt>>,
    },
    /// See [`SHIRStmt::If`] for why the scrutinee is carried twice.
    Match {
        scrutinee: SHIRExpr,
        /// `scrutinee` in pre-edge terms — see [`SHIRStmt::If`].
        edge_scrutinee: SHIRExpr,
        arms: Vec<SHIRMatchArm>,
    },
    /// `for <var> in <start>..<end>` (exclusive; `..=` when `inclusive`, D-27), emitted
    /// as an SV `for`.
    ForLoop {
        var: String,
        start: SHIRExpr,
        end: SHIRExpr,
        inclusive: bool,
        body: Vec<SHIRStmt>,
    },
    /// `base[index] = value;` — single-bit assignment.
    IndexAssign {
        base: String,
        index: SHIRExpr,
        value: SHIRExpr,
    },
    /// Stage a read address on a memory read port — see `CHIRStmt::MemRead`.
    MemRead {
        mem: String,
        port: usize,
        addr: SHIRExpr,
    },
    /// Stage a write on a memory write port — see `CHIRStmt::MemWrite`.
    MemWrite {
        mem: String,
        port: usize,
        addr: SHIRExpr,
        value: SHIRExpr,
    },
}

#[derive(Debug)]
pub struct SHIRMatchArm {
    pub patterns: Vec<SHIRPattern>,
    pub guard: Option<SHIRExpr>,
    pub stmts: Vec<SHIRStmt>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SHIRPattern {
    Lit(SHIRLit),
    Wildcard,
    Tuple(Vec<SHIRPattern>),
    EnumVariant { name: String, inner: Option<Box<SHIRPattern>> },
    /// A named constant (`localparam` / parameter) as a case label — see
    /// [`crate::chir::CHIRPattern::Const`].
    Const(String),
}

// ── Expression model ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum SHIRExpr {
    Var(String),
    Lit(SHIRLit),
    /// All-X literal (CHIR `XLit`).
    XLit { width: Option<crate::chir::Width> },
    BinOp {
        left: Box<SHIRExpr>,
        op: CHIRBinOp,
        right: Box<SHIRExpr>,
    },
    UnOp {
        op: CHIRUnOp,
        expr: Box<SHIRExpr>,
    },
    Mux {
        cond: Box<SHIRExpr>,
        then_val: Box<SHIRExpr>,
        else_val: Box<SHIRExpr>,
    },
    Case {
        scrutinee: Box<SHIRExpr>,
        arms: Vec<SHIRCaseArm>,
        default: Box<SHIRExpr>,
    },
    Concat(Vec<SHIRExpr>),
    /// Signedness reinterpretation — see `CHIRExpr::SignCast`; passes through
    /// this stage unchanged.
    SignCast {
        signed: bool,
        expr: Box<SHIRExpr>,
    },
    Slice {
        expr: Box<SHIRExpr>,
        high: usize,
        low: usize,
    },
    /// Single-bit select at a dynamic index — `base[index]`.
    DynBit {
        base: Box<SHIRExpr>,
        index: Box<SHIRExpr>,
    },
    /// Width-cast — `width'(expr)`.
    Resize {
        expr: Box<SHIRExpr>,
        width: Width,
    },
    /// Compares phase_r == idx. Used in multi-phase post_edge conditions.
    PhaseEq(usize),
    /// `mem.read_port::<port>().data()` — the read port's output value.
    MemData { mem: String, port: usize },
    /// `mem.read_port::<port>().is_ready()` — the read port's output-valid flag.
    MemValid { mem: String, port: usize },
    /// Combinational memory-array word read — see `CHIRExpr::MemIndex`.
    MemIndex { mem: String, addr: Box<SHIRExpr> },
}

#[derive(Debug, Clone, PartialEq)]
pub struct SHIRCaseArm {
    pub pattern: SHIRPattern,
    pub guard: Option<SHIRExpr>,
    pub value: SHIRExpr,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SHIRLit {
    pub ty: CHIRType,
    pub value: u128,
}

// ── Error model ───────────────────────────────────────────────────────────────

#[derive(Debug)]
pub enum SHIRLowerError {
    /// `AwaitTick` appeared inside an `If` or `Match` arm.
    TickInsideBranch { span: SourceSpan },
    /// Sequential module has no `AwaitTick`.
    NoTick { span: SourceSpan },
    /// `AwaitTick` references a different clock than declared.
    CrossClockTick { expected: String, found: String, span: SourceSpan },
    /// A construct not supported at Phase C level.
    UnsupportedConstruct { description: String, span: SourceSpan },
}

impl SHIRLowerError {
    pub fn span(&self) -> &SourceSpan {
        match self {
            SHIRLowerError::TickInsideBranch { span } => span,
            SHIRLowerError::NoTick { span } => span,
            SHIRLowerError::CrossClockTick { span, .. } => span,
            SHIRLowerError::UnsupportedConstruct { span, .. } => span,
        }
    }
}

impl std::fmt::Display for SHIRLowerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let span = self.span();
        write!(f, "{}:{}: ", span.start_line, span.start_col)?;
        match self {
            SHIRLowerError::TickInsideBranch { .. } =>
                write!(f, "clk.tick().await inside a conditional branch is not supported"),
            SHIRLowerError::NoTick { .. } =>
                write!(f, "sequential module has no clk.tick().await"),
            SHIRLowerError::CrossClockTick { expected, found, .. } =>
                write!(f, "tick on clock '{}' but module uses clock '{}'", found, expected),
            SHIRLowerError::UnsupportedConstruct { description, .. } =>
                write!(f, "unsupported construct: {}", description),
        }
    }
}
