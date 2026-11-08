//! The memory model of SPEC §I.10 (D-16) — derived from the clause text, not from
//! the simulator's `copper-core/src/memory.rs` or the transpiler's lowering, so the
//! reference leg can disagree with either (D-16's first Phase C item; D-20 item 9).
//!
//! A `Memory` is a variable of H(M) of its own kind (I.10.1(a)): an array `A` of
//! `depth` words and, per read port `k`, a delivery pipeline `P_k` of `L` stages;
//! per write port `j`, a write pipeline `Q_j` of `M` stages. It is not a local and
//! not a register (D-9): the walker keeps it beside the environment, and only the
//! edge changes it (I.1.3).
//!
//! | clause            | here                                                         |
//! |-------------------|--------------------------------------------------------------|
//! | I.10.1(a)         | [`MemState::new`]: `T`, `R`, `W`, `L ≥ 1`, `M ≥ 1`, `depth`  |
//! | I.10.1(b)         | [`MemState::stage_read`], [`MemState::stage_write`]: an effect of the resumption, last staging wins, out of range a run-time failure |
//! | I.10.1(c), I.10.2 | [`MemState::edge`]: the read step and the write step in the mode's order |
//! | I.10.1(d)         | [`MemState::is_ready`], [`MemState::data`]: committed state — what the last edge delivered; `data()` unready a run-time failure (D-16 1A) |
//! | I.10.3            | the initial array: all zero (`new`), listed (`from_contents`), `A[i] = ⟦e⟧(i)` (`from_fn`); every pipeline empty |

use crate::value::{Ty, Value};
use crate::RefError;

/// The read-during-write mode of the instance (I.10.2): the order of the two steps
/// of the edge. Declared by the builder and inferred from nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// The read step, then the write step: a read of an address committed at the
    /// same edge delivers the word `A` held before it.
    ReadFirst,
    /// The write step, then the read step: it delivers the committed word.
    WriteFirst,
}

/// One memory instance's state (I.10.1(a)) and the stagings of the current
/// resumption (I.10.1(b), consumed at the edge).
#[derive(Clone, Debug)]
pub struct MemState {
    pub name: String,
    pub word: Ty,
    pub mode: Mode,
    pub read_lat: usize,
    pub write_lat: usize,
    array: Vec<Value>,
    /// `s_k`: the address staged on read port `k` this resumption, if any.
    staged_read: Vec<Option<usize>>,
    /// `P_k`: stage 0 first, stage `L-1` the output stage.
    read_pipe: Vec<Vec<Option<Value>>>,
    /// `w_j`: the write staged on write port `j` this resumption, if any.
    staged_write: Vec<Option<(usize, Value)>>,
    /// `Q_j`: stage 0 first, stage `M-1` the committing stage.
    write_pipe: Vec<Vec<Option<(usize, Value)>>>,
}

fn rt(s: impl Into<String>) -> RefError {
    RefError::Runtime(s.into())
}

impl MemState {
    /// I.10.1(a) and I.10.3: the instance with its initial array. Every pipeline
    /// starts empty — nothing has been staged before the entry.
    pub fn new(
        name: &str,
        word: Ty,
        reads: usize,
        writes: usize,
        read_lat: usize,
        write_lat: usize,
        mode: Mode,
        init: Vec<Value>,
    ) -> Result<MemState, RefError> {
        // I.10.1(a): `L = 0` is OUTSIDE (no combinational read); `M ≥ 1`.
        if read_lat == 0 {
            return Err(RefError::Unsupported(format!(
                "memory `{name}`: READ_LAT = 0 is a combinational read, OUTSIDE (SPEC I.10.1(a))"
            )));
        }
        if write_lat == 0 {
            return Err(RefError::Unsupported(format!("memory `{name}`: WRITE_LAT = 0 (SPEC I.10.1(a): M ≥ 1)")));
        }
        let init = init.into_iter().map(|v| v.coerce_to(&word)).collect::<Result<Vec<_>, _>>()?;
        Ok(MemState {
            name: name.to_string(),
            word,
            mode,
            read_lat,
            write_lat,
            array: init,
            staged_read: vec![None; reads],
            read_pipe: vec![vec![None; read_lat]; reads],
            staged_write: vec![None; writes],
            write_pipe: vec![vec![None; write_lat]; writes],
        })
    }

    pub fn depth(&self) -> usize {
        self.array.len()
    }

