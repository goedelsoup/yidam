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

use std::path::PathBuf;

fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(rel: &str) -> String {
    let p = crate_root().join(rel);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{} unreadable: {e}", p.display()))
}

/// Every source file that answers a query, as opposed to building an index.
const READ_PATH: &[&str] = &[
    "src/retrieval/mod.rs",
    "src/retrieval/vector.rs",
    "src/model.rs",
    "src/cmd/serve/mod.rs",
    "src/cmd/serve/tools.rs",
    "src/cmd/serve/resources.rs",
    "src/cmd/query/anchor.rs",
    "src/embedding.rs",
];

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
