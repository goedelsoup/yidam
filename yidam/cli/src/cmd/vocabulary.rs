//! `yidam vocabulary` — the closed commit vocabulary, and one subject checked against it.
//!
//! Two audiences, one report. A person asks *which verb do I use*; a client asks *is this
//! subject legal* while the message is still in the box, before there is a commit for
//! `lint --commits` to read.
//!
//! # Where membership comes from, and where prose comes from
//!
//! Membership is [`is_recognized_verb`] — the parity-certified predicate that
//! `lint --commits` gates on, proven total against the Dafny spec. GRAPH.md supplies the
//! **When** column and nothing else.
//!
//! That split matters. A consumer that decided membership from GRAPH.md would be a second
//! implementation of a rule three SDKs already certify, and it could disagree with the gate
//! — an editor that green-lights a verb CI then reports is worse than one that says nothing.
//!
//! # And so the "kept in sync" comment becomes checkable
//!
//! `git.rs`, `git.py` and `git.ts` each say the constants are *"kept in sync with the commit
//! vocabulary in `prelude/GRAPH.md`"*. Three comments, in three languages, and nothing
//! verified it — the exact shape of drift this repository keeps finding by reading. Parsing
//! the tables to fetch their prose means the two can be compared for free, so `drift` is a
//! field rather than a hope.

use anyhow::Result;
use std::path::{Path, PathBuf};

use yidam_core::git::{
    classify_commit, is_recognized_verb, CommitKind, EPISTEMIC_VERBS, OPERATIONAL_VERBS,
};

use crate::paths::repo_root;

#[derive(Debug, serde::Serialize)]
pub struct Verb {
    pub verb: String,
    /// `epistemic` or `operational`, from the certified lists.
    pub kind: &'static str,
    /// The **When** cell from GRAPH.md, or empty when the document is absent.
    pub when: String,
}

/// One rule a subject line broke.
#[derive(Debug, serde::Serialize)]
pub struct SubjectViolation {
    /// Stable id: `scope-suffix`, `unrecognized-verb`, `no-verb`.
    pub rule: String,
    /// Read from the `unrecognized-verb` check rather than restated.
    ///
    /// All three rules are sub-cases of that one check — it reports a scoped verb and a
    /// missing verb through the same finding — so they share its severity. A surface that
    /// squiggled harder than the gate would be asserting a verdict nobody agreed to.
    pub severity: &'static str,
    pub message: String,
}

fn severity() -> &'static str {
    super::lint::commit_verb_severity().as_str()
}

#[derive(Debug, serde::Serialize)]
pub struct SubjectCheck {
    pub text: String,
    /// Everything before the first `: `, exactly as `classify_commit` reads it.
    pub verb: String,
    /// What the commit would be filed as. Total: every subject gets a kind.
    pub kind: &'static str,
    pub recognized: bool,
    pub violations: Vec<SubjectViolation>,
}

#[derive(Debug, serde::Serialize)]
pub struct VocabularyReport {
    /// Repository-relative path of the GRAPH.md the prose came from, or empty.
    pub source: String,
    pub verbs: Vec<Verb>,
    /// Disagreements between GRAPH.md's tables and the certified lists.
    ///
    /// Empty is the expected state. A non-empty one means the document and the predicate
    /// that gates on it have come apart, and the document is the half that is wrong.
    pub drift: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subject: Option<SubjectCheck>,
}

/// Where the vocabulary's prose lives, in preference order.
///
/// The vendored copy first: it is what the repository actually carries and what
/// `mise run yidam-vendor-update` replaces. `yidam/prelude/` is the yidam repository
/// reading its own source.
fn graph_md(root: &Path) -> Option<PathBuf> {
    let candidates = [
        root.join(".yidam/.vendor/prelude/GRAPH.md"),
        root.join("yidam/prelude/GRAPH.md"),
    ];
    candidates.into_iter().find(|p| p.exists())
}

