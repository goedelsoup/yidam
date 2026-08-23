//! `VERSIONING.md` names files. This checks they are there.
//!
//! A versioning document is a promise about where things live, and a promise nothing checks
//! is one that rots at the first move. This one had already rotted: the paths in Layers 2
//! and 3 predate the reorganisation that put everything under `yidam/`, and the release
//! process points at manifests that have not been at those paths for some time. Nobody
//! noticed, because nobody types a path out of a versioning document until the day they
//! release something.
//!
//! Deliberately narrow. It checks that referenced files exist, and that each version this
//! repository states in two places states the same thing in both. It does not check prose,
//! and it cannot tell a correct bump from a wrong one — that is what review is for.

use std::path::PathBuf;

fn repo_root() -> PathBuf {
    // CARGO_MANIFEST_DIR = yidam/cli/
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn versioning() -> String {
    std::fs::read_to_string(repo_root().join("VERSIONING.md")).expect("VERSIONING.md")
}

/// Every backticked token that looks like a repository path.
///
/// Path-shaped means: contains a `/`, and ends in an extension this repository actually
/// uses for a manifest or a source file. That excludes `ma/<elector>`-style namespaces and
/// `sdk/rust/v0.1.0`-style tags, which are backticked, contain slashes, and are not files.
fn referenced_paths(text: &str) -> Vec<String> {
    let extensions = [".toml", ".json", ".rs", ".md", ".yml"];
    let mut out: Vec<String> = text
        .split('`')
        .skip(1)
        .step_by(2)
        .map(str::trim)
        .filter(|t| t.contains('/') && extensions.iter().any(|e| t.ends_with(e)))
        .map(str::to_string)
        .collect();
    out.sort();
    out.dedup();
    out
}

/// Also the paths inside markdown links, which is how the newer sections write them.
fn linked_paths(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for chunk in text.split("](").skip(1) {
        let Some(end) = chunk.find(')') else { continue };
        let target = chunk[..end].trim();
        if target.starts_with('#') || target.contains("://") {
            continue;
        }
        out.push(target.to_string());
    }
    out.sort();
    out.dedup();
    out
}

/// And the path comments inside code fences, which is how a section points at the file a
/// constant lives in. Not backticked and not a link, so neither scan above sees them — and
/// the one in Layer 3 was wrong.
fn commented_paths(text: &str) -> Vec<String> {
    let extensions = [".toml", ".json", ".rs", ".md", ".yml"];
    let mut out: Vec<String> = text
        .lines()
        .filter_map(|l| l.trim().strip_prefix("// "))
        .map(str::trim)
        .filter(|t| t.contains('/') && extensions.iter().any(|e| t.ends_with(e)))
        .map(str::to_string)
        .collect();
    out.sort();
    out.dedup();
    out
}

#[test]
fn every_file_versioning_md_names_exists() {
    let root = repo_root();
    let text = versioning();

    let mut referenced = referenced_paths(&text);
    referenced.extend(linked_paths(&text));
    referenced.extend(commented_paths(&text));
    assert!(
        referenced.len() > 5,
        "found only {} path(s) — the scan is broken, not the document",
        referenced.len()
    );

    let missing: Vec<String> = referenced
        .iter()
        .filter(|p| !root.join(p).exists())
        .cloned()
        .collect();

    assert!(
        missing.is_empty(),
        "VERSIONING.md names files that do not exist:\n{}\n\nA versioning document is a \
         promise about where things live. Fix the path, or move the file.",
        missing
            .iter()
            .map(|p| format!("  {p}"))
            .collect::<Vec<_>>()
            .join("\n")
    );
}

/// Layer 4 declares `format_version` as the contract between the CLI and the editor client,
/// and quotes it. The document and the constant have to say the same thing.
///
/// Read out of the source rather than linked against: `report::FORMAT_VERSION` is not public
/// API of the crate, and widening a surface to satisfy a test is how surfaces widen.
#[test]
fn the_documented_format_version_is_the_declared_one() {
    let quoted = |text: &str| -> Option<String> {
        text.split("pub const FORMAT_VERSION: &str = \"")
            .nth(1)?
            .split('"')
            .next()
            .map(str::to_string)
    };
    let documented = quoted(&versioning()).expect("VERSIONING.md quotes FORMAT_VERSION");
    let declared =
        quoted(&std::fs::read_to_string(repo_root().join("yidam/cli/src/report.rs")).unwrap())
            .expect("report.rs declares FORMAT_VERSION");
    assert_eq!(
        documented, declared,
        "VERSIONING.md says format_version is {documented:?} and report.rs says {declared:?}"
    );
}

/// Layer 3 declares `PROTOCOL_VERSION` as the bootstrap protocol's version, and quotes the
/// file it lives in. Same shape as the check above — with one difference worth naming: for
/// the whole of 0.1.0 this assertion would have failed on the harness side, because the
/// document quoted a constant that was declared nowhere. `every_file_versioning_md_names_exists`
/// did not catch it; it checks that named *paths* resolve, and the path was fine.
///
/// Read out of the source because the harness is a separate cargo workspace — `yidam-harness`
/// is not a dependency of the CLI and should not become one to satisfy a test.
#[test]
fn the_documented_protocol_version_is_the_declared_one() {
    let quoted = |text: &str| -> Option<String> {
        text.split("pub const PROTOCOL_VERSION: &str = \"")
            .nth(1)?
            .split('"')
            .next()
            .map(str::to_string)
    };
    let harness = repo_root().join("yidam/tests/harness/yidam-harness/src/lib.rs");
    let documented = quoted(&versioning()).expect("VERSIONING.md quotes PROTOCOL_VERSION");
    let declared = quoted(&std::fs::read_to_string(&harness).unwrap())
        .expect("the harness declares PROTOCOL_VERSION");
    assert_eq!(
        documented, declared,
        "VERSIONING.md says the bootstrap protocol is {documented:?} and the harness says \
         {declared:?}"
    );
}

/// Layer 4's two artifacts must both carry a version their registries can read.
#[test]
fn both_tooling_artifacts_declare_a_version() {
    let root = repo_root();
    let cargo = std::fs::read_to_string(root.join("yidam/cli/Cargo.toml")).unwrap();
    assert!(
        cargo.lines().any(|l| l.starts_with("version = \"")),
        "the CLI has no version for crates.io to publish"
    );
    let package = std::fs::read_to_string(root.join("yidam/editors/vscode/package.json")).unwrap();
    assert!(
        package.contains("\"version\":"),
        "the extension has no version for the Marketplace to publish"
    );
}

/// The MCP contract states its version twice, and the two must agree.
///
/// `tools.json`'s `contract` field is the live one — `serve --mcp` compiles it in and
/// returns it in the `yidam` capability block, and the E2E test asserts the server reports
/// what the file says. `mcp/VERSION` is read by nothing, which is the whole problem: a
/// version nobody reads is a version nobody notices going stale, and the next hand-bump has
/// even odds of touching one and not the other.
///
/// It lives here rather than in `mcp_serve.rs` deliberately. That file is
/// `#![cfg(feature = "index")]` and the full-feature job runs on main and the weekly
/// schedule, never on a pull request — so a check placed there would first speak up after
/// the merge that broke it. This file is in the default build.
#[test]
fn the_mcp_contract_states_one_version() {
    let mcp = repo_root().join("yidam/prelude/sdks/parity/mcp");
    let tools: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(mcp.join("tools.json")).unwrap()).unwrap();
    let declared = tools["contract"]
        .as_str()
        .expect("tools.json has `contract`");
    let stated = std::fs::read_to_string(mcp.join("VERSION")).unwrap();
    assert_eq!(
        declared,
        stated.trim(),
        "tools.json says the MCP contract is {declared:?} and mcp/VERSION says {:?}",
        stated.trim()
    );
}

