//! Symbol-grained staleness (RFC-0051 §4.1, #1524), and the promise that keeps it optional:
//! **the light build resolves no tree-sitter crate.**
//!
//! Not behind `#[cfg(feature = "symbol-pins")]`, for the reason `gluon_arm.rs` gives: a gate
//! that only runs in the build it is about asserts nothing about the build that matters. Each
//! test reads the manifest, the lockfile or the source tree, so it is the same assertion in
//! every build.

use std::path::PathBuf;

fn cli_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(name: &str) -> String {
    let p = cli_dir().join(name);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{} unreadable: {e}", p.display()))
}

const GRAMMARS: [&str; 4] = [
    "tree-sitter",
    "tree-sitter-rust",
    "tree-sitter-python",
    "tree-sitter-typescript",
];

/// The feature names exactly the four optional dependencies, is not in `default`, and is in
/// `full`.
#[test]
fn the_parser_is_outside_the_default_set() {
    let toml = read("Cargo.toml");
    let default = toml
        .lines()
        .find(|l| l.starts_with("default = ["))
        .expect("no `default` feature line");
    assert!(
        !default.contains("symbol-pins"),
        "`symbol-pins` is in the default set:\n  {default}\n\n\
         tree-sitter and three grammars are C compiled at build time, which the light build \
         does not do for parsing. A symbol pin in a build without it is read at file grain, \
         which only ever reports more reads due."
    );
    let wanted = format!(
        "symbol-pins = [{}]",
        GRAMMARS
            .iter()
            .map(|g| format!("\"dep:{g}\""))
            .collect::<Vec<_>>()
            .join(", ")
    );
    assert!(
        toml.contains(&wanted),
        "the `symbol-pins` feature no longer reads `{wanted}`"
    );
    for g in GRAMMARS {
        let key = format!("{g} = {{");
        let line = toml
            .lines()
            .find(|l| l.starts_with(&key))
            .unwrap_or_else(|| panic!("no `{key}` dependency line"));
        assert!(
            line.contains("optional = true"),
            "{g} is not optional, so the default build links it:\n  {line}"
        );
    }
    let full_start = toml.find("\nfull = [").expect("no `full` feature");
    let full_end = toml[full_start..].find(']').expect("unterminated `full`") + full_start;
    assert!(
        toml[full_start..full_end].contains("\"symbol-pins\""),
        "`full` does not name `symbol-pins`, so only `--all-features` would compile it"
    );
}

/// No package in the lockfile but a tree-sitter crate depends on one, so nothing else in
/// the default build can pull the parser in by a path the manifest does not show.
#[test]
fn only_the_feature_reaches_a_tree_sitter_crate() {
    let lock = read("Cargo.lock");
    let mut strays = Vec::new();
    for package in lock.split("\n[[package]]\n").skip(1) {
        let name = package
            .lines()
            .find_map(|l| l.strip_prefix("name = \""))
            .and_then(|l| l.strip_suffix('"'))
            .unwrap_or_default();
        if name == "yidam" || name.starts_with("tree-sitter") {
            continue;
        }
        if package.contains("\n \"tree-sitter") {
            strays.push(name.to_string());
        }
    }
    assert!(
        strays.is_empty(),
        "these packages depend on a tree-sitter crate: {strays:?}\n\n\
         The light build resolves none, which holds only while `symbol-pins` is the one way in."
    );
}

/// Nothing outside `src/cmd/lint/symbols.rs` names the parser.
#[test]
fn only_the_symbol_module_names_the_parser() {
    let src = cli_dir().join("src");
    let mut strays: Vec<String> = Vec::new();
    for entry in walkdir::WalkDir::new(&src).into_iter().flatten() {
        if !entry.file_type().is_file() || entry.path().extension().is_none_or(|e| e != "rs") {
            continue;
        }
        let rel = entry
            .path()
            .strip_prefix(&src)
            .unwrap_or(entry.path())
            .to_string_lossy()
            .replace('\\', "/");
        if rel == "cmd/lint/symbols.rs" {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(entry.path()) else {
            continue;
        };
        for (i, line) in text.lines().enumerate() {
            let code = line.split("//").next().unwrap_or("");
            if code.contains("tree_sitter") {
                strays.push(format!("  src/{rel}:{}: {}", i + 1, line.trim()));
            }
        }
    }
    assert!(
        strays.is_empty(),
        "the parser is reached from outside `src/cmd/lint/symbols.rs`:\n{}\n\n\
         Every use belongs behind `symbol-pins` in one module, which hands the rest of the \
         crate a `Grain` and nothing of tree-sitter's.",
        strays.join("\n")
    );
}
