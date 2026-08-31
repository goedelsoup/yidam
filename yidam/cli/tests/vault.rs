//! `yidam vault`, end to end, with no network and no shared state.
//!
//! Every test here runs the real binary against a real `file://` vault in a `TempDir`, which
//! is the point: `file://` is a shipped backend rather than a test double, so these exercise
//! the code a corpus on a mounted archive actually runs. Nothing is mocked and nothing is
//! skipped when a server is absent, because no server is involved.
//!
//! **Each test gets its own cache.** `YIDAM_VAULT_CACHE` is set per invocation, so no test
//! touches the developer's real cache and no two tests can see each other's artifacts —
//! which matters more than usual here, because the cache is machine-wide by design and
//! `cargo test` runs these in parallel.
//!
//! Assertions are on **exit codes** wherever a command is a gate. `verify` that prints
//! "corrupt" and returns 0 is broken in the way that matters, and prose is checked only where
//! the message itself is the feature — a refusal that does not name what is wrong.

use std::path::{Path, PathBuf};
use std::process::Command;

struct Run {
    stdout: String,
    stderr: String,
    code: i32,
}

impl Run {
    fn ok(&self) -> &Run {
        assert_eq!(
            self.code, 0,
            "expected success\n{}{}",
            self.stdout, self.stderr
        );
        self
    }
    fn failed(&self) -> &Run {
        assert_ne!(
            self.code, 0,
            "expected a nonzero exit\n{}{}",
            self.stdout, self.stderr
        );
        self
    }
    /// stdout and stderr together — a refusal's text may land on either.
    fn said(&self) -> String {
        format!("{}{}", self.stdout, self.stderr)
    }
}

fn run(dir: &Path, cache: &Path, args: &[&str]) -> Run {
    let out = Command::new(env!("CARGO_BIN_EXE_yidam"))
        .current_dir(dir)
        .env("YIDAM_VAULT_CACHE", cache)
        .args(args)
        .output()
        .expect("running yidam");
    Run {
        stdout: String::from_utf8_lossy(&out.stdout).to_string(),
        stderr: String::from_utf8_lossy(&out.stderr).to_string(),
        code: out.status.code().unwrap_or(-1),
    }
}

/// A derived repository, optionally with a `[vault.…]` section.
fn repo(config: Option<&str>) -> tempfile::TempDir {
    let tmp = tempfile::tempdir().unwrap();
    assert!(Command::new("git")
        .args(["init", "-q"])
        .current_dir(tmp.path())
        .status()
        .unwrap()
        .success());
    std::fs::create_dir_all(tmp.path().join(".yidam")).unwrap();
    if let Some(c) = config {
        std::fs::write(tmp.path().join(".yidam/config.toml"), c).unwrap();
    }
    tmp
}

fn write(path: &Path, bytes: &[u8]) -> PathBuf {
    if let Some(p) = path.parent() {
        std::fs::create_dir_all(p).unwrap();
    }
    std::fs::write(path, bytes).unwrap();
    path.to_path_buf()
}

/// Seed a `file://` vault **by hand**, rather than through `yidam` itself.
///
/// Deliberate: if the layout were written by the same code that reads it, the two could
/// agree on something wrong and this suite would never notice. Spelling the key out here
/// makes `sha256/<aa>/<digest>` an assertion rather than an implementation detail.
fn seed_vault(vault: &Path, bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let digest = hex::encode(Sha256::digest(bytes));
    write(
        &vault.join("sha256").join(&digest[..2]).join(&digest),
        bytes,
    );
    digest
}

/// Two vaults, each declaring what it holds — the shape #416 exists for.
fn two_vaults(a: &Path, a_holds: &str, b: &Path, b_holds: &str) -> String {
    format!(
        "[vault.default]\nurl = \"file://{}\"\naudience = \"anyone reading this corpus\"\n\
         holds = [\"{a_holds}\"]\n\
         [vault.sources]\nurl = \"file://{}\"\naudience = \"the sangha only\"\n\
         holds = [\"{b_holds}\"]\n",
        a.display(),
        b.display()
    )
}

/// Whether a `file://` vault directory holds this digest, at the layout the store writes.
fn holds(vault: &Path, digest: &str) -> bool {
    vault
        .join("sha256")
        .join(&digest[..2])
        .join(digest)
        .is_file()
}

fn vault_config(vault_dir: &Path) -> String {
    format!(
        "[vault.default]\nurl = \"file://{}\"\naudience = \"the test suite\"\n",
        vault_dir.display()
    )
}

// ── the cache, with no vault involved ────────────────────────────────────────

