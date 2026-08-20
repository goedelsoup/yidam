//! Captures the commit this binary was built from, for the report handshake.
//!
//! A consumer of `--format json` is versioned independently of the binary a given
//! repository pins in `.yidam.toml`. `format_version` tells it whether it can parse the
//! envelope; this tells it *which* yidam produced the answer, which is what makes a bug
//! report actionable and a skew diagnosable.
//!
//! Best-effort by construction. A build from a tarball, a vendored crate, or a dirty tree
//! has no commit to report, and `unknown` is the honest answer — the field is never
//! guessed and never omitted, so a consumer can distinguish "not recorded" from "absent".

use std::process::Command;

fn main() {
    // Only re-run when HEAD moves, not on every source edit.
    println!("cargo:rerun-if-changed=../../.git/HEAD");
    println!("cargo:rerun-if-env-changed=YIDAM_BUILD_COMMIT");

    let commit = std::env::var("YIDAM_BUILD_COMMIT")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .or_else(|| {
            Command::new("git")
                .args(["rev-parse", "--short", "HEAD"])
                .output()
                .ok()
                .filter(|o| o.status.success())
                .and_then(|o| String::from_utf8(o.stdout).ok())
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
        })
        .unwrap_or_else(|| "unknown".to_string());

    println!("cargo:rustc-env=YIDAM_BUILD_COMMIT={commit}");
}
