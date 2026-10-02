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

mod common;

fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/catalog-fetch")
}

/// Every commit this file makes carries the same timestamp, so a report's dates are the
/// fixture's and not the run's.
fn git(dir: &Path, args: &[&str]) -> String {
    common::git::out_at(dir, args, "@1700000000 +0000")
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
        self.run_env(args, &[])
    }

    /// `YIDAM_CONTACT` is always cleared first, so a developer's own does not reach a test
    /// that asserts its absence.
    fn run_env(&self, args: &[&str], env: &[(&str, &str)]) -> Run {
        let out = Command::new(env!("CARGO_BIN_EXE_yidam"))
            .current_dir(self.root())
            .args(args)
            .env_remove("YIDAM_CONTACT")
            .envs(env.iter().copied())
            .env("NO_PROXY", "127.0.0.1")
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

    fn head_body(&self) -> String {
        git(self.root(), &["log", "-1", "--format=%b"])
    }

    /// Write a decision onto the entry's latest record and commit it, as an operator would.
    ///
    /// Spliced textually rather than by rewriting the file, because what the next fetch reads
    /// has to be a record a person actually typed — the same four lines, in the same place.
    fn decide(&self, field: &str) {
        let text = self.entry_text();
        let at = text.rfind("    from: ").expect("a record to decide about");
        let eol = text[at..].find('\n').unwrap() + at + 1;
        let mut out = text.clone();
        out.insert_str(eol, &format!("    {field}\n"));
        std::fs::write(self.entry(), &out).unwrap();
        git(self.root(), &["add", "-A"]);
        git(
            self.root(),
            &["commit", "-q", "-m", "note: the publisher's terms"],
        );
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

/// #1200, end to end. A re-fetch that records a new version names the node that read the
/// old one — in the report and in the commit it makes — and `due` holds that node until a
/// commit touches it. The first fetch has nothing to supersede and names nobody.
#[test]
fn a_new_version_names_the_nodes_that_read_the_old_one() {
    const NODE: &str = ".yidam/corpus/record/hydrant-count.yml";
    let s = stage();
    let first = s
        .run(&["catalog-fetch", "local-registry", "--location", "0"])
        .ok();
    assert!(
        !first.contains(NODE),
        "a first fetch supersedes nothing:\n{first}"
    );
    assert!(!s.head_body().contains(NODE), "{}", s.head_body());

    std::fs::write(
        s.root().join("sources/registry-2026.csv"),
        "asset_id,placed_year,static_psi\nVC-0001,1974,62\nVC-0005,2026,70\n",
    )
    .unwrap();
    git(s.root(), &["add", "-A"]);
    git(s.root(), &["commit", "-q", "-m", "extract: a new edition"]);

    let second = s
        .run(&["catalog-fetch", "local-registry", "--location", "0"])
        .ok();
    assert!(second.contains("a new version"), "{second}");
    assert!(
        second.contains(NODE),
        "the report names the citing node:\n{second}"
    );
    let body = s.head_body();
    assert!(body.contains(NODE), "the commit names it too:\n{body}");

    // And `due` carries it from here, once the corpus has said how long a re-read may wait.
    let config = s.root().join(".yidam/config.toml");
    let mut toml = std::fs::read_to_string(&config).unwrap();
    toml.push_str("\n[due]\nsuperseded_after = 0\n");
    std::fs::write(&config, toml).unwrap();
    git(s.root(), &["add", "-A"]);
    git(
        s.root(),
        &["commit", "-q", "-m", "config: a re-read may not wait"],
    );
    let due = s.run(&["due"]).ok();
    assert!(due.contains(NODE), "due holds the node:\n{due}");

    // A commit to the node discharges it.
    let node = s.root().join(NODE);
    let mut text = std::fs::read_to_string(&node).unwrap();
    text.push_str("properties_note: re-read against the new edition\n");
    std::fs::write(&node, text).unwrap();
    git(s.root(), &["add", "-A"]);
    git(
        s.root(),
        &["commit", "-q", "-m", "revise: hydrant count, re-read"],
    );
    let due = s.run(&["due"]).ok();
    assert!(!due.contains(NODE), "the commit discharged it:\n{due}");
}

/// #1074, end to end. An operator cleared this source for redistribution; the next edition
/// arrives carrying that decision rather than a blank where `vault push` reads a refusal.
///
/// Through the real command against a real repository, because the unit tests exercise
/// `artifact_for` and the defect was in what the *fetch loop* handed it: nothing read the
/// entry's existing records at all.
#[test]
fn a_re_fetch_carries_the_licence_the_entry_already_recorded() {
    let s = stage();
    s.run(&["catalog-fetch", "local-registry", "--location", "0"])
        .ok();
    let first = registry_digest();
    s.decide("redistributable: true");

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
    assert_eq!(
        text.matches("redistributable: true").count(),
        2,
        "the new record carries the licence the old one stated:\n{text}"
    );
    assert!(
        s.head_body().contains(&format!(
            "carried forward from sha256:{first}: redistributable: true"
        )),
        "the commit says the licence was continued and not established:\n{}",
        s.head_body()
    );
}

/// The other half, and the one that loses bytes rather than time. `vault: none` is bytes an
/// operator chose to keep in the local cache — RFC-0023 calls it *"the local cache and nowhere
/// else, by decision — for which the cache is the only copy there will ever be."* A hold-back
/// that lapsed at the next edition would route the new bytes to a store the operator held the
/// previous ones back from.
#[test]
fn a_re_fetch_carries_a_local_only_hold_back() {
    let s = stage();
    s.run(&["catalog-fetch", "local-registry", "--location", "0"])
        .ok();
    s.decide("vault: none");

    std::fs::write(
        s.root().join("sources/registry-2026.csv"),
        "asset_id,placed_year,static_psi\nVC-0009,2019,58\n",
    )
    .unwrap();
    git(s.root(), &["add", "-A"]);
    git(s.root(), &["commit", "-q", "-m", "extract: a new edition"]);

    s.run(&["catalog-fetch", "local-registry", "--location", "0"])
        .ok();
    let text = s.entry_text();
    assert_eq!(
        text.matches("vault: none").count(),
        2,
        "the hold-back survives the edition:\n{text}"
    );
}

/// A first capture is still silent, and the record shows it. This is the assertion the carry
/// must not cost: a fetch of a source nobody has ruled on says nothing about its licence.
///
/// Read off the `artifacts:` block rather than the whole file. The entry is a document whose
/// prose is the substance, and a page that came to discuss its own licensing would make a
/// whole-file `contains` fail over a sentence.
#[test]
fn a_first_capture_records_neither_field() {
    let s = stage();
    s.run(&["catalog-fetch", "local-registry", "--location", "0"])
        .ok();
    let text = s.entry_text();
    let at = text.find("artifacts:").expect("a record was written");
    let records = &text[at..text[at..].find("\n---").unwrap() + at];
    assert!(!records.contains("redistributable"), "{records}");
    assert!(!records.contains("vault:"), "{records}");
    assert!(
        !s.head_body().contains("carried forward"),
        "{}",
        s.head_body()
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

// ── identifiers, through a source pack (RFC-0048 §5) ──────────────────────────

/// Answer each of `answers` on a loopback port in turn, and hand back what each request said.
///
/// The listener closes once it has answered them all, so a client that retried would get a
/// refused connection and fail, which the test sees.
fn serve(answers: Vec<String>) -> (u16, std::thread::JoinHandle<Vec<String>>) {
    use std::io::{Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let handle = std::thread::spawn(move || {
        let mut seen = Vec::new();
        for answer in answers {
            let (mut sock, _) = listener.accept().unwrap();
            let mut got = Vec::new();
            let mut buf = [0u8; 4096];
            while !String::from_utf8_lossy(&got).contains("\r\n\r\n") {
                let n = sock.read(&mut buf).unwrap();
                if n == 0 {
                    break;
                }
                got.extend_from_slice(&buf[..n]);
            }
            sock.write_all(answer.as_bytes()).unwrap();
            seen.push(String::from_utf8_lossy(&got).to_string());
        }
        seen
    });
    (port, handle)
}

const PAPER: &str = "\
---
name: loopback-paper
description: A record a source pack resolves, served from loopback.
type: dataset
location:
  - kind: identifier
    value: lb:rec-1
    description: The record, by its identifier.
---

# A record resolved through a pack
";

/// Stage the fixture plus an authored pack declaring `lb:` against `port`, and an entry that
/// cites `lb:rec-1`.
fn stage_pack(port: u16, transport: &str) -> Staged {
    let s = stage();
    let pack = s.root().join(".yidam/sources/loopback");
    std::fs::create_dir_all(&pack).unwrap();
    std::fs::write(
        pack.join("pack.toml"),
        format!(
            "[pack]\nname = \"loopback\"\nversion = \"0.1.0\"\n\n\
             [scheme.lb]\npattern = '^rec-\\d+$'\ntype = \"dataset\"\n\
             resolve = {{ template = \"http://127.0.0.1:{port}/records/{{id}}.csv\" }}\n\n\
             [transport]\n{transport}\n"
        ),
    )
    .unwrap();
    std::fs::write(s.root().join(".yidam/catalog/loopback-paper.md"), PAPER).unwrap();
    git(s.root(), &["add", "-A"]);
    git(
        s.root(),
        &["commit", "-q", "-m", "scaffold: a loopback pack"],
    );
    s
}

fn ok(body: &str) -> String {
    format!(
        "HTTP/1.1 200 OK\r\ncontent-type: text/csv\r\ncontent-length: {}\r\n\
         connection: close\r\n\r\n{body}",
        body.len()
    )
}

/// The identifier resolves through the authored pack, the request names the contact and sends
/// the pack's credential, and the record and the `refresh:` commit follow as for any location.
#[test]
fn an_identifier_is_fetched_through_its_pack_as_its_transport_says() {
    let (port, server) = serve(vec![ok("id,n\n1,2\n")]);
    let s = stage_pack(
        port,
        "contact = \"required\"\nauth = [{ env = \"LB_TOKEN\", header = \"X-Api-Key\" }]",
    );
    let before = s.commit_count();
    let out = s
        .run_env(
            &["catalog-fetch", "loopback-paper", "--format", "json"],
            &[("YIDAM_CONTACT", "ops@example.org"), ("LB_TOKEN", "s3cret")],
        )
        .ok();

    let seen = server.join().unwrap().remove(0).to_ascii_lowercase();
    assert!(seen.starts_with("get /records/rec-1.csv "), "{seen}");
    assert!(seen.contains("(+ops@example.org)"), "{seen}");
    assert!(seen.contains("x-api-key: s3cret"), "{seen}");

    let report: serde_json::Value = serde_json::from_str(&out).unwrap();
    let row = &report["fetched"][0]["obtained"][0];
    assert_eq!(row["pack"], "loopback@0.1.0", "{out}");
    assert_eq!(row["declared"], "lb:rec-1");
    assert!(
        !out.contains("s3cret"),
        "the credential is never reported:\n{out}"
    );

    assert_eq!(s.commit_count(), before + 1);
    assert!(s.head_subject().starts_with("refresh: loopback-paper"));
    assert!(!s.head_body().contains("s3cret"));
    let entry = std::fs::read_to_string(s.root().join(".yidam/catalog/loopback-paper.md")).unwrap();
    assert!(entry.contains("from: 0"), "{entry}");
}

/// A 403 is recorded on the entry and not retried: the listener answers once and closes, so a
/// second request would fail the run with a connection error rather than a refusal.
#[test]
fn a_refusal_is_a_finding_on_the_entry_and_not_retried() {
    let (port, server) = serve(vec![
        "HTTP/1.1 403 Forbidden\r\ncontent-length: 0\r\nconnection: close\r\n\r\n".into(),
    ]);
    let s = stage_pack(port, "");
    let before = s.commit_count();
    let run = s.run_env(
        &["catalog-fetch", "loopback-paper", "--format", "json"],
        &[],
    );
    assert!(!run.ok, "a refusal fails the run:\n{}", run.stdout);
    assert_eq!(server.join().unwrap().len(), 1);

    let report: serde_json::Value = serde_json::from_str(&run.stdout).unwrap();
    let refused = &report["fetched"][0]["refused"][0];
    assert_eq!(refused["status"], 403, "{}", run.stdout);
    assert_eq!(refused["location"], 0);
    assert_eq!(s.commit_count(), before, "nothing was recorded");
}

/// `contact = "required"` and no `YIDAM_CONTACT`: refused before any request. No listener is
/// started, so a request would fail with a connection error rather than this message.
#[test]
fn a_pack_requiring_a_contact_is_refused_without_one() {
    let s = stage_pack(9, "contact = \"required\"");
    let out = s.run(&["catalog-fetch", "loopback-paper"]).ok();
    assert!(out.contains("requires a contact"), "{out}");
    assert!(out.contains("YIDAM_CONTACT"), "{out}");
}

/// `--archive` writes `wayback:` locations, so it needs a pack that declares the scheme.
#[test]
fn archive_without_a_wayback_pack_is_refused_before_anything_is_asked() {
    let s = stage_pack(9, "");
    let run = s.run(&["catalog-fetch", "loopback-paper", "--archive"]);
    assert!(!run.ok);
    assert!(run.stderr.contains("`wayback` scheme"), "{}", run.stderr);
}

// ── a listing, read once and pinned (#1342) ───────────────────────────────────

/// Stage an authored pack whose `lb:` scheme reads its file names off a listing at `port`,
/// and an entry citing `value`.
fn stage_listing(port: u16, value: &str) -> Staged {
    let s = stage();
    let pack = s.root().join(".yidam/sources/loopback");
    std::fs::create_dir_all(&pack).unwrap();
    std::fs::write(pack.join("entry.md"), "").unwrap();
    std::fs::write(
        pack.join("pack.toml"),
        format!(
            "[pack]\nname = \"loopback\"\nversion = \"0.1.0\"\n\n\
             [scheme.lb]\npattern = '^[a-z]+$'\ntype = \"dataset\"\n\
             resolve = {{ listing = \"http://127.0.0.1:{port}/files/\", \
             match = '^{{id}}-\\d{{8}}\\.csv$', pick = \"latest\" }}\n"
        ),
    )
    .unwrap();
    std::fs::write(
        s.root().join(".yidam/catalog/loopback-paper.md"),
        PAPER.replace("lb:rec-1", value),
    )
    .unwrap();
    git(s.root(), &["add", "-A"]);
    git(
        s.root(),
        &["commit", "-q", "-m", "scaffold: a listing pack"],
    );
    s
}

/// A fetch follows the pin to the file it names, and never asks the listing: the server answers
/// once, so a request for the listing first would take the answer meant for the file.
#[test]
fn a_pinned_identifier_is_fetched_from_the_file_it_pins() {
    let (port, server) = serve(vec![ok("id,n\n1,2\n")]);
    let s = stage_listing(port, "lb:rec@rec-20260901.csv");
    let out = s
        .run(&["catalog-fetch", "loopback-paper", "--format", "json"])
        .ok();
    let seen = server.join().unwrap().remove(0).to_ascii_lowercase();
    assert!(seen.starts_with("get /files/rec-20260901.csv "), "{seen}");
    let report: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(
        report["fetched"][0]["obtained"][0]["declared"], "lb:rec@rec-20260901.csv",
        "{out}"
    );
}

/// An unpinned identifier names whichever file the listing shows today, so a fetch skips it,
/// as it skips every refused identifier, and says how to pin it. No listener is started: a
/// request would fail the run as a connection error.
#[test]
fn an_unpinned_identifier_is_skipped_with_how_to_pin_it() {
    let s = stage_listing(9, "lb:rec");
    let before = s.commit_count();
    let out = s.run(&["catalog-fetch", "loopback-paper"]).ok();
    assert!(out.contains("location 0 skipped"), "{out}");
    assert!(out.contains("source add --dry-run lb:rec"), "{out}");
    assert_eq!(s.commit_count(), before, "nothing was recorded");
}

/// `source add` reads the listing over the network and writes the newest matching name as
/// the pin. The listing's link to another host is never a candidate, however new it is.
#[test]
fn source_add_reads_the_listing_and_writes_the_pin() {
    let listing = "<a href=\"../\">up</a>\n\
                   <a href=\"rec-20260901.csv\">x</a> <a href=\"rec-20261001.csv\">y</a>\n\
                   <a href=\"other-20261101.csv\">z</a>\n\
                   <a href=\"https://mirror.example/files/rec-20991231.csv\">m</a>\n";
    let (port, server) = serve(vec![format!(
        "HTTP/1.1 200 OK\r\ncontent-type: text/html\r\ncontent-length: {}\r\n\
         connection: close\r\n\r\n{listing}",
        listing.len()
    )]);
    let s = stage_listing(port, "lb:other@other-20261101.csv");
    let out = s.run(&["source", "add", "lb:rec", "--format", "json"]).ok();
    let seen = server.join().unwrap().remove(0).to_ascii_lowercase();
    assert!(seen.starts_with("get /files/ "), "{seen}");
    let report: serde_json::Value = serde_json::from_str(&out).unwrap();
    let located = &report["drafts"][0]["locations"][0];
    assert_eq!(located["identifier"], "lb:rec@rec-20261001.csv", "{out}");
    assert_eq!(
        located["url"],
        format!("http://127.0.0.1:{port}/files/rec-20261001.csv")
    );
    let path = report["drafts"][0]["path"].as_str().unwrap();
    let entry = std::fs::read_to_string(s.root().join(path)).unwrap();
    assert!(entry.contains("value: lb:rec@rec-20261001.csv"), "{entry}");
    assert!(entry.contains("pinned to the file"), "{entry}");

    // The same source, re-read, is the entry already written, whatever file it pins.
    let again = s.run(&["source", "add", "lb:rec@rec-20260901.csv"]);
    assert!(!again.ok);
    assert!(
        again.stderr.contains("already catalogued"),
        "{}",
        again.stderr
    );
}

// ── a paged service, read to its end (#1341) ──────────────────────────────────

/// Stage an authored pack whose `lb:` scheme is an ArcGIS-style query at `port`, paged as
/// `paginate` says when it says, and an entry citing `lb:rec-1`.
fn stage_paged(port: u16, paginate: Option<&str>) -> Staged {
    let s = stage();
    let pack = s.root().join(".yidam/sources/loopback");
    std::fs::create_dir_all(&pack).unwrap();
    std::fs::write(
        pack.join("pack.toml"),
        format!(
            "[pack]\nname = \"loopback\"\nversion = \"0.1.0\"\n\n\
             [scheme.lb]\npattern = '^rec-\\d+$'\ntype = \"dataset\"\n\
             resolve = {{ template = \"http://127.0.0.1:{port}/{{id}}/query?where=1%3D1&f=json\", \
             media = \"application/json\" }}\n{}",
            paginate
                .map(|p| format!("paginate = {p}\n"))
                .unwrap_or_default()
        ),
    )
    .unwrap();
    std::fs::write(s.root().join(".yidam/catalog/loopback-paper.md"), PAPER).unwrap();
    git(s.root(), &["add", "-A"]);
    git(s.root(), &["commit", "-q", "-m", "scaffold: a paged pack"]);
    s
}

fn json_ok(body: &str) -> String {
    ok(body).replace("content-type: text/csv", "content-type: application/json")
}

const ARCGIS: &str =
    r#"{ offset = "resultOffset", items = "features/*", until = "!exceededTransferLimit" }"#;

/// The acceptance case: a service that answers in two pages is asked for both, at the offset
/// the first page's records reach, and the entry records one artifact holding every record.
#[test]
fn a_two_page_answer_is_recorded_as_one_artifact() {
    let (port, server) = serve(vec![
        json_ok(r#"{"features":[{"id":1},{"id":2}],"exceededTransferLimit":true}"#),
        json_ok(r#"{"features":[{"id":3}]}"#),
    ]);
    let s = stage_paged(port, Some(ARCGIS));
    let before = s.commit_count();
    let out = s
        .run(&["catalog-fetch", "loopback-paper", "--format", "json"])
        .ok();

    let seen = server.join().unwrap();
    assert_eq!(seen.len(), 2);
    assert!(
        seen[0].starts_with("GET /rec-1/query?where=1%3D1&f=json&resultOffset=0 "),
        "{}",
        seen[0]
    );
    assert!(
        seen[1].starts_with("GET /rec-1/query?where=1%3D1&f=json&resultOffset=2 "),
        "{}",
        seen[1]
    );

    let report: serde_json::Value = serde_json::from_str(&out).unwrap();
    let row = &report["fetched"][0]["obtained"];
    assert_eq!(row.as_array().unwrap().len(), 1, "{out}");
    assert_eq!(row[0]["pages"], 2, "{out}");
    assert_eq!(
        row[0]["followed"],
        format!("http://127.0.0.1:{port}/rec-1/query?where=1%3D1&f=json")
    );

    let sha = row[0]["sha256"].as_str().unwrap();
    let held = std::fs::read(s.cache().join("sha256").join(&sha[..2]).join(sha)).unwrap();
    let doc: serde_json::Value = serde_json::from_slice(&held).unwrap();
    assert_eq!(
        doc,
        serde_json::json!({"features":[{"id":1},{"id":2},{"id":3}]}),
        "every record, and nothing saying the answer was cut short"
    );

    assert_eq!(s.commit_count(), before + 1);
    assert!(s.head_body().contains(", in 2 pages)"), "{}", s.head_body());
    let entry = std::fs::read_to_string(s.root().join(".yidam/catalog/loopback-paper.md")).unwrap();
    assert_eq!(entry.matches("sha256:").count(), 1, "{entry}");
}

/// Without `paginate`, a page that says it was cut short is refused: a finding on the entry,
/// nothing recorded, and the run fails. The listener answers once, so the fetch did not page.
#[test]
fn a_truncated_page_is_refused_without_a_paginate_declaration() {
    let (port, server) = serve(vec![json_ok(
        r#"{"features":[{"id":1}],"exceededTransferLimit":true}"#,
    )]);
    let s = stage_paged(port, None);
    let before = s.commit_count();
    let run = s.run_env(
        &["catalog-fetch", "loopback-paper", "--format", "json"],
        &[],
    );
    assert!(!run.ok, "a truncated page fails the run:\n{}", run.stdout);
    assert_eq!(server.join().unwrap().len(), 1);

    let report: serde_json::Value = serde_json::from_str(&run.stdout).unwrap();
    let refused = &report["fetched"][0]["refused"][0];
    assert_eq!(refused["status"], 200, "{}", run.stdout);
    let why = refused["why"].as_str().unwrap();
    assert!(why.contains("exceededTransferLimit"), "{why}");
    assert!(why.contains("declares no `paginate`"), "{why}");
    assert!(report["fetched"][0]["obtained"]
        .as_array()
        .unwrap()
        .is_empty());
    assert_eq!(s.commit_count(), before, "nothing was recorded");
}
