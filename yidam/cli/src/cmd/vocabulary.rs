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
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use yidam_core::git::{
    classify_commit, is_recognized_verb, CommitKind, EPISTEMIC_VERBS, OPERATIONAL_VERBS,
};

use crate::cmd::lint::commits::{autosquash_prefix, is_merge, split_scope, unwrap_revert, verb_of};
use crate::kuten::{Registers, Touch};

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
    /// Why the subject is not the author's to check, when it is not (#1518).
    ///
    /// Text-only: the frozen MCP contract and the report schema carry the verdict — an empty
    /// `violations` — and nothing about why, so this never reaches JSON.
    #[serde(skip)]
    pub exempt: Option<&'static str>,
    /// The subject actually judged, when git wrote a wrapper around it (#1518).
    ///
    /// `verb`, `kind` and `recognized` read the line as written, because the frozen contract
    /// defines `verb` as everything before its first `: `. The violations are about this one.
    /// Text-only for the same reason as [`Self::exempt`].
    #[serde(skip)]
    pub checked: Option<String>,
}

/// What `--check` knows about the commit being written, beyond its message (#1518).
///
/// `lint --commits` reads both from the commit itself — its parent count and the paths it
/// touched. Before the commit exists they come from the repository instead.
#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct Writing {
    /// A merge is in progress, so a generated merge subject is git's.
    pub merging: bool,
    /// The staged change touches only `[object]` paths, which the vocabulary does not govern.
    pub object_only: bool,
}

impl Writing {
    pub(crate) fn of_repo(root: &Path) -> Self {
        let git = || crate::git::Git::new(root);
        let merging = git()
            .args(["rev-parse", "-q", "--verify", "MERGE_HEAD"])
            .succeeded();
        // A merge has no paths of its own in `lint --commits`, so it is never object-only.
        let registers = Registers::of_repo(root);
        let object_only = !merging
            && registers.declares_object()
            && git()
                .args(["diff", "--cached", "--name-only", "-z"])
                .lines()
                .map(|l| {
                    l.iter()
                        .flat_map(|l| l.split('\0'))
                        .filter(|p| !p.is_empty())
                        .map(str::to_string)
                        .collect::<Vec<_>>()
                })
                .is_some_and(|paths| registers.touch(&paths) == Touch::ObjectOnly);
        Self {
            merging,
            object_only,
        }
    }
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

// ── the commit-kind roster (#822) ────────────────────────────────────────────
//
// `report.schema.json` declares `$defs/commit_kind` as a closed enum and nothing compared it
// to the code: adding a third member and re-running the whole `report_goldens` suite left it
// green, the same way a fourth in `$defs/severity` did.
//
// Building the roster turned up why that was easy to miss. The lowercase strings a report
// carries were spelled **three times** in this crate — in `kind_of` below, in the
// `SubjectCheck` match, and again in `log.rs` — and `yidam_core`'s own `CommitKind::as_str`
// returns a **fourth, capitalized** form (`"Epistemic"`) that the schema does not name and no report
// carries. A roster read off any one of those would have been a claim about one copy.
//
// All three now route through `commit_kind_name`, so the roster is the strings the reports
// actually hold.

/// One commit kind, as a report spells it.
///
/// **No wildcard**, deliberately: `CommitKind` lives in `yidam_core`, so a variant added there
/// is a compile error *here*, which is where the report's vocabulary is decided. Not
/// `CommitKind::as_str`, which answers `"Epistemic"` — a different vocabulary, for a different
/// reader.
pub(crate) const fn commit_kind_name(kind: CommitKind) -> &'static str {
    match kind {
        CommitKind::Epistemic => "epistemic",
        CommitKind::Operational => "operational",
    }
}

