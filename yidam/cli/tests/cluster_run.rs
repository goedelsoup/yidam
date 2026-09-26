//! The cluster contract, exercised against a bare remote and a `file://` vault — and the
//! boundary it exists for, tested by trying to break it (#475).
//!
//! Every pod of a generated workflow is one `yidam cluster <sub>` invocation, so the whole
//! run can be driven here without Argo: pin, step, land, in the order the DAG imposes, with
//! the records passed between them as the workflow passes them. What Argo adds — scheduling,
//! retries, which secret is mounted where — is asserted over the generated manifest in
//! `src/cmd/cluster/workflow.rs`'s own tests and pinned as the worked example under
//! `docs/cluster/`.
//!
//! # The boundary
//!
//! RFC-0026 §3 requires that the invariant — a run authors operational commits and only
//! *proposes* epistemic ones — is not expressible as a manifest field or a policy override.
//! The cluster answers with a credential, and the tests below hold the credential to account
//! rather than the code path: a step pod handed a valid, well-formed commit and a remote it
//! cannot write to does not land it, and what it hears back is the remote refusing, not a
//! check in this binary agreeing to refuse. [`a_valid_sha_without_the_write_credential_does_not_land`]
//! is that test. If it ever passes because of a check, the invariant has become a code path
//! again and the issue reopens.

mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

use common::git::{git, out, succeeded};
use common::Example;
use serde_json::Value;

/// A corpus, the bare remote it pushes to, a vault directory, and a workspace for records.
struct Cluster {
    e: Example,
    work: tempfile::TempDir,
}

impl Cluster {
    fn new() -> Self {
        let e = Example::materialize("streamflow");
        let work = tempfile::tempdir().unwrap();
        let remote = work.path().join("remote.git");
        git(work.path(), &["init", "-q", "--bare", "remote.git"]);
        std::fs::create_dir_all(work.path().join("vault")).unwrap();
        git(
            &e.path(),
            &["remote", "add", "origin", remote.to_str().unwrap()],
        );
        git(&e.path(), &["push", "-q", "origin", "HEAD:refs/heads/main"]);
        Self { e, work }
    }

    fn remote(&self) -> PathBuf {
        self.work.path().join("remote.git")
    }

    fn vault_url(&self) -> String {
        format!("file://{}", self.work.path().join("vault").display())
    }

    /// Run the binary in the workspace — deliberately **not** in a checkout, because no pod
    /// stands in one either.
    fn yidam(&self, args: &[&str]) -> (String, String, i32) {
        let o = Command::new(env!("CARGO_BIN_EXE_yidam"))
            .current_dir(self.work.path())
            .args(args)
            // A `file://` vault reads no credential, and a developer's ambient AWS session
            // must not be what decides whether this suite passes.
            .env_remove("AWS_ACCESS_KEY_ID")
            .env_remove("AWS_SECRET_ACCESS_KEY")
            .env_remove("AWS_SESSION_TOKEN")
            .env_remove("AWS_PROFILE")
            .output()
            .unwrap();
        (
            String::from_utf8_lossy(&o.stdout).to_string(),
            String::from_utf8_lossy(&o.stderr).to_string(),
            o.status.code().unwrap_or(-1),
        )
    }

    /// The record a command delivers under `--format json`, unwrapped from the report.
    fn record(&self, args: &[&str]) -> Value {
        let mut full: Vec<&str> = args.to_vec();
        full.extend(["--format", "json"]);
        let (stdout, stderr, code) = self.yidam(&full);
        assert_eq!(code, 0, "yidam {args:?} failed:\n{stdout}{stderr}");
        let v: Value = serde_json::from_str(&stdout)
            .unwrap_or_else(|e| panic!("yidam {args:?} did not print JSON: {e}\n{stdout}"));
        assert_eq!(
            v["format_version"], "1",
            "the report envelope is the report module's, whatever the record's own version"
        );
        let record = v["record"].clone();
        assert_eq!(
            record["format_version"], 1,
            "the record carries the contract version"
        );
        record
    }

    fn remote_args(&self) -> Vec<String> {
        vec!["--remote".into(), self.remote().display().to_string()]
    }

    fn vault_args(&self) -> Vec<String> {
        vec!["--vault-url".into(), self.vault_url()]
    }

    fn pin(&self) -> Value {
        let mut args = vec!["cluster".to_string(), "pin".to_string()];
        args.extend(self.remote_args());
        args.extend(self.vault_args());
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        self.record(&args)
    }

