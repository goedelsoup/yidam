//! A domain article gates `yidam lint` through the real binary (#593, RFC-0047).
//!
//! The unit tests in `lint::articles` hold each finding to its cause. What they cannot show is
//! the part the issue asks for: that a derived repository violating its own genesis-time article
//! **fails**. That is an exit code, and only the binary has one.

use std::path::Path;
use std::process::{Command, Output};

mod common;

fn git(dir: &Path, args: &[&str]) -> String {
    common::git::out_at(dir, args, "@1700000000 +0000")
}

/// The genealogy seed's article, as bootstrap copies it into `.yidam/constitution/`.
fn seed(suffix: &str) -> String {
    let p = common::repo_root()
        .join("samudaya/examples/genealogy")
        .join(format!("augmentation-identity-needs-two-lines{suffix}"));
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

fn write(root: &Path, rel: &str, text: &str) {
    let p = root.join(rel);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, text).unwrap();
}

/// A repository whose genesis commit carries the article, then one resolution reading `tips`.
fn repository(tips: &[&str]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    std::fs::create_dir_all(root.join(".yidam/corpus")).unwrap();
    for suffix in [".md", ".rego", "_test.rego"] {
        write(
            root,
            &format!(".yidam/constitution/identity-needs-two-lines{suffix}"),
            &seed(suffix),
        );
    }
    // Both seats registered, so `resolution-elector-unregistered` has nothing to say.
    write(
        root,
        ".yidam/sangha/electors.md",
        "| Name | Branch |\n|---|---|\n| A | ma/a |\n| B | ma/b |\n",
    );
    git(root, &["init", "-q", "-b", "main"]);
    git(root, &["config", "user.email", "tester@example.org"]);
    git(root, &["config", "user.name", "Tester"]);
    git(root, &["add", "-A"]);
    git(
        root,
        &["commit", "-q", "-m", "genesis: born with a domain article"],
    );

    let list: String = tips.iter().map(|t| format!("  - {t}\n")).collect();
    write(
        root,
        ".yidam/sangha/resolutions/2026-09-29-identity.md",
        &format!(
            "---\nevolution: identity\ndate: 2026-09-29\nsynthesized-by: ma/a\n\
             tips:\n{list}---\n\n## What was resolved\n"
        ),
    );
    git(root, &["add", "-A"]);
    git(root, &["commit", "-q", "-m", "resolve: identity"]);
    dir
}

fn lint(root: &Path) -> (Output, serde_json::Value) {
    let out = Command::new(env!("CARGO_BIN_EXE_yidam"))
        .current_dir(root)
        .args(["lint", "--format", "json"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let report = serde_json::from_str(&stdout).unwrap_or_else(|e| {
        panic!(
            "lint --format json: {e}\nstdout: {stdout}\nstderr: {}",
            String::from_utf8_lossy(&out.stderr)
        )
    });
    (out, report)
}

fn violations(report: &serde_json::Value, id: &str) -> Vec<String> {
    report["checks"]
        .as_array()
        .expect("checks")
        .iter()
        .find(|c| c["id"].as_str() == Some(id))
        .unwrap_or_else(|| panic!("no check `{id}` in the report"))["violations"]
        .as_array()
        .expect("violations")
        .iter()
        .map(|v| v["detail"].as_str().unwrap_or("?").to_string())
        .collect()
}

/// The pair differs by one tip, so the exit code is the article's and no other check's.
#[test]
fn a_repository_violating_its_genesis_article_fails_lint() {
    let held = repository(&["ma/a@1", "ma/b@2"]);
    let (out, report) = lint(held.path());
    assert!(violations(&report, "domain-article-violated").is_empty());
    let held_code = out.status.code();

    let broken = repository(&["ma/a@1"]);
    let (out, report) = lint(broken.path());
    let found = violations(&report, "domain-article-violated");
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].contains("reads 1 tip(s)"), "{found:?}");
    assert!(!out.status.success(), "a violated article must fail lint");
    assert_eq!(
        held_code,
        Some(0),
        "the held repository must pass, or the failure above is not the article's"
    );
}