/// Every commit kind a report can carry — the roster `$defs/commit_kind` is held to.
///
/// Walked along a successor `match` rather than written as `["epistemic", "operational"]`, and
/// that is the whole difference. An array literal takes a new `CommitKind` variant silently:
/// `yidam_core` grows one, this list does not, and the roster is then a subset agreeing with a
/// schema that is also a subset. The `match` below has **no wildcard**, so a third variant is a
/// compile error here; once it is placed the chain yields three elements and the `[_; 2]` width
/// fails to evaluate. Neither can be satisfied without widening the schema too, because
/// `report_goldens.rs` compares the two both ways.
///
/// Deliberately stronger than [`crate::git::REF_STATES`], whose own doc documents the runtime
/// scan it settled for.
pub const COMMIT_KINDS: [&str; 2] = {
    let mut out = [""; 2];
    let mut cur = Some(CommitKind::Epistemic);
    let mut i = 0;
    while let Some(k) = cur {
        // Successor first, name second. `CommitKind` is not `Copy` and `commit_kind_name` takes
        // it by value, so that call is the last read of `k`; the `match` above it binds nothing
        // and therefore does not move. Re-spelling the names here to dodge the ordering would
        // have put a second copy of the vocabulary in the roster that exists to have one.
        cur = match k {
            CommitKind::Epistemic => Some(CommitKind::Operational),
            CommitKind::Operational => None,
        };
        out[i] = commit_kind_name(k);
        i += 1;
    }
    out
};

fn kind_of(verb: &str) -> &'static str {
    commit_kind_name(if OPERATIONAL_VERBS.contains(&verb) {
        CommitKind::Operational
    } else {
        CommitKind::Epistemic
    })
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

/// [`check_subject_in`] with nothing known about the commit being written.
#[cfg(test)]
pub(crate) fn check_subject(text: &str) -> SubjectCheck {
    check_subject_in(text, Writing::default())
}

/// [`check_subject`] for a commit about to be written in `root`: what `--check` and
/// `serve --mcp`'s `check_subject` both answer, so the two cannot disagree (#1518).
pub(crate) fn check_subject_at(root: &Path, text: &str) -> SubjectCheck {
    check_subject_in(text, Writing::of_repo(root))
}

/// The subject a message will leave behind once git's wrappers are gone (#1518).
///
/// `Revert "…"`/`Reapply "…"` are peeled to the reverted subject, and an autosquash marker
/// to the subject the squash keeps: the target's, after `fixup! ` or `squash! `, or the
/// replacement an `amend! ` commit carries as the first line of its body. None when there
/// was no wrapper.
fn unwrapped(text: &str) -> Option<String> {
    let first = text.lines().next().unwrap_or("").trim();
    let mut inner = first.to_string();
    // The body belongs to the outermost subject only; an inner marker has none.
    let mut body = Some(text);
    loop {
        if let Some(rest) = unwrap_revert(&inner) {
            inner = rest.to_string();
            body = None;
        } else if let Some(marker) = autosquash_prefix(&inner) {
            let rest = inner[marker.len()..].to_string();
            let replacement = (marker == "amend! ")
                .then_some(body)
                .flatten()
                .and_then(|t| t.lines().skip(1).map(str::trim).find(|l| !l.is_empty()));
            inner = replacement.map(str::to_string).unwrap_or(rest);
            body = None;
        } else {
            break;
        }
    }
    (inner != first).then_some(inner)
}

