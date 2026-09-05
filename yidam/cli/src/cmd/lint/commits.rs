//! Checking the git log against the closed commit vocabulary in `prelude/GRAPH.md`.
//!
//! Separate from the corpus checks because it reads a different thing — history rather
//! than working tree — and because a repository can be perfectly well-formed and still
//! have an illegible log. That is the failure this catches, and it is invisible to every
//! other check here.

use std::path::Path;
use std::process::Command;

use yidam_core::git::is_recognized_verb;

use super::model::{Check, Severity, Violation};

/// One commit's subject, reduced to what the vocabulary cares about.
pub struct Subject {
    pub hash: String,
    pub verb: String,
    pub text: String,
    /// How many parents the commit has. Two or more means git may have written the
    /// subject, which is what [`is_merge`] turns on.
    pub parents: usize,
}

/// Read commit subjects, newest first.
///
/// `range` is any git revision range. `None` reads all of history — which for a corpus is
/// usually what you want, since the log *is* the record and its legibility is not a
/// property of the last twenty commits.
pub fn read_subjects(root: &Path, range: Option<&str>) -> Vec<Subject> {
    let mut args = vec!["log".to_string(), "--format=%H%x00%P%x00%s".to_string()];
    if let Some(r) = range {
        args.push(r.to_string());
    }
    let Ok(out) = Command::new("git").current_dir(root).args(&args).output() else {
        return vec![];
    };
    if !out.status.success() {
        return vec![];
    }
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|line| {
            let mut fields = line.splitn(3, '\0');
            let hash = fields.next()?;
            let parents = fields.next()?;
            let subject = fields.next()?;
            Some(Subject {
                hash: hash.to_string(),
                verb: verb_of(subject),
                text: subject.to_string(),
                parents: parents.split_whitespace().count(),
            })
        })
        .collect()
}

/// The leading verb of a subject line, or empty if it has no `verb: ` prefix.
///
/// Matches the parse in `classify_commit` so the two cannot disagree about where the verb
/// ends.
fn verb_of(subject: &str) -> String {
    match subject.find(": ") {
        Some(pos) => subject[..pos].trim().to_string(),
        None => String::new(),
    }
}

/// Whether this commit's subject is one git generated rather than one somebody chose.
///
/// Exemption turns on **parent count plus a generated-looking subject**, not on the subject
/// alone. Matching prefixes was the first implementation and it under-matched: `git merge
/// rigpa/electoral-purpose` produces the bare `Merge <ref>` form, which starts with neither
/// `Merge branch` nor `Merge pull request`, and a derived repository wrote ten of them —
/// every one reported as a commit with no verb, which is true and is not the author's fault.
///
/// Parent count alone would over-match in the other direction. A merge is a synthesis event
/// and `git merge -m` takes a real subject; the same repository wrote 23 of those. Exempting
/// every merge would stop checking the verb on the commits most worth checking it on, since
/// a merge is where two inquiry threads join. So: a merge whose subject still looks
/// git-generated is exempt, and a merge whose author wrote something is checked like any
/// other commit.
pub(crate) fn is_merge(subject: &str, parents: usize) -> bool {
    parents >= 2 && subject.starts_with("Merge ")
}

pub fn unrecognized_verb(subjects: &[Subject]) -> Check {
    let violations = subjects
        .iter()
        .filter(|s| !is_merge(&s.text, s.parents))
        .filter(|s| !is_recognized_verb(&s.verb))
        .map(|s| {
            let short = &s.hash[..s.hash.len().min(8)];
            // The hash is already the node; repeating it here would print it twice.
            let detail = if s.verb.is_empty() {
                format!("no `verb: ` prefix — {:?}", truncate(&s.text))
            } else {
                format!("`{}` is not in the vocabulary", s.verb)
            };
            // Identified by commit, not by file: history is immutable, so a baselined
            // entry here stays put and the ratchet gates only what is authored next.
            Violation::new(short, detail)
        })
        .collect();
    Check::new(
        "unrecognized-verb",
        "Commit outside the closed vocabulary",
        Severity::Warn,
        "The commit vocabulary in GRAPH.md is what makes the epistemic/operational split \
         recoverable by anything other than a person reading all of it. An open vocabulary \
         decays into one verb per commit: a repository derived from this template ran ~60 \
         distinct leading words across 100 commits, each evocative and collectively \
         useless. Reported rather than gated, because history cannot be rewritten to fix it. \
         Nor can it be blessed away: the baseline records error-severity checks only, so a \
         pre-vocabulary log keeps reporting. Read the findings as a guide to what to write \
         next, not as a list to clear.",
        violations,
    )
}