/// Four layers, numbered without a gap.
#[test]
fn the_layers_are_numbered_and_counted_consistently() {
    let text = versioning();
    let headings: Vec<&str> = text
        .lines()
        .filter(|l| l.starts_with("## Layer "))
        .collect();
    assert_eq!(headings.len(), 4, "{headings:?}");
    for (i, heading) in headings.iter().enumerate() {
        assert!(
            heading.starts_with(&format!("## Layer {}", i + 1)),
            "layers are out of order or renumbered: {heading}"
        );
    }
    assert!(
        text.contains("four independent versioning layers"),
        "the opening sentence still counts three"
    );
}

/// The tag pattern `VERSIONING.md` documents is the one a workflow actually triggers on.
///
/// Line 205 has said "CI publishes to registries on matching tag patterns" since it was
/// written, and for that whole time `.github/workflows/` held `ci.yml` and nothing else.
/// The sentence was not wrong about intent and was wrong about the world, which is the
/// failure mode this file exists for: a versioning document is a promise, and a promise
/// nothing checks rots at the first move.
///
/// It asserts the *pattern*, not merely that a release workflow exists. The CLI layer tags
/// `cli/v{...}` and the template layer tags `v{...}`; a workflow listening on `v*` would
/// publish a CLI binary every time the template released, and both spellings look correct
/// in a diff.
#[test]
fn the_cli_release_workflow_triggers_on_the_documented_tag_pattern() {
    let workflow = repo_root().join(".github/workflows/release.yml");
    let text = std::fs::read_to_string(&workflow).unwrap_or_else(|e| {
        panic!(
            "{} is unreadable ({e}) — VERSIONING.md promises CI publishes on tag patterns",
            workflow.display()
        )
    });

    assert!(
        text.contains("'cli/v*'") || text.contains("\"cli/v*\"") || text.contains("- cli/v*"),
        "release.yml must trigger on the `cli/v*` pattern VERSIONING.md's artifact table \
         documents for the yidam CLI"
    );

    // The template layer's bare `v*` would match `cli/v0.1.0` too under some globbing, but
    // more importantly it would fire this workflow on a template release. Neither layer may
    // publish the other's artifact — VERSIONING.md: "Never bump a layer as a side effect of
    // another layer's release."
    for line in text.lines() {
        let t = line
            .trim()
            .trim_start_matches("- ")
            .trim_matches(['\'', '"']);
        assert_ne!(
            t, "v*",
            "release.yml listens on the template layer's bare `v*`; it must publish the CLI \
             layer's `cli/v*` only"
        );
    }
}