/// The rows of every table whose header is `| Verb | When |`.
///
/// Anchored on the header rather than on the `## Commit vocabulary` heading, and certainly
/// not on "any table with a backticked first cell" — GRAPH.md carries tables of ref
/// namespaces and directories that look exactly like vocabulary rows and are not. The
/// first version read all of them and reported five directories as undocumented verbs.
///
/// Indifferent to *which* of the two tables a row is in: the kind comes from the certified
/// lists, so a verb that moved between them shows up as drift rather than as a silent
/// reclassification.
pub(crate) fn parse_when_column(text: &str) -> Vec<(String, String)> {
    let cells_of = |line: &str| -> Option<Vec<String>> {
        let t = line.trim();
        if !t.starts_with('|') {
            return None;
        }
        Some(
            t.trim_matches('|')
                .split('|')
                .map(|c| c.trim().to_string())
                .collect(),
        )
    };

    let mut out = Vec::new();
    let mut in_table = false;
    for line in text.lines() {
        let Some(cells) = cells_of(line) else {
            in_table = false;
            continue;
        };
        if cells.len() >= 2
            && cells[0].eq_ignore_ascii_case("verb")
            && cells[1].eq_ignore_ascii_case("when")
        {
            in_table = true;
            continue;
        }
        if !in_table || cells.len() < 2 {
            continue;
        }
        // The `|---|---|` separator.
        if cells[0].chars().all(|c| c == '-' || c == ':') {
            continue;
        }
        let verb = cells[0].trim_matches('`').trim();
        if verb.is_empty() || verb.contains(' ') {
            continue;
        }
        out.push((verb.to_string(), cells[1].clone()));
    }
    out
}

/// Compare the document's tables against the predicate that gates on them.
pub(crate) fn drift_between(documented: &[(String, String)]) -> Vec<String> {
    let mut out = Vec::new();
    for (verb, _) in documented {
        if !is_recognized_verb(verb) {
            out.push(format!(
                "GRAPH.md documents `{verb}`, which the certified vocabulary does not carry"
            ));
        }
    }
    for verb in EPISTEMIC_VERBS.iter().chain(OPERATIONAL_VERBS.iter()) {
        if !documented.iter().any(|(v, _)| v == verb) {
            out.push(format!(
                "`{verb}` is in the certified vocabulary and has no row in GRAPH.md"
            ));
        }
    }
    out
}

fn kind_of(verb: &str) -> &'static str {
    if OPERATIONAL_VERBS.contains(&verb) {
        "operational"
    } else {
        "epistemic"
    }
}

/// `vendor(yidam)` → `("vendor", "yidam")`. None when there is no parenthesised suffix.
fn split_scope(verb: &str) -> Option<(&str, &str)> {
    let open = verb.find('(')?;
    if !verb.ends_with(')') || open == 0 {
        return None;
    }
    Some((&verb[..open], &verb[open + 1..verb.len() - 1]))
}

/// Check one subject line the way `lint --commits` checks a committed one.
/// The closed vocabulary, as a flat list.
///
/// From the certified lists rather than from GRAPH.md: the document supplies the `when`
/// column and nothing else, and `drift_between` exists to report the document as the wrong
/// half when they disagree. Exposed for `serve --mcp`'s `check_subject`, which travels the
/// list with its verdict and must not read a file per call.
pub(crate) fn vocabulary_verbs() -> Vec<&'static str> {
    EPISTEMIC_VERBS
        .iter()
        .chain(OPERATIONAL_VERBS.iter())
        .copied()
        .collect()
}

pub(crate) fn check_subject(text: &str) -> SubjectCheck {
    // `classify_commit` rather than a local parse. The rule this whole check turns on —
    // *everything before the first `: ` is the verb* — is the reason a `(scope)` suffix
    // costs twice, and re-stating it here would be a second implementation of the one
    // sentence the finding is about. There are already three copies of it in this
    // repository; this is not the fourth.
    let event = classify_commit("", text);
    let first = text.lines().next().unwrap_or("").trim();
    let verb = event.verb;
    let recognized = is_recognized_verb(&verb);
    let mut violations = Vec::new();

    if verb.is_empty() {
        violations.push(SubjectViolation {
            rule: "no-verb".to_string(),
            severity: severity(),
            message: "No `verb: ` prefix. Every commit subject begins with one.".to_string(),
        });
    } else if let Some((base, scope)) = split_scope(&verb) {
        // Worth its own rule rather than folding into "unrecognized": this one has a known
        // cause and a known cost, and saying so is the difference between a squiggle that
        // teaches and one that scolds.
        let cost = if kind_of(base) == "operational" {
            "an operational commit"
        } else {
            "the commit"
        };
        let rest = first.split_once(": ").map(|(_, r)| r).unwrap_or("");
        violations.push(SubjectViolation {
            rule: "scope-suffix".to_string(),
            severity: severity(),
            message: format!(
                "`{verb}` carries a conventional-commits scope. Everything before the first \
                 `: ` is the verb, so this costs twice — it is outside the vocabulary, and \
                 classification falls through to Epistemic, filing {cost} as a change in \
                 understanding. Put the scope in the subject: `{base}: {scope} {rest}`."
            ),
        });
    } else if !recognized {
        violations.push(SubjectViolation {
            rule: "unrecognized-verb".to_string(),
            severity: severity(),
            message: format!(
                "`{verb}` is not in the closed vocabulary. Reach for the closest verb rather \
                 than inventing one; a verb outside the list is not a richer description, it \
                 is an unclassifiable one."
            ),
        });
    }

    SubjectCheck {
        kind: match event.kind {
            CommitKind::Operational => "operational",
            CommitKind::Epistemic => "epistemic",
        },
        text: first.to_string(),
        verb,
        recognized,
        violations,
    }
}

