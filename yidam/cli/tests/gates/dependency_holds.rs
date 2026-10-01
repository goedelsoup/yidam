//! A dependency that must not move, and the thing that stops it.
//!
//! `.github/dependabot.yml` carried four `ignore` entries. #668 gave the fourth
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
//! #945 added a fifth, on the typescript major, and the same rule applies: the ignore stops a
//! weekly proposal, and [`every_editor_type_checks_with_one_typescript_major`] is the hold.
//!
//! #536 removed the fastembed `ignore`: the hold resolved when transformers.js moved into the
//! same space, and a proposed fastembed major is now a measurement to run, not a known-bad bump
//! to suppress. The gate stayed, and gained its other half — see
//! [`every_conforming_parity_runtime_is_the_major_its_lockfile_resolves`].
//!
//! Two things are gated about the configuration itself. That it is not stale: see
//! [`every_dependabot_ignore_names_something_that_exists`] — an `ignore` for a dependency
//! nobody declares any more reads as a hold and holds nothing. And that it is not
//! incomplete: see [`every_lockfile_is_one_dependabot_resolves_or_has_no_registry_dependency`]
//! — a lockfile no entry names is one nobody maintains, and it looks exactly like one that is.

use crate::common;

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

/// Every `embed_config` fixture, by file name, parsed.
fn embed_fixtures() -> Vec<(String, toml::Value)> {
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
    fixtures
        .iter()
        .map(|f| {
            let name = f.file_name().unwrap().to_string_lossy().into_owned();
            let fx = toml::from_str(
                &std::fs::read_to_string(f).unwrap_or_else(|e| panic!("{name} unreadable: {e}")),
            )
            .unwrap_or_else(|e| panic!("{name} is not TOML: {e}"));
            (name, fx)
        })
        .collect()
}

/// The runtime that recorded the parity prefix is the one this workspace resolves.
///
/// `prelude/sdks/parity/fixtures/embed_config/` holds eight normalized dimensions that three
/// runtimes must agree on to 1e-5. A major bump on the runtime that recorded them can move them:
/// fastembed 4 → 6 took `ort` rc.12 → rc.13, which skips the bundled ONNX Runtime ahead four
/// versions, and answered ~3e-4 differently. transformers.js then still agreed with 4.9.1, so
/// the bump moved the *Rust reference* out of the space the other two shared, and re-recording
/// the fixture would have relabelled the outlier as the reference (#536).
///
/// None of that was checkable, because the fixture recorded eight floats and not what produced
/// them. It now records `reference = { runtime, version }`, and this compares that major
/// against the version the lockfile resolves — so the bump is red here, on every PR, rather
/// than red only in `embed_config_parity`, which needs `YIDAM_EMBED_PARITY=1` and model weights
/// and does not run on a pull request at all.
///
/// Patch and minor releases within the major are wanted and pass.
#[test]
fn every_parity_reference_runtime_is_the_major_the_lockfile_resolves() {
    let packages = locked_packages();
    for (name, fx) in embed_fixtures() {
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
             {runtime} {}. A major bump on the reference runtime can be a different embedding \
             space: the eight dimensions in that fixture are the ones the TypeScript and \
             Python runners are held to, and they were measured somewhere else. Run \
             `mise run embed-parity` under the bump; if the prefix moved, move the fixture \
             and every `conforming` runtime together, deliberately (#536).",
            resolved.version,
        );
    }
}