#[test]
fn put_then_path_round_trips_an_artifact_through_the_cache() {
    let tmp = repo(None);
    let cache = tmp.path().join("cache");
    let src = write(&tmp.path().join("doc.pdf"), b"a fetched document");

    let put = run(tmp.path(), &cache, &["vault", "put", src.to_str().unwrap()]);
    let digest = put.ok().stdout.trim().to_string();
    assert_eq!(
        digest.len(),
        64,
        "stdout carries the digest alone: {digest:?}"
    );

    let found = run(tmp.path(), &cache, &["vault", "path", &digest]);
    let at = PathBuf::from(found.ok().stdout.trim());
    assert_eq!(std::fs::read(&at).unwrap(), b"a fetched document");
}

/// `yidam vault path $h || fetch-it` has to work, so an absent artifact is a nonzero exit
/// rather than an empty line.
#[test]
fn path_exits_nonzero_for_an_artifact_the_cache_does_not_hold() {
    let tmp = repo(None);
    let cache = tmp.path().join("cache");
    let absent = "0".repeat(64);
    run(tmp.path(), &cache, &["vault", "path", &absent]).failed();
}

#[test]
fn a_malformed_digest_is_refused_before_anything_is_looked_up() {
    let tmp = repo(None);
    let cache = tmp.path().join("cache");
    for bad in ["nonsense", &"A".repeat(64), &"z".repeat(64)] {
        let r = run(tmp.path(), &cache, &["vault", "path", bad]);
        assert!(r.failed().said().contains("sha256"), "for {bad}");
    }
}

/// The rule from `cmd/vault.rs`: cache-only commands need no repository.
#[test]
fn the_cache_commands_work_outside_a_repository() {
    let tmp = tempfile::tempdir().unwrap();
    let cache = tmp.path().join("cache");
    let src = write(&tmp.path().join("a.bin"), b"no repository here");

    let digest = run(tmp.path(), &cache, &["vault", "put", src.to_str().unwrap()])
        .ok()
        .stdout
        .trim()
        .to_string();
    run(tmp.path(), &cache, &["vault", "path", &digest]).ok();
    run(tmp.path(), &cache, &["vault", "verify"]).ok();
}

/// …and the other half of that rule: a command that reads the vault config needs one.
#[test]
fn listing_vaults_outside_a_repository_says_so() {
    let tmp = tempfile::tempdir().unwrap();
    let cache = tmp.path().join("cache");
    let r = run(tmp.path(), &cache, &["vault", "list"]);
    assert!(
        r.failed().said().contains("not a yidam repository"),
        "{}",
        r.said()
    );
}

// ── verify ───────────────────────────────────────────────────────────────────

#[test]
fn verify_passes_on_an_intact_cache_and_reports_the_count() {
    let tmp = repo(None);
    let cache = tmp.path().join("cache");
    for (name, body) in [("a", &b"one"[..]), ("b", b"two")] {
        let src = write(&tmp.path().join(name), body);
        run(tmp.path(), &cache, &["vault", "put", src.to_str().unwrap()]).ok();
    }
    let r = run(tmp.path(), &cache, &["vault", "verify"]);
    assert!(r.ok().said().contains("2 artifacts"), "{}", r.said());
}

/// The case the whole design exists to make detectable. It must **exit nonzero** — a report
/// that names the corruption and returns 0 is a gate that does not gate.
#[test]
fn verify_exits_nonzero_when_cached_bytes_are_not_what_they_claim() {
    let tmp = repo(None);
    let cache = tmp.path().join("cache");
    let src = write(&tmp.path().join("a"), b"original bytes");
    let digest = run(tmp.path(), &cache, &["vault", "put", src.to_str().unwrap()])
        .ok()
        .stdout
        .trim()
        .to_string();

    let at = PathBuf::from(
        run(tmp.path(), &cache, &["vault", "path", &digest])
            .ok()
            .stdout
            .trim(),
    );
    std::fs::write(&at, b"something else entirely").unwrap();

    let r = run(tmp.path(), &cache, &["vault", "verify"]);
    let said = r.failed().said();
    assert!(said.contains("corrupt"), "{said}");
    assert!(said.contains(&digest), "names which artifact: {said}");
}

// ── a file:// vault ──────────────────────────────────────────────────────────

