//! Containment for a panic out of gluon itself, so a script cannot abort the run.
//!
//! # The one class of failure the entry point's type does not cover
//!
//! [`super`]'s note argues that a typed calculator's failures are found before it runs: the macro
//! refusal decides what a script may reach, and the typecheck decides whether it is a calculator
//! at all. Both of those produce a refusal. Gluon's compiler has a third answer, which is neither
//! — `gluon_base`'s `ice!` macro is `panic!`, and a `.glu` can reach it.
//!
//! Measured on 0.18.4. A `rec let` group whose drawn-in binding is not a record or a variant
//! panics `gluon_vm::compiler` at *Expected record as last expression of recursive binding*, and
//! the ordinary way to write one is to leave out the `in`:
//!
//! ```text
//! rec let settle state =
//!     let round = relax state
//!     if round.changed then settle round.state else round.state
//!
//! let final = settle start
//! ```
//!
//! Gluon's layout draws `final` into the recursive group, the script typechecks, and the compiler
//! panics on it. That is an authoring mistake, not anything exotic, and it happens on the compile
//! path — before any evaluation — so a guard on `f.call` alone would never see it.
//!
//! # Why a panic here is not an acceptable outcome
//!
//! A capability's implementation is corpus data. A `.glu` arrives in a corpus the same way a `.sh`
//! calculator does, and a shell calculator that is nonsense gets a non-zero exit and a refusal
//! naming the step. An abort instead means the step is not refused by name, so the transcript does
//! not say which capability was at fault; a corpus author reads an invitation to file a bug
//! against a third-party compiler; and whatever [`crate::cmd::run::plan_and_write`] would have
//! done about a failed step does not happen, because nothing returns.
//!
//! # What the containment is, and what it is not
//!
//! [`contain`] is an unwind boundary and nothing more. It does not inspect the script's shape —
//! a check for the one layout known to reach `ice!` would be a denylist of today's gluon bugs,
//! which is the argument [`super::entry`] already declines to make about module names. Every
//! `ice!` in the engine is contained by the same boundary, including the ones nobody has found.
//!
//! The panic is still reported as gluon's: [`contain`] quotes the message and says whose it is.
//! Containment is not a claim that the compiler was right to panic.

use std::any::Any;
use std::cell::Cell;
use std::panic::{self, AssertUnwindSafe};
use std::sync::OnceLock;

use anyhow::{anyhow, Result};

/// How much of a panic payload is quoted back into the refusal.
///
/// The payload is not a sentence. `ice!` formats the offending expression with `{:?}`, and for the
/// script above that is several kilobytes of typed AST with raw pointer addresses in it — text
/// that would bury the refusal it is attached to and say nothing to a calculator author. The
/// first line carries the binding's name, which is the part that locates the mistake.
const QUOTED: usize = 240;

thread_local! {
    /// Whether the default panic hook should stay silent on this thread.
    static QUIET: Cell<bool> = const { Cell::new(false) };
}

/// Install, once per process, a hook that prints nothing while [`contain`] is on the stack.
///
/// The hook runs *before* the unwind reaches `catch_unwind`, so without this the ICE text and its
/// backtrace note reach stderr however well the panic is caught — and the invitation to file a
/// gluon bug is half of what makes the abort unreadable.
///
/// It is installed once and never removed, and the suppression is thread-local. A `set_hook` and
/// `take_hook` pair around each compile would be a data race against every other thread in the
/// process: the hook is global, this crate's own test suite runs in parallel, and a panic
/// elsewhere during the window would vanish. Here the window is one thread's, so a panic on any
/// other thread still prints exactly as it did.
fn install_quiet_hook() {
    static INSTALLED: OnceLock<()> = OnceLock::new();
    INSTALLED.get_or_init(|| {
        let default = panic::take_hook();
        panic::set_hook(Box::new(move |info| {
            if QUIET.with(Cell::get) {
                return;
            }
            default(info);
        }));
    });
}

/// Sets the thread's quiet flag and restores what it was, whichever way the scope leaves.
///
/// Restoring the previous value rather than clearing it keeps a nested [`contain`] honest, and
/// `Drop` rather than a trailing assignment is what makes the flag right on the unwind — which is
/// the only path that matters here.
struct Quiet(bool);

impl Quiet {
    fn hush() -> Self {
        Self(QUIET.with(|q| q.replace(true)))
    }
}

impl Drop for Quiet {
    fn drop(&mut self) {
        QUIET.with(|q| q.set(self.0));
    }
}

