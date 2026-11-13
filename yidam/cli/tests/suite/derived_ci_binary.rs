//! A derived repository's CI must not need a Rust compiler to get its yidam.
//!
//! `sadhana/github/workflows/` is installed into every repository bootstrap produces. Its
//! `corpus` job is the one that runs from the genesis commit onward. It used to provision a
//! toolchain and compile the CLI, so a job that is pure Rust reading Markdown made a Rust
//! toolchain a prerequisite of every derived repository's CI, forever.
//!
//! The corpus job was fixed first and alone. The privacy job beside it and both release
//! jobs went on cloning and compiling at every pin, released or not (#1308). So these tests
//! are not about the corpus job. They are about every job in every scaffolded workflow that
//! installs or runs yidam, found by reading the workflows rather than from a list. A job
//! added later is held to the same sequence the day it lands, or it reddens here.
//!
//! The corpus job is the reference: the other jobs' scripts must equal its scripts. One
//! job, `index`, compiles on purpose, and [`ALWAYS_COMPILES`] says why.
//!
//! Nothing in this repository ever runs these workflows (see `derived_repo_smoke.rs` on why
//! that matters), so these are structural assertions over their text. They cannot prove the
//! shell is right. What they pin is the set of properties an edit could remove while the
//! file still looks reasonable and every job stays green.

use std::path::PathBuf;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The jobs that compile yidam on every run, by `(workflow file, job id, why)`.
///
/// `index` needs `--features index`, and no release carries it. That is a decision, not an
/// omission. The comment on `--features vector-read` in this repository's own
/// `.github/workflows/release.yml` records it (#442, #1287), and what would reverse it.
const ALWAYS_COMPILES: &[(&str, &str, &str)] = &[(
    "index.yml",
    "index",
    "needs --features index, which no release is built with",
)];

/// One job of one scaffolded workflow.
struct Job {
    file: String,
    id: String,
    steps: Vec<serde_yaml::Value>,
}

impl Job {
    fn at(&self) -> String {
        format!("{}:{}", self.file, self.id)
    }

    fn runs(&self) -> impl Iterator<Item = &str> {
        self.steps.iter().filter_map(|s| s["run"].as_str())
    }

    /// The first step whose name contains `needle`, with its index.
    fn named(&self, needle: &str) -> (usize, &serde_yaml::Value) {
        self.steps
            .iter()
            .enumerate()
            .find(|(_, s)| s["name"].as_str().is_some_and(|n| n.contains(needle)))
            .unwrap_or_else(|| panic!("{} has no step named like `{needle}`", self.at()))
    }

    /// The first step that `uses:` something containing `needle`.
    fn using(&self, needle: &str) -> (usize, &serde_yaml::Value) {
        self.steps
            .iter()
            .enumerate()
            .find(|(_, s)| s["uses"].as_str().is_some_and(|u| u.contains(needle)))
            .unwrap_or_else(|| panic!("{} uses no `{needle}`", self.at()))
    }

    /// Whether a step builds yidam from its source tree.
    fn compiles_yidam(&self) -> bool {
        self.runs().any(|r| {
            r.contains("yidam/cli") && (r.contains("cargo install") || r.contains("cargo build"))
        })
    }

    /// Whether a step invokes the `yidam` binary.
    ///
    /// The converse of [`Job::compiles_yidam`]. A job could fetch yidam some new way that
    /// the compile scan does not recognize, and that scan would then pass having seen
    /// nothing. Any job that runs the binary has to get it from somewhere, so this finds it
    /// anyway.
    fn runs_yidam(&self) -> bool {
        self.runs().flat_map(str::lines).any(|l| {
            let l = l.trim_start();
            l.starts_with("yidam ") || l == "yidam"
        })
    }
}

