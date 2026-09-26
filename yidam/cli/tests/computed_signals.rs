//! `.yidam/computed/**` reaching a retrieval surface — the whole of what #1028 is about.
//!
//! Two calculators had been declared, invoked and committed since #472, and nothing in the CLI
//! ever opened the directory they wrote into. A corpus could compute how far each of its
//! assertions may travel, commit the answer, and have no way to ask a search for the nodes that
//! may leave the repository. The reader is in `src/computed.rs`; this is the vertical slice over
//! it, and the reason it is a vertical slice is that everything below runs the real binary and
//! reads the files it wrote.
//!
//! # Discovered, not listed
//!
//! Every test here reads the declared set out of the example's own manifest, the way
//! `capability_run.rs` does and for the reason it gives: a suite naming `travel-tier` stops
//! covering a second calculator the day one is declared, and goes quiet without going red.
//! [`a_calculator_writes_a_signal_table_for_these_tests_to_run`] is what closes the failure one
//! level up, and is the test to fix first if this file ever stops asserting anything.
//!
//! # A run does not reach your checkout
//!
//! `yidam run` commits and leaves the working tree alone — `cmd/run/mod.rs` states that as a
//! property and its report names the `git restore` that syncs one. So every test that wants to
//! *read* what a run produced does that restore first, and the one that deliberately does not
//! is [`a_run_that_has_not_reached_the_checkout_is_not_reported_as_never_having_run`].

mod common;

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use common::git::{git, out as git_out};
use common::{examples, Example};

/// Where a computed file lives, as both the manifest and the reader spell it.
const COMPUTED: &str = ".yidam/computed";

/// The examples declaring a calculator that writes under [`COMPUTED`].
///
/// By what the manifest declares rather than by what is on disk, because none of it is on disk
/// until a run has happened — no example commits a computed file, and one that did would be
/// shipping an answer nobody could tell from a stale one.
fn corpora_that_compute() -> Vec<String> {
    #[derive(serde::Deserialize, Default)]
    struct Declared {
        #[serde(default)]
        writes: Vec<String>,
    }
    #[derive(serde::Deserialize, Default)]
    struct Manifest {
        #[serde(default)]
        capability: BTreeMap<String, Declared>,
    }

    let mut all = Vec::new();
    for name in examples() {
        let path = common::repo_root().join(format!("examples/{name}/.yidam/capabilities.toml"));
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let m: Manifest = toml::from_str(&text).unwrap_or_default();
        let computes = m
            .capability
            .values()
            .any(|c| c.writes.iter().any(|w| w.starts_with(COMPUTED)));
        if computes {
            all.push(name);
        }
    }
    all
}

/// `yidam run`, then the `git restore` that brings what it committed into the checkout.
///
/// The restore is over `.yidam/` rather than over the paths the report names, because the
/// report is the thing under test elsewhere in this suite and a fixture that parsed it would
/// fail for two unrelated reasons.
fn run_and_take(e: &Example, name: &str) {
    let (out, err, code) = e.run(&["run"]);
    assert_eq!(code, 0, "`yidam run` failed in {name}:\n{out}{err}");
    git(
        &e.path(),
        &[
            "restore",
            "--source=HEAD",
            "--worktree",
            "--staged",
            "--",
            ".yidam/",
        ],
    );
}

/// Every signal name the computed files in a checkout declare, in byte order.
///
/// Parsed from the files rather than taken from the reader, which is the point: the reader is
/// what is under test, so a list it supplied would agree with itself. `node` is the key, not a
/// signal, which is the one piece of the contract this fixture has to know.
fn declared_signal_names(root: &Path) -> BTreeSet<String> {
    #[derive(serde::Deserialize, Default)]
    struct Table {
        #[serde(default)]
        signals: Vec<serde_yaml::Mapping>,
    }
    let mut names = BTreeSet::new();
    let Ok(entries) = std::fs::read_dir(root.join(COMPUTED)) else {
        return names;
    };
    for entry in entries.flatten() {
        let Ok(text) = std::fs::read_to_string(entry.path()) else {
            continue;
        };
        let table: Table = serde_yaml::from_str(&text).unwrap_or_default();
        for row in table.signals {
            for key in row.keys().filter_map(|k| k.as_str()) {
                if key != "node" {
                    names.insert(key.to_string());
                }
            }
        }
    }
    names
}