    fn step(&self, name: &str, bundle: &str) -> Value {
        let mut args = vec![
            "cluster".to_string(),
            "step".to_string(),
            name.to_string(),
            "--bundle".to_string(),
            bundle.to_string(),
        ];
        args.extend(self.vault_args());
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        self.record(&args)
    }

    /// Hand a step record to the lander the way the workflow does: as a file.
    fn land_raw(&self, record: &Value) -> (String, String, i32) {
        let file = self.work.path().join("step.json");
        std::fs::write(&file, serde_json::to_string(record).unwrap()).unwrap();
        let mut args = vec![
            "cluster".to_string(),
            "land".to_string(),
            "--step-output".to_string(),
            format!("@{}", file.display()),
        ];
        args.extend(self.remote_args());
        args.extend(self.vault_args());
        args.extend(["--format".to_string(), "json".to_string()]);
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        self.yidam(&args)
    }

    fn land(&self, record: &Value) -> Value {
        let (stdout, stderr, code) = self.land_raw(record);
        assert_eq!(code, 0, "cluster land failed:\n{stdout}{stderr}");
        let v: Value = serde_json::from_str(&stdout).unwrap();
        v["record"].clone()
    }

    fn admit(&self) -> Value {
        let mut args = vec!["cluster".to_string(), "admit".to_string()];
        args.extend(self.remote_args());
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        self.record(&args)
    }

    fn remote_ref(&self, name: &str) -> Option<String> {
        let o = common::git::raw(&self.remote(), &["rev-parse", "--verify", "-q", name]);
        o.status
            .success()
            .then(|| String::from_utf8_lossy(&o.stdout).trim().to_string())
    }

    fn main_tip(&self) -> String {
        self.remote_ref("refs/heads/main")
            .expect("the remote has a main")
    }

    /// Bring the corpus checkout to the remote's `main`, so a commit made here can be pushed.
    fn sync(&self) {
        git(
            &self.e.path(),
            &["fetch", "-q", "origin", "refs/heads/main"],
        );
        git(&self.e.path(), &["reset", "-q", "--hard", "FETCH_HEAD"]);
    }

    /// Commit `message` over whatever `edit` did to the checkout, and push it to `main`.
    fn push_to_main(&self, message: &str, edit: impl FnOnce(&Path)) -> String {
        self.sync();
        edit(&self.e.path());
        git(&self.e.path(), &["add", "-A"]);
        git(&self.e.path(), &["commit", "-q", "-m", message]);
        git(
            &self.e.path(),
            &["push", "-q", "origin", "HEAD:refs/heads/main"],
        );
        out(&self.e.path(), &["rev-parse", "HEAD"])
    }

    /// Subjects on the remote's `main`, newest first.
    fn subjects(&self, n: usize) -> Vec<String> {
        out(
            &self.remote(),
            &["log", &format!("-{n}"), "--format=%s", "refs/heads/main"],
        )
        .lines()
        .map(str::to_string)
        .collect()
    }
}

fn s(v: &Value) -> &str {
    v.as_str().unwrap_or_else(|| panic!("not a string: {v}"))
}

// ── the pipeline ──────────────────────────────────────────────────────────────

