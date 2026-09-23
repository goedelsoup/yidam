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

mod common;

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

/// The other end of that contract, which nothing was checking.
///
/// `format_version` is the whole reason the CLI and the editor clients can carry separate tags
/// (VERSIONING.md, Layer 4). It is written down once per consumer plus twice in documents:
/// `report.rs` declares it, `VERSIONING.md` quotes it, and each client's `handshake.ts` names
/// the major that build will parse. The test above pins the first two to each other — the two
/// *documents*. The places that act on the value were pinned to nothing.
///
/// That was survivable while the extension reached people only as a `.vsix` built from a
/// checkout, because the two constants were then the same checkout by construction. It stops
/// being survivable the moment a client publishes: separate artifacts, separate tags, separate
/// release cadences, and a reader who has one of each. #609 added the third consumer —
/// `@goedelsoup/yidam-edit`, which an `npx` user fetches from npm with no checkout at all.
///
/// The failure this prevents is quiet in the worst direction. `handshake.ts` degrades loudly
/// on a major it does not know — says so, disables verdict features rather than guessing. It
/// is *bumping* `FORMAT_VERSION` without bumping the clients that is silent: every published
/// client starts refusing every current binary, and the first report of it comes from a user.
///
/// Compared as majors, because that is what `handshake.ts` compares. A minor added to the
/// contract must not fail this — consumers are required to ignore fields they do not know,
/// and if an additive change broke the check, the check would be arguing for the opposite of
/// what Layer 4 says.
///
/// The consumers are **discovered** under `yidam/editors/`, not listed. A list here would have
/// gone on passing over two clients on the day a third was added, which is the whole of #609's
/// definition of done: a file-scanning check that looks at nothing passes.
#[test]
fn every_client_understands_the_contract_the_cli_speaks() {
    let major = |v: &str| v.split('.').next().unwrap_or_default().to_string();

    let report = std::fs::read_to_string(repo_root().join("yidam/cli/src/report.rs")).unwrap();
    let declared = report
        .split("pub const FORMAT_VERSION: &str = \"")
        .nth(1)
        .and_then(|t| t.split('"').next())
        .expect("report.rs declares FORMAT_VERSION");

    let consumers = format_version_consumers();
    assert!(
        consumers.len() >= 2,
        "found {} consumer(s) of SUPPORTED_FORMAT_VERSION under yidam/editors/; there have \
         been two since #609 landed the web editor, so this walk is reading the wrong tree \
         and the assertions below are vacuous: {consumers:?}",
        consumers.len()
    );

    for (path, understood) in &consumers {
        assert_eq!(
            major(declared),
            major(understood),
            "the CLI emits report contract {declared:?} and {path} parses {understood:?}. \
             Bumping FORMAT_VERSION strands every installed copy of that client on the old \
             major — update SUPPORTED_FORMAT_VERSION there and release its layer with it."
        );
    }
}

