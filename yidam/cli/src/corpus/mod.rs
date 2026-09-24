//! What a corpus is made of, independent of what any command does with it.
//!
//! [`crate::corpus::Node`], [`crate::corpus::Class`] and [`crate::corpus::Source`] are the three records a yidam
//! repository is written in, and the loaders here are the one way bytes on disk become
//! them. They lived inside `cmd/lint/checks.rs` until #924, which is where they were first
//! needed — and by then twenty-two files outside `lint` were importing them, including
//! `claims`, `universal`, `retrievable` and nine other commands. The library depended on
//! one subcommand's internals, `checks.rs` could not be split because of it, and `pack`
//! had to spell a corpus type as `cmd::lint::checks::Node`.
//!
//! `lint` is now a consumer like the others. Nothing here knows a check exists.

pub(crate) mod class;
pub(crate) mod load;
pub(crate) mod node;
pub(crate) mod overlay;
pub(crate) mod source;

// Glob rather than a name list, and for once that is the explicit form. Each file below
// holds one part of the model and nothing else, so its surface *is* what belongs at
// `crate::corpus::` — where a curated list would go stale the moment a field type is added,
// and would emit an unused-import warning (a denied lint here) for every name the non-test
// build happens not to mention.
pub(crate) use class::*;
pub(crate) use load::*;
pub(crate) use node::*;
pub(crate) use overlay::*;
pub(crate) use source::*;

/// A YAML document read into `T`, and the reason if it could not be.
///
/// **The one place the corpus model turns bytes into a record**, and it is one place because it
/// was five — each of them `serde_yaml::from_str(&text).unwrap_or_default()`, each of them
/// turning a file nobody could read into an *empty* record that the checks then read as fact.
/// One unclosed quote in an instance produced eight findings across six checks, the first of
/// which said the file declared no `class:` about a file whose first line is a `class:` field.
/// The same typo in a `<class>.ont.yml` dropped every declared property, which switched off
/// `missing-property`, `undeclared-property`, `property-type`, `edge-target-class` and
/// `unlicensed-edge` at once and left the gate reporting a clean corpus (#676).
///
/// The default is still returned, because the checks downstream are written against a record
/// and a half-read file is better described as empty than guessed at. What changes is that
/// the failure comes back with it, so [`crate::cmd::lint::checks::malformed_yaml`] can report the file and the rest of
/// the report can stay quiet about it.
///
/// **An empty document is not a failure.** `serde_yaml` reads no bytes at all as the absent
/// value, and a file with nothing in it has not contradicted anything — `missing-class` and
/// its siblings already describe that file correctly. Keeping it out of scope is also what
/// keeps `Overlay::read`'s unreadable-file-as-`""` out of scope, which is a different
/// question with a different blast radius.
pub(crate) fn parse_or_default<T: Default + serde::de::DeserializeOwned>(
    text: &str,
) -> (T, Option<String>) {
    match serde_yaml::from_str(text) {
        Ok(parsed) => (parsed, None),
        Err(e) => (T::default(), Some(e.to_string())),
    }
}
