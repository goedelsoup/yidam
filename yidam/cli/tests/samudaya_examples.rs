//! The domain seed sets must be readable seeds, and must not be this repository's seeds.
//!
//! `samudaya/examples/` holds two different things: four stubs demonstrating the format, one
//! per `kind`, and complete seed sets for one domain each. The domain sets exist because
//! `docs/quickstart.md` step 4 tells a reader to write `samudaya/` before bootstrapping, and
//! until #449 the only thing to read was four schema templates that had never seeded
//! anything.
//!
//! # Why they are under `examples/`, and what that costs
//!
//! `samudaya/` is a *transient* layer: `yidam clone` copies the whole of it into a new
//! repository whenever it holds seeds, and the bootstrap consumes it at genesis. So a domain
//! set placed anywhere else under `samudaya/` would not be an example — it would be a live
//! seed of this repository, inherited by every derived repo, landing three other domains'
//! axioms in a repository meant to be born with its own ontology.
//!
//! Both consumers skip exactly one subdirectory and nothing else: `count_seeds` in
//! `cmd/overlay.rs` and the walk in `cmd/samudaya_audit.rs` both exclude `examples/`. Putting
//! the sets there needs no new exclusion and no second skip to keep in step with the first.
//!
//! The cost is that `samudaya-audit` cannot validate them either — it skips the directory
//! they are in. This file is the replacement, and it reads the audit's own
//! [`SAMUDAYA_KINDS`] and parse rather than restating either.

mod common;

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::process::Command;

use common::{repo_root, tracked_under};

/// Every seed file in a domain set, as repo-relative paths.
///
/// Discovered from `git ls-files`, like every other suite here, and *excluding* the four
/// `kind` stubs that sit directly in `examples/` — those are format templates with
/// placeholder bodies, not seed sets.
fn domain_seeds() -> Vec<String> {
    tracked_under(&repo_root(), "samudaya/examples/")
        .into_iter()
        .filter(|p| p.ends_with(".md"))
        .filter(|p| !p.ends_with("README.md"))
        // A stub lives at `samudaya/examples/<kind>.md`; a domain seed one level deeper.
        .filter(|p| p.trim_start_matches("samudaya/examples/").contains('/'))
        .collect()
}

