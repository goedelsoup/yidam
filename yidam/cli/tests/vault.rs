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

/// **The load-bearing refusal.** Two declared vaults must not resolve to the first one
/// silently, and the message has to name both — a refusal that does not say which two
/// stores are in play leaves the reader to guess which one was about to be written to.
#[test]
fn two_vaults_are_refused_by_name_rather_than_resolved_to_the_first() {
    let a = tempfile::tempdir().unwrap();
    let b = tempfile::tempdir().unwrap();
    let config = format!(
        "[vault.default]\nurl = \"file://{}\"\naudience = \"everyone\"\n\
         [vault.sources]\nurl = \"file://{}\"\naudience = \"the sangha\"\n",
        a.path().display(),
        b.path().display()
    );
    let tmp = repo(Some(&config));
    let cache = tmp.path().join("cache");

    let said = run(tmp.path(), &cache, &["vault", "list"]).failed().said();
    assert!(said.contains("`default`"), "names the first: {said}");
    assert!(said.contains("`sources`"), "names the second: {said}");
    assert!(
        said.contains("RFC-0023"),
        "says where the design is: {said}"
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

/// An `s3://` url must be refused as *not built yet*, not as gibberish — the two send the
/// reader to different places.
#[test]
fn an_s3_vault_is_reported_as_unbuilt_rather_than_unknown() {
    let config = "[vault.default]\nurl = \"s3://bucket/prefix\"\naudience = \"the sangha\"\n";
    let tmp = repo(Some(config));
    let cache = tmp.path().join("cache");
    // `list` reports it rather than failing: the declaration is legitimate and it is this
    // build that cannot act on it.
    let said = run(tmp.path(), &cache, &["vault", "list"]).ok().said();
    assert!(said.contains("no S3 transport"), "{said}");

    // A command that actually needs the store does fail.
    run(tmp.path(), &cache, &["vault", "get", &"3".repeat(64)]).failed();
}
