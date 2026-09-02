//! What "the light build" is, said once.
//!
//! `reports` is a feature name with a description that reads like a build instruction — the
//! four reports plus every pure-Rust command, no protoc, no C library, no ML runtime — and
//! the build it invites, `--no-default-features --features reports`, fails seven of its own
//! tests. The light build is the **default set**, and the difference had no home: it was
//! recorded in a comment on an unrelated mise task, which is where #482 eventually found it.
//!
//! Two failures follow from having no home, and this file guards against both.
//!
//! **The set gets restated and the restatements go stale.** `install.sh` and `mise.toml` both
//! glossed the light build as "(`reports` + `tonpa`)". That was right until `vault-s3` joined
//! `default`; then it was two wrong sentences, and nothing went red, because nothing was
//! looking. `install_channels.rs` had already reached the rule this enforces — *"the default
//! IS the light set, and spelling it in either place makes a second definition for the two to
//! drift apart on"* — for the one file it could see.
//!
//! **The report's list stops keeping up.** `report_goldens.rs` used to assert that the light
//! build names exactly one feature. It stayed true by retreating: when `tonpa` joined the
//! default set it was added to a `cfg(not(any(…)))` so the test would stop running, rather
//! than to the expectation. When `vault-s3` joined, nobody added it — and the test went on
//! passing in builds no CI compiles. The assertion here does not retreat, because it is
//! derived from `[features]` rather than written down beside it.

use std::collections::BTreeSet;
use std::path::PathBuf;
use walkdir::WalkDir;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn manifest() -> String {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{} unreadable: {e}", p.display()))
}

/// Names that stand for other features rather than for a capability.
///
/// A report listing `full` would tell a consumer nothing it can act on — `full` is not a
/// thing the binary can or cannot do — so `report.rs` names the constituents instead, and
/// this file must not require it to name the aggregates.
const AGGREGATE: &[&str] = &["default", "full"];

/// Every key in `[features]`, as written.
fn declared_features() -> BTreeSet<String> {
    let toml = manifest();
    let head = "\n[features]\n";
    let start = toml
        .find(head)
        .expect("yidam/cli/Cargo.toml has no [features] table");
    let rest = &toml[start + head.len()..];
    let end = rest.find("\n[").unwrap_or(rest.len());
    let mut names = BTreeSet::new();
    for line in rest[..end].lines() {
        let line = line.trim_start();
        if line.starts_with('#') {
            continue;
        }
        if let Some((key, _)) = line.split_once(" = ") {
            if is_feature_shaped(key) {
                names.insert(key.to_string());
            }
        }
    }
    assert!(
        names.len() >= 5,
        "only {} features parsed out of [features]; the parser is reading the wrong thing \
         and every assertion built on it is vacuous: {names:?}",
        names.len()
    );
    names
}

/// The feature names in one `[features]` entry's list body.
fn feature_body(name: &str) -> BTreeSet<String> {
    let toml = manifest();
    let key = format!("\n{name} = [");
    let start = toml
        .find(&key)
        .unwrap_or_else(|| panic!("no `{name}` feature in Cargo.toml"));
    let rest = &toml[start + key.len()..];
    let end = rest.find(']').expect("unterminated feature list");
    quoted(&rest[..end])
}

