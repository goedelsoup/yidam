//! `yidam doctor` — the one command you can point at a repository you did not write.
//!
//! Two properties are asserted here rather than in the unit tests, because both are about
//! the *process*: what it leaves behind on disk, and what it returns to a shell.
//!
//! The first is the reason this command exists in the shape it does. Ten subcommands
//! rewrite a README block in whatever repository they are run against — `status` most
//! notably, which reads like a read and is not. `doctor` calls the same generators, in the
//! same order, through `regen --check`'s non-writing mode. If that mode ever stops holding,
//! `doctor` silently becomes a writer, and the first person to find out will be someone who
//! ran it against a checkout they only meant to inspect.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

mod common;

fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../prelude/sdks/parity/fixtures/reports/basic")
}

/// A derived repository with a deliberately stale `yidam status` REGEN block — the fixture
/// ships a placeholder, so a fresh copy is stale by construction. That is the case where a
/// writing `doctor` would be caught.
fn stage() -> tempfile::TempDir {
    let tmp = tempfile::tempdir().unwrap();
    let src = fixture_dir().join("repo");
    for entry in walkdir::WalkDir::new(&src)
        .into_iter()
        .filter_map(Result::ok)
    {
        let rel = entry.path().strip_prefix(&src).unwrap();
        let dest = tmp.path().join(rel);
        if entry.file_type().is_dir() {
            std::fs::create_dir_all(&dest).unwrap();
        } else {
            std::fs::create_dir_all(dest.parent().unwrap()).unwrap();
            std::fs::copy(entry.path(), &dest).unwrap();
        }
    }
    let git = |args: &[&str]| common::git::git_at(tmp.path(), args, common::git::FIXTURE_DATE);
    git(&["init", "-q", "-b", "main"]);
    git(&["config", "user.email", "fixture@yidam.test"]);
    git(&["config", "user.name", "Fixture"]);
    git(&["add", "-A"]);
    git(&["commit", "-q", "-m", "genesis: reports fixture"]);
    tmp
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
        stdout: String::from_utf8_lossy(&out.stdout).to_string(),
        code: out.status.code().unwrap_or(-1),
    }
}

