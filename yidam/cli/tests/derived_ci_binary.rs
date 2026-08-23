//! A derived repository's graph gate must not need a Rust compiler.
//!
//! `sadhana/github/workflows/ci.yml` is installed into every repository bootstrap produces,
//! and its `corpus` job is the one that runs from the genesis commit onward. It used to
//! provision a toolchain and compile the CLI — so a job that is pure Rust reading Markdown
//! made a Rust toolchain a prerequisite of every derived repository's CI, forever.
//!
//! Nothing in this repository ever runs that workflow (see `derived_repo_smoke.rs` on why
//! that matters), so these are structural assertions over its text. They cannot prove the
//! shell is right. What they pin is the set of properties an edit could remove while the
//! file still looks reasonable and every job stays green.

use std::path::PathBuf;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn workflow() -> serde_yaml::Value {
    let p = root().join("sadhana/github/workflows/ci.yml");
    let text =
        std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{} unreadable: {e}", p.display()));
    serde_yaml::from_str(&text).expect("the derived-repo ci.yml parses")
}

/// Every step of the `corpus` job, as (name-or-uses, step).
fn corpus_steps() -> Vec<(String, serde_yaml::Value)> {
    workflow()["jobs"]["corpus"]["steps"]
        .as_sequence()
        .expect("the corpus job has steps")
        .iter()
        .map(|s| {
            let label = s["name"]
                .as_str()
                .or_else(|| s["uses"].as_str())
                .unwrap_or("<unnamed>")
                .to_string();
            (label, s.clone())
        })
        .collect()
}

fn step_containing(needle: &str) -> serde_yaml::Value {
    corpus_steps()
        .into_iter()
        .find(|(label, s)| {
            label.contains(needle) || s["run"].as_str().is_some_and(|r| r.contains(needle))
        })
        .unwrap_or_else(|| panic!("no corpus step mentions {needle}"))
        .1
}

/// The toolchain step must be conditional.
///
/// This is the whole prize of the change: a derived repository with no `crates/` workspace
/// of its own — which the `detect` job gates on separately — provisions no Rust toolchain
/// at all on the ordinary path. An unconditional `dtolnay/rust-toolchain` reinstates the
/// dependency while every other step still reads as a download.
#[test]
fn the_rust_toolchain_is_only_provisioned_when_something_must_be_compiled() {
    let (label, step) = corpus_steps()
        .into_iter()
        .find(|(_, s)| {
            s["uses"]
                .as_str()
                .is_some_and(|u| u.contains("rust-toolchain"))
        })
        .expect("the corpus job no longer references a Rust toolchain at all");
    let cond = step["if"]
        .as_str()
        .unwrap_or_else(|| panic!("`{label}` provisions a toolchain unconditionally"));
    assert!(
        cond.contains("download") && cond.contains("cache-hit"),
        "the toolchain must be skipped when the binary was downloaded or restored: {cond}"
    );
}

/// The compile must be a fallback, not the path.
#[test]
fn the_compile_runs_only_when_the_download_and_the_cache_both_missed() {
    let step = step_containing("cargo install");
    let cond = step["if"].as_str().expect("the compile is unconditional");
    assert!(
        cond.contains("steps.download.outputs.ok != 'true'"),
        "the compile must not run after a successful download: {cond}"
    );
    assert!(
        cond.contains("cache-hit"),
        "the compile must not run after a cache hit: {cond}"
    );
}

/// A download must be verified before it is installed.
///
/// The binary this fetches is what gates every commit in the repository. An unverified
/// artifact installed quietly is the one outcome worth failing for, and the fallback below
/// makes failing cheap — there is always a compile to fall through to.
#[test]
fn the_downloaded_artifact_is_checksummed_before_it_is_installed() {
    let step = step_containing("Download the released binary");
    let run = step["run"]
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
    let step = step_containing("Download the released binary");
    assert_eq!(
        step["continue-on-error"].as_bool(),
        Some(true),
        "a download failure must fall through to the source build, not redden the gate"
    );
}

/// `--version` must run on every path, unconditionally.
///
/// It is more valuable against a downloaded artifact than against a compiled one: it is the
/// only check that the thing which arrived is the thing that was asked for. A condition on
/// it would exempt exactly the path that most needs it.
#[test]
fn the_binary_is_asked_what_it_is_on_every_path() {
    let (label, step) = corpus_steps()
        .into_iter()
        .find(|(_, s)| {
            s["run"]
                .as_str()
                .is_some_and(|r| r.contains("yidam --version"))
        })
        .expect("nothing asks the binary what it is");
    assert!(
        step["if"].is_null(),
        "`{label}` is conditional; the version check must cover the download path too"
    );
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
    let step = step_containing("stale");
    let run = step["run"]
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

/// The workflow's tag resolver and `mise.yidam.toml`'s must be the same program.
///
/// Two transcriptions of one resolver, each pinned only by itself, is how one of them
/// silently stops matching — and neither failure mode here is an error. A resolver that
/// reads the bare `refs/tags/<x>` line compares a commit against an annotated tag's OBJECT
/// sha and never matches; the symptom is a download path that looks exactly like a pin with
/// no release, forever.
#[test]
fn the_workflow_resolves_tags_with_the_same_program_as_yidam_build() {
    fn extract(source: &str) -> String {
        let marker = "resolve_tag='";
        let start = source.find(marker).expect("no resolve_tag here") + marker.len();
        source[start..start + source[start..].find('\'').expect("unterminated awk")]
            .lines()
            .map(str::trim)
            .collect::<Vec<_>>()
            .join("\n")
    }

    let task: toml::Value =
        toml::from_str(&std::fs::read_to_string(root().join("mise.yidam.toml")).unwrap())
            .expect("mise.yidam.toml parses");
    let from_task = extract(
        task["yidam-build"]["run"]
            .as_str()
            .expect("yidam-build.run"),
    );

    let from_workflow = extract(
        step_containing("Resolve the pin")["run"]
            .as_str()
            .expect("the resolve step runs a script"),
    );

    assert_eq!(
        from_workflow, from_task,
        "the derived-repo CI resolver has drifted from `yidam-build`'s"
    );
}

/// And it must still work. Driven over a fixture `git ls-remote --tags` listing, so this
/// asserts on behaviour rather than on the two copies agreeing about the same mistake.
#[test]
fn the_workflow_resolver_matches_the_commit_and_not_the_tag_object() {
    let run = step_containing("Resolve the pin")["run"]
        .as_str()
        .expect("the resolve step runs a script")
        .to_string();
    let marker = "resolve_tag='";
    let start = run.find(marker).expect("no resolve_tag") + marker.len();
    let program = &run[start..start + run[start..].find('\'').expect("unterminated awk")];

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
    let run = step_containing("Resolve the pin")["run"]
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
