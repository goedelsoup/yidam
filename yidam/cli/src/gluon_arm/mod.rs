//! The typed calculator arm (RFC-0042) — a calculator whose purity is a closed scope and a typecheck.
//!
//! **Engine only.** Nothing in `run`, `regen` or the manifest reaches this module yet, and that
//! is deliberate: RFC-0042 is `Draft`, and its own decision rule is that the arm fails *"on the
//! first row of the table"* if it cannot be gated out of the default build. This is that row,
//! proved, before a surface depends on it. What the arm needs from a surface — the `run = {
//! gluon = … }` shape, the `unrunnable_because` sentence, the receipt — is not here.
//!
//! # What the arm is for
//!
//! `run` today is an argv, so a calculator is a process, and *pure* is a norm the executor
//! enforces only on the way out: [`crate::cmd::run::exec::check_declared`] refuses a step that
//! wrote outside its `writes`, and nothing says what it read or whether the answer is a function
//! of the corpus at all. A calculator whose entry point is typed `Corpus -> Computed` is one the
//! executor can decline *before* it runs, and for a reason a reader can check.
//!
//! # Two mechanisms, and the second one is not defence in depth
//!
//! RFC-0042 argued the entry point's type was the mechanism. Implementing it found that it is
//! necessary and not sufficient, and the measurement is worth keeping because it is the one
//! place the RFC's central claim is wrong.
//!
//! Every effectful module gluon 0.18.4 registers is typed in `IO`, so importing it buys a script
//! nothing: an `IO ()` does not fit any position in `Corpus -> Computed`, and three separate
//! attempts — using an effect's result in a pure position, returning `IO`, binding inside `IO` —
//! are each refused at typecheck. Except `std.debug`. Its `trace` is declared `a -> ()`, it is
//! registered unconditionally, and it is `println!` to **stdout**. A function whose declared type
//! is exactly `{ nodes : Array String } -> { rows : Array String }` prints from inside it and
//! typechecks. For this CLI that is not a curiosity: stdout is where `--format json` goes, so a
//! declared-pure calculator could corrupt a machine-readable report, which is the failure
//! `regen`'s `checking()` mode exists to prevent one layer up.
//!
//! So the type is one half. The other is [`entry::refuse_macros`] together with [`prelude`]: the
//! only route from a script to a registered native module is the `import!` macro, a calculator
//! script may invoke no macro, and the names it has are the ones the prelude binds. `std.debug`
//! is not among them. That list is closed by default rather than a denylist of what is known to
//! be dangerous — which matters precisely because `std.debug` is the module a denylist reasoned
//! from types would have missed.
//!
//! The registrations are still there. This module does not build a restricted VM: reimplementing
//! gluon's private `build_inner` would be version-coupled in the worst way, silently admitting
//! whatever a future gluon adds. What it does is make them unreachable, which is a property of
//! the script's vocabulary and not of the VM's contents.
//!
//! # Outside the default set, and why RFC-0024's argument does not transfer
//!
//! `calculators-gluon` resolves 71 marginal packages against `regorus`'s 8, for +6.8 MB. RFC-0024
//! ungated `regorus` because *a build that cannot evaluate policy is a build that cannot refuse*.
//! A light build that cannot run a gluon calculator is in no such position: it declines the step
//! by name and commits nothing. `tests/gluon_arm.rs` is where that gating is asserted.

pub mod budget;
pub mod entry;
pub mod marshal;

use anyhow::Result;
use gluon::vm::api::OwnedFunction;
use gluon::ThreadExt;

use marshal::{Computed, Corpus};

/// The `std` modules a calculator script may use.
///
/// Every one is pure-typed, checked: [`crate::gluon_arm`]'s note explains why that is not the
/// test that matters, and `tests/gluon_arm.rs` holds this list to the *excluded* set as well, so
/// a module added here that turns out to reach the outside world fails a test rather than a
/// report.
///
/// Alphabetical, and deliberately not exhaustive. `std.json` and the typeclass modules
/// (`std.functor`, `std.applicative`, `std.monad`, `std.traversable`, `std.semigroup`,
/// `std.monoid`, `std.num`) are pure and absent because nothing has needed them: a data module
/// already exposes its own instances — `array.functor.map` needs no `std.functor` — and a list
/// that grows on demand is one every entry on can be justified by a caller.
pub const PRELUDE_MODULES: &[&str] = &[
    "array", "bool", "byte", "char", "cmp", "float", "foldable", "function", "int", "list", "map",
    "option", "result", "show", "string",
];