/// Every file's content, keyed by relative path. `.git/` is excluded — running any git
/// command churns it, and it is not what "writes nothing" is about.
fn contents(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
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

/// The property the command is built around.
#[test]
fn doctor_changes_nothing_on_disk() {
    let tmp = stage();
    let before = contents(tmp.path());
    assert!(!before.is_empty(), "the fixture staged nothing");

    let r = run(tmp.path(), &["doctor"]);
    assert_eq!(
        contents(tmp.path()),
        before,
        "doctor wrote to the repository it was pointed at"
    );

    // And it did find the stale block — otherwise the assertion above passes for the
    // uninteresting reason that nothing needed writing.
    assert!(r.stdout.contains("regen"), "{}", r.stdout);
    assert_eq!(r.code, 1, "a stale REGEN block is a failing check");
}

/// `regen --check` and `doctor` must reach the same verdict about the same repository.
/// They share a generator list precisely so they cannot drift; this is the assertion that
/// the sharing survived.
#[test]
fn doctor_and_regen_check_agree_about_staleness() {
    let tmp = stage();
    let stale = run(tmp.path(), &["regen", "--check"]);
    assert_eq!(stale.code, 1, "the fixture's status block ships stale");

    let doctor = run(tmp.path(), &["doctor"]);
    assert!(
        doctor.stdout.contains("fail ") && doctor.stdout.contains("block(s) stale"),
        "doctor did not report the staleness regen --check found:\n{}",
        doctor.stdout
    );

    // Refresh, and both must go green on the regen question.
    assert_eq!(run(tmp.path(), &["regen"]).code, 0);
    assert_eq!(run(tmp.path(), &["regen", "--check"]).code, 0);
    let after = run(tmp.path(), &["doctor", "--format", "json"]);
    let v: serde_json::Value = serde_json::from_str(&after.stdout).unwrap();
    let regen = v["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == "regen")
        .expect("a regen check");
    assert_eq!(regen["verdict"], "ok", "{}", after.stdout);
}

/// The question a new collaborator asks first, asked from the wrong directory. One answer,
/// not seven — and a nonzero exit, because a gate that cannot see its repository must never
/// report that the repository is fine.
#[test]
fn outside_a_derived_repository_it_says_so_and_exits_nonzero() {
    let tmp = tempfile::tempdir().unwrap();
    let r = run(tmp.path(), &["doctor"]);
    assert_eq!(r.code, 1, "{}", r.stdout);
    assert!(r.stdout.contains("fail  repository"), "{}", r.stdout);
    // The build line is answerable anywhere, and is what a person debugging this needs.
    assert!(r.stdout.contains("build"), "{}", r.stdout);
    assert!(r.stdout.contains("skip"), "{}", r.stdout);
}

/// **The same questions on both paths**, compared between two runs of the real binary
/// rather than against a list written down anywhere.
///
/// `doctor` had two constructors — a healthy one and an early-return one that built a
/// hand-written roster of ids to report as `skipped` — and `policy` was in the first and not
/// the second. So on a directory that is not a derived repository the check did not appear
/// at all, which the report contract says must be distinguishable from one that ran and
/// found nothing (#656). A list in this test would be a third copy that agrees with whichever
/// one it was written from; two runs compared to each other cannot be.
#[test]
fn both_paths_ask_the_same_questions_and_the_unanswerable_ones_say_skipped() {
    let ids = |v: &serde_json::Value| -> Vec<String> {
        v["checks"]
            .as_array()
            .expect("checks")
            .iter()
            .map(|c| c["id"].as_str().unwrap().to_string())
            .collect()
    };
    let report = |root: &Path| -> serde_json::Value {
        let r = run(root, &["doctor", "--format", "json"]);
        serde_json::from_str(&r.stdout).expect("valid JSON")
    };

    let answered = report(stage().path());
    let outside = report(tempfile::tempdir().unwrap().path());
    assert_eq!(
        ids(&answered),
        ids(&outside),
        "a question answered in a repository and absent outside one cannot be told from a \
         question that never ran"
    );

    // And absent is not what they are: every question that needs a repository says so.
    for check in outside["checks"].as_array().unwrap() {
        let id = check["id"].as_str().unwrap();
        if id == "repository" || id == "build" {
            continue;
        }
        assert_eq!(check["verdict"], "skipped", "{check:#?}");
        assert!(check["remedy"].is_null(), "{check:#?}");
    }
}

/// The report contract, not the prose. A consumer keys on `id` and `verdict`.
#[test]
fn the_json_report_carries_the_envelope_and_every_check() {
    let tmp = stage();
    let r = run(tmp.path(), &["doctor", "--format", "json"]);
    let v: serde_json::Value = serde_json::from_str(&r.stdout).expect("valid JSON");
    assert_eq!(v["format_version"], "1");
    assert!(v["yidam"]["commit"].is_string());
    assert_eq!(v["passed"], false);

    let ids: Vec<&str> = v["checks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["id"].as_str().unwrap())
        .collect();
    let mut want = [
        "repository",
        "provenance",
        "binary",
        "path",
        "prelude",
        "index",
        "computed",
        "regen",
        "routes",
        "catalog",
        "corpora",
        "corpus",
        "contract",
        "vault",
        "remote-index",
        "policy",
        "governance",
        "kuten",
        "kuten-read",
        "build",
    ];
    let mut got = ids.clone();
    got.sort_unstable();
    want.sort_unstable();
    assert_eq!(got, want, "the set of questions changed: {ids:?}");
}

/// **Where a broken policy is caught**, decided on RFC-0024's fourth open question.
///
/// Not a new mise task and not a new CI job in every derived repository: `doctor` is already
/// offline and read-only, and derived CI already runs it. A rule that cannot be evaluated
/// refuses nothing, so this fails rather than warns.
#[test]
fn a_policy_that_does_not_compile_fails_the_doctor() {
    let tmp = stage();
    let dir = tmp.path().join(".yidam/policy");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("record.rego"),
        "package yidam.disclose.record\n{{{\n",
    )
    .unwrap();

    let r = run(tmp.path(), &["doctor", "--format", "json"]);
    let v: serde_json::Value = serde_json::from_str(&r.stdout).expect("doctor emits JSON");
    let policy = v["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == "policy")
        .expect("doctor asks about policy");
    assert_eq!(policy["verdict"], "fail", "{policy:#?}");
}

/// An override is reported and is **not** a failure. The repository decided; `lint`'s
/// `policy-override` is what makes the decision visible, at `Info`, gating nothing.
#[test]
fn an_overridden_decision_is_named_and_does_not_fail_the_doctor() {
    let tmp = stage();
    let dir = tmp.path().join(".yidam/policy");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("record.rego"),
        "package yidam.disclose.record\n\ndecision := {\"allow\": true, \"deny\": []}\n",
    )
    .unwrap();

    let r = run(tmp.path(), &["doctor", "--format", "json"]);
    let v: serde_json::Value = serde_json::from_str(&r.stdout).expect("doctor emits JSON");
    let policy = v["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == "policy")
        .unwrap();
    assert_eq!(policy["verdict"], "ok", "{policy:#?}");
    assert!(
        policy["detail"]
            .as_str()
            .unwrap()
            .contains("disclose/record"),
        "the overridden decision must be named: {policy:#?}"
    );
}

/// A warning is not a failure, and `--strict` is how a CI job says it wants it to be.
/// Asserted on a repository whose only complaints are warnings.
#[test]
fn strict_is_what_turns_a_warning_into_a_nonzero_exit() {
    let tmp = stage();
    // Clear the one real failure so warnings are all that remain.
    assert_eq!(run(tmp.path(), &["regen"]).code, 0);
    std::fs::write(
        tmp.path().join(".yidam.toml"),
        "[yidam]\ncommit = \"0123456789abcdef0123456789abcdef01234567\"\n\
         template = \"untagged\"\ncommitted = \"2020-01-01\"\n",
    )
    .unwrap();

    let lenient = run(tmp.path(), &["doctor"]);
    assert_eq!(
        lenient.code, 0,
        "warnings alone must not gate:\n{}",
        lenient.stdout
    );
    assert!(
        lenient.stdout.contains("nothing broken"),
        "{}",
        lenient.stdout
    );

    let strict = run(tmp.path(), &["doctor", "--strict"]);
    assert_eq!(strict.code, 1, "{}", strict.stdout);
}

/// **The state #694 found in the field.** A corpus adopted `inquiry` into a repository with
/// no `AGENTS.md`, and every surface reported it as fine: `regen --check` exits 0, `doctor`
/// said `ok kuten`, and `yidam kuten` printed the block to stdout and wrote nothing.
///
/// `adopt` was right to decline the write — conjuring an `AGENTS.md` from a command asked to
/// record a decision reaches past what was asked. The defect was that nothing said so, at
/// adoption or ever after. This is the "ever after" half.
fn declaring_a_kuten(root: &Path) {
    let profile =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../prelude/kuten/inquiry/kuten.yml");
    let text = std::fs::read_to_string(&profile).expect("the shipped profile");
    // The record names the revision the profile *actually* ships at, not a literal. These two
    // tests are about whether a declaration reaches a document; a revision bumped upstream —
    // #692 bumped it to 2 — would otherwise turn both into skew-warning tests instead.
    let revision = yidam::kuten::Profile::parse(&text)
        .expect("the binary can read the shipped profile")
        .revision;
    let vendored = root.join(".yidam/.vendor/prelude/kuten/inquiry");
    std::fs::create_dir_all(&vendored).unwrap();
    std::fs::write(vendored.join("kuten.yml"), &text).unwrap();
    std::fs::create_dir_all(root.join(".yidam/decisions")).unwrap();
    std::fs::write(
        root.join(".yidam/decisions/kuten.yml"),
        format!("id: kuten\nkuten: inquiry\nrevision: {revision}\n"),
    )
    .unwrap();
}

fn check<'a>(v: &'a serde_json::Value, id: &str) -> &'a serde_json::Value {
    v["checks"]
        .as_array()
        .expect("checks")
        .iter()
        .find(|c| c["id"] == id)
        .unwrap_or_else(|| panic!("doctor asks about {id}"))
}

#[test]
fn a_kuten_no_document_carries_is_reported_rather_than_passed() {
    let tmp = stage();
    declaring_a_kuten(tmp.path());
    assert!(
        !tmp.path().join("AGENTS.md").exists(),
        "the fixture must be in the state the field was in"
    );

    let r = run(tmp.path(), &["doctor", "--format", "json"]);
    let v: serde_json::Value = serde_json::from_str(&r.stdout).expect("doctor emits JSON");

    // The declaration itself is sound, and that check must go on saying so — the two
    // questions are separate, which is why this is a separate id.
    assert_eq!(
        check(&v, "kuten")["verdict"],
        "ok",
        "{:#?}",
        check(&v, "kuten")
    );

    let read = check(&v, "kuten-read");
    assert_eq!(read["verdict"], "warn", "{read:#?}");
    assert!(
        read["detail"].as_str().unwrap().contains("`inquiry`"),
        "{read:#?}"
    );
    assert!(
        read["remedy"].as_str().unwrap().contains("AGENTS.md"),
        "a warning must name the repair: {read:#?}"
    );
}

/// The same repository once something carries the block. `AGENTS.md` is where `adopt` and the
/// scaffold put it, and the marker is what decides — not the filename.
#[test]
fn a_kuten_a_document_carries_reads_as_held() {
    let tmp = stage();
    declaring_a_kuten(tmp.path());
    std::fs::write(
        tmp.path().join("AGENTS.md"),
        "# Agents\n\n<!-- REGEN: yidam kuten\n-->\n_Run `yidam kuten` to populate._\n\
         <!-- /REGEN -->\n",
    )
    .unwrap();

    let r = run(tmp.path(), &["doctor", "--format", "json"]);
    let v: serde_json::Value = serde_json::from_str(&r.stdout).expect("doctor emits JSON");
    let read = check(&v, "kuten-read");
    assert_eq!(read["verdict"], "ok", "{read:#?}");
    assert_eq!(read["remedy"], serde_json::Value::Null);
}

/// Holding no kuten skips the question rather than passing it. There is nothing to carry, and
/// an `ok` would read as *something was checked and found present* — which is the shape of
/// error this whole check exists to correct.
#[test]
fn a_repository_holding_no_kuten_is_not_asked_whether_anything_reads_one() {
    let tmp = stage();
    let r = run(tmp.path(), &["doctor", "--format", "json"]);
    let v: serde_json::Value = serde_json::from_str(&r.stdout).expect("doctor emits JSON");
    assert_eq!(check(&v, "kuten-read")["verdict"], "skipped");
}

/// An `AGENTS.md` from before RFC-0039 has no `yidam routes` block, and `regen --check` cannot
/// see a block that is missing. This is the one surface that says so (#1135).
#[test]
fn an_agents_md_with_no_routes_block_is_reported_with_the_migration() {
    let tmp = stage();
    std::fs::write(
        tmp.path().join("AGENTS.md"),
        "# Agents\n\n## Before taking substantive action\n\n\
         - [Identity](.yidam/.vendor/prelude/IDENTITY.md)\n",
    )
    .unwrap();
    let routes = tmp.path().join(".yidam/.vendor/prelude/routes.yml");
    std::fs::create_dir_all(routes.parent().unwrap()).unwrap();
    std::fs::write(&routes, "always: []\noccasions: []\nreference: []\n").unwrap();

    let r = run(tmp.path(), &["doctor", "--format", "json"]);
    let v: serde_json::Value = serde_json::from_str(&r.stdout).expect("doctor emits JSON");
    let c = check(&v, "routes");
    assert_eq!(c["verdict"], "warn", "{c:#?}");
    assert_eq!(
        c["remedy"], "yidam migrate routes, committed as a `migrate:` commit",
        "{c:#?}"
    );
}

/// Before a re-vendor there are no routes to migrate to, and the remedy says to fetch them first.
#[test]
fn a_routes_warning_before_a_revendor_names_the_revendor_first() {
    let tmp = stage();
    std::fs::write(tmp.path().join("AGENTS.md"), "# Agents\n").unwrap();
    let r = run(tmp.path(), &["doctor", "--format", "json"]);
    let v: serde_json::Value = serde_json::from_str(&r.stdout).expect("doctor emits JSON");
    let c = check(&v, "routes");
    assert_eq!(c["verdict"], "warn", "{c:#?}");
    assert!(
        c["remedy"]
            .as_str()
            .unwrap()
            .starts_with("mise run yidam-vendor-update"),
        "{c:#?}"
    );
}

#[test]
fn an_agents_md_carrying_the_routes_block_reads_as_held() {
    let tmp = stage();
    std::fs::write(
        tmp.path().join("AGENTS.md"),
        "# Agents\n\n<!-- REGEN: yidam routes\n-->\n_Run `yidam routes`._\n<!-- /REGEN -->\n",
    )
    .unwrap();
    let r = run(tmp.path(), &["doctor", "--format", "json"]);
    let v: serde_json::Value = serde_json::from_str(&r.stdout).expect("doctor emits JSON");
    let c = check(&v, "routes");
    assert_eq!(c["verdict"], "ok", "{c:#?}");
    assert_eq!(c["remedy"], serde_json::Value::Null);
}

#[test]
fn a_repository_with_no_agents_md_is_not_asked_about_routes() {
    let tmp = stage();
    let r = run(tmp.path(), &["doctor", "--format", "json"]);
    let v: serde_json::Value = serde_json::from_str(&r.stdout).expect("doctor emits JSON");
    assert_eq!(check(&v, "routes")["verdict"], "skipped");
}

/// The line `yidam-vendor-update` runs to ask `contract` (#1078), out of the file that ships.
///
/// By content rather than by line number, and exactly one: the task keeps it on one line so
/// this test runs what a re-vendor runs, and not a copy of it.
fn vendor_contract_line() -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../mise.yidam.toml");
    let doc: toml::Table = std::fs::read_to_string(path)
        .expect("mise.yidam.toml is readable")
        .parse()
        .expect("mise.yidam.toml parses");
    let body = doc["yidam-vendor-update"]["run"]
        .as_str()
        .unwrap()
        .to_string();
    let found: Vec<&str> = body
        .lines()
        .filter(|l| l.contains("doctor --only contract"))
        .collect();
    assert_eq!(found.len(), 1, "one line asks `contract`: {found:?}");
    found[0].trim().to_string()
}

