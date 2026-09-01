//! The read path must not drift back onto the build feature.
//!
//! `vector-read` exists because reading an index and building one have different costs:
//! `lancedb` is named in exactly one file and it is what requires protoc, while decoding
//! `corpus.arrow` and embedding a query need neither. RFC-0023 gave an index a way to travel
//! between machines; this is the build that can receive one.
//!
//! The split is enforced by `#[cfg]` attributes scattered across seven files, and a single
//! `feature = "index"` written in the read path would put vector search back behind protoc
//! without failing anything — the default build would still degrade correctly, the full build
//! would still work, and only the middle build nobody's CI compiles would quietly lose its
//! reason to exist. So the arrangement is asserted rather than assumed.

use std::collections::BTreeSet;
use std::path::PathBuf;

fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(rel: &str) -> String {
    let p = crate_root().join(rel);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{} unreadable: {e}", p.display()))
}

/// Files that answer a query, as opposed to building an index.
///
/// Curated, and it has to be: the property is "every `#[cfg]` in this file is about reading",
/// which no scan can decide. What #468's audit found is that nothing checked the list was
/// *complete* — three files carrying the `vector-read` gate were absent from it, so the
/// load-bearing assertion below, whose own comment says "nothing else would notice", did not
/// look at them. One of the three, `src/cmd/export.rs`, belonged here.
///
/// [`MIXED`] carries the other two and the reason each is excluded, and
/// `every_file_in_the_split_is_accounted_for` requires every gated file to be in one list or
/// the other. A file added tomorrow fails that test until somebody decides which it is —
/// which is the inverted-roster shape `report_goldens.rs` uses, for the same reason.
const READ_PATH: &[&str] = &[
    "src/retrieval/mod.rs",
    "src/retrieval/vector.rs",
    "src/model.rs",
    "src/cmd/serve/mod.rs",
    "src/cmd/serve/tools.rs",
    "src/cmd/serve/resources.rs",
    "src/cmd/query/anchor.rs",
    "src/cmd/export.rs",
    "src/embedding.rs",
];

/// Files that take part in the split and may legitimately name `index` as well.
///
/// The reasons are load-bearing. Both of these would fail the scan below on a line that is
/// correct, and writing down why is what keeps the next reader from "fixing" it.
const MIXED: &[(&str, &str)] = &[
    (
        "src/lib.rs",
        "declares both halves: `#[cfg(feature = \"index\")] pub use cmd::index_build` is the \
         build path's re-export, not a gate on reading",
    ),
    (
        "src/report.rs",
        "reports the feature list, so it must ask `cfg!(feature = \"index\")` about the build \
         it is describing — `the_reported_feature_list_separates_reading_from_building` in \
         this file requires exactly that",
    ),
];

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

/// Files that name the read feature, discovered.
fn files_naming_the_read_feature() -> BTreeSet<String> {
    let out: BTreeSet<String> = source_files()
        .into_iter()
        .filter(|p| {
            std::fs::read_to_string(p)
                .unwrap_or_default()
                .contains("feature = \"vector-read\"")
        })
        .map(|p| {
            p.strip_prefix(crate_root())
                .unwrap_or(&p)
                .to_string_lossy()
                .replace('\\', "/")
        })
        .collect();
    assert!(
        out.len() >= 8,
        "only {} files name `vector-read` ({out:?}); if that spelling changed, every \
         assertion built on this is vacuous",
        out.len()
    );
    out
}

/// Nothing in the split is unaccounted for.
///
/// The check the hardcoded list never had. A file that joins the split and lands in neither
/// list is not scanned by the assertion below and nobody is told — the way `src/cmd/export.rs`
/// was not scanned, for as long as it has existed.
#[test]
fn every_file_in_the_split_is_accounted_for() {
    let listed: BTreeSet<&str> = READ_PATH
        .iter()
        .copied()
        .chain(MIXED.iter().map(|(rel, _)| *rel))
        .collect();
    let unaccounted: Vec<String> = files_naming_the_read_feature()
        .into_iter()
        .filter(|rel| !listed.contains(rel.as_str()))
        .collect();
    assert!(
        unaccounted.is_empty(),
        "these files take part in the vector-read split and are in neither list: \
         {unaccounted:?}.\n\nAdd each to READ_PATH, so the scan covers it, or to MIXED with \
         the reason it may also name `index`. Leaving it out is the third option and it is \
         the one that fails silently."
    );

    for (rel, reason) in MIXED {
        assert!(
            crate_root().join(rel).is_file(),
            "MIXED names {rel}, which is gone"
        );
        assert!(!reason.is_empty(), "{rel} is excluded and does not say why");
    }
}

/// **The load-bearing assertion.** A `feature = "index"` anywhere in the read path puts vector
/// search back behind protoc, and nothing else would notice.
#[test]
fn no_read_path_file_is_gated_on_the_build_feature() {
    let mut offenders = Vec::new();
    for rel in READ_PATH {
        let text = read(rel);
        for (i, line) in text.lines().enumerate() {
            // Comments discuss `--features index` legitimately; a `cfg` is the thing that
            // changes what compiles.
            let code = line.split("//").next().unwrap_or("");
            if code.contains("feature = \"index\"") {
                offenders.push(format!("  {rel}:{} — {}", i + 1, line.trim()));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "the read path must be gated on `vector-read`, not `index`:\n{}",
        offenders.join("\n")
    );
}

/// The scan has to be looking at something. A renamed or moved file would otherwise make the
/// test above pass by reading nothing.
#[test]
fn the_read_path_files_all_exist_and_are_gated() {
    let mut gated = 0;
    for rel in READ_PATH {
        assert!(
            crate_root().join(rel).is_file(),
            "{rel} is gone — this scan is checking less than it claims"
        );
        if read(rel).contains("feature = \"vector-read\"") {
            gated += 1;
        }
    }
    assert!(
        gated >= 5,
        "only {gated} read-path files mention the feature; the split has been undone"
    );
}

/// `index` must imply `vector-read`, or a full build loses the read path it depends on.
#[test]
fn building_an_index_implies_being_able_to_read_one() {
    let toml = read("Cargo.toml");
    let index = feature_body(&toml, "index");
    assert!(
        index.contains("\"vector-read\""),
        "`index` must include `vector-read`, got: {index}"
    );
}

/// The point of the feature is what it does *not* pull. `lancedb` is what requires protoc.
#[test]
fn reading_an_index_needs_neither_lancedb_nor_a_runtime() {
    let toml = read("Cargo.toml");
    let vr = feature_body(&toml, "vector-read");
    for forbidden in ["lancedb", "futures", "tokio"] {
        assert!(
            !vr.contains(forbidden),
            "`vector-read` must not pull {forbidden} — that is a build-an-index cost: {vr}"
        );
    }
    assert!(vr.contains("fastembed"), "it does need the model: {vr}");
    assert!(vr.contains("arrow-ipc"), "and the decoder: {vr}");
}

/// The three builds must be distinguishable by what they report, or a client cannot tell
/// "cannot read an index" from "can read but not build one".
#[test]
fn the_reported_feature_list_separates_reading_from_building() {
    let features = yidam::report::YidamBlock::current().features;
    let can_read = features.iter().any(|f| f == "vector-read");
    let can_build = features.iter().any(|f| f == "index");
    assert_eq!(
        can_read,
        cfg!(feature = "vector-read"),
        "the list must say whether this build can read an index: {features:?}"
    );
    assert!(
        !can_build || can_read,
        "a build that can build an index can read one: {features:?}"
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