/// Pin, step, land, step, land: both steps of the manifest, each landed on `main` with its
/// receipt, and afterwards nothing owed.
#[test]
fn the_pipeline_lands_every_step_with_its_receipt_and_then_owes_nothing() {
    let c = Cluster::new();
    let before = c.main_tip();

    let admission = c.admit();
    assert_eq!(admission["admitted"], true, "{admission}");
    assert_eq!(
        admission["stale"],
        serde_json::json!(["travel-tier", "disclosure-envelope"]),
        "both steps are stale in a corpus nothing has run in"
    );

    let pin = c.pin();
    assert_eq!(pin["branch"], "main");
    assert_eq!(s(&pin["sha"]), before);
    let bundle = s(&pin["bundle"]).to_string();
    assert!(
        c.work.path().join("vault").join(&bundle[..2]).exists()
            || std::fs::read_dir(c.work.path().join("vault"))
                .unwrap()
                .count()
                > 0,
        "the pin was put in the vault"
    );

    // ── step one ──
    let step = c.step("travel-tier", &bundle);
    assert_eq!(step["outcome"], "ran", "{step}");
    assert_eq!(step["class"], "operational");
    assert_eq!(step["verb"], "compute");
    assert_eq!(step["input"], before.as_str());
    assert_eq!(step["receipt"], ".yidam/runs/travel-tier.yml");
    let built = s(&step["sha"]).to_string();
    assert!(step["bundle"].is_string(), "a ran step bundles its commit");
    assert_eq!(
        c.main_tip(),
        before,
        "the step wrote to the vault and nowhere else: main did not move"
    );

    let landed = c.land(&step);
    assert_eq!(landed["target"], "main");
    assert_eq!(landed["reparented"], false);
    assert_eq!(landed["attempts"], 1);
    assert_eq!(s(&landed["landed"]), built);
    assert_eq!(
        c.main_tip(),
        built,
        "the lander moved main to the step's commit"
    );
    assert_eq!(
        landed["next"]["sha"],
        built.as_str(),
        "the next pin is the branch after landing"
    );

    // ── step two, on the pin the lander took ──
    let step2 = c.step("disclosure-envelope", s(&landed["next"]["bundle"]));
    assert_eq!(step2["outcome"], "ran", "{step2}");
    assert_eq!(step2["input"], built.as_str());
    let landed2 = c.land(&step2);
    assert_eq!(s(&landed2["landed"]), s(&step2["sha"]));

    let subjects = c.subjects(3);
    assert!(
        subjects[0].starts_with("compute:") && subjects[1].starts_with("compute:"),
        "two operational commits landed on main: {subjects:?}"
    );
    for step in ["travel-tier", "disclosure-envelope"] {
        assert!(
            succeeded(
                &c.remote(),
                &[
                    "cat-file",
                    "-e",
                    &format!("refs/heads/main:.yidam/runs/{step}.yml")
                ]
            ),
            "`{step}`'s receipt is on the branch — the receipt is the provenance, not the log"
        );
    }

    // ── afterwards ──
    let admission = c.admit();
    assert_eq!(admission["admitted"], false, "{admission}");
    assert_eq!(admission["stale"], serde_json::json!([]));
    assert!(
        s(&admission["because"]).contains("nothing is owed"),
        "{admission}"
    );

    let fresh = c.step("travel-tier", s(&landed2["next"]["bundle"]));
    assert_eq!(fresh["outcome"], "fresh", "{fresh}");
    assert!(fresh["sha"].is_null() && fresh["bundle"].is_null());
    let nothing = c.land(&fresh);
    assert!(nothing["landed"].is_null() && nothing["target"].is_null());
    assert_eq!(nothing["next"]["sha"], s(&step2["sha"]));
}

// ── the boundary ──────────────────────────────────────────────────────────────

fn set_mode_recursively(dir: &Path, dir_mode: u32, file_mode: u32) {
    use std::os::unix::fs::PermissionsExt;
    for entry in std::fs::read_dir(dir).unwrap() {
        let p = entry.unwrap().path();
        if p.is_dir() {
            set_mode_recursively(&p, dir_mode, file_mode);
            std::fs::set_permissions(&p, std::fs::Permissions::from_mode(dir_mode)).unwrap();
        } else {
            std::fs::set_permissions(&p, std::fs::Permissions::from_mode(file_mode)).unwrap();
        }
    }
    std::fs::set_permissions(dir, std::fs::Permissions::from_mode(dir_mode)).unwrap();
}

