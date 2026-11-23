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
    /// [`Self::text`] with its inline REGEN tags removed, when it has any (RFC-0058 §2.5).
    ///
    /// [`Self::inst`] is parsed from this, so a reader of a field sees `0.75 s` where the file
    /// holds a `signal` block. `text` stays the bytes as read, because callers hold byte spans
    /// into it and some write it back.
    pub(crate) stripped: Option<String>,
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
        let stripped = match crate::regen::strip_markers(&text) {
            std::borrow::Cow::Owned(stripped) => Some(stripped),
            std::borrow::Cow::Borrowed(_) => None,
        };
        // Malformed is the raw file's verdict, which `malformed-yaml` reports. Stripping only
        // removes tags inside a scalar, so a file that parsed still parses.
        let inst = match (&stripped, &malformed) {
            (Some(prose), None) => super::parse_or_default(prose).0,
            _ => inst,
        };
        Self {
            path,
            rel: rel.into(),
            inst,
            text,
            malformed,
            stripped,
        }
    }

    /// The node's prose as a reader sees it: [`Self::text`] without inline REGEN tags.
    pub fn prose(&self) -> &str {
        self.stripped.as_deref().unwrap_or(&self.text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A field reads the value a block holds and not the tags around it; the bytes are kept.
    #[test]
    fn a_signal_block_in_a_node_is_read_as_its_value() {
        let text = "label: Gage\ndescription: |\n  Up to <!-- REGEN: yidam signal max_s -->0.75<!-- /REGEN --> s.\n";
        let n = Node::parse(PathBuf::from("c/x.yml"), "c/x.yml", text);
        assert!(n.malformed.is_none(), "{:?}", n.malformed);
        assert_eq!(n.inst.description.as_deref(), Some("Up to 0.75 s.\n"));
        assert_eq!(n.prose(), "label: Gage\ndescription: |\n  Up to 0.75 s.\n");
        assert_eq!(n.text, text);
    }

    #[test]
    fn a_node_with_no_block_borrows_its_text() {
        let n = Node::parse(PathBuf::from("c/x.yml"), "c/x.yml", "label: Gage\n");
        assert!(n.stripped.is_none());
        assert_eq!(n.prose(), "label: Gage\n");
    }
}