/// The modules deliberately left out, and what each one would buy a script.
///
/// Kept as data rather than as prose because a test reads it: the two lists must not intersect,
/// and every module gluon registers unconditionally must appear in one of them. That is the gate
/// that would have caught `std.debug` — it is registered, it is pure-typed, and no reasoning
/// from types puts it here.
pub const EXCLUDED: &[(&str, &str)] = &[
    ("debug", "`trace` is declared `a -> ()` and prints to stdout — the one effect the entry point's type does not refuse"),
    ("io", "reading and writing files and the terminal"),
    ("fs", "the filesystem"),
    ("process", "spawning a process"),
    ("env", "the environment the run happens in"),
    ("thread", "concurrency, and a second VM"),
    ("channel", "communication between threads"),
    ("reference", "mutable state across calls, which makes the answer depend on call order"),
    ("http", "the network"),
    ("regex", "nothing — it needs a gluon feature this build does not enable — and naming it here is how a script gets told that rather than *module not found*"),
];

/// The type definitions a script needs in scope, mirroring the Rust sums in [`marshal`].
///
/// Gluon's variant types are structural, so a textual definition unifies with the one
/// `#[derive(VmType)]` produces as long as the constructors and their argument types match
/// exactly. They must therefore be kept in step by hand, which is what
/// `the_injected_types_match_the_derived_ones` in `tests/gluon_arm.rs` is for: it renders the
/// derived type and compares.
pub const PRELUDE_TYPES: &[&str] =
    &["type Value = | Text String | Int Int | Number Float | Flag Bool | Empty | Unrepresentable"];

/// Everything a calculator script has in scope, as one physical line.
///
/// **One line, and that is the whole reason it is unreadable.** Gluon reports a type error at a
/// line and column of the source it was given, so a prelude occupying its own lines would shift
/// every number a calculator author reads by exactly that many — quietly, and in the direction
/// that sends them to the wrong line. Concatenated onto the front of line 1, every line after
/// the first is the script's own line, and only a line-1 error carries a column offset.
pub fn prelude() -> String {
    let mut s = String::new();
    for m in PRELUDE_MODULES {
        s.push_str("let ");
        s.push_str(m);
        s.push_str(" = import! std.");
        s.push_str(m);
        s.push_str(" in ");
    }
    for t in PRELUDE_TYPES {
        s.push_str(t);
        s.push_str(" in ");
    }
    s
}

/// A script checked and made ready to typecheck: the refusal first, then the prelude.
///
/// The macro refusal runs against the script alone, because the prelude's own `import!`s are the
/// thing the refusal exists to be the only instance of.
pub fn prepare(script: &str) -> Result<String> {
    entry::refuse_macros(script)?;
    Ok(format!("{}{}", prelude(), script))
}

/// What a calculator run produced, and what it cost.
#[derive(Debug)]
pub struct Outcome {
    pub computed: Computed,
    /// Calls charged. Reported whether the run finished or was refused at the budget.
    pub calls: usize,
}

/// Apply a calculator script to a corpus.
///
/// The order is the design: refuse a macro, typecheck against `Corpus -> Computed`, install the
/// budget, and only then run. Nothing about the script's *contents* is inspected beyond the
/// macro rule — everything else it is not allowed to do, it is not allowed to do because the
/// type refuses it or because the name is not bound.
/// `corpus` is the projected value and not the reader, so that this function's signature is a
/// public one — and so that a test can hand over a corpus it constructed rather than one it had
/// to write to disk. [`marshal::project`] is the bridge from the reader the CLI holds.
pub fn evaluate(name: &str, script: &str, corpus: Corpus, calls: usize) -> Result<Outcome> {
    let source = prepare(script)?;
    let vm = gluon::new_vm();
    entry::typecheck(&vm, name, &source)?;

    let budget = budget::Budget::install(&vm, calls);

    // Both paths ask the budget, because both can exhaust it. `run_expr` does not only compile:
    // it evaluates the module's top level, so `rec let go n = go (n + 1) in let x = go 0 in …`
    // never reaches the call. Reporting that as `compiling {name}` would tell a calculator
    // author the wrong thing about their own script, and would hide the only refusal in this
    // arm that a corpus can do something about.
    let refuse = |stage: &str, e: &dyn std::fmt::Display| {
        if budget.exhausted() {
            anyhow::anyhow!(
                "{name} was stopped after {} calls, which is its budget.\n  \
                 A calculator that does not finish is a run that hangs rather than one that \
                 fails, so the budget is a refusal and not a warning.",
                budget.limit()
            )
        } else {
            anyhow::anyhow!("{name} failed {stage} after {} calls: {e}", budget.spent())
        }
    };

    let (mut f, _) = vm
        .run_expr::<OwnedFunction<fn(Corpus) -> Computed>>(name, &source)
        .map_err(|e| refuse("while being loaded", &e))?;
    let computed = f.call(corpus).map_err(|e| refuse("while running", &e))?;
    Ok(Outcome {
        computed,
        calls: budget.spent(),
    })
}
