//! Golden fixtures for the report contract — RFC-0001's `reports/` family, RFC-0016 Phase 0.
//!
//! Two obligations, and the first is the one that constrains the design:
//!
//! 1. **`--format text` is byte-identical to what these commands have always printed.**
//!    JSON was added beside the prose path, not through it. A repository that never passes
//!    `--format` must not be able to tell this work happened.
//!
//! 2. **`--format json` matches a committed golden**, so a consumer versioned independently
//!    of the binary has something to write tests against — which is the whole point of
//!    promoting the reports from CLI behaviour to a contract.
//!
//! Run with `UPDATE_GOLDENS=1` to rewrite the expected files after an intended change. The
//! diff is the review.

use std::path::{Path, PathBuf};
use std::process::Command;

fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../prelude/sdks/parity/fixtures/reports/basic")
}

/// Copy the fixture repo into a tempdir and make it a git repository.
///
/// `repo_root()` shells out to `git rev-parse --show-toplevel`, so the reports cannot run
/// against a bare directory. Committing also gives `status` a genesis date to report.
fn stage() -> tempfile::TempDir {
    let tmp = tempfile::tempdir().unwrap();
    copy_dir(&fixture_dir().join("repo"), tmp.path());
    let git = |args: &[&str]| {
        let ok = Command::new("git")
            .current_dir(tmp.path())
            .args(args)
            .status()
            .unwrap()
            .success();
        assert!(ok, "git {args:?} failed");
    };
    git(&["init", "-q", "-b", "main"]);
    git(&["config", "user.email", "fixture@yidam.test"]);
    git(&["config", "user.name", "Fixture"]);
    git(&["add", "-A"]);
    // Fixed dates keep `status`'s genesis field stable across runs.
    let commit = |msg: &str| {
        Command::new("git")
            .current_dir(tmp.path())
            .args(["commit", "-q", "-m", msg])
            .env("GIT_AUTHOR_DATE", "2026-01-01T00:00:00Z")
            .env("GIT_COMMITTER_DATE", "2026-01-01T00:00:00Z")
            .status()
            .unwrap();
    };
    commit("genesis: reports fixture");

    // A second commit so `diff HEAD~1..HEAD` has a range to report on, and one that
    // exercises the modified-node arm rather than only add/remove.
    let node = tmp.path().join(".yidam/corpus/concept/tailwater.yml");
    let text = std::fs::read_to_string(&node).unwrap();
    std::fs::write(
        &node,
        text.replace(
            "A discharge regime.",
            "A discharge regime, revised. [verified]",
        )
        .replace(
            "Water downstream of a structure.",
            "Water downstream of a structure. [inference]",
        ),
    )
    .unwrap();
    git(&["add", "-A"]);
    commit("revise: tailwater carries a claim tag");

    // One operational commit, so the log goldens show the split rather than a column of
    // [E]. A fixture whose history is all one kind cannot demonstrate a classifier.
    std::fs::write(tmp.path().join(".yidam/corpus/.gitkeep"), "").unwrap();
    git(&["add", "-A"]);
    commit("regen: REGEN blocks refreshed");

    // Refs the sangha and phase reports read. `ma/gauge-reader` is deliberately absent:
    // the fixture registers that elector anyway, so `branch_present: false` is exercised
    // by a golden rather than only by a unit test.
    git(&["branch", "ma/hydrologist"]);
    git(&["branch", "rigpa/tailwater-regime"]);
    tmp
}

fn copy_dir(from: &Path, to: &Path) {
    for entry in walkdir::WalkDir::new(from)
        .into_iter()
        .filter_map(Result::ok)
    {
        let rel = entry.path().strip_prefix(from).unwrap();
        let dest = to.join(rel);
        if entry.file_type().is_dir() {
            std::fs::create_dir_all(&dest).unwrap();
        } else {
            std::fs::create_dir_all(dest.parent().unwrap()).unwrap();
            std::fs::copy(entry.path(), &dest).unwrap();
        }
    }
}

