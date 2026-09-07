//! Checking the git log against the closed commit vocabulary in `prelude/GRAPH.md`.
//!
//! Separate from the corpus checks because it reads a different thing — history rather
//! than working tree — and because a repository can be perfectly well-formed and still
//! have an illegible log. That is the failure this catches, and it is invisible to every
//! other check here.

use std::path::Path;
use std::process::Command;

use yidam_core::git::is_recognized_verb;

use crate::kuten::{Registers, Touch};

use super::model::{Check, Severity, Violation};

/// One commit's subject, reduced to what the vocabulary cares about.
pub struct Subject {
    pub hash: String,
    pub verb: String,
    pub text: String,
    /// How many parents the commit has. Two or more means git may have written the
    /// subject, which is what [`is_merge`] turns on.
    pub parents: usize,
    /// The repository-relative paths the commit touched, as `--name-only` reports them.
    ///
    /// **Empty is a real state and not a parse failure.** `git log --name-only` prints no
    /// names for a merge commit, so every merge arrives here with nothing — which
    /// [`Touch::None`] handles by keeping the commit under the corpus register.
    pub paths: Vec<String>,
}

/// Read commit subjects, newest first.
///
/// `range` is any git revision range. `None` reads all of history — which for a corpus is
/// usually what you want, since the log *is* the record and its legibility is not a
/// property of the last twenty commits.
///
/// # Why a record separator
///
/// The format is prefixed with `%x1e`, because `--name-only` makes a commit's output span
/// an unknown number of lines and the old line-per-commit parse cannot see where one commit
/// ends. Splitting the whole output on `\x1e` puts each commit back in one piece: its first
/// line is the `hash\0parents\0subject` header, and every non-empty line after it is a path.
pub fn read_subjects(root: &Path, range: Option<&str>) -> Vec<Subject> {
    let mut args = vec![
        // Without this git renders a path holding any byte above ASCII as a C-quoted
        // string — `"docs/\303\251tude.md"` — and the register globs would then be matched
        // against the quoting rather than against the path.
        "-c".to_string(),
        "core.quotePath=false".to_string(),
        "log".to_string(),
        "--format=%x1e%H%x00%P%x00%s".to_string(),
        "--name-only".to_string(),
    ];
    if let Some(r) = range {
        args.push(r.to_string());
    }
    let Ok(out) = Command::new("git").current_dir(root).args(&args).output() else {
        return vec![];
    };
    if !out.status.success() {
        return vec![];
    }
    parse_records(&String::from_utf8_lossy(&out.stdout))
}

