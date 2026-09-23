//! A dependency that must not move, and the thing that stops it.
//!
//! `.github/dependabot.yml` carries four `ignore` entries. #668 gave the fourth
//! (`dtolnay/rust-toolchain`) a gate in `toolchain_pins.rs`; #904 is the other three —
//! `fastembed`, the three `arrow-*`, and `@types/vscode` — each held by nothing but the
//! comment above it. The fastembed block says why that is not enough, about its own earlier
//! self: *"That argument was written in a comment one line above the dependency, and a bump
//! took the dependency to 6 anyway — comments are not a mechanism."* Moving the comment from
//! above the dependency to above the `ignore` is a better place for it and the same mechanism.
//!
//! **This file does not gate the `ignore` entries.** Asserting one is present would make
//! dependabot's configuration the thing that holds the constraint, and dependabot is not a
//! gate: it proposes, CI decides. Each test here reads the constraint's *own* two sides and
//! compares them, so a bump that breaks one is red whether it arrived from the `actions` group,
//! from a person, or from a merge. The `ignore` then stops being load-bearing and becomes what
//! it is good at — not opening the pull request every Monday.
//!
//! The one thing gated about the configuration itself is that it is not stale: see
//! [`every_dependabot_ignore_names_something_that_exists`]. An `ignore` for a dependency
//! nobody declares any more reads as a hold and holds nothing.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn read(rel: &str) -> String {
    let p = repo_root().join(rel);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{} unreadable: {e}", p.display()))
}

/// The major of a version or a range: `~1.90.0`, `^4`, `53.4.1` → `1`, `4`, `53`.
fn major(spec: &str) -> u64 {
    let digits: String = spec
        .trim_start_matches(['~', '^', '=', '>', '<', 'v', ' '])
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    digits
        .parse()
        .unwrap_or_else(|_| panic!("`{spec}` has no leading version number"))
}

/// `major.minor` of a version or a range, as a pair.
fn major_minor(spec: &str) -> (u64, u64) {
    let cleaned = spec.trim_start_matches(['~', '^', '=', '>', '<', 'v', ' ']);
    let mut parts = cleaned.split('.');
    let maj = major(cleaned);
    let min = parts
        .nth(1)
        .and_then(|p| {
            p.chars()
                .take_while(char::is_ascii_digit)
                .collect::<String>()
                .parse()
                .ok()
        })
        .unwrap_or_else(|| panic!("`{spec}` names no minor version"));
    (maj, min)
}

// ── the lockfile ──────────────────────────────────────────────────────────────

/// One `[[package]]` stanza of a `Cargo.lock`.
struct Locked {
    version: String,
    dependencies: Vec<String>,
}

/// Every package in `yidam/cli/Cargo.lock`, by name, to the versions resolved for it.
///
/// A name with more than one entry is a duplicated crate: two majors of the same library
/// compiled into one binary, which for arrow is the failure this file is about.
fn locked_packages() -> BTreeMap<String, Vec<Locked>> {
    let lock: toml::Value =
        toml::from_str(&read("yidam/cli/Cargo.lock")).expect("yidam/cli/Cargo.lock is not TOML");
    let mut out: BTreeMap<String, Vec<Locked>> = BTreeMap::new();
    for pkg in lock["package"]
        .as_array()
        .expect("Cargo.lock declares no packages")
    {
        let name = pkg["name"].as_str().expect("a package with no name");
        let version = pkg["version"].as_str().expect("a package with no version");
        let dependencies = pkg
            .get("dependencies")
            .and_then(|d| d.as_array())
            .map(|d| {
                d.iter()
                    .filter_map(|v| v.as_str())
                    // `"name"` when one version is in the graph, `"name version"` when
                    // several are. Either way the first field is the name.
                    .map(|s| s.split_whitespace().next().unwrap_or(s).to_string())
                    .collect()
            })
            .unwrap_or_default();
        out.entry(name.to_string()).or_default().push(Locked {
            version: version.to_string(),
            dependencies,
        });
    }
    out
}

