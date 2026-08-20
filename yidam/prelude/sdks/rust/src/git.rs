#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum CommitKind {
    Epistemic,
    Operational,
}

impl CommitKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            CommitKind::Epistemic => "Epistemic",
            CommitKind::Operational => "Operational",
        }
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CommitEvent {
    pub hash: String,
    pub kind: CommitKind,
    pub verb: String,
    pub subject: String,
    pub context: Option<String>,
}

/// Verbs marking a commit as pipeline work rather than a change in understanding.
///
/// Kept in sync with the commit vocabulary in `prelude/GRAPH.md` and with the
/// `OperationalVerbs` set in `prelude/sdks/spec/graph.dfy`.
pub const OPERATIONAL_VERBS: &[&str] = &[
    "extract",
    "refresh",
    "compute",
    "index",
    "bundle",
    "reconcile",
    "regen",
    "build",
    "implement",
    "scaffold",
    "catalog",
    "migrate",
    "fix",
    "vendor",
    "consume",
];

/// Verbs marking a commit as a change in understanding.
///
/// `classify_commit` does not consult this list — Operational is the explicitly marked
/// case and everything else is Epistemic, a totality the Dafny spec proves. The list
/// exists so that a verb belonging to *neither* set can be recognized as outside the
/// vocabulary entirely; see [`is_recognized_verb`].
pub const EPISTEMIC_VERBS: &[&str] = &[
    "establish",
    "revise",
    "assess",
    "scope",
    "synthesize",
    "withdraw",
    "open",
    "close",
    "transport",
    "resolve",
    "adopt",
    "decide",
    "phase",
    "genesis",
    "overlay",
];

/// Whether `verb` is in the closed vocabulary defined by `prelude/GRAPH.md`.
///
/// This is the check `yidam lint --commits` runs. It is deliberately separate from
/// [`classify_commit`], which must remain total: every commit gets a kind, including the
/// ones written before a repository adopted the vocabulary. Recognition is the question
/// of whether the log is *legible*; classification is the question of what a commit did.
pub fn is_recognized_verb(verb: &str) -> bool {
    OPERATIONAL_VERBS.contains(&verb) || EPISTEMIC_VERBS.contains(&verb)
}

pub fn classify_commit(hash: &str, message: &str) -> CommitEvent {
    let first_line = message.lines().next().unwrap_or("").trim();

    let (verb, subject) = match first_line.find(": ") {
        Some(pos) => (
            first_line[..pos].trim().to_string(),
            first_line[pos + 2..].trim().to_string(),
        ),
        None => (String::new(), first_line.to_string()),
    };

    let kind = if OPERATIONAL_VERBS.contains(&verb.as_str()) {
        CommitKind::Operational
    } else {
        CommitKind::Epistemic
    };

    CommitEvent {
        hash: hash.to_string(),
        kind,
        verb,
        subject,
        context: None,
    }
}
