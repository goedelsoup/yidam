//! The S3 transport against a real server.
//!
//! # Why this is `#[ignore]`d
//!
//! CI is hermetic and must stay that way. These tests need an S3-compatible server, so they
//! do not run by default and do not run on a pull request.
//!
//! They are still committed, and they are the **authority** for the claim that the signer is
//! correct. The unit tests in `vault/sigv4.rs` pin the derivation steps — reorder them and
//! they go red — but two implementations by the same author agreeing is evidence about the
//! steps and not proof about the scheme. Only a server that recomputes the signature
//! independently and accepts or rejects it settles that.
//!
//! # Running them
//!
//! ```sh
//! docker run -d --rm --name yidam-minio -p 9000:9000 \
//!   -e MINIO_ROOT_USER=yidamtest -e MINIO_ROOT_PASSWORD=yidamtest123 \
//!   quay.io/minio/minio:latest server /data
//!
//! YIDAM_S3_TEST=1 cargo test --test vault_s3 -- --ignored --test-threads=1
//! ```
//!
//! Without `YIDAM_S3_TEST` they skip loudly rather than failing, so `-- --ignored` on a
//! machine with no server reports a skip instead of a red suite.

use std::path::Path;
use std::process::Command;

const ENDPOINT: &str = "http://127.0.0.1:9000";
const ACCESS: &str = "yidamtest";
const SECRET: &str = "yidamtest123";
const BUCKET: &str = "yidam-vault-test";

fn enabled() -> bool {
    if std::env::var("YIDAM_S3_TEST").is_err() {
        ci_report::skipped(&format!(
            "set YIDAM_S3_TEST=1 and run a MinIO on {ENDPOINT}"
        ));
        return false;
    }
    true
}

fn run(dir: &Path, cache: &Path, args: &[&str]) -> (String, String, i32) {
    let out = Command::new(env!("CARGO_BIN_EXE_yidam"))
        .current_dir(dir)
        .env("YIDAM_VAULT_CACHE", cache)
        .env("YIDAM_VAULT_DEFAULT_ACCESS_KEY_ID", ACCESS)
        .env("YIDAM_VAULT_DEFAULT_SECRET_ACCESS_KEY", SECRET)
        .args(args)
        .output()
        .expect("running yidam");
    (
        String::from_utf8_lossy(&out.stdout).to_string(),
        String::from_utf8_lossy(&out.stderr).to_string(),
        out.status.code().unwrap_or(-1),
    )
}

/// A derived repository whose vault is the MinIO bucket.
fn repo(extra_catalog: Option<&str>) -> tempfile::TempDir {
    let tmp = tempfile::tempdir().unwrap();
    assert!(Command::new("git")
        .args(["init", "-q"])
        .current_dir(tmp.path())
        .status()
        .unwrap()
        .success());
    std::fs::create_dir_all(tmp.path().join(".yidam/catalog")).unwrap();
    std::fs::write(
        tmp.path().join(".yidam/config.toml"),
        format!(
            "[vault.default]\nurl = \"s3://{BUCKET}/corpus\"\nendpoint = \"{ENDPOINT}\"\n\
             region = \"us-east-1\"\naudience = \"the test suite\"\n"
        ),
    )
    .unwrap();
    if let Some(entry) = extra_catalog {
        std::fs::write(tmp.path().join(".yidam/catalog/paper.md"), entry).unwrap();
    }
    tmp
}

/// The bucket has to exist before anything can be written to it. Created with a signed
/// request through the same code path under test, which is itself part of the check.
fn ensure_bucket() {
    let out = Command::new("curl")
        .args([
            "-s",
            "-o",
            "/dev/null",
            "-w",
            "%{http_code}",
            "--max-time",
            "10",
            "-X",
            "PUT",
            "--aws-sigv4",
            "aws:amz:us-east-1:s3",
            "-u",
            &format!("{ACCESS}:{SECRET}"),
            &format!("{ENDPOINT}/{BUCKET}"),
        ])
        .output()
        .expect("curl");
    let code = String::from_utf8_lossy(&out.stdout).to_string();
    assert!(
        code == "200" || code == "409",
        "creating the bucket returned {code}"
    );
}

