//! Documents that say a git hook runs, against the hooks this repository ships (#693).
//!
//! Three documents and an issue described a `commit-msg` hook running `yidam vocabulary
//! --check`. There is no such hook: no script, no installer, no `.githooks/`, and no derived
//! repository installs one. The claim was load-bearing rather than decorative — #652's
//! cheapest proposed remedy was *"the hook reads the staged paths"*, a fix to a file that does
//! not exist — and one of the three sites was a published release note.
//!
//! The set's stance is `conformance, not hooks` (RFC-0004, quoted in RFC-0014's open
//! questions), so the absence is a decision rather than an omission. That is what makes this
//! checkable: a document asserting that something *runs in* a hook is a claim about this
//! repository, and the repository can read its own file list.
//!
//! # It reads the assertive form only
//!
//! `runs in the commit-msg hook` is a claim. `this project ships no commit-msg hook` and
//! RFC-0014's *"with a local pre-commit hook optional"* are not — they are the absence being
//! stated and a proposal being weighed, and a guard that could not tell those apart would
//! forbid documenting the decision. So the pattern is a hook name in the subject or object of
//! a running verb, and nothing wider. A document that invents some new way to assert a hook
//! runs will slip through; the failure this exists for is the sentence that was actually
//! written, three times.

use std::path::{Path, PathBuf};
use std::process::Command;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn tracked(root: &Path) -> Vec<String> {
    let out = Command::new("git")
        .current_dir(root)
        .args(["ls-files"])
        .output()
        .expect("git ls-files runs");
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(str::to_string)
        .collect()
}

/// The git hooks this repository ships: a tracked path under a hooks directory, or one named
/// for a hook git would run.
///
/// Vendored trees are excluded — a dependency's own hooks are not this repository's claim.
fn shipped_hooks(files: &[String]) -> Vec<&String> {
    files
        .iter()
        .filter(|f| !f.contains("node_modules/") && !f.contains("/.vendor/"))
        .filter(|f| {
            let lower = f.to_ascii_lowercase();
            lower.contains(".githooks/")
                || lower.contains("git-hooks/")
                || lower.contains("/hooks/")
                || HOOKS.iter().any(|h| {
                    Path::new(&lower)
                        .file_name()
                        .and_then(|n| n.to_str())
                        .is_some_and(|n| n.starts_with(h))
                })
        })
        .collect()
}

/// The client-side hooks a document might name. Not git's whole set — the ones a project like
/// this one would plausibly claim to install.
const HOOKS: &[&str] = &[
    "commit-msg",
    "pre-commit",
    "prepare-commit-msg",
    "pre-push",
    "post-commit",
    "post-checkout",
    "post-merge",
];

/// The text with every struck-through span blanked, newlines kept so line numbers survive.
///
/// `~~…~~` is this repository's convention for a claim that was made and is now withdrawn —
/// RFC-0028 carries three of them, each with a `Corrected (date, #NNN)` section under it. A
/// guard that read a struck sentence as a live claim would forbid the RFCs from quoting what
/// they superseded, which is the one place a wrong claim has to stay visible.
fn without_struck(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(open) = rest.find("~~") {
        out.push_str(&rest[..open]);
        let after = &rest[open + 2..];
        let Some(close) = after.find("~~") else {
            out.push_str(&rest[open..]);
            return out;
        };
        out.extend(after[..close].chars().map(|c| match c {
            '\n' => '\n',
            _ => ' ',
        }));
        rest = &after[close + 2..];
    }
    out.push_str(rest);
    out
}

/// Whether `line` asserts that a hook runs, as opposed to naming one.
///
/// Two shapes, both taken from the sentences #693 found: the hook as the place something runs
/// (`runs in the commit-msg hook`) and the hook as the thing running (`the commit-msg hook
/// runs`, `the commit-msg hook is not scoped`, `the hook still warns`).
fn asserts_a_hook_runs(line: &str) -> Option<&'static str> {
    /// What a hook does in a sentence that claims it is running.
    const ACTS: &[&str] = &[
        "runs",
        "warns",
        "reports",
        "fires",
        "reads",
        "is not scoped",
        "still",
    ];
    let lower = line.to_ascii_lowercase();
    HOOKS.iter().copied().find(|hook| {
        let needle = format!("{hook} hook");
        let Some(at) = lower.find(&needle) else {
            return false;
        };
        let before = &lower[..at];
        let after = lower[at + needle.len()..].trim_start_matches([' ', ',']);
        // `runs in the <hook> hook`, `in a <hook> hook`.
        let placed = before.ends_with("in the ") || before.ends_with("in a ");
        // `the <hook> hook runs`, `… warns`, `… is not scoped by it`.
        let acting = ACTS.iter().any(|v| after.starts_with(v));
        placed || acting
    })
}