pub(crate) fn vocabulary_data(root: &Path, check: Option<&str>) -> VocabularyReport {
    let path = graph_md(root);
    let documented = path
        .as_ref()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .map(|t| parse_when_column(&t))
        .unwrap_or_default();

    let when = |verb: &str| {
        documented
            .iter()
            .find(|(v, _)| v == verb)
            .map(|(_, w)| w.clone())
            .unwrap_or_default()
    };

    let verbs = EPISTEMIC_VERBS
        .iter()
        .chain(OPERATIONAL_VERBS.iter())
        .map(|v| Verb {
            verb: (*v).to_string(),
            kind: kind_of(v),
            when: when(v),
        })
        .collect();

    VocabularyReport {
        source: path
            .map(|p| {
                p.strip_prefix(root)
                    .unwrap_or(&p)
                    .to_string_lossy()
                    .replace('\\', "/")
            })
            .unwrap_or_default(),
        verbs,
        // No document means no comparison, not a table full of missing rows.
        drift: if documented.is_empty() {
            Vec::new()
        } else {
            drift_between(&documented)
        },
        subject: check.map(check_subject),
    }
}

pub(crate) fn render_vocabulary(r: &VocabularyReport) -> String {
    // With `--check`, the answer is the verdict. Reprinting thirty rows above it buries
    // the one line the caller asked for.
    if let Some(s) = &r.subject {
        return render_subject(s);
    }
    let mut out = String::new();
    for kind in ["epistemic", "operational"] {
        out.push_str(&format!(
            "{}{}\n",
            if out.is_empty() { "" } else { "\n" },
            if kind == "epistemic" {
                "Epistemic — understanding was added, revised, or retracted"
            } else {
                "Operational — the pipeline advanced; no understanding changed"
            }
        ));
        for v in r.verbs.iter().filter(|v| v.kind == kind) {
            match v.when.is_empty() {
                true => out.push_str(&format!("  {}\n", v.verb)),
                false => out.push_str(&format!("  {:<12} {}\n", v.verb, v.when)),
            }
        }
    }
    for d in &r.drift {
        out.push_str(&format!("\n[drift] {d}"));
    }
    out.trim_end().to_string()
}

fn render_subject(s: &SubjectCheck) -> String {
    let mut out = format!(
        "{:?}\n  verb {} · {} · {}\n",
        s.text,
        if s.verb.is_empty() {
            "(none)".to_string()
        } else {
            format!("`{}`", s.verb)
        },
        s.kind,
        if s.recognized {
            "in the vocabulary"
        } else {
            "outside the vocabulary"
        }
    );
    for v in &s.violations {
        out.push_str(&format!("  [{}] {}\n", v.rule, v.message));
    }
    out.trim_end().to_string()
}