/// Split `git log`'s output on the record separator and read one [`Subject`] from each.
fn parse_records(text: &str) -> Vec<Subject> {
    text.split('\u{1e}')
        .filter_map(|record| {
            let mut lines = record.lines();
            let header = lines.next()?;
            let mut fields = header.splitn(3, '\0');
            let hash = fields.next()?;
            let parents = fields.next()?;
            let subject = fields.next()?;
            Some(Subject {
                hash: hash.to_string(),
                verb: verb_of(subject),
                text: subject.to_string(),
                parents: parents.split_whitespace().count(),
                paths: lines
                    .filter(|l| !l.trim().is_empty())
                    .map(|l| l.to_string())
                    .collect(),
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

/// `vendor(yidam)` → `("vendor", "yidam")`. None when there is no parenthesised suffix.
///
/// Beside [`verb_of`] because it reads the same parse from the other end: `verb_of` says
/// where the verb ends, and this says what the author meant by the part of it GRAPH.md
/// forbids. Two callers need that answer for two unrelated reasons — `yidam vocabulary`, to
/// name the cost in its finding, and [`crate::kuten::settles_a_phase`], to answer a question
/// about practice rather than about spelling — and a second copy could disagree with the
/// finding about what a scope even is.
///
/// **This is not a step in recognising a verb, and nothing here may make it one.**
/// `is_recognized_verb` never sees this function, `classify_commit` never sees it, and
/// `lint --commits` still reports `phase(x):` as outside the vocabulary. Stripping the suffix
/// before matching is the rule GRAPH.md's commit-vocabulary section forbids, and it is what
/// A0's extraction script did by accident — reading exactly zero off-vocabulary commits out
/// of a population that held four (#644).
pub(crate) fn split_scope(verb: &str) -> Option<(&str, &str)> {
    let open = verb.find('(')?;
    if !verb.ends_with(')') || open == 0 {
        return None;
    }
    Some((&verb[..open], &verb[open + 1..verb.len() - 1]))
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

/// The corpus vocabulary, checked against the commits the corpus register governs.
///
/// # The register scopes recognition and never classification
///
/// RFC-0028 §4 decided arm (b): the filter sits here, before [`is_recognized_verb`], and
/// `classify_commit` is untouched. Classification stays one total function everywhere — a
/// `feat:` on the artifact is Epistemic to `log`, to the three SDKs and to the Dafny proof
/// alike — and only *jurisdiction* differs. A register-aware classifier would be
/// corpus-relative, and a corpus-relative parity function cannot be pinned by fixtures of the
/// shape `(hash, message) → (kind, verb, subject)`, which is what the parity surface is.
///
/// The harm lives in this report and nowhere else. A0 reported **40** off-vocabulary commits
/// in one derived repository and **75** in another touching no corpus file at all — `feat:`,
/// `fix:`, `test:` on the artifact, every one reported here as a corpus-vocabulary violation,
/// which it is not. Re-measured 2026-09-06 with this code, declaring every top-level path but
/// `.yidam/` as the object: **30** of matt-huffman's 132 and **72** of
/// ohio-education-funding's 211. The gap is method — `--name-only` lists nothing for a merge,
/// so a merge cannot be counted as artifact-only here, and 92 of matt-huffman's are merges.
///
/// # What is *not* filtered
///
/// Only [`Touch::ObjectOnly`]. A commit spanning both registers is corpus work that also
/// touched the artifact, and it stays governed; a commit with no paths at all stays governed
/// for the reason [`Touch::None`] gives. So a repository declaring no object reports exactly
/// what it reported before this parameter existed — [`Registers::corpus_only`] puts every
/// path in the corpus and no commit can reach `ObjectOnly`.
pub fn unrecognized_verb(subjects: &[Subject], registers: &Registers) -> Check {
    let violations = subjects
        .iter()
        .filter(|s| !is_merge(&s.text, s.parents))
        .filter(|s| registers.touch(&s.paths) != Touch::ObjectOnly)
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

    /// The check as every repository declaring no object runs it.
    fn unrecognized_verb(subjects: &[Subject]) -> Check {
        super::unrecognized_verb(subjects, &Registers::corpus_only())
    }

    /// An ordinary single-parent commit, touching one corpus file.
    fn subj(hash: &str, text: &str) -> Subject {
        merge_subj(hash, text, 1)
    }

    /// The same commit, touching the paths given.
    fn touching(hash: &str, text: &str, paths: &[&str]) -> Subject {
        Subject {
            paths: paths.iter().map(|p| p.to_string()).collect(),
            ..subj(hash, text)
        }
    }

    fn merge_subj(hash: &str, text: &str, parents: usize) -> Subject {
        Subject {
            paths: vec![".yidam/corpus/node.md".to_string()],
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

    // -- the parse ------------------------------------------------------------------
    //
    // `--name-only` makes one commit span an unknown number of lines, so the record
    // separator is what puts it back together. These pin the shape git actually emits.

    #[test]
    fn a_record_carries_its_paths() {
        let out = "\u{1e}aaaaaaaa\0bbbbbbbb\0extract: a thing\n\n\
                   .yidam/corpus/a.md\nweb/index.html\n";
        let s = parse_records(out);
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].verb, "extract");
        assert_eq!(s[0].paths, vec![".yidam/corpus/a.md", "web/index.html"]);
    }

    #[test]
    fn a_merge_record_carries_no_paths() {
        // git prints no names for a merge commit without `-m`, which is where
        // `Touch::None` comes from in the field rather than in a fixture.
        let out = "\u{1e}aaaaaaaa\0bbbbbbbb cccccccc\0Merge branch 'x'\n";
        let s = parse_records(out);
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].parents, 2);
        assert!(s[0].paths.is_empty());
    }

    #[test]
    fn records_do_not_bleed_into_each_other() {
        // The old parse read one commit per line. With paths on their own lines it would
        // have read `web/index.html` as a commit; this asserts the boundary is the
        // separator and not the newline.
        let out = "\u{1e}aaaaaaaa\0p\0extract: one\n\nweb/index.html\n\
                   \u{1e}bbbbbbbb\0p\0open: two\n\n.yidam/corpus/b.md\n";
        let s = parse_records(out);
        assert_eq!(s.len(), 2);
        assert_eq!(s[0].paths, vec!["web/index.html"]);
        assert_eq!(s[1].paths, vec![".yidam/corpus/b.md"]);
    }

    // -- the registers --------------------------------------------------------------

    /// A repository that declares `web/` and `crates/` as its object.
    fn object_coupled() -> Registers {
        Registers::of_globs(vec!["web/**".into(), "crates/**".into()])
    }

    /// **G7** — a commit touching no corpus path is not a vocabulary finding.
    ///
    /// This is the defect, and its size is measured rather than supposed: **40**
    /// off-vocabulary commits in matt-huffman and **75** in ohio-education-funding touch no
    /// corpus file at all. `feat:` on the artifact was reported as a corpus-vocabulary
    /// violation, which it is not.
    #[test]
    fn a_commit_touching_only_the_object_is_not_a_vocabulary_finding() {
        let s = vec![
            touching("aaaaaaaa", "feat: add dark mode", &["web/app.tsx"]),
            touching(
                "bbbbbbbb",
                "test: cover pagination",
                &["crates/x/src/lib.rs"],
            ),
        ];
        assert!(super::unrecognized_verb(&s, &object_coupled()).passed());
        // And the same two commits are findings in a repository that declares no object,
        // so the silence is the declaration's doing and not the subject's.
        assert_eq!(unrecognized_verb(&s).violations.len(), 2);
    }

    /// The corpus half is untouched: `establish:` on the corpus passes and an invented verb
    /// on the corpus is still reported, in the very same repository.
    #[test]
    fn the_corpus_register_is_checked_exactly_as_before() {
        let r = object_coupled();
        let ok = vec![touching(
            "aaaaaaaa",
            "establish: confounding as a corpus concept",
            &[".yidam/corpus/confounding.md"],
        )];
        assert!(super::unrecognized_verb(&ok, &r).passed());

        let bad = vec![touching(
            "bbbbbbbb",
            "viewport: charts that cannot draw a number",
            &[".yidam/corpus/charts.md"],
        )];
        let c = super::unrecognized_verb(&bad, &r);
        assert_eq!(c.violations.len(), 1);
        assert_eq!(c.violations[0].node, "bbbbbbbb");
    }

    /// A commit spanning both registers is corpus work that also touched the artifact, and
    /// the corpus register governs it. **The mixed-register conduct finding is #643's**, on a
    /// measurement showing it would raise ~710 findings across six corpora that are already
    /// 100% vocabulary-conformant — most often on `regen`, the export act whose job is to
    /// cross the registers. Nothing new is reported here.
    #[test]
    fn a_commit_spanning_both_registers_is_governed_by_the_corpus() {
        let r = object_coupled();
        let s = vec![touching(
            "aaaaaaaa",
            "feat: add dark mode",
            &["web/app.tsx", ".yidam/corpus/theme.md"],
        )];
        let c = super::unrecognized_verb(&s, &r);
        assert_eq!(c.violations.len(), 1, "the corpus register governs it");
        assert!(c.violations[0].detail.contains("feat"));
        // …and it is one finding, not two. No conduct rule fires here.
        assert_eq!(c.violations[0].node, "aaaaaaaa");
    }

    /// **G8** — a commit with no paths is governed by the corpus register.
    ///
    /// `git log --name-only` lists nothing for a merge, so an authored merge — `git merge -m
    /// "lift: …"`, the form the vocabulary asks for — reaches the check with an empty path
    /// list. Absence of evidence is not a declaration of jurisdiction. Reading it as
    /// `ObjectOnly` would silence the verb check on exactly the commits where two inquiry
    /// threads join.
    #[test]
    fn a_commit_with_no_paths_is_governed_by_the_corpus_register() {
        let r = object_coupled();
        assert_eq!(r.touch(&[]), Touch::None);

        let authored_merge = Subject {
            paths: vec![],
            ..merge_subj("aaaaaaaa", "lift: the baseline forward", 2)
        };
        let c = super::unrecognized_verb(&[authored_merge], &r);
        assert_eq!(c.violations.len(), 1, "an authored merge is still checked");
        assert!(c.violations[0].detail.contains("lift"));

        // A git-generated merge subject stays exempt for the reason it always was — nobody
        // chose its verb — and not because it has no paths.
        let generated = Subject {
            paths: vec![],
            ..merge_subj("bbbbbbbb", "Merge branch 'phase/outcome-axis'", 2)
        };
        assert!(super::unrecognized_verb(&[generated], &r).passed());
    }

    /// **G9** — a repository declaring no object reports byte-identically to today.
    ///
    /// The population covers every shape the register split can see: a corpus path, an
    /// artifact-looking path, both together, and none at all. Under
    /// [`Registers::corpus_only`] every one of them is corpus work, so the expected list is
    /// what the check produced before it took a register at all — hard-coded here rather
    /// than recomputed, because a recomputed expectation moves with the mutation.
    #[test]
    fn a_repository_declaring_no_object_reports_what_it_always_did() {
        let s = vec![
            touching("aaaaaaaa", "feat: add dark mode", &["web/app.tsx"]),
            touching(
                "bbbbbbbb",
                "extract: 14 permit fields",
                &[".yidam/corpus/p.md"],
            ),
            touching(
                "cccccccc",
                "viewport: charts",
                &["web/x.tsx", ".yidam/corpus/c.md"],
            ),
            Subject {
                paths: vec![],
                ..subj("dddddddd", "just some words")
            },
            touching(
                "eeeeeeee",
                "test: cover pagination",
                &["crates/x/src/lib.rs"],
            ),
        ];
        let got: Vec<(String, String)> = unrecognized_verb(&s)
            .violations
            .into_iter()
            .map(|v| (v.node, v.detail))
            .collect();
        assert_eq!(
            got,
            vec![
                (
                    "aaaaaaaa".to_string(),
                    "`feat` is not in the vocabulary".to_string()
                ),
                (
                    "cccccccc".to_string(),
                    "`viewport` is not in the vocabulary".to_string()
                ),
                (
                    "dddddddd".to_string(),
                    "no `verb: ` prefix — \"just some words\"".to_string()
                ),
                (
                    "eeeeeeee".to_string(),
                    "`test` is not in the vocabulary".to_string()
                ),
            ]
        );
    }
}
