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

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;

fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../prelude/sdks/parity/fixtures/reports/basic")
}

/// Copy the fixture repo into a tempdir and make it the repository `stage.toml` describes.
///
/// `repo_root()` shells out to `git rev-parse --show-toplevel`, so the reports cannot run
/// against a bare directory. Committing also gives `status` a genesis date to report.
///
/// **The recipe is the fixture's, not this file's.** It used to live here, and separately in
/// six copies across the extension's tests, which is how the goldens came to describe a
/// three-commit two-branch repository while the extension was exercised on a one-commit one.
fn stage() -> tempfile::TempDir {
    let tmp = tempfile::tempdir().unwrap();
    copy_dir(&fixture_dir().join("repo"), tmp.path());
    apply_recipe(tmp.path(), &fixture_dir().join("stage.toml"));
    tmp
}

/// Build the history `recipe` describes in an already-populated directory.
fn apply_recipe(root: &Path, recipe: &Path) {
    let spec: toml::Value =
        toml::from_str(&std::fs::read_to_string(recipe).expect("stage.toml")).expect("stage.toml");

    let git = |args: &[&str]| {
        let ok = Command::new("git")
            .current_dir(root)
            .args(args)
            .status()
            .unwrap()
            .success();
        assert!(ok, "git {args:?} failed");
    };
    git(&["init", "-q", "-b", "main"]);
    git(&["config", "user.email", "fixture@yidam.test"]);
    git(&["config", "user.name", "Fixture"]);

    for commit in spec["commits"].as_array().expect("commits") {
        // Edits first, then stage everything: a commit's `replace` and `write` describe the
        // tree as of that commit, not a change made after it.
        for edit in commit
            .get("replace")
            .and_then(|v| v.as_array())
            .unwrap_or(&Vec::new())
        {
            let path = root.join(edit["file"].as_str().unwrap());
            let text = std::fs::read_to_string(&path).unwrap();
            let from = edit["from"].as_str().unwrap();
            assert!(
                text.contains(from),
                "stage.toml: {} does not contain {from:?}",
                edit["file"].as_str().unwrap()
            );
            std::fs::write(&path, text.replace(from, edit["to"].as_str().unwrap())).unwrap();
        }
        for write in commit
            .get("write")
            .and_then(|v| v.as_array())
            .unwrap_or(&Vec::new())
        {
            let path = root.join(write["file"].as_str().unwrap());
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, write["content"].as_str().unwrap()).unwrap();
        }
        git(&["add", "-A"]);
        // Fixed dates keep `status`'s genesis field stable across runs.
        Command::new("git")
            .current_dir(root)
            .args(["commit", "-q", "-m", commit["message"].as_str().unwrap()])
            .env("GIT_AUTHOR_DATE", "2026-01-01T00:00:00Z")
            .env("GIT_COMMITTER_DATE", "2026-01-01T00:00:00Z")
            .status()
            .unwrap();
    }

    // Refs the sangha and phase reports read. `ma/gauge-reader` is deliberately absent: the
    // fixture registers that elector anyway, so `branch_present: false` is exercised by a
    // golden rather than only by a unit test.
    for branch in spec["branches"].as_array().expect("branches") {
        git(&["branch", branch.as_str().unwrap()]);
    }
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
    redact_features(&redact_first_commit(&redacted))
}

