//! The shipped CLI reads the receipt this one writes (#475, amended 2026-09-30).
//!
//! `receipt::FORMAT_VERSION` 2 added four optional fields — what produced the output — and the
//! claim that makes that bump additive is about a binary this crate is not: the last release,
//! already installed in every derived corpus, reading a receipt the next one commits. A unit test
//! over this crate's own `Landed` would only show that *today's* reader skips the fields, which is
//! the reader that was written knowing about them.
//!
//! So this runs the released binary. `mise run receipt-compat` downloads it for the host, checks
//! it against the release's `SHA256SUMS`, and hands it here as `YIDAM_PREVIOUS_BIN`; CI runs that
//! task as its own job. The test is `#[ignore]` so that `cargo test` on a laptop does not reach the
//! network, and it **fails** rather than skips when run without the binary: an ignored test that
//! passed with nothing to run would be the job going green having compared nothing.
//!
//! Both of the released binary's receipt readers are exercised, and each is held to an answer
//! only a successful parse can give. `run --dry-run` reads through `Receipt::committed_state`,
//! and a receipt it cannot parse is *stale*, not an error — so "exit 0" would pass a reader that
//! failed. The assertion is **fresh**. `doctor` reads through `Receipt::landed`, and is held to
//! the same: no step reported as computed from something else.

mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

use common::Example;

/// The four fields v2 added, written into every committed receipt whether or not the step that
/// wrote it had them — a shell step records none — so the reader is shown all of them.
const V2_FIELDS: &str = "model: an-elector-model\n\
version: 0.18.0\n\
config: 0000000000000000000000000000000000000000000000000000000000000000\n\
image_digest: sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef\n";

fn previous() -> PathBuf {
    let bin = std::env::var_os("YIDAM_PREVIOUS_BIN").unwrap_or_else(|| {
        panic!(
            "YIDAM_PREVIOUS_BIN is not set. Run `mise run receipt-compat`, which downloads the \
             last released CLI and sets it; this test does not skip, because a compatibility \
             check that compared nothing would pass"
        )
    });
    let bin = PathBuf::from(bin);
    assert!(bin.is_file(), "{} is not a file", bin.display());
    bin
}

fn git(root: &Path, args: &[&str]) -> String {
    let o = Command::new("git")
        .current_dir(root)
        .args(args)
        .output()
        .unwrap();
    assert!(
        o.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&o.stderr)
    );
    String::from_utf8_lossy(&o.stdout).to_string()
}

fn run(bin: &Path, root: &Path, args: &[&str]) -> (String, String, i32) {
    let o = Command::new(bin)
        .current_dir(root)
        .args(args)
        .output()
        .unwrap();
    (
        String::from_utf8_lossy(&o.stdout).to_string(),
        String::from_utf8_lossy(&o.stderr).to_string(),
        o.status.code().unwrap_or(-1),
    )
}

/// The released binary's dry-run verdict for every step, by name.
fn verdicts(bin: &Path, root: &Path) -> Vec<(String, String)> {
    let (out, err, code) = run(bin, root, &["run", "--dry-run", "--format", "json"]);
    assert_eq!(code, 0, "{out}{err}");
    let v: serde_json::Value =
        serde_json::from_str(&out).unwrap_or_else(|x| panic!("not JSON: {x}\n{out}{err}"));
    v["steps"]
        .as_array()
        .expect("a plan")
        .iter()
        .map(|s| {
            (
                s["step"].as_str().unwrap().to_string(),
                s["freshness"].as_str().unwrap_or_default().to_string(),
            )
        })
        .collect()
}

#[test]
#[ignore = "runs the released CLI: `mise run receipt-compat` downloads it"]
fn the_released_cli_reads_a_v2_receipt_as_the_answer_it_records() {
    let prev = previous();
    let (version, _, code) = run(&prev, Path::new("."), &["--version"]);
    assert_eq!(code, 0);
    if let Ok(want) = std::env::var("YIDAM_PREVIOUS_VERSION") {
        assert!(
            version.contains(&want),
            "YIDAM_PREVIOUS_BIN is {version:?}, not {want}"
        );
    }

    let e = Example::materialize_runnable("streamflow");
    let root = e.path();
    let (out, err, code) = e.run(&["run"]);
    assert_eq!(code, 0, "this build's run failed:\n{out}{err}");

    // Every receipt this build committed is v2, and gets every v2 field.
    let receipts: Vec<String> = git(&root, &["ls-tree", "--name-only", "HEAD", ".yidam/runs/"])
        .lines()
        .map(str::to_string)
        .collect();
    assert!(
        !receipts.is_empty(),
        "the run committed no receipt, so there is nothing for the released CLI to read"
    );
    git(
        &root,
        &[
            "restore",
            "--source=HEAD",
            "--worktree",
            "--staged",
            "--",
            ".yidam/",
        ],
    );
    for path in &receipts {
        let file = root.join(path);
        let text = std::fs::read_to_string(&file).unwrap();
        assert!(
            text.starts_with("format_version: 2\n"),
            "{path} is not a v2 receipt:\n{text}"
        );
        let mut kept: String = text
            .lines()
            .filter(|l| {
                !["model:", "version:", "config:", "image_digest:"]
                    .iter()
                    .any(|k| l.starts_with(k))
            })
            .map(|l| format!("{l}\n"))
            .collect();
        kept.push_str(V2_FIELDS);
        std::fs::write(&file, kept).unwrap();
    }
    git(&root, &["add", "-A", ".yidam/runs/"]);
    git(
        &root,
        &[
            "commit",
            "-q",
            "-m",
            "compute: every v2 field, for the released reader",
        ],
    );

    // `run --dry-run`: through `committed_state`. Fresh is the answer only a parse gives.
    let seen = verdicts(&prev, &root);
    assert!(!seen.is_empty());
    for (step, freshness) in &seen {
        assert_eq!(
            freshness, "fresh",
            "the released CLI reads `{step}`'s v2 receipt as {freshness}: {seen:?}"
        );
    }

    // `doctor`: through `Landed`. Its exit code is a gate over every finding, so the question is
    // asked of the text.
    let (out, err, _) = run(&prev, &root, &["doctor", "--format", "json"]);
    let v: serde_json::Value =
        serde_json::from_str(&out).unwrap_or_else(|x| panic!("not JSON: {x}\n{out}{err}"));
    let text = serde_json::to_string(&v).unwrap();
    assert!(
        !text.contains("is not what its receipt was computed from"),
        "the released `doctor` does not read the v2 receipt's input state:\n{out}"
    );

    // The control: the same binary, the same receipts, one input state wrong. A reader that had
    // failed to parse would say stale here too, and it must — otherwise `fresh` above could be
    // an answer this binary gives regardless.
    let first = root.join(&receipts[0]);
    let text = std::fs::read_to_string(&first).unwrap();
    let bent: String = text
        .lines()
        .map(|l| {
            if l.starts_with("input_state:") {
                format!("input_state: {}\n", "0".repeat(64))
            } else {
                format!("{l}\n")
            }
        })
        .collect();
    assert_ne!(bent, text);
    std::fs::write(&first, bent).unwrap();
    git(
        &root,
        &["commit", "-q", "-am", "compute: a wrong input state"],
    );
    let step = receipts[0]
        .trim_start_matches(".yidam/runs/")
        .trim_end_matches(".yml");
    let after = verdicts(&prev, &root);
    assert!(
        after.iter().any(|(s, f)| s == step && f != "fresh"),
        "the released CLI calls `{step}` fresh with an input state that is not its own, so \
         `fresh` above proved nothing: {after:?}"
    );
}