/// The definition of done, verbatim: a step pod handed a valid commit sha and no ref
/// credential cannot land it, and the failure is a permission error rather than a check.
///
/// The remote is the credential here. A pod that can read it can pin from it; a pod that
/// cannot write to it cannot move a ref on it, and there is nothing in this binary to argue
/// with about that. So: a well-formed step record, its commit in the vault, and a remote the
/// process cannot write. The lander's exit is non-zero, its stderr is git's refusal in git's
/// words, and `main` is where it was. Then the same record, with the permission restored,
/// lands — which is what shows the first refusal was the permission and nothing else.
#[test]
fn a_valid_sha_without_the_write_credential_does_not_land() {
    let c = Cluster::new();
    let before = c.main_tip();
    let pin = c.pin();
    let step = c.step("travel-tier", s(&pin["bundle"]));
    assert_eq!(step["outcome"], "ran", "{step}");

    set_mode_recursively(&c.remote(), 0o555, 0o444);
    let probe = std::fs::write(c.remote().join("objects").join("probe"), b"");
    if probe.is_ok() {
        // Root, or a filesystem that ignores mode bits: this process can write anything, and
        // the remote cannot stand in for a missing credential. The assertion below would be
        // measuring nothing, so it does not run.
        let _ = std::fs::remove_file(c.remote().join("objects").join("probe"));
        set_mode_recursively(&c.remote(), 0o755, 0o644);
        eprintln!(
            "skipped: this process writes a read-only directory, so the remote cannot refuse it"
        );
        return;
    }

    let (stdout, stderr, code) = c.land_raw(&step);
    set_mode_recursively(&c.remote(), 0o755, 0o644);

    assert_ne!(code, 0, "landing without write access succeeded:\n{stdout}");
    assert!(
        stderr.contains("could not write refs/heads/main"),
        "the lander names what it could not do:\n{stderr}"
    );
    assert!(
        stderr.contains("remote rejected")
            || stderr.contains("Permission denied")
            || stderr.contains("unable to create"),
        "the refusal is the remote's, in the remote's words:\n{stderr}"
    );
    for check in ["not a permission either", "refs moved", "is not up to date"] {
        assert!(
            !stderr.contains(check),
            "the refusal came from a check in this binary, not from the permission:\n{stderr}"
        );
    }
    assert_eq!(c.main_tip(), before, "main did not move");

    // The same record, once the process may write: it lands. Nothing about the record
    // changed between the two attempts, so the credential was the whole difference.
    let landed = c.land(&step);
    assert_eq!(s(&landed["landed"]), s(&step["sha"]));
    assert_eq!(c.main_tip(), s(&step["sha"]));
}

/// The step has no flag that names a remote or a ref: it could not land even if it wanted to,
/// and a manifest field cannot give it one because there is no field to set.
#[test]
fn the_step_command_cannot_be_told_where_to_land() {
    let c = Cluster::new();
    let (help, _, code) = c.yidam(&["cluster", "step", "--help"]);
    assert_eq!(code, 0);
    // The flags the help declares, not the prose — which says, in words, that there is no
    // `--remote`, and would match itself.
    let declared: Vec<&str> = help
        .lines()
        .filter_map(|l| l.trim_start().strip_prefix("--"))
        .filter_map(|l| l.split_whitespace().next())
        .collect();
    for flag in ["remote", "branch", "push", "ref", "target"] {
        assert!(
            !declared.contains(&flag),
            "`cluster step` accepts `--{flag}`, so a step pod can name a ref:\n{help}"
        );
    }
    let (help, _, _) = c.yidam(&["cluster", "land", "--help"]);
    assert!(help.contains("--remote") && help.contains("--branch"));
}

/// The class in a step's record is a claim, and the lander reads the class off the commit.
/// A record whose class does not match its commit's lands nothing, in either direction: the
/// mismatch is what matters and not which way it runs. The commit here is operational and the
/// record claims otherwise.
#[test]
fn a_forged_class_in_the_record_is_refused_and_nothing_lands() {
    let c = Cluster::new();
    let before = c.main_tip();
    let pin = c.pin();
    let mut step = c.step("travel-tier", s(&pin["bundle"]));
    assert_eq!(step["class"], "operational");
    step["class"] = Value::String("epistemic".into());

    let (stdout, stderr, code) = c.land_raw(&step);
    assert_ne!(code, 0, "a forged class landed:\n{stdout}");
    assert!(
        stderr.contains("not a permission either"),
        "the refusal says what a claim in a record is:\n{stderr}"
    );
    assert_eq!(c.main_tip(), before);
    assert!(
        c.remote_ref("refs/heads/propose/*").is_none()
            && out(&c.remote(), &["for-each-ref", "refs/heads/propose/"]).is_empty(),
        "and it did not become a proposal either"
    );
}

// ── the compare-and-swap ──────────────────────────────────────────────────────