fn jobs() -> Vec<Job> {
    let dir = root().join("sadhana/github/workflows");
    let mut files: Vec<_> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("{} unreadable: {e}", dir.display()))
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|x| x == "yml"))
        .collect();
    files.sort();
    let mut out = Vec::new();
    for path in files {
        let text = std::fs::read_to_string(&path).unwrap();
        let doc: serde_yaml::Value = serde_yaml::from_str(&text)
            .unwrap_or_else(|e| panic!("{} does not parse: {e}", path.display()));
        let file = path.file_name().unwrap().to_string_lossy().to_string();
        for (id, job) in doc["jobs"].as_mapping().expect("a workflow has jobs") {
            out.push(Job {
                file: file.clone(),
                id: id.as_str().unwrap().to_string(),
                steps: job["steps"].as_sequence().cloned().unwrap_or_default(),
            });
        }
    }
    out
}

fn exempt(job: &Job) -> bool {
    ALWAYS_COMPILES
        .iter()
        .any(|(f, id, _)| *f == job.file && *id == job.id)
}

/// Every job that gets yidam the standard way. The corpus job is among them.
fn installers() -> Vec<Job> {
    jobs()
        .into_iter()
        .filter(|j| !exempt(j) && (j.compiles_yidam() || j.runs_yidam()))
        .collect()
}

fn corpus() -> Job {
    jobs()
        .into_iter()
        .find(|j| j.file == "ci.yml" && j.id == "corpus")
        .expect("ci.yml has no corpus job")
}

/// The condition a step runs under, or the empty string.
fn cond(step: &serde_yaml::Value) -> &str {
    step["if"].as_str().unwrap_or("")
}

/// The gate a job puts before everything it does, if any: the resolve step's condition.
///
/// The privacy job and the release guard run nothing when nothing is declared private. Their
/// install steps carry that gate, and only that gate, in addition to their own conditions.
fn gate(job: &Job) -> String {
    cond(job.named("Resolve the pin").1).to_string()
}

/// Every job that runs yidam installs it, and every one installs it the standard way.
///
/// This is the test #1308 was missing. The privacy job and both release jobs ran yidam,
/// and every one of them compiled it at every pin. Nothing looked at them, because the
/// tests named only the corpus job.
#[test]
fn every_job_that_runs_yidam_downloads_it_first() {
    let all = installers();
    assert!(
        all.iter().any(|j| j.id == "corpus"),
        "the scan no longer finds the corpus job, so it is blind to the form it looks for"
    );
    for job in &all {
        assert!(
            job.compiles_yidam(),
            "{} runs yidam but has no compile fallback, so a pin with no release has no binary",
            job.at()
        );
        job.named("Download the released binary");
    }
}

/// Each exemption names a job that still compiles. An exemption whose job is gone, or that
/// stopped compiling, exempts nothing — and it would exempt the next job of that name.
#[test]
fn every_exemption_is_still_a_compile() {
    let all = jobs();
    for (file, id, why) in ALWAYS_COMPILES {
        let job = all
            .iter()
            .find(|j| j.file == *file && j.id == *id)
            .unwrap_or_else(|| panic!("{file}:{id} is exempted ({why}) but does not exist"));
        assert!(
            job.compiles_yidam(),
            "{file}:{id} is exempted ({why}) but no longer compiles yidam; drop it from \
             ALWAYS_COMPILES so it is held to the download path"
        );
    }
}

/// The scripts are the corpus job's, byte for byte.
///
/// Four copies of a resolver, a download and a compile, each pinned only by itself, is how
/// one of them silently stops matching. The corpus job's are the ones the rest of this file
/// tests by behaviour, so the others are held to them.
#[test]
fn every_installer_runs_the_corpus_jobs_scripts() {
    let reference = corpus();
    for step in [
        "Resolve the pin",
        "Download the released binary",
        "Compile the yidam CLI",
    ] {
        let want = reference.named(step).1["run"].as_str().unwrap().to_string();
        for job in installers() {
            assert_eq!(
                job.named(step).1["run"].as_str(),
                Some(want.as_str()),
                "{}'s `{step}` script has drifted from the corpus job's",
                job.at()
            );
        }
    }
}

