//! The one tokio runtime, and the synchronous façade every transport reaches it through.
//!
//! # Why one
//!
//! Before #930 there were six: `index-build` and `tonpa` each built one at the top of the
//! command, `catalog fetch` built one per call, the HTTP server built its own, and the S3 and
//! S3 Vectors transports each **owned** one in a struct field and called `block_on` on it.
//! The last shape is a hazard rather than a cost. `Runtime::block_on` panics on a thread that
//! is already inside a runtime — *"Cannot start a runtime from within a runtime"* — and the
//! HTTP server is exactly such a thread. `serve --mcp --http` over a repository with
//! `[index.remote]` reaches the S3 Vectors transport from inside a connection task, and under
//! `--features full` that `retrieve` panicked; nothing outside the full build could see it,
//! because the light build has no remote arm to reach.
//!
//! One runtime, built on first use, means a transport has nothing to own. What it needs is a
//! way to run a future to completion from synchronous code, and that is [`block_on`].
//!
//! # Why the façade stays synchronous
//!
//! `vault::store` gives the reason and it is not repeated: an async trait would put `tokio` in
//! the signature of every caller and therefore in the ungated half of the vault and the S3
//! Vectors module, spending what the feature split bought. So the seam is a function from a
//! future to its output, and *where the caller is standing* is this module's problem rather
//! than the caller's.
//!
//! # From inside the runtime
//!
//! A thread already driving the runtime cannot block on it — that is tokio's rule, and it is
//! the right one, since a parked driver thread drives nothing. So a call to [`block_on`] from
//! such a thread hands the future to a scoped thread, which blocks on the runtime from
//! outside it while the runtime's own worker carries the I/O. The calling thread waits on the
//! join, which is a blocking wait inside a task: the HTTP server's one thread serves nothing
//! else for the duration of the round trip. That is the same shape the server already has for
//! embedding a query — seconds of synchronous work on the same thread — and its module header
//! says why one thread was chosen. A transport call adds a network round trip to that, not a
//! new kind of stall.
//!
//! Scoped rather than spawned so the future can borrow: every transport's future holds
//! references into the request it is sending, and forcing those to `'static` would mean
//! cloning a signed request to send it.

use std::future::Future;
use std::sync::LazyLock;

use tokio::runtime::{Handle, Runtime};

static RUNTIME: LazyLock<Runtime> = LazyLock::new(|| {
    // Multi-threaded with tokio's default worker count, which is what `index-build` had
    // before through `Runtime::new()` — lancedb spreads its work across the workers, and a
    // single-worker runtime here would be a slowdown nobody measured. The other users make
    // one request at a time and leave the pool parked; that costs a few idle threads, once.
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("building the tokio runtime")
});

/// Run `fut` to completion from synchronous code, wherever that code is standing.
///
/// From a plain thread this is `Runtime::block_on`. From a thread already inside the runtime
/// — a connection task in the HTTP server — it is the same call made from a scoped thread,
/// because tokio refuses to block a thread that is driving it. The module header says what
/// that costs.
///
/// `Send` on the future is the price of the second path: a future that may be handed to
/// another thread has to be able to cross to it. Every transport's future is.
pub fn block_on<F>(fut: F) -> F::Output
where
    F: Future + Send,
    F::Output: Send,
{
    if Handle::try_current().is_ok() {
        std::thread::scope(|s| {
            s.spawn(|| RUNTIME.block_on(fut))
                .join()
                .unwrap_or_else(|panic| std::panic::resume_unwind(panic))
        })
    } else {
        RUNTIME.block_on(fut)
    }
}

/// Run a future that cannot cross threads, from a thread that is not inside the runtime.
///
/// The HTTP server's future is one: its state is behind `Rc` on a `LocalSet`, for the reason
/// `cmd::serve::http::serve` gives. Only a command's top level should call this; from inside
/// the runtime it panics with tokio's own message, which is what [`block_on`] exists to avoid.
pub fn block_on_local<F: Future>(fut: F) -> F::Output {
    RUNTIME.block_on(fut)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_plain_thread_blocks_on_the_runtime_directly() {
        assert_eq!(block_on(async { 1 + 1 }), 2);
    }

    /// The shape #930 named: a synchronous façade reached from inside a task on the runtime.
    /// Before this module that was an owned runtime's `block_on` inside another runtime's, and
    /// tokio panics on it. The inner future yields to the scheduler before it resolves, so
    /// the test proves the inner call is driven, not merely that it returns.
    #[test]
    fn a_task_on_the_runtime_can_block_on_it_without_panicking() {
        let out = block_on_local(async {
            block_on(async {
                tokio::task::yield_now().await;
                "driven"
            })
        });
        assert_eq!(out, "driven");
    }

    /// The server's exact shape: a `LocalSet` with a `!Send` task, which calls the façade.
    #[test]
    fn a_local_set_task_can_block_on_the_runtime() {
        let local = tokio::task::LocalSet::new();
        let out = block_on_local(local.run_until(async {
            let rc = std::rc::Rc::new(3);
            let rc2 = std::rc::Rc::clone(&rc);
            tokio::task::spawn_local(async move {
                // The `Rc` stays on this side of the seam; what crosses is `Send`.
                let base = *rc2;
                let n = block_on(async {
                    tokio::task::yield_now().await;
                    base * 2
                });
                n + *rc2
            })
            .await
            .unwrap()
        }));
        assert_eq!(out, 9);
    }

    /// The façade borrows: a future holding a reference into the caller's frame crosses to
    /// the scoped thread and back. This is the property that lets a transport send a signed
    /// request without cloning it.
    #[test]
    fn a_borrowing_future_crosses_the_scoped_thread() {
        let request = String::from("signed");
        let out = block_on_local(async {
            block_on(async {
                tokio::task::yield_now().await;
                request.len()
            })
        });
        assert_eq!(out, 6);
    }

    /// A panic inside the future surfaces on the caller's thread with its payload, rather than
    /// as a `JoinError` a caller never asked for.
    #[test]
    fn a_panic_in_the_future_reaches_the_caller() {
        let caught = std::panic::catch_unwind(|| {
            block_on_local(async {
                block_on(async {
                    tokio::task::yield_now().await;
                    panic!("from inside");
                })
            })
        });
        let payload = caught.unwrap_err();
        assert_eq!(payload.downcast_ref::<&str>(), Some(&"from inside"));
    }
}