#[test]
fn get_fetches_from_the_vault_when_the_cache_misses_and_caches_it() {
    let vault = tempfile::tempdir().unwrap();
    let digest = seed_vault(vault.path(), b"an artifact only the vault has");
    let tmp = repo(Some(&vault_config(vault.path())));
    let cache = tmp.path().join("cache");

    // Not in the cache to begin with.
    run(tmp.path(), &cache, &["vault", "path", &digest]).failed();

    let got = run(tmp.path(), &cache, &["vault", "get", &digest]);
    let at = PathBuf::from(got.ok().stdout.trim());
    assert_eq!(
        std::fs::read(&at).unwrap(),
        b"an artifact only the vault has"
    );

    // …and now it is, so a second get needs no vault at all.
    run(tmp.path(), &cache, &["vault", "path", &digest]).ok();
}

#[test]
fn get_writes_a_named_copy_with_out() {
    let vault = tempfile::tempdir().unwrap();
    let digest = seed_vault(vault.path(), b"pearl 2009");
    let tmp = repo(Some(&vault_config(vault.path())));
    let cache = tmp.path().join("cache");

    let dest = tmp.path().join("sub/dir/pearl-2009.pdf");
    run(
        tmp.path(),
        &cache,
        &["vault", "get", &digest, "--out", dest.to_str().unwrap()],
    )
    .ok();
    assert_eq!(std::fs::read(&dest).unwrap(), b"pearl 2009");
}

/// A store that hands back bytes which are not the digest asked for is exactly what content
/// addressing exists to catch. Nothing may be cached, because a cached artifact is one a
/// later command will trust without re-checking.
#[test]
fn a_vault_returning_the_wrong_bytes_is_refused_and_nothing_is_cached() {
    let vault = tempfile::tempdir().unwrap();
    let digest = seed_vault(vault.path(), b"the right bytes");
    // Rewrite the store's copy underneath the name it is filed under.
    let planted = vault.path().join("sha256").join(&digest[..2]).join(&digest);
    std::fs::write(&planted, b"the WRONG bytes").unwrap();

    let tmp = repo(Some(&vault_config(vault.path())));
    let cache = tmp.path().join("cache");

    let r = run(tmp.path(), &cache, &["vault", "get", &digest]);
    let said = r.failed().said();
    assert!(said.contains("not"), "{said}");
    assert!(said.contains(&digest), "names what was asked for: {said}");
    run(tmp.path(), &cache, &["vault", "path", &digest]).failed();
}

#[test]
fn get_says_where_it_looked_when_neither_the_cache_nor_the_vault_has_it() {
    let vault = tempfile::tempdir().unwrap();
    let tmp = repo(Some(&vault_config(vault.path())));
    let cache = tmp.path().join("cache");
    let absent = "1".repeat(64);

    let r = run(tmp.path(), &cache, &["vault", "get", &absent]);
    let said = r.failed().said();
    assert!(said.contains("cache"), "{said}");
    assert!(said.contains("default"), "names the vault it asked: {said}");
}

#[test]
fn get_without_a_configured_vault_says_there_is_nowhere_to_look() {
    let tmp = repo(None);
    let cache = tmp.path().join("cache");
    let r = run(tmp.path(), &cache, &["vault", "get", &"2".repeat(64)]);
    assert!(
        r.failed().said().contains("declares no vault"),
        "{}",
        r.said()
    );
}

// ── configuration ────────────────────────────────────────────────────────────

/// A corpus with no vault is every corpus until somebody configures one. It is not an error
/// and `list` must not treat it as one.
#[test]
fn listing_with_no_vault_configured_succeeds_and_shows_the_shape() {
    let tmp = repo(None);
    let cache = tmp.path().join("cache");
    let r = run(tmp.path(), &cache, &["vault", "list"]);
    let said = r.ok().said();
    assert!(said.contains("No vault configured"), "{said}");
    assert!(said.contains("[vault.default]"), "shows the shape: {said}");
}

#[test]
fn listing_a_configured_vault_shows_its_audience() {
    let vault = tempfile::tempdir().unwrap();
    let tmp = repo(Some(&vault_config(vault.path())));
    let cache = tmp.path().join("cache");
    let said = run(tmp.path(), &cache, &["vault", "list"]).ok().said();
    assert!(said.contains("the test suite"), "{said}");
}

/// **A vault among several that claims nothing is refused**, rather than made the catch-all
/// for whatever the others did not take. Routing by default is how a licensed document
/// reaches the store meant for public output without anybody deciding it should.
#[test]
fn a_second_vault_that_claims_nothing_is_refused_rather_than_made_a_catch_all() {
    let a = tempfile::tempdir().unwrap();
    let b = tempfile::tempdir().unwrap();
    let config = format!(
        "[vault.default]\nurl = \"file://{}\"\naudience = \"everyone\"\n\
         [vault.sources]\nurl = \"file://{}\"\naudience = \"the sangha\"\n\
         holds = [\"catalog\"]\n",
        a.path().display(),
        b.path().display()
    );
    let tmp = repo(Some(&config));
    let cache = tmp.path().join("cache");

    let said = run(tmp.path(), &cache, &["vault", "list"]).failed().said();
    assert!(said.contains("`default`"), "names the silent one: {said}");
    assert!(said.contains("catch-all"), "says why: {said}");
}

