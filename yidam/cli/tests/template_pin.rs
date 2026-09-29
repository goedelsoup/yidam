//! `.yidam.toml`'s `template` field names the template layer, and only that layer.
//!
//! Four layers release from one repository — `v*` for the template, `cli/v*`, `sdk/rust/v*`,
//! `bootstrap/v*`, `editor/v*` — and they routinely tag the same commit. `a7aaa5f` carries
//! three of them. So "is there a tag here?" is the wrong question and `git describe --tags
//! --exact-match` is exactly that question: at `a7aaa5f` it answers `editor/v0.1.0`, and
//! every repository cloned there recorded a VS Code extension's version as the version of its
//! template layer.
//!
//! Two places ask it — `provenance.rs` when `yidam clone` writes the manifest, and
//! `mise.yidam.toml`'s `yidam-vendor-update` when a derived repository re-pins itself. The
//! second is the one that lands in a derived repo and is the one nothing here would otherwise
//! compile, so it is discovered rather than named.

use std::path::PathBuf;

mod common;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

use common::git::git;

/// A commit carrying every layer's tag resolves to the template's, not to whichever git prefers.
///
/// The tags are created in an order that makes the wrong answer the tempting one: `v0.1.0`
/// first, so a resolver preferring the newest tag returns something else.
#[test]
fn a_commit_tagged_by_every_layer_still_names_the_template() {
    let dir = tempfile::tempdir().expect("tempdir");
    let p = dir.path();
    git(p, &["init", "-q", "-b", "main"]);
    git(p, &["config", "user.email", "t@yidam.test"]);
    git(p, &["config", "user.name", "T"]);
    git(p, &["config", "tag.gpgsign", "false"]);
    git(p, &["commit", "-q", "--allow-empty", "-m", "release"]);
    for tag in [
        "v0.1.0",
        "sdk/rust/v0.3.0",
        "bootstrap/v0.4.0",
        "cli/v0.5.0",
        "editor/v0.1.0",
    ] {
        git(p, &["tag", tag]);
    }

    let resolved = common::git::out(
        p,
        &[
            "describe",
            "--tags",
            "--exact-match",
            "--match",
            yidam::provenance::TEMPLATE_TAG_GLOB,
            "HEAD",
        ],
    );
    assert_eq!(
        resolved, "v0.1.0",
        "the template pin must answer with the template layer's tag, not with whichever \
         other layer shares the commit"
    );

    // And the unrestricted question — the one that shipped — gets it wrong here, so this
    // test is about the `--match` and not about `git describe` being obviously right.
    let naive = common::git::out(p, &["describe", "--tags", "--exact-match", "HEAD"]);
    assert_ne!(
        naive, "v0.1.0",
        "if the unrestricted form now answers correctly, this fixture no longer reproduces \
         the defect and must be rebuilt rather than deleted"
    );
}

/// Every place that resolves a tag for the `template` pin restricts to the template's glob.
///
/// Discovered, not listed. The CLI and `mise.yidam.toml` are the two today; a third would be
/// a third silent way to write a CLI version into a field that names the template.
#[test]
fn every_template_tag_lookup_is_restricted_to_the_template_layer() {
    let root = repo_root();
    let mut checked = 0;
    let mut unrestricted = Vec::new();

    let mut files: Vec<PathBuf> = vec![
        root.join("mise.yidam.toml"),
        root.join("mise.toml"),
        root.join("yidam/cli/src/provenance.rs"),
        root.join("sadhana/root/mise.toml"),
    ];
    for entry in walkdir::WalkDir::new(root.join(".github/workflows"))
        .into_iter()
        .filter_map(Result::ok)
    {
        if entry.file_type().is_file() {
            files.push(entry.path().to_path_buf());
        }
    }

    for f in files {
        let Ok(text) = std::fs::read_to_string(&f) else {
            continue;
        };
        // Comments stripped first: three of them *describe* the flag, and a scan that reads
        // prose as code reports the explanation as the defect. Then whitespace collapsed,
        // because the Rust call spans six lines and the shell one spans a single long one —
        // a line-oriented scan can only be right about one of them.
        let code: String = text
            .lines()
            .map(|l| {
                let l = l.trim_start();
                if l.starts_with("//") || l.starts_with('#') {
                    ""
                } else {
                    l
                }
            })
            .collect::<Vec<_>>()
            .join(" ");
        let code: String = code.split_whitespace().collect::<Vec<_>>().join(" ");

        for (i, _) in code.match_indices("--exact-match") {
            checked += 1;
            // `--match` is not a substring of `--exact-match`, so the window cannot match
            // the very flag it is checking.
            let lo = i.saturating_sub(240);
            let hi = (i + 240).min(code.len());
            if !code[lo..hi].contains("--match") {
                let snippet: String = code[lo..hi].chars().take(160).collect();
                unrestricted.push(format!("  {}\n      …{snippet}…", f.display()));
            }
        }
    }

    assert!(
        checked >= 2,
        "found {checked} exact-match tag lookup(s) — the scan is broken, not the repository"
    );
    assert!(
        unrestricted.is_empty(),
        "{} tag lookup(s) ask whether ANY layer tagged this commit:\n{}\n\nFour layers \
         release from one repository and tag the same commit; the answer is whichever git \
         prefers. Restrict with `--match`.",
        unrestricted.len(),
        unrestricted.join("\n")
    );
}

/// The keys of `[yidam]`, as the lines between that header and the next table or the end.
fn yidam_keys(manifest: &str) -> Vec<String> {
    let mut keys: Vec<String> = manifest
        .lines()
        .skip_while(|l| l.trim() != "[yidam]")
        .skip(1)
        .take_while(|l| !l.trim_start().starts_with('['))
        .filter_map(|l| l.split_once('='))
        .map(|(k, _)| k.trim().to_string())
        .filter(|k| !k.is_empty() && !k.starts_with('#'))
        .collect();
    keys.sort();
    keys
}

/// `yidam clone` and `yidam-vendor-update` write the same `[yidam]` table.
///
/// The vendor task overwrites `.yidam.toml` wholesale, so a field only the CLI writes is a
/// field the first re-vendor deletes. `cli` was added to both (#1076); this is what keeps
/// the next one from reaching only one.
#[test]
fn the_vendor_task_writes_every_key_clone_writes() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let clone_keys = yidam_keys(&yidam::provenance::Provenance::read(tmp.path()).render());
    assert!(
        clone_keys.iter().any(|k| k == "cli"),
        "the scan found no `cli` in the CLI's own manifest — the scan is broken: {clone_keys:?}"
    );

    let text =
        std::fs::read_to_string(repo_root().join("mise.yidam.toml")).expect("mise.yidam.toml");
    let tasks: toml::Value = toml::from_str(&text).expect("mise.yidam.toml parses");
    let run = tasks["yidam-vendor-update"]["run"]
        .as_str()
        .expect("yidam-vendor-update has a run script");
    let heredoc = run
        .split_once("cat > .yidam.toml <<EOF\n")
        .and_then(|(_, rest)| rest.split_once("\nEOF\n"))
        .map(|(body, _)| body)
        .expect("yidam-vendor-update writes .yidam.toml from a heredoc");

    assert_eq!(
        yidam_keys(heredoc),
        clone_keys,
        "`yidam-vendor-update` and `yidam clone` write different `[yidam]` keys; a re-vendor \
         would drop or invent a field"
    );
}