/// The version `yidam/cli/Cargo.toml` declares for an optional dependency, if it declares one.
fn declared(dep: &str) -> Option<String> {
    let manifest: toml::Value =
        toml::from_str(&read("yidam/cli/Cargo.toml")).expect("yidam/cli/Cargo.toml is not TOML");
    let entry = manifest.get("dependencies")?.get(dep)?;
    match entry {
        toml::Value::String(s) => Some(s.clone()),
        other => other.get("version")?.as_str().map(str::to_string),
    }
}

// ── arrow must be lancedb's arrow ─────────────────────────────────────────────

/// One arrow, not two.
///
/// `Cargo.toml` states the rule one line above the declarations — *"Versions must match
/// lancedb's transitive dependency — update together if bumping either"* — and nothing
/// compared them. A bump moved arrow 53 → 54 and left lancedb at 0.15, which pins 53, so
/// `index_build` handed a lancedb builder a `RecordBatchIterator` of the wrong arrow and
/// `IntoArrow` went unsatisfied.
///
/// The lockfile is where the two sides meet, and it says so in its own notation: cargo writes
/// a bare `"arrow-array"` in a dependency list while one version is in the graph and
/// `"arrow-array 54.0.0"` once there are two. So the assertion is that each arrow crate this
/// workspace declares resolves to exactly one package, and that lancedb is a dependent of it.
/// Under that, the majors cannot disagree — there is only one major to have.
///
/// Discovered from the manifest, so it covers the fourth arrow crate somebody adds.
#[test]
fn every_arrow_this_workspace_declares_is_the_one_lancedb_resolves() {
    let manifest: toml::Value =
        toml::from_str(&read("yidam/cli/Cargo.toml")).expect("yidam/cli/Cargo.toml is not TOML");
    let deps = manifest["dependencies"]
        .as_table()
        .expect("yidam/cli/Cargo.toml declares no dependencies");
    let arrow_crates: Vec<String> = deps
        .keys()
        .filter(|k| k.starts_with("arrow"))
        .cloned()
        .collect();
    assert!(
        !arrow_crates.is_empty(),
        "yidam/cli/Cargo.toml declares no arrow crate. If the vector index stopped decoding \
         arrow this test is obsolete; while it decodes arrow, this is reading the wrong manifest."
    );

    let packages = locked_packages();
    let lancedb = packages
        .get("lancedb")
        .and_then(|v| v.first())
        .expect("Cargo.lock resolves no lancedb, and the arrow versions exist to match it");

    for krate in &arrow_crates {
        let resolved = packages.get(krate).unwrap_or_else(|| {
            panic!("yidam/cli/Cargo.toml declares `{krate}` and Cargo.lock resolves no such crate")
        });
        let versions: Vec<&str> = resolved.iter().map(|l| l.version.as_str()).collect();
        assert_eq!(
            versions.len(),
            1,
            "Cargo.lock resolves {} versions of `{krate}`: {versions:?}. Two arrow majors are \
             compiled into one binary, so a `RecordBatchIterator` built from one does not \
             satisfy the other's `IntoArrow` — which is how `index_build` stopped compiling \
             when arrow went to 54 against lancedb 0.15. A major arrow bump is only ever \
             correct alongside a lancedb release that took the same one.",
            versions.len(),
        );
        assert!(
            lancedb.dependencies.iter().any(|d| d == krate),
            "lancedb {} does not depend on `{krate}`, so nothing here constrains its version \
             any more and this test has stopped checking what it claims to. Either the \
             declaration is now free to move — say so where it is declared — or lancedb \
             renamed the crate and this needs to follow.",
            lancedb.version,
        );
        // Stated separately, because the uniqueness above only holds while both sides are
        // *in* one graph; this is the comparison the manifest comment asks a person to make.
        let declared = declared(krate)
            .unwrap_or_else(|| panic!("`{krate}` is declared with no version requirement"));
        assert_eq!(
            major(&declared),
            major(&resolved[0].version),
            "yidam/cli/Cargo.toml asks for `{krate}` {declared} and the graph resolves \
             {}, which lancedb {} also uses.",
            resolved[0].version,
            lancedb.version,
        );
    }
}

// ── the embedding reference is the runtime that recorded it ───────────────────