/// One kind, two stores, and nothing that could pick between them.
#[test]
fn a_kind_claimed_by_two_vaults_is_refused_naming_both() {
    let (a, b) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let tmp = repo(Some(&two_vaults(a.path(), "catalog", b.path(), "catalog")));
    let cache = tmp.path().join("cache");
    let said = run(tmp.path(), &cache, &["vault", "list"]).failed().said();
    assert!(said.contains("`catalog`"), "{said}");
    assert!(
        said.contains("`default`") && said.contains("`sources`"),
        "{said}"
    );
}

#[test]
fn a_vault_that_does_not_say_who_can_read_it_is_refused() {
    let vault = tempfile::tempdir().unwrap();
    let config = format!(
        "[vault.default]\nurl = \"file://{}\"\n",
        vault.path().display()
    );
    let tmp = repo(Some(&config));
    let cache = tmp.path().join("cache");
    let said = run(tmp.path(), &cache, &["vault", "list"]).failed().said();
    assert!(said.contains("audience"), "{said}");
}

/// An `s3://` vault with no credentials in the environment is a *configuration* state, not a
/// broken build. `list` reports it and still succeeds — the declaration is legitimate and it
/// is the environment that is incomplete — and the message names the variables to set.
#[test]
fn an_s3_vault_without_credentials_is_reported_and_names_what_to_set() {
    let config = "[vault.default]\nurl = \"s3://bucket/prefix\"\naudience = \"the sangha\"\n";
    let tmp = repo(Some(config));
    let cache = tmp.path().join("cache");
    let said = run(tmp.path(), &cache, &["vault", "list"]).ok().said();
    assert!(said.contains("no credentials"), "{said}");
    assert!(
        said.contains("YIDAM_VAULT_DEFAULT_ACCESS_KEY_ID"),
        "names what to set: {said}"
    );

    // A command that actually needs the store fails rather than reporting.
    run(tmp.path(), &cache, &["vault", "get", &"3".repeat(64)]).failed();
}

// ── push, and the guard that comes with it ───────────────────────────────────

/// A catalog entry naming one artifact, with whatever the test needs to say about it.
fn entry(tmp: &Path, name: &str, digest: &str, extra: &str) {
    let dir = tmp.join(".yidam/catalog");
    std::fs::create_dir_all(&dir).unwrap();
    write(
        &dir.join(format!("{name}.md")),
        format!(
            "---\nname: {name}\ndescription: A source.\ntype: paper\n\
             artifacts:\n  - sha256: {digest}\n{extra}---\n\nBody.\n"
        )
        .as_bytes(),
    );
}

fn cached(tmp: &Path, cache: &Path, body: &[u8]) -> String {
    let src = write(&tmp.join(format!("src-{}", body.len())), body);
    let (o, e, c) = {
        let r = run(tmp, cache, &["vault", "put", src.to_str().unwrap()]);
        (r.stdout.clone(), r.stderr.clone(), r.code)
    };
    assert_eq!(c, 0, "{o}{e}");
    o.trim().to_string()
}

/// **The default is refusal.** A record that says nothing about redistribution is not a
/// licence, and the first push anybody runs must not become one.
#[test]
fn a_record_that_does_not_license_redistribution_is_not_pushed() {
    let vault = tempfile::tempdir().unwrap();
    let tmp = repo(Some(&vault_config(vault.path())));
    let cache = tmp.path().join("cache");
    let digest = cached(tmp.path(), &cache, b"a licensed paper");
    entry(tmp.path(), "paper", &digest, "");

    let said = run(tmp.path(), &cache, &["vault", "push"]).ok().said();
    assert!(said.contains("1 refused"), "{said}");
    assert!(said.contains("does not say"), "{said}");
    // Nothing reached the store.
    assert!(!vault.path().join("sha256").exists(), "nothing may be sent");
}

#[test]
fn an_explicit_licence_is_what_sends_it() {
    let vault = tempfile::tempdir().unwrap();
    let tmp = repo(Some(&vault_config(vault.path())));
    let cache = tmp.path().join("cache");
    let digest = cached(tmp.path(), &cache, b"an open-access paper");
    entry(tmp.path(), "paper", &digest, "    redistributable: true\n");

    let said = run(tmp.path(), &cache, &["vault", "push"]).ok().said();
    assert!(said.contains("1 sent"), "{said}");
    let at = vault.path().join("sha256").join(&digest[..2]).join(&digest);
    assert_eq!(std::fs::read(&at).unwrap(), b"an open-access paper");
}

