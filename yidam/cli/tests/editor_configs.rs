//! The editor guides name commands. This checks the binary still has them.
//!
//! `yidam/editors/README.md` is the only documentation for `serve --lsp` and, in this
//! repository, its only consumer: the VS Code extension polls `--format json` instead, and
//! the README says so. So a renamed subcommand or flag breaks every Neovim and Helix user
//! and nothing here notices. `broken-prose-link` walks `.yidam/**` and `docs/**`, not
//! `yidam/editors/`, and the LSP's own tests drive `run()` in memory rather than the
//! documented command line.
//!
//! **Lexical on purpose.** `installed_layout_links.rs` is the model: a documented string
//! checked against reality catches the whole typo class, needs no editor, and runs anywhere.
//! Spawning the server to re-cover a message loop that is already covered in memory would
//! buy one thing — that the wiring is real — at the cost of a slow, platform-sensitive test.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Command;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

fn guide() -> PathBuf {
    repo_root().join("yidam/editors/README.md")
}

fn read_guide() -> String {
    std::fs::read_to_string(guide()).expect("yidam/editors/README.md")
}

/// `--help` for a subcommand, or for the binary itself when `args` is empty.
fn help(args: &[&str]) -> String {
    let out = Command::new(env!("CARGO_BIN_EXE_yidam"))
        .args(args)
        .arg("--help")
        .output()
        .expect("running yidam --help");
    // clap writes long help to stdout and errors to stderr; take both so an unknown
    // subcommand surfaces as an empty command list rather than as a silent pass.
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

/// Every subcommand `yidam --help` lists.
///
/// The listing is `help::render`'s, not clap's flat `Commands:` block: group headings sit
/// flush left, command rows are indented two spaces, and the trailing legend's continuation
/// line is indented four. Anything between the usage line and `Options:` that is indented
/// exactly two spaces is a command row.
fn subcommands() -> HashSet<String> {
    let text = help(&[]);
    let body = text
        .split_once("Usage:")
        .expect("top-level help carries a usage line")
        .1;
    body.lines()
        .take_while(|l| !l.starts_with("Options:"))
        .filter(|l| l.starts_with("  ") && !l.starts_with("   "))
        .filter_map(|l| l.split_whitespace().next())
        // `-` opens an option, `*` opens the write-marker legend.
        .filter(|w| !w.starts_with('-') && *w != "*")
        .map(str::to_string)
        .collect()
}

/// Inline code spans outside fenced blocks — where this guide names its commands.
fn prose_spans(text: &str) -> Vec<String> {
    let mut spans = Vec::new();
    let mut fence = false;
    for line in text.lines() {
        if line.trim_start().starts_with("```") {
            fence = !fence;
            continue;
        }
        if fence {
            continue;
        }
        let mut parts = line.split('`');
        parts.next();
        while let Some(inner) = parts.next() {
            spans.push(inner.to_string());
            parts.next();
        }
    }
    spans
}

/// Fenced blocks, as raw text.
fn fences(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current: Option<String> = None;
    for line in text.lines() {
        if line.trim_start().starts_with("```") {
            match current.take() {
                Some(body) => out.push(body),
                None => current = Some(String::new()),
            }
            continue;
        }
        if let Some(body) = current.as_mut() {
            body.push_str(line);
            body.push('\n');
        }
    }
    out
}

/// Check one documented invocation against the binary. `argv` excludes the program name.
fn assert_accepted(argv: &[&str], where_: &str) {
    let (sub, rest) = argv
        .split_first()
        .expect("an invocation names a subcommand");
    assert!(
        subcommands().contains(*sub),
        "the guide names {where_}, and this binary has no `{sub}` subcommand"
    );
    let sub_help = help(&[sub]);
    for flag in rest.iter().filter(|t| t.starts_with("--")) {
        assert!(
            sub_help.contains(flag),
            "the guide names {where_}, and `yidam {sub} --help` does not list `{flag}`"
        );
    }
}

/// Every command the guide names in prose is one this binary accepts.
///
/// A span counts as an invocation when it starts with `yidam`, or when its first word is a
/// subcommand this binary has — which covers `serve --mcp`, written without the program
/// name. The second arm cannot catch a subcommand renamed *away*, since the span then names
/// nothing and is skipped; the first arm can, and does, because the guide also spells
/// `yidam serve --lsp` in full.
#[test]
fn every_command_the_guide_names_is_one_the_binary_accepts() {
    let text = read_guide();
    let subs = subcommands();
    let mut checked = 0usize;

    for span in prose_spans(&text) {
        let tokens: Vec<&str> = span.split_whitespace().collect();
        let argv: Vec<&str> = match tokens.split_first() {
            Some((&"yidam", rest)) if !rest.is_empty() => rest.to_vec(),
            Some((first, _)) if subs.contains(*first) => tokens.clone(),
            _ => continue,
        };
        assert_accepted(&argv, &format!("`{span}`"));
        checked += 1;
    }

    // A sweep that matches nothing passes, which is the one way this test could rot into
    // decoration. The guide names at least `serve --lsp`, `serve --mcp`, `lint`, `rename`
    // and `schema`.
    assert!(
        checked >= 5,
        "only {checked} invocation(s) recognised — the extractor stopped seeing this guide"
    );
}

/// The snippets a reader pastes must invoke what the prose documents.
///
/// Both configurations pass argv as quoted tokens — a Lua table and a TOML array — so the
/// check is that each token is present as a quoted string, which is true of both forms and
/// of any third editor that spells an argv the same way.
#[test]
fn the_editor_snippets_invoke_what_the_prose_documents() {
    let text = read_guide();
    let configs: Vec<String> = fences(&text)
        .into_iter()
        .filter(|f| f.contains("'yidam'") || f.contains("\"yidam\""))
        .collect();
    assert_eq!(
        configs.len(),
        2,
        "expected the Neovim and Helix configurations; found {}",
        configs.len()
    );
    for fence in &configs {
        for token in ["serve", "--lsp"] {
            assert!(
                fence.contains(&format!("'{token}'")) || fence.contains(&format!("\"{token}\"")),
                "an editor configuration does not pass {token:?}:\n{fence}"
            );
        }
    }
}

/// Every repo-relative link in the guide resolves from the guide's own directory.
///
/// The lint's prose-link check does not reach here, and these links point at an RFC and at
/// the extension's own README — both of which move.
#[test]
fn every_link_in_the_guide_resolves() {
    let text = read_guide();
    let dir = guide().parent().unwrap().to_path_buf();
    let mut checked = 0usize;
    for (i, line) in text.lines().enumerate() {
        let mut rest = line;
        while let Some(at) = rest.find("](") {
            rest = &rest[at + 2..];
            let Some(close) = rest.find(')') else { break };
            let target = rest[..close].split('#').next().unwrap_or("").trim();
            rest = &rest[close + 1..];
            if target.is_empty() || target.contains("://") || target.starts_with('/') {
                continue;
            }
            assert!(
                dir.join(target).exists(),
                "yidam/editors/README.md:{}: `{target}` does not resolve",
                i + 1
            );
            checked += 1;
        }
    }
    assert!(checked > 0, "no links found — the extractor is broken");
}

/// The install line's feature gate names a feature this crate has.
#[test]
fn the_documented_feature_gate_is_a_real_cargo_feature() {
    let text = read_guide();
    let manifest =
        std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml"))
            .expect("Cargo.toml");
    let features = manifest
        .split_once("[features]")
        .expect("a [features] table")
        .1;

    let mut checked = 0usize;
    for fence in fences(&text) {
        for (i, _) in fence.match_indices("--features ") {
            let after = &fence[i + "--features ".len()..];
            let name = after.split_whitespace().next().unwrap_or("");
            assert!(
                features
                    .lines()
                    .any(|l| l.split('=').next().unwrap_or("").trim() == name),
                "the guide installs `--features {name}`, which yidam/cli/Cargo.toml does not define"
            );
            checked += 1;
        }
    }
    assert!(
        checked > 0,
        "no --features line found — the extractor is broken"
    );
}