/// [`check_subject`], knowing what is being written (#1518).
///
/// Two subjects are not the author's to check at all: a generated merge subject — only when
/// a merge is in progress, because [`is_merge`] requires two parents and a bare
/// `Merge branch '…'` on an ordinary commit is authored — and any subject on a commit whose
/// staged paths are all `[object]` paths, which `lint --commits` does not hold to the
/// vocabulary either. Subjects git wraps — reverts and autosquash markers — are checked by
/// what the wrapper holds (see [`unwrapped`]); a reverted merge subject is git's.
pub(crate) fn check_subject_in(text: &str, writing: Writing) -> SubjectCheck {
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
    let checked = unwrapped(text);
    let judged = checked.as_deref().unwrap_or(first);
    let exempt = if is_merge(first, if writing.merging { 2 } else { 1 }) {
        Some("git wrote this merge subject")
    } else if writing.object_only {
        Some(
            "the staged change touches only `[object]` paths, which the vocabulary does not govern",
        )
    } else if checked.is_some() && is_merge(judged, 2) {
        Some("git wrote the merge subject this reverts")
    } else {
        None
    };
    // What the violations are about, read the same way `classify_commit` reads `verb`.
    let judged_verb = if checked.is_some() {
        verb_of(judged)
    } else {
        verb.clone()
    };
    let inside = match &checked {
        Some(c) => format!("Inside git's wrapper, the subject is {c:?}. "),
        None => String::new(),
    };

    if exempt.is_some() {
        // Nothing to say: the subject is not the author's to check.
    } else if judged_verb.is_empty() {
        violations.push(SubjectViolation {
            rule: "no-verb".to_string(),
            severity: severity(),
            message: format!("{inside}No `verb: ` prefix. Every commit subject begins with one."),
        });
    } else if let Some((base, scope)) = split_scope(&judged_verb) {
        // Worth its own rule rather than folding into "unrecognized": this one has a known
        // cause and a known cost, and saying so is the difference between a squiggle that
        // teaches and one that scolds.
        let cost = if kind_of(base) == "operational" {
            "an operational commit"
        } else {
            "the commit"
        };
        let rest = judged.split_once(": ").map(|(_, r)| r).unwrap_or("");
        violations.push(SubjectViolation {
            rule: "scope-suffix".to_string(),
            severity: severity(),
            message: format!(
                "{inside}`{judged_verb}` carries a conventional-commits scope. Everything \
                 before the first `: ` is the verb, so this costs twice — it is outside the \
                 vocabulary, and classification falls through to Epistemic, filing {cost} as \
                 a change in understanding. Put the scope in the subject: \
                 `{base}: {scope} {rest}`."
            ),
        });
    } else if !is_recognized_verb(&judged_verb) {
        violations.push(SubjectViolation {
            rule: "unrecognized-verb".to_string(),
            severity: severity(),
            message: format!(
                "{inside}`{judged_verb}` is not in the closed vocabulary. Reach for the \
                 closest verb rather than inventing one; a verb outside the list is not a \
                 richer description, it is an unclassifiable one."
            ),
        });
    }

    SubjectCheck {
        kind: commit_kind_name(event.kind),
        text: first.to_string(),
        verb,
        recognized,
        violations,
        exempt,
        checked,
    }
}

/// `writing` is what the repository says about the commit being written (#1518).
fn vocabulary_data(root: &Path, check: Option<&str>, writing: Writing) -> VocabularyReport {
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
        subject: check.map(|c| check_subject_in(c, writing)),
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
        let _ = writeln!(
            out,
            "{}{}",
            if out.is_empty() { "" } else { "\n" },
            if kind == "epistemic" {
                "Epistemic — understanding was added, revised, or retracted"
            } else {
                "Operational — the pipeline advanced; no understanding changed"
            }
        );
        for v in r.verbs.iter().filter(|v| v.kind == kind) {
            match v.when.is_empty() {
                true => {
                    let _ = writeln!(out, "  {}", v.verb);
                }
                false => {
                    let _ = writeln!(out, "  {:<12} {}", v.verb, v.when);
                }
            }
        }
    }
    for d in &r.drift {
        let _ = write!(out, "\n[drift] {d}");
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
    if let Some(inner) = &s.checked {
        let _ = writeln!(
            out,
            "  checked: {inner:?} — the subject inside git's wrapper"
        );
    }
    if let Some(why) = s.exempt {
        let _ = writeln!(out, "  exempt: {why}");
    }
    for v in &s.violations {
        let _ = writeln!(out, "  [{}] {}", v.rule, v.message);
    }
    out.trim_end().to_string()
}

