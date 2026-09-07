//! `yidam catalog-fetch` and `catalog-reconcile` through the binary (#720).
//!
//! Integration tests rather than unit tests, because what matters is what a repository looks
//! like afterwards: which bytes are in the cache, what the entry says, what commit exists and
//! who authored it. `cmd::catalog`'s own tests hold the planner, the splice and the invariant;
//! this holds the loop.
//!
//! ```text
//!   an address nothing has followed
//!     → fetch → bytes under their digest, a record in the entry, a `refresh:` commit
//!     → fetch again → nothing, because nothing changed
//!   a `used-by` that drifted
//!     → reconcile → a `reconcile:` commit, and the gate's own drift function reports none
//! ```
//!
//! # The cache is redirected, always
//!
//! Every test sets `YIDAM_VAULT_CACHE` into its own temp directory. Without it these would
//! write into the developer's real `~/.cache/yidam/vault` and, worse, would pass or fail
//! depending on what was already in it — `Obtained::cached` is one of the things asserted.

use std::path::{Path, PathBuf};
use std::process::Command;

fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/catalog-fetch")
}

fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .current_dir(dir)
        .args(args)
        .env("GIT_AUTHOR_DATE", "@1700000000 +0000")
        .env("GIT_COMMITTER_DATE", "@1700000000 +0000")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

/// A staging area holding the fixture as its own git repository, plus a private cache.
struct Staged {
    dir: tempfile::TempDir,
    root: PathBuf,
}

impl Staged {
    fn root(&self) -> &Path {
        &self.root
    }

    fn cache(&self) -> PathBuf {
        self.dir.path().join("cache")
    }

    fn entry(&self) -> PathBuf {
        self.root().join(".yidam/catalog/local-registry.md")
    }

    fn entry_text(&self) -> String {
        std::fs::read_to_string(self.entry()).unwrap()
    }

    fn run(&self, args: &[&str]) -> Run {
        let out = Command::new(env!("CARGO_BIN_EXE_yidam"))
            .current_dir(self.root())
            .args(args)
            .env("YIDAM_VAULT_CACHE", self.cache())
            .env("GIT_AUTHOR_DATE", "@1700000000 +0000")
            .env("GIT_COMMITTER_DATE", "@1700000000 +0000")
            .output()
            .unwrap();
        Run {
            stdout: String::from_utf8_lossy(&out.stdout).to_string(),
            stderr: String::from_utf8_lossy(&out.stderr).to_string(),
            ok: out.status.success(),
        }
    }

    fn head_subject(&self) -> String {
        git(self.root(), &["log", "-1", "--format=%s"])
    }

    fn head_author(&self) -> String {
        git(self.root(), &["log", "-1", "--format=%an <%ae>"])
    }

    fn commit_count(&self) -> usize {
        git(self.root(), &["rev-list", "--count", "HEAD"])
            .parse()
            .unwrap()
    }
}

struct Run {
    stdout: String,
    stderr: String,
    ok: bool,
}

impl Run {
    fn ok(self) -> String {
        assert!(self.ok, "command failed:\n{}\n{}", self.stdout, self.stderr);
        self.stdout
    }
}

/// Copy the shipped fixture into a temp directory and make it a repository.
///
/// The same recipe `mcp_serve.rs` uses — copy, `git init`, one commit — and for the reason it
/// gives: the corpus ships as files somebody can read, and staging it is ten lines.
fn stage() -> Staged {
    let dir = tempfile::TempDir::new().unwrap();
    let root = dir.path().join("repo");
    std::fs::create_dir_all(&root).unwrap();
    copy_dir(&fixture_dir(), &root);

    git(&root, &["init", "-q", "-b", "main"]);
    git(&root, &["config", "user.email", "runner@test"]);
    git(&root, &["config", "user.name", "Runner"]);
    git(&root, &["add", "."]);
    git(
        &root,
        &["commit", "-q", "-m", "scaffold: catalog-fetch fixture"],
    );
    Staged { dir, root }
}

