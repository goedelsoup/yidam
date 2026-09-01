//! One palette, and every surface reads it rather than a copy of it.
//!
//! `yidam/design/tokens/*.css` had two consumers and neither imported it (#465). The docs
//! site retyped the hex values into Starlight's variable slots with the token name in a
//! trailing comment; the CLI's web agent retyped a subset into a `:root` block whose own
//! first line called it a subset. Three copies, kept in step by nothing.
//!
//! They had not stayed in step. Ten of the CLI copy's twenty-four values no longer matched
//! the source, and `--inference-*` and `--open-*` had **swapped colour families**: an
//! exported corpus showed inference in blue and open questions in amber while every other
//! surface said the reverse. The web agent's own `DESIGN.md` had specified
//! `--status-{verified,inference,open}-*` from the start. Nothing made it so.
//!
//! What this file checks is that the collapse holds. The lint that reads JSX is a separate
//! job — oxlint parses JavaScript and the two drifted consumers are CSS, which it cannot
//! read at all, so the CSS side is guarded here instead.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use walkdir::WalkDir;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn read(rel: &str) -> String {
    let p = repo_root().join(rel);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{} unreadable: {e}", p.display()))
}

const DESIGN: &str = "yidam/design";

/// CSS with `/* … */` blocks blanked, line structure preserved.
///
/// Necessary, and found the way these are found: the first version of this file scanned raw
/// text and failed on its own prose. `#465` in a header comment read as a colour, and a
/// `var(--…)` in a sentence explaining the rule read as a dangling token. A guard that
/// grades comments is the mirror of #461's guard that was *satisfied* by one — the same
/// mistake seen from the other side.
///
/// Newlines survive so reported line numbers stay true.
fn css_code_only(css: &str) -> String {
    let mut out = String::with_capacity(css.len());
    let mut chars = css.char_indices().peekable();
    let mut in_comment = false;
    while let Some((i, c)) = chars.next() {
        if !in_comment && c == '/' && css[i..].starts_with("/*") {
            in_comment = true;
            chars.next();
            continue;
        }
        if in_comment {
            if c == '*' && css[i..].starts_with("*/") {
                in_comment = false;
                chars.next();
            } else if c == '\n' {
                out.push('\n');
            }
            continue;
        }
        out.push(c);
    }
    out
}

/// `--name: value` declarations in a stylesheet, first definition winning.
fn declarations(css: &str) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for line in css_code_only(css).lines() {
        let line = line.trim();
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        let name = name.trim();
        if !name.starts_with("--") {
            continue;
        }
        out.entry(name.to_string())
            .or_insert_with(|| value.trim().trim_end_matches(';').trim().to_string());
    }
    out
}

/// Every token the design system declares.
fn source_tokens() -> BTreeMap<String, String> {
    let dir = repo_root().join(DESIGN).join("tokens");
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
        .expect("yidam/design/tokens is readable")
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "css"))
        .collect();
    files.sort();
    let mut out = BTreeMap::new();
    for f in files {
        for (k, v) in declarations(&std::fs::read_to_string(&f).expect("token file")) {
            out.entry(k).or_insert(v);
        }
    }
    assert!(
        out.len() > 100,
        "only {} tokens parsed from yidam/design/tokens; every assertion below would be \
         vacuous",
        out.len()
    );
    out
}

/// Stylesheets outside the design system that consume its tokens — discovered, not listed.
///
/// A consumer is any `.css` outside `yidam/design/` that references an `--ink-*` or
/// `--gold-*` token. P6's quality pages and the next UI kit are covered the moment they use
/// the palette, which is the point: a roster here would stop covering whatever came next.
fn consumer_stylesheets() -> Vec<(String, String)> {
    let mut out = Vec::new();
    for entry in WalkDir::new(repo_root())
        .into_iter()
        .filter_entry(|e| {
            let n = e.file_name().to_string_lossy();
            n != "node_modules" && n != "target" && n != ".git" && n != "dist" && n != ".claude"
        })
        .filter_map(Result::ok)
    {
        let path = entry.path();
        if !entry.file_type().is_file() || path.extension().is_none_or(|e| e != "css") {
            continue;
        }
        let rel = path
            .strip_prefix(repo_root())
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/");
        if rel.starts_with(DESIGN) {
            continue;
        }
        let text = std::fs::read_to_string(path).unwrap_or_default();
        if text.contains("--ink-") || text.contains("--gold-") {
            out.push((rel, text));
        }
    }
    out.sort();
    out
}