/// `main` moved under the step between the pin and the landing. If the move touched nothing
/// the step reads, its result holds at the new tip and is re-parented there; if it touched an
/// input, the result is refused, because a receipt over stale inputs is not a receipt.
#[test]
fn a_landing_re_parents_over_an_unrelated_commit_and_refuses_over_a_moved_input() {
    let c = Cluster::new();
    let pin = c.pin();
    let step = c.step("travel-tier", s(&pin["bundle"]));
    assert_eq!(step["outcome"], "ran", "{step}");

    let unrelated = c.push_to_main("annotate: a note that no step reads", |root| {
        let readme = root.join("README.md");
        let mut text = std::fs::read_to_string(&readme).unwrap();
        text.push_str("\nA sentence appended while a step was running.\n");
        std::fs::write(readme, text).unwrap();
    });

    let landed = c.land(&step);
    assert_eq!(landed["reparented"], true, "{landed}");
    assert_eq!(landed["target"], "main");
    let sha = s(&landed["landed"]);
    assert_ne!(
        sha,
        s(&step["sha"]),
        "the landed commit is a rebuild, not the pod's object"
    );
    assert_eq!(
        out(&c.remote(), &["rev-parse", &format!("{sha}^")]),
        unrelated,
        "…whose parent is the commit that moved main"
    );
    assert_eq!(c.main_tip(), sha);
    assert!(
        out(&c.remote(), &["log", "-1", "--format=%s", sha]).starts_with("compute:"),
        "with the step's own subject"
    );
    assert!(
        !succeeded(&c.remote(), &["cat-file", "-e", s(&step["sha"])]),
        "the pod's own object never reached the remote"
    );
    assert!(
        succeeded(
            &c.remote(),
            &[
                "cat-file",
                "-e",
                &format!("{sha}:.yidam/runs/travel-tier.yml")
            ]
        ),
        "and the receipt"
    );

    // Now the second step, whose declared input is what the first one wrote. Move that
    // input on main while the step is in flight.
    let pin = c.pin();
    let step2 = c.step("disclosure-envelope", s(&pin["bundle"]));
    assert_eq!(step2["outcome"], "ran", "{step2}");
    let moved = c.push_to_main(
        "compute: an edit to what disclosure-envelope reads",
        |root| {
            let input = root.join(".yidam/computed/travel-tier.yml");
            let mut text = std::fs::read_to_string(&input).unwrap();
            text.push_str("# edited under the step\n");
            std::fs::write(input, text).unwrap();
        },
    );

    let (stdout, stderr, code) = c.land_raw(&step2);
    assert_ne!(code, 0, "a result over a moved input landed:\n{stdout}");
    assert!(
        stderr.contains("touched what it reads"),
        "the refusal names the reason:\n{stderr}"
    );
    assert_eq!(c.main_tip(), moved, "main is where the edit left it");
}

// ── the route ─────────────────────────────────────────────────────────────────