/// Every embedding record in a checkout, by the path it describes.
fn records(root: &Path) -> BTreeMap<String, serde_json::Value> {
    let mut out = BTreeMap::new();
    for entry in walkdir::WalkDir::new(root.join(".yidam/embeddings"))
        .into_iter()
        .flatten()
    {
        if entry.path().extension() != Some(std::ffi::OsStr::new("json")) {
            continue;
        }
        let text = std::fs::read_to_string(entry.path()).unwrap();
        let v: serde_json::Value = serde_json::from_str(&text)
            .unwrap_or_else(|e| panic!("{} is not a record: {e}", entry.path().display()));
        out.insert(v["path"].as_str().unwrap_or_default().to_string(), v);
    }
    out
}

/// One `doctor` check, by id.
fn check(e: &Example, id: &str) -> serde_json::Value {
    let (out, err, _) = e.run(&["doctor", "--format", "json"]);
    let report: serde_json::Value = serde_json::from_str(&out)
        .unwrap_or_else(|e| panic!("`doctor --format json` is not json ({e}):\n{out}{err}"));
    report["checks"]
        .as_array()
        .expect("checks")
        .iter()
        .find(|c| c["id"] == id)
        .unwrap_or_else(|| panic!("no `{id}` check in:\n{out}"))
        .clone()
}

/// The guard on the discovery above: a suite that runs nothing passes everything.
///
/// Two halves, because there are two ways for this file to go quiet. No example declaring a
/// calculator is the obvious one. The other is an example whose calculator writes a file with
/// no signal table in it, which the reader lists and does not read — legal, and it would make
/// every assertion below vacuously true.
#[test]
fn a_calculator_writes_a_signal_table_for_these_tests_to_run() {
    let computing = corpora_that_compute();
    assert!(
        !computing.is_empty(),
        "no example declares a capability writing under {COMPUTED}, so every test in this \
         file is vacuous. `.yidam/capabilities.toml` must be git-tracked — an untracked file \
         under `examples/*/.yidam/` is invisible to `Example::materialize`"
    );
    for name in computing {
        let e = Example::materialize(&name);
        run_and_take(&e, &name);
        assert!(
            !declared_signal_names(&e.path()).is_empty(),
            "{name} computes files that carry no `signals:` table, so nothing below asserts \
             that a signal reaches anything. A calculator adopts the contract by emitting \
             `format_version` and a `signals:` list of rows keyed on `node:`"
        );
    }
}

/// The issue's title, as a test: a calculator's answer reaches a retrieval surface.
///
/// The surface is the embedding record, which is where the corpus meets the index. Index
/// *columns* are #1029's, and deliberately not asserted here — that issue owns the schema
/// decision and a second one made in passing would be the thing it has to undo.
#[test]
fn a_signal_a_calculator_computed_reaches_the_embedding_record() {
    for name in corpora_that_compute() {
        let e = Example::materialize(&name);
        run_and_take(&e, &name);
        let declared = declared_signal_names(&e.path());

        let (out, err, code) = e.run(&["embed"]);
        assert_eq!(code, 0, "`yidam embed` failed in {name}:\n{out}{err}");

        let records = records(&e.path());
        let carried: BTreeSet<String> = records
            .values()
            .filter_map(|r| r["signals"].as_object())
            .flat_map(|s| s.keys().cloned())
            .collect();
        assert_eq!(
            carried, declared,
            "the signals that reached the records in {name} are not the ones its computed \
             files declare"
        );

        // And on the right nodes: a reader that attached every signal to every record would
        // satisfy the set comparison above. Each row names a node, and the record for that
        // node is the one that has to carry it.
        let with_signals: Vec<&String> = records
            .iter()
            .filter(|(_, r)| r["signals"].as_object().is_some_and(|s| !s.is_empty()))
            .map(|(p, _)| p)
            .collect();
        assert!(
            !with_signals.is_empty(),
            "no record in {name} carries a signal, although its computed files declare {}",
            declared.len()
        );
        for path in with_signals {
            assert!(
                e.path().join(path).exists(),
                "a signal in {name} reached a record for {path}, which is not a corpus file"
            );
        }
    }
}