/// Collapse `first_commit` — the commit a corpus-state finding dates from — to a
/// placeholder.
///
/// It is genuinely reproducible: `apply_recipe` fixes the author, the committer, both dates
/// and every byte of content, so the fixture's shas are the same on every machine. It is
/// redacted anyway, for the reason the module header gives about the feature set. A sha is
/// an opaque forty characters that changes whenever `stage.toml` gains a commit *before*
/// this one, so pinning it here would put an unreadable hex diff in front of whoever next
/// extends the fixture — and would say nothing about whether the sha is the right one,
/// since any sha matches any other sha's shape.
///
/// The property it would have pinned is stated where it can be checked exactly instead:
/// `history::tests::the_dated_commit_is_the_one_that_orphaned_the_node` asserts the sha
/// against `git rev-parse` on a repository built for the purpose.
fn redact_first_commit(out: &str) -> String {
    let key = "\"first_commit\": \"";
    let mut result = String::with_capacity(out.len());
    let mut rest = out;
    while let Some(start) = rest.find(key) {
        let after = start + key.len();
        let end = after + rest[after..].find('"').unwrap_or(0);
        result.push_str(&rest[..after]);
        result.push_str("<FIRST_COMMIT>");
        rest = &rest[end..];
    }
    result.push_str(rest);
    result
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
    // `--every 0` so the golden holds every row: a sampled golden would pin the sampler
    // rather than the series, and would move whenever the fixture gained a commit.
    ("replay", &["replay", "--every", "0"]),
    ("diff", &["diff", "HEAD~1..HEAD"]),
    // The same range as `diff`, and deliberately: one reads the corpus change and the
    // other reads the code change, and the fixture's last commit makes both non-empty.
    ("check-diff", &["check-diff", "HEAD~1..HEAD"]),
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
    // The fixture declares no kuten, so this golden pins the arm every repository is in
    // today: holding none is a supported state and reports as one rather than as a finding.
    // The held arm cannot be a golden here — giving the fixture a kuten would move the
    // other twenty goldens — and is covered by `kuten_cluster.rs` against the six shapes
    // that defined the profile.
    ("kuten-check", &["kuten", "check"]),
];

/// Reports checked by running them, because they cannot have a golden.
///
/// Each of these varies with the day it runs — a pin's age in days, a source's overdue count,
/// an index's build date — so a committed golden would go red on the calendar rather than on
/// a change. Their *shape* is checkable all the same, and shape is what the contract is.
///
/// **Every invocation here writes nothing**, which is not incidental: this runs against the
/// same staged fixture the goldens read, and a writer would corrupt it for whatever test ran
/// next. `regen` and `propose` are named with the flags that make that true — `--check` and
/// `--dry-run` — and both refuse to write in those modes by design rather than by luck.
const LIVE: &[(&str, &[&str])] = &[
    ("doctor", &["doctor"]),
    ("due", &["due"]),
    ("regen", &["regen", "--check"]),
    ("propose", &["propose", "--dry-run"]),
];

/// Commands carrying a `--format` flag that this file cannot exercise, and why.
///
/// **An inverted roster, and that is the point.** A list of what *is* covered stops covering
/// new commands without ever going red — the failure this repository has now found in a guard
/// twice. This lists what is not covered, so a command added tomorrow is required by default
/// and fails [`every_reporting_command_has_its_fields_checked`] until somebody decides which
/// bucket it belongs in.
///
/// The reasons are load-bearing. `export`'s `--format` is not the report contract's at all —
/// it names the export format — and it is here so that fact is written down rather than
/// rediscovered.
const NO_REPORT: &[(&str, &str)] = &[
    (
        "export",
        "its `--format` names the export format (bundle, rdf, …), not the report contract",
    ),
    (
        "bench",
        "refuses to run without a committed goal set, and inventing one would measure whoever \
         wrote the fallback",
    ),
    (
        "index-verify",
        "requires a built index, and the `index` feature to build one",
    ),
    ("migrate", "a subcommand group, and every migration writes"),
    ("query", "requires a query expression"),
    ("pack", "requires a query expression"),
    ("estimate", "requires a query expression"),
];

/// Every subcommand whose own `--help` offers `--format`.
///
/// Asked of the built binary rather than of the source, for `cli_reference.rs`'s reason: it
/// is the same question a consumer asks, answered the same way, and it survives a refactor of
/// how the clap enum is spelled. The match is on an option line named exactly `--format` —
/// `bundle` mentions the word in its description and has no such flag.
fn reporting_commands() -> BTreeSet<String> {
    let help = |args: &[&str]| -> String {
        let out = Command::new(env!("CARGO_BIN_EXE_yidam"))
            .args(args)
            .output()
            .expect("running --help");
        String::from_utf8_lossy(&out.stdout).to_string()
    };

    let mut names = BTreeSet::new();
    for line in help(&["--help"]).lines() {
        let Some(rest) = line.strip_prefix("  ") else {
            continue;
        };
        if rest.starts_with(' ') || rest.starts_with('-') {
            continue; // a continuation line, or an option
        }
        let name = rest.split_whitespace().next().unwrap_or_default();
        if name.is_empty()
            || name == "help"
            || !name.chars().all(|c| c.is_ascii_lowercase() || c == '-')
        {
            continue;
        }
        names.insert(name.to_string());
    }
    assert!(
        names.len() > 20,
        "parsed only {} command(s) from --help — the output shape changed and this is no \
         longer reading it: {names:?}",
        names.len()
    );

    let reporting: BTreeSet<String> = names
        .into_iter()
        .filter(|name| {
            help(&[name, "--help"]).lines().any(|l| {
                let t = l.trim_start();
                l.starts_with(' ') && (t.starts_with("--format ") || t.contains(", --format "))
            })
        })
        .collect();
    // The floor that keeps this from passing by finding nothing. A scan that returns an empty
    // set satisfies every "is it covered?" question asked of it, which is how a guard goes
    // green while covering none of its subject.
    assert!(
        reporting.len() > 15,
        "only {} command(s) advertise `--format` — the help layout changed and this filter is \
         no longer matching it: {reporting:?}",
        reporting.len()
    );
    reporting
}

