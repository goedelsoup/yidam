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

use anyhow::{bail, Result};
use gluon::vm::api::VmType;
use gluon::ThreadExt;

use super::marshal::{Computed, Corpus};

/// Refuse a script that invokes a macro.
///
/// An identifier immediately followed by `!`. Gluon has no `!` operator — nothing in its `std`
/// defines one — so there is no legitimate use of that shape in an expression, and the rule
/// costs a calculator author nothing they would otherwise have written.
pub fn refuse_macros(src: &str) -> Result<()> {
    let bytes = src.as_bytes();
    for (i, c) in src.char_indices() {
        if c != '!' || i == 0 {
            continue;
        }
        let prev = bytes[i - 1];
        if !(prev.is_ascii_alphanumeric() || prev == b'_') {
            continue;
        }
        let start = src[..i]
            .rfind(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
            .map_or(0, |b| b + 1);
        let name = &src[start..i];
        let line = src[..i].matches('\n').count() + 1;
        bail!(
            "line {line}: `{name}!` is a macro invocation, and a calculator script may not \
             invoke a macro.\n  \
             Gluon reaches a native module only through `import!`, so a script that imports \
             nothing can only use the names the calculator prelude binds — which is what makes \
             this arm's purity a property of the build rather than a norm. The prelude binds: \
             {}.\n  \
             A step that needs anything else declares the shell arm.",
            super::PRELUDE_MODULES.join(", ")
        );
    }
    Ok(())
}

/// The type every calculator script must have.
///
/// Built from the Rust types rather than written as a string, so a field added to
/// [`Corpus`] or [`Computed`] changes what scripts are accepted without anyone remembering to
/// restate it here.
pub fn expected_type(vm: &gluon::Thread) -> gluon::base::types::ArcType {
    <fn(Corpus) -> Computed as VmType>::make_type(vm)
}

/// Typecheck `source` as `Corpus -> Computed`.
///
/// `source` is the prelude and the script concatenated — see [`super::prepare`], which keeps
/// them on one physical line so that a line number in the error below is the script's own.
pub fn typecheck(vm: &gluon::Thread, name: &str, source: &str) -> Result<()> {
    let expected = expected_type(vm);
    match vm.typecheck_str(name, source, Some(&expected)) {
        Ok(_) => Ok(()),
        Err(e) => bail!(
            "{name} is not a calculator.\n  \
             A calculator script is a function from the corpus to a signal table, and this one \
             does not have that type. An effect is the common cause: `io.println x` has type \
             `IO ()`, and there is no position in `Corpus -> Computed` that a value of that \
             type fits.\n\n{e}"
        ),
    }
}