fn truncate(s: &str) -> String {
    if s.chars().count() <= 50 {
        return s.to_string();
    }
    format!("{}…", s.chars().take(49).collect::<String>())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An ordinary single-parent commit.
    fn subj(hash: &str, text: &str) -> Subject {
        merge_subj(hash, text, 1)
    }

    fn merge_subj(hash: &str, text: &str, parents: usize) -> Subject {
        Subject {
            hash: hash.to_string(),
            verb: verb_of(text),
            text: text.to_string(),
            parents,
        }
    }

    #[test]
    fn verb_is_the_text_before_the_first_colon_space() {
        assert_eq!(verb_of("establish: a thing"), "establish");
        assert_eq!(verb_of("no verb here"), "");
        // A scope in parentheses is part of the verb token, and is not in the vocabulary.
        assert_eq!(verb_of("fix(cli): a thing"), "fix(cli)");
    }

    #[test]
    fn vocabulary_verbs_pass() {
        let s = vec![
            subj("aaaaaaaa", "establish: confounding as a corpus concept"),
            subj("bbbbbbbb", "extract: 14 permit fields from document Y"),
            subj("cccccccc", "phase: the local half settles"),
        ];
        assert!(unrecognized_verb(&s).passed());
    }

    #[test]
    fn an_invented_verb_is_reported_with_its_hash() {
        let s = vec![subj(
            "deadbeef",
            "viewport: charts that cannot draw a number",
        )];
        let c = unrecognized_verb(&s);
        assert_eq!(c.violations.len(), 1);
        assert_eq!(c.violations[0].node, "deadbeef");
        assert!(c.violations[0].detail.contains("viewport"));
    }

    #[test]
    fn a_subject_with_no_verb_is_reported() {
        let s = vec![subj("deadbeef", "just some words")];
        let c = unrecognized_verb(&s);
        assert_eq!(c.violations.len(), 1);
        assert!(
            c.violations[0].detail.contains("no `verb: ` prefix"),
            "{}",
            c.violations[0].detail
        );
    }

    #[test]
    fn git_generated_merge_subjects_are_exempt() {
        // git authors these; there is no verb to choose.
        let s = vec![
            merge_subj("aaaaaaaa", "Merge branch 'phase/outcome-axis'", 2),
            merge_subj("bbbbbbbb", "Merge pull request #12 from x/y", 2),
            // The bare form, from `git merge rigpa/x`. The prefix test missed this and a
            // derived repository wrote ten of them.
            merge_subj("cccccccc", "Merge rigpa/electoral-purpose", 2),
            merge_subj("dddddddd", "Merge remote-tracking branch 'origin/main'", 2),
        ];
        assert!(unrecognized_verb(&s).passed());
    }

    #[test]
    fn an_authored_merge_subject_is_checked_like_any_other() {
        // `git merge -m "adopt: …"` is the prescribed form: a merge is a synthesis event
        // and deserves a verb. Exempting it would stop checking the commits where two
        // inquiry threads join, which is where the check earns its keep.
        let ok = vec![merge_subj(
            "aaaaaaaa",
            "adopt: the baseline after electoral-purpose",
            2,
        )];
        assert!(unrecognized_verb(&ok).passed());

        let bad = vec![merge_subj("bbbbbbbb", "lift: the baseline forward", 2)];
        let c = unrecognized_verb(&bad);
        assert_eq!(c.violations.len(), 1);
        assert!(c.violations[0].detail.contains("lift"));
    }

    #[test]
    fn a_single_parent_commit_named_merge_is_not_exempt() {
        // Parent count is half the test precisely so that an ordinary commit cannot buy
        // exemption by starting its subject with the word.
        let s = vec![subj("deadbeef", "Merge the two threads by hand")];
        assert!(!unrecognized_verb(&s).passed());
    }

    #[test]
    fn the_collective_verbs_pass() {
        let s = vec![
            subj(
                "aaaaaaaa",
                "resolve: the maneuver class, redefined documentarily",
            ),
            subj("bbbbbbbb", "adopt: the baseline after maneuver-class"),
            subj(
                "cccccccc",
                "scope: the whole commission swept — one member of five",
            ),
        ];
        assert!(unrecognized_verb(&s).passed());
    }

    #[test]
    fn the_check_warns_rather_than_gates() {
        // History is immutable; failing on it would wedge every existing repository shut.
        assert_eq!(unrecognized_verb(&[]).severity, Severity::Warn);
    }

    #[test]
    fn a_long_subject_is_truncated_in_the_detail() {
        let long = "x".repeat(200);
        let s = vec![subj("deadbeef", &long)];
        let c = unrecognized_verb(&s);
        assert!(c.violations[0].detail.len() < 120);
    }
}
