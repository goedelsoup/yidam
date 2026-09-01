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

/// The design system, concatenated into one string the binary can embed.
///
/// `export --format web` writes a self-contained artifact — it is opened over `file://` and
/// must carry its own styling — so the CLI cannot `@import` the token files at runtime the
/// way the docs site does. Before #465 that constraint was met by retyping a subset of the
/// palette into `assets/web/main.css`, where ten of twenty-four values drifted and two
/// evidence-tag colour families ended up transposed.
///
/// So the concatenation happens here instead, and **the order comes from
/// `_ds_manifest.json`** rather than from a list in this file. The manifest has declared it
/// since the system was synced; this is the first thing to read it. Order is load-bearing:
/// `semantic.css` resolves `var(--ink-…)` from the files above it, so a rearrangement
/// produces a stylesheet whose variables silently resolve to nothing.
fn emit_design_tokens() {
    // Through the crate-root symlink (`yidam/cli/design -> ../design`), not `../design`
    // directly. `cargo package` copies only what lives under the crate root, so a build
    // script reading past it produces a tarball whose build fails for everyone installing
    // from crates.io while every CI job here stays green — `packaging.rs` caught exactly
    // that when this was written, and its module header records the two releases that
    // shipped one command away from the same thing.
    let design = std::path::Path::new("design");
    let manifest_path = design.join("_ds_manifest.json");
    println!("cargo:rerun-if-changed={}", manifest_path.display());

    let manifest = std::fs::read_to_string(&manifest_path)
        .unwrap_or_else(|e| panic!("{} is unreadable ({e})", manifest_path.display()));
    let value: serde_json::Value = serde_json::from_str(&manifest)
        .unwrap_or_else(|e| panic!("{} does not parse as JSON ({e})", manifest_path.display()));
    let paths = value
        .get("globalCssPaths")
        .and_then(|p| p.as_array())
        .unwrap_or_else(|| panic!("{} declares no globalCssPaths", manifest_path.display()));

    let mut bundle = String::from(
        "/* Generated at build time from yidam/design/_ds_manifest.json — do not edit. */\n",
    );
    for entry in paths {
        let rel = entry.as_str().expect("globalCssPaths holds strings");
        let path = design.join(rel);
        println!("cargo:rerun-if-changed={}", path.display());
        let css = std::fs::read_to_string(&path).unwrap_or_else(|e| {
            panic!(
                "{} is named by the manifest and unreadable ({e})",
                path.display()
            )
        });
        bundle.push_str(&format!("\n/* {rel} */\n"));
        bundle.push_str(&css);
    }

    // A bundle that resolved to nothing would still compile and still export, producing an
    // unstyled page nobody would notice until they opened one.
    assert!(
        bundle.contains("--ink-900") && bundle.contains("--verified-bg"),
        "the design bundle is missing tokens the web agent uses; the manifest's \
         globalCssPaths no longer name the palette"
    );

    let out = std::path::Path::new(&std::env::var("OUT_DIR").expect("cargo sets OUT_DIR"))
        .join("design-tokens.css");
    std::fs::write(&out, bundle).expect("writing the design bundle");
}

fn main() {
    emit_design_tokens();

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