/// Every command that emits the report contract has its fields checked somewhere.
///
/// The guard over the other two checks. `doctor` and `propose` both emitted top-level fields
/// `report.schema.json` did not declare, for as long as they did, because the schema check
/// read only the goldens and neither command has one — a consumer reading the contract was
/// being told nothing about `failed`, `warned`, or the whole of `propose`. Nothing was red.
#[test]
fn every_reporting_command_has_its_fields_checked() {
    let golden: BTreeSet<&str> = COMMANDS.iter().map(|(_, args)| args[0]).collect();
    let live: BTreeSet<&str> = LIVE.iter().map(|(_, args)| args[0]).collect();
    let exempt: BTreeSet<&str> = NO_REPORT.iter().map(|(name, _)| *name).collect();

    let mut uncovered = Vec::new();
    for name in reporting_commands() {
        let n = name.as_str();
        if !golden.contains(n) && !live.contains(n) && !exempt.contains(n) {
            uncovered.push(name);
        }
    }
    assert!(
        uncovered.is_empty(),
        "these commands emit `--format json` and nothing checks their fields against \
         report.schema.json: {uncovered:?}\n\nAdd each to COMMANDS (with a golden), to LIVE \
         (run live, for a report that varies by day), or to NO_REPORT with the reason it \
         cannot be exercised."
    );

    // And the exemptions are not stale: a command that no longer exists, or that lost its
    // `--format`, must not keep a licence to be unchecked.
    //
    // Every name in `NO_REPORT` is in the light default build, which is what makes this
    // symmetric assertion safe. Exempting a **feature-gated** command would fail here on the
    // light build for the honest-looking wrong reason — the command is absent, not stale —
    // and the fix is `help.rs`'s: a companion list licensing the absence, not a weakening of
    // this check.
    let real = reporting_commands();
    let stale: Vec<&str> = NO_REPORT
        .iter()
        .map(|(name, _)| *name)
        .filter(|n| !real.contains(*n))
        .collect();
    assert!(
        stale.is_empty(),
        "NO_REPORT names {stale:?}, which no longer emit a report. An exemption is a licence \
         to go unchecked; delete the ones nothing needs."
    );
}

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

/// The committed contract, parsed once.
fn schema() -> serde_json::Value {
    serde_json::from_str(
        &std::fs::read_to_string(fixture_dir().parent().unwrap().join("report.schema.json"))
            .expect("report.schema.json"),
    )
    .expect("schema is valid JSON")
}