/// Print the closed commit vocabulary, optionally checking one subject against it.
pub fn vocabulary(check: Option<String>, format: crate::report::Format) -> Result<()> {
    let root = repo_root()?;
    let data = vocabulary_data(&root, check.as_deref());
    if format.is_json() {
        return crate::report::emit(&root, data);
    }
    println!("{}", render_vocabulary(&data));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The comment three SDKs carry — *"kept in sync with the commit vocabulary in
    /// `prelude/GRAPH.md`"* — asserted rather than trusted. It had never been checked.
    #[test]
    fn graph_md_and_the_certified_vocabulary_agree() {
        let text = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../prelude/GRAPH.md"),
        )
        .unwrap();
        let documented = parse_when_column(&text);
        assert!(
            documented.len() >= EPISTEMIC_VERBS.len() + OPERATIONAL_VERBS.len(),
            "the tables parsed to {} rows",
            documented.len()
        );
        let drift = drift_between(&documented);
        assert!(drift.is_empty(), "{}", drift.join("\n"));
    }

    /// A table that is not the vocabulary is not the vocabulary.
    ///
    /// The first version of this parser took any table row with a backticked first cell,
    /// and GRAPH.md's ref-namespace and directory tables look exactly like that. It
    /// reported `corpus/` and `refs/heads/ma/<elector>` as undocumented verbs.
    #[test]
    fn only_a_verb_when_table_is_read_as_vocabulary() {
        let text = "| Ref | Meaning |\n|---|---|\n| `refs/heads/ma/<elector>` | a position |\n\
                    \n\
                    | Verb | When |\n|---|---|\n\
                    | `establish` | New understanding committed |\n";
        let rows = parse_when_column(text);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].0, "establish");
        assert_eq!(rows[0].1, "New understanding committed");
    }

    /// A row the document carries and the predicate does not is drift, not a new verb.
    #[test]
    fn a_documented_verb_outside_the_certified_list_is_drift() {
        let rows = vec![("lift".to_string(), "Invented here".to_string())];
        assert!(drift_between(&rows).iter().any(|d| d.contains("`lift`")));
    }

    #[test]
    fn a_scope_suffix_is_its_own_rule_and_names_the_double_cost() {
        let c = check_subject("vendor(yidam): the prelude at 4e1a2b0");
        assert_eq!(c.verb, "vendor(yidam)");
        assert!(!c.recognized);
        // The cost that is easy to miss: `vendor` is operational, and the scoped form is
        // filed as Epistemic.
        assert_eq!(c.kind, "epistemic");
        assert_eq!(c.violations.len(), 1);
        assert_eq!(c.violations[0].rule, "scope-suffix");
        assert!(c.violations[0].message.contains("an operational"));
        assert!(c.violations[0].message.contains("`vendor: "));
    }

    #[test]
    fn a_legal_subject_has_nothing_to_say() {
        let c = check_subject("establish: the tailwater node");
        assert!(c.recognized);
        assert_eq!(c.kind, "epistemic");
        assert!(c.violations.is_empty());

        let c = check_subject("regen: REGEN blocks refreshed");
        assert_eq!(c.kind, "operational");
        assert!(c.violations.is_empty());
    }

    #[test]
    fn an_invented_verb_and_a_missing_one_are_different_findings() {
        assert_eq!(
            check_subject("lift: something").violations[0].rule,
            "unrecognized-verb"
        );
        assert_eq!(
            check_subject("just some words").violations[0].rule,
            "no-verb"
        );
    }

    /// Only the subject line is checked. A body mentioning a verb is not a subject.
    #[test]
    fn only_the_first_line_is_the_subject() {
        let c = check_subject("establish: a node\n\nlift: this is body prose\n");
        assert_eq!(c.text, "establish: a node");
        assert!(c.violations.is_empty());
    }

    /// The squiggle reports at exactly the severity the gate does.
    #[test]
    fn the_severity_is_the_lint_checks_own() {
        let c = check_subject("lift: something");
        assert_eq!(c.violations[0].severity, "warn");
        assert_eq!(
            c.violations[0].severity,
            super::super::lint::commit_verb_severity().as_str()
        );
    }

    /// A colon with no space is not the separator `classify_commit` reads.
    #[test]
    fn the_separator_is_colon_space() {
        assert_eq!(check_subject("establish:no space").verb, "");
    }

    /// The vendored copy wins, because it is the one `yidam-vendor-update` replaces.
    #[test]
    fn the_vendored_graph_md_is_preferred_over_the_source_tree() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path();
        let vendored = root.join(".yidam/.vendor/prelude/GRAPH.md");
        let source = root.join("yidam/prelude/GRAPH.md");
        std::fs::create_dir_all(source.parent().unwrap()).unwrap();
        std::fs::write(
            &source,
            "| Verb | When |\n|---|---|\n| `establish` | from source |\n",
        )
        .unwrap();
        assert_eq!(graph_md(root), Some(source.clone()));

        std::fs::create_dir_all(vendored.parent().unwrap()).unwrap();
        std::fs::write(
            &vendored,
            "| Verb | When |\n|---|---|\n| `establish` | vendored |\n",
        )
        .unwrap();
        assert_eq!(graph_md(root), Some(vendored));

        let r = vocabulary_data(root, None);
        assert_eq!(r.source, ".yidam/.vendor/prelude/GRAPH.md");
        assert_eq!(
            r.verbs.iter().find(|v| v.verb == "establish").unwrap().when,
            "vendored"
        );
    }

    /// No document at all is not thirty missing rows.
    #[test]
    fn an_absent_graph_md_costs_the_prose_and_reports_no_drift() {
        let tmp = tempfile::TempDir::new().unwrap();
        let r = vocabulary_data(tmp.path(), None);
        assert!(r.source.is_empty());
        assert!(r.drift.is_empty(), "nothing to compare is not disagreement");
        assert_eq!(
            r.verbs.len(),
            EPISTEMIC_VERBS.len() + OPERATIONAL_VERBS.len()
        );
        assert!(r.verbs.iter().all(|v| v.when.is_empty()));
    }
}