/// The toolchain is provisioned only when something must be compiled.
///
/// This is the whole prize of the change. A repository whose pin has a release provisions no
/// Rust toolchain on the ordinary path. An unconditional `dtolnay/rust-toolchain` reinstates
/// the dependency while every other step still reads as a download.
#[test]
fn the_rust_toolchain_is_only_provisioned_when_something_must_be_compiled() {
    for job in installers() {
        let c = cond(job.using("rust-toolchain").1);
        assert!(
            c.contains("steps.download.outputs.ok != 'true'") && c.contains("cache-hit"),
            "{} provisions a toolchain when the binary was downloaded or restored: `{c}`",
            job.at()
        );
    }
}

/// The compile is a fallback, not the path, and the only one.
#[test]
fn the_compile_runs_only_when_the_download_and_the_cache_both_missed() {
    for job in installers() {
        let c = cond(job.named("Compile the yidam CLI").1);
        assert!(
            c.contains("steps.download.outputs.ok != 'true'") && c.contains("cache-hit"),
            "{} compiles after a successful download or a cache hit: `{c}`",
            job.at()
        );
        let compiles = job
            .runs()
            .filter(|r| r.contains("yidam/cli") && r.contains("cargo "))
            .count();
        assert_eq!(
            compiles,
            1,
            "{} has {compiles} steps that build yidam; the second one is not a fallback",
            job.at()
        );
    }
}

/// The cache is consulted only when the download did not deliver.
///
/// On the path most repositories take, a cache step with no condition runs for nothing.
#[test]
fn the_cache_is_consulted_only_after_a_missed_download() {
    for job in installers() {
        let (_, step) = job.named("Cache the compiled binary");
        assert!(
            cond(step).contains("steps.download.outputs.ok != 'true'"),
            "{} restores the compile cache even after a download: `{}`",
            job.at(),
            cond(step)
        );
    }
}

/// The sequence runs in order: resolve, download, cache, toolchain, compile, then ask.
///
/// Each step reads the one before it. A cache step above the download sees no
/// `steps.download` output, reads it as "not ok", and runs every time.
#[test]
fn the_install_steps_run_in_order() {
    for job in installers() {
        let order = [
            job.named("Resolve the pin").0,
            job.named("Download the released binary").0,
            job.named("Cache the compiled binary").0,
            job.using("rust-toolchain").0,
            job.named("Compile the yidam CLI").0,
            job.named("Say which yidam is answering").0,
        ];
        assert!(
            order.windows(2).all(|w| w[0] < w[1]),
            "{}'s install steps are out of order: {order:?}",
            job.at()
        );
    }
}

/// A gated job carries its gate on every install step.
///
/// The privacy job and the release guard exist to cost one runner-second when nothing is
/// declared private. An install step missing the gate runs anyway, and a toolchain
/// provisioned for nothing is that runner-second turned into minutes.
#[test]
fn a_gated_job_gates_every_install_step() {
    for job in installers() {
        let gate = gate(&job);
        if gate.is_empty() {
            continue;
        }
        for (label, step) in [
            ("download", job.named("Download the released binary").1),
            ("cache", job.named("Cache the compiled binary").1),
            ("toolchain", job.using("rust-toolchain").1),
            ("compile", job.named("Compile the yidam CLI").1),
            ("version", job.named("Say which yidam is answering").1),
        ] {
            assert!(
                cond(step).contains(&gate),
                "{}'s {label} step drops the job's gate `{gate}`: `{}`",
                job.at(),
                cond(step)
            );
        }
    }
}