/// One envelope against the contract: every required field present, every emitted field
/// declared.
///
/// Not full JSON-Schema validation — that would want a dependency this crate does not
/// otherwise need. It is the half that actually rots: a field added to a report and not to
/// the schema, which leaves a consumer reading a contract that does not describe the data
/// it is being sent.
fn contract_problems(
    schema: &serde_json::Value,
    name: &str,
    doc: &serde_json::Value,
) -> Vec<String> {
    let declared = schema["properties"].as_object().unwrap();
    let obj = doc.as_object().expect("an envelope is an object");
    let mut problems = Vec::new();
    for key in schema["required"].as_array().unwrap() {
        let key = key.as_str().unwrap();
        if !obj.contains_key(key) {
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
    problems
}

const CONTRACT_ADVICE: &str = "\n\nAdd the field to report.schema.json, or stop emitting it. \
     A consumer reading a contract that does not describe the data it is sent is the failure \
     this contract exists to prevent.";

#[test]
fn every_golden_field_is_declared_in_the_schema() {
    let schema = schema();
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
        problems.extend(contract_problems(&schema, &name, &doc));
    }

    assert!(seen > 0, "no JSON goldens found — the scan is broken");
    assert!(
        problems.is_empty(),
        "goldens and schema disagree:\n{}{CONTRACT_ADVICE}",
        problems.join("\n")
    );
}

/// The same check for the reports that have no golden to read.
///
/// This is the one that was missing. `doctor` shipped `failed` and `warned`, and `propose`
/// shipped its entire report — six top-level fields — with none of them in
/// `report.schema.json`, because the check above reads the `expected/` directory and neither
/// command has a file in it. The contract was wrong about two commands and nothing could say
/// so.
#[test]
fn every_live_report_field_is_declared_in_the_schema() {
    let schema = schema();
    let tmp = stage();
    let mut problems = Vec::new();
    for (name, args) in LIVE {
        let mut a = args.to_vec();
        a.extend_from_slice(&["--format", "json"]);
        let r = run(tmp.path(), &a);
        let doc: serde_json::Value = serde_json::from_str(&r.stdout).unwrap_or_else(|e| {
            panic!(
                "`yidam {}` did not emit JSON: {e}\n{}",
                a.join(" "),
                r.stdout
            )
        });
        problems.extend(contract_problems(&schema, name, &doc));
    }
    assert!(
        problems.is_empty(),
        "live reports and schema disagree:\n{}{CONTRACT_ADVICE}",
        problems.join("\n")
    );
}

/// A report run live must still leave the fixture alone.
///
/// [`every_live_report_field_is_declared_in_the_schema`] runs four commands against a staged
/// repository, two of them commands that write in their default mode. If `--check` or
/// `--dry-run` ever stopped holding, that test would silently start rewriting its own
/// subject — and the first symptom would be a golden elsewhere drifting for no reason anyone
/// could trace.
#[test]
fn running_the_live_reports_writes_nothing() {
    let tmp = stage();
    let before = tree(tmp.path());
    assert!(!before.is_empty(), "the fixture staged nothing");
    for (_, args) in LIVE {
        let mut a = args.to_vec();
        a.extend_from_slice(&["--format", "json"]);
        run(tmp.path(), &a);
    }
    assert_eq!(
        tree(tmp.path()),
        before,
        "a LIVE invocation wrote to the fixture it was pointed at"
    );
}

/// Every file's content, keyed by relative path. `.git/` is excluded — `propose` writes git
/// objects and a ref by design, and what is asserted here is the working tree.
fn tree(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    walkdir::WalkDir::new(root)
        .into_iter()
        .filter_entry(|e| e.file_name() != ".git")
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
        .map(|e| {
            (
                e.path().strip_prefix(root).unwrap().to_path_buf(),
                std::fs::read(e.path()).unwrap(),
            )
        })
        .collect()
}

/// A real report carries a feature array, and the redaction the goldens rely on finds it.
///
/// This replaces `the_light_build_reports_exactly_reports`, which asserted that the light
/// build names exactly one feature. That was true when `default = ["reports"]`, and it was
/// kept true afterwards by *retreating*: `tonpa` joined the default set and was added to a
/// `cfg(not(any(…)))` so the test would stop running, rather than to the expectation. When
/// `vault-s3` joined, nobody added it — so the test survived by only ever running in builds
/// no CI compiles, under a name claiming it was about the light one. #482.
///
/// What it was for now lives in `light_build.rs`, which holds `report.rs`'s list against the
/// features Cargo.toml declares and does not retreat when the set grows. What is left here
/// is the part that belongs beside the goldens: that `redact_features` fires on the real
/// thing, and not only on the string literal `the_feature_set_is_redacted_whatever_it_holds`
/// hands it.
#[test]
fn the_feature_array_is_redacted_out_of_a_real_report() {
    let tmp = stage();
    let r = run(tmp.path(), &["status", "--format", "json"]);
    // Read from the unredacted process output — `redact` is what this test exists beside.
    let out = Command::new(env!("CARGO_BIN_EXE_yidam"))
        .current_dir(tmp.path())
        .args(["status", "--format", "json"])
        .output()
        .unwrap();
    let doc: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let named = doc["yidam"]["features"]
        .as_array()
        .expect("a report must carry a feature array");
    assert!(
        named.iter().any(|f| f == "reports"),
        "every build carries the base, whatever else it has: {named:?}"
    );
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
