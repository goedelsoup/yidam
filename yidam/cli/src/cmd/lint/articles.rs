//! A domain article binds as the genesis commit wrote it (#593).
//!
//! The constitution lets a bootstrap append domain articles at genesis. They are permanent and
//! may not contradict Articles I–VI. Until this module, such an article was prose in a vendored
//! file, and nothing read it. It was not even durable: the vendored prelude is replaced whole
//! on a re-vendor.
//!
//! # Where an article lives
//!
//! `.yidam/constitution/<name>.md` is the article's prose. `<name>.rego` beside it, where there
//! is one, is its rule, and `<name>_test.rego` holds the rule's cases. Bootstrap writes all of
//! them in the genesis commit.
//!
//! # Sealed by the genesis commit, not by a pin
//!
//! Every rule is read from the genesis commit's tree, never from the working tree, so an edit
//! made later is reported and not obeyed. The genesis commit is the pin: a hash written into a
//! file would be one more corpus-editable artefact, and editing it would unseal the rule. A
//! rewritten genesis is possible, but it changes the hash of every commit in the repository,
//! and every clone sees that.
//!
//! # Composition lives here
//!
//! Each rule runs in an engine of its own, holding the rule and its test and nothing else:
//! no default policy, and nothing from `.yidam/policy/`. RFC-0024 made a local policy file
//! authoritative over the rule it names. RFC-0026 (§3, item 2) refused that for a
//! constitutional family, because a family whose composition rule is written in the artefact
//! it composes can be loosened by editing that artefact. Here the composition rule is that no
//! corpus file is consulted, and only this Rust can change it.
//!
//! A rule refuses by adding to `deny`, and nothing else it defines is read. So an article can
//! add refusals to Articles I–VI and cannot remove one: Article V's own text settles that
//! direction (#561), and this shape cannot express the other.
//!
//! # A shallow clone decides nothing
//!
//! Its boundary commit is not the genesis commit. Reading rules from it would obey whatever
//! the clone's tip holds, so nothing is evaluated there, and the directory is reported as
//! unverifiable.

use std::collections::BTreeMap;
use std::path::Path;

use serde_json::Value as Json;

use super::model::{Check, Severity, Violation};
use crate::git::Git;
use crate::policy::Policies;

/// The directory, as a repository-relative path.
const DIR: &str = ".yidam/constitution";

/// What lint could establish about this repository's domain articles.
#[derive(Debug)]
pub(crate) enum Sealed {
    /// No article at genesis and none on disk: the repository was born without one.
    Absent,
    /// Articles are on disk, and the commit that would seal them cannot be read.
    Unverifiable { why: String },
    /// Read from the genesis commit.
    Read {
        /// The genesis commit, 12 characters, so a message reads the same in every clone.
        genesis: String,
        drift: Vec<Drift>,
        rules: Vec<Rule>,
    },
}

/// One file under `.yidam/constitution/` that differs from the genesis commit.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Drift {
    pub path: String,
    pub how: DriftKind,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub(crate) enum DriftKind {
    Changed,
    Deleted,
    Added,
}

/// One article's rule, evaluated as the genesis commit holds it.
#[derive(Debug)]
pub(crate) struct Rule {
    pub path: String,
    /// The messages in `deny`, or why the rule could not answer.
    pub refusals: Result<Vec<String>, String>,
    /// The test file beside it in the genesis commit.
    pub test: Option<String>,
    /// Each failing case: the test rule, and why it failed.
    pub failed: Vec<(String, String)>,
}

