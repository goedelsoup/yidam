//! A decision record: one `.yidam/decisions/*.yml` file, parsed once.

use std::path::{Path, PathBuf};

/// A decision record, and whether its bytes were the document they were read as.
///
/// The fourth record a yidam repository is written in, and the last one whose parse failure was
/// invisible. `read_decision` was `serde_yaml::from_str(&text).unwrap_or_default()`, so one
/// unclosed quote turned a record that governed a phase into a `Decision` with no `id:` and no
/// `summary:` — which [`crate::cmd::decisions::decision_id`] then names by its file stem and
/// the log renders with an em dash. Nothing in the repository could say the difference between
/// that and a record nobody had filled in, and a corpus lost a phase's decision to it (#1056).
///
/// So the parse outcome is recorded where the parse happens, as it is for a [`super::Node`] and
/// a [`super::Class`], and [`crate::cmd::lint::checks::malformed_yaml`] reports it.
pub struct DecisionRecord {
    pub path: PathBuf,
    pub rel: String,
    pub decision: crate::parse::Decision,
    /// Why the bytes did not parse, when they did not. See [`super::parse_or_default`].
    pub malformed: Option<String>,
}

impl DecisionRecord {
    /// The only constructor, for the reason [`super::Node::parse`] is: [`Self::malformed`] is
    /// unforgeable only while the parse and the record it produced cannot be separated.
    pub(crate) fn parse(path: PathBuf, rel: impl Into<String>, text: &str) -> Self {
        let (decision, malformed) = super::parse_or_default(text);
        Self {
            path,
            rel: rel.into(),
            decision,
            malformed,
        }
    }

    /// How this record is identified: its `id:`, or its file stem when it declares none.
    ///
    /// Both forms are accepted because both are what a reader would write. The fallback is the
    /// log's own, and lives here rather than beside the log — a second rule for naming one
    /// record is a second answer to *which record is this*, and `find_decision` resolves a
    /// pointer by the same name the log prints.
    pub(crate) fn id(&self) -> String {
        self.decision.id.clone().unwrap_or_else(|| {
            self.path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string()
        })
    }

    /// The filename, which is what a link in the log's own directory points at.
    pub(crate) fn filename(&self) -> String {
        self.path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string()
    }
}

/// Every decision record under `dir`, in the walk's order.
///
/// Takes the directory rather than a root, because `due` and the log resolve one from a path
/// they already hold, and because a decisions directory is the one part of a repository that is
/// read without a [`super::Corpus`] in hand.
pub(crate) fn load_decisions(root: &Path, dir: &Path) -> Vec<DecisionRecord> {
    crate::walk::walk_decision_files(dir)
        .into_iter()
        .map(|p| {
            let text = std::fs::read_to_string(&p).unwrap_or_default();
            let rel = p
                .strip_prefix(root)
                .unwrap_or(&p)
                .to_string_lossy()
                .to_string();
            DecisionRecord::parse(p, rel, &text)
        })
        .collect()
}