fn copy_dir(from: &Path, to: &Path) {
    for entry in walkdir::WalkDir::new(from)
        .into_iter()
        .filter_map(Result::ok)
    {
        let rel = entry.path().strip_prefix(from).unwrap();
        if rel.as_os_str().is_empty() {
            continue;
        }
        let dest = to.join(rel);
        if entry.file_type().is_dir() {
            std::fs::create_dir_all(&dest).unwrap();
        } else {
            std::fs::create_dir_all(dest.parent().unwrap()).unwrap();
            std::fs::copy(entry.path(), &dest).unwrap();
        }
    }
}

/// The digest of the fixture's own bytes, computed rather than pasted.
///
/// Through the CLI's own [`yidam::vault::ContentHash`], which is what files the artifact, so
/// this asserts that the fetch agrees with the addressing scheme rather than that both agree
/// with a hex string somebody transcribed. A literal would also go stale silently the first
/// time a row is added to the CSV.
fn registry_digest() -> String {
    yidam::vault::ContentHash::of_file(&fixture_dir().join("sources/registry-2026.csv"))
        .unwrap()
        .as_str()
        .to_string()
}

// ── the fetch loop ────────────────────────────────────────────────────────────

/// The whole of #720's missing middle, in one run: address → bytes → cache → record → commit.
#[test]
fn a_file_location_is_followed_cached_recorded_and_committed() {
    let s = stage();
    let before = s.commit_count();
    let out = s
        .run(&["catalog-fetch", "local-registry", "--location", "0"])
        .ok();

    let digest = registry_digest();
    assert!(out.contains(&digest), "the digest is reported:\n{out}");
    assert!(out.contains("local-registry"), "{out}");

    // The bytes are in the cache, under their content address.
    let cached = s.cache().join("sha256").join(&digest[..2]).join(&digest);
    assert!(cached.is_file(), "{} was not cached", cached.display());
    assert_eq!(
        std::fs::read(&cached).unwrap(),
        std::fs::read(fixture_dir().join("sources/registry-2026.csv")).unwrap()
    );

    // The record is in the entry, and the entry still parses.
    let text = s.entry_text();
    assert!(text.contains(&format!("sha256: {digest}")), "{text}");
    assert!(text.contains("from: 0"), "{text}");
    assert!(text.contains("bytes: "), "{text}");

    // The prose survived.
    assert!(text.contains("# The county hydrant registry"), "{text}");
    assert!(text.contains("It does not say whether it was"), "{text}");
    assert!(text.contains("kind: url_template"), "{text}");

    // And a `refresh:` commit records it, authored by the tool and committed by the person.
    assert_eq!(s.commit_count(), before + 1);
    let subject = s.head_subject();
    assert!(subject.starts_with("refresh: local-registry"), "{subject}");
    assert_eq!(s.head_author(), "yidam catalog <catalog@yidam>");
    assert_eq!(
        git(s.root(), &["log", "-1", "--format=%ce"]),
        "runner@test",
        "the committer is whoever ran it"
    );
    // The body carries what the subject cannot.
    let body = git(s.root(), &["log", "-1", "--format=%b"]);
    assert!(body.contains(&digest), "{body}");
    assert!(body.contains("from location 0"), "{body}");
}

/// The commit a run writes must be one the vocabulary calls operational — RFC-0026's
/// invariant, checked against the parity function on the commit that actually landed.
#[test]
fn the_commit_a_run_authors_is_operational() {
    let s = stage();
    s.run(&["catalog-fetch", "local-registry", "--location", "0"])
        .ok();
    let subject = s.head_subject();
    assert_eq!(
        yidam_core::git::classify_commit("", &subject).kind,
        yidam_core::git::CommitKind::Operational,
        "{subject}"
    );

    // And nothing it wrote is on a proposal branch, because it had no epistemic commit to
    // put there. Stated as an assertion so that a future change producing one goes red here.
    let branches = git(
        s.root(),
        &["for-each-ref", "--format=%(refname:short)", "refs/heads"],
    );
    assert_eq!(branches, "main", "a fetch creates no branch");
}