/// Read this repository's domain articles, and evaluate each rule against `input`.
///
/// `input` is called only when there is a rule to evaluate, so a repository without articles
/// never builds it.
pub(crate) fn read(root: &Path, input: impl FnOnce() -> Json) -> Sealed {
    let on_disk = files_on_disk(root);
    let Some(genesis) = crate::git::genesis_hash(root) else {
        if on_disk.is_empty() {
            return Sealed::Absent;
        }
        let why = if crate::git::is_shallow(root) {
            "this is a shallow clone, so the genesis commit that seals them is not here"
        } else {
            "the genesis commit that seals them cannot be read: this is not the top of a git \
             repository, or nothing is committed yet"
        };
        return Sealed::Unverifiable {
            why: why.to_string(),
        };
    };

    let sealed = sealed_blobs(root, &genesis);
    if sealed.is_empty() && on_disk.is_empty() {
        return Sealed::Absent;
    }

    let drift = drift(root, &sealed, &on_disk);

    let is_rule = |p: &str| p.ends_with(".rego") && !p.ends_with("_test.rego");
    // Built only when a rule will read it: the sangha report walks git, and a repository whose
    // articles are all prose should not pay for it.
    let json = if sealed.keys().any(|p| is_rule(p)) {
        input()
    } else {
        Json::Null
    };
    let mut rules = Vec::new();
    for (path, blob) in sealed.iter().filter(|(p, _)| is_rule(p)) {
        let test = path.strip_suffix(".rego").map(|s| format!("{s}_test.rego"));
        let test = test.filter(|t| sealed.contains_key(t));
        rules.push(evaluate(root, path, blob, test, &sealed, &json));
    }

    Sealed::Read {
        genesis: genesis.chars().take(12).collect(),
        drift,
        rules,
    }
}

/// Every file under the directory in the working tree, repository-relative and sorted.
fn files_on_disk(root: &Path) -> Vec<String> {
    let dir = crate::paths::yidam_constitution_dir(root);
    let mut out: Vec<String> = walkdir::WalkDir::new(&dir)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
        .filter_map(|e| {
            e.path()
                .strip_prefix(root)
                .ok()
                .map(|p| p.to_string_lossy().replace('\\', "/"))
        })
        .collect();
    out.sort();
    out
}

/// Path → blob id for every file under the directory in the genesis commit.
fn sealed_blobs(root: &Path, genesis: &str) -> BTreeMap<String, String> {
    Git::new(root)
        .args(["ls-tree", "-r", "--full-tree"])
        .rev(genesis)
        .paths([DIR])
        .lines()
        .unwrap_or_default()
        .iter()
        .filter_map(|l| {
            let (meta, path) = l.split_once('\t')?;
            let blob = meta.split_whitespace().nth(2)?;
            Some((path.to_string(), blob.to_string()))
        })
        .collect()
}

/// How the working tree differs from the genesis commit under the directory.
///
/// Compared by blob id, which `hash-object` computes with the same filters `git add` would
/// apply, so a line-ending conversion on checkout is not reported as an edit.
fn drift(root: &Path, sealed: &BTreeMap<String, String>, on_disk: &[String]) -> Vec<Drift> {
    let present: Vec<&String> = sealed.keys().filter(|p| on_disk.contains(p)).collect();
    let now: Vec<String> = if present.is_empty() {
        Vec::new()
    } else {
        Git::new(root)
            .arg("hash-object")
            .paths(&present)
            .lines()
            .unwrap_or_default()
    };
    let now: BTreeMap<&String, &String> = present.iter().copied().zip(now.iter()).collect();

    let mut out = Vec::new();
    for (path, blob) in sealed {
        match now.get(path) {
            None => out.push(Drift {
                path: path.clone(),
                how: DriftKind::Deleted,
            }),
            Some(b) if *b != blob => out.push(Drift {
                path: path.clone(),
                how: DriftKind::Changed,
            }),
            Some(_) => {}
        }
    }
    for path in on_disk {
        if !sealed.contains_key(path) {
            out.push(Drift {
                path: path.clone(),
                how: DriftKind::Added,
            });
        }
    }
    out.sort_by(|a, b| a.path.cmp(&b.path));
    out
}

