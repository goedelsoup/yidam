//! `yidam index-verify` driven end to end, with a real provider process.
//!
//! The provider contract is stdin/stdout precisely so the consumer this exists for — a
//! Python process holding an index directory and its own embedder — can satisfy it in three
//! lines. So the tests satisfy it in three lines too, rather than calling a Rust function
//! and asserting the wiring works by assumption.

use std::path::Path;
use std::process::Command;

fn run(dir: &Path, args: &[&str]) -> (String, i32) {
    let out = Command::new(env!("CARGO_BIN_EXE_yidam"))
        .current_dir(dir)
        .args(args)
        .output()
        .unwrap();
    (
        String::from_utf8_lossy(&out.stdout).to_string(),
        out.status.code().unwrap_or(-1),
    )
}

/// An index directory carrying a witness, and nothing else.
///
/// No vectors, no `meta.json`: `index-verify` reads the contract and asks a provider about
/// it, which is the whole point — a consumer must be able to check itself *before* querying,
/// and against an index it did not build.
fn stage(prefix: &[f32], with_witness: bool) -> tempfile::TempDir {
    let tmp = tempfile::tempdir().unwrap();
    let index = tmp.path().join("index");
    std::fs::create_dir_all(&index).unwrap();

    let mut config = serde_json::json!({
        "format_version": "1",
        "model_id": "Xenova/all-MiniLM-L6-v2",
        "embedding_dim": prefix.len(),
        "model_file": "onnx/model_quantized.onnx",
        "pooling": "mean",
        "normalize": true,
        "fastembed_model_enum": "AllMiniLML6V2Q"
    });
    if with_witness {
        config["verification"] = serde_json::json!({
            "probe": "knowledge graph traversal",
            "prefix_dims": prefix.len(),
            "prefix": prefix,
            "tolerance": 1e-5,
            "known_delta": { "sentence-transformers": { "tolerance": 1e-2 } }
        });
    }
    std::fs::write(
        index.join("embed.config.json"),
        serde_json::to_string_pretty(&config).unwrap(),
    )
    .unwrap();
    tmp
}

/// A provider script: reads the probe on stdin, writes a JSON array on stdout.
fn provider(dir: &Path, name: &str, vector: &[f32]) -> String {
    let path = dir.join(name);
    std::fs::write(
        &path,
        format!(
            "#!/bin/sh\ncat >/dev/null\necho '{}'\n",
            serde_json::to_string(vector).unwrap()
        ),
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    path.to_string_lossy().to_string()
}

fn unit4() -> Vec<f32> {
    vec![0.5, 0.5, 0.5, 0.5]
}

/// Nudge one element and re-normalize: a vector from a different space, not a longer one.
fn drifted(v: &[f32], nudge: f32) -> Vec<f32> {
    let mut out = v.to_vec();
    out[0] += nudge;
    let n = out.iter().map(|x| x * x).sum::<f32>().sqrt();
    out.iter().map(|x| x / n).collect()
}

fn index_arg(tmp: &Path) -> String {
    tmp.join("index").to_string_lossy().to_string()
}

#[test]
fn a_matching_provider_passes() {
    let tmp = stage(&unit4(), true);
    let cmd = provider(tmp.path(), "same.sh", &unit4());
    let (out, code) = run(
        tmp.path(),
        &[
            "index-verify",
            "--index",
            &index_arg(tmp.path()),
            "--provider",
            &cmd,
        ],
    );
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("same vector space"), "{out}");
}

/// The case this feature exists for.
///
/// fp32 where the index is quantized: retrieval still works, cosine scores still look
/// plausible, and the answer comes from a different vector space. It passes — and it says so
/// by name, which is the entire difference between an expected degradation and one nobody
/// can see.
#[test]
fn the_fp32_consumer_is_told_it_is_in_a_different_space() {
    let tmp = stage(&unit4(), true);
    let cmd = provider(tmp.path(), "fp32.sh", &drifted(&unit4(), 5e-3));
    let (out, code) = run(
        tmp.path(),
        &[
            "index-verify",
            "--index",
            &index_arg(tmp.path()),
            "--provider",
            &cmd,
            "--runtime",
            "sentence-transformers",
        ],
    );
    assert_eq!(code, 0, "a declared drift is not a failure: {out}");
    assert!(out.contains("DIFFERENT vector space"), "{out}");
    assert!(out.contains("sentence-transformers"), "{out}");
}

/// The same provider, not naming itself, is held to the strict bound.
///
/// A `known_delta` is a runtime declaring its drift in advance. Handing it to whoever turns
/// up would make the block a discount rather than a declaration.
#[test]
fn the_same_drift_without_a_runtime_name_fails() {
    let tmp = stage(&unit4(), true);
    let cmd = provider(tmp.path(), "anon.sh", &drifted(&unit4(), 5e-3));
    let (out, code) = run(
        tmp.path(),
        &[
            "index-verify",
            "--index",
            &index_arg(tmp.path()),
            "--provider",
            &cmd,
        ],
    );
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("degrade to keyword search"), "{out}");
}

/// Every index built before this field existed. Reported, and not failed.
#[test]
fn an_index_without_a_witness_is_unverifiable_and_exits_zero() {
    let tmp = stage(&unit4(), false);
    let cmd = provider(tmp.path(), "any.sh", &unit4());
    let (out, code) = run(
        tmp.path(),
        &[
            "index-verify",
            "--index",
            &index_arg(tmp.path()),
            "--provider",
            &cmd,
        ],
    );
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("cannot be checked"), "{out}");
}

/// The verdict must not depend on how the answer was asked for.
#[test]
fn exit_codes_are_identical_across_formats() {
    let tmp = stage(&unit4(), true);
    let cmd = provider(tmp.path(), "bad.sh", &[1.0, 0.0, 0.0, 0.0]);
    let index = index_arg(tmp.path());
    let (_, text) = run(
        tmp.path(),
        &["index-verify", "--index", &index, "--provider", &cmd],
    );
    let (json, code) = run(
        tmp.path(),
        &[
            "index-verify",
            "--index",
            &index,
            "--provider",
            &cmd,
            "--format",
            "json",
        ],
    );
    assert_eq!(text, code);
    assert_eq!(code, 1);
    let doc: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(doc["verdict"], "mismatch");
    assert_eq!(doc["passed"], false);
    assert_eq!(doc["format_version"], "1");
}

/// A provider that writes something other than a vector is a usage error, not a mismatch.
///
/// Reporting it as a mismatch would tell a consumer its embeddings are wrong when what is
/// wrong is its script.
#[test]
fn a_provider_that_writes_nonsense_is_an_error_not_a_verdict() {
    let tmp = stage(&unit4(), true);
    let path = tmp.path().join("broken.sh");
    std::fs::write(&path, "#!/bin/sh\ncat >/dev/null\necho 'not a vector'\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    let out = Command::new(env!("CARGO_BIN_EXE_yidam"))
        .current_dir(tmp.path())
        .args([
            "index-verify",
            "--index",
            &index_arg(tmp.path()),
            "--provider",
            &path.to_string_lossy(),
        ])
        .output()
        .unwrap();
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("JSON array of floats"), "{stderr}");
}

/// Without a provider it reports what the index declares and stops.
#[test]
fn no_provider_reports_the_contract_and_asks_for_one() {
    let tmp = stage(&unit4(), true);
    let (out, code) = run(
        tmp.path(),
        &["index-verify", "--index", &index_arg(tmp.path())],
    );
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("--provider"), "{out}");
}
