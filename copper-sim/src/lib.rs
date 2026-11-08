use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

pub mod verification;
pub use verification::{
    is_missing_verilator, sha256_hex, verify_with_verilator, verify_with_verilator_report,
    verilator_cache_root, verilator_status, verilator_version, CacheOutcome, CycleData,
    SimulationTrace, VerilatorCachePolicy, VerilatorPowerOn, VerilatorRun, VERILATOR_NOT_INSTALLED,
};

pub mod testing;
pub use testing::{HardwareTest, TestResult, make_cycle};

pub mod executor;

pub use executor::{
    __instance, HardwareExecutor, HardwareModule, PollOrder, SchedulerMode, UNSTARTED_MODULE_MESSAGE,
};

/// A future that suspends for exactly one delta cycle, then resumes.
///
/// On the first poll it returns `Pending`, allowing other tasks in the same
/// delta-cycle pass to run.  On the next pass the executor polls it again and
/// it returns `Ready(())`, so the caller continues immediately.
///
/// This is the building block for writing purely combinational modules that
/// re-evaluate every delta cycle instead of every clock edge.  It is also
/// what makes the delta-cycle limitation observable: a module that drives an
/// output to a new value unconditionally before `delta_yield().await` will mark
/// the signal dirty on *every* pass, preventing the executor from ever detecting
/// a fixed point and causing it to panic at the oscillation threshold.
pub struct DeltaYield {
    yielded: bool,
}

impl Future for DeltaYield {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<()> {
        if self.yielded {
            Poll::Ready(())
        } else {
            self.yielded = true;
            Poll::Pending
        }
    }
}

/// Suspend for one delta cycle and resume on the next executor pass.
///
/// See [`DeltaYield`] for full semantics.
pub fn delta_yield() -> DeltaYield {
    DeltaYield { yielded: false }
}

// `PreEdgeBarrier` / `pre_edge_barrier` (the parking point the macro injected
// before a deferred input read) and the per-domain `POLL_PHASE` it read were
// deleted with the two-job executor (Phase C step 3, D-8): under one resumption
// per tick every read sees `x_{N+1}` with no barrier.

// The `spawn_child!` macro (a spawn recording a parent/child relationship) was
// deleted in Phase C step 11b with `HardwareExecutor::spawn_child` (SPEC I.13.5, D-23).

