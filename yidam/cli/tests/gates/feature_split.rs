//! Building an index and reading one are one build, and it needs no protoc.
//!
//! #442 split `vector-read` out of `index` because the two had different costs: building an
//! index wrote a LanceDB table, lancedb's `prost-build` required protoc 31, and reading needed
//! only fastembed and arrow. #1287 found that nothing ever opened that table — every reader
//! decodes `corpus.arrow` — and removed it. `index-build` moved into `vector-read`, and `index`
//! became an alias kept so every `--features index` already written still builds.
//!
//! What this file holds is that the split does not quietly come back. A `feature = "index"`
//! written anywhere in `src/` would gate code on a flag that enables nothing of its own, so a
//! `vector-read` build — the one that can now build an index — would lose it without failing
//! anything. And a dependency that needs protoc would put the cost #1287 removed back on every
//! machine that builds an index.

use std::collections::BTreeSet;
use std::path::PathBuf;

fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(rel: &str) -> String {
    let p = crate_root().join(rel);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{} unreadable: {e}", p.display()))
}

/// Every `.rs` under `src/`.
fn source_files() -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![crate_root().join("src")];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "rs") {
                out.push(path);
            }
        }
    }
    out.sort();
    out
}

fn rel(p: &std::path::Path) -> String {
    p.strip_prefix(crate_root())
        .unwrap_or(p)
        .to_string_lossy()
        .replace('\\', "/")
}

/// `(file, line number, line)` for each line of `src/` whose *code* — not a comment on it —
/// names `needle`.
fn code_lines_naming(needle: &str) -> Vec<(String, usize, String)> {
    let mut out = Vec::new();
    for path in source_files() {
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        for (i, line) in text.lines().enumerate() {
            // Comments discuss `--features index` legitimately; a `cfg` is the thing that
            // changes what compiles.
            let code = line.split("//").next().unwrap_or("");
            if code.contains(needle) {
                out.push((rel(&path), i + 1, line.trim().to_string()));
            }
        }
    }
    out
}

/// **The load-bearing assertion.** No code is gated on `index`.
///
/// The whole tree rather than a curated list of read-path files, which is what this used to
/// scan: the list had to be kept complete by hand, and #468 found three gated files missing
/// from it. With one feature there is no file that may legitimately name the other.
#[test]
fn no_code_is_gated_on_the_alias() {
    let offenders: Vec<String> = code_lines_naming("feature = \"index\"")
        .into_iter()
        .map(|(file, n, line)| format!("  {file}:{n} — {line}"))
        .collect();
    assert!(
        offenders.is_empty(),
        "`index` is an alias for `vector-read` and enables nothing of its own, so code gated \
         on it is missing from the `vector-read` build that can run it. Gate on \
         `vector-read`:\n{}",
        offenders.join("\n")
    );
}

/// The scan above has to be looking at something. If the spelling of a feature gate changed,
/// it would pass by matching nothing.
#[test]
fn the_scan_sees_the_gates_it_is_looking_for() {
    let gated: BTreeSet<String> = code_lines_naming("feature = \"vector-read\"")
        .into_iter()
        .map(|(file, _, _)| file)
        .collect();
    assert!(
        gated.len() >= 8,
        "only {} files gate on `vector-read` ({gated:?}); if that spelling changed, \
         `no_code_is_gated_on_the_alias` is vacuous",
        gated.len()
    );
    assert!(
        gated.contains("src/cmd/mod.rs"),
        "`index_build` is declared in src/cmd/mod.rs behind `vector-read`, and the scan did \
         not see it: {gated:?}"
    );
}

/// `index` is exactly `vector-read`, and nothing more.
///
/// A dependency added here would be a cost only `--features index` pays, which is the split
/// #1287 removed coming back under the old name.
#[test]
fn the_index_feature_is_an_alias() {
    let toml = read("Cargo.toml");
    let index: Vec<String> = feature_body(&toml, "index")
        .split(',')
        .map(|s| s.trim().trim_matches('"').to_string())
        .filter(|s| !s.is_empty())
        .collect();
    assert_eq!(
        index,
        ["vector-read"],
        "`index` must be an alias for `vector-read`"
    );
}

/// The point of the feature is what it does *not* pull.
#[test]
fn building_an_index_needs_no_runtime_and_no_lancedb() {
    let toml = read("Cargo.toml");
    let vr = feature_body(&toml, "vector-read");
    for forbidden in ["lancedb", "futures", "tokio"] {
        assert!(
            !vr.contains(forbidden),
            "`vector-read` must not pull {forbidden}: {vr}"
        );
    }
    assert!(vr.contains("fastembed"), "it does need the model: {vr}");
    assert!(vr.contains("arrow-ipc"), "and the encoder: {vr}");

    let manifest: toml::Value = toml::from_str(&toml).expect("Cargo.toml is not TOML");
    assert!(
        manifest["dependencies"].get("lancedb").is_none(),
        "lancedb is back in Cargo.toml. Nothing reads a LanceDB table — every reader decodes \
         `corpus.arrow` — and lancedb is what made building an index need protoc (#1287)."
    );
}

/// protoc is required by `prost-build`, and only by it. Nothing in the graph may reach it.
///
/// Read off the lockfile, so it covers every feature at once: a dependency of `export-sqlite`
/// or `calculators-gluon` that took `prost-build` would put protoc back on the `--features
/// full` build as surely as lancedb did, and `mise.toml` no longer provisions it.
#[test]
fn nothing_in_the_lockfile_needs_protoc() {
    let lock = read("Cargo.lock");
    assert!(
        lock.contains("\nname = \"fastembed\""),
        "Cargo.lock resolves no fastembed — this is reading the wrong lockfile"
    );
    assert!(
        !lock.contains("\nname = \"prost-build\""),
        "Cargo.lock resolves `prost-build`, which runs protoc at build time. Find what pulls it \
         with `cargo tree -e build -i prost-build --features full --target all`."
    );
}

/// A client tells a build that can make an index from one that cannot by the reported list,
/// and since #1287 those are the same builds that can and cannot read one.
#[test]
fn the_reported_feature_list_says_whether_this_build_can_make_an_index() {
    let features = yidam::report::YidamBlock::current().features;
    let can_read = features.iter().any(|f| f == "vector-read");
    let can_build = features.iter().any(|f| f == "index");
    assert_eq!(
        can_read,
        cfg!(feature = "vector-read"),
        "the list must say whether this build can read an index: {features:?}"
    );
    assert_eq!(
        can_build, can_read,
        "a build that can read an index can build one, and the list must say so: {features:?}"
    );
}

/// The body of one `[features]` entry, as written.
fn feature_body(toml: &str, name: &str) -> String {
    let key = format!("\n{name} = [");
    let start = toml
        .find(&key)
        .unwrap_or_else(|| panic!("no `{name}` feature in Cargo.toml"));
    let rest = &toml[start + key.len()..];
    let end = rest.find(']').expect("unterminated feature list");
    rest[..end].to_string()
}