/// The runtime that recorded the parity prefix is the one this workspace resolves.
///
/// `prelude/sdks/parity/fixtures/embed_config/` holds eight normalized dimensions that three
/// runtimes must agree on to 1e-5. They were produced by fastembed 4.9.1, and fastembed 6
/// answers ~3e-4 differently — one `ort` rc.12 → rc.13 bump, which skips the bundled ONNX
/// Runtime ahead four versions. transformers.js agrees with 4.9.1 and not with 6, so the bump
/// moves the *Rust reference* out of the space the other two share, and re-recording the
/// fixture relabels the outlier as the reference and makes the TypeScript SDK non-conforming
/// to its own parity contract (#536).
///
/// None of that was checkable, because the fixture recorded eight floats and not what produced
/// them. It now records `reference = { runtime, version }`, and this compares that major
/// against the version the lockfile resolves — so the bump is red here, on every PR, rather
/// than red only in `embed_config_parity`, which needs `YIDAM_EMBED_PARITY=1` and model weights
/// and does not run on a pull request at all.
///
/// Patch and minor releases within the major are wanted and pass: they are where the fix this
/// hold is waiting for will arrive.
#[test]
fn every_parity_reference_runtime_is_the_major_the_lockfile_resolves() {
    let dir = repo_root().join("yidam/prelude/sdks/parity/fixtures/embed_config");
    let mut fixtures: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("{} unreadable: {e}", dir.display()))
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "toml"))
        .collect();
    fixtures.sort();
    assert!(
        !fixtures.is_empty(),
        "no embed_config fixtures in {}; this test is reading the wrong tree",
        dir.display()
    );

    let packages = locked_packages();
    for f in &fixtures {
        let name = f.file_name().unwrap().to_string_lossy().into_owned();
        let fx: toml::Value = toml::from_str(
            &std::fs::read_to_string(f).unwrap_or_else(|e| panic!("{name} unreadable: {e}")),
        )
        .unwrap_or_else(|e| panic!("{name} is not TOML: {e}"));

        let reference = fx["expected"].get("reference").unwrap_or_else(|| {
            panic!(
                "{name} records a `prefix` and not what produced it. Eight floats with no \
                 provenance cannot be compared against anything: add \
                 `reference = {{ runtime = \"…\", version = \"…\" }}` under `[expected]` \
                 naming the runtime the numbers were recorded from."
            )
        });
        let runtime = reference["runtime"]
            .as_str()
            .unwrap_or_else(|| panic!("{name}: reference.runtime is not a string"));
        let recorded = reference["version"]
            .as_str()
            .unwrap_or_else(|| panic!("{name}: reference.version is not a string"));

        let resolved = packages
            .get(runtime)
            .and_then(|v| v.first())
            .unwrap_or_else(|| {
                panic!(
                    "{name} names `{runtime}` as the runtime that recorded its prefix and \
                     yidam/cli/Cargo.lock resolves no such crate. Either the reference moved \
                     to another library — in which case these numbers were recorded by \
                     something that is no longer here — or the name is a typo."
                )
            });

        assert_eq!(
            major(recorded),
            major(&resolved.version),
            "{name} records its prefix from {runtime} {recorded} and this workspace resolves \
             {runtime} {}. A major bump on the reference runtime is a different embedding \
             space: the eight dimensions in that fixture are the ones the TypeScript and \
             Python runners are held to, and they were measured somewhere else. Re-recording \
             them makes whichever runtime still agrees with {recorded} the non-conforming one \
             (#536). Taking the bump means moving the fixture and both other runtimes \
             together, deliberately.",
            resolved.version,
        );
    }
}

// ── the extension's types follow the floor it declares ───────────────────────