/// Load one rule and its test from the genesis blobs, run the test, and ask the rule.
fn evaluate(
    root: &Path,
    path: &str,
    blob: &str,
    test: Option<String>,
    sealed: &BTreeMap<String, String>,
    input: &Json,
) -> Rule {
    let mut rule = Rule {
        path: path.to_string(),
        refusals: Ok(Vec::new()),
        test: test.clone(),
        failed: Vec::new(),
    };
    let text = |blob: &str| {
        Git::new(root)
            .args(["cat-file", "blob"])
            .rev(blob)
            .run()
            .map_err(|e| format!("{e:#}"))
    };
    let mut modules = Vec::new();
    match text(blob) {
        Ok(t) => modules.push((path.to_string(), t)),
        Err(e) => {
            rule.refusals = Err(e);
            return rule;
        }
    }
    if let Some(t) = &test {
        match text(&sealed[t]) {
            Ok(body) => modules.push((t.clone(), body)),
            Err(e) => rule.failed.push((t.clone(), e)),
        }
    }
    let Some(package) = Policies::package_of(&modules[0].1) else {
        rule.refusals = Err("it declares no package".to_string());
        return rule;
    };
    let mut engine = match Policies::sealed(&modules) {
        Ok(e) => e,
        Err(e) => {
            rule.refusals = Err(format!("{e:#}"));
            return rule;
        }
    };
    if test.is_some() {
        match engine.run_tests() {
            Ok(outcomes) => rule.failed.extend(
                outcomes
                    .into_iter()
                    .filter(|o| !o.passed)
                    .map(|o| (o.rule, o.detail.unwrap_or_default())),
            ),
            Err(e) => rule
                .failed
                .push((test.clone().unwrap_or_default(), format!("{e:#}"))),
        }
    }
    rule.refusals = engine
        .refusals(&package, input)
        .map_err(|e| format!("{e:#}"));
    rule
}

/// The document a rule is asked about: the sangha report and the corpus's nodes.
pub(crate) fn input(
    sangha: &crate::cmd::sangha::SanghaReport,
    nodes: &[crate::corpus::Node],
) -> Json {
    serde_json::json!({
        "sangha": sangha,
        "nodes": nodes
            .iter()
            .map(|n| serde_json::json!({"file": n.rel, "class": n.inst.class}))
            .collect::<Vec<_>>(),
    })
}