/// Every runtime the fixture says conforms is the major its lockfile resolves.
///
/// The other half of the hold above, and the half #536 was decided on. The reference is only a
/// reference while the runtimes held to it still answer the same way, and transformers.js
/// moves its own ONNX Runtime: 3.8.1 bundles 1.21 and agreed with fastembed 4.9.1; 4.3.0
/// bundles 1.30 and agrees with fastembed 7.1.0 instead. It is a `devDependency` of the
/// TypeScript SDK, used by nothing but `embed_parity.test.ts`, which no pull request runs — so
/// the npm group bumping it a major was exactly as silent as the cargo group bumping fastembed.
///
/// `sentence-transformers` is not in `conforming` and is not checked here: it cannot load
/// `model_file`, runs fp32 weights under `known_delta`, and is ~1e-2 away from any ONNX answer.
#[test]
fn every_conforming_parity_runtime_is_the_major_its_lockfile_resolves() {
    // Where each runtime a fixture may name is locked. A name not here is a runtime nobody
    // taught this test to find, and failing on it is the point — a `conforming` entry this test
    // skips is one nothing holds.
    let lockfiles: BTreeMap<&str, &str> = [(
        "@huggingface/transformers",
        "yidam/prelude/sdks/typescript/package-lock.json",
    )]
    .into();

    let mut checked = 0;
    for (name, fx) in embed_fixtures() {
        let Some(conforming) = fx["expected"].get("conforming") else {
            continue;
        };
        let conforming = conforming
            .as_table()
            .unwrap_or_else(|| panic!("{name}: `conforming` is not a table of runtime = version"));
        for (runtime, recorded) in conforming {
            let recorded = recorded
                .as_str()
                .unwrap_or_else(|| panic!("{name}: conforming.\"{runtime}\" is not a string"));
            let lock = lockfiles.get(runtime.as_str()).unwrap_or_else(|| {
                panic!(
                    "{name} says `{runtime}` conforms and this test does not know which \
                     lockfile resolves it. Add it to `lockfiles` above."
                )
            });
            let lock_json: serde_json::Value = serde_json::from_str(&read(lock))
                .unwrap_or_else(|e| panic!("{lock} is not JSON: {e}"));
            let resolved = lock_json["packages"][format!("node_modules/{runtime}")]["version"]
                .as_str()
                .unwrap_or_else(|| panic!("{lock} resolves no `{runtime}`"));
            assert_eq!(
                major(recorded),
                major(resolved),
                "{name} records `{runtime}` {recorded} as conforming to its prefix and {lock} \
                 resolves {resolved}. transformers.js bundles its own ONNX Runtime, and the \
                 quantized kernels have already answered ~3e-4 differently across one such \
                 move. Run `mise run embed-parity` under the bump before taking it (#536).",
            );
            checked += 1;
        }
    }
    assert!(
        checked > 0,
        "no embed_config fixture names a `conforming` runtime, so this checked nothing"
    );
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

/// The extension, the web editor and the docs site compile with the same TypeScript major.
///
/// The docs site and the web editor type-check with `@astrojs/check`, whose peer range stops
/// at `^6`. The extension has no astro, so a grouped dependabot bump took it to 7 on its own,
/// and the repository had two TypeScript majors in two editors that share a reader, a lint
/// config and a review (#945). Nothing broke, which is the point: the next person to move a
/// shared pattern between them finds out which compiler they were on from the error.
///
/// Two halves per project — the lockfile resolves the major the range declares, so a stale
/// lock is not read as agreement — and then one major across all three. Moving to 7 is a
/// change to all three in one pull request, once `@astrojs/check` allows it.
#[test]
fn every_editor_type_checks_with_one_typescript_major() {
    const PROJECTS: [&str; 3] = [
        "yidam/editors/vscode",
        "yidam/editors/web",
        "yidam/web/docs",
    ];

    let mut majors = BTreeMap::new();
    for dir in PROJECTS {
        let pkg: serde_json::Value = serde_json::from_str(&read(&format!("{dir}/package.json")))
            .unwrap_or_else(|e| panic!("{dir}/package.json is not JSON: {e}"));
        let declared = pkg["devDependencies"]["typescript"]
            .as_str()
            .or_else(|| pkg["dependencies"]["typescript"].as_str())
            .unwrap_or_else(|| panic!("{dir} declares no typescript"));

        let lock: serde_json::Value =
            serde_json::from_str(&read(&format!("{dir}/package-lock.json")))
                .unwrap_or_else(|e| panic!("{dir}/package-lock.json is not JSON: {e}"));
        let locked = lock["packages"]["node_modules/typescript"]["version"]
            .as_str()
            .unwrap_or_else(|| panic!("{dir}/package-lock.json resolves no typescript"));

        assert_eq!(
            major(locked),
            major(declared),
            "{dir} declares typescript {declared} and its lockfile resolves {locked}. Run \
             `npm install` there; until then the declared range is not what compiles."
        );
        majors.insert(dir, major(declared));
    }

    let distinct: std::collections::BTreeSet<_> = majors.values().collect();
    assert!(
        distinct.len() == 1,
        "the editors compile with different TypeScript majors: {majors:?}. The docs site and \
         the web editor are held at `@astrojs/check`'s peer range; the extension follows them, \
         so a major moves in all three package.json files in one change, not in one \
         dependabot group at a time."
    );
}

// ── the configuration is neither stale nor incomplete ─────────────────────────

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
        checked >= 8,
        "only {checked} ignore rules found in dependabot.yml. There are four holds — one covers \
         three arrow crates and one is repeated in three npm entries — so a count this low \
         means the file was read wrong or a hold was dropped without its reason going with it."
    );
    assert!(
        dangling.is_empty(),
        "these `ignore` entries hold nothing — the dependency is not declared where that \
         ecosystem declares them:\n{}\nA stale hold reads exactly like a live one.",
        dangling.join("\n")
    );
}