/// **The load-bearing guard.** A declared-private path refuses a push the licence would
/// otherwise have allowed. The two questions are independent and an artifact clears both.
#[test]
fn a_private_path_refuses_a_push_the_licence_would_have_allowed() {
    let vault = tempfile::tempdir().unwrap();
    let tmp = repo(Some(&vault_config(vault.path())));
    let cache = tmp.path().join("cache");
    let digest = cached(tmp.path(), &cache, b"embargoed material");
    entry(tmp.path(), "secret", &digest, "    redistributable: true\n");
    write(
        &tmp.path().join(".yidam/private-paths"),
        b"# nothing here may be published\n.yidam/catalog\n",
    );

    let said = run(tmp.path(), &cache, &["vault", "push"]).ok().said();
    assert!(said.contains("1 refused"), "{said}");
    assert!(said.contains("private-paths"), "{said}");
    assert!(said.contains("outlives the access"), "says why: {said}");
    assert!(!vault.path().join("sha256").exists(), "nothing may be sent");
}

/// `--artifact` narrows what is pushed. A digest with no record has no `redistributable` and
/// no path to check, so allowing it would be a hole in the guard shaped like a flag.
#[test]
fn artifact_cannot_push_something_the_corpus_does_not_record() {
    let vault = tempfile::tempdir().unwrap();
    let tmp = repo(Some(&vault_config(vault.path())));
    let cache = tmp.path().join("cache");
    let digest = cached(tmp.path(), &cache, b"unrecorded bytes");

    let said = run(
        tmp.path(),
        &cache,
        &["vault", "push", "--artifact", &digest],
    )
    .failed()
    .said();
    assert!(said.contains("no catalog entry names"), "{said}");
    assert!(!vault.path().join("sha256").exists(), "nothing may be sent");
}

/// `vault: none` is a route to the local cache. It is refused as a route rather than as a
/// licensing problem, because those need different things from the reader.
#[test]
fn vault_none_is_refused_as_a_route() {
    let vault = tempfile::tempdir().unwrap();
    let tmp = repo(Some(&vault_config(vault.path())));
    let cache = tmp.path().join("cache");
    let digest = cached(tmp.path(), &cache, b"local only");
    entry(
        tmp.path(),
        "local",
        &digest,
        "    vault: none\n    redistributable: true\n",
    );
    let said = run(tmp.path(), &cache, &["vault", "push"]).ok().said();
    assert!(said.contains("local cache"), "{said}");
    assert!(!vault.path().join("sha256").exists());
}

/// A push refusal quotes the destination's own audience, so the reader learns what they were
/// about to publish to rather than only that something was blocked.
#[test]
fn a_refusal_names_the_audience_of_the_store_it_declined_to_send_to() {
    let vault = tempfile::tempdir().unwrap();
    let tmp = repo(Some(&vault_config(vault.path())));
    let cache = tmp.path().join("cache");
    let digest = cached(tmp.path(), &cache, b"x");
    entry(tmp.path(), "paper", &digest, "");
    let said = run(tmp.path(), &cache, &["vault", "push"]).ok().said();
    assert!(said.contains("the test suite"), "{said}");
}

/// A fresh clone has recorded artifacts and none of the bytes. That is normal, not a failure,
/// and `push` must not go red on it.
#[test]
fn a_push_from_a_clone_that_has_fetched_nothing_is_not_a_failure() {
    let vault = tempfile::tempdir().unwrap();
    let tmp = repo(Some(&vault_config(vault.path())));
    let cache = tmp.path().join("cache");
    let digest = "9".repeat(64);
    entry(tmp.path(), "paper", &digest, "    redistributable: true\n");
    let said = run(tmp.path(), &cache, &["vault", "push"]).ok().said();
    assert!(said.contains("1 not cached"), "{said}");
}

// ── pull and status ──────────────────────────────────────────────────────────

#[test]
fn pull_fetches_what_the_corpus_names_and_the_cache_lacks() {
    let vault = tempfile::tempdir().unwrap();
    let digest = seed_vault(vault.path(), b"a paper the vault has");
    let tmp = repo(Some(&vault_config(vault.path())));
    let cache = tmp.path().join("cache");
    entry(tmp.path(), "paper", &digest, "");

    run(tmp.path(), &cache, &["vault", "pull"]).ok();
    run(tmp.path(), &cache, &["vault", "path", &digest]).ok();
}