/// `domain-article-violated`, `-edited`, `-unproven` and `-unverifiable`, in that order.
pub(crate) fn checks(sealed: &Sealed) -> [Check; 4] {
    let mut violated = Vec::new();
    let mut edited = Vec::new();
    let mut unproven = Vec::new();
    let mut unverifiable = Vec::new();

    match sealed {
        Sealed::Absent => {}
        Sealed::Unverifiable { why } => unverifiable.push(Violation::new(
            DIR,
            format!("{why}. No domain article here was checked"),
        )),
        Sealed::Read {
            genesis,
            drift,
            rules,
        } => {
            for d in drift {
                let detail = match d.how {
                    DriftKind::Changed => format!(
                        "changed since the genesis commit {genesis}. Lint evaluated the text \
                         genesis holds, not this one. Restore it with `git checkout {genesis} \
                         -- {}`",
                        d.path
                    ),
                    DriftKind::Deleted => format!(
                        "deleted since the genesis commit {genesis}. The article still binds as \
                         genesis wrote it. Restore it with `git checkout {genesis} -- {}`",
                        d.path
                    ),
                    DriftKind::Added => format!(
                        "added after the genesis commit {genesis}. A domain article is written \
                         at genesis, and lint does not evaluate this file. Remove it, or bring \
                         the rule to `.yidam/policy/`"
                    ),
                };
                edited.push(Violation::new(d.path.clone(), detail));
            }
            for r in rules {
                match &r.refusals {
                    Ok(msgs) => violated.extend(msgs.iter().map(|m| {
                        Violation::new(r.path.clone(), format!("{m} (as sealed at {genesis})"))
                    })),
                    Err(e) => violated.push(Violation::new(
                        r.path.clone(),
                        format!(
                            "the rule sealed at {genesis} could not answer: {e}. A rule that \
                             cannot answer has not permitted anything"
                        ),
                    )),
                }
                match &r.test {
                    None => unproven.push(Violation::new(
                        r.path.clone(),
                        "has no `_test.rego` beside it in the genesis commit, so no case shows \
                         the rule refuses what its article says",
                    )),
                    Some(t) => unproven.extend(r.failed.iter().map(|(case, why)| {
                        Violation::new(t.clone(), format!("{case} fails: {why}"))
                    })),
                }
            }
        }
    }

    [
        Check::new(
            "domain-article-violated",
            "The corpus breaks a domain article this repository was born with",
            Severity::Error,
            "A domain article is part of this repository's constitution, committed at genesis \
             and permanent. Its rule is evaluated as the genesis commit holds it, so each \
             finding here is a refusal the article made before any of today's work existed. \
             The repair is to the corpus, never to the rule.",
            violated,
        ),
        Check::new(
            "domain-article-edited",
            "A file under .yidam/constitution/ differs from the genesis commit",
            Severity::Error,
            "A domain article binds permanently, so an edit to it is detected rather than \
             obeyed: lint keeps evaluating the genesis text. Leaving the edit in place makes \
             the file on disk say something the repository does not enforce. Restoring the \
             genesis text is always possible, so this gates.",
            edited,
        ),
        Check::new(
            "domain-article-unproven",
            "A domain article's rule has no passing test beside it",
            Severity::Warn,
            "A rule with no case beside it may refuse nothing its article says, and nothing \
             would show it. This warns rather than gates: the rule and its test are sealed at \
             genesis, so no commit this repository can make would repair them.",
            unproven,
        ),
        Check::new(
            "domain-article-unverifiable",
            "Domain articles are present and the genesis commit cannot be read",
            Severity::Warn,
            "The genesis commit is what seals a domain article. Without it, evaluating the \
             working tree's rule would obey whatever it says now, so nothing is evaluated and \
             this says so. Fetch the full history to check them.",
            unverifiable,
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::fixture::{commit, git, init, write};
    use serde_json::json;

    /// The samudaya example, read from where it ships.
    fn example(suffix: &str) -> String {
        let p = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../samudaya/examples/genealogy")
            .join(format!("augmentation-identity-needs-two-lines{suffix}"));
        std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
    }

    const RULE: &str = ".yidam/constitution/identity-needs-two-lines.rego";
    const TEST: &str = ".yidam/constitution/identity-needs-two-lines_test.rego";
    const PROSE: &str = ".yidam/constitution/identity-needs-two-lines.md";

    /// A repository born with the example article, its rule, and its test.
    fn born_with_article(root: &Path) {
        init(root);
        write(root, PROSE, &example(".md"));
        write(root, RULE, &example(".rego"));
        write(root, TEST, &example("_test.rego"));
        commit(root, "genesis: born with a domain article");
    }

    fn one_tip() -> Json {
        json!({"sangha": {"resolutions": [
            {"file": ".yidam/sangha/resolutions/lone.md", "tips": ["ma/a@1"]},
        ]}})
    }

    fn two_tips() -> Json {
        json!({"sangha": {"resolutions": [
            {"file": ".yidam/sangha/resolutions/pair.md", "tips": ["ma/a@1", "ma/b@2"]},
        ]}})
    }

    fn ids(checks: &[Check; 4]) -> Vec<(&str, usize)> {
        checks.iter().map(|c| (c.id, c.violations.len())).collect()
    }

    #[test]
    fn the_samudaya_example_ships_with_passing_cases() {
        let tmp = tempfile::TempDir::new().unwrap();
        born_with_article(tmp.path());
        let [_, edited, unproven, unverifiable] = checks(&read(tmp.path(), two_tips));
        assert!(edited.violations.is_empty());
        assert!(unproven.violations.is_empty(), "{:?}", unproven.violations);
        assert!(unverifiable.violations.is_empty());
    }

    #[test]
    fn a_corpus_violating_its_genesis_article_gates() {
        let tmp = tempfile::TempDir::new().unwrap();
        born_with_article(tmp.path());

        let [violated, ..] = checks(&read(tmp.path(), one_tip));
        assert_eq!(violated.severity, Severity::Error);
        assert_eq!(violated.violations.len(), 1);
        assert_eq!(violated.violations[0].node, RULE);
        assert!(
            violated.violations[0]
                .detail
                .contains("resolutions/lone.md reads 1 tip(s)"),
            "{}",
            violated.violations[0].detail
        );

        let [violated, ..] = checks(&read(tmp.path(), two_tips));
        assert!(violated.violations.is_empty());
    }

    /// The rule is loosened after genesis — committed, then again uncommitted. The finding
    /// still comes from the genesis text, and the edit is named.
    #[test]
    fn an_edit_after_genesis_is_detected_and_not_obeyed() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path();
        born_with_article(root);
        let loosened = "package constitution.identity_needs_two_lines\n\ndeny := set()\n";

        write(root, RULE, loosened);
        commit(root, "revise: loosen the article");
        let c = checks(&read(root, one_tip));
        assert_eq!(
            ids(&c),
            [
                ("domain-article-violated", 1),
                ("domain-article-edited", 1),
                ("domain-article-unproven", 0),
                ("domain-article-unverifiable", 0),
            ]
        );
        assert_eq!(c[1].severity, Severity::Error);
        assert_eq!(c[1].violations[0].node, RULE);
        assert!(c[1].violations[0]
            .detail
            .starts_with("changed since the genesis commit"));

        // Restored in the index but edited again on disk: still named, still not obeyed.
        git(root, &["revert", "--no-edit", "HEAD"]);
        write(root, RULE, loosened);
        let c = checks(&read(root, one_tip));
        assert_eq!(c[0].violations.len(), 1);
        assert_eq!(c[1].violations.len(), 1);
    }

    #[test]
    fn a_deleted_article_still_binds_and_an_added_one_is_not_evaluated() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path();
        born_with_article(root);
        std::fs::remove_file(root.join(RULE)).unwrap();
        std::fs::remove_file(root.join(TEST)).unwrap();
        let later = ".yidam/constitution/later.rego";
        write(
            root,
            later,
            "package constitution.later\n\ndeny contains \"always\" if true\n",
        );

        let c = checks(&read(root, one_tip));
        // The deleted rule is still evaluated from genesis; the added one is not.
        assert_eq!(c[0].violations.len(), 1);
        assert_eq!(c[0].violations[0].node, RULE);
        let edited: Vec<(&str, bool)> = c[1]
            .violations
            .iter()
            .map(|v| (v.node.as_str(), v.detail.starts_with("deleted")))
            .collect();
        assert_eq!(edited, [(RULE, true), (TEST, true), (later, false)]);
        assert!(c[1].violations[2].detail.starts_with("added after"));
    }

    /// A repository's own policy file claiming the article's package changes nothing: the rule
    /// is loaded in an engine that never reads `.yidam/policy/`.
    #[test]
    fn a_local_policy_cannot_override_an_article() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path();
        born_with_article(root);
        write(
            root,
            ".yidam/policy/override.rego",
            "package constitution.identity_needs_two_lines\n\ndeny := set()\n",
        );
        commit(root, "revise: try to override the article");
        let c = checks(&read(root, one_tip));
        assert_eq!(c[0].violations.len(), 1);
        // The article's own refusal, not a collision between the two files.
        assert!(
            c[0].violations[0].detail.contains("reads 1 tip(s)"),
            "{}",
            c[0].violations[0].detail
        );
        assert!(c[1].violations.is_empty());
    }

    #[test]
    fn a_rule_without_cases_or_with_a_failing_one_is_unproven_and_warns() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path();
        init(root);
        write(
            root,
            ".yidam/constitution/bare.rego",
            "package constitution.bare\n\ndeny := set()\n",
        );
        write(
            root,
            ".yidam/constitution/wrong.rego",
            "package constitution.wrong\n\ndeny := set()\n",
        );
        write(
            root,
            ".yidam/constitution/wrong_test.rego",
            "package constitution.wrong_test\n\nimport data.constitution.wrong\n\n\
             test_refuses_something if count(wrong.deny) > 0\n",
        );
        commit(root, "genesis");

        let [violated, _, unproven, _] = checks(&read(root, one_tip));
        assert!(violated.violations.is_empty());
        assert_eq!(unproven.severity, Severity::Warn);
        let got: Vec<&str> = unproven
            .violations
            .iter()
            .map(|v| v.node.as_str())
            .collect();
        assert_eq!(
            got,
            [
                ".yidam/constitution/bare.rego",
                ".yidam/constitution/wrong_test.rego"
            ]
        );
        assert!(unproven.violations[1]
            .detail
            .contains("test_refuses_something fails"));
    }

    /// No `deny`, and a rule that does not compile, are refusals to answer, not permits.
    #[test]
    fn a_rule_that_cannot_answer_gates() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path();
        init(root);
        let rules = [
            // No `deny` at all.
            ("absent", "allow := true"),
            // A complete `deny` whose body did not hold: undefined, not empty.
            ("undefined", "deny := {\"x\"} if false"),
            // Does not compile.
            ("broken", "deny contains if {"),
            // A partial `deny` matching nothing is an empty set: an answer, and a pass.
            ("quiet", "deny contains \"x\" if false"),
        ];
        for (name, body) in rules {
            write(
                root,
                &format!(".yidam/constitution/{name}.rego"),
                &format!("package constitution.{name}\n\n{body}\n"),
            );
        }
        commit(root, "genesis");

        let [violated, ..] = checks(&read(root, one_tip));
        let got: Vec<(&str, &str)> = violated
            .violations
            .iter()
            .map(|v| {
                let why = if v.detail.contains("is undefined") {
                    "undefined"
                } else if v.detail.contains("could not answer") {
                    "error"
                } else {
                    "refusal"
                };
                (v.node.as_str(), why)
            })
            .collect();
        assert_eq!(
            got,
            [
                (".yidam/constitution/absent.rego", "error"),
                (".yidam/constitution/broken.rego", "error"),
                (".yidam/constitution/undefined.rego", "undefined"),
            ]
        );
    }

    #[test]
    fn a_repository_born_without_articles_is_silent_and_builds_no_input() {
        let tmp = tempfile::TempDir::new().unwrap();
        init(tmp.path());
        write(tmp.path(), "README.md", "x\n");
        commit(tmp.path(), "genesis");
        let s = read(tmp.path(), || panic!("no rule, so no input"));
        assert!(matches!(s, Sealed::Absent));
        assert!(checks(&s).iter().all(|c| c.violations.is_empty()));

        // And outside git altogether.
        let bare = tempfile::TempDir::new().unwrap();
        assert!(matches!(read(bare.path(), || panic!()), Sealed::Absent));
    }

    #[test]
    fn a_shallow_clone_evaluates_nothing_and_says_so() {
        let tmp = tempfile::TempDir::new().unwrap();
        let origin = tmp.path().join("origin");
        std::fs::create_dir(&origin).unwrap();
        born_with_article(&origin);
        write(&origin, "later.md", "x\n");
        commit(&origin, "later");

        let url = format!("file://{}", origin.display());
        git(
            tmp.path(),
            &["clone", "-q", "--depth", "1", &url, "shallow"],
        );
        let shallow = tmp.path().join("shallow");

        let [violated, edited, _, unverifiable] = checks(&read(&shallow, one_tip));
        assert!(violated.violations.is_empty());
        assert!(edited.violations.is_empty());
        assert_eq!(unverifiable.severity, Severity::Warn);
        assert_eq!(unverifiable.violations.len(), 1);
        assert!(unverifiable.violations[0].detail.contains("shallow clone"));
    }
}