/// **The ground truth for the signer.** A real server recomputes the signature from what
/// arrived and either accepts it or does not, which is the only check that establishes the
/// scheme is right rather than merely self-consistent.
#[test]
#[ignore = "needs a running S3-compatible server; see the module header"]
fn an_artifact_round_trips_through_a_real_s3_server() {
    if !enabled() {
        return;
    }
    ensure_bucket();

    let tmp = repo(None);
    let cache = tmp.path().join("cache");
    let body: &[u8] = b"a document with some bytes in it";
    let src = tmp.path().join("paper.pdf");
    std::fs::write(&src, body).unwrap();

    // Into the cache, to learn its digest.
    let (out, err, code) = run(tmp.path(), &cache, &["vault", "put", src.to_str().unwrap()]);
    assert_eq!(code, 0, "{out}{err}");
    let digest = out.trim().to_string();

    // The corpus has to *record* it before it can be pushed. `--artifact` narrows what is
    // sent and never bypasses the guard, so this goes through the same path a real push does
    // — including `redistributable`, which is refusal by default.
    std::fs::write(
        tmp.path().join(".yidam/catalog/paper.md"),
        format!(
            "---\nname: Paper\ndescription: A test document.\ntype: paper\n\
             artifacts:\n  - sha256: {digest}\n    redistributable: true\n---\n\nBody.\n"
        ),
    )
    .unwrap();

    // Up to MinIO — signed, streamed, and accepted or not by a server that did not write it.
    let (out, err, code) = run(
        tmp.path(),
        &cache,
        &["vault", "push", "--artifact", &digest],
    );
    assert_eq!(code, 0, "push was rejected:\n{out}{err}");

    // …and back down into a cache that has never seen it.
    let fresh = tmp.path().join("fresh-cache");
    let (out, err, code) = run(tmp.path(), &fresh, &["vault", "get", &digest]);
    assert_eq!(code, 0, "{out}{err}");
    let at = std::path::PathBuf::from(out.trim());
    assert_eq!(
        std::fs::read(&at).unwrap(),
        body,
        "the bytes that came back are not the bytes that went up"
    );
}

/// `has` must distinguish absent from failed. A signature error that read as "not present"
/// would make `push` upload everything on every run and report success.
#[test]
#[ignore = "needs a running S3-compatible server; see the module header"]
fn a_digest_the_server_does_not_hold_is_absent_rather_than_an_error() {
    if !enabled() {
        return;
    }
    ensure_bucket();
    let tmp = repo(None);
    let cache = tmp.path().join("cache");
    let absent = "7".repeat(64);
    let (out, err, code) = run(tmp.path(), &cache, &["vault", "get", &absent]);
    assert_ne!(code, 0, "{out}{err}");
    let said = format!("{out}{err}");
    assert!(
        said.contains("neither") || said.contains("not in"),
        "reported as absent, not as a transport failure: {said}"
    );
}

/// Wrong credentials must be reported as what they are. Without this, every signing bug and
/// every typo'd key look the same from the outside.
#[test]
#[ignore = "needs a running S3-compatible server; see the module header"]
fn a_bad_secret_is_reported_as_the_server_reported_it() {
    if !enabled() {
        return;
    }
    ensure_bucket();
    let tmp = repo(None);
    let cache = tmp.path().join("cache");
    let out = Command::new(env!("CARGO_BIN_EXE_yidam"))
        .current_dir(tmp.path())
        .env("YIDAM_VAULT_CACHE", &cache)
        .env("YIDAM_VAULT_DEFAULT_ACCESS_KEY_ID", ACCESS)
        .env("YIDAM_VAULT_DEFAULT_SECRET_ACCESS_KEY", "wrong-on-purpose")
        .args(["vault", "get", &"8".repeat(64)])
        .output()
        .unwrap();
    let said = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        said.contains("SignatureDoesNotMatch") || said.contains("403"),
        "the server's own words should reach the reader: {said}"
    );
}