/// Replace what belongs to the run rather than to the corpus.
///
/// The absolute root, the binary's version and its build commit all vary by machine and by
/// checkout. Redacting them is what lets the rest — every field name, every value, and the
/// order they appear in — be compared literally.
///
/// **The feature set belongs here too**, and its absence is why `ci (cli · full features)`
/// failed on every push to main from the day these goldens landed. It runs `--all-features`,
/// which reports five, against goldens recorded from the light build, which reports one. The
/// job does not run on pull requests, so nothing said so until somebody looked at main.
///
/// Redacting it does not lose the property: [`the_light_build_reports_exactly_reports`]
/// asserts the light feature set on its own, where it can be stated once instead of repeated
/// through eighteen goldens that are not about features.
fn redact(out: &str, root: &Path) -> String {
    let root_s = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    let redacted = out
        .replace(&root_s.display().to_string(), "<ROOT>")
        .replace(&root.display().to_string(), "<ROOT>")
        .replace(
            &format!("\"version\": \"{}\"", env!("CARGO_PKG_VERSION")),
            "\"version\": \"<VERSION>\"",
        )
        .replace(
            &format!("\"commit\": \"{}\"", env!("YIDAM_BUILD_COMMIT")),
            "\"commit\": \"<COMMIT>\"",
        );
    redact_features(&redacted)
}

/// Collapse the `features` array to a placeholder, whatever it holds.
///
/// Textual because these goldens are compared as text: parsing and re-emitting would
/// reformat every document and make the diff on an intended change unreadable, which is the
/// one thing a golden has to stay good at.
fn redact_features(out: &str) -> String {
    let Some(start) = out.find("\"features\": [") else {
        return out.to_string();
    };
    let open = start + "\"features\": [".len();
    let Some(close) = out[open..].find(']') else {
        return out.to_string();
    };
    format!(
        "{}\"features\": [\n      \"<FEATURES>\"\n    {}",
        &out[..start],
        &out[open + close..]
    )
}

struct Run {
    stdout: String,
    code: i32,
}

fn run(root: &Path, args: &[&str]) -> Run {
    let out = Command::new(env!("CARGO_BIN_EXE_yidam"))
        .current_dir(root)
        .args(args)
        .output()
        .unwrap();
    Run {
        stdout: redact(&String::from_utf8_lossy(&out.stdout), root),
        code: out.status.code().unwrap_or(-1),
    }
}

fn assert_golden(name: &str, actual: &str) {
    let path = fixture_dir().join("expected").join(name);
    if std::env::var("UPDATE_GOLDENS").is_ok() {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, actual).unwrap();
        return;
    }
    let expected = std::fs::read_to_string(&path).unwrap_or_else(|_| {
        panic!(
            "missing golden {}. Run with UPDATE_GOLDENS=1 to create it.",
            path.display()
        )
    });
    assert_eq!(
        expected, actual,
        "\n{name} drifted from its golden. If the change is intended, \
         re-run with UPDATE_GOLDENS=1 and review the diff.\n"
    );
}

/// Every command that grew a `--format` flag, in both formats.
const COMMANDS: &[(&str, &[&str])] = &[
    ("lint", &["lint"]),
    ("graph-check", &["graph-check"]),
    ("status", &["status"]),
    ("open-questions", &["open-questions"]),
    ("corpus-index", &["corpus-index"]),
    ("catalog-audit", &["catalog-audit"]),
    ("phases", &["phases"]),
    ("diff", &["diff", "HEAD~1..HEAD"]),
    ("log", &["log"]),
    ("log-epistemic", &["log", "--epistemic"]),
    ("index-status", &["index-status"]),
    ("sangha", &["sangha"]),
    ("graph", &["graph"]),
    // A plan, not a rename: the golden must not mutate the fixture the other goldens read.
    (
        "rename",
        &[
            "rename",
            "concept/low-flow.yml",
            "concept/base-flow.yml",
            "--dry-run",
        ],
    ),
    (
        "neighbors",
        &["neighbors", "concept/tailwater.yml", "--depth", "2"],
    ),
    ("vocabulary", &["vocabulary"]),
    // The fixture carries no vendored GRAPH.md, so this golden also pins the
    // no-document arm: every `when` empty, and `drift` empty rather than thirty
    // rows of "missing".
    (
        "vocabulary-check",
        &[
            "vocabulary",
            "--check",
            "vendor(yidam): the prelude at 4e1a2b0",
        ],
    ),
];

#[test]
fn text_output_is_byte_identical_to_its_golden() {
    let tmp = stage();
    for (name, args) in COMMANDS {
        let r = run(tmp.path(), args);
        assert_golden(&format!("{name}.txt"), &r.stdout);
    }
}

