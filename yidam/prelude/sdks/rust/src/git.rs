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

const OPERATIONAL_VERBS: &[&str] = &[
    "extract", "refresh", "compute", "index", "bundle", "reconcile", "build", "fix", "regen",
];

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

    CommitEvent { hash: hash.to_string(), kind, verb, subject, context: None }
}
