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

use std::fmt::Write as _;
use std::process::Command;

/// `css` with every `/* … */` removed, so a rule named in prose is not read as a rule.
fn without_comments(css: &str) -> String {
    let mut out = String::new();
    let mut rest = css;
    while let Some(open) = rest.find("/*") {
        out.push_str(&rest[..open]);
        rest = rest[open + 2..]
            .split_once("*/")
            .map_or("", |(_, after)| after);
    }
    out.push_str(rest);
    out
}

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
        let _ = write!(bundle, "\n/* {rel} */\n");
        // Every `@import` is dropped, because a concatenation has no use for one. The
        // relative ones (`styles.css` is nothing else) name files already inlined above, and
        // would resolve against the exported page's `assets/` if a browser honoured them. The
        // remote one is `tokens/fonts.css` asking Google for the four families — a request to
        // a third party from a page that is opened over `file://`, made before anything on it
        // is clicked and named in no config (#1680). Without it the page renders in the
        // families the reader has installed, or the fallbacks `typography.css` declares.
        let mut omitted = 0;
        for line in css.lines() {
            if line.trim_start().starts_with("@import") {
                omitted += 1;
            } else {
                bundle.push_str(line);
                bundle.push('\n');
            }
        }
        // Said in the output, because the file's own header may still describe the rule that
        // is no longer under it — `fonts.css` opens by saying the families load from a CDN.
        if omitted > 0 {
            let _ = writeln!(
                bundle,
                "/* {omitted} import rule(s) omitted from this bundle: an exported page \
                 requests nothing. */"
            );
        }
    }

    // A bundle that resolved to nothing would still compile and still export, producing an
    // unstyled page nobody would notice until they opened one.
    assert!(
        bundle.contains("--ink-900") && bundle.contains("--verified-bg"),
        "the design bundle is missing tokens the web agent uses; the manifest's \
         globalCssPaths no longer name the palette"
    );

    // The filter above reads lines, and CSS is not line-oriented: an `@import` that opens
    // mid-line, or a `url()` naming a host, would put the request back with nothing here
    // changing. So the result is checked as rules, comments aside.
    let rules = without_comments(&bundle);
    assert!(
        !rules.contains("@import"),
        "the design bundle still carries an @import, so the exported page would make a \
         request its reader was not asked about (#1680)"
    );
    for (at, _) in rules.match_indices("url(") {
        let target = rules[at + 4..].trim_start_matches([' ', '\'', '"']);
        assert!(
            !["http:", "https:", "//"]
                .iter()
                .any(|scheme| target.starts_with(scheme)),
            "the design bundle names a remote resource, and the exported page is opened \
             offline (#1680): url({}",
            target.chars().take(60).collect::<String>()
        );
    }

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