/// An artifact the corpus names and nowhere holds is a real problem — a reader following a
/// citation cannot get the document — so `pull` exits nonzero rather than reporting quietly.
#[test]
fn pull_exits_nonzero_when_something_named_is_nowhere() {
    let vault = tempfile::tempdir().unwrap();
    let tmp = repo(Some(&vault_config(vault.path())));
    let cache = tmp.path().join("cache");
    entry(tmp.path(), "paper", &"4".repeat(64), "");
    let said = run(tmp.path(), &cache, &["vault", "pull"]).failed().said();
    assert!(said.contains("1 unavailable"), "{said}");
}

#[test]
fn status_says_where_each_named_artifact_is() {
    let vault = tempfile::tempdir().unwrap();
    let digest = seed_vault(vault.path(), b"in the vault only");
    let tmp = repo(Some(&vault_config(vault.path())));
    let cache = tmp.path().join("cache");
    entry(tmp.path(), "paper", &digest, "");

    let said = run(tmp.path(), &cache, &["vault", "status"]).ok().said();
    assert!(said.contains(&digest), "{said}");
    assert!(said.contains('-'), "not cached: {said}");

    let said = run(tmp.path(), &cache, &["vault", "status", "--remote"])
        .ok()
        .said();
    assert!(said.contains("stored"), "the vault has it: {said}");
}

/// `status` is the one command that re-hashes rather than trusting the cache, so a rotted
/// artifact is reported — and it exits nonzero, because a wrong local copy is a real defect.
#[test]
fn status_exits_nonzero_when_a_cached_artifact_is_not_what_the_corpus_records() {
    let vault = tempfile::tempdir().unwrap();
    let tmp = repo(Some(&vault_config(vault.path())));
    let cache = tmp.path().join("cache");
    let digest = cached(tmp.path(), &cache, b"original");
    entry(tmp.path(), "paper", &digest, "");

    let at = PathBuf::from(
        run(tmp.path(), &cache, &["vault", "path", &digest])
            .ok()
            .stdout
            .trim(),
    );
    std::fs::write(&at, b"rotted").unwrap();

    let said = run(tmp.path(), &cache, &["vault", "status"])
        .failed()
        .said();
    assert!(said.contains("CORRUPT"), "{said}");
}

// ── named vaults, plural (#416) ──────────────────────────────────────────────

/// **The load-bearing test of this phase.** A licensed document routes to the store whose
/// audience is the sangha, and the store meant for public output never sees it.
#[test]
fn a_catalog_artifact_reaches_the_vault_that_holds_catalogs_and_no_other() {
    let (pub_, priv_) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let tmp = repo(Some(&two_vaults(
        pub_.path(),
        "index",
        priv_.path(),
        "catalog",
    )));
    let cache = tmp.path().join("cache");
    let digest = cached(tmp.path(), &cache, b"a licensed paper");
    entry(tmp.path(), "paper", &digest, "    redistributable: true\n");

    let said = run(tmp.path(), &cache, &["vault", "push"]).ok().said();
    assert!(said.contains("1 sent"), "{said}");
    assert!(holds(priv_.path(), &digest), "must reach `sources`: {said}");
    assert!(
        !holds(pub_.path(), &digest),
        "must never reach the public store: {said}"
    );
}

/// **`--vault` narrows and never re-routes.** Naming the public store does not move a record
/// routed to the private one into it — that is an edit to the record, in a commit.
#[test]
fn the_vault_flag_narrows_and_never_re_routes() {
    let (pub_, priv_) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let tmp = repo(Some(&two_vaults(
        pub_.path(),
        "index",
        priv_.path(),
        "catalog",
    )));
    let cache = tmp.path().join("cache");
    let digest = cached(tmp.path(), &cache, b"a licensed paper");
    entry(tmp.path(), "paper", &digest, "    redistributable: true\n");

    let said = run(tmp.path(), &cache, &["vault", "push", "--vault", "default"])
        .ok()
        .said();
    assert!(
        !holds(pub_.path(), &digest),
        "a flag is not a route: {said}"
    );
    assert!(!holds(priv_.path(), &digest), "and it did narrow: {said}");

    // Naming the store the record actually routes to does send it.
    run(tmp.path(), &cache, &["vault", "push", "--vault", "sources"]).ok();
    assert!(holds(priv_.path(), &digest));
}