/// No surface outside the design system spells a colour.
///
/// This is the copy-prevention, and it is deliberately about *hex* rather than about every
/// literal: a stylesheet may reasonably say `3px` or `50%`, and may not reasonably decide
/// what colour gold is. Every copy this phase collapsed was a colour copy.
#[test]
fn no_consumer_declares_a_raw_colour() {
    let consumers = consumer_stylesheets();
    assert!(
        !consumers.is_empty(),
        "no stylesheet outside yidam/design references the palette. Either both consumers \
         stopped using the design system, or this test is looking at the wrong tree — and \
         either way it is now asserting nothing."
    );

    let mut offenders = Vec::new();
    for (rel, text) in &consumers {
        // Comments may name a colour — this file's own header does. Declarations may not.
        let stripped = css_code_only(text);
        for (i, code) in stripped.lines().enumerate() {
            if !code.contains(':') {
                continue;
            }
            for (idx, c) in code.char_indices() {
                if c != '#' {
                    continue;
                }
                let hex: String = code[idx + 1..]
                    .chars()
                    .take_while(|c| c.is_ascii_hexdigit())
                    .collect();
                if (3..=8).contains(&hex.len()) {
                    offenders.push(format!("  {rel}:{}: #{hex}", i + 1));
                }
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "these declare a colour instead of referencing one. That is how the palette came to \
         have three copies and how two evidence-tag colour families ended up transposed \
         between them:\n{}",
        offenders.join("\n")
    );
}

/// Every token a consumer references exists at the source.
///
/// The other direction, and the one that fails on a rename: `var(--ink-900)` against a
/// palette that no longer declares `--ink-900` resolves to nothing, and CSS says nothing
/// about it. The page renders unstyled and the build stays green.
#[test]
fn every_token_a_consumer_uses_is_declared_at_the_source() {
    let source = source_tokens();
    let mut dangling = Vec::new();
    for (rel, text) in consumer_stylesheets() {
        let stripped = css_code_only(&text);
        for (i, line) in stripped.lines().enumerate() {
            let mut rest = line;
            while let Some(at) = rest.find("var(--") {
                let after = &rest[at + 4..];
                let name: String = after
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '-')
                    .collect();
                // A consumer may define and use its own local variables (Starlight's `--sl-*`).
                let local = declarations(&text).contains_key(&name);
                if !local && !source.contains_key(&name) {
                    dangling.push(format!("  {rel}:{}: var({name})", i + 1));
                }
                rest = &after[name.len()..];
            }
        }
    }
    assert!(
        dangling.is_empty(),
        "these reference tokens the design system does not declare; each resolves to nothing \
         and renders as an unstyled element under a green build:\n{}",
        dangling.join("\n")
    );
}

/// The bundle imports what the manifest declares, in the order it declares.
///
/// `_ds_manifest.json` has listed the token files since the system was synced and nothing
/// read the list. Order is load-bearing — `semantic.css` resolves `var(--ink-…)` from the
/// files above it — so an entry point that reordered them would produce a stylesheet whose
/// variables silently resolve to nothing.
#[test]
fn the_entry_point_imports_the_manifests_files_in_order() {
    let manifest: serde_json::Value =
        serde_json::from_str(&read(&format!("{DESIGN}/_ds_manifest.json")))
            .expect("_ds_manifest.json parses");
    let declared: Vec<String> = manifest["globalCssPaths"]
        .as_array()
        .expect("globalCssPaths is an array")
        .iter()
        .map(|v| v.as_str().expect("a string").to_string())
        .collect();
    assert!(!declared.is_empty(), "the manifest names no CSS");

    let entry = read(&format!("{DESIGN}/tokens.css"));
    let imported: Vec<String> = entry
        .lines()
        .filter_map(|l| l.trim().strip_prefix("@import \"./"))
        .filter_map(|r| r.split('"').next())
        .map(str::to_string)
        .collect();

    assert_eq!(
        imported, declared,
        "yidam/design/tokens.css and _ds_manifest.json disagree about what the system is or \
         in what order it loads"
    );
}

/// The binary embeds the same set, and by reading the manifest rather than a list of its own.
#[test]
fn the_web_exporter_embeds_the_design_system() {
    let build_rs = read("yidam/cli/build.rs");
    assert!(
        build_rs.contains("_ds_manifest.json") && build_rs.contains("globalCssPaths"),
        "build.rs no longer reads the manifest; the exporter's token set is a second opinion \
         about what the design system contains"
    );
    let export = read("yidam/cli/src/cmd/export_web.rs");
    assert!(
        export.contains("DESIGN_TOKENS"),
        "the web exporter no longer embeds the design bundle — an exported page would carry \
         `var(--…)` references and nothing to resolve them"
    );
    let main_css = read("yidam/cli/assets/web/main.css");
    let declared: BTreeSet<String> = declarations(&main_css).into_keys().collect();
    assert!(
        declared.is_empty(),
        "assets/web/main.css declares {} token(s) of its own again; that is the copy #465 \
         removed, and the copy had transposed two colour families: {declared:?}",
        declared.len()
    );
}

/// Changing a token at the source moves every surface. The deliverable, stated as a test.
///
/// Not a simulation: it reads the real bundle the build script produced and the real
/// consumers, and asserts that each resolves the value through a reference rather than
/// holding one. If any surface held its own copy, that surface would be unaffected by a
/// change at the source — which is exactly the state this phase found and is the only thing
/// worth asserting here.
#[test]
fn one_change_at_the_source_reaches_every_surface() {
    let source = source_tokens();
    let probe = "--gold-400";
    let source_value = source
        .get(probe)
        .unwrap_or_else(|| panic!("{probe} is no longer a token; pick another probe"));

    for (rel, text) in consumer_stylesheets() {
        let code = css_code_only(&text);
        assert!(
            !code.contains(source_value.as_str()),
            "{rel} contains the literal value of {probe} ({source_value}). It is a copy: a \
             change at the source would leave it behind, silently, which is how the CLI's \
             copy came to disagree with the palette in ten places."
        );
        assert!(
            code.contains(&format!("var({probe})")) || code.contains("var(--ink-"),
            "{rel} references the palette but never through {probe} or an ink token; either \
             it stopped being a consumer or it found a third way to spell a colour"
        );
    }
}