/// A name Cargo would accept for a feature: lowercase, digits, dashes.
fn is_feature_shaped(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// Every `"…"` literal in a fragment.
fn quoted(s: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let mut rest = s;
    while let Some(open) = rest.find('"') {
        rest = &rest[open + 1..];
        let Some(close) = rest.find('"') else { break };
        out.insert(rest[..close].to_string());
        rest = &rest[close + 1..];
    }
    out
}

/// Source with `//`-comments removed, so prose about a feature cannot answer for code.
fn code_only(src: &str) -> String {
    src.lines()
        .map(|l| l.split("//").next().unwrap_or(""))
        .collect::<Vec<_>>()
        .join("\n")
}

// ── the default set is the light build ────────────────────────────────────────

/// The light build is `default`, and `default` is what the binary is actually built from.
///
/// Not a restatement of the manifest: it asserts that the two features whose absence makes
/// `--features reports` fail are the ones `default` carries, which is the claim the note on
/// `reports` in Cargo.toml makes to a reader who is deciding how to build.
#[test]
fn the_default_set_carries_what_the_light_build_promises() {
    let default = feature_body("default");
    for required in ["reports", "tonpa", "vault-s3"] {
        assert!(
            default.contains(required),
            "`{required}` left the default set. If that is deliberate, the note on the \
             `reports` feature in Cargo.toml is now wrong, and so is every place that calls \
             the default build the light one: {default:?}"
        );
    }
}

/// `reports` gates nothing, which is the load-bearing half of the note in Cargo.toml.
///
/// If a `#[cfg(feature = "reports")]` ever appears, naming the feature starts to change what
/// compiles — and then `report.rs` listing it unconditionally becomes a lie about the build,
/// and `--no-default-features` becomes a fourth thing a reader can mean by "light".
#[test]
fn naming_reports_changes_nothing_that_compiles() {
    assert!(
        manifest().contains("\nreports = []\n"),
        "`reports` now enables something; it is documented as a name for the base build"
    );

    let src = repo_root().join("yidam/cli/src");
    let mut gated = Vec::new();
    let mut scanned = 0;
    for entry in WalkDir::new(&src).into_iter().filter_map(Result::ok) {
        if !entry.file_type().is_file() || entry.path().extension() != Some("rs".as_ref()) {
            continue;
        }
        scanned += 1;
        let text = std::fs::read_to_string(entry.path()).expect("source is readable");
        for (i, line) in code_only(&text).lines().enumerate() {
            if line.contains("feature = \"reports\"") {
                let rel = entry
                    .path()
                    .strip_prefix(&src)
                    .unwrap()
                    .display()
                    .to_string();
                gated.push(format!("  src/{rel}:{}", i + 1));
            }
        }
    }
    assert!(
        scanned > 40,
        "only {scanned} source files scanned; this test is looking at the wrong tree"
    );
    assert!(
        gated.is_empty(),
        "`reports` is documented as gating nothing, and now gates code:\n{}",
        gated.join("\n")
    );
}

// ── the report names the build ────────────────────────────────────────────────

/// `YidamBlock::current`'s body, so the scan sees the list and not the whole module.
fn current_body() -> String {
    let p = repo_root().join("yidam/cli/src/report.rs");
    let src =
        std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{} unreadable: {e}", p.display()));
    let src = code_only(&src);
    let open = "pub fn current() -> Self {";
    let start = src
        .find(open)
        .expect("report.rs no longer has `pub fn current() -> Self` — this scan reads nothing");
    let rest = &src[start + open.len()..];
    let end = rest
        .find("\n    }")
        .expect("could not find the end of `current`'s body");
    rest[..end].to_string()
}

/// Every feature the binary can be built with is a feature its report can name.
///
/// This is the hole `vault-s3` went through in `report_goldens.rs` and nearly went through
/// here: a new feature is added, `report.rs` is not told, and the binary silently under-claims
/// to every consumer that reads the block to tell "this build cannot do that" from "that
/// failed". Derived from `[features]`, so adding a feature is what fails it.
#[test]
fn the_report_can_name_every_feature_and_no_others() {
    let declared: BTreeSet<String> = declared_features()
        .into_iter()
        .filter(|f| !AGGREGATE.contains(&f.as_str()))
        .collect();
    let named: BTreeSet<String> = quoted(&current_body())
        .into_iter()
        .filter(|s| is_feature_shaped(s))
        .collect();

    let unnameable: Vec<_> = declared.difference(&named).collect();
    assert!(
        unnameable.is_empty(),
        "these features exist and `YidamBlock::current` never names them, so a build \
         carrying one reports less than it has: {unnameable:?}"
    );
    let invented: Vec<_> = named.difference(&declared).collect();
    assert!(
        invented.is_empty(),
        "`YidamBlock::current` names features Cargo.toml does not declare — a renamed or \
         mistyped feature reports as permanently absent: {invented:?}"
    );
}

// ── nothing restates the set ──────────────────────────────────────────────────

/// The feature names in each parenthetical that glosses the default set.
///
/// Deliberately narrow: a parenthetical containing two or more backticked names, *all* of
/// which are features of this crate, introduced within the preceding stretch of text by the
/// word "default". That is the exact shape both stale glosses had, and each of the three
/// conditions is what keeps it from fighting ordinary prose. This repository backticks names
/// constantly — `reqwest` and `aws-lc-sys` in one dependency note, `http` and `time` in the
/// regorus one — and a rule that graded every enumeration would be a rule that has to be
/// argued with. Each condition is pinned by a fixture below.
///
/// The scan reads the whole text rather than a line at a time. Comment leaders are left
/// alone: a `#` or `//!` inside a parenthetical disturbs neither the backtick scan nor the
/// lookbehind, and stripping them was code with nothing depending on it.
fn glosses_of_the_default_set(text: &str, declared: &BTreeSet<String>) -> Vec<BTreeSet<String>> {
    let mut found = Vec::new();
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] != '(' {
            i += 1;
            continue;
        }
        let Some(close) = chars[i..].iter().position(|c| *c == ')') else {
            break;
        };
        let inside: String = chars[i + 1..i + close].iter().collect();
        let lead_from = i.saturating_sub(60);
        let lead: String = chars[lead_from..i].iter().collect();

        let names: BTreeSet<String> = ticked(&inside)
            .into_iter()
            .filter(|n| declared.contains(n))
            .collect();
        let all_ticked_are_features = ticked(&inside).len() == names.len();

        if names.len() >= 2 && all_ticked_are_features && lead.to_lowercase().contains("default") {
            found.push(names);
        }
        i += close + 1;
    }
    found
}

