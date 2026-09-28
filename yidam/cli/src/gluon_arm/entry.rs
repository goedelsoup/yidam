//! What a calculator script is allowed to be, decided before it runs.
//!
//! Two refusals, and the second one is the load-bearing one. RFC-0042 argued that *"the entry
//! point's type is the mechanism"* — a script declared `Corpus -> Computed` cannot perform an
//! effect, because performing one in a pure position does not typecheck. That is true of every
//! effectful module gluon registers **except one**, and the exception is measured in
//! [`super`]'s note. So the type is necessary and it is not sufficient, and this module holds
//! both halves.
//!
//! # A calculator script invokes no macro
//!
//! Gluon reaches a registered native module through the `import!` macro and through nothing
//! else. Measured on 0.18.4: a global registered as `std.debug.prim` is not a variable, so
//! `std.debug.prim.trace "x"` is *Undefined variable `std`*, and the macro cannot be spelled
//! with anything between the name and the bang — `import ! std.debug` and `import/*x*/!
//! std.debug` are both *Undefined variable `!`*. Every spelling that actually imports contains
//! the literal token `import!`.
//!
//! That is what makes a scan sound rather than approximate. The names a script may use are the
//! ones [`super::prelude`] binds, and a script that may invoke no macro cannot add to them. The
//! list is closed by default: a future gluon that registers a new effectful module changes
//! nothing here, which is the property a denylist of module names would not have had.
//!
//! The scan is over macro invocations generally and not over `import!` alone. `lift_io!` is the
//! other macro the default VM registers, and a rule naming today's two would be the denylist
//! again one level down.
//!
//! It is a *lexical* rule, so it over-matches: a script with `import!` inside a string literal
//! or a comment is refused although it imports nothing. That direction is safe and it is the
//! only direction available — refusing a script that would have run is recoverable by editing
//! it, and admitting one that reaches stdout is not.
//!
//! # One admission, two callers
//!
//! [`admit`] is the whole of what a calculator script has to be, and it has two callers since
//! #1099: [`super::evaluate`], which runs the script it admits, and `lint`'s calculator checks,
//! which admit it and stop. RFC-0042 states why that has to be one function rather than two
//! agreeing implementations — *"a second opinion about a script's vocabulary is worse than no
//! gate: it either passes something `run` refuses, or refuses something `run` would have
//! accepted"* — and `#1087` sharpened it, because the arm's purity is a **closed prelude** plus
//! a typecheck, so the commonest refusal is now a name [`super::PRELUDE_MODULES`] does not bind.
//! A lint that built its own scope would differ from `run` in exactly the dimension the
//! hermeticity argument rests on. `tests/gluon_arm.rs` holds the two callers to the one
//! function.
//!
//! # The three refusals are told apart, because two of them read alike
//!
//! RFC-0042 open question 5 counts three ways a script is not a calculator, and [`Refused`] is
//! that count made into a value. The middle one is the reason this exists: an **unbound name**
//! is refused by the typechecker, so before #1099 it arrived under a sentence opening *"this one
//! does not have that type"* and offering an effect in a pure position as the likely cause —
//! which is the wrong thing to tell the author of a script whose only mistake is naming
//! `std.json`. It is a scope failure, it is now named as one, and the refusal lists what the
//! prelude binds instead of speculating about effects.

use std::fmt;

#[cfg(feature = "calculators-gluon")]
use gluon::vm::api::VmType;
#[cfg(feature = "calculators-gluon")]
use gluon::ThreadExt;

#[cfg(feature = "calculators-gluon")]
use super::marshal::{Computed, Corpus};