/// No document may say a hook runs while this repository ships none.
#[test]
fn no_document_says_a_hook_runs_that_this_repository_does_not_ship() {
    let root = repo_root();
    let files = tracked(&root);
    let hooks = shipped_hooks(&files);
    if !hooks.is_empty() {
        // The premise changed: something ships a hook now, and the documents may describe it.
        // Failing here would be this test outliving its reason.
        return;
    }
    let mut claims = Vec::new();
    for rel in files.iter().filter(|f| f.ends_with(".md")) {
        let Ok(text) = std::fs::read_to_string(root.join(rel)) else {
            continue;
        };
        for (i, line) in without_struck(&text).lines().enumerate() {
            if let Some(hook) = asserts_a_hook_runs(line) {
                claims.push(format!(
                    "  {rel}:{} — names the {hook} hook as running",
                    i + 1
                ));
            }
        }
    }
    assert!(
        claims.is_empty(),
        "this repository ships no git hook, and these documents say one runs:\n{}",
        claims.join("\n")
    );
}

/// The guard has to be able to fire, or it is a test of nothing. The sentence here is
/// `docs/upgrading.md`'s, as it read before #693.
#[test]
fn the_sentence_that_shipped_is_one_this_guard_catches() {
    let shipped = "**One asymmetry, deliberate.** `yidam vocabulary --check` runs in the \
                   commit-msg hook, before the commit exists.";
    assert_eq!(asserts_a_hook_runs(shipped), Some("commit-msg"));
    assert_eq!(
        asserts_a_hook_runs("**The commit-msg hook is not scoped by it either.**"),
        Some("commit-msg")
    );
}

/// Stating the absence, and weighing whether to ship one, are not claims that a hook runs —
/// and a guard that could not tell them apart would forbid recording the decision.
#[test]
fn naming_a_hook_to_say_it_is_absent_is_not_a_claim_that_it_runs() {
    for line in [
        "**This project ships no commit-msg hook.** The stance is conformance, not hooks.",
        "the lean is: a report rule that CI gates, with a local pre-commit hook optional.",
        "Decide separately whether a commit-msg hook should ship.",
    ] {
        assert_eq!(asserts_a_hook_runs(line), None, "{line}");
    }
}

/// A withdrawn claim stays readable. RFC-0028 keeps the sentence #693 corrected, struck, with
/// the correction under it — and the guard has to read that as history rather than as a claim.
#[test]
fn a_struck_claim_is_history_and_not_a_claim() {
    let rfc = "> **One asymmetry ships open.** ~~`yidam vocabulary --check` runs in the \
               commit-msg hook, before\n> the commit exists.~~ **Corrected 2026-09-07 under \
               #693 — see below.**\n";
    assert!(
        without_struck(rfc)
            .lines()
            .all(|l| asserts_a_hook_runs(l).is_none()),
        "{}",
        without_struck(rfc)
    );
    // Line numbering survives, or a finding would name the wrong line.
    assert_eq!(without_struck(rfc).lines().count(), rfc.lines().count());
}

/// The other half of the premise: a repository that ships a hook may describe it. Without
/// this, the guard's `return` above is an untested branch.
#[test]
fn a_shipped_hook_is_found_where_git_would_look_for_one() {
    let files: Vec<String> = vec![
        "docs/configuration.md".to_string(),
        ".githooks/commit-msg".to_string(),
    ];
    assert_eq!(shipped_hooks(&files).len(), 1);
    assert!(shipped_hooks(&["docs/configuration.md".to_string()]).is_empty());
}