/// The part of a panic payload worth showing: the first line, without gluon's invitation.
///
/// `ice!` puts *Please report an issue at …* into every message it formats, and where it lands
/// depends on which arm was used: after the AST dump on the formatted arm, on the first line on the
/// one-argument arm, and as the whole message on the no-argument arm. Cut at the sentence rather
/// than at a separator — the refusal says where the panic came from in its own words, and a corpus
/// author reading an instruction to file a bug against gluon is the outcome this module exists to
/// remove.
fn quoted(payload: &(dyn Any + Send)) -> String {
    let text = payload
        .downcast_ref::<String>()
        .map(String::as_str)
        .or_else(|| payload.downcast_ref::<&'static str>().copied())
        .unwrap_or("a panic carrying no message");
    let first = text.lines().next().unwrap_or(text);
    let shown = first
        .split("Please report an issue")
        .next()
        .unwrap_or(first)
        .trim_end_matches([' ', '.', ':']);
    let mut short: String = shown.chars().take(QUOTED).collect();
    if shown.chars().count() > QUOTED {
        short.push('…');
    }
    short
}

/// Run `f`, and turn a panic out of it into a refusal naming the step.
///
/// `stage` is drawn from the same words [`super::evaluate`]'s other refusals use, so every way one
/// step can fail reads as an answer to the same question. The closure is asserted unwind-safe:
/// gluon's VM and the call budget are dropped without being read again on this path, and the
/// alternative on offer is the abort.
pub fn contain<T>(name: &str, stage: &str, f: impl FnOnce() -> Result<T>) -> Result<T> {
    install_quiet_hook();
    let _quiet = Quiet::hush();
    match panic::catch_unwind(AssertUnwindSafe(f)) {
        Ok(result) => result,
        Err(payload) => Err(anyhow!(
            "{name} panicked gluon's compiler {stage}:\n  {}\n\n  \
             That is an internal error in gluon, not a refusal gluon decided to make, so the \
             line above is about the compiler and not about this calculator. It is caught rather \
             than left to abort the run, because a capability's implementation is corpus data: a \
             shell calculator that is nonsense gets an exit code and a step named, and a typed one \
             gets the same.\n  \
             A `rec let` group whose drawn-in binding is not a record or a variant is the shape \
             known to reach it, and the ordinary way to write one is to leave out the `in` after \
             the group — gluon's layout then draws the following `let` in. If that is this script, \
             the `in` is the fix.",
            quoted(payload.as_ref())
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A panic becomes a refusal naming the step, and the payload's first line survives.
    #[test]
    fn a_panic_is_a_refusal_naming_the_step() {
        let err = contain("tier", "while being loaded", || -> Result<()> {
            panic!("Expected record as last expression of recursive binding `final`: Call(1)\nCall(Ident(…)). Please report an issue at https://github.com/gluon-lang/gluon/issues")
        })
        .expect_err("a panic must not pass for a result");
        let msg = err.to_string();
        assert!(msg.contains("tier"), "{msg}");
        assert!(msg.contains("while being loaded"), "{msg}");
        assert!(msg.contains("recursive binding `final`"), "{msg}");
        assert!(
            !msg.contains("gluon-lang/gluon/issues"),
            "the refusal still asks a corpus author to file a gluon bug: {msg}"
        );
    }

    /// An `Ok` and an ordinary `Err` pass through untouched.
    #[test]
    fn a_result_is_not_rewritten() {
        let out = contain("tier", "while running", || Ok(7)).expect("an ok result passes through");
        assert_eq!(out, 7);
        let err = contain("tier", "while running", || -> Result<()> {
            Err(anyhow!("tier is not a calculator"))
        })
        .expect_err("an error passes through");
        assert_eq!(err.to_string(), "tier is not a calculator");
    }

    /// The invitation is cut off the one-argument `ice!` form too, where it is on the first line.
    #[test]
    fn the_invitation_is_cut_wherever_it_lands() {
        let err = contain("tier", "while being loaded", || -> Result<()> {
            panic!("ICE: Please report an issue at https://github.com/gluon-lang/gluon/issues")
        })
        .expect_err("a panic must not pass for a result");
        let msg = err.to_string();
        assert!(!msg.contains("gluon-lang"), "{msg}");
        assert!(msg.contains("ICE"), "{msg}");
    }

    /// A payload longer than the quota is truncated, and on a character boundary.
    #[test]
    fn an_enormous_payload_is_truncated() {
        let long = format!("binding `n`: {}", "é".repeat(QUOTED * 4));
        let err = contain("tier", "while being loaded", move || -> Result<()> {
            panic!("{long}")
        })
        .expect_err("a panic must not pass for a result");
        let msg = err.to_string();
        assert!(msg.contains('…'), "the payload was not truncated: {msg}");
        assert!(msg.contains("binding `n`"), "{msg}");
    }

    /// The hook stays quiet only on the thread inside `contain`.
    ///
    /// The suppression is what keeps gluon's ICE text off stderr, and it is thread-local so that
    /// installing it once cannot swallow a panic anywhere else in the process.
    #[test]
    fn the_hook_is_quiet_only_on_the_containing_thread() {
        let _ = contain("tier", "while being loaded", || -> Result<()> {
            let elsewhere = std::thread::spawn(|| QUIET.with(Cell::get));
            assert!(
                !elsewhere.join().unwrap_or(true),
                "another thread inherited the quiet flag"
            );
            assert!(QUIET.with(Cell::get), "the containing thread is not quiet");
            Ok(())
        });
        assert!(
            !QUIET.with(Cell::get),
            "the flag outlived the scope that set it"
        );
    }
}