/// RFC-0026 §2 on a cluster: an epistemic step's result reaches `propose/<input>` and `main`
/// is byte-for-byte where it was. Then the two consequences the cap and the lander carry: the
/// same result landed twice is landed once, and a proposal nobody has merged counts against
/// `[cluster] max_open_proposals`.
#[test]
fn an_epistemic_step_lands_on_a_proposal_branch_and_main_does_not_move() {
    let c = Cluster::new();
    c.push_to_main(
        "scaffold: disclosure-envelope establishes rather than computes",
        |root| {
            let manifest = root.join(".yidam/capabilities.toml");
            let text = std::fs::read_to_string(&manifest).unwrap();
            let at = text.find("[capability.disclosure-envelope]").unwrap();
            let (head, tail) = text.split_at(at);
            let tail = tail.replacen("verb   = \"compute\"", "verb   = \"establish\"", 1);
            assert_ne!(tail, text[at..], "nothing was re-declared");
            std::fs::write(manifest, format!("{head}{tail}")).unwrap();
        },
    );

    // The operational upstream first, as the workflow orders it.
    let pin = c.pin();
    let up = c.step("travel-tier", s(&pin["bundle"]));
    assert_eq!(up["outcome"], "ran", "{up}");
    let landed = c.land(&up);
    let main_before = c.main_tip();
    assert_eq!(main_before, s(&up["sha"]));

    let step = c.step("disclosure-envelope", s(&landed["next"]["bundle"]));
    assert_eq!(step["outcome"], "ran", "{step}");
    assert_eq!(step["class"], "epistemic");
    assert_eq!(step["verb"], "establish");
    let short = &main_before[..out(&c.remote(), &["rev-parse", "--short", &main_before]).len()];
    let proposal = format!("propose/{short}");

    let landed = c.land(&step);
    assert_eq!(landed["class"], "epistemic");
    assert_eq!(s(&landed["target"]), proposal, "{landed}");
    assert_eq!(c.main_tip(), main_before, "main did not move");
    assert_eq!(
        c.remote_ref(&format!("refs/heads/{proposal}")).as_deref(),
        Some(s(&landed["landed"])),
        "the proposal branch holds the commit"
    );
    assert!(out(
        &c.remote(),
        &["log", "-1", "--format=%s", s(&landed["landed"])]
    )
    .starts_with("establish:"));
    assert_eq!(
        landed["next"]["sha"],
        main_before.as_str(),
        "the next pin is main, not the proposal"
    );

    // Landed again — the workflow retried, or two pods proposed the same thing — the
    // proposal already holds the result and nothing is stacked on it.
    let again = c.land(&step);
    assert!(again["landed"].is_null(), "{again}");
    assert_eq!(s(&again["target"]), proposal);
    assert_eq!(
        c.remote_ref(&format!("refs/heads/{proposal}")).as_deref(),
        Some(s(&landed["landed"])),
        "the branch did not move"
    );

    // Admission reads the proposal as the step's result: nothing is owed while it is open.
    let admission = c.admit();
    assert_eq!(admission["admitted"], false, "{admission}");
    assert_eq!(admission["open_proposals"], 1);
    assert_eq!(admission["stale"], serde_json::json!([]));
    assert!(
        admission["max_open_proposals"].is_null(),
        "no cap declared: reported as absent"
    );

    // Declare a cap, which is itself a change to the corpus that makes every step owed
    // again — and the one open proposal is what stops the run.
    c.push_to_main("scaffold: one open proposal at a time", |root| {
        std::fs::write(
            root.join(".yidam/config.toml"),
            "[cluster]\nmax_open_proposals = 1\n",
        )
        .unwrap();
    });
    let admission = c.admit();
    assert_eq!(admission["admitted"], false, "{admission}");
    assert_eq!(admission["max_open_proposals"], 1);
    assert!(
        s(&admission["because"]).contains("max_open_proposals is 1"),
        "{admission}"
    );
    assert!(
        !admission["stale"].as_array().unwrap().is_empty(),
        "something was owed; the cap is what refused it: {admission}"
    );

    // A person rejects the proposal, and the run is admitted.
    git(
        &c.remote(),
        &["update-ref", "-d", &format!("refs/heads/{proposal}")],
    );
    let admission = c.admit();
    assert_eq!(admission["admitted"], true, "{admission}");
    assert_eq!(admission["open_proposals"], 0);
}

// ── the worked example ────────────────────────────────────────────────────────

/// The manifest under `docs/cluster/` is generated from the streamflow example and checked
/// in. It is regenerated with `UPDATE_GOLDENS=1`, and this fails when the generator and the
/// document disagree, so the docs never show a workflow the binary would not write.
#[test]
fn the_documented_workflow_is_what_the_generator_writes() {
    let c = Cluster::new();
    let docs = common::repo_root().join("docs/cluster");
    for (file, extra) in [
        ("streamflow.workflow.yml", vec![]),
        ("streamflow.cronworkflow.yml", vec!["--cron", "0 6 * * *"]),
    ] {
        let mut args = vec![
            "cluster",
            "workflow",
            "--remote",
            "git@github.com:goedelsoup/streamflow.git",
            "--image",
            "ghcr.io/goedelsoup/yidam-cluster:latest",
            "--vault-url",
            "file:///var/yidam/vault",
        ];
        args.extend(extra);
        let o = Command::new(env!("CARGO_BIN_EXE_yidam"))
            .current_dir(c.e.path())
            .args(&args)
            .output()
            .unwrap();
        assert!(
            o.status.success(),
            "cluster workflow failed:\n{}",
            String::from_utf8_lossy(&o.stderr)
        );
        let actual = String::from_utf8_lossy(&o.stdout).to_string();
        let path = docs.join(file);
        if std::env::var("UPDATE_GOLDENS").is_ok() {
            std::fs::create_dir_all(&docs).unwrap();
            std::fs::write(&path, &actual).unwrap();
            continue;
        }
        let expected = std::fs::read_to_string(&path).unwrap_or_else(|_| {
            panic!(
                "missing {}. Run with UPDATE_GOLDENS=1 to write it.",
                path.display()
            )
        });
        assert_eq!(
            expected, actual,
            "\n{file} drifted from what `yidam cluster workflow` writes. If the change is \
             intended, re-run with UPDATE_GOLDENS=1 and review the diff.\n"
        );
        serde_yaml::from_str::<Value>(&actual)
            .unwrap_or_else(|e| panic!("{file} is not YAML: {e}"));
    }
}