/// What makes the command safe to put on a clock. The second run finds the same bytes, so
/// there is nothing to record and no commit to write.
#[test]
fn fetching_again_with_nothing_changed_writes_no_second_commit() {
    let s = stage();
    s.run(&["catalog-fetch", "local-registry", "--location", "0"])
        .ok();
    let after_first = s.commit_count();
    let text_after_first = s.entry_text();

    let out = s
        .run(&["catalog-fetch", "local-registry", "--location", "0"])
        .ok();
    assert_eq!(s.commit_count(), after_first, "no second commit");
    assert_eq!(s.entry_text(), text_after_first, "the entry is untouched");
    assert!(
        out.contains("already held") || out.contains("nothing new to record"),
        "the run says why it did nothing:\n{out}"
    );
}

/// A source that changed produces a second record beside the first rather than replacing it —
/// overwriting would delete the provenance of any claim resting on the older bytes.
#[test]
fn a_changed_source_appends_a_second_record() {
    let s = stage();
    s.run(&["catalog-fetch", "local-registry", "--location", "0"])
        .ok();
    let first = registry_digest();

    std::fs::write(
        s.root().join("sources/registry-2026.csv"),
        "asset_id,placed_year,static_psi\nVC-0001,1974,62\nVC-0005,2026,70\n",
    )
    .unwrap();
    git(s.root(), &["add", "-A"]);
    git(s.root(), &["commit", "-q", "-m", "extract: a new edition"]);

    s.run(&["catalog-fetch", "local-registry", "--location", "0"])
        .ok();
    let text = s.entry_text();
    assert!(text.contains(&first), "the first digest survives:\n{text}");
    assert_eq!(
        text.matches("sha256: ").count(),
        2,
        "two records, not one replaced:\n{text}"
    );
}

// ── what it refuses ───────────────────────────────────────────────────────────

/// The case the whole issue is named after: `url_template` is validated by the linter and
/// followed by nothing. It is followable now — but only once its slots are bound, and the
/// refusal has to say which.
#[test]
fn an_unbound_template_is_refused_by_name() {
    let s = stage();
    let out = s
        .run(&["catalog-fetch", "local-registry", "--location", "1"])
        .ok();
    assert!(
        out.contains("--bind year="),
        "the refusal repairs itself:\n{out}"
    );
    assert_eq!(s.commit_count(), 1, "nothing was written");
}

/// A physical address is a real way to hold a source, not a failure. Asked for by index it is
/// explained; sitting beside a followable location it is passed over in silence.
#[test]
fn an_address_is_explained_when_named_and_silent_when_not() {
    let s = stage();
    let named = s
        .run(&["catalog-fetch", "local-registry", "--location", "2"])
        .ok();
    assert!(named.contains("place rather than an endpoint"), "{named}");

    let all = s
        .run(&["catalog-fetch", "local-registry", "--dry-run"])
        .ok();
    assert!(
        !all.contains("Court Street"),
        "an address beside a followable location is not reported:\n{all}"
    );
}

/// `--dry-run` resolves every address and writes nothing — no bytes, no record, no commit.
#[test]
fn a_dry_run_writes_nothing() {
    let s = stage();
    let before = s.entry_text();
    let out = s
        .run(&["catalog-fetch", "local-registry", "--dry-run"])
        .ok();
    assert!(out.contains("would fetch"), "{out}");
    assert_eq!(s.entry_text(), before);
    assert_eq!(s.commit_count(), 1);
    assert!(!s.cache().join("sha256").exists(), "nothing was cached");
}

/// An entry edited and not committed is refused *before* anything moves, so the message names
/// a person's work rather than the edit this command was about to make.
#[test]
fn an_uncommitted_entry_is_refused() {
    let s = stage();
    let text = s.entry_text();
    std::fs::write(s.entry(), text.replace("obtained: true", "obtained: false")).unwrap();

    let run = s.run(&["catalog-fetch", "local-registry", "--location", "0"]);
    assert!(!run.ok, "should refuse: {}", run.stdout);
    assert!(run.stderr.contains("uncommitted changes"), "{}", run.stderr);
    assert!(!s.cache().join("sha256").exists(), "nothing was fetched");
}

#[test]
fn an_unknown_entry_says_where_to_look() {
    let s = stage();
    let run = s.run(&["catalog-fetch", "no-such-source"]);
    assert!(!run.ok);
    assert!(run.stderr.contains("catalog-audit"), "{}", run.stderr);
}