/// Every `` `…` `` token in a fragment.
fn ticked(s: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let mut rest = s;
    while let Some(open) = rest.find('`') {
        rest = &rest[open + 1..];
        let Some(close) = rest.find('`') else { break };
        out.insert(rest[..close].to_string());
        rest = &rest[close + 1..];
    }
    out
}

/// The detector fires on the sentence that was wrong for a release cycle.
///
/// Without this the scan below could be looking at nothing and would still pass — which is
/// how the two glosses survived in the first place.
#[test]
fn the_detector_sees_the_gloss_that_went_stale() {
    let declared = declared_features();
    let historical = "# Downloads the light default build (`reports` + `tonpa`) for this\n\
                      # platform from the latest `cli/v*` release.";
    let found = glosses_of_the_default_set(historical, &declared);
    assert_eq!(
        found.len(),
        1,
        "the detector no longer sees the gloss it exists for: {found:?}"
    );
    assert_ne!(
        found[0],
        feature_body("default"),
        "this fixture is the *stale* gloss; if it now equals the default set the fixture \
         has to be replaced with one that is actually wrong"
    );

    // A parenthetical that spans lines. This is the case a line-at-a-time scan cannot see,
    // and the reason the scan runs over the whole text.
    let wrapped = "# The light default set is\n# (`reports`, `tonpa`\n#  and `vault-s3`)\n";
    assert_eq!(
        glosses_of_the_default_set(wrapped, &declared).len(),
        1,
        "a gloss whose parenthetical wraps must still be seen"
    );

    // Prose that names things which are not features of this crate is not a gloss, however
    // close to the word "default" it sits. Without this the note on `regorus` in Cargo.toml
    // would be graded as a claim about the light build.
    let innocent = "# `default-features = false` is load-bearing: the default set is \
                    `full-opa`, which enables the (`http` and `time`) builtins.";
    assert!(
        glosses_of_the_default_set(innocent, &declared).is_empty(),
        "the detector is firing on names that are not features of this crate"
    );

    // And an enumeration of real features that is not introduced as the default set is not a
    // gloss either — otherwise every sentence listing two features would be required to list
    // all three of them.
    let unrelated = "The two export formats (`export-sqlite` and `export-graph`) are \
                     source builds.";
    assert!(
        glosses_of_the_default_set(unrelated, &declared).is_empty(),
        "the detector is grading enumerations that make no claim about the default set"
    );
}