#[test]
fn json_output_matches_its_golden() {
    let tmp = stage();
    for (name, args) in COMMANDS {
        let mut a = args.to_vec();
        a.extend_from_slice(&["--format", "json"]);
        let r = run(tmp.path(), &a);
        let parsed: serde_json::Value = serde_json::from_str(&r.stdout).unwrap_or_else(|e| {
            panic!("{name} --format json is not valid JSON: {e}\n{}", r.stdout)
        });
        assert_eq!(parsed["format_version"], "1", "{name} handshake");
        assert_golden(&format!("{name}.json"), &r.stdout);
    }
}

/// The gate must not depend on how the answer was asked for.
#[test]
fn exit_codes_are_identical_across_formats() {
    let tmp = stage();
    for (name, args) in COMMANDS {
        let text = run(tmp.path(), args);
        let mut a = args.to_vec();
        a.extend_from_slice(&["--format", "json"]);
        let json = run(tmp.path(), &a);
        assert_eq!(
            text.code, json.code,
            "{name}: text exited {} and json exited {} — a gate that gates differently \
             depending on the output format is not a gate",
            text.code, json.code
        );
    }
}

/// Every field a golden emits must be declared in the committed schema, and every
/// required field must be present.
///
/// Not full JSON-Schema validation — that would want a dependency this crate does not
/// otherwise need. It is the half that actually rots: a field added to a report and not to
/// the schema, which leaves a consumer reading a contract that does not describe the data
/// it is being sent.
#[test]
fn every_golden_field_is_declared_in_the_schema() {
    let schema: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(fixture_dir().parent().unwrap().join("report.schema.json"))
            .expect("report.schema.json"),
    )
    .expect("schema is valid JSON");

    let required: Vec<&str> = schema["required"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    let declared = schema["properties"].as_object().unwrap();

    let mut problems = Vec::new();
    let mut seen = 0;
    for entry in std::fs::read_dir(fixture_dir().join("expected")).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|x| x != "json") {
            continue;
        }
        seen += 1;
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        let doc: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let obj = doc.as_object().expect("an envelope is an object");
        for key in &required {
            if !obj.contains_key(*key) {
                problems.push(format!("  {name}: missing required `{key}`"));
            }
        }
        for key in obj.keys() {
            if !declared.contains_key(key) {
                problems.push(format!(
                    "  {name}: emits `{key}`, which report.schema.json does not declare"
                ));
            }
        }
    }

    assert!(seen > 0, "no JSON goldens found — the scan is broken");
    assert!(
        problems.is_empty(),
        "goldens and schema disagree:\n{}\n\nAdd the field to report.schema.json, or stop \
         emitting it. A consumer reading a contract that does not describe the data it is \
         sent is the failure this contract exists to prevent.",
        problems.join("\n")
    );
}

/// The light build reports exactly `reports`, and that is worth asserting once.
///
/// It used to be asserted eighteen times, once per golden, as a side effect of the feature
/// array being compared literally — which is why `--all-features` failed all eighteen and
/// said nothing about features. Stated here, it holds for the build it is about and leaves
/// the goldens to be about report shape.
#[test]
#[cfg(not(any(
    feature = "index",
    feature = "export-sqlite",
    feature = "export-graph",
    feature = "tonpa"
)))]
fn the_light_build_reports_exactly_reports() {
    let tmp = stage();
    let r = run(tmp.path(), &["status", "--format", "json"]);
    // Read from the unredacted process output — `redact` is what this test exists beside.
    let out = Command::new(env!("CARGO_BIN_EXE_yidam"))
        .current_dir(tmp.path())
        .args(["status", "--format", "json"])
        .output()
        .unwrap();
    let doc: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(
        doc["yidam"]["features"].as_array().unwrap(),
        &vec![serde_json::json!("reports")],
        "the light default must report exactly one feature, so a consumer can tell \
         `this binary cannot do that` from `that failed`"
    );
    // And the redaction the goldens rely on actually fires.
    assert!(r.stdout.contains("<FEATURES>"), "{}", r.stdout);
}

/// Whatever the build, the goldens see the same thing.
///
/// The bug this closes was a redaction that covered version and commit and not the third
/// thing that varies by build. Asserting the collapse directly is cheaper than discovering
/// the next one on main.
#[test]
fn the_feature_set_is_redacted_whatever_it_holds() {
    let one = redact_features("  \"features\": [\n      \"reports\"\n    ]\n");
    let many = redact_features(
        "  \"features\": [\n      \"reports\",\n      \"index\",\n      \"tonpa\"\n    ]\n",
    );
    assert_eq!(
        one, many,
        "a light and a full build must redact identically"
    );
    assert!(one.contains("<FEATURES>"), "{one}");
}