/// Every `SUPPORTED_FORMAT_VERSION` declared under `yidam/editors/`, with the file it is in.
///
/// Walks rather than lists, and matches on the declaration rather than on a filename, so a
/// client that puts it somewhere other than `handshake.ts` is still covered. `node_modules`
/// and build output are skipped — a stale copy of this project's own module inside a
/// dependency tree would answer for the source, and `dist/` is the source compiled — by
/// [`common::repo_walk`], which reads that set from `.gitignore` rather than naming it here.
fn format_version_consumers() -> Vec<(String, String)> {
    let root = repo_root();
    let mut out = Vec::new();
    for entry in common::repo_walk(&root.join("yidam/editors")) {
        let path = entry.path();
        if !entry.file_type().is_file() || !path.to_string_lossy().ends_with(".ts") {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(path) else {
            continue;
        };
        if let Some(v) = text
            .split("export const SUPPORTED_FORMAT_VERSION = '")
            .nth(1)
            .and_then(|t| t.split('\'').next())
        {
            let rel = path
                .strip_prefix(&root)
                .unwrap_or(path)
                .to_string_lossy()
                .to_string();
            out.push((rel, v.to_string()));
        }
    }
    out.sort();
    out
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

/// Every Layer 4 artifact must carry a version its registry can read.
///
/// The population is **read out of VERSIONING.md's Layer 4 table**, not listed here. This
/// test used to be `both_tooling_artifacts_declare_a_version` and named its two manifests
/// inline; #609 added a third row and the test went on passing without looking at it, which
/// is the shape `guard lists rot into holes` describes — a hardcoded set stops covering a new
/// member without ever going red. A fourth row is covered the moment it is written.
///
/// The manifest column is the join. It is already checked to resolve on disk by
/// `every_file_versioning_md_names_exists`; what is added here is that the file it names
/// declares something a registry can publish.
#[test]
fn every_tooling_artifact_declares_a_version() {
    let root = repo_root();
    let manifests = layer_4_manifests();
    assert!(
        manifests.len() >= 3,
        "the Layer 4 table yielded {} manifest(s); the table has had three rows since #609, \
         so this is parsing the wrong thing and every assertion below is vacuous: {manifests:?}",
        manifests.len()
    );

    for manifest in &manifests {
        let text = std::fs::read_to_string(root.join(manifest))
            .unwrap_or_else(|e| panic!("VERSIONING.md names {manifest} as a manifest: {e}"));
        // Two manifest formats, one question. `Cargo.toml` declares `version = "…"` at the
        // top level; `package.json` declares `"version": "…"`. Anchored on the line start
        // for the TOML so a dependency's `version = ` three tables down cannot answer for
        // the package's own.
        let declared = match manifest.rsplit('.').next() {
            Some("toml") => text.lines().any(|l| l.starts_with("version = \"")),
            _ => text.contains("\"version\":"),
        };
        assert!(
            declared,
            "{manifest} declares no version, so the registry VERSIONING.md names for it has \
             nothing to publish"
        );
    }
}

/// The `Manifest` column of VERSIONING.md's Layer 4 table.
///
/// Rows only — the header and the `|---|` separator are dropped by requiring the cell to
/// look like a path, which is also what keeps this from collecting the other layers' tables:
/// it reads the slice of the document between `## Layer 4` and the next `## `.
fn layer_4_manifests() -> Vec<String> {
    let doc = versioning();
    let from = doc
        .find("## Layer 4")
        .expect("VERSIONING.md has a Layer 4 section");
    let section = &doc[from..];
    let section = match section[3..].find("\n## ") {
        Some(end) => &section[..end + 3],
        None => section,
    };
    section
        .lines()
        .filter(|l| l.starts_with('|'))
        .filter_map(|l| l.split('|').nth(2))
        .map(|cell| cell.trim().trim_matches('`').to_string())
        .filter(|cell| cell.contains('/') && cell.contains('.'))
        .collect()
}

/// The MCP contract states its version twice, and the two must agree.
///
/// `tools.json`'s `contract` field is the live one — `serve --mcp` compiles it in and
/// returns it in the `yidam` capability block, and the E2E test asserts the server reports
/// what the file says. `mcp/VERSION` is read by nothing, which is the whole problem: a
/// version nobody reads is a version nobody notices going stale, and the next hand-bump has
/// even odds of touching one and not the other.
///
/// It lives here rather than in `mcp_serve.rs` for a reason that has since been fixed one
/// directory over: that file used to open `#![cfg(feature = "index")]`, so a check placed
/// there ran only in the full-feature job — main and the weekly schedule, never a pull
/// request — and would first have spoken up after the merge that broke it. `serve` is no
/// longer gated and neither is that file, so the argument no longer distinguishes them; this
/// one stays here because it is about the parity layer's versioning, not about the server.
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

    // Three times, in fact. `mcp/README.md` shows a worked capability block, and that block
    // is the first thing an implementer copies — it had drifted two minor versions behind
    // the file it documents before anything noticed, which is this test's own argument
    // playing out one directory over.
    let readme = std::fs::read_to_string(mcp.join("README.md")).unwrap();
    assert!(
        readme.contains(&format!(r#""contract": "{declared}""#)),
        "mcp/README.md's example capability block does not show contract {declared:?} — \
         an implementer copying it would conform to a version that no longer exists"
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

/// No publishing workflow answers to another layer's tag.
///
/// Same reasoning as the CLI check above, generalised, and the case for it got sharper with
/// every layer added. Five patterns in this repository end in `v*` and four of them are
/// prefixed; `release.sh` is the only place that knows which belongs to which layer, and it
/// knows it as a string. A workflow listening on the wrong one publishes the wrong artifact
/// on someone else's release, and every spelling involved looks correct in a diff.
///
/// `edit/v*` and `editor/v*` are the pair that makes this non-negotiable: four characters
/// apart, disjoint as glob patterns, and both correct-looking. Crossing them is silent in
/// both directions — a workflow that fires on the wrong tag publishes nothing and reads
/// exactly like one that has not finished.
///
/// The inverse matters as much. A CLI patch must not imply an editor release and an editor
/// patch must not imply a CLI one; that independence is the whole reason Layer 4 carries
/// three tags for one layer.
#[test]
fn every_publishing_workflow_triggers_on_its_own_tag_pattern_only() {
    let layers = layer_tag_patterns();
    assert!(
        layers.len() >= 5,
        "release.sh yielded {} layer(s); it has had five since #609 — this is parsing the \
         wrong thing and every assertion below is vacuous: {layers:?}",
        layers.len()
    );

    // Every tag pattern any layer publishes on, so "listens on someone else's" is answerable
    // without naming the someone else. `edit/v*` and `editor/v*` are the pair this matters
    // most for: four characters apart, and a workflow that listened on the wrong one would
    // publish nothing and look exactly like one that had not finished.
    let patterns: Vec<String> = layers.iter().map(|(p, _)| p.clone()).collect();

    let mut checked = 0;
    // Which layers were actually *heard* — a pattern some workflow of that layer listens
    // on. Counting graded triggers is not the same question: a scanner that quietly stopped
    // reading one workflow's `tags:` list would still clear a count floor on the others,
    // and the layer it stopped reading would be unguarded without ever going red.
    let mut heard_by: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for (pattern, workflows) in &layers {
        for workflow in workflows {
            let path = repo_root().join(workflow);
            let text = std::fs::read_to_string(&path).unwrap_or_else(|e| {
                panic!(
                    "{workflow} is unreadable ({e}) — release.sh names it for the {pattern} \
                     layer, and refuses to tag without it"
                )
            });

            // A workflow a layer requires need not be tag-triggered at all: `tap.yml` and
            // `publish-crates.yml` are required by `cli`, and `publish-crates.yml` is shared
            // with `sdk/rust`. What is checked is the contrapositive — a workflow that
            // listens on tags at all must not listen on a pattern that is not one of the
            // layers requiring it.
            let listens: Vec<String> = tag_patterns_listened_for(&text);
            if listens.is_empty() {
                continue;
            }

            let mine: Vec<&String> = layers
                .iter()
                .filter(|(_, ws)| ws.iter().any(|w| w == workflow))
                .map(|(p, _)| p)
                .collect();

            for heard in &listens {
                // Only patterns some layer actually publishes on are graded. A workflow may
                // listen on anything else it likes; what it may not do is fire on another
                // layer's release.
                if !patterns.contains(heard) {
                    continue;
                }
                assert!(
                    mine.contains(&heard),
                    "{workflow} triggers on {heard:?}, which belongs to another layer. \
                     release.sh names this workflow for {mine:?} only, so a release of that \
                     other layer would start it — and a workflow that publishes the wrong \
                     artifact, or nothing, is indistinguishable from one still running."
                );
                checked += 1;
                heard_by.insert(heard.to_string());
            }
        }
    }
    assert!(
        checked >= 3,
        "graded only {checked} tag trigger(s); release.sh names five layers and at least \
         three of them have a tag-triggered workflow, so this walk is reading nothing"
    );

    // Per layer, not a floor. `edit/v*` is the newest of these and would be the first to
    // fall out of the scan; a layer that publishes through a workflow and is heard by none
    // of them is either mis-triggered or invisible to this test, and those are the same
    // failure from here.
    for (pattern, workflows) in &layers {
        if workflows.is_empty() {
            continue;
        }
        assert!(
            heard_by.contains(pattern),
            "release.sh releases {pattern:?} through {workflows:?}, and no tag trigger in \
             any of them was read as {pattern:?}. Either nothing fires on that tag, or this \
             scan stopped seeing that workflow's `tags:` list — which looks identical from \
             here and leaves the layer ungraded."
        );
    }
}

/// `(tag pattern, the workflows release.sh requires at HEAD)` for every layer the script
/// knows, read out of its own `case` branches.
///
/// Derived rather than listed. This test used to be
/// `the_editor_workflow_triggers_on_the_documented_tag_pattern` and named `editor.yml` and
/// `editor/v*` inline; when #609 added a fifth layer whose pattern is `editor/v*` minus two
/// characters it went on passing over it, which is the hole a hardcoded population always
/// has. `release.sh` is the right source because it is the only file that must already agree
/// with both VERSIONING.md (`every_layer_versioning_md_names_is_one_the_script_can_release`)
/// and the filesystem (`every_workflow_release_sh_requires_exists`).
fn layer_tag_patterns() -> Vec<(String, Vec<String>)> {
    let script = std::fs::read_to_string(repo_root().join("release.sh")).expect("release.sh");
    let mut out = Vec::new();
    for line in script.lines() {
        let Some(rest) = line.split("TAG=\"").nth(1) else {
            continue;
        };
        let Some(tag) = rest.split('"').next() else {
            continue;
        };
        // `cli/v$VERSION` → `cli/v*`, which is the form a workflow's `tags:` list spells.
        let Some(prefix) = tag.strip_suffix("$VERSION") else {
            continue;
        };
        let workflows: Vec<String> = line
            .split("WORKFLOWS=\"")
            .nth(1)
            .and_then(|c| c.split('"').next())
            .unwrap_or_default()
            .split_whitespace()
            .map(str::to_string)
            .collect();
        out.push((format!("{prefix}*"), workflows));
    }
    out
}

/// The tag patterns a workflow's `on: push: tags:` list names.
///
/// Comment lines are stripped first. Every one of these workflows explains in prose which
/// patterns it is *not* listening on — editor.yml's trigger comment says "NOT `v*` … and NOT
/// `cli/v*`" — and a scan over the raw text reads those as triggers, which would make this
/// test fail on the files that are most careful. `prose answers for code` is the lesson;
/// here it would have answered in the wrong direction.
fn tag_patterns_listened_for(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut in_tags = false;
    for line in text.lines() {
        let raw = line.trim_end();
        let trimmed = raw.trim();
        if trimmed.starts_with('#') || trimmed.is_empty() {
            continue;
        }
        if trimmed.starts_with("tags:") {
            in_tags = true;
            continue;
        }
        if !in_tags {
            continue;
        }
        let Some(item) = trimmed.strip_prefix("- ") else {
            // The list ended at the first non-item line.
            in_tags = false;
            continue;
        };
        out.push(item.trim().trim_matches(['\'', '"']).to_string());
    }
    out
}

/// `release.sh` names a workflow file per layer, and refuses a tag whose workflow is absent
/// at HEAD. That refusal is only as good as the paths it names.
///
/// The failure it guards against has already happened once here: `publish-crates.yml` was
/// committed AFTER the only tag this repository had, and a workflow fires only if it exists
/// at the ref being pushed — so it had never run, and nothing looked like it was wrong. The
/// script now refuses that case. A typo in one of these paths would restore it silently,
/// because a path that resolves to nothing and a workflow that does not exist produce the
/// same refusal, and the refusal is the thing a person is trying to get past.
#[test]
fn every_workflow_release_sh_requires_exists() {
    let script = std::fs::read_to_string(repo_root().join("release.sh")).expect("release.sh");
    let mut checked = 0;
    for chunk in script.split("WORKFLOWS=\"").skip(1) {
        let Some(list) = chunk.split('"').next() else {
            continue;
        };
        for path in list.split_whitespace() {
            assert!(
                repo_root().join(path).is_file(),
                "release.sh requires {path} for a layer's tag, and it does not exist — the \
                 tag would be refused with `no-workflow` and the path is the reason"
            );
            checked += 1;
        }
    }
    assert!(
        checked >= 4,
        "found only {checked} required workflows — the scan is broken"
    );
}

/// Every registry Layer 4 names is one this project actually publishes to AND checks.
///
/// The Layer 2 version of this asserts a publisher exists. This asserts two things, because
/// #231's whole finding was that a publisher is not enough: six channels existed, two had
/// delivered, and nothing anywhere tested whether a person could obtain what was published.
/// A registry named here is read as a promise, so it must have a path that publishes to it
/// and a check that asks it what it serves.
///
/// It is what makes `editor.yml`'s Marketplace step honest in skipping. That step notices and
/// exits 0 on a missing `VSCE_PAT` — acceptable only while nothing claims the Marketplace, and
/// a permanently red release step otherwise. Putting the row back without the publish path and
/// the channel check fails here, so the three cannot drift apart in either direction.
#[test]
fn the_registries_layer_4_names_are_delivered_and_checked() {
    /// Registry as the table spells it → (what publishes to it, what asks what it serves).
    ///
    /// Both are substrings of files under `.github/workflows/`. Deliberately the *command*
    /// and the *host*, not a job name: a job can be renamed and a workflow reorganised, but
    /// nothing publishes to Open VSX without `ovsx publish` and nothing asks the Marketplace
    /// what it serves without naming its host.
    const CHANNELS: &[(&str, &str, &str)] = &[
        ("crates.io", "cargo publish", "cargo install --locked yidam"),
        ("GitHub releases", "gh release create", "install.sh"),
        (
            "`goedelsoup/homebrew-tap`",
            "Formula/yidam.rb",
            "brew install",
        ),
        ("Open VSX", "ovsx publish", "open-vsx.org"),
        (
            "VS Code Marketplace",
            "vsce publish",
            "marketplace.visualstudio.com",
        ),
        // Layer 4's third artifact (#609). The publish string is the command with its
        // argument, not a bare `npm publish`: `npm publish` alone would be satisfied by a
        // workflow that repacks from source, and what has to exist is the step that uploads
        // the tarball `check-package.mjs` graded and ran.
        (
            "npm",
            "npm publish dist/yidam-edit.tgz",
            "registry.npmjs.org",
        ),
    ];

    let text = versioning();
    let layer_4 = text
        .split("## Layer 4")
        .nth(1)
        .expect("VERSIONING.md has a Layer 4 section")
        .split("\n## ")
        .next()
        .unwrap();

    let workflows = repo_root().join(".github/workflows");
    let all: String = std::fs::read_dir(&workflows)
        .expect("the workflows directory")
        .filter_map(|e| std::fs::read_to_string(e.ok()?.path()).ok())
        .collect();
    let checks = std::fs::read_to_string(workflows.join("install-channels.yml"))
        .expect("install-channels.yml");

    let mut named = 0;
    for row in layer_4.lines().filter(|l| l.starts_with("| `")) {
        let cell = row
            .rsplit('|')
            .nth(1)
            .expect("a table row has a last cell")
            .trim();
        if cell == "Registry" {
            continue;
        }
        for registry in cell.split(", ").map(str::trim).filter(|r| !r.is_empty()) {
            let (_, publishes, checked) = CHANNELS
                .iter()
                .find(|(name, _, _)| *name == registry)
                .unwrap_or_else(|| {
                    panic!(
                        "Layer 4 names the registry {registry:?}, which this test does not \
                         know how to verify. Add it to CHANNELS with the command that \
                         publishes to it and the string that checks it — or stop naming a \
                         registry nothing delivers to."
                    )
                });
            assert!(
                all.contains(publishes),
                "Layer 4 names {registry:?} as a registry this project publishes to, and no \
                 workflow contains {publishes:?}. Ship the channel, or stop naming it."
            );
            assert!(
                checks.contains(checked),
                "Layer 4 names {registry:?} and install-channels.yml never asks what it \
                 serves. A channel with a publisher and no check is exactly #231: the \
                 artifact builds and nobody has asked whether it can be obtained."
            );
            named += 1;
        }
    }
    assert!(
        named >= 6,
        "found only {named} registries in Layer 4 — the scan is broken"
    );
}

/// The extension id `VERSIONING.md` names is the one the manifest produces.
///
/// A Marketplace id is `<publisher>.<name>`, assembled from two fields rather than declared,
/// which is why it can be documented wrong without anything looking wrong. Layer 4's table
/// said `goedelsoup.yidam` while the manifest said `yidam-vscode` — a mismatch found by
/// writing the channel check in install-channels.yml, which queries the registries by that
/// exact string and would have reported "the Marketplace does not know this extension" for a
/// correctly published one.
///
/// It is the id a person types into `code --install-extension`, and the id the channel check
/// asks both registries about. Two consumers, one string, assembled from two fields nobody
/// reads together.
#[test]
fn the_documented_extension_id_is_the_one_the_manifest_makes() {
    let manifest = std::fs::read_to_string(repo_root().join("yidam/editors/vscode/package.json"))
        .expect("the extension manifest");
    let field = |key: &str| -> String {
        manifest
            .split(&format!("\"{key}\":"))
            .nth(1)
            .and_then(|t| t.split('"').nth(1))
            .unwrap_or_else(|| panic!("package.json declares {key}"))
            .to_string()
    };
    let id = format!("{}.{}", field("publisher"), field("name"));

    assert!(
        versioning().contains(&id),
        "VERSIONING.md's Layer 4 table does not name {id:?}, which is what publisher and \
         name in the extension manifest assemble to — and what the registries are queried by"
    );
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