/// A download must be verified before it is installed.
///
/// The binary this fetches is what gates every commit in the repository. An unverified
/// artifact installed quietly is the one outcome worth failing for, and the fallback below
/// makes failing cheap — there is always a compile to fall through to.
#[test]
fn the_downloaded_artifact_is_checksummed_before_it_is_installed() {
    let job = corpus();
    let run = job.named("Download the released binary").1["run"]
        .as_str()
        .expect("the download step runs a script");
    let check = run
        .lines()
        .position(|l| l.contains("sha256sum -c"))
        .expect("the download is never checksummed");
    let install = run
        .lines()
        .position(|l| l.contains("install -m"))
        .expect("the download is never installed");
    assert!(
        check < install,
        "the checksum is verified after the binary is installed, which verifies nothing"
    );
    assert!(
        run.lines().any(|l| l.trim() == "set -eu"),
        "without `set -eu` a failed checksum is a printed warning and the install proceeds"
    );
}

/// A failed download must fall through to the compile, not fail the job.
///
/// A missing asset, a network blip, a platform the release skipped: none of them is a
/// reason a repository cannot have its binary, and the compile path is the one that has
/// always worked.
#[test]
fn a_failed_download_is_not_a_failed_gate() {
    for job in installers() {
        assert_eq!(
            job.named("Download the released binary").1["continue-on-error"].as_bool(),
            Some(true),
            "{}: a download failure must fall through to the source build, not redden the job",
            job.at()
        );
    }
}

/// `--version` runs on every install path, and before anything else asks yidam a question.
///
/// It is more valuable against a downloaded artifact than against a compiled one: it is the
/// only check that the thing which arrived is the thing that was asked for. A condition
/// naming the download or the cache would exempt exactly the path that most needs it; a
/// job's own gate is the only condition it may carry.
#[test]
fn the_binary_is_asked_what_it_is_on_every_path() {
    for job in installers() {
        let (at, step) = job.named("Say which yidam is answering");
        assert_eq!(
            step["run"].as_str().map(str::trim),
            Some("yidam --version"),
            "{}'s version step no longer asks the binary",
            job.at()
        );
        assert_eq!(
            cond(step),
            gate(&job),
            "{}'s version step is conditional on more than the job's gate",
            job.at()
        );
        let first_use = job
            .steps
            .iter()
            .position(|s| {
                s["run"].as_str().is_some_and(|r| {
                    r.lines().any(|l| {
                        let l = l.trim_start();
                        l.starts_with("yidam ") && l != "yidam --version"
                    })
                })
            })
            .unwrap_or(usize::MAX);
        assert!(
            at < first_use,
            "{} runs yidam before asking which yidam it is",
            job.at()
        );
    }
}

/// The staleness report must obtain its own clone.
///
/// It reads commit metadata out of `/tmp/yidam` and used to assume the compile step had
/// left a tree there. Caching the binary made that false on most runs and downloading it
/// made it false on every run — and because the step is `continue-on-error`, the failure
/// was a silently missing report rather than a red build. A check that exists to make
/// staleness visible had itself become invisibly absent.
#[test]
fn the_staleness_report_does_not_depend_on_a_clone_someone_else_made() {
    let job = corpus();
    let run = job.named("stale").1["run"]
        .as_str()
        .expect("the staleness step runs a script");
    let clone = run
        .lines()
        .position(|l| l.contains("git clone"))
        .expect("the staleness report never clones; it depends on a tree that may not exist");
    let cd = run
        .lines()
        .position(|l| l.trim() == "cd /tmp/yidam")
        .expect("the staleness report no longer enters the clone");
    assert!(
        clone < cd,
        "the clone must happen before the step enters the directory"
    );
}

fn resolver(source: &str) -> &str {
    let marker = "resolve_tag='";
    let start = source.find(marker).expect("no resolve_tag here") + marker.len();
    &source[start..start + source[start..].find('\'').expect("unterminated awk")]
}