/// Which of the three refusals RFC-0042 open question 5 counts.
///
/// A value rather than three prose messages, because two callers need to *act* on the
/// distinction and not merely print it: `lint` reports each one under its own check id, so a
/// baseline can carry a scope failure without also carrying every type failure, and a report can
/// say which of the three a corpus has. The RFC's own account is that the middle one "reads as a
/// type error in the output today", and an enum is how that stops being true in more than one
/// place at once.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refused {
    /// A macro invocation — refused before any gluon runs, by [`refuse_macros`].
    Macro,
    /// A name the calculator prelude does not bind.
    ///
    /// Reported by the typechecker, and a **scope** failure rather than a type failure: the
    /// script did not get the type wrong, it named something that is not there. #1087 makes this
    /// the commonest refusal the arm has, because the purity argument is a closed prelude and not
    /// the entry point's type alone — so a script reaching for `std.json` fails here and not on
    /// its shape.
    Scope,
    /// Not a function from the corpus to a signal table. A type failure.
    Shape,
}

impl Refused {
    /// Every refusal, so a test covering them is held to the enum rather than to a list.
    ///
    /// Public and unconditional, unlike [`crate::cmd::run::manifest::Kind::ALL`]'s `#[cfg(test)]`:
    /// this arm's tests are `tests/gluon_arm.rs`, an integration target that compiles against the
    /// library without `cfg(test)`, so a gated constant would be invisible to the only thing that
    /// wants it.
    pub const ALL: [Self; 3] = [Self::Macro, Self::Scope, Self::Shape];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Macro => "macro",
            Self::Scope => "scope",
            Self::Shape => "shape",
        }
    }
}

/// A script that is not a calculator: which way, and the sentence that says so.
///
/// The sentence is built where the refusal is decided and carried rather than re-derived,
/// because `run` prints it and `lint` files it as a finding's detail — and the two must be the
/// same words. A caller that rendered its own from [`Refused`] would be the second opinion this
/// module exists to not have.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refusal {
    pub refused: Refused,
    pub detail: String,
}

impl Refusal {
    /// Public so a caller's *tests* can name a refusal without reaching a typechecker —
    /// `cmd::lint::calculators` reports the three kinds through three checks, and the reading
    /// that sorts them is worth a test in a build that has no VM to produce one with.
    pub fn new(refused: Refused, detail: impl Into<String>) -> Self {
        Self {
            refused,
            detail: detail.into(),
        }
    }
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.detail)
    }
}

impl std::error::Error for Refusal {}

/// Why [`admit`] said no: the script's refusal, or gluon's own failure to decide.
///
/// Two shapes and not one, because only the first is a statement about the corpus.
/// [`super::ice`] argues the case at length: gluon's `ice!` is `panic!`, a `.glu` can reach it,
/// and a panic is "neither" of the arm's refusals. `lint` needs that told apart — a corpus whose
/// calculator crashed gluon's compiler has not been shown to have a broken calculator, and a
/// gating finding would be this binary's failure charged to the repository.
#[derive(Debug)]
pub enum Rejected {
    /// The script is refused. [`Refusal::refused`] says which of the three ways.
    Refused(Refusal),
    /// Gluon's compiler panicked while deciding. Contained rather than fatal; see
    /// [`super::ice::contain`], whose message this carries.
    Ice(anyhow::Error),
}

impl fmt::Display for Rejected {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Refused(r) => r.fmt(f),
            Self::Ice(e) => write!(f, "{e}"),
        }
    }
}

impl From<Rejected> for anyhow::Error {
    fn from(r: Rejected) -> Self {
        match r {
            // The contained panic's message is already `ice::contain`'s, naming the step and the
            // stage. Wrapping it again would put a refusal's framing around something that is
            // explicitly not a refusal.
            Rejected::Ice(e) => e,
            Rejected::Refused(r) => Self::new(r),
        }
    }
}