/// The release must be built from the committed lock file.
///
/// Without `--locked`, cargo is free to update a dependency during the release build, so
/// the artifact is not the thing the tagged commit describes — and the pin a derived
/// repository resolves to that tag would name a build nobody can reproduce.
#[test]
fn the_release_build_is_locked() {
    let text = std::fs::read_to_string(repo_root().join(".github/workflows/release.yml"))
        .expect("release.yml");
    let builds: Vec<&str> = text
        .lines()
        .map(str::trim)
        .filter(|l| l.starts_with("cargo build"))
        .collect();
    assert!(!builds.is_empty(), "release.yml runs no cargo build");
    for b in &builds {
        assert!(
            b.contains("--locked"),
            "release build must pass --locked, got: {b}"
        );
    }
}

/// A registry named in Layer 2's table must be one something can actually publish to.
///
/// That table named crates.io, npm and PyPI from the day it was written, and for that whole
/// time none of the three packages was published anywhere. A registry column is a promise
/// about where a consumer obtains a package; three names and zero packages made it a
/// description of an intention, indistinguishable in shape from a description of the world.
///
/// So the check is not "is it published" — that is what the install-channel job asks, and it
/// can only ask after the fact. It is: does a publishing mechanism exist for every registry
/// this table names. Putting npm back in the table fails here until a workflow publishes to
/// npm, which is the order those two things have to happen in.
#[test]
fn every_registry_layer_2_names_has_something_that_publishes_to_it() {
    /// The registry as the table spells it, and the command that would publish to it.
    const PUBLISHERS: &[(&str, &str)] = &[
        ("crates.io", "cargo publish"),
        ("npm", "npm publish"),
        ("PyPI", "twine upload"),
    ];

    let text = versioning();
    let layer_2 = text
        .split("## Layer 2")
        .nth(1)
        .expect("VERSIONING.md has a Layer 2 section")
        .split("\n## ")
        .next()
        .unwrap();

    let workflow =
        std::fs::read_to_string(repo_root().join(".github/workflows/publish-crates.yml"))
            .expect("publish-crates.yml");

    let mut named = 0;
    for row in layer_2.lines().filter(|l| l.starts_with("| `")) {
        let registry = row
            .rsplit('|')
            .nth(1)
            .expect("a table row has a last cell")
            .trim();
        if registry == "not published" || registry == "Registry" {
            continue;
        }
        named += 1;
        let (_, publishes) = PUBLISHERS
            .iter()
            .find(|(name, _)| registry.contains(name))
            .unwrap_or_else(|| {
                panic!(
                    "Layer 2 names the registry '{registry}', which this test does not know \
                     how to check. Add it to PUBLISHERS with the command that publishes to it."
                )
            });
        assert!(
            workflow.contains(publishes),
            "Layer 2's table names {registry} and nothing runs `{publishes}` — either publish \
             to it or stop naming it. A registry column nobody delivers is a promise that \
             outlives the decision not to keep it."
        );
    }
    assert!(
        named > 0,
        "Layer 2's table names no registry at all; this test is guarding nothing"
    );
}
