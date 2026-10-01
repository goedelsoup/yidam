//! The cluster contract, exercised against a bare remote and a `file://` vault — and the
//! boundary it exists for, tested by trying to break it (#475).
//!
//! Every pod of a generated workflow is one `yidam cluster <sub>` invocation, so the whole
//! run can be driven here without Argo: pin, step, land, in the order the DAG imposes, with
//! the records passed between them as the workflow passes them. What Argo adds — scheduling,
//! retries, which secret is mounted where — is asserted over the generated manifest in
//! `src/cmd/cluster/workflow.rs`'s own tests and pinned as the worked example in
//! `yidam/cluster/overlays/streamflow/`, which is built and compared with the objects it names.
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
use serde::Deserialize as _;
use serde_json::Value;

/// A corpus, the bare remote it pushes to, a vault directory, and a workspace for records.
struct Cluster {
    e: Example,
    work: tempfile::TempDir,
    /// Set by [`Cluster::with_github_app`]: the lander pushes as a GitHub App (#1233).
    app: Option<MockGithub>,
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
        Self { e, work, app: None }
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
            // The App-mode lander talks to a mock API on 127.0.0.1, which a developer's
            // proxy must not be asked to reach.
            .env_remove("HTTPS_PROXY")
            .env_remove("https_proxy")
            .env_remove("HTTP_PROXY")
            .env_remove("http_proxy")
            .env_remove("ALL_PROXY")
            .env_remove("all_proxy")
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
        match &self.app {
            None => args.extend(self.remote_args()),
            Some(app) => args.extend(app.land_args()),
        }
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

/// Run every step admission calls stale, in its order, as the workflow would: a pin, then a
/// step and a land each, each step on the pin the last landing left. Returns the landings.
fn run_admitted(c: &Cluster, stale: &[Value]) -> Vec<Value> {
    let mut bundle = s(&c.pin()["bundle"]).to_string();
    let mut landings = vec![];
    for step in stale {
        let record = c.step(s(step), &bundle);
        let landed = c.land(&record);
        bundle = s(&landed["next"]["bundle"]).to_string();
        landings.push(landed);
    }
    landings
}

/// #1235. `--on-push` submits a run on every push to the branch, and the lander's push is a
/// push. What keeps that from looping is admission's answer rather than a filter on the
/// committer, so the answer is asserted here rather than assumed.
///
/// The sequence is a person's push, the run it is admitted for, and then the run each landing
/// push submits. Those queue on the corpus's mutex behind the run that landed them, so each
/// asks `admit` after that run has finished. The mid-chain question is asked too, and is
/// answered *admitted*. That is the race the mutex exists for, and if it ever answered
/// otherwise the mutex would be decoration.
#[test]
fn a_push_is_admitted_once_and_the_landers_own_push_is_not() {
    let c = Cluster::new();
    let quiet = c.admit();
    let landings = run_admitted(&c, quiet["stale"].as_array().unwrap());
    assert!(
        !landings.is_empty(),
        "the corpus started with nothing to run"
    );
    assert_eq!(c.admit()["admitted"], false, "the corpus did not settle");

    // A person downgrades a claim. `travel-tier` reads it, and its output changes, so
    // `disclosure-envelope`, which reads that output, has something to do as well.
    c.push_to_main("revise: canyon-outlet's claim is open again", |root| {
        let node = root.join(".yidam/corpus/gage/canyon-outlet.yml");
        let text = std::fs::read_to_string(&node).unwrap();
        let text = text.replacen("claim_tag: inference", "claim_tag: open", 1);
        assert!(
            text.contains("claim_tag: open"),
            "canyon-outlet moved its claim tag"
        );
        std::fs::write(node, text).unwrap();
    });
    let admission = c.admit();
    assert_eq!(admission["admitted"], true, "{admission}");
    let stale = admission["stale"].as_array().unwrap().clone();
    assert_eq!(
        stale.first().map(s),
        Some("travel-tier"),
        "the edit made its reader stale: {admission}"
    );

    // The run, asking what a push-submitted run would ask after each landing push.
    let mut bundle = s(&c.pin()["bundle"]).to_string();
    let mut mid_chain = vec![];
    for (i, step) in stale.iter().enumerate() {
        let landed = c.land(&c.step(s(step), &bundle));
        assert_eq!(landed["target"], "main", "{landed}");
        bundle = s(&landed["next"]["bundle"]).to_string();
        if i + 1 < stale.len() {
            mid_chain.push(c.admit());
        }
    }
    assert!(
        !mid_chain.is_empty(),
        "one step was stale, so no run asked between two landings and the race below is unread"
    );
    for early in &mid_chain {
        assert_eq!(
            early["admitted"], true,
            "a run that asked between two landings would race the run still landing: \
             {early}. That is why every generated workflow holds the corpus's mutex."
        );
    }

    // What every run the lander's pushes submitted asks, once the run before it is done.
    assert_eq!(
        out(
            &c.remote(),
            &["log", "-1", "--format=%cn", "refs/heads/main"]
        ),
        "yidam cluster",
        "the last push to main was the lander's"
    );
    let after = c.admit();
    assert_eq!(
        after["admitted"], false,
        "the lander's own push was admitted, so a run on push would run again on its own \
         landing: {after}"
    );
    assert_eq!(after["stale"], serde_json::json!([]));
    assert!(s(&after["because"]).contains("nothing is owed"), "{after}");
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

// ── the boundary, under a GitHub App ──────────────────────────────────────────

/// The token the mock API hands out. Distinctive enough that finding it anywhere is a leak.
const APP_TOKEN: &str = "ghs_SENTINELx1233tokenNeverWrittenAnywhere0";

/// The remote a GitHub-hosted corpus would declare. The lander turns it into
/// `https://github.com/acme/corpus.git`, and the global config the binary reads rewrites that
/// to the bare remote, so the push it makes over "HTTPS" lands on a directory this test can
/// make read-only — the same credential-shaped refusal as the deploy-key run.
const APP_REMOTE: &str = "git@github.com:acme/corpus.git";

/// One request the mock GitHub API was sent.
#[derive(Debug, Clone)]
struct Seen {
    method: String,
    path: String,
    headers: Vec<(String, String)>,
    body: String,
}

impl Seen {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }
}

/// GitHub's two App endpoints, served from a thread on 127.0.0.1 — the one non-HTTPS API the
/// lander agrees to send a JWT to.
struct MockGithub {
    api: String,
    key: PathBuf,
    seen: std::sync::Arc<std::sync::Mutex<Vec<Seen>>>,
}

#[cfg_attr(not(feature = "github-app"), allow(dead_code))]
impl MockGithub {
    fn start() -> Self {
        use std::io::{BufRead, BufReader, Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let api = format!("http://{}", listener.local_addr().unwrap());
        let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let log = seen.clone();
        // Detached: it serves until the test process exits.
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { continue };
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut line = String::new();
                if reader.read_line(&mut line).is_err() {
                    continue;
                }
                let mut parts = line.split_whitespace();
                let method = parts.next().unwrap_or_default().to_string();
                let path = parts.next().unwrap_or_default().to_string();
                let mut headers = Vec::new();
                loop {
                    let mut h = String::new();
                    if reader.read_line(&mut h).unwrap_or(0) == 0 || h == "\r\n" {
                        break;
                    }
                    if let Some((k, v)) = h.trim_end().split_once(':') {
                        headers.push((k.trim().to_string(), v.trim().to_string()));
                    }
                }
                let len: usize = headers
                    .iter()
                    .find(|(k, _)| k.eq_ignore_ascii_case("content-length"))
                    .and_then(|(_, v)| v.parse().ok())
                    .unwrap_or(0);
                let mut body = vec![0; len];
                let _ = reader.read_exact(&mut body);
                let (status, reply) = match (method.as_str(), path.as_str()) {
                    ("GET", "/repos/acme/corpus/installation") => {
                        ("200 OK", r#"{"id":42}"#.to_string())
                    }
                    ("POST", "/app/installations/42/access_tokens") => (
                        "201 Created",
                        format!(r#"{{"token":"{APP_TOKEN}","expires_at":"2099-01-01T00:00:00Z"}}"#),
                    ),
                    _ => ("404 Not Found", r#"{"message":"Not Found"}"#.to_string()),
                };
                log.lock().unwrap().push(Seen {
                    method,
                    path,
                    headers,
                    body: String::from_utf8_lossy(&body).to_string(),
                });
                let _ = write!(
                    stream,
                    "HTTP/1.1 {status}\r\nContent-Type: application/json\r\n\
                     Content-Length: {}\r\nConnection: close\r\n\r\n{reply}",
                    reply.len()
                );
            }
        });
        Self {
            api,
            key: Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/github-app/private-key.pem"),
            seen,
        }
    }

    fn land_args(&self) -> Vec<String> {
        [
            "--remote",
            APP_REMOTE,
            "--git-auth",
            "github-app",
            "--github-app-id",
            "1233",
            "--github-app-key",
            self.key.to_str().unwrap(),
            "--github-api",
            &self.api,
        ]
        .map(String::from)
        .to_vec()
    }

    fn seen(&self) -> Vec<Seen> {
        self.seen.lock().unwrap().clone()
    }
}

#[cfg_attr(not(feature = "github-app"), allow(dead_code))]
impl Cluster {
    /// The lander pushes as a GitHub App, to the bare remote under GitHub's HTTPS URL.
    fn with_github_app(mut self) -> Self {
        let config = self.work.path().join("gitconfig");
        use std::fmt::Write as _;
        let mut text = std::fs::read_to_string(&config).unwrap();
        let _ = write!(
            text,
            "[url \"{}\"]\n\tinsteadOf = https://github.com/acme/corpus.git\n",
            self.remote().display()
        );
        std::fs::write(&config, text).unwrap();
        self.app = Some(MockGithub::start());
        self
    }
}

/// Standard or URL-safe-unpadded base64, for what the lander sends and what must not leak.
#[cfg_attr(not(feature = "github-app"), allow(dead_code))]
fn base64(bytes: &[u8], url_safe: bool) -> String {
    let table: &[u8; 64] = if url_safe {
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_"
    } else {
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/"
    };
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let n = chunk
            .iter()
            .enumerate()
            .fold(0u32, |n, (i, b)| n | (u32::from(*b) << (16 - 8 * i)));
        for i in 0..=chunk.len() {
            out.push(table[(n >> (18 - 6 * i) & 63) as usize] as char);
        }
        if !url_safe {
            for _ in chunk.len()..3 {
                out.push('=');
            }
        }
    }
    out
}

/// [`a_valid_sha_without_the_write_credential_does_not_land`] with the lander pushing as a
/// GitHub App: a token in hand is not the remote's permission, and the refusal is still the
/// remote's.
#[cfg(feature = "github-app")]
#[test]
fn a_valid_sha_without_the_write_credential_does_not_land_under_a_github_app() {
    let c = Cluster::new().with_github_app();
    let pin = c.pin();
    let step = c.step("travel-tier", s(&pin["bundle"]));
    assert_eq!(step["outcome"], "ran", "{step}");
    refused_without_the_write_credential(&c, &step);
    assert!(
        !c.app.as_ref().unwrap().seen().is_empty(),
        "the lander landed without asking the App for a token"
    );
}

/// The definition of done for #1233: the lander mints a token for one repository with
/// `contents: write`, pushes with it, and writes it nowhere — not to stdout, stderr or its
/// record, not to the receipt or any object on the remote, and not to any file it leaves.
#[cfg(feature = "github-app")]
#[test]
fn a_minted_token_is_never_written_to_a_record_a_receipt_or_stdout() {
    let c = Cluster::new().with_github_app();
    let pin = c.pin();
    let step = c.step("travel-tier", s(&pin["bundle"]));
    assert_eq!(step["outcome"], "ran", "{step}");
    let (stdout, stderr, code) = c.land_raw(&step);
    assert_eq!(code, 0, "cluster land failed:\n{stdout}{stderr}");
    let record: Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(s(&record["record"]["landed"]), s(&step["sha"]));
    assert_eq!(c.main_tip(), s(&step["sha"]));

    // What GitHub was asked: the installation of this repository, then a token for it alone.
    let seen = c.app.as_ref().unwrap().seen();
    let paths: Vec<(&str, &str)> = seen
        .iter()
        .map(|r| (r.method.as_str(), r.path.as_str()))
        .collect();
    assert_eq!(
        paths,
        [
            ("GET", "/repos/acme/corpus/installation"),
            ("POST", "/app/installations/42/access_tokens")
        ]
    );
    for r in &seen {
        let jwt = r
            .header("authorization")
            .and_then(|a| a.strip_prefix("Bearer "))
            .unwrap_or_else(|| panic!("{} {} carried no JWT: {r:?}", r.method, r.path));
        let parts: Vec<&str> = jwt.split('.').collect();
        assert_eq!(parts.len(), 3, "{jwt}");
        assert_eq!(parts[0], base64(br#"{"alg":"RS256","typ":"JWT"}"#, true));
        assert_eq!(r.header("x-github-api-version"), Some("2022-11-28"));
    }
    let asked: Value = serde_json::from_str(&seen[1].body).unwrap();
    assert_eq!(
        asked,
        serde_json::json!({"repositories": ["corpus"], "permissions": {"contents": "write"}})
    );

    // Where it must not be: the token, and the header git was handed it in.
    let basic = base64(format!("x-access-token:{APP_TOKEN}").as_bytes(), false);
    let leaked = |what: &str, bytes: &[u8]| {
        let text = String::from_utf8_lossy(bytes);
        assert!(!text.contains(APP_TOKEN), "the token is in {what}");
        assert!(!text.contains(&basic), "the token's header is in {what}");
    };
    leaked("stdout", stdout.as_bytes());
    leaked("stderr", stderr.as_bytes());
    let receipt = committed_receipt(&c, "travel-tier");
    leaked(
        "the receipt",
        serde_yaml::to_string(&receipt).unwrap().as_bytes(),
    );
    let objects = common::git::raw(&c.remote(), &["cat-file", "--batch-all-objects", "--batch"]);
    assert!(objects.status.success());
    assert!(!objects.stdout.is_empty(), "the scan read no objects");
    leaked("an object on the remote", &objects.stdout);
    let mut files = 0;
    for entry in walkdir::WalkDir::new(c.work.path()) {
        let entry = entry.unwrap();
        if entry.file_type().is_file() {
            files += 1;
            let bytes = std::fs::read(entry.path()).unwrap();
            leaked(&entry.path().display().to_string(), &bytes);
        }
    }
    assert!(files > 0, "the scan read no files");
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

/// The image carries `calculators-gluon` (`yidam/cluster/Dockerfile`), so a pod runs a typed
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
        // By task, not template: a built-in that reads the world runs from its own (#1232).
        .filter(|t| s(&t["name"]).starts_with("step-"))
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

// ── one corpus, one credential ────────────────────────────────────────────────

/// One object a manifest refers to: the field that names it, the name, and whether the pod
/// starts without it (`optional: true` on a `secretRef`).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Named {
    field: String,
    name: String,
    optional: bool,
}

impl Named {
    /// The kind of object the field names.
    fn kind(&self) -> &'static str {
        match self.field.as_str() {
            "serviceAccountName" => "ServiceAccount",
            "claimName" => "PersistentVolumeClaim",
            _ => "Secret",
        }
    }
}

/// Every object a manifest refers to by name, by the field that names it.
///
/// Walked from the parsed document rather than searched for in its text, so a name in a
/// comment is not a name in the manifest, and `yidam-a-git-write` is not mistaken for a
/// match of `yidam-a-git`.
fn named_objects(doc: &serde_yaml::Value) -> Vec<Named> {
    let mut found = Vec::new();
    let mut stack = vec![doc];
    while let Some(v) = stack.pop() {
        match v {
            serde_yaml::Value::Mapping(m) => {
                for (k, v) in m {
                    let field = k.as_str().unwrap_or_default();
                    match (field, v) {
                        (
                            "serviceAccountName" | "secretName" | "claimName",
                            serde_yaml::Value::String(name),
                        ) => found.push(Named {
                            field: field.to_string(),
                            name: name.clone(),
                            optional: false,
                        }),
                        // `webhookSecret` and `authSecret` are Argo Events' (#1235).
                        ("secretRef" | "secretKeyRef" | "webhookSecret" | "authSecret", _) => {
                            if let Some(name) = v["name"].as_str() {
                                found.push(Named {
                                    field: field.to_string(),
                                    name: name.to_string(),
                                    optional: v["optional"].as_bool() == Some(true),
                                });
                            }
                        }
                        _ => {}
                    }
                    stack.push(v);
                }
            }
            serde_yaml::Value::Sequence(s) => stack.extend(s),
            _ => {}
        }
    }
    found
}

/// #1228: two corpora in one namespace share no credential, account, or vault.
///
/// These names were constants, so a namespace running two corpora held one `yidam-git-write`.
/// Either one corpus's lander could not push, or one key could push to both, and either lander
/// could land the other's refs. The invariant (#460 decision 7) is that exactly one component
/// holds the credential that moves a ref, and that holds only if the credential is one
/// corpus's alone.
#[test]
fn two_corpora_in_one_namespace_share_no_named_object() {
    let c = Cluster::as_declared();
    // The same corpus under a second name: everything the generator reads is identical but
    // the identity it derives names from.
    let other = c.work.path().join("rivergage");
    let copied = Command::new("cp")
        .args(["-R"])
        .arg(c.e.path())
        .arg(&other)
        .status()
        .unwrap();
    assert!(copied.success());

    // On push and on a cron, so the Sensor's account, the webhook secret, the objects' own
    // names and the mutex are compared too (#1235).
    let generate = |dir: &Path| -> serde_yaml::Value {
        let o = Command::new(env!("CARGO_BIN_EXE_yidam"))
            .current_dir(dir)
            .args([
                "cluster",
                "workflow",
                "--remote",
                "git@example.com:corpus.git",
                "--image",
                "ghcr.io/goedelsoup/yidam-cluster:test",
                "--vault-url",
                "file:///var/yidam/vault",
                "--cron",
                "0 6 * * *",
                "--on-push",
                "webhook",
            ])
            .output()
            .unwrap();
        assert!(
            o.status.success(),
            "cluster workflow failed in {}:\n{}",
            dir.display(),
            String::from_utf8_lossy(&o.stderr)
        );
        let text = String::from_utf8(o.stdout).unwrap();
        serde_yaml::Value::Sequence(
            serde_yaml::Deserializer::from_str(&text)
                .map(|d| serde_yaml::Value::deserialize(d).expect("the manifest parses"))
                .collect(),
        )
    };
    // What a manifest names, and what it is itself named and locks: an EventSource or a
    // mutex two corpora shared would be one corpus's push running the other's chain.
    let own = |docs: &serde_yaml::Value| -> Vec<Named> {
        let mut found = named_objects(docs);
        for d in docs.as_sequence().unwrap() {
            let name = d["metadata"]["name"].as_str().map(str::to_string);
            found.extend(name.map(|name| Named {
                field: format!("{}.metadata.name", d["kind"].as_str().unwrap()),
                name,
                optional: false,
            }));
        }
        for d in docs.as_sequence().unwrap() {
            let spec = if d["kind"] == "CronWorkflow" {
                &d["spec"]["workflowSpec"]
            } else {
                &d["spec"]["triggers"][0]["template"]["k8s"]["source"]["resource"]["spec"]
            };
            for m in spec["synchronization"]["mutexes"]
                .as_sequence()
                .into_iter()
                .flatten()
            {
                found.push(Named {
                    field: "mutex".to_string(),
                    name: m["name"].as_str().unwrap().to_string(),
                    optional: false,
                });
            }
        }
        found
    };
    let a = own(&generate(&c.e.path()));
    let b = own(&generate(&other));

    // Every kind of reference is present, so an empty intersection is not a blind walker's.
    for field in [
        "serviceAccountName",
        "secretName",
        "claimName",
        "secretRef",
        "authSecret",
        "EventSource.metadata.name",
        "Sensor.metadata.name",
        "mutex",
    ] {
        for (corpus, found) in [("streamflow", &a), ("rivergage", &b)] {
            assert!(
                found.iter().any(|n| n.field == field),
                "{corpus}'s manifest names no `{field}`; found {found:?}"
            );
        }
    }
    let names = |found: &[Named]| -> std::collections::BTreeSet<String> {
        found.iter().map(|n| n.name.clone()).collect()
    };
    let shared: Vec<String> = names(&a).intersection(&names(&b)).cloned().collect();
    assert!(
        shared.is_empty(),
        "two corpora's manifests name the same objects: {shared:?}. In one namespace they \
         would share a credential, and either lander could move the other's refs."
    );
    assert!(
        a.iter()
            .any(|n| n.field == "secretName" && n.name == "yidam-streamflow-git-write"),
        "the write secret is not named for its corpus: {a:?}"
    );
    assert!(
        a.iter()
            .any(|n| n.field == "serviceAccountName" && n.name == "yidam-streamflow-events"),
        "the Sensor's account is not named for its corpus: {a:?}"
    );
    assert_eq!(
        a.iter().filter(|n| n.field == "mutex").count(),
        2,
        "the cron's run and the push's run should each hold the mutex: {a:?}"
    );
}

/// `[cluster.names]` overrides a derived name, and a name Kubernetes would refuse is refused
/// here first.
#[test]
fn a_declared_name_overrides_the_derived_one() {
    let c = Cluster::as_declared();
    let config = c.e.path().join(".yidam/config.toml");
    let before = std::fs::read_to_string(&config).unwrap_or_default();
    let generate = |names: &str| {
        std::fs::write(&config, format!("{before}\n[cluster.names]\n{names}\n")).unwrap();
        Command::new(env!("CARGO_BIN_EXE_yidam"))
            .current_dir(c.e.path())
            .args([
                "cluster",
                "workflow",
                "--remote",
                "git@example.com:corpus.git",
                "--image",
                "img",
                "--vault-url",
                "file:///var/yidam/vault",
            ])
            .output()
            .unwrap()
    };

    let o = generate("git_write = \"streamflow-lander\"");
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let doc: serde_yaml::Value = serde_yaml::from_slice(&o.stdout).unwrap();
    let found = named_objects(&doc);
    let mounts = |name: &str| {
        found
            .iter()
            .any(|n| n.field == "secretName" && n.name == name)
    };
    assert!(mounts("streamflow-lander"), "{found:?}");
    assert!(
        mounts("yidam-streamflow-git-read"),
        "an override of one name moved another: {found:?}"
    );

    let o = generate("git_write = \"Streamflow_Lander\"");
    assert!(!o.status.success(), "an invalid object name was written");
    let err = String::from_utf8_lossy(&o.stderr);
    assert!(
        err.contains("git_write") && err.contains("Streamflow_Lander"),
        "{err}"
    );
}

/// `[capability.<name>.cluster]` reaches that step's pod and no other, over `[cluster.pod]`
/// key by key (#1231).
///
/// Two capabilities override two different keys, so a template shared between them, or bounds
/// applied by position, would put one's key on the other. `travel-tier-typed` declares none and
/// shares `travel-tier`'s prefix, so a match by prefix would hand it `travel-tier`'s bounds.
#[test]
fn a_capability_bounds_reach_only_its_own_step_pod() {
    let c = Cluster::as_declared();
    let root = c.e.path();
    let manifest = root.join(".yidam/capabilities.toml");
    let text = std::fs::read_to_string(&manifest).unwrap();
    std::fs::write(
        &manifest,
        format!(
            "{text}\n[capability.travel-tier.cluster]\nmemory_limit = \"8Gi\"\n\n\
             [capability.disclosure-envelope.cluster]\ndeadline_seconds = 600\n"
        ),
    )
    .unwrap();
    let config = root.join(".yidam/config.toml");
    let before = std::fs::read_to_string(&config).unwrap_or_default();
    std::fs::write(
        &config,
        format!("{before}\n[cluster.pod]\ncpu_request = \"1\"\n"),
    )
    .unwrap();

    let o = Command::new(env!("CARGO_BIN_EXE_yidam"))
        .current_dir(root)
        .args([
            "cluster",
            "workflow",
            "--remote",
            "git@example.com:corpus.git",
            "--image",
            "img",
            "--vault-url",
            "file:///var/yidam/vault",
        ])
        .output()
        .unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let doc: serde_yaml::Value = serde_yaml::from_slice(&o.stdout).unwrap();
    let templates = doc["spec"]["templates"].as_sequence().unwrap();
    let template = |name: &str| {
        templates
            .iter()
            .find(|t| t["name"] == name)
            .unwrap_or_else(|| panic!("no template {name}"))
    };
    let bounds = |name: &str| {
        let t = template(name);
        let r = &t["container"]["resources"];
        (
            r["requests"]["cpu"].as_str().unwrap().to_string(),
            r["limits"]["memory"].as_str().unwrap().to_string(),
            t["activeDeadlineSeconds"].as_u64().unwrap(),
        )
    };
    // Each override, and the corpus's CPU request kept beneath it.
    assert_eq!(bounds("step-travel-tier"), ("1".into(), "8Gi".into(), 3600));
    assert_eq!(
        bounds("step-disclosure-envelope"),
        ("1".into(), "2Gi".into(), 600)
    );
    // Every other pod has the corpus's bounds and neither capability's.
    for other in ["step", "step-catalog-fetch", "pin", "land"] {
        assert_eq!(bounds(other), ("1".into(), "2Gi".into(), 3600), "{other}");
    }

    let tasks = templates[0]["dag"]["tasks"].as_sequence().unwrap();
    let uses = |task: &str| {
        tasks.iter().find(|t| t["name"] == task).unwrap()["template"]
            .as_str()
            .unwrap()
            .to_string()
    };
    assert_eq!(uses("step-travel-tier"), "step-travel-tier");
    assert_eq!(uses("step-disclosure-envelope"), "step-disclosure-envelope");
    assert_eq!(uses("step-travel-tier-typed"), "step");
    // Its own template for its network, not its bounds (#1232).
    assert_eq!(uses("step-catalog-fetch"), "step-catalog-fetch");
}

// ── the worked example ────────────────────────────────────────────────────────

/// The streamflow overlay: the worked example's generated workflows, and the objects they name.
fn overlay() -> PathBuf {
    common::repo_root().join("yidam/cluster/overlays/streamflow")
}

/// The streamflow overlay on push (#1235): the streamflow overlay, plus the EventSource and the
/// Sensor that submit its run.
fn on_push_overlay() -> PathBuf {
    common::repo_root().join("yidam/cluster/overlays/streamflow-on-push")
}

/// The manifests in the streamflow overlay are generated from the streamflow example and
/// checked in. They are regenerated with `UPDATE_GOLDENS=1`, and this fails when the generator
/// and the files disagree, so the overlay never deploys a workflow the binary would not write.
#[test]
fn the_documented_workflow_is_what_the_generator_writes() {
    let c = Cluster::as_declared();
    let workflow = [
        "cluster",
        "workflow",
        "--remote",
        "git@github.com:goedelsoup/streamflow.git",
        "--image",
        "ghcr.io/goedelsoup/yidam-cluster:latest",
        "--vault-url",
        "file:///var/yidam/vault",
    ];
    // The executor's address is a placeholder from the documentation range: an operator
    // regenerates with their own API server's (#1232).
    let policy = [
        "cluster",
        "network-policy",
        "--vault-url",
        "file:///var/yidam/vault",
        "--executor",
        "192.0.2.1/32",
    ];
    for (dir, file, args) in [
        (overlay(), "streamflow.workflow.yml", workflow.to_vec()),
        (
            overlay(),
            "streamflow.cronworkflow.yml",
            [&workflow[..], &["--cron", "0 6 * * *"]].concat(),
        ),
        (overlay(), "streamflow.netpol.yml", policy.to_vec()),
        (
            on_push_overlay(),
            "streamflow.onpush.yml",
            [&workflow[..], &["--on-push", "github"]].concat(),
        ),
    ] {
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
        let path = dir.join(file);
        if std::env::var("UPDATE_GOLDENS").is_ok() {
            std::fs::create_dir_all(&dir).unwrap();
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
        for doc in serde_yaml::Deserializer::from_str(&actual) {
            Value::deserialize(doc).unwrap_or_else(|e| panic!("{file} is not YAML: {e}"));
        }
    }
}

/// The walker reads optionality and every form a pod names a secret by, since the overlay check
/// below treats an optional secret as one the overlay need not create.
#[test]
fn every_form_a_pod_names_an_object_by_is_read() {
    let spec: serde_yaml::Value = serde_yaml::from_str(
        "serviceAccountName: sa\n\
         volumes:\n\
         - {name: a, secret: {secretName: s}}\n\
         - {name: b, persistentVolumeClaim: {claimName: c}}\n\
         templates:\n\
         - container:\n    \
             envFrom: [{secretRef: {name: e, optional: true}}]\n    \
             env: [{name: X, valueFrom: {secretKeyRef: {name: k, key: x}}}]\n\
         github: {push: {webhookSecret: {name: w, key: secret}}}\n\
         webhook: {push: {authSecret: {name: t, key: secret}}}\n",
    )
    .unwrap();
    let mut got: Vec<(&str, String, bool)> = named_objects(&spec)
        .iter()
        .map(|n| (n.kind(), n.name.clone(), n.optional))
        .collect();
    got.sort();
    assert_eq!(
        got,
        [
            ("PersistentVolumeClaim", "c".to_string(), false),
            ("Secret", "e".to_string(), true),
            ("Secret", "k".to_string(), false),
            ("Secret", "s".to_string(), false),
            ("Secret", "t".to_string(), false),
            ("Secret", "w".to_string(), false),
            ("ServiceAccount", "sa".to_string(), false),
        ]
    );
}

/// Copy `from` to `to`, leaving out any `secrets/` directory: an operator's real keys in a
/// checkout have no business in a test's tempdir.
fn copy_tree(from: &Path, to: &Path) {
    for entry in walkdir::WalkDir::new(from)
        .into_iter()
        .filter_entry(|e| e.file_name() != "secrets")
    {
        let entry = entry.unwrap();
        let dest = to.join(entry.path().strip_prefix(from).unwrap());
        if entry.file_type().is_dir() {
            std::fs::create_dir_all(&dest).unwrap();
        } else {
            std::fs::copy(entry.path(), &dest).unwrap();
        }
    }
}

/// `kustomize build` of the overlay `name` under `yidam/cluster/overlays/`, from a copy with a
/// placeholder at every path its secret generators read. The paths are read off the generators rather than listed here,
/// so a key added to a secret is written too.
fn build_overlay(name: &str) -> Vec<serde_yaml::Value> {
    let tmp = tempfile::tempdir().unwrap();
    copy_tree(&common::repo_root().join("yidam/cluster"), tmp.path());
    // Every kustomization's generators, since an overlay layered on streamflow's builds
    // streamflow's objects too. Counted for the named overlay's own, so a walk that found
    // none is not a pass.
    let own = tmp.path().join("overlays").join(name).join("objects");
    let mut placeholders = 0;
    for entry in walkdir::WalkDir::new(tmp.path()) {
        let entry = entry.unwrap();
        if entry.file_name() != "kustomization.yaml" {
            continue;
        }
        let dir = entry.path().parent().unwrap();
        let k: serde_yaml::Value =
            serde_yaml::from_str(&std::fs::read_to_string(entry.path()).unwrap()).unwrap();
        for g in k["secretGenerator"].as_sequence().into_iter().flatten() {
            let files = g["files"].as_sequence().into_iter().flatten();
            let envs = g["envs"].as_sequence().into_iter().flatten();
            for f in files.chain(envs).filter_map(serde_yaml::Value::as_str) {
                let path = dir.join(f.rsplit_once('=').map_or(f, |(_, p)| p));
                std::fs::create_dir_all(path.parent().unwrap()).unwrap();
                std::fs::write(&path, "placeholder\n").unwrap();
                placeholders += usize::from(dir == own);
            }
        }
    }
    assert!(
        placeholders > 0,
        "the {name} overlay's objects generate no secret from a file, so this read the \
         wrong kustomization or the generators moved"
    );

    let o = Command::new("kustomize")
        .arg("build")
        .arg(tmp.path().join("overlays").join(name))
        .output()
        .unwrap_or_else(|e| {
            panic!(
                "could not run `kustomize` ({e}). `mise run ci-cli` provisions the version \
                 mise.toml pins; running the suite outside it needs `kustomize` on PATH"
            )
        });
    assert!(
        o.status.success(),
        "kustomize build of the {name} overlay failed:\n{}",
        String::from_utf8_lossy(&o.stderr)
    );
    let text = String::from_utf8(o.stdout).unwrap();
    serde_yaml::Deserializer::from_str(&text)
        .map(|doc| serde_yaml::Value::deserialize(doc).unwrap())
        .collect()
}

/// A manifest in the streamflow overlay, parsed.
fn overlay_manifest(file: &str) -> serde_yaml::Value {
    serde_yaml::from_str(&std::fs::read_to_string(overlay().join(file)).unwrap()).unwrap()
}

/// #1229. Deploying a corpus was `argo-rbac.yml` under a `sed`, two `kubectl create secret`
/// lines and a claim, from a docs page, with nothing checking they agreed with the workflow.
/// A secret named wrong showed up as a pod stuck in `ContainerCreating`.
///
/// The overlay's build is compared with the workflow in both directions. Every object the
/// workflow needs exists, as the kind its field names. Every account, secret and claim the
/// build creates is one the workflow names, so a leftover is as red as a gap. Kustomize's
/// secret hash suffix is exactly such a gap: it renames the secret and cannot rewrite the
/// workflow's reference, because it does not know Argo's fields.
#[test]
fn the_overlay_creates_what_the_workflow_mounts_and_nothing_else() {
    use std::collections::BTreeSet;

    let built = build_overlay("streamflow");
    let kind = |v: &serde_yaml::Value| v["kind"].as_str().unwrap_or_default().to_string();
    let name = |v: &serde_yaml::Value| {
        v["metadata"]["name"]
            .as_str()
            .unwrap_or_default()
            .to_string()
    };

    let crons: Vec<&serde_yaml::Value> =
        built.iter().filter(|v| kind(v) == "CronWorkflow").collect();
    assert_eq!(crons.len(), 1, "the overlay should deploy one CronWorkflow");
    let cron = crons[0];
    assert_eq!(
        cron,
        &overlay_manifest("streamflow.cronworkflow.yml"),
        "the overlay changed the generated CronWorkflow on the way through. A transformer \
         (a prefix, a label, a namespace) reached it, and the names inside it may no longer \
         be the ones the objects were given"
    );

    // #1232: the policies arrive as generated. A prefix reaching them would rename them, and
    // a label transformer would widen what they select.
    let policies: Vec<&serde_yaml::Value> = built
        .iter()
        .filter(|v| kind(v) == "NetworkPolicy")
        .collect();
    let text = std::fs::read_to_string(overlay().join("streamflow.netpol.yml")).unwrap();
    let generated: Vec<serde_yaml::Value> = serde_yaml::Deserializer::from_str(&text)
        .map(|d| serde_yaml::Value::deserialize(d).unwrap())
        .collect();
    assert!(!generated.is_empty());
    for g in &generated {
        assert!(
            policies.contains(&g),
            "the overlay does not deploy {:?} as generated",
            g["metadata"]["name"]
        );
    }
    assert_eq!(
        policies.len(),
        generated.len(),
        "the overlay deploys another policy"
    );

    let wanted = named_objects(&cron["spec"]["workflowSpec"]);
    for k in ["ServiceAccount", "Secret", "PersistentVolumeClaim"] {
        assert!(
            wanted.iter().any(|n| n.kind() == k && !n.optional),
            "the CronWorkflow names no {k}, so the walk is not reading it:\n{wanted:#?}"
        );
    }

    let objects = ["ServiceAccount", "Secret", "PersistentVolumeClaim"];
    let created: BTreeSet<(String, String)> = built
        .iter()
        .filter(|v| objects.contains(&kind(v).as_str()))
        .map(|v| (kind(v), name(v)))
        .collect();

    let missing: BTreeSet<String> = wanted
        .iter()
        .filter(|n| !n.optional && !created.contains(&(n.kind().to_string(), n.name.clone())))
        .map(|n| format!("  {} {:?}, named by `{}`", n.kind(), n.name, n.field))
        .collect();
    assert!(
        missing.is_empty(),
        "the workflow names objects the overlay does not create, and its pods will not start:\n\
         {}\ncreated: {created:?}",
        missing.into_iter().collect::<Vec<_>>().join("\n")
    );

    let unnamed: Vec<String> = created
        .iter()
        .filter(|(k, n)| !wanted.iter().any(|w| w.kind() == k && &w.name == n))
        .map(|(k, n)| format!("  {k} {n:?}"))
        .collect();
    assert!(
        unnamed.is_empty(),
        "the overlay creates objects the workflow never names:\n{}",
        unnamed.join("\n")
    );

    // One role, bound to the account the workflow runs as.
    let account = cron["spec"]["workflowSpec"]["serviceAccountName"]
        .as_str()
        .expect("the CronWorkflow names its account");
    let roles: Vec<String> = built
        .iter()
        .filter(|v| kind(v) == "Role")
        .map(name)
        .collect();
    let bindings: Vec<&serde_yaml::Value> =
        built.iter().filter(|v| kind(v) == "RoleBinding").collect();
    assert_eq!(bindings.len(), 1, "the overlay should bind one role");
    let b = bindings[0];
    assert_eq!(
        roles,
        [b["roleRef"]["name"].as_str().unwrap_or_default()],
        "the RoleBinding should refer to the one Role the overlay creates: {b:?}"
    );
    assert!(
        b["subjects"]
            .as_sequence()
            .into_iter()
            .flatten()
            .any(|s| s["kind"].as_str() == Some("ServiceAccount")
                && s["name"].as_str() == Some(account)),
        "the RoleBinding does not bind {account:?}: {b:?}"
    );

    // A git secret holds the files its pod reads out of the mount. The file names come from
    // the workflow's own ssh command, not from a list here.
    let text = serde_yaml::to_string(cron).unwrap();
    let files: BTreeSet<&str> = text
        .split("/etc/yidam/git/")
        .skip(1)
        .map(|rest| {
            let end = rest
                .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
                .unwrap_or(rest.len());
            &rest[..end]
        })
        .collect();
    assert!(
        files.contains("key"),
        "the workflow reads no `key` under /etc/yidam/git/, so the git mount moved: {files:?}"
    );
    let mounted: Vec<&Named> = wanted.iter().filter(|n| n.field == "secretName").collect();
    assert!(!mounted.is_empty());
    for n in mounted {
        let secret = built
            .iter()
            .find(|v| kind(v) == "Secret" && name(v) == n.name)
            .unwrap();
        let keys: BTreeSet<&str> = secret["data"]
            .as_mapping()
            .into_iter()
            .flatten()
            .filter_map(|(k, _)| k.as_str())
            .collect();
        assert_eq!(
            keys, files,
            "{} holds {keys:?}, and the pod that mounts it reads {files:?}",
            n.name
        );
    }

    // Anything else the build carries is something this check does not compare yet.
    let read = ["Role", "RoleBinding", "CronWorkflow", "NetworkPolicy"];
    let unread: BTreeSet<String> = built
        .iter()
        .map(kind)
        .filter(|k| !objects.contains(&k.as_str()) && !read.contains(&k.as_str()))
        .collect();
    assert!(
        unread.is_empty(),
        "the overlay deploys kinds this check does not compare with the workflow: {unread:?}"
    );
}

/// #1235. The on-push overlay is the streamflow overlay and three things more: the Sensor's
/// account with the one role it needs, the webhook secret, and the two generated objects.
///
/// The Sensor creates the run streamflow's CronWorkflow schedules, so the run it carries is
/// compared with the CronWorkflow's: the same spec, under the same mutex, admitting first.
/// A difference would be a second kind of run, and the mutex would be all that kept two
/// landers apart.
#[test]
fn the_on_push_overlay_adds_what_the_sensor_names_and_nothing_else() {
    use std::collections::BTreeSet;

    let base = build_overlay("streamflow");
    let built = build_overlay("streamflow-on-push");
    let kind = |v: &serde_yaml::Value| v["kind"].as_str().unwrap_or_default().to_string();
    let name = |v: &serde_yaml::Value| {
        v["metadata"]["name"]
            .as_str()
            .unwrap_or_default()
            .to_string()
    };
    for b in &base {
        assert!(
            built.contains(b),
            "the on-push overlay changed or dropped streamflow's {} {:?}",
            kind(b),
            name(b)
        );
    }
    let added: Vec<&serde_yaml::Value> = built.iter().filter(|v| !base.contains(v)).collect();
    let mut kinds: Vec<String> = added.iter().map(|v| kind(v)).collect();
    kinds.sort();
    assert_eq!(
        kinds,
        [
            "EventSource",
            "Role",
            "RoleBinding",
            "Secret",
            "Sensor",
            "ServiceAccount"
        ],
        "the on-push overlay should add these and only these"
    );
    let one = |k: &str| *added.iter().find(|v| kind(v) == k).unwrap();

    // The generated objects arrive as generated: a prefix would rename them, and the Sensor's
    // `eventSourceName` would name nothing.
    let text = std::fs::read_to_string(on_push_overlay().join("streamflow.onpush.yml")).unwrap();
    let generated: Vec<serde_yaml::Value> = serde_yaml::Deserializer::from_str(&text)
        .map(|d| serde_yaml::Value::deserialize(d).unwrap())
        .collect();
    assert_eq!(generated.len(), 2);
    for g in &generated {
        assert!(
            added.contains(&g),
            "the overlay does not deploy {} as generated",
            kind(g)
        );
    }
    let (source, sensor) = (one("EventSource"), one("Sensor"));
    let dependency = &sensor["spec"]["dependencies"][0];
    assert_eq!(dependency["eventSourceName"], source["metadata"]["name"]);

    // The account the Sensor creates the run as exists, and may create a Workflow and nothing
    // else. The run itself is the CronWorkflow's account's, which streamflow's overlay made.
    let account = sensor["spec"]["template"]["serviceAccountName"]
        .as_str()
        .expect("the Sensor names its account");
    assert_eq!(name(one("ServiceAccount")), account);
    let binding = one("RoleBinding");
    assert_eq!(binding["subjects"][0]["name"].as_str(), Some(account));
    assert_eq!(binding["subjects"].as_sequence().map(Vec::len), Some(1));
    let role = one("Role");
    assert_eq!(
        binding["roleRef"]["name"].as_str(),
        Some(name(role).as_str())
    );
    let rules: serde_yaml::Value = serde_yaml::from_str(
        "[{apiGroups: [argoproj.io], resources: [workflows], verbs: [create]}]",
    )
    .unwrap();
    assert_eq!(
        role["rules"], rules,
        "the Sensor's account may do more than create a run"
    );

    // The secret the EventSource checks a push against, with the key it reads.
    let wanted = named_objects(source);
    assert_eq!(wanted.len(), 1, "{wanted:?}");
    let secret = one("Secret");
    assert_eq!(name(secret), wanted[0].name);
    let key = source["spec"]["github"]["push"]["webhookSecret"]["key"]
        .as_str()
        .unwrap();
    let keys: BTreeSet<&str> = secret["data"]
        .as_mapping()
        .into_iter()
        .flatten()
        .filter_map(|(k, _)| k.as_str())
        .collect();
    assert_eq!(keys, BTreeSet::from([key]));

    // The run on push is the run on the cron.
    let run = &sensor["spec"]["triggers"][0]["template"]["k8s"]["source"]["resource"];
    assert_eq!(run["kind"].as_str(), Some("Workflow"));
    assert!(run["metadata"]["generateName"].is_string());
    let cron = base.iter().find(|v| kind(v) == "CronWorkflow").unwrap();
    assert_eq!(run["spec"], cron["spec"]["workflowSpec"]);
    assert_eq!(
        run["spec"]["synchronization"]["mutexes"][0]["name"].as_str(),
        Some("yidam-streamflow"),
        "a run on push and a run on the cron would not queue on one lock"
    );
}

/// The one-shot workflow is not in the overlay, since kustomize refuses its `generateName`.
/// It is submitted against the objects the overlay created, so it must name the same ones.
#[test]
fn the_one_shot_workflow_names_what_the_cron_workflow_names() {
    let once = overlay_manifest("streamflow.workflow.yml");
    let cron = overlay_manifest("streamflow.cronworkflow.yml");
    assert!(once["metadata"]["generateName"].is_string());
    let sorted = |v: &serde_yaml::Value| {
        let mut n = named_objects(v);
        n.sort();
        n.dedup();
        n
    };
    let once = sorted(&once["spec"]);
    assert!(!once.is_empty(), "the one-shot workflow names nothing");
    assert_eq!(once, sorted(&cron["spec"]["workflowSpec"]));
}

// ── a gather as a cluster step (#1217) ────────────────────────────────────────

/// A peer's `.yiz`: a manifest pinned to `commit`, and one station in the peer's own words.
fn peer_yiz(commit: &str) -> Vec<u8> {
    use std::io::Write as _;
    let files = [
        ("manifest.yml", format!("commit: \"{commit}\"\n")),
        (
            "corpus/station.ont.yml",
            "class: station\nproperties:\n  - name: code\n    type: string\n  - name: unit\n    \
             type: string\n"
                .to_string(),
        ),
        (
            "corpus/station/one.yml",
            "class: station\nlabel: one\nproperties:\n  code: \"00060\"\n  unit: cfs\n".to_string(),
        ),
    ];
    let gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    let mut tar = tar::Builder::new(gz);
    for (path, body) in files {
        let mut header = tar::Header::new_gnu();
        header.set_size(body.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        tar.append_data(&mut header, path, body.as_bytes()).unwrap();
    }
    let mut gz = tar.into_inner().unwrap();
    gz.flush().unwrap();
    gz.finish().unwrap()
}

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::Digest as _;
    hex::encode(sha2::Sha256::digest(bytes))
}

impl Cluster {
    /// Put bytes in the `file://` vault under their digest, as a pod's `put` would.
    fn vault_put(&self, bytes: &[u8]) -> String {
        let hash = sha256_hex(bytes);
        let dir = self.work.path().join("vault/sha256").join(&hash[..2]);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(&hash), bytes).unwrap();
        hash
    }

    /// A gather over one locked peer whose url nothing serves, and whose bundle the vault
    /// already holds — committed and pushed.
    fn with_a_gather(&self) {
        let bytes = peer_yiz("aaa1111");
        let sha = self.vault_put(&bytes);
        let root = self.e.path();
        std::fs::create_dir_all(root.join(".yidam/gathers")).unwrap();
        std::fs::write(
            root.join(".yidam/gathers/units.toml"),
            "question = \"What units does each gage publish?\"\nquery = \"gage\"\nanswer = \
             \"units\"\nkey = \"parameter\"\nlands_as = \"concept\"\n\n[peers.alpha]\nclasses \
             = { gage = \"station\" }\nproperties = { parameter = \"code\", units = \"unit\" }\n",
        )
        .unwrap();
        std::fs::create_dir_all(root.join(".yidam/tonpa")).unwrap();
        std::fs::write(
            root.join(".yidam/tonpa/tonpa.lock"),
            format!(
                "[[package]]\nname = \"alpha\"\nurl = \"http://127.0.0.1:9/alpha.yiz\"\nsha256 = \
                 \"{sha}\"\ncommit = \"aaa1111\"\n"
            ),
        )
        .unwrap();
        git(
            &root,
            &["add", "-f", ".yidam/gathers", ".yidam/tonpa/tonpa.lock"],
        );
        git(
            &root,
            &["commit", "-qm", "configure: gather units from alpha"],
        );
        git(&root, &["push", "-q", "origin", "HEAD:refs/heads/main"]);
    }

    /// survey → one ask per planned peer → gather, the way the workflow chains them.
    fn gather_pods(&self, bundle: &str) -> Value {
        let mut args = vec!["cluster", "survey", "units", "--bundle", bundle];
        let vault = self.vault_args();
        args.extend(vault.iter().map(String::as_str));
        let survey = self.record(&args);
        let mut asked = Vec::new();
        for ask in survey["asks"].as_array().unwrap() {
            let json = ask.to_string();
            let mut args = vec!["cluster", "ask", "--ask", json.as_str()];
            args.extend(vault.iter().map(String::as_str));
            let out = self.record(&args);
            assert_eq!(out["source"], "vault", "{out}");
            asked.push(out.to_string());
        }
        let asked = serde_json::to_string(&asked).unwrap();
        let mut args = vec![
            "cluster", "gather", "units", "--bundle", bundle, "--asked", &asked,
        ];
        args.extend(vault.iter().map(String::as_str));
        self.record(&args)
    }
}

#[test]
fn a_gather_runs_as_pods_and_lands_on_its_own_proposal_branch() {
    let c = Cluster::new();
    c.with_a_gather();
    let main = c.main_tip();
    let pin = c.pin();
    let step = c.gather_pods(s(&pin["bundle"]));
    assert_eq!(step["step"], "gather/units");
    assert_eq!(step["outcome"], "ran");
    assert_eq!(step["class"], "epistemic");

    let landed = c.land(&step);
    let target = s(&landed["target"]).to_string();
    assert!(target.starts_with("propose/gather/units/"), "{landed}");
    let tip = c
        .remote_ref(&format!("refs/heads/{target}"))
        .expect("the gather branch is on the remote");
    assert_eq!(c.main_tip(), main, "a gather never moves the branch");
    let receipt = out(
        &c.remote(),
        &["show", &format!("{tip}:.yidam/runs/gather/units/alpha.yml")],
    );
    assert!(receipt.contains("format_version: 2"), "{receipt}");
    assert!(receipt.contains("commit: aaa1111"), "{receipt}");

    // The same pins again: the same tree, and the lander writes nothing.
    let pin = c.pin();
    let again = c.land(&c.gather_pods(s(&pin["bundle"])));
    assert!(again["landed"].is_null(), "{again}");
    assert_eq!(c.remote_ref(&format!("refs/heads/{target}")), Some(tip));
}

#[test]
fn the_lander_refuses_a_gather_record_that_names_another_gather() {
    let c = Cluster::new();
    c.with_a_gather();
    let pin = c.pin();
    let mut step = c.gather_pods(s(&pin["bundle"]));
    // The commits are `units`'s; the record says they are some other gather's.
    step["step"] = "gather/other".into();
    step["receipt"] = ".yidam/runs/gather/other".into();
    let (stdout, stderr, code) = c.land_raw(&step);
    assert_ne!(code, 0, "{stdout}");
    assert!(stderr.contains("only adds its own"), "{stderr}");
    let branches = out(&c.remote(), &["for-each-ref", "refs/heads/propose"]);
    assert_eq!(branches, "", "nothing landed");
}

#[test]
fn the_lander_refuses_a_gather_whose_commit_is_not_an_open() {
    // `regen:` is operational and would go to the branch; `chore:` is outside the vocabulary,
    // which classifies as epistemic — so a class check alone would land it.
    for subject in ["regen: gather units", "chore: gather units"] {
        refused_as_a_gather(subject);
    }
}

fn refused_as_a_gather(subject: &str) {
    let c = Cluster::new();
    c.with_a_gather();
    let input = c.main_tip();
    // A commit holding exactly what a gather writes, under a subject that would put it on the
    // branch — built by hand, the way a pod that lied would build it.
    let forge = c.work.path().join("forge");
    // The branch is named: the bare remote's HEAD follows `init.defaultBranch`, which is
    // `master` on a runner with no config, and a clone of that would check out nothing.
    git(
        c.work.path(),
        &[
            "clone",
            "-q",
            "--branch",
            "main",
            c.remote().to_str().unwrap(),
            "forge",
        ],
    );
    for (path, body) in [
        (
            ".yidam/corpus/concept/gather-units.yml",
            "class: concept\nlabel: \"? units\"\n",
        ),
        (".yidam/runs/gather/units/alpha.yml", "format_version: 2\n"),
    ] {
        let file = forge.join(path);
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(file, body).unwrap();
    }
    git(&forge, &["add", "-A"]);
    git(
        &forge,
        &[
            "-c",
            "user.name=pod",
            "-c",
            "user.email=pod@x",
            "commit",
            "-qm",
            subject,
        ],
    );
    let sha = out(&forge, &["rev-parse", "HEAD"]);
    git(&forge, &["update-ref", "refs/yidam/out", &sha]);
    let file = c.work.path().join("forged.bundle");
    git(
        &forge,
        &[
            "bundle",
            "create",
            "-q",
            file.to_str().unwrap(),
            &format!("{input}..refs/yidam/out"),
        ],
    );
    let bundle = c.vault_put(&std::fs::read(&file).unwrap());
    let step = serde_json::json!({
        "format_version": 1,
        "step": "gather/units",
        "outcome": "ran",
        "sha": sha,
        "class": "epistemic",
        "verb": "open",
        "receipt": ".yidam/runs/gather/units",
        "input": input,
        "bundle": bundle,
    });
    let (stdout, stderr, code) = c.land_raw(&step);
    assert_ne!(code, 0, "{stdout}");
    assert!(
        stderr.contains("only opens questions"),
        "{subject}: {stderr}"
    );
    assert_eq!(c.main_tip(), input, "the branch did not move");
    assert_eq!(
        out(&c.remote(), &["for-each-ref", "refs/heads/propose"]),
        ""
    );
}