/// Refuse a script that invokes a macro.
///
/// An identifier immediately followed by `!`. Gluon has no `!` operator — nothing in its `std`
/// defines one — so there is no legitimate use of that shape in an expression, and the rule
/// costs a calculator author nothing they would otherwise have written.
///
/// **Compiled in every build**, unlike everything below it. The scan is text and a list of
/// module names; it needs no VM, no typechecker and no gluon at all, so the light binary — the
/// one `install.sh` downloads — can decide this refusal for itself. That is what lets `lint`
/// gate refusal 1 identically in a build that cannot run a calculator and one that can, which is
/// strictly better than the alternative on offer: the same gate answering differently depending
/// on which binary the reader is holding.
pub fn refuse_macros(src: &str) -> std::result::Result<(), Refusal> {
    for (i, c) in src.char_indices() {
        if c != '!' || i == 0 {
            continue;
        }
        // Walked as chars rather than bytes throughout. A calculator may hold any text in a
        // string literal, and the scan runs before anything else in `prepare` — so a byte index
        // landing inside a multi-byte character would take the process down on a script whose
        // only crime is a `¡` before an exclamation mark, which is the one outcome a refusal
        // scan must not have.
        let mut back = src[..i].char_indices().rev();
        let Some((_, prev)) = back.next() else {
            continue;
        };
        if !(prev.is_ascii_alphanumeric() || prev == '_') {
            continue;
        }
        let start = back
            .find(|(_, c)| !(c.is_ascii_alphanumeric() || *c == '_'))
            .map_or(0, |(b, c)| b + c.len_utf8());
        let name = &src[start..i];
        let line = src[..i].matches('\n').count() + 1;
        return Err(Refusal::new(
            Refused::Macro,
            format!(
                "line {line}: `{name}!` is a macro invocation, and a calculator script may not \
                 invoke a macro.\n  \
                 Gluon reaches a native module only through `import!`, so a script that imports \
                 nothing can only use the names the calculator prelude binds — which is what \
                 makes this arm's purity a property of the build rather than a norm. The prelude \
                 binds: {}.\n  \
                 A step that needs anything else declares the shell arm.",
                super::PRELUDE_MODULES.join(", ")
            ),
        ));
    }
    Ok(())
}

/// The type every calculator script must have.
///
/// Built from the Rust types rather than written as a string, so a field added to
/// [`Corpus`] or [`Computed`] changes what scripts are accepted without anyone remembering to
/// restate it here.
#[cfg(feature = "calculators-gluon")]
pub fn expected_type(vm: &gluon::Thread) -> gluon::base::types::ArcType {
    <fn(Corpus) -> Computed as VmType>::make_type(vm)
}

/// Every name the typechecker found nothing bound to, in the order it reported them.
///
/// Structural rather than a scan of the rendered message. Gluon's `Error` is an enum and
/// `TypeError::UndefinedVariable` is a variant of it, so the classification is the compiler's own
/// answer; matching on *"Undefined variable"* in formatted output would be this crate's guess at
/// gluon's prose, which changes between releases with nothing going red.
///
/// `UndefinedType` is here beside `UndefinedVariable` because [`super::PRELUDE_TYPES`] binds types
/// as well as values: a script naming `Ordering` — which the prelude does **not** inject, however
/// much it reads like it should — is refused for the same reason and wants the same sentence.
///
/// `Error::Multiple` is walked recursively. A script with two unbound names arrives as one
/// `Multiple` holding two, and a report naming only the first would send its author round the
/// loop once per typo.
#[cfg(feature = "calculators-gluon")]
fn unbound(e: &gluon::Error) -> Vec<String> {
    use gluon::check::typecheck::TypeError;

    let mut out: Vec<String> = Vec::new();
    match e {
        gluon::Error::Multiple(errors) => {
            for inner in errors {
                out.extend(unbound(inner));
            }
        }
        gluon::Error::Typecheck(in_file) => {
            for spanned in in_file.errors() {
                match &spanned.value.error {
                    TypeError::UndefinedVariable(id) | TypeError::UndefinedType(id) => {
                        out.push(id.declared_name().to_string())
                    }
                    _ => {}
                }
            }
        }
        _ => {}
    }
    out
}

