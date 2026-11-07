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
    HardwareExecutor, HardwareModule, ModuleInfo, PollOrder, SchedulerMode,
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

/// A helper macro to spawn a child module's future and track the parent-child
/// relationship in the executor.
///
/// The trailing `reads` argument is the child's input wire-ids (`vec![
/// port.wire_id(), .. ]`), recorded for the item-6 dependency graph. Phase 1
/// records only — it has no effect on the current scheduler.
#[macro_export]
macro_rules! spawn_child {
    ($exec:expr, $parent:expr, $module_future:expr, $reads:expr) => {{
        $exec.spawn_child(stringify!($module_future), $parent, $module_future, $reads)
    }};
    ($exec:expr, $parent:expr, $child_name:expr, $module_future:expr, $reads:expr) => {{
        $exec.spawn_child($child_name, $parent, $module_future, $reads)
    }};
}