/// The workflow's tag resolver and `mise.yidam.toml`'s must be the same program.
///
/// Two transcriptions of one resolver, each pinned only by itself, is how one of them
/// silently stops matching — and neither failure mode here is an error. A resolver that
/// reads the bare `refs/tags/<x>` line compares a commit against an annotated tag's OBJECT
/// sha and never matches; the symptom is a download path that looks exactly like a pin with
/// no release, forever. The other installers equal the corpus job's, so this covers them.
#[test]
fn the_workflow_resolves_tags_with_the_same_program_as_yidam_build() {
    fn normalize(awk: &str) -> String {
        awk.lines().map(str::trim).collect::<Vec<_>>().join("\n")
    }

    let task: toml::Value =
        toml::from_str(&std::fs::read_to_string(root().join("mise.yidam.toml")).unwrap())
            .expect("mise.yidam.toml parses");
    let from_task = normalize(resolver(
        task["yidam-build"]["run"]
            .as_str()
            .expect("yidam-build.run"),
    ));

    let job = corpus();
    let from_workflow = normalize(resolver(
        job.named("Resolve the pin").1["run"]
            .as_str()
            .expect("the resolve step runs a script"),
    ));

    assert_eq!(
        from_workflow, from_task,
        "the derived-repo CI resolver has drifted from `yidam-build`'s"
    );
}

/// And it must still work. Driven over a fixture `git ls-remote --tags` listing, so this
/// asserts on behaviour rather than on the two copies agreeing about the same mistake.
#[test]
fn the_workflow_resolver_matches_the_commit_and_not_the_tag_object() {
    let job = corpus();
    let run = job.named("Resolve the pin").1["run"]
        .as_str()
        .expect("the resolve step runs a script")
        .to_string();
    let program = resolver(&run);

    let a = "a".repeat(40);
    let b = "b".repeat(40);
    let c = "c".repeat(40);
    let listing = format!(
        "{a}\trefs/tags/cli/v0.2.0\n{b}\trefs/tags/cli/v0.2.0^{{}}\n{c}\trefs/tags/cli/v0.1.0\n"
    );

    let resolve = |commit: &str| -> String {
        let dir = tempfile::tempdir().unwrap();
        let listing_path = dir.path().join("ls");
        let prog_path = dir.path().join("prog.awk");
        std::fs::write(&listing_path, &listing).unwrap();
        std::fs::write(&prog_path, program).unwrap();
        let out = std::process::Command::new("awk")
            .args(["-v", &format!("c={commit}"), "-f"])
            .arg(&prog_path)
            .arg(&listing_path)
            .output()
            .expect("awk");
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    };

    assert_eq!(
        resolve(&b),
        "cli/v0.2.0",
        "an annotated tag resolves from its peeled commit"
    );
    assert_eq!(
        resolve(&a),
        "",
        "a tag OBJECT sha must not resolve — a pin is never a tag object"
    );
    assert_eq!(
        resolve(&c),
        "cli/v0.1.0",
        "a lightweight tag resolves from its only line"
    );
    assert_eq!(
        resolve(&"d".repeat(40)),
        "",
        "an unreleased commit resolves to nothing, so the gate compiles"
    );
}

/// The refspec must keep its trailing `*`, here as in `yidam-build`.
///
/// `git ls-remote` filters before peeled refs are emitted, so a pattern naming one tag
/// exactly returns only its bare line — the tag object — and the resolver above can never
/// find a release again. Narrowing it reads as a tidy-up and retires the download path.
#[test]
fn the_workflow_tag_refspec_globs() {
    let job = corpus();
    let run = job.named("Resolve the pin").1["run"]
        .as_str()
        .expect("the resolve step runs a script")
        .to_string();
    let line = run
        .lines()
        .filter(|l| !l.trim_start().starts_with('#'))
        .find(|l| l.contains("ls-remote --tags") && l.contains("cli/v"))
        .expect("the resolve step no longer lists cli/v tags");
    let start = line.find('\'').expect("the refspec is quoted") + 1;
    let refspec = &line[start..start + line[start..].find('\'').expect("unterminated refspec")];
    assert!(
        refspec.ends_with('*'),
        "the tag refspec is `{refspec}`; without the glob no `^{{}}` peeled ref is returned \
         and the resolver can never match a release again"
    );
}