/// `@types/vscode` names the minor that `engines.vscode` declares, and cannot float past it.
///
/// The types follow the floor, not the registry: vsce refuses to package an extension whose
/// types are newer than the `engines.vscode` it declares, and the npm group bumped them to
/// 1.138 against a floor of 1.90. Green on every pull request, because `ci (vscode)` packages
/// nothing, and red on the first `editor/v*` tag — after the merge, on the release path.
///
/// Two halves. The minors must match, which is the comparison nobody was making. And the range
/// must be tilde or exact, because `^1.90.0` lets `npm install` float the types to 1.x on its
/// own, with no bump to review and no pull request to ignore — the range itself would be the
/// thing that breaks the tag.
#[test]
fn the_extension_types_name_the_floor_the_extension_declares() {
    let pkg: serde_json::Value = serde_json::from_str(&read("yidam/editors/vscode/package.json"))
        .expect("the extension's package.json is not JSON");

    let floor = pkg["engines"]["vscode"]
        .as_str()
        .expect("the extension declares no engines.vscode");
    let types = pkg["devDependencies"]["@types/vscode"]
        .as_str()
        .expect("the extension declares no @types/vscode");

    assert_eq!(
        major_minor(types),
        major_minor(floor),
        "the extension declares engines.vscode {floor} and @types/vscode {types}. vsce refuses \
         to package types newer than the declared floor, and the job that would catch it \
         (`ci (vscode)`) packages nothing on a pull request — so this reads as green until an \
         `editor/v*` tag. Raising the types is the decision to drop every VS Code older than \
         them, which means raising engines.vscode in the same change."
    );
    assert!(
        types.starts_with('~') || types.starts_with(char::is_numeric),
        "@types/vscode is `{types}`. A caret lets npm resolve any 1.x, so the types can pass \
         the floor with no bump to review and nothing in the diff — the range would be the \
         failure. Tilde, so only patches move."
    );
}

// ── the configuration is not stale ───────────────────────────────────────────

/// Every `ignore` in `.github/dependabot.yml` names something this repository still has.
///
/// The only thing here that gates the configuration rather than a constraint, and it gates the
/// opposite direction: not *"the hold is present"* but *"the hold is about something real"*.
/// An `ignore` on a dependency that was removed or renamed is indistinguishable from one doing
/// work — it sits in the file reading like a decision, and the next person to look for what
/// holds a version finds it and stops there.
#[test]
fn every_dependabot_ignore_names_something_that_exists() {
    let config: serde_yaml::Value =
        serde_yaml::from_str(&read(".github/dependabot.yml")).expect("dependabot.yml is not YAML");
    let updates = config["updates"]
        .as_sequence()
        .expect("dependabot.yml declares no `updates`");

    let mut checked = 0;
    let mut dangling = Vec::new();
    for entry in updates {
        let ecosystem = entry["package-ecosystem"].as_str().unwrap_or("");
        let directory = entry["directory"].as_str().unwrap_or("/");
        let empty = Vec::new();
        for rule in entry["ignore"].as_sequence().unwrap_or(&empty) {
            let Some(name) = rule["dependency-name"].as_str() else {
                continue;
            };
            checked += 1;
            // Where that ecosystem states its dependencies. For actions it is not a manifest
            // at all — the `uses:` lines are the declaration.
            let (haystack, what) = match ecosystem {
                "cargo" => (
                    read(&format!("{}/Cargo.toml", directory.trim_matches('/'))),
                    "Cargo.toml",
                ),
                "npm" => (
                    read(&format!("{}/package.json", directory.trim_matches('/'))),
                    "package.json",
                ),
                "github-actions" => {
                    let mut all = String::new();
                    for f in workflow_files() {
                        all.push_str(&std::fs::read_to_string(&f).unwrap_or_default());
                    }
                    (all, "any workflow's `uses:`")
                }
                other => panic!(
                    "dependabot.yml configures the `{other}` ecosystem and this test does not \
                     know where that one declares its dependencies, so its ignores are \
                     unchecked. Teach it, or the next stale hold in `{other}` is invisible."
                ),
            };
            if !haystack.contains(name) {
                dangling.push(format!(
                    "  {ecosystem} {directory}: `{name}` is in no {what}"
                ));
            }
        }
    }

    assert!(
        checked >= 4,
        "only {checked} ignore rules found in dependabot.yml. There are four holds and one of \
         them covers three arrow crates, so a count this low means the file was read wrong or \
         a hold was dropped without its reason going with it."
    );
    assert!(
        dangling.is_empty(),
        "these `ignore` entries hold nothing — the dependency is not declared where that \
         ecosystem declares them:\n{}\nA stale hold reads exactly like a live one.",
        dangling.join("\n")
    );
}

/// Every `.yml` under `.github/`, workflows and all.
fn workflow_files() -> Vec<PathBuf> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(&p, out);
            } else if p.extension().is_some_and(|x| x == "yml") {
                out.push(p);
            }
        }
    }
    let mut out = Vec::new();
    walk(&repo_root().join(".github"), &mut out);
    out.sort();
    out
}