/// Run the vendor task's line in `root`, under the task's own `set -eu`, with `bin` as the
/// whole of `PATH`. Returns stdout, and fails the test if the line would stop a re-vendor.
fn run_vendor_line(root: &Path, bin: &Path) -> String {
    let out = Command::new("/bin/sh")
        .args(["-euc", &vendor_contract_line()])
        .current_dir(root)
        .env("PATH", bin)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "the line must never fail a re-vendor: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).to_string()
}

/// A directory holding only a `yidam` that is this build.
fn bin_dir() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::os::unix::fs::symlink(env!("CARGO_BIN_EXE_yidam"), dir.path().join("yidam")).unwrap();
    dir
}

/// **A re-vendor asks the question** (#1078). ohio-education-funding and matt-huffman both
/// re-vendored past the commit that introduced `required:`, and neither was told. The line
/// prints the warning when the ontology never answered, and prints nothing once it has.
#[test]
fn a_re_vendor_asks_whether_the_ontology_stated_its_contract() {
    let tmp = stage();
    let bin = bin_dir();

    // The fixture answers `required:` on one class and states no `edge_policy:` anywhere.
    let out = run_vendor_line(tmp.path(), bin.path());
    assert!(out.contains("warn  contract"), "{out}");
    assert!(out.contains("`edge_policy:`"), "{out}");

    for class in ["concept", "gauge"] {
        let path = tmp.path().join(format!(".yidam/corpus/{class}.ont.yml"));
        let mut text = std::fs::read_to_string(&path).unwrap();
        text.push_str("edge_policy: characteristic\n");
        std::fs::write(&path, text).unwrap();
    }
    let out = run_vendor_line(tmp.path(), bin.path());
    assert_eq!(out, "", "an ontology that answered is not told again");
}

/// With no `yidam` on `PATH`, or one too old to know `--only`, the line prints nothing and
/// the re-vendor goes on. The old binary is a stand-in that refuses the flag the way clap
/// does: usage on stderr, nothing on stdout, exit 2.
#[test]
fn a_re_vendor_without_a_binary_that_knows_the_question_is_quiet() {
    use std::os::unix::fs::PermissionsExt;
    let tmp = stage();
    let empty = tempfile::tempdir().unwrap();
    assert_eq!(run_vendor_line(tmp.path(), empty.path()), "");

    let old = tempfile::tempdir().unwrap();
    let yidam = old.path().join("yidam");
    std::fs::write(
        &yidam,
        "#!/bin/sh\necho \"error: unexpected argument '--only' found\" >&2\nexit 2\n",
    )
    .unwrap();
    std::fs::set_permissions(&yidam, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(run_vendor_line(tmp.path(), old.path()), "");
}