/// The domains that have a seed set.
fn domains() -> Vec<String> {
    domain_seeds()
        .iter()
        .filter_map(|p| {
            p.trim_start_matches("samudaya/examples/")
                .split('/')
                .next()
                .map(str::to_string)
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn read(rel: &str) -> String {
    let p = repo_root().join(rel);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{} unreadable: {e}", p.display()))
}

/// **Fix this one first if the suite goes quiet.** Every check below iterates the discovered
/// set, so a predicate that matches nothing passes everything.
#[test]
fn the_domain_seed_sets_are_discovered() {
    let seeds = domain_seeds();
    assert!(
        !seeds.is_empty(),
        "no domain seed files found under samudaya/examples/ — every check here is vacuous"
    );
    let domains = domains();
    assert!(
        !domains.is_empty(),
        "seed files found but no domain directories parsed out of them: {seeds:?}"
    );
    for d in &domains {
        let n = seeds
            .iter()
            .filter(|p| p.starts_with(&format!("samudaya/examples/{d}/")))
            .count();
        assert!(
            n >= 2,
            "{d} has {n} seed file(s); a set of one is a stub with a directory around it"
        );
    }
}

/// Every seed declares a `kind` the command would accept.
///
/// Through `yidam::samudaya_seed_kind`, which is the parse `samudaya-audit` runs, against
/// `yidam::SAMUDAYA_KINDS`, which is the list it validates against. Reading the frontmatter
/// here instead would be this test's own opinion of what a seed file is, and it could drift
/// from the command's without either changing.
#[test]
fn every_seed_declares_a_kind_the_audit_would_accept() {
    for rel in domain_seeds() {
        let kind = yidam::samudaya_seed_kind(&read(&rel))
            .unwrap_or_else(|| panic!("{rel} declares no `kind:` in its frontmatter"));
        assert!(
            yidam::SAMUDAYA_KINDS.contains(&kind.as_str()),
            "{rel} declares kind `{kind}`, which `samudaya-audit` would reject — expected \
             one of: {}",
            yidam::SAMUDAYA_KINDS.join(", ")
        );
    }
}

/// Every seed has the `# ` title the audit requires, and a body under it.
///
/// `samudaya_audit::has_title` is private, so this restates the rule rather than calling it —
/// which is a smaller duplication than exposing it, and it is checked against the real
/// command by [`the_audit_would_accept_a_copied_seed_set`] below rather than trusted.
#[test]
fn every_seed_has_a_title_and_says_something_under_it() {
    for rel in domain_seeds() {
        let text = read(&rel);
        let body = text
            .split_once("\n---\n")
            .map(|(_, b)| b)
            .unwrap_or_else(|| panic!("{rel} has no frontmatter block"));
        assert!(
            body.lines().any(|l| l.starts_with("# ") && l.len() > 2),
            "{rel} has no `# ` title; `samudaya-audit` reports that as an issue"
        );
        let prose: usize = body
            .lines()
            .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
            .map(|l| l.len())
            .sum();
        assert!(
            prose > 200,
            "{rel} carries {prose} characters of body — the stubs beside it are placeholders, \
             and a domain set exists to show what a seed actually says"
        );
    }
}

/// A domain set covers more than one `kind`.
///
/// A set of four axioms is a class list wearing a seed set's clothes, and the mistake these
/// examples are most likely to cause is a reader supplying the ontology instead of seeding
/// the dialogue that finds one.
#[test]
fn every_domain_set_shows_more_than_one_kind_of_seed() {
    for d in domains() {
        let kinds: BTreeSet<String> = domain_seeds()
            .iter()
            .filter(|p| p.starts_with(&format!("samudaya/examples/{d}/")))
            .filter_map(|p| yidam::samudaya_seed_kind(&read(p)))
            .collect();
        assert!(
            kinds.len() >= 3,
            "{d} uses only {kinds:?}; a seed set that is all axioms teaches that samudaya is \
             where the ontology goes, which is the one thing it is not"
        );
    }
}

// ── the inertness this placement exists for ───────────────────────────────────

/// **The load-bearing test.** This repository must have no seeds of its own.
///
/// Run through the real command rather than by re-deriving `count_seeds`: the bug this
/// guards against is a seed set landing one directory too high, and a test that reimplements
/// the exclusion would move with the mistake instead of catching it.
///
/// If this fails, `yidam overlay` will copy `samudaya/` into an existing repository, and
/// every repository derived from this template will report three other domains' axioms as
/// seeds it is expected to consume at genesis.
///
/// `clone` is not the mechanism, though the issue that filed this said it was. It copies
/// `samudaya/` wholesale — `copy_dir_excluding_top(&root, target, &["docs", "examples"])`
/// excludes only *top-level* `docs/` and `examples/`, so a derived repository gets these
/// sets either way. What changes is whether its own audit then reads them as live, which is
/// what [`a_derived_repository_inherits_no_seeds`] checks.
#[test]
fn this_repository_still_has_no_seeds_of_its_own() {
    let out = Command::new(env!("CARGO_BIN_EXE_yidam"))
        .current_dir(repo_root())
        .arg("samudaya-audit")
        .output()
        .expect("samudaya-audit runs");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "samudaya-audit failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        stdout.contains("no seed files"),
        "samudaya/ in this repository now holds live seeds. A domain set has been placed \
         outside `examples/`, and `yidam clone` will copy it into every derived \
         repository:\n{stdout}"
    );
}

/// The other half: a copied seed set *is* seen, once it is where a seed belongs.
///
/// Without this, [`this_repository_still_has_no_seeds_of_its_own`] would also pass if the
/// sets were empty, unreadable, or in a format the audit ignores — the failure that looks
/// exactly like success.
#[test]
fn the_audit_would_accept_a_copied_seed_set() {
    let domain = domains().first().expect("a domain set exists").clone();
    let tmp = tempfile::tempdir().unwrap();
    let samudaya = tmp.path().join("samudaya");
    std::fs::create_dir_all(&samudaya).unwrap();
    std::fs::create_dir_all(tmp.path().join(".git")).unwrap();

    let mut copied = 0;
    for rel in domain_seeds() {
        if !rel.starts_with(&format!("samudaya/examples/{domain}/")) {
            continue;
        }
        let name = PathBuf::from(&rel);
        std::fs::copy(
            repo_root().join(&rel),
            samudaya.join(name.file_name().unwrap()),
        )
        .unwrap();
        copied += 1;
    }
    assert!(copied >= 2, "copied {copied} seeds for {domain}");

    let out = Command::new(env!("CARGO_BIN_EXE_yidam"))
        .current_dir(tmp.path())
        .arg("samudaya-audit")
        .output()
        .expect("samudaya-audit runs");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains(&format!("Seeds found: {copied}")),
        "the audit did not count the {domain} set as {copied} seeds:\n{stdout}"
    );
    // On stderr, not stdout: `samudaya_audit` reports issues with `eprintln!`. Checking
    // stdout for them is an assertion that cannot fire, which is what it was until a
    // mutation putting a bad `kind` in a seed left this test green.
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        !stderr.contains("[warn]"),
        "the audit reports issues against the {domain} seed set:\n{stderr}"
    );
}

/// The end-to-end claim: a repository derived from this template inherits **no seeds**.
///
/// This is where the harm would land. `clone` carries `samudaya/` across whatever these
/// files say, so the question is not whether they travel but whether the new repository's own
/// audit reads them as seeds it must consume — three foreign domains' axioms folded into an
/// ontology dialogue that is supposed to be about the deriver's own work.
///
/// Through the real commands, both of them, for the reason `clone_does_not_copy_the_example…`
/// gives: the bug this guards against is a one-directory edit, and a test that re-derived the
/// exclusion would move with the mistake instead of catching it.
#[test]
fn a_derived_repository_inherits_no_seeds() {
    let tmp = tempfile::tempdir().unwrap();
    let target = tmp.path().join("derived");
    let out = Command::new(env!("CARGO_BIN_EXE_yidam"))
        .current_dir(repo_root())
        .args(["clone", target.to_str().unwrap()])
        .output()
        .expect("clone runs");
    assert!(
        out.status.success(),
        "clone failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let out = Command::new(env!("CARGO_BIN_EXE_yidam"))
        .current_dir(&target)
        .arg("samudaya-audit")
        .output()
        .expect("samudaya-audit runs");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("no seed files"),
        "a freshly derived repository reports seeds of its own. It is about to be \
         bootstrapped with somebody else's axioms:\n{stdout}"
    );
}