    fn check_addr(&self, kind: &str, port: usize, a: usize) -> Result<(), RefError> {
        // I.10.1(b), I.14.6(b): an out-of-range address is a run-time failure.
        if a >= self.array.len() {
            return Err(rt(format!(
                "memory `{}` {kind} port {port}: address {a} is out of range for depth {} (SPEC I.10.1(b), I.14.6(b))",
                self.name,
                self.array.len()
            )));
        }
        Ok(())
    }

    fn read_port(&self, k: usize) -> Result<(), RefError> {
        if k >= self.staged_read.len() {
            return Err(rt(format!(
                "memory `{}`: read port {k} of {} (I.14.6: `k ≥ R` is a shape error)",
                self.name,
                self.staged_read.len()
            )));
        }
        Ok(())
    }

    /// `stage m k a` (I.10.1(b)): `s_k := a`. A later staging of the same port in
    /// the same resumption replaces it — the last wins (D-16 (b) B).
    pub fn stage_read(&mut self, k: usize, a: usize) -> Result<(), RefError> {
        self.read_port(k)?;
        self.check_addr("read", k, a)?;
        self.staged_read[k] = Some(a);
        Ok(())
    }

    /// `stage_write m j a e` (I.10.1(b)): `w_j := (a, e)`, last wins.
    pub fn stage_write(&mut self, j: usize, a: usize, e: Value) -> Result<(), RefError> {
        if j >= self.staged_write.len() {
            return Err(rt(format!(
                "memory `{}`: write port {j} of {} (I.14.6: `k ≥ W` is a shape error)",
                self.name,
                self.staged_write.len()
            )));
        }
        self.check_addr("write", j, a)?;
        let e = e.coerce_to(&self.word)?;
        self.staged_write[j] = Some((a, e));
        Ok(())
    }

    /// `is_ready()` (I.10.1(d)): port `k`'s output stage holds a word — what the
    /// last edge delivered; this resumption's staging is not seen.
    pub fn is_ready(&self, k: usize) -> Result<bool, RefError> {
        self.read_port(k)?;
        Ok(self.read_pipe[k][self.read_lat - 1].is_some())
    }

    /// `data()` (I.10.1(d)): the output stage's word; when it holds none, a
    /// run-time failure (D-16 1A) — the program has no word to read.
    pub fn data(&self, k: usize) -> Result<Value, RefError> {
        self.read_port(k)?;
        self.read_pipe[k][self.read_lat - 1].clone().ok_or_else(|| {
            rt(format!(
                "memory `{}` read port {k}: data() of an undelivered word — the output stage is empty \
                 (SPEC I.10.1(d), D-16 1A: a run-time failure)",
                self.name
            ))
        })
    }

    /// The read step of I.10.1(c): every `P_k` shifts one stage toward its output
    /// and stage 0 captures `A[s_k]` if `s_k` was staged, else nothing; `s_k` is
    /// consumed. An unstaged port's output stage therefore empties one edge on.
    fn read_step(&mut self) {
        for k in 0..self.read_pipe.len() {
            for i in (1..self.read_lat).rev() {
                self.read_pipe[k][i] = self.read_pipe[k][i - 1].take();
            }
            self.read_pipe[k][0] = self.staged_read[k].take().map(|a| self.array[a].clone());
        }
    }

    /// The write step of I.10.1(c): every `Q_j` takes `w_j` into stage 0 and
    /// shifts, and its committing stage — the oldest, `M-1` — is written into `A`,
    /// in ascending port order, so of two ports committing one address the higher
    /// index wins. A write staged in the resumption ending at edge N+1 is thereby
    /// in `A` from edge N+M (the clause's stated consequence; see the note in
    /// `design_docs/review/MEASUREMENTS.md` §O.1 on the order of its two sentences).
    fn write_step(&mut self) {
        let mut commits = Vec::new();
        for j in 0..self.write_pipe.len() {
            let incoming = self.staged_write[j].take();
            for i in (1..self.write_lat).rev() {
                self.write_pipe[j][i] = self.write_pipe[j][i - 1].take();
            }
            self.write_pipe[j][0] = incoming;
            if let Some(c) = self.write_pipe[j][self.write_lat - 1].take() {
                commits.push(c);
            }
        }
        for (a, e) in commits {
            self.array[a] = e;
        }
    }

    /// The edge (I.10.1(c)), in the instance's mode order (I.10.2).
    pub fn edge(&mut self) {
        match self.mode {
            Mode::ReadFirst => {
                self.read_step();
                self.write_step();
            }
            Mode::WriteFirst => {
                self.write_step();
                self.read_step();
            }
        }
    }
}
