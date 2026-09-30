//! A call budget, so a committed script that loops is CI that fails rather than CI that hangs.
//!
//! RFC-0042 §"A budget through the VM hook": the counterpart of the timer RFC-0024 gave
//! `regorus`, and in calls rather than wall-clock for a stated reason. A call count is a
//! function of the corpus and the script; a deadline is a function of the machine. A run whose
//! refusal depends on which runner picked it up is a run whose receipt means less than it
//! claims, and a receipt that means less is the one thing RFC-0026 exists to prevent.
//!
//! Gluon's hook is per call rather than per instruction, which is the right granularity for
//! this and not merely the available one: a gluon script cannot loop without calling something.
//! There is no `while`, so recursion is the only repetition, and recursion is a call.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::task::Poll;

use gluon::vm::thread::{HookFlags, ThreadInternal};
use gluon::vm::Error as VmError;

/// The default number of calls a calculator may make.
///
/// Measured against the shape a calculator actually has: the worked example in
/// `tests/gluon_arm.rs` folds two properties across two nodes in 75 calls, so this is roughly
/// four orders of magnitude of headroom over a small corpus and two over a large one. It is a
/// constant here rather than a manifest field because whether a corpus may raise its own budget
/// is RFC-0042's open question 4 and not something to settle by shipping a field.
pub const DEFAULT_CALLS: usize = 5_000_000;

/// A budget installed on a VM, still readable after the run.
///
/// Kept so that the count is reportable whether the run finished or was interrupted — a step
/// refused at the budget should be able to say what the budget was, and a step that finished
/// near it is the warning that the next corpus will not.
pub struct Budget {
    calls: Arc<AtomicUsize>,
    limit: usize,
}

impl Budget {
    /// Install a hook on `vm` that interrupts it past `limit` calls.
    ///
    /// The hook is set through the thread's context and masked to `CALL_FLAG`, so nothing is
    /// charged for anything but a call. `Error::Interrupted` is what gluon's own `Thread`
    /// returns for a hook that declines to continue, so a caller matching on it is matching on
    /// the engine's vocabulary rather than one invented here.
    pub fn install(vm: &gluon::Thread, limit: usize) -> Self {
        let calls = Arc::new(AtomicUsize::new(0));
        let seen = Arc::clone(&calls);
        let mut ctx = vm.context();
        ctx.set_hook(Some(Box::new(move |_, _| {
            if seen.fetch_add(1, Ordering::Relaxed) >= limit {
                Poll::Ready(Err(VmError::Interrupted))
            } else {
                Poll::Ready(Ok(()))
            }
        })));
        ctx.set_hook_mask(HookFlags::CALL_FLAG);
        Self { calls, limit }
    }

    /// Calls charged so far.
    pub fn spent(&self) -> usize {
        self.calls.load(Ordering::Relaxed)
    }

    /// Whether the budget is what stopped the run.
    pub fn exhausted(&self) -> bool {
        self.spent() > self.limit
    }

    pub fn limit(&self) -> usize {
        self.limit
    }
}
