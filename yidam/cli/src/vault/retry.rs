//! One request, sent again when what failed was the connection rather than the answer.
//!
//! # Why the retry is per request and not per run
//!
//! `vault push` is already resumable: it HEADs each artifact before sending and skips what
//! the store holds, so a second invocation continues rather than restarts. That property is
//! what makes retrying *here* the right place rather than around the whole command — a reset
//! while sending one object says nothing about the next one, and re-walking the catalog to
//! discover that costs a HEAD per artifact to learn nothing.
//!
//! # Why the pause is injected
//!
//! A backoff that a test has to wait through is a backoff no test exercises. [`Policy`] takes
//! the pause as a closure, so the schedule is asserted by recording what it asked for. This
//! is the shape [`crate::s3vectors::ops::Session`] arrived at against the same provider; the
//! two are not one type because the error they classify is not one type — that one reads S3
//! Vectors' JSON exceptions, and this one reads S3's statuses and the transport underneath.

use std::time::Duration;

/// What a failed request says about trying it again.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fault {
    /// The connection, not the answer — a reset, a broken pipe, a timeout, a 503. Another
    /// attempt opens a different connection, and the bytes it was sending have not moved.
    Transient,
    /// The server answered and the answer will not change — a 403, a 404, a signature it
    /// disagrees with. Retrying spends the latency twice to be told the same thing.
    Permanent,
}

/// A failed attempt: what went wrong, and whether repeating it could help.
pub struct Failed {
    pub fault: Fault,
    pub error: anyhow::Error,
}

impl Failed {
    pub fn of(fault: Fault, error: anyhow::Error) -> Self {
        Self { fault, error }
    }

    /// A failure worth another attempt.
    pub fn transient(error: anyhow::Error) -> Self {
        Self::of(Fault::Transient, error)
    }
}

/// Anything not classified is permanent.
///
/// Deliberately this direction. The failures inside a request that are *not* the transport —
/// opening the file to upload, signing, parsing an endpoint — are all of them permanent, and
/// they reach this type through `?`. Defaulting the other way would retry a missing file
/// three times before reporting it.
impl From<anyhow::Error> for Failed {
    fn from(error: anyhow::Error) -> Self {
        Self {
            fault: Fault::Permanent,
            error,
        }
    }
}

/// What one attempt returns.
pub type Attempt<T> = std::result::Result<T, Failed>;

/// Total attempts per request, so `4` is three retries.
///
/// Sized against the failure that prompted it: a first push of a populated corpus took six
/// invocations of `vault push` to finish, every one of them stopped by a connection reset.
/// Those resets are independent — a fresh connection clears one — so three retries turn a
/// run that needs six attempts into one that needs one, while still bounding the wait a
/// genuinely unreachable store can impose.
pub const ATTEMPTS: u32 = 4;

/// How long to wait before the first retry. Each subsequent wait doubles.
///
/// Doubling rather than a fixed pause because the two transient faults want different
/// answers: a reset wants a prompt reconnect, and a `503 SlowDown` wants the opposite of
/// promptness. Starting short and growing serves the first without ignoring the second, and
/// caps the added latency of a hopeless request at 1.75s.
pub const FIRST_PAUSE: Duration = Duration::from_millis(250);

static SLEEP: fn(Duration) = std::thread::sleep;

/// How many times to send a request, and how long to wait between.
pub struct Policy<'a> {
    pause: &'a dyn Fn(Duration),
    attempts: u32,
    first_pause: Duration,
}

impl Default for Policy<'_> {
    fn default() -> Self {
        Self {
            pause: &SLEEP,
            attempts: ATTEMPTS,
            first_pause: FIRST_PAUSE,
        }
    }
}