/// A corpus that computes nothing about a record writes the record it wrote before (#1028).
///
/// The catalog is the case that is always available: a signal is keyed to a corpus node, and a
/// source is not one, so a source record must carry no `signals` key at all rather than an
/// empty object. It is the difference between a field being added and every record changing —
/// an index already built from the old records stays valid only if the bytes are identical.
#[test]
fn a_record_with_no_signal_carries_no_signal_field() {
    for name in corpora_that_compute() {
        let e = Example::materialize(&name);
        run_and_take(&e, &name);
        let (out, err, code) = e.run(&["embed"]);
        assert_eq!(code, 0, "`yidam embed` failed in {name}:\n{out}{err}");

        let all = records(&e.path());
        let sources: Vec<(&String, &serde_json::Value)> =
            all.iter().filter(|(_, r)| r["kind"] == "source").collect();
        assert!(
            !sources.is_empty(),
            "{name} embeds no catalog source, so this test asserts nothing"
        );
        for (path, record) in sources {
            assert!(
                record.get("signals").is_none(),
                "the source record for {path} in {name} carries a `signals` key, so adopting \
                 the reader moved bytes for a record no calculator says anything about"
            );
        }
    }
}

/// `doctor` says a computed answer no longer stands, and names the run that would fix it.
///
/// The edit is to a corpus file the calculator declares it reads, which is the drift that
/// actually happens: somebody revises a node and the computed answer beside it is now about the
/// node as it was. Nothing in this repository could see that before #1028.
#[test]
fn doctor_reports_a_computed_answer_whose_inputs_moved() {
    for name in corpora_that_compute() {
        let e = Example::materialize(&name);
        run_and_take(&e, &name);
        assert_eq!(
            check(&e, "computed")["verdict"],
            "ok",
            "{name} is not clean immediately after a run and a restore, so the drift below \
             would not be attributable to the edit"
        );

        // Any node file: the calculators read `.yidam/corpus/**`, and which node moved is not
        // what is under test.
        let node = common::tracked_under(&e.path(), ".yidam/corpus/")
            .into_iter()
            .find(|p| p.ends_with(".yml") && !p.ends_with(".ont.yml"))
            .unwrap_or_else(|| panic!("{name} holds no corpus node to edit"));
        let path = e.path().join(&node);
        let text = std::fs::read_to_string(&path).unwrap();
        std::fs::write(&path, format!("{text}\n# an edit\n")).unwrap();

        let c = check(&e, "computed");
        assert_eq!(c["verdict"], "warn", "{name} after editing {node}: {c}");
        assert_eq!(
            c["remedy"], "yidam run",
            "{name} reports drift and does not name the command that resolves it: {c}"
        );
    }
}

/// The interval between a run and the restore that syncs a checkout is reported as what it is.
///
/// A run commits and does not touch the tree, so for that interval the computed files are at
/// HEAD and absent from the checkout. Reading the receipt off disk would answer *"it has never
/// run"* for exactly that window — which is both false and the moment somebody is most likely
/// to ask. `cmd::run::standing` reads the receipt from HEAD for this reason, and this is what
/// holds it there.
#[test]
fn a_run_that_has_not_reached_the_checkout_is_not_reported_as_never_having_run() {
    for name in corpora_that_compute() {
        let e = Example::materialize(&name);
        let (out, err, code) = e.run(&["run"]);
        assert_eq!(code, 0, "`yidam run` failed in {name}:\n{out}{err}");
        assert!(
            git_out(&e.path(), &["status", "--porcelain"]).contains(COMPUTED),
            "{name}: a run left the checkout holding its computed files, so this test is \
             asserting about a state that no longer exists"
        );

        let c = check(&e, "computed");
        let detail = c["detail"].as_str().unwrap_or_default();
        assert_eq!(c["verdict"], "warn", "{name}: {c}");
        assert!(
            !detail.contains("never run"),
            "{name} reports a run that just landed as never having happened: {c}"
        );
        assert!(
            c["remedy"]
                .as_str()
                .unwrap_or_default()
                .starts_with("git restore"),
            "{name} does not name the command that syncs the checkout: {c}"
        );
    }
}