/// Typecheck `source` as `Corpus -> Computed`, and say which refusal a failure is.
///
/// `source` is the prelude and the script concatenated — see [`super::prepare`], which keeps
/// them on one physical line so that a line number in the error below is the script's own.
///
/// The two refusals this can produce are told apart by [`unbound`], and the sentences differ in
/// what they offer as the likely cause, which is the whole point. A [`Refused::Shape`] failure is
/// usually an effect in a pure position; a [`Refused::Scope`] failure is usually a module the
/// prelude does not bind, and telling its author about `IO ()` sends them to look at the one part
/// of their script that is fine.
#[cfg(feature = "calculators-gluon")]
pub fn typecheck(vm: &gluon::Thread, name: &str, source: &str) -> std::result::Result<(), Refusal> {
    let expected = expected_type(vm);
    let e = match vm.typecheck_str(name, source, Some(&expected)) {
        Ok(_) => return Ok(()),
        Err(e) => e,
    };

    let unbound = unbound(&e);
    if unbound.is_empty() {
        return Err(Refusal::new(
            Refused::Shape,
            format!(
                "{name} is not a calculator.\n  \
                 A calculator script is a function from the corpus to a signal table, and this \
                 one does not have that type. An effect is the common cause: `io.println x` has \
                 type `IO ()`, and there is no position in `Corpus -> Computed` that a value of \
                 that type fits.\n\n{e}"
            ),
        ));
    }

    let named = unbound.join(", ");
    Err(Refusal::new(
        Refused::Scope,
        format!(
            "{name} names {named}, which the calculator prelude does not bind.\n  \
             This is a scope failure and not a type failure — the script did not get its shape \
             wrong, it reached for something that is not in scope. A calculator script may invoke \
             no macro, so `import!` is not available to it and the names it has are exactly the \
             ones the prelude injects: {}.\n  \
             A step that needs anything else declares the shell arm.\n\n{e}",
            super::PRELUDE_MODULES.join(", ")
        ),
    ))
}

/// A script admitted: the VM it was typechecked against, and the source that was.
///
/// The VM travels with the source because [`super::evaluate`] goes on to install a call budget on
/// it and run the expression, and a second VM would typecheck against one set of registrations
/// and run against another.
#[cfg(feature = "calculators-gluon")]
pub struct Admitted {
    pub vm: gluon::RootedThread,
    /// The prelude and the script, concatenated as [`super::prepare`] spells it.
    pub source: String,
}

/// Everything a calculator script has to be, decided without running it.
///
/// **The one function, called from both sides.** [`super::evaluate`] admits a script and then runs
/// it; `lint`'s calculator checks admit it and stop. RFC-0042 requires exactly this — a lint that
/// built its own prelude would be "a second opinion about a script's vocabulary", passing what
/// `run` refuses or refusing what `run` would have taken — and #1080 reached the same answer for
/// the resolved corpus one artifact over: one builder, one answer, and a test that says so.
///
/// The order is the design: refuse a macro, then typecheck. Nothing about the script's *contents*
/// is inspected beyond the macro rule — everything else it may not do, it may not do because the
/// type refuses it or because the name is not bound.
///
/// Contained, because gluon's compiler can panic rather than answer. The boundary is here and not
/// only around the evaluation for the reason [`super::ice`] gives, and it is what makes this
/// callable from `lint`: a `yidam lint` that aborted on a corpus holding a script gluon chokes on
/// would be a gate that a calculator can switch off.
#[cfg(feature = "calculators-gluon")]
pub fn admit(name: &str, script: &str) -> std::result::Result<Admitted, Rejected> {
    refuse_macros(script).map_err(Rejected::Refused)?;
    let source = format!("{}{}", super::prelude(), script);
    let vm = gluon::new_vm();
    // Nested deliberately: the outer `Result` is gluon's failure to decide and the inner one is
    // the script's verdict. `ice::contain` speaks anyhow, so the verdict rides through it as a
    // value rather than being flattened into the same channel as the panic.
    match super::ice::contain(name, "while being typechecked", || {
        Ok(typecheck(&vm, name, &source))
    }) {
        Err(e) => Err(Rejected::Ice(e)),
        Ok(Err(r)) => Err(Rejected::Refused(r)),
        Ok(Ok(())) => Ok(Admitted { vm, source }),
    }
}