impl<'a> Policy<'a> {
    /// A policy with the waiting replaced — for a test, which must not sleep.
    ///
    /// `cfg(test)` because there is no second production caller and `dead_code` is this
    /// crate's registry guard: a constructor nothing reaches should go red rather than sit.
    #[cfg(test)]
    pub fn with(pause: &'a dyn Fn(Duration), attempts: u32) -> Self {
        Self {
            pause,
            attempts,
            first_pause: FIRST_PAUSE,
        }
    }

    /// Send, repeating a transient failure until it clears or the attempts run out.
    pub fn run<T>(&self, mut attempt: impl FnMut() -> Attempt<T>) -> anyhow::Result<T> {
        // Written so the last failure is returned from where it happened. Accumulating it
        // into an `Option` and unwrapping after the loop needs a panic path to express "a
        // loop that ran at least once recorded something", and `panic_paths.rs` ratchets
        // those.
        let mut spent = 0;
        let mut wait = self.first_pause;
        loop {
            match attempt() {
                Ok(v) => return Ok(v),
                Err(f) => {
                    spent += 1;
                    if spent >= self.attempts || f.fault == Fault::Permanent {
                        return Err(f.error);
                    }
                    (self.pause)(wait);
                    wait *= 2;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    /// A store that fails the first `failures` calls and then answers.
    fn flaky(failures: usize, fault: Fault) -> impl FnMut() -> Attempt<usize> {
        let mut calls = 0usize;
        move || {
            calls += 1;
            if calls <= failures {
                Err(Failed {
                    fault,
                    error: anyhow::anyhow!("Connection reset by peer (os error 54)"),
                })
            } else {
                Ok(calls)
            }
        }
    }

    #[test]
    fn a_reset_that_clears_is_not_the_caller_s_problem() {
        let slept: RefCell<Vec<Duration>> = RefCell::new(Vec::new());
        let pause = |d: Duration| slept.borrow_mut().push(d);
        let got = Policy::with(&pause, ATTEMPTS)
            .run(flaky(2, Fault::Transient))
            .unwrap();
        assert_eq!(got, 3, "the third attempt is the one that answered");
        assert_eq!(
            *slept.borrow(),
            vec![Duration::from_millis(250), Duration::from_millis(500)],
            "one wait per retry, doubling"
        );
    }

    #[test]
    fn a_store_that_never_answers_stops_rather_than_waiting_forever() {
        let slept: RefCell<Vec<Duration>> = RefCell::new(Vec::new());
        let pause = |d: Duration| slept.borrow_mut().push(d);
        let e = Policy::with(&pause, ATTEMPTS)
            .run(flaky(usize::MAX, Fault::Transient))
            .unwrap_err();
        assert!(e.to_string().contains("Connection reset"));
        assert_eq!(
            slept.borrow().len(),
            (ATTEMPTS - 1) as usize,
            "the last attempt is not followed by a wait"
        );
        assert_eq!(
            slept.borrow().iter().sum::<Duration>(),
            Duration::from_millis(250 + 500 + 1000)
        );
    }

    #[test]
    fn a_denial_is_reported_once_rather_than_argued_with() {
        let slept: RefCell<Vec<Duration>> = RefCell::new(Vec::new());
        let pause = |d: Duration| slept.borrow_mut().push(d);
        let mut calls = 0usize;
        let e = Policy::with(&pause, ATTEMPTS)
            .run(|| {
                calls += 1;
                Err::<(), _>(Failed::from(anyhow::anyhow!("HTTP 403 AccessDenied")))
            })
            .unwrap_err();
        assert_eq!(calls, 1, "a 403 retried is a 403");
        assert!(slept.borrow().is_empty());
        assert!(e.to_string().contains("403"));
    }

    #[test]
    fn a_failure_nothing_classified_is_permanent() {
        let f = Failed::from(anyhow::anyhow!("opening the file to upload"));
        assert_eq!(f.fault, Fault::Permanent);
        assert_eq!(
            Failed::transient(anyhow::anyhow!("x")).fault,
            Fault::Transient
        );
    }
}
