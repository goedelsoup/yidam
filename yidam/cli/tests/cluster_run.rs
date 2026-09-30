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
    /// Streamflow's shell pipeline: `travel-tier`, then `disclosure-envelope` after it.
    ///
    /// The typed calculator is dropped in every build. It declares no `after` and nothing
    /// reads it, so it is not part of the chain these tests walk, and a light build refuses
    /// a plan holding it. Whether a pod runs it is its own pair of tests, one per build,
    /// under "the typed arm" below.
    fn new() -> Self {
        let c = Self::as_declared();
        c.drop_typed_steps();
        c
    }

    /// Streamflow as the example declares it, typed step included. For what does not run a
    /// step, such as generating the workflow, which is the same in either build.
    fn as_declared() -> Self {
        let e = Example::materialize("streamflow");
        let work = tempfile::tempdir().unwrap();
        let remote = work.path().join("remote.git");
        git(work.path(), &["init", "-q", "--bare", "remote.git"]);
        std::fs::create_dir_all(work.path().join("vault")).unwrap();
        // The global config the binary sees: one that forbids git's fallback of inventing an
        // identity from the account name and hostname. A CI runner has no name to invent
        // one from, and this is what its failure looks like on a machine that does.
        std::fs::write(
            work.path().join("gitconfig"),
            "[user]\n\tuseConfigOnly = true\n",
        )
        .unwrap();
        git(
            &e.path(),
            &["remote", "add", "origin", remote.to_str().unwrap()],
        );
        git(&e.path(), &["push", "-q", "origin", "HEAD:refs/heads/main"]);
        Self { e, work }
    }

    /// Remove every capability whose `run` is a table (the typed arm), commit, and push.
    ///
    /// Cut out of the text by section rather than re-serialized, so every other line of the
    /// manifest stays byte-for-byte what the tests below edit by string replacement.
    fn drop_typed_steps(&self) {
        let path = self.e.path().join(".yidam/capabilities.toml");
        let text = std::fs::read_to_string(&path).unwrap();
        let manifest: toml::Table = toml::from_str(&text).unwrap();
        let typed: Vec<String> = manifest["capability"]
            .as_table()
            .unwrap()
            .iter()
            .filter(|(_, c)| matches!(c.get("run"), Some(toml::Value::Table(_))))
            .map(|(name, _)| format!("[capability.{name}]"))
            .collect();
        assert!(
            !typed.is_empty(),
            "streamflow declares no typed step, so this narrowing is dead code"
        );
        let mut skipping = false;
        let kept: String = text
            .lines()
            .filter(|line| {
                if line.starts_with('[') {
                    skipping = typed.iter().any(|header| line.trim() == header);
                }
                !skipping
            })
            .map(|line| format!("{line}\n"))
            .collect();
        let narrowed: toml::Table = toml::from_str(&kept).unwrap();
        assert_eq!(
            narrowed["capability"].as_table().unwrap().len(),
            manifest["capability"].as_table().unwrap().len() - typed.len(),
            "the cut removed exactly the typed sections"
        );
        std::fs::write(&path, kept).unwrap();
        git(
            &self.e.path(),
            &[
                "commit",
                "-qam",
                "chore: drop the typed step this build cannot run",
            ],
        );
        git(
            &self.e.path(),
            &["push", "-q", "origin", "HEAD:refs/heads/main"],
        );
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
            // A pod has no git identity but the one the binary gives its scratch clone. A
            // developer's global config must not supply the committer CI does not have.
            .env("GIT_CONFIG_GLOBAL", self.work.path().join("gitconfig"))
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env_remove("GIT_AUTHOR_NAME")
            .env_remove("GIT_AUTHOR_EMAIL")
            .env_remove("GIT_COMMITTER_NAME")
            .env_remove("GIT_COMMITTER_EMAIL")
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
        self.step_from(name, bundle, None)
    }

    /// A step started from `image`, which the workflow hands every step as `--image`.
    fn step_from(&self, name: &str, bundle: &str, image: Option<&str>) -> Value {
        let mut args = vec![
            "cluster".to_string(),
            "step".to_string(),
            name.to_string(),
            "--bundle".to_string(),
            bundle.to_string(),
        ];
        if let Some(image) = image {
            args.extend(["--image".to_string(), image.to_string()]);
        }
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

    /// Give the catalog one entry a pod can fetch with no network, and push it to `main`.
    ///
    /// A `kind: file` location over a file the corpus holds. Streamflow's own entry keeps its
    /// `url_template`, which needs a `--bind` a workflow does not pass and is skipped, and
    /// loses its bare `url`, which a pod built with `catalog-fetch` would really request.
    fn declare_a_file_location(&self) -> String {
        self.push_to_main("catalog: a gauge table a pod can read", |root| {
            let nwis = root.join(".yidam/catalog/usgs-nwis.md");
            let text = std::fs::read_to_string(&nwis).unwrap();
            let url = "  - kind: url\n    value: https://waterdata.usgs.gov/nwis\n    \
                       description: Human-facing query interface.\n";
            assert!(
                text.contains(url),
                "streamflow's entry changed shape:\n{text}"
            );
            std::fs::write(&nwis, text.replace(url, "")).unwrap();
            std::fs::create_dir_all(root.join("sources")).unwrap();
            std::fs::write(root.join(GAUGES), "site,cfs\ncanyon-outlet,412\n").unwrap();
            std::fs::write(
                root.join(".yidam/catalog/gauge-table.md"),
                format!(
                    "---\nname: gauge-table\ndescription: A table of gauges.\ntype: dataset\n\
                     obtained: true\nlocation:\n  - kind: file\n    value: {GAUGES}\n---\n\n\
                     # Gauge table\n"
                ),
            )
            .unwrap();
        })
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

/// The file `declare_a_file_location` points a catalog entry at.
const GAUGES: &str = "sources/gauges.csv";

/// An image pinned by digest, as a release workflow names it.
const PINNED_HEX: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

fn pinned_image() -> String {
    format!("ghcr.io/example/yidam@sha256:{PINNED_HEX}")
}

/// The receipt committed on `main` for `step`.
fn committed_receipt(c: &Cluster, step: &str) -> serde_yaml::Value {
    serde_yaml::from_str(&out(
        &c.remote(),
        &["show", &format!("refs/heads/main:.yidam/runs/{step}.yml")],
    ))
    .unwrap()
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

    // ── step one, from an image named by a tag ──
    let step = c.step_from("travel-tier", &bundle, Some("ghcr.io/example/yidam:latest"));
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
    assert_eq!(
        out(
            &c.remote(),
            &["log", "-1", "--format=%an <%ae> / %cn <%ce>", &built]
        ),
        "yidam run <run@yidam> / yidam cluster <cluster@yidam>",
        "the run is the author and the pod is the committer; no person's identity was read"
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
    // A tag is not a digest and a shell step's program is not this binary: the v2 receipt
    // records neither, rather than a claim the next push to `latest` would falsify.
    let receipt = committed_receipt(&c, "travel-tier");
    assert_eq!(receipt["format_version"].as_u64(), Some(2));
    for absent in ["image_digest", "version", "model", "config"] {
        assert!(
            receipt.get(absent).is_none(),
            "a shell step from a tagged image records no `{absent}`: {receipt:?}"
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
    let pin = c.pin();
    let step = c.step("travel-tier", s(&pin["bundle"]));
    assert_eq!(step["outcome"], "ran", "{step}");
    refused_without_the_write_credential(&c, &step);
}

/// The same boundary for a built-in step: its commit is built by compiled-in code rather than
/// a declared argv, and the only thing between it and `main` is still the credential.
#[test]
fn a_builtin_steps_sha_without_the_write_credential_does_not_land() {
    let c = Cluster::new();
    c.declare_a_file_location();
    let pin = c.pin();
    let step = c.step_from("catalog-fetch", s(&pin["bundle"]), Some(&pinned_image()));
    assert_eq!(step["outcome"], "ran", "{step}");
    assert_eq!(step["class"], "operational");
    refused_without_the_write_credential(&c, &step);
}

/// Land `step` against a remote this process cannot write, and hold the refusal to being the
/// remote's; then land it with the permission back, which shows nothing else refused it.
fn refused_without_the_write_credential(c: &Cluster, step: &Value) {
    let before = c.main_tip();
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

    let (stdout, stderr, code) = c.land_raw(step);
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
    let landed = c.land(step);
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

// ── the typed arm ─────────────────────────────────────────────────────────────

/// The image carries `calculators-gluon` (`docs/cluster/Dockerfile`), so a pod runs a typed
/// calculator the way it runs a shell one: pinned bundle in, operational sha out, landed on
/// `main` with its receipt.
#[cfg(feature = "calculators-gluon")]
#[test]
fn a_typed_step_runs_in_a_pod_and_lands_with_its_receipt() {
    let c = Cluster::as_declared();
    let admission = c.admit();
    assert_eq!(admission["admitted"], true, "{admission}");
    assert!(
        admission["stale"]
            .as_array()
            .unwrap()
            .iter()
            .any(|s| s == "travel-tier-typed"),
        "{admission}"
    );

    let pin = c.pin();
    let step = c.step_from(
        "travel-tier-typed",
        s(&pin["bundle"]),
        Some(&pinned_image()),
    );
    assert_eq!(step["outcome"], "ran", "{step}");
    assert_eq!(step["class"], "operational");
    let landed = c.land(&step);
    assert_eq!(landed["target"], "main");
    assert_eq!(c.main_tip(), s(&step["sha"]));
    assert!(succeeded(
        &c.remote(),
        &[
            "cat-file",
            "-e",
            "refs/heads/main:.yidam/runs/travel-tier-typed.yml"
        ]
    ));
    // This binary interpreted the script, so its version is what produced the output.
    let receipt = committed_receipt(&c, "travel-tier-typed");
    assert_eq!(receipt["version"].as_str(), Some(env!("CARGO_PKG_VERSION")));
    assert_eq!(
        receipt["image_digest"].as_str(),
        Some(format!("sha256:{PINNED_HEX}").as_str())
    );
}

/// A build without the feature refuses the plan by name at admission, before any pod is
/// scheduled — the sentence `run` gives, not a pod failing halfway through a workflow.
#[cfg(not(feature = "calculators-gluon"))]
#[test]
fn a_light_build_refuses_a_typed_step_at_admission() {
    let c = Cluster::as_declared();
    let mut args = vec!["cluster".to_string(), "admit".to_string()];
    args.extend(c.remote_args());
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let (stdout, stderr, code) = c.yidam(&args);
    assert_ne!(code, 0, "admitted a plan this build cannot run:\n{stdout}");
    assert!(
        stderr.contains("travel-tier-typed") && stderr.contains("calculators-gluon"),
        "{stderr}"
    );
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

// ── the built-in steps ────────────────────────────────────────────────────────

/// #475's definition of done: `catalog-fetch`, run the way a pod runs it — a pinned bundle, no
/// checkout, no credential that names a ref — builds a `refresh:` commit the lander lands, with
/// the receipt in its tree.
#[test]
fn a_catalog_fetch_step_run_as_a_pod_lands_a_refresh_commit_with_its_receipt() {
    let c = Cluster::new();
    let before = c.declare_a_file_location();
    let pin = c.pin();
    assert_eq!(s(&pin["sha"]), before);

    let step = c.step_from("catalog-fetch", s(&pin["bundle"]), Some(&pinned_image()));
    assert_eq!(step["outcome"], "ran", "{step}");
    assert_eq!(step["class"], "operational");
    assert_eq!(step["verb"], "refresh");
    assert_eq!(step["receipt"], ".yidam/runs/catalog-fetch.yml");
    assert_eq!(step["input"], before.as_str());
    let sha = s(&step["sha"]).to_string();
    assert_eq!(c.main_tip(), before, "the step moved nothing");

    let landed = c.land(&step);
    assert_eq!(s(&landed["landed"]), sha);
    assert_eq!(landed["target"], "main");
    assert_eq!(c.main_tip(), sha);

    let subject = &c.subjects(1)[0];
    assert!(subject.starts_with("refresh: "), "{subject}");
    assert_eq!(
        yidam_core::git::classify_commit(&sha, subject).kind,
        yidam_core::git::CommitKind::Operational,
        "the lander's own classifier calls it operational: {subject}"
    );
    assert_eq!(
        out(
            &c.remote(),
            &[
                "log",
                "-1",
                "--format=%an <%ae> / %cn <%ce>",
                "refs/heads/main"
            ]
        ),
        "yidam catalog <catalog@yidam> / yidam cluster <cluster@yidam>",
        "the tool authors, the pod commits"
    );

    let receipt = committed_receipt(&c, "catalog-fetch");
    assert_eq!(receipt["format_version"].as_u64(), Some(2));
    assert_eq!(receipt["step"].as_str(), Some("catalog-fetch"));
    // What produced it: this binary, in the image the pod was started from, by digest.
    assert_eq!(receipt["version"].as_str(), Some(env!("CARGO_PKG_VERSION")));
    assert_eq!(
        receipt["image_digest"].as_str(),
        Some(format!("sha256:{PINNED_HEX}").as_str())
    );
    assert!(receipt.get("model").is_none() && receipt.get("config").is_none());
    assert_eq!(receipt["kind"].as_str(), Some("connector"));
    assert_eq!(receipt["verb"].as_str(), Some("refresh"));
    assert_eq!(receipt["input"]["commit"].as_str(), Some(before.as_str()));
    let outputs: Vec<&str> = receipt["outputs"]
        .as_sequence()
        .unwrap()
        .iter()
        .filter_map(|o| o["path"].as_str())
        .collect();
    assert!(
        outputs.contains(&".yidam/catalog/gauge-table.md"),
        "the receipt names what the fetch wrote: {outputs:?}"
    );
    let entry = out(
        &c.remote(),
        &["show", "refs/heads/main:.yidam/catalog/gauge-table.md"],
    );
    assert!(
        entry.contains("artifacts:"),
        "the fetch was recorded:\n{entry}"
    );
}

/// Every built-in the binary compiles in runs in a pod against streamflow, and whatever it
/// builds lands. Derived from the set, so a built-in added there is run here.
#[test]
fn every_builtin_runs_in_a_pod_and_what_it_builds_lands() {
    let c = Cluster::new();
    c.declare_a_file_location();
    let mut bundle = s(&c.pin()["bundle"]).to_string();
    for b in yidam::CLUSTER_BUILTINS {
        let step = c.step(b.name, &bundle);
        assert_eq!(step["class"], "operational", "{}: {step}", b.name);
        assert_eq!(step["verb"], b.verb, "{}", b.name);
        let landed = c.land(&step);
        if step["outcome"] == "ran" {
            assert_eq!(landed["landed"], step["sha"], "{}", b.name);
            assert!(
                c.subjects(1)[0].starts_with(&format!("{}: ", b.verb)),
                "{}: {:?}",
                b.name,
                c.subjects(1)
            );
        } else {
            assert_eq!(step["outcome"], "unchanged", "{}: {step}", b.name);
        }
        bundle = s(&landed["next"]["bundle"]).to_string();
    }
}

/// The generated workflow runs every built-in, in the set's order, before the manifest's first
/// step — read off the DAG, not off a list of names written here.
#[test]
fn the_workflow_runs_every_builtin_ahead_of_the_manifest() {
    let c = Cluster::as_declared();
    let o = Command::new(env!("CARGO_BIN_EXE_yidam"))
        .current_dir(c.e.path())
        .args([
            "cluster",
            "workflow",
            "--remote",
            "r",
            "--image",
            "i",
            "--vault-url",
            "file:///v",
        ])
        .output()
        .unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let doc: Value = serde_yaml::from_slice(&o.stdout).unwrap();
    let steps: Vec<String> = doc["spec"]["templates"][0]["dag"]["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|t| t["template"] == "step")
        .map(|t| s(&t["arguments"]["parameters"][0]["value"]).to_string())
        .collect();
    let builtins: Vec<&str> = yidam::CLUSTER_BUILTINS.iter().map(|b| b.name).collect();
    assert!(!builtins.is_empty());
    assert_eq!(&steps[..builtins.len()], builtins.as_slice(), "{steps:?}");
    assert!(
        steps.len() > builtins.len() && steps.contains(&"travel-tier".to_string()),
        "the manifest's steps follow: {steps:?}"
    );
}

/// A manifest declaring a built-in's name is refused where the workflow is written, rather than
/// having one of the two quietly win.
#[test]
fn a_manifest_that_shadows_a_builtin_has_no_workflow() {
    let c = Cluster::as_declared();
    let name = yidam::CLUSTER_BUILTINS[0].name;
    let path = c.e.path().join(".yidam/capabilities.toml");
    let text = std::fs::read_to_string(&path).unwrap();
    std::fs::write(
        &path,
        text.replace("[capability.travel-tier]", &format!("[capability.{name}]"))
            .replace("\"travel-tier\"", &format!("\"{name}\"")),
    )
    .unwrap();
    let o = Command::new(env!("CARGO_BIN_EXE_yidam"))
        .current_dir(c.e.path())
        .args([
            "cluster",
            "workflow",
            "--remote",
            "r",
            "--image",
            "i",
            "--vault-url",
            "file:///v",
        ])
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&o.stderr);
    assert!(
        !o.status.success(),
        "a shadowing manifest generated a workflow"
    );
    assert!(
        stderr.contains(name) && stderr.contains("Rename"),
        "{stderr}"
    );
}

// ── the worked example ────────────────────────────────────────────────────────

/// The manifest under `docs/cluster/` is generated from the streamflow example and checked
/// in. It is regenerated with `UPDATE_GOLDENS=1`, and this fails when the generator and the
/// document disagree, so the docs never show a workflow the binary would not write.
#[test]
fn the_documented_workflow_is_what_the_generator_writes() {
    let c = Cluster::as_declared();
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