/// Every tracked lockfile is in a directory some dependabot entry names, or its manifest has
/// no registry dependency for dependabot to propose.
///
/// The configuration's other direction from the test above. An ecosystem entry resolves the
/// directories it names and nothing else, so a lockfile outside them drifts with nothing
/// reporting it — and the file gives no sign of it, because the entries it does have look
/// complete. #945 counted six entries over nineteen JS projects.
///
/// The exemption is cargo-only and checked rather than listed: a crate whose every dependency
/// is a `path` has nothing on a registry, so the entry for whatever it points at moves its
/// lock. That is the fourteen domain crates, which reach the parity testkit and nothing else.
/// The moment one takes a registry dependency, it needs an entry and this is red. npm gets no
/// such exemption — a `package.json` with no dependencies has no lockfile to track.
#[test]
fn every_lockfile_is_one_dependabot_resolves_or_has_no_registry_dependency() {
    let config: serde_yaml::Value =
        serde_yaml::from_str(&read(".github/dependabot.yml")).expect("dependabot.yml is not YAML");
    let updates = config["updates"]
        .as_sequence()
        .expect("dependabot.yml declares no `updates`");

    // (lockfile name, directory patterns) for each entry that keeps a lockfile.
    let mut covered: Vec<(&str, Vec<String>)> = Vec::new();
    for entry in updates {
        let lockfile = match entry["package-ecosystem"].as_str().unwrap_or("") {
            "cargo" => "Cargo.lock",
            "npm" => "package-lock.json",
            _ => continue,
        };
        let mut dirs: Vec<String> = entry["directory"]
            .as_str()
            .map(String::from)
            .into_iter()
            .collect();
        for d in entry["directories"].as_sequence().into_iter().flatten() {
            dirs.extend(d.as_str().map(String::from));
        }
        covered.push((lockfile, dirs));
    }

    let tracked: Vec<String> = ["*Cargo.lock", "*package-lock.json"]
        .iter()
        .flat_map(|spec| common::tracked_under(&repo_root(), spec))
        .collect();

    let mut seen = 0;
    let mut exempt = 0;
    let mut orphaned = Vec::new();
    for path in &tracked {
        seen += 1;
        let (dir, file) = path.rsplit_once('/').unwrap_or(("", path));
        let dir = format!("/{dir}");
        let is_covered = covered.iter().any(|(lockfile, patterns)| {
            *lockfile == file && patterns.iter().any(|p| directory_matches(p, &dir))
        });
        if is_covered {
            continue;
        }
        if file == "Cargo.lock" && only_path_dependencies(&format!("{}/Cargo.toml", &dir[1..])) {
            exempt += 1;
            continue;
        }
        orphaned.push(format!("  {path}"));
    }

    assert!(
        seen >= 30,
        "git ls-files found only {seen} lockfiles. There are over thirty — four cargo \
         workspaces, fourteen domain crates and their fifteen TypeScript siblings, three \
         editors and the docs — so the listing went wrong, and a coverage check over nothing \
         passes."
    );
    assert!(
        exempt > 0,
        "no lockfile used the path-only exemption, and the fourteen domain crates are what it \
         exists for. Either they gained entries — then drop the exemption and its comment in \
         dependabot.yml — or the manifest check stopped reading them."
    );
    assert!(
        orphaned.is_empty(),
        "these lockfiles are in no directory a dependabot entry names, and declare registry \
         dependencies, so nothing proposes their updates:\n{}\nName the directory in \
         `.github/dependabot.yml` — a `directories` glob with `group-by: dependency-name` if it \
         is one of a family — or remove the registry dependency.",
        orphaned.join("\n")
    );
}

/// Whether a dependabot directory pattern names `dir`. `*` is one path segment, which is all
/// the configuration uses; a pattern this does not understand matches nothing and shows up as
/// an orphaned lockfile, not as a silent pass.
fn directory_matches(pattern: &str, dir: &str) -> bool {
    let p: Vec<&str> = pattern.trim_end_matches('/').split('/').collect();
    let d: Vec<&str> = dir.trim_end_matches('/').split('/').collect();
    p.len() == d.len() && p.iter().zip(&d).all(|(p, d)| *p == "*" || p == d)
}

/// Whether every dependency in a `Cargo.toml` — normal, dev and build — is a `path`.
fn only_path_dependencies(manifest: &str) -> bool {
    let table: toml::Table = read(manifest)
        .parse()
        .unwrap_or_else(|e| panic!("{manifest} is not TOML: {e}"));
    ["dependencies", "dev-dependencies", "build-dependencies"]
        .iter()
        .filter_map(|k| table.get(*k).and_then(toml::Value::as_table))
        .flat_map(|deps| deps.values())
        .all(|spec| spec.get("path").is_some())
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