// ── reconcile ─────────────────────────────────────────────────────────────────

/// The `reconcile:` row of #460's table: catalog and corpus brought back into agreement.
///
/// The fixture drifts in both directions — it claims a node that no longer exists and omits
/// one that cites it — because `UsedByDrift` reports the two separately and a fixture with
/// only one would leave half the repair unexercised.
#[test]
fn a_drifted_used_by_is_reconciled_and_committed() {
    let s = stage();
    let before = s.commit_count();

    let out = s.run(&["catalog-reconcile", "local-registry"]).ok();
    assert!(out.contains("+ hydrant-count.yml"), "{out}");
    assert!(out.contains("- withdrawn-survey.yml"), "{out}");

    let text = s.entry_text();
    assert!(
        text.contains("../corpus/record/hydrant-count.yml"),
        "written in the form entries are written in:\n{text}"
    );
    assert!(!text.contains("withdrawn-survey"), "{text}");
    assert!(
        text.contains("# The county hydrant registry"),
        "prose survives"
    );

    assert_eq!(s.commit_count(), before + 1);
    let subject = s.head_subject();
    assert!(
        subject.starts_with("reconcile: local-registry"),
        "{subject}"
    );
    assert_eq!(
        yidam_core::git::classify_commit("", &subject).kind,
        yidam_core::git::CommitKind::Operational
    );

    // The body says which way each name moved — the diff alone cannot.
    let body = git(s.root(), &["log", "-1", "--format=%b"]);
    assert!(body.contains("added 1"), "{body}");
    assert!(body.contains("removed 1"), "{body}");
}

/// A second reconcile has nothing to do, which is what a repair that actually converged looks
/// like. If the two lists were compared in different forms this would loop forever.
#[test]
fn reconciling_twice_is_a_no_op_the_second_time() {
    let s = stage();
    s.run(&["catalog-reconcile", "local-registry"]).ok();
    let after = s.commit_count();
    let text = s.entry_text();

    let out = s.run(&["catalog-reconcile", "local-registry"]).ok();
    assert_eq!(s.commit_count(), after, "converged");
    assert_eq!(s.entry_text(), text);
    assert!(out.contains("agrees with the citations"), "{out}");
}

/// The repair has to leave the gate green. `lint` is the arbiter, not this test's own reading
/// of the drift — a repair that agreed with itself and not with the gate would pass a test and
/// fail a build.
#[test]
fn reconciling_clears_the_gate_finding_it_was_written_for() {
    let s = stage();
    let before = s.run(&["lint", "--format", "json"]).ok();
    assert!(
        before.contains("catalog-used-by-drift"),
        "the fixture must actually drift, or this asserts nothing"
    );
    let drifting: serde_json::Value = serde_json::from_str(&before).unwrap();
    // `checks` sits at the top of the RFC-0016 envelope, beside `gate` and `root` — not under
    // a `report` key. Reading the wrong path here would make this test pass by finding zero
    // violations both times, which is the one way it could assert nothing while looking green.
    let violations = |v: &serde_json::Value| -> usize {
        v["checks"]
            .as_array()
            .map(|cs| {
                cs.iter()
                    .filter(|c| c["id"] == "catalog-used-by-drift")
                    .map(|c| c["violations"].as_array().map_or(0, Vec::len))
                    .sum()
            })
            .unwrap_or(0)
    };
    assert_eq!(violations(&drifting), 1, "one drifted entry to start");

    s.run(&["catalog-reconcile", "local-registry"]).ok();

    let after: serde_json::Value =
        serde_json::from_str(&s.run(&["lint", "--format", "json"]).ok()).unwrap();
    assert_eq!(
        violations(&after),
        0,
        "the gate the repair was written for is clear"
    );
}

/// `--dry-run` reports the substitution and writes nothing.
#[test]
fn a_reconcile_dry_run_writes_nothing() {
    let s = stage();
    let before = s.entry_text();
    let out = s.run(&["catalog-reconcile", "--dry-run"]).ok();
    assert!(out.contains("+ hydrant-count.yml"), "{out}");
    assert_eq!(s.entry_text(), before);
    assert_eq!(s.commit_count(), 1);
}