/// Print the closed commit vocabulary, optionally checking one subject against it.
pub fn vocabulary(
    root: Option<&std::path::Path>,
    check: Option<String>,
    format: crate::report::Format,
) -> Result<()> {
    let root = crate::paths::resolve_root(root)?;
    // `--check` runs before the commit exists, so what `lint --commits` reads from the
    // commit — its parents, its paths — is read from the repository (#1518). Only asked
    // when there is a subject to check.
    let writing = match check {
        Some(_) => Writing::of_repo(&root),
        None => Writing::default(),
    };
    let data = vocabulary_data(&root, check.as_deref(), writing);
    // `--check` is a gate (#1518): a hook or CI step reads the exit code, and a verdict
    // printed under exit 0 refuses nothing. Listing mode has no verdict and stays 0.
    let passed = data
        .subject
        .as_ref()
        .is_none_or(|s| s.violations.is_empty());
    crate::report::gate(&root, format, data, passed, |r| {
        println!("{}", render_vocabulary(r))
    })
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

    /// #1518: a subject git wraps is checked by what the wrapper holds, so a commit-msg hook
    /// neither refuses `git revert` nor lets `Revert "wip"` through.
    #[test]
    fn a_wrapped_subject_is_judged_by_what_it_holds() {
        for text in [
            "Revert \"establish: the tailwater node\"",
            "Reapply \"establish: the tailwater node\"",
            "Revert \"Revert \"establish: the tailwater node\"\"",
            "fixup! establish: the tailwater node",
            "squash! establish: the tailwater node",
            "fixup! fixup! establish: the tailwater node",
            "amend! establish: the tailwater node",
        ] {
            let c = check_subject(text);
            assert!(c.violations.is_empty(), "{text}: {:?}", c.violations);
            assert_eq!(
                c.checked.as_deref(),
                Some("establish: the tailwater node"),
                "{text}"
            );
            assert!(c.exempt.is_none(), "{text}");
        }
        for (text, rule) in [
            ("Revert \"wip\"", "no-verb"),
            ("Reapply \"lift: a node\"", "unrecognized-verb"),
            ("fixup! wip", "no-verb"),
            ("squash! fix(cli): a node", "scope-suffix"),
        ] {
            let c = check_subject(text);
            assert_eq!(c.violations.len(), 1, "{text}");
            assert_eq!(c.violations[0].rule, rule, "{text}");
            assert!(
                c.violations[0].message.starts_with("Inside git's wrapper"),
                "{text}"
            );
        }
        // A reverted merge subject is git's twice over.
        let c = check_subject("Revert \"Merge branch 'x' into main\"");
        assert!(c.violations.is_empty(), "{:?}", c.violations);
        assert!(c.exempt.is_some());
        // An authored revert names a verb like anything else.
        assert_eq!(
            check_subject("Revert the cache change").violations[0].rule,
            "no-verb"
        );
    }

    /// #1518: `--fixup=amend:` keeps the body's first line as the new subject, so that is
    /// the line checked — not the target's subject after the marker.
    #[test]
    fn an_amend_is_judged_by_its_replacement_subject() {
        let c = check_subject("amend! establish: a node\n\nwip\n\nthe body\n");
        assert_eq!(c.checked.as_deref(), Some("wip"));
        assert_eq!(c.violations[0].rule, "no-verb");

        let c = check_subject("amend! wip\n\nrevise: a node\n");
        assert_eq!(c.checked.as_deref(), Some("revise: a node"));
        assert!(c.violations.is_empty(), "{:?}", c.violations);
    }

    /// #1518: a generated merge subject is exempt only while a merge is being written —
    /// `lint --commits` exempts it only at two parents, and the hook must agree.
    #[test]
    fn a_merge_subject_is_exempt_only_while_merging() {
        let text = "Merge branch 'main' into phase/x";
        let merging = Writing {
            merging: true,
            ..Writing::default()
        };
        let c = check_subject_in(text, merging);
        assert!(c.violations.is_empty(), "{:?}", c.violations);
        assert!(c.exempt.is_some());

        let c = check_subject_in(text, Writing::default());
        assert_eq!(c.violations[0].rule, "no-verb");
        assert!(c.exempt.is_none());
    }

    /// #1518: `lint --commits` does not hold an object-only commit to the vocabulary, so the
    /// hook does not refuse its subject either.
    #[test]
    fn an_object_only_commit_is_exempt() {
        let object_only = Writing {
            object_only: true,
            ..Writing::default()
        };
        let c = check_subject_in("tweak the widget", object_only);
        assert!(c.violations.is_empty(), "{:?}", c.violations);
        assert!(c.exempt.is_some());
    }

    /// The exemption is text-only: the frozen contract carries no field for it.
    #[test]
    fn the_exemption_never_reaches_json() {
        let v = serde_json::to_value(check_subject("fixup! establish: x")).unwrap();
        assert!(v.get("exempt").is_none(), "{v}");
        assert!(v.get("checked").is_none(), "{v}");
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

        let r = vocabulary_data(root, None, Writing::default());
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
        let r = vocabulary_data(tmp.path(), None, Writing::default());
        assert!(r.source.is_empty());
        assert!(r.drift.is_empty(), "nothing to compare is not disagreement");
        assert_eq!(
            r.verbs.len(),
            EPISTEMIC_VERBS.len() + OPERATIONAL_VERBS.len()
        );
        assert!(r.verbs.iter().all(|v| v.when.is_empty()));
    }
}