/// No file in the repository glosses the default set except the manifest that defines it.
#[test]
fn nothing_restates_what_the_default_set_is() {
    let declared = declared_features();
    let default = feature_body("default");
    let root = repo_root().canonicalize().expect("repo root is readable");

    let mut offenders = Vec::new();
    let mut scanned = 0;
    for entry in WalkDir::new(&root)
        .into_iter()
        .filter_entry(|e| {
            let name = e.file_name().to_string_lossy();
            e.depth() == 0 || !(name.starts_with('.') || name == "target" || name == "node_modules")
        })
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
    {
        let name = entry.file_name().to_string_lossy().to_lowercase();
        if !["md", "mdx", "sh", "toml", "yml", "yaml", "rs"]
            .iter()
            .any(|ext| name.ends_with(&format!(".{ext}")))
        {
            continue;
        }
        // This file carries the stale gloss as a fixture, on purpose.
        if name == "light_build.rs" {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(entry.path()) else {
            continue;
        };
        scanned += 1;
        let rel = entry
            .path()
            .strip_prefix(&root)
            .unwrap()
            .display()
            .to_string();
        for gloss in glosses_of_the_default_set(&text, &declared) {
            if gloss != default {
                offenders.push(format!(
                    "  {rel} — says {gloss:?}, `default` is {default:?}"
                ));
            }
        }
    }

    assert!(
        scanned > 100,
        "only {scanned} files scanned; the walk is not reaching the repository"
    );
    assert!(
        offenders.is_empty(),
        "these restate the default feature set, and one of them will be wrong the next time \
         it changes — say `the default set` and let Cargo.toml define it:\n{}",
        offenders.join("\n")
    );
}

// ── the per-row `*(default)*` markers (#532) ─────────────────────────────────
//
// `nothing_restates_what_the_default_set_is` catches a *gloss* — prose naming the set. It
// does not catch a feature table that marks its rows one at a time, and both
// `docs/installation.md` and `README.md` do. Both were wrong: `vault-s3` joined `default` and
// appeared in neither table, so a reader saw a light build of `reports + tonpa` and had done
// since #482. Exactly the drift `light_build.rs`'s own header describes, in the one shape its
// detector was not looking for.
//
// Both sides discovered: the marked rows come from walking the documents, the truth from the
// manifest. Neither is a list in this file.

/// A feature table row: the feature it names, and whether it is marked as default.
fn marked_rows(text: &str) -> Vec<(String, bool)> {
    text.lines()
        .filter(|l| l.trim_start().starts_with('|'))
        .filter_map(|l| {
            let first = l.trim_start().trim_start_matches('|').split('|').next()?;
            let name = first.split('`').nth(1)?.trim().to_string();
            Some((name, first.contains("*(default)*")))
        })
        .collect()
}

/// Every feature table agrees with the manifest about which features are default.
#[test]
fn a_feature_table_marks_exactly_the_default_features() {
    let default = feature_body("default");
    let declared = declared_features();
    let mut problems: Vec<String> = Vec::new();
    let mut tables = 0;

    for doc in ["README.md", "docs/installation.md"] {
        let path = repo_root().join(doc);
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("{} unreadable: {e}", path.display()));
        let rows: Vec<(String, bool)> = marked_rows(&text)
            .into_iter()
            .filter(|(name, _)| declared.contains(name))
            .collect();
        // A document with no feature rows is not a document this test is about; one with a
        // handful is, and losing them silently is how the check stops checking.
        if rows.len() < 3 {
            continue;
        }
        tables += 1;

        for (name, marked) in &rows {
            match (default.contains(name), marked) {
                (true, false) => problems.push(format!(
                    "  {doc}: `{name}` is in the default set and its row is not marked \
                     *(default)*"
                )),
                (false, true) => problems.push(format!(
                    "  {doc}: `{name}` is marked *(default)* and is not in the default set"
                )),
                _ => {}
            }
        }

        // And the other direction: a default feature with no row at all is the way
        // `vault-s3` went missing — a table cannot mark a row it does not have.
        let listed: BTreeSet<&String> = rows.iter().map(|(n, _)| n).collect();
        for f in &default {
            if !listed.contains(f) {
                problems.push(format!(
                    "  {doc}: `{f}` is in the default set and the table has no row for it"
                ));
            }
        }
    }

    assert!(
        tables >= 2,
        "{tables} feature table(s) found; this test is reading the wrong documents or the \
         row parser stopped recognising one"
    );
    assert!(
        problems.is_empty(),
        "{} feature table row(s) disagree with `[features] default`:\n{}\n\n\
         The default set is what `cargo install yidam` resolves and what the release ships. \
         A table that names a different one sends a reader to build something else.",
        problems.len(),
        problems.join("\n")
    );
}
