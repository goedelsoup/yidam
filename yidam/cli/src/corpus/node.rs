//! A corpus instance: one `.yml` file under a class directory, parsed once.

use std::path::PathBuf;

use crate::parse::CorpusInstance;

/// A corpus instance parsed once, with the paths needed to talk about it.
pub struct Node {
    pub path: PathBuf,
    pub rel: String,
    pub inst: CorpusInstance,
    /// The file's bytes, as they were read.
    ///
    /// Kept rather than dropped after parsing, because three callers were reading the same
    /// file again from disk to get it back: `query`'s `--select body`, the keyword arm of a
    /// similarity anchor, and — the one that makes this a correctness fix rather than a
    /// tidy-up — a query at a past commit, where `path` names a file whose *current*
    /// contents are the wrong answer and which may not exist at all. `load_nodes` already
    /// had this string in hand and threw it away.
    pub text: String,
    /// Why the bytes did not parse, when they did not. See [`super::parse_or_default`].
    pub malformed: Option<String>,
}

impl Node {
    /// Build one from an instance file's bytes and where it came from.
    ///
    /// The only constructor, for the reason [`crate::corpus::Class::parse`] is the only one for a class:
    /// [`Self::text`] and [`Self::inst`] must come from the same string, and a caller holding
    /// both could hand over a mismatched pair with nothing to say so. It is also what makes
    /// [`Self::malformed`] unforgeable — the parse outcome is recorded where the parse
    /// happens, and there is no other way to build one.
    pub(crate) fn parse(path: PathBuf, rel: impl Into<String>, text: impl Into<String>) -> Self {
        let text = text.into();
        let (inst, malformed) = super::parse_or_default(&text);
        Self {
            path,
            rel: rel.into(),
            inst,
            text,
            malformed,
        }
    }
}