#[test]
fn the_vault_flag_refuses_a_name_nothing_declares() {
    let (a, b) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let tmp = repo(Some(&two_vaults(a.path(), "index", b.path(), "catalog")));
    let cache = tmp.path().join("cache");
    let said = run(tmp.path(), &cache, &["vault", "push", "--vault", "archive"])
        .failed()
        .said();
    assert!(said.contains("--vault archive"), "{said}");
    assert!(said.contains("does not create a route"), "{said}");
}

/// A record's own `vault:` outranks the route its kind would take. Specific beats general.
#[test]
fn a_records_own_vault_overrides_the_route_its_kind_would_take() {
    let (pub_, priv_) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let tmp = repo(Some(&two_vaults(
        pub_.path(),
        "index",
        priv_.path(),
        "catalog",
    )));
    let cache = tmp.path().join("cache");
    let digest = cached(tmp.path(), &cache, b"an openly licensed dataset");
    entry(
        tmp.path(),
        "dataset",
        &digest,
        "    vault: default\n    redistributable: true\n",
    );

    run(tmp.path(), &cache, &["vault", "push"]).ok();
    assert!(holds(pub_.path(), &digest), "the record's own route wins");
    assert!(!holds(priv_.path(), &digest));
}

/// A kind nobody claims is reported at the artifact, with the remedy — and nothing moves.
#[test]
fn an_artifact_whose_kind_no_vault_claims_is_stranded_rather_than_sent_somewhere() {
    let (a, b) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let tmp = repo(Some(&two_vaults(a.path(), "index", b.path(), "bundle")));
    let cache = tmp.path().join("cache");
    let digest = cached(tmp.path(), &cache, b"a paper with nowhere to go");
    entry(tmp.path(), "paper", &digest, "    redistributable: true\n");

    let said = run(tmp.path(), &cache, &["vault", "push"]).ok().said();
    assert!(said.contains("no vault holds `catalog`"), "{said}");
    assert!(said.contains("1 refused"), "{said}");
    assert!(
        !holds(a.path(), &digest) && !holds(b.path(), &digest),
        "{said}"
    );
}

/// **Stores are opened lazily.** A vault this machine cannot open must not stop a push to one
/// it can — that is the situation `--vault` was added for, and it has to hold without the flag
/// too, because a CI runner does not want to name its own store on every invocation.
#[test]
fn a_vault_with_nothing_routed_to_it_is_never_opened() {
    let good = tempfile::tempdir().unwrap();
    // `file://relative` is a host, not a path, and `open` refuses it. Declared but unusable.
    let config = format!(
        "[vault.default]\nurl = \"file://{}\"\naudience = \"anyone\"\nholds = [\"catalog\"]\n\
         [vault.broken]\nurl = \"file://nowhere/x\"\naudience = \"nobody\"\nholds = [\"index\"]\n",
        good.path().display()
    );
    let tmp = repo(Some(&config));
    let cache = tmp.path().join("cache");
    let digest = cached(tmp.path(), &cache, b"a paper");
    entry(tmp.path(), "paper", &digest, "    redistributable: true\n");

    let said = run(tmp.path(), &cache, &["vault", "push"]).ok().said();
    assert!(holds(good.path(), &digest), "{said}");
}

/// A refusal names the audience of the store it declined to send to, per store — with several
/// vaults that is the only way to tell which boundary held.
#[test]
fn a_refusal_is_grouped_under_the_audience_of_the_store_it_was_headed_for() {
    let (pub_, priv_) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let tmp = repo(Some(&two_vaults(
        pub_.path(),
        "index",
        priv_.path(),
        "catalog",
    )));
    let cache = tmp.path().join("cache");
    let digest = cached(tmp.path(), &cache, b"unlicensed");
    entry(tmp.path(), "paper", &digest, "");

    let said = run(tmp.path(), &cache, &["vault", "push"]).ok().said();
    assert!(said.contains("sources — the sangha only"), "{said}");
    assert!(said.contains("does not say"), "{said}");
    assert!(
        !said.contains("anyone reading this corpus"),
        "the wrong audience: {said}"
    );
}

#[test]
fn list_shows_every_vault_with_what_it_holds_and_who_reads_it() {
    let (a, b) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let tmp = repo(Some(&two_vaults(a.path(), "index", b.path(), "catalog")));
    let cache = tmp.path().join("cache");
    let digest = cached(tmp.path(), &cache, b"a paper");
    entry(tmp.path(), "paper", &digest, "    redistributable: true\n");

    let said = run(tmp.path(), &cache, &["vault", "list"]).ok().said();
    assert!(said.contains("anyone reading this corpus"), "{said}");
    assert!(said.contains("the sangha only"), "{said}");
    assert!(said.contains("index"), "{said}");
    assert!(said.contains("1 artifact the corpus names"), "{said}");
}

#[test]
fn status_groups_what_the_corpus_names_by_the_store_it_goes_to() {
    let (a, b) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let tmp = repo(Some(&two_vaults(a.path(), "index", b.path(), "catalog")));
    let cache = tmp.path().join("cache");
    let digest = cached(tmp.path(), &cache, b"a paper");
    entry(tmp.path(), "paper", &digest, "    redistributable: true\n");
    seed_vault(b.path(), b"a paper");

    let said = run(tmp.path(), &cache, &["vault", "status", "--remote"])
        .ok()
        .said();
    assert!(said.contains("sources — the sangha only"), "{said}");
    assert!(said.contains("stored"), "{said}");
}

/// `pull --vault` touches only that store, so a corpus can fetch what one runner can reach.
#[test]
fn pull_can_be_narrowed_to_one_store() {
    let (pub_, priv_) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let tmp = repo(Some(&two_vaults(
        pub_.path(),
        "index",
        priv_.path(),
        "catalog",
    )));
    let cache = tmp.path().join("cache");
    let digest = seed_vault(priv_.path(), b"a paper only the sangha has");
    entry(tmp.path(), "paper", &digest, "");

    // Narrowed to the store the record does not route to: nothing is in scope at all.
    let said = run(tmp.path(), &cache, &["vault", "pull", "--vault", "default"])
        .ok()
        .said();
    assert!(said.contains("Nothing to pull"), "{said}");

    run(tmp.path(), &cache, &["vault", "pull", "--vault", "sources"]).ok();
    assert_eq!(
        run(tmp.path(), &cache, &["vault", "path", &digest])
            .ok()
            .code,
        0
    );
}

/// **The isolation warning.** Two vaults on the same account is legal — a corpus may own two
/// buckets — and it is also exactly what a half-finished setup looks like: somebody declared
/// `sources`, exported nothing for it, and it is quietly running on the account meant for
/// public output. The two are indistinguishable from outside, so `doctor` reports the shape.
///
/// Behind `vault-s3` because a build without the transport has no credentials to compare, and
/// would report a clean bill for a setup this exists to question.
#[cfg(feature = "vault-s3")]
#[test]
fn doctor_says_when_two_vaults_are_running_on_one_account() {
    let config = "[vault.default]\nurl = \"s3://public/yidam\"\naudience = \"anyone\"\n\
                  holds = [\"index\"]\n\
                  [vault.sources]\nurl = \"s3://licensed/yidam\"\naudience = \"the sangha\"\n\
                  holds = [\"catalog\"]\n";
    let tmp = repo(Some(config));
    let cache = tmp.path().join("cache");

    let doctor = |pairs: &[(&str, &str)]| -> String {
        let mut c = Command::new(env!("CARGO_BIN_EXE_yidam"));
        c.current_dir(tmp.path())
            .env("YIDAM_VAULT_CACHE", &cache)
            .arg("doctor");
        for (k, v) in pairs {
            c.env(k, v);
        }
        let out = c.output().expect("running yidam doctor");
        format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        )
    };

    let shared = doctor(&[
        ("YIDAM_VAULT_DEFAULT_ACCESS_KEY_ID", "AKIA_ONE"),
        ("YIDAM_VAULT_DEFAULT_SECRET_ACCESS_KEY", "s"),
        ("YIDAM_VAULT_SOURCES_ACCESS_KEY_ID", "AKIA_ONE"),
        ("YIDAM_VAULT_SOURCES_SECRET_ACCESS_KEY", "s"),
    ]);
    assert!(
        shared.contains("same credentials"),
        "one account, two vaults: {shared}"
    );
    assert!(
        shared.contains("`default`") && shared.contains("`sources`"),
        "names both: {shared}"
    );
    // The key id identifies the account and is never printed — a report gets pasted into
    // issues, and naming it there helps nobody.
    assert!(!shared.contains("AKIA_ONE"), "must not print it: {shared}");

    let separate = doctor(&[
        ("YIDAM_VAULT_DEFAULT_ACCESS_KEY_ID", "AKIA_ONE"),
        ("YIDAM_VAULT_DEFAULT_SECRET_ACCESS_KEY", "s"),
        ("YIDAM_VAULT_SOURCES_ACCESS_KEY_ID", "AKIA_TWO"),
        ("YIDAM_VAULT_SOURCES_SECRET_ACCESS_KEY", "s"),
    ]);
    assert!(
        !separate.contains("same credentials"),
        "properly isolated, so nothing to say: {separate}"
    );
}
