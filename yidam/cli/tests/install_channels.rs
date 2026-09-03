//! Every documented way to get the binary is checked against reality.
//!
//! `cargo binstall yidam` stood in the README for a release cycle while `yidam` did not
//! exist on crates.io. Nothing caught it, because nothing here had ever asked the question
//! CI does not ask: not "does the artifact build" but "can anyone obtain one". The answer
//! lives in `.github/workflows/install-channels.yml`, which runs each documented line in a
//! clean container weekly.
//!
//! This file is the join between the two. A workflow that checks three channels while the
//! README documents four is the same failure one channel later, and the diff that adds the
//! fourth line to the README is the diff that looks completely fine.

use std::path::PathBuf;
use walkdir::WalkDir;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn read(rel: &str) -> String {
    let p = repo_root().join(rel);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{} is unreadable ({e})", p.display()))
}

/// Every prose file in the repository, as repo-relative paths.
///
/// Discovered rather than listed. Both tests below used to read a two-name list —
/// `README.md` and `docs/quickstart.md` — while `docs/mcp-server.md` had been documenting an
/// install line *and* a pinned `cli/v*` tag that neither of them could see. A list of
/// documents is the same hole as a list of channels, one document later, and it closes the
/// same silent way: the diff that puts an install line in a third file looks completely fine.
///
/// Dot-directories are skipped, which is what keeps `.claude/worktrees/*` — checkouts of this
/// repository at other commits, pinning other releases — from being graded as documentation.
fn documented_files() -> Vec<String> {
    let root = repo_root().canonicalize().expect("repo root is readable");
    let mut docs: Vec<String> = WalkDir::new(&root)
        .into_iter()
        .filter_entry(|e| {
            let name = e.file_name().to_string_lossy();
            e.depth() == 0 || !(name.starts_with('.') || name == "target" || name == "node_modules")
        })
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
        .filter(|e| {
            let name = e.file_name().to_string_lossy().to_lowercase();
            name.ends_with(".md") || name.ends_with(".mdx")
        })
        .filter_map(|e| {
            e.path()
                .strip_prefix(&root)
                .ok()
                .map(|r| r.to_string_lossy().into_owned())
        })
        .collect();
    docs.sort();
    assert!(
        docs.contains(&"README.md".to_string()),
        "the documentation walk did not find README.md; it is scanning the wrong tree and \
         every assertion built on it is vacuous"
    );
    docs
}

/// One way to obtain the binary: how a documented line for it *opens*, what identifies a
/// collected line as this channel, and what `install-channels.yml` must be running to be
/// checking it.
///
/// The three are separate because the workflow resolves the repository and the released tag
/// rather than hardcoding them — it is the *channel* that must correspond, not the spelling.
///
/// `opener` used to be a second list, `INSTALL_PREFIXES`, sitting beside this one. Nothing
/// held the two in step, and the gap was not theoretical: `curl`, `brew` and `cargo` were the
/// only commands it knew, so a `mise` line was never collected, never matched here, and never
/// required to have a job. Documenting mise would have bought exactly what this file exists
/// to prevent — an instruction on the front page with nothing asserting it can succeed —
/// and the diff that added it would have looked completely fine.
///
/// So the collector reads its openers from here. Adding a channel cannot leave it behind,
/// and `every_channel_can_be_seen_by_the_collector` fails if an opener does not match how the
/// channel is actually written down.
struct Channel {
    /// What a documented line for this channel begins with, after trimming. Not necessarily a
    /// command: a channel declared in a config file opens with whatever key names it.
    opener: &'static str,
    /// The substring that says a collected line IS this channel.
    marker: &'static str,
    /// The substring `.github/workflows/install-channels.yml` must contain to be running it.
    probe: &'static str,
    /// How *this* channel proves it obtained the released version rather than merely
    /// something.
    ///
    /// It used to be `yidam --version` for every channel, hardcoded in one assertion, because
    /// every channel delivered the binary. Two of them now deliver a VS Code extension, which
    /// has no `--version` and is never on `PATH` — and a count of `yidam --version` against
    /// `CHANNELS.len()` would have failed those for the wrong reason, or been lowered until it
    /// asserted nothing.
    ///
    /// **It is the comparison, not the command.** `yidam --version` was the first spelling
    /// here and it failed `installer-linux`, which verifies the version by running
    /// `"$HOME/.local/bin/yidam" --version` — the binary is not on `PATH` inside that
    /// container. The literal appeared only in the step's `name:`, which is the hole this file
    /// already knows about from the other direction. What every channel genuinely has in
    /// common is that it compares what it obtained against the release the workflow resolved.
    version_probe: &'static str,
}

/// What each family of channels compares against: the release its own job resolved, rather
/// than a version hardcoded anywhere.
const CLI_VERSION: &str = "needs.released.outputs.version";
const EDITOR_VERSION: &str = "needs.editor-released.outputs.version";

const CHANNELS: &[Channel] = &[
    Channel {
        opener: "curl -fsSL",
        marker: "install.sh | sh",
        probe: "install.sh | sh",
        version_probe: CLI_VERSION,
    },
    Channel {
        opener: "brew install",
        marker: "brew install goedelsoup/tap/yidam",
        probe: "brew install goedelsoup/tap/yidam",
        version_probe: CLI_VERSION,
    },
    Channel {
        opener: "cargo binstall",
        marker: "cargo binstall yidam",
        probe: "cargo binstall",
        version_probe: CLI_VERSION,
    },
    Channel {
        opener: "cargo install",
        marker: "cargo install --git",
        probe: "cargo install --git",
        version_probe: CLI_VERSION,
    },
    // `marker` and `probe` are the same string, and deliberately carry the tool option: a
    // job that resolved the binary without `version_prefix` would be checking a channel
    // nobody can use. Four layers publish onto one release list, so without it mise asks
    // the repository-wide question — `@latest` resolves `editor/v*`, whose release ships
    // only a `.vsix`, and a bare `@0.6.0` 404s. Both verified against mise 2026.7.0.
    Channel {
        opener: "mise use",
        marker: "github:goedelsoup/yidam[version_prefix=cli/v]",
        probe: "github:goedelsoup/yidam[version_prefix=cli/v]",
        version_probe: CLI_VERSION,
    },
    // ── the bundle, which is a channel with no install command at all ──────────────────
    //
    // #421. Every other row here is a line someone types; this one is a file someone drags
    // onto an application, which is the entire point of it — the audience is the person who
    // cannot install a Rust binary. `open <file>` is what that gesture is in a shell, and it
    // is documented as the alternative rather than the instruction.
    //
    // The channel is checked the way `editor-vsix` is, and for the same reason: the artifact
    // is a zip, so the manifest inside it is this channel's `yidam --version` — an asset that
    // downloads is not evidence it is the one that was cut. It goes further than the .vsix
    // job can, because this archive carries an executable: the job runs the bundled binary
    // and makes it say its own version, on a macOS runner, which is the only check here that
    // exercises the bytes a Claude Desktop user actually runs.
    Channel {
        opener: "open yidam-",
        marker: ".mcpb",
        probe: "manifest.json",
        version_probe: CLI_VERSION,
    },
    // ── the extension is a channel too, and was exempt for as long as it was unlisted ──
    //
    // #313 asked for two things: document how the extension is obtained, and check that the
    // documentation can succeed. The documentation landed, in `README.md` and
    // `docs/editor-setup.md`. The check landed for Open VSX only, and the `.vsix` line stayed
    // uncovered — not by a decision, but because no `opener` matched it, which is precisely
    // the silent exemption `every_channel_can_be_seen_by_the_collector` exists to describe.
    //
    // It is the line that matters most. Open VSX does not serve VS Code proper, so for a VS
    // Code user the release asset is not a fallback — it is the only route the extension has,
    // and it was the one nothing asked about. `install-channels.yml` said so in a comment.
    Channel {
        opener: "codium --install-extension",
        marker: "codium --install-extension goedelsoup.yidam-vscode",
        probe: "open-vsx.org",
        version_probe: EDITOR_VERSION,
    },
    Channel {
        opener: "code --install-extension",
        marker: "code --install-extension yidam-vscode-",
        // The manifest inside the archive, which is this channel's `yidam --version`: an
        // asset that downloads and unpacks is not evidence it is the one that was cut.
        probe: "extension/package.json",
        version_probe: EDITOR_VERSION,
    },
];

/// Lines that build from a checkout the reader already has. There is no registry, tag or
/// artifact for these to be wrong about and nothing for a clean container to fetch, so they
/// are not distribution channels — but they open with `cargo install` and would otherwise
/// read as an unchecked one. Named here rather than filtered by shape, so that calling a
/// line "not a channel" stays a decision someone wrote down.
const NOT_A_CHANNEL: &[&str] = &[
    "cargo install --path",
    // `code --install-extension yidam/editors/vscode/dist/yidam-vscode.vsix`, under "Or build
    // it from a checkout". The path is the point: it names a file `mise run ext-package`
    // writes inside this repository, so there is no release, registry or asset for it to be
    // wrong about. The release-asset line two blocks above it — `yidam-vscode-<version>.vsix`
    // — is a channel and is not exempted by this.
    //
    // Found by adding the `code --install-extension` opener, which is the whole argument for
    // deriving openers from `CHANNELS`: the line had been in `README.md` and
    // `docs/editor-setup.md` all along, collected by nothing.
    "yidam/editors/vscode/dist/",
];

fn documented_install_lines(doc: &str) -> Vec<String> {
    read(doc)
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| CHANNELS.iter().any(|c| l.starts_with(c.opener)))
        .filter(|l| l.contains("yidam"))
        .collect()
}

/// What `install-channels.yml` actually *runs*, with everything it merely says removed.
///
/// A probe matched against the whole file is matched against the job's `name:` too — and
/// every job here is named after the line it runs. So the check for "does a job run this"
/// was answerable by the job's title: deleting
/// `cargo binstall --no-confirm --disable-strategies compile yidam` from the binstall step
/// and leaving `name: cargo binstall yidam` two lines above it kept this file green.
/// Verified by doing it. Three of the four channels had that hole; only `install.sh | sh`
/// happened not to appear in a name or a comment.
///
/// This is the same reading two tests below already take for the opposite reason — they skip
/// comments so a fix is not failed by the prose explaining it. Here the prose is not merely
/// noise, it is a second copy of the answer.
fn workflow_commands(text: &str) -> String {
    let mut commands: Vec<String> = Vec::new();
    // The indentation of the `run:` key whose block we are inside, if any. A block ends at
    // the first non-blank line indented no further than its key.
    let mut block: Option<usize> = None;

    for line in text.lines() {
        let indent = line.len() - line.trim_start().len();
        if let Some(base) = block {
            if line.trim().is_empty() || indent > base {
                if !line.trim_start().starts_with('#') {
                    commands.push(line.to_string());
                }
                continue;
            }
            block = None;
        }
        let trimmed = line.trim_start();
        if trimmed.starts_with('#') {
            continue;
        }
        let Some(rest) = trimmed
            .strip_prefix("- run:")
            .or_else(|| trimmed.strip_prefix("run:"))
        else {
            continue;
        };
        let rest = rest.trim();
        // `run: |` opens a block; `run: <command>` is the whole command.
        if rest.is_empty() || rest.starts_with('|') || rest.starts_with('>') {
            block = Some(indent);
        } else {
            commands.push(rest.to_string());
        }
    }
    commands.join("\n")
}

/// Every job in the workflow, as (job name, the text of its `run:` steps).
///
/// Needed because a *count* of version assertions cannot say which channel made them. Two
/// channels assert with the editor release's own output and one job says it three times, so a
/// per-probe count of two was satisfied while the other job asserted nothing — measured by
/// deleting that job's comparison and watching this file stay green.
///
/// Jobs are the two-space keys under `jobs:`, which is how this file is written throughout.
fn workflow_jobs(text: &str) -> Vec<(String, String)> {
    let mut jobs: Vec<(String, String)> = Vec::new();
    let body = match text.split_once("\njobs:\n") {
        Some((_, rest)) => rest,
        None => return jobs,
    };
    let mut current: Option<(String, String)> = None;
    for line in body.lines() {
        let is_job_key = line.starts_with("  ")
            && !line.starts_with("   ")
            && line.trim_end().ends_with(':')
            && !line.trim_start().starts_with('#');
        if is_job_key {
            if let Some(job) = current.take() {
                jobs.push(job);
            }
            let name = line.trim().trim_end_matches(':').to_string();
            current = Some((name, String::new()));
        } else if let Some((_, buf)) = current.as_mut() {
            buf.push_str(line);
            buf.push('\n');
        }
    }
    if let Some(job) = current {
        jobs.push(job);
    }
    jobs.into_iter()
        .map(|(name, text)| {
            // `::error::` lines are dropped, and that is the point rather than tidiness. Every
            // one of these jobs names the expected version in its failure message as well as
            // in its comparison, so a mutation that deletes the comparison and leaves the
            // message reads as still checking — measured: `installer-linux`'s
            // `case "yidam ${{ … }} "*)` was replaced with `case "yidam "*)` and this file
            // stayed green, because the `::error::` echo below it still carried the string.
            //
            // A message that names the release is not a check of it.
            let commands = workflow_commands(&text)
                .lines()
                .filter(|l| !l.contains("::error::"))
                .collect::<Vec<_>>()
                .join("\n");
            (name, commands)
        })
        .collect()
}

/// Nothing may be documented as an install path without something running it.
///
/// The assertion runs in the direction that matters. A check with no documented line is
/// harmless — extra coverage. A documented line with no check is the bug this whole issue
/// was: an instruction that cannot succeed, on the front page, indefinitely.
#[test]
fn every_documented_install_line_is_checked_end_to_end() {
    let workflow = workflow_commands(&read(".github/workflows/install-channels.yml"));

    let mut checked = 0;
    for doc in documented_files() {
        for line in documented_install_lines(&doc) {
            if NOT_A_CHANNEL.iter().any(|m| line.contains(m)) {
                continue;
            }
            let channel = CHANNELS.iter().find(|c| line.contains(c.marker));
            let probe = channel.map(|c| c.probe).unwrap_or_else(|| {
                panic!(
                    "{doc} documents an install path this test does not know:\n  {line}\n\
                     Add it to CHANNELS *and* to .github/workflows/install-channels.yml — or \
                     to NOT_A_CHANNEL if it builds from a checkout. A channel nobody runs is \
                     how `cargo binstall yidam` came to be documented and impossible."
                )
            });
            assert!(
                workflow.contains(probe),
                "{doc} documents `{line}` and no `run:` step in install-channels.yml \
                 contains `{probe}` — the docs are making a promise no job checks. A job \
                 *named* after the line does not run it."
            );
            checked += 1;
        }
    }
    assert!(
        checked > 0,
        "no documented install line was found anywhere in the repository; either every \
         channel was removed from the docs or this test is reading nothing"
    );
}

/// Every channel must be one the collector can actually find in the documentation.
///
/// Deriving the openers from `CHANNELS` closes the gap between two lists, but it does not by
/// itself make an opener *correct*. A channel declared with an opener that matches nothing —
/// `mise use` while the docs show a `mise.toml` block, say — is collected from no document,
/// matched against no line, and required to have no job. That is the same hole one level in:
/// the list is now single, and still describes a channel nobody checks.
///
/// So this asserts the round trip. Each channel must appear in at least one prose file, in a
/// line the collector picks up. It runs in the direction the test above deliberately does not:
/// there, a workflow job with no documented line is harmless extra coverage; here, a
/// `CHANNELS` entry is by construction "a line the README tells someone to run", so one that
/// no document contains is either an undocumented channel or an opener that cannot see it,
/// and both are worth a red mark.
#[test]
fn every_channel_can_be_seen_by_the_collector() {
    let docs = documented_files();
    let mut unseen: Vec<&str> = Vec::new();

    for channel in CHANNELS {
        let found = docs.iter().any(|doc| {
            documented_install_lines(doc)
                .iter()
                .any(|line| line.contains(channel.marker))
        });
        if !found {
            unseen.push(channel.marker);
        }
    }

    assert!(
        unseen.is_empty(),
        "no prose file contains a line the collector picks up for: {unseen:?}\n\
         Either the channel is undocumented — in which case nobody can use it and it should \
         not be in CHANNELS — or its `opener` does not match how it is written down, so it \
         is silently exempt from `every_documented_install_line_is_checked_end_to_end`. \
         That exemption is what let `mise` be a channel this file could not see."
    );

    // ...and the collector must be reading prose, not returning nothing. `unseen` being empty
    // is only meaningful if `documented_install_lines` found something at all; a walk over the
    // wrong tree makes every assertion above vacuous in the passing direction.
    let collected: usize = docs.iter().map(|d| documented_install_lines(d).len()).sum();
    assert!(
        collected >= CHANNELS.len(),
        "the collector found {collected} install line(s) across {} document(s) for {} \
         channel(s); it is not reading what it claims to",
        docs.len(),
        CHANNELS.len()
    );
}

/// Each channel must prove it obtained the *released* version, not merely something.
///
/// A tap that lags a release, or a binstall that quietly compiled from source, both end with
/// a working `yidam` on PATH. Only the version tells them apart. The same holds for a `.vsix`
/// that downloads and unpacks: that is not evidence it is the one that was cut.
///
/// **Asserted in the job that obtains it, not counted across the file.** Two earlier shapes of
/// this test both passed a mutation that deleted a real comparison:
///
/// - counting occurrences in the raw file, where a job `name:` and the comment above a step
///   answer for the step — the hole `workflow_commands` exists for, on another test, and this
///   one had it too;
/// - counting them per `version_probe` across all jobs, where `editor-openvsx` says
///   `needs.editor-released.outputs.version` three times and so covered the quota for two
///   channels while `editor-vsix` asserted nothing.
///
/// Both were measured by deleting the comparison and leaving its comment. So the claim is now
/// the one that was meant all along: the job whose steps run this channel's `probe` is the job
/// whose steps must contain its `version_probe`, read with `::error::` lines removed — see
/// [`workflow_jobs`] for the third mutation that made that last clause necessary.
///
/// It is still a substring check over shell, and there is a shape it cannot see: a comparison
/// rewritten to compare against something *else* that happens to mention the release nearby.
/// No substring check can. That is what review is for; what this closes is the whole class of
/// channel with no assertion at all, which is the one that let a stale tap look healthy.
#[test]
fn each_channel_asserts_the_released_version() {
    let workflow = read(".github/workflows/install-channels.yml");
    let jobs = workflow_jobs(&workflow);
    assert!(
        jobs.len() > CHANNELS.len(),
        "the job split found {} job(s) in a workflow with {} channels; it is parsing the \
         wrong thing and every assertion below is vacuous",
        jobs.len(),
        CHANNELS.len()
    );

    for channel in CHANNELS {
        let running: Vec<&(String, String)> = jobs
            .iter()
            .filter(|(_, cmds)| cmds.contains(channel.probe))
            .collect();
        assert!(
            !running.is_empty(),
            "no job's `run:` steps contain `{}`, so nothing obtains the {} channel",
            channel.probe,
            channel.marker
        );
        for (name, cmds) in running {
            assert!(
                cmds.contains(channel.version_probe),
                "job `{name}` obtains the `{}` channel and never checks `{}` — it proves \
                 something was installed and not that it was the release",
                channel.marker,
                channel.version_probe
            );
        }
    }
}

/// The check must run without anyone remembering to run it, and must run when it lands.
///
/// `publish-crates.yml` was committed after the only tag this repository had ever cut. A
/// workflow fires only if it exists at the ref being pushed, so it never ran once — the
/// exact silence a weekly job added today would inherit.
#[test]
fn the_channel_check_runs_on_a_schedule_and_when_it_changes() {
    let workflow = read(".github/workflows/install-channels.yml");
    assert!(
        workflow.contains("schedule:") && workflow.contains("cron:"),
        "install-channels.yml must run on a schedule; a channel breaks between releases, \
         not during one"
    );
    assert!(
        workflow.contains(".github/workflows/install-channels.yml"),
        "install-channels.yml must trigger on pushes that touch itself, so it runs at least \
         once at the commit that adds it"
    );
    // ...and on main only. A tag push starts this workflow too, four seconds before the
    // release it is about to grade has published anything: the `cli/v0.2.1` push ran it
    // against `cli/v0.2.0` and produced four red jobs that meant nothing. A check whose
    // failures are sometimes real and sometimes timing is one people learn to shrug at.
    let push = workflow
        .split("  push:")
        .nth(1)
        .expect("install-channels.yml has a push trigger")
        .split("\njobs:")
        .next()
        .unwrap();
    assert!(
        push.contains("branches: [main]") || push.contains("branches:\n      - main"),
        "install-channels.yml's push trigger is not limited to main, so a tag push races the \
         release it is meant to check"
    );
}

/// A publish that published nothing must not be green.
///
/// `CARGO_REGISTRY_TOKEN` has never been configured. With the earlier warn-and-exit-0, the
/// first tag to reach that workflow would have run green and uploaded nothing, while the
/// README went on telling people to run `cargo binstall yidam`.
#[test]
fn a_publish_without_a_token_fails_rather_than_passing() {
    let workflow = read(".github/workflows/publish-crates.yml");
    let guard = workflow
        .split("CARGO_REGISTRY_TOKEN:-")
        .nth(1)
        .expect("publish-crates.yml must guard on an unset CARGO_REGISTRY_TOKEN");
    let body: String = guard.lines().take(6).collect::<Vec<_>>().join("\n");
    assert!(
        body.contains("exit 1"),
        "publish-crates.yml must fail when no registry token is configured, not exit 0:\n{body}"
    );
    assert!(
        !body.contains("exit 0"),
        "publish-crates.yml still exits 0 on a missing token — a channel that did not ship \
         must not look like one that did:\n{body}"
    );
}

/// The Linux tarballs must be linked against a glibc old enough to be worth serving.
///
/// A glibc binary runs on the version it was linked against or newer, and never on older.
/// `ubuntu-latest` moved to 24.04 / glibc 2.39, so `cli/v0.2.0`'s Linux tarball downloaded,
/// checksummed and installed on Debian 12 and then would not start. Every step of the
/// install reported success; only running it failed.
///
/// The runner image *is* the floor — nothing else in the repository declares one — so a diff
/// putting `ubuntu-latest` back would raise it silently.
#[test]
fn the_linux_release_builds_do_not_float_to_the_newest_glibc() {
    let text = read(".github/workflows/release.yml");
    let mut linux_targets = 0;
    let mut os = None;
    for line in text.lines().map(str::trim) {
        if let Some(t) = line.strip_prefix("- target: ") {
            os = None;
            if t.contains("linux-gnu") {
                linux_targets += 1;
                os = Some(t.to_string());
            }
        } else if let (Some(target), Some(runner)) = (&os, line.strip_prefix("os: ")) {
            assert_ne!(
                runner, "ubuntu-latest",
                "{target} builds on ubuntu-latest, whose glibc rises without a diff; pin the \
                 runner so the binary's minimum glibc is a decision and not a default"
            );
        }
    }
    assert!(
        linux_targets > 0,
        "release.yml builds no *-linux-gnu target; this test is guarding nothing"
    );
}

/// The cross-compile CI runs must be the one the release runs.
///
/// `release.yml` builds `aarch64-unknown-linux-gnu` and fires on `cli/v*` and
/// workflow_dispatch only, so no pull request had ever compiled that target — and `tonpa`
/// joining the default set put a TLS stack, and `ring`'s vendored C, into the light build
/// after the only release that exists. `ci.yml`'s cross job exists to compile it on every
/// PR instead of on the tag.
///
/// It is worth exactly as much as its resemblance to the real thing. A cross job that stops
/// passing `--locked`, or names a different linker, is a green check for a build nobody
/// ships — which is the same shape as a documented install path nobody runs.
#[test]
fn the_cross_compile_check_mirrors_the_release_build() {
    let ci = read(".github/workflows/ci.yml");
    let release = read(".github/workflows/release.yml");

    let cross = ci
        .split("  cross:")
        .nth(1)
        .expect(
            "ci.yml must have a cross-compile job; release.yml's aarch64 target is the \
                 one nothing else builds",
        )
        .split("\n  cli-full:")
        .next()
        .unwrap();

    for setting in [
        "aarch64-unknown-linux-gnu",
        "CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER",
        "CC_aarch64_unknown_linux_gnu",
        "aarch64-linux-gnu-gcc",
        "gcc-aarch64-linux-gnu",
        "--locked",
    ] {
        assert!(
            cross.contains(setting),
            "ci.yml's cross job does not set `{setting}`, which release.yml's build for this \
             target does — the rehearsal has to be the performance"
        );
        assert!(
            release.contains(setting),
            "release.yml no longer sets `{setting}`; ci.yml's cross job is now rehearsing a \
             build that does not happen"
        );
    }

    // No `--features`: the default IS the light set, and spelling it in either place makes
    // a second definition for the two to drift apart on. Read from the commands rather than
    // the whole block, since the comments above them discuss the flag by name.
    let commanded: String = cross
        .lines()
        .filter(|l| !l.trim_start().starts_with('#'))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        !commanded.contains("--features"),
        "ci.yml's cross job names features explicitly; the release build does not, so the \
         two would be compiling different things"
    );
    assert!(
        cross.contains("ARM aarch64"),
        "ci.yml's cross job must assert the artifact IS aarch64 — `it compiled` and `it \
         compiled for aarch64` are different claims, and cc-rs falling back to the host \
         compiler is what makes them differ"
    );
}

/// Every tag the docs tell people to install must be the version this repository is at.
///
/// `cargo install --git … --tag cli/vX --locked` is the one documented path that pins a
/// specific release, which is what makes it reproducible and also what makes it go stale:
/// nothing about bumping a version forces the line to move, and a reader who follows it
/// installs whatever was current when it was last edited.
///
/// The `.mcpb` bundle is the second such pin and arrived later (#421). Its asset name
/// *contains* the version — `yidam-0.9.0-aarch64-apple-darwin.mcpb` — so a documented line
/// naming one is a filename that 404s the day after a release, handed to the one audience
/// this project has that cannot diagnose it. Scanned here rather than in a test of its own,
/// because "a version written into prose" is one hazard with two instances, and the loop
/// that only knew about the first is how the second went uncovered while this test was green.
///
/// The other channels resolve "latest" at install time and cannot drift this way. These buy
/// reproducibility with a version in prose, and prose is the thing that rots.
///
/// Every document, not just the README. `release.sh` rewrites no prose at all — both pins
/// this repository carries moved by hand in the release commit, and only one of them had a
/// test standing behind it. The unguarded one is in `docs/mcp-server.md`, which is the page
/// someone reads when they are wiring up an MCP server and least able to notice that the
/// version they were handed is a release old.
#[test]
fn every_pinned_tag_is_the_version_this_repository_declares() {
    let manifest = read("yidam/cli/Cargo.toml");
    let declared = manifest
        .lines()
        .find_map(|l| l.strip_prefix("version = \""))
        .and_then(|v| v.split('"').next())
        .expect("yidam/cli/Cargo.toml declares a version");

    let mut pinned = 0;
    for doc in documented_files() {
        let text = read(&doc);
        for line in text.lines().filter(|l| l.contains("--tag cli/v")) {
            assert!(
                line.contains(&format!("--tag cli/v{declared} ")),
                "{doc} pins a different release than yidam/cli/Cargo.toml declares \
                 ({declared}):\n  {}\nBumping the version has to move this line; nothing \
                 else will.",
                line.trim()
            );
            pinned += 1;
        }
        for line in text.lines().filter(|l| l.contains(".mcpb")) {
            // Only lines naming a release asset. `.mcpb` is also written about as a format
            // — "an `.mcpb` is a zip holding a manifest" — and prose about the format
            // carries no version to be wrong.
            if !line.contains("yidam-") {
                continue;
            }
            assert!(
                line.contains(&format!("yidam-{declared}-")),
                "{doc} names a bundle from a different release than yidam/cli/Cargo.toml \
                 declares ({declared}):\n  {}\nThat filename is a 404 for the one channel \
                 whose users cannot diagnose it.",
                line.trim()
            );
            pinned += 1;
        }
    }
    assert!(
        pinned > 0,
        "no document pins a `cargo install --git … --tag cli/v…` line or an `.mcpb` asset \
         name; if those channels were removed, remove this test with them"
    );
}

/// A job running in a container must not use bash-only shell options.
///
/// GitHub runs `run:` steps under the *container's* `/bin/sh`, which on a slim Debian image
/// is dash. `set -euo pipefail` — correct everywhere else in this repository — exits 2 there
/// on the option itself, before the step does anything.
///
/// This survived a full run undetected, because on that run `install.sh` failed on glibc
/// before the assertion step was reached. The release that fixed glibc is the one that
/// exposed it: a check's own failure modes only become reachable once the thing it checks
/// stops failing first, which is an argument for testing the checks.
#[test]
fn steps_inside_a_container_avoid_bash_only_shell_options() {
    let workflow = read(".github/workflows/install-channels.yml");

    // Job boundaries are lines indented exactly two spaces and ending in a colon. Splitting
    // on "\n  " instead fragments at every deeper-indented line, which silently produced a
    // test that passed on the very regression it was written for.
    let mut jobs: Vec<String> = Vec::new();
    for line in workflow.lines() {
        let is_job_header = line.starts_with("  ")
            && !line.starts_with("   ")
            && line.trim_end().ends_with(':')
            && !line.trim_start().starts_with('#');
        if is_job_header {
            jobs.push(String::new());
        }
        if let Some(current) = jobs.last_mut() {
            current.push_str(line);
            current.push('\n');
        }
    }
    let containerised: Vec<&String> = jobs.iter().filter(|j| j.contains("container:")).collect();
    assert!(
        !containerised.is_empty(),
        "no job in install-channels.yml runs in a container; the clean-container install \
         check is the one that tests install.sh on a machine that has never seen this project"
    );
    for job in containerised {
        // Commands only. The comment above the fixed step names the option it avoids, and a
        // test that reads prose fails on the explanation of its own fix.
        let job: String = job
            .lines()
            .filter(|l| !l.trim_start().starts_with('#'))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            !job.contains("pipefail"),
            "a job in install-channels.yml runs in a container and uses `pipefail`; the \
             container's /bin/sh is dash, which exits 2 on that option:\n{job}"
        );
    }
}

/// Nothing resolves a layer's release through the repository's latest release.
///
/// `releases/latest` answers "what did this repository publish most recently", and this
/// repository publishes four layers onto one release list. Every caller here wants a
/// different question — which `cli/v*` is newest — and each one asked the repository-wide
/// one and then rejected the answer if it was not a CLI release.
///
/// That was correct for as long as the CLI was the only layer producing GitHub releases. It
/// broke the first time two trains landed together: `editor/v0.1.0` was pushed nine seconds
/// after `cli/v0.4.0`, and in those nine seconds it became the answer to all three.
///
/// - `tap.yml` refused to push the formula, and refused the dispatch built to repair that.
/// - `install-channels.yml` would have failed `released`, taking five channel jobs with it.
/// - `install.sh` — the `curl | sh` line the README documents — failed for every user.
///
/// One line each, written months apart, none of them wrong when written. So this is
/// discovered rather than listed, for the reason `documented_files` gives one test up: a
/// list of three files is the same hole one file later, and `editor-released` in
/// install-channels.yml had already been written the right way without the older jobs
/// learning anything from it.
#[test]
fn nothing_resolves_a_layer_release_through_the_repository_latest() {
    let root = repo_root().canonicalize().expect("repo root is readable");
    let mut scanned: Vec<String> = Vec::new();

    let mut candidates: Vec<PathBuf> = std::fs::read_dir(root.join(".github/workflows"))
        .expect(".github/workflows is unreadable")
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| {
            let n = p.to_string_lossy();
            n.ends_with(".yml") || n.ends_with(".yaml")
        })
        .collect();
    candidates.push(root.join("install.sh"));
    candidates.sort();

    for path in &candidates {
        let text = std::fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("{} is unreadable ({e})", path.display()));
        let rel = path
            .strip_prefix(&root)
            .unwrap_or(path)
            .to_string_lossy()
            .into_owned();
        // Commands only: the fixes explain the endpoint they avoid, and a test that reads
        // prose fails on the explanation of its own fix.
        let code: String = text
            .lines()
            .filter(|l| !l.trim_start().starts_with('#'))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            !code.contains("releases/latest"),
            "{rel} resolves a release through `releases/latest`, which is repository-wide. \
             Four layers publish onto one list, so the answer is whichever layer was tagged \
             last — an `editor/v*` tag then decides which CLI is installed, checked, or \
             served. Filter the release list by the layer's own tag prefix instead."
        );
        scanned.push(rel);
    }

    // The walk must actually have looked at the three files this is about; a scan of
    // nothing passes every assertion in it.
    for required in ["install.sh", ".github/workflows/tap.yml"] {
        assert!(
            scanned.iter().any(|s| s == required),
            "the scan did not read {required}, so it is looking at the wrong tree and every \
             assertion built on it is vacuous (read: {scanned:?})"
        );
    }
    assert!(
        scanned.len() >= 5,
        "the scan read only {} files; this repository has more workflows than that, so it \
         is not reading what it claims to (read: {scanned:?})",
        scanned.len()
    );
}

/// A verification step that verifies nothing must not read as if it succeeded.
///
/// mise prints two lines on every install of this binary:
///
/// ```text
/// mise github:goedelsoup/yidam@0.6.0 [2/3] verify GitHub artifact attestations
/// mise github:goedelsoup/yidam@0.6.0 [2/3] verify SLSA provenance
/// ```
///
/// and `release.yml` declared `contents: write`, had no attest step anywhere, and so gave
/// both lines nothing to find. They passed over an absence and printed what they would print
/// for a signed artifact — a third party's verification, phrased as a success, shown to the
/// user during install.
///
/// The two states *are* distinguishable, which is the uncomfortable part. Installing a
/// repository that does publish attestations adds a line this one never produced —
/// `[2/3] ✓ GitHub artifact attestations verified` — so the only thing separating "verified"
/// from "found nothing to verify" was a line that was not there, and nobody reads for those.
/// Verified against mise 2026.7.0 by installing `github:cli/cli` beside this one.
///
/// The permissions are asserted with the step because either alone attests nothing. Missing
/// permissions fail the step at tag time rather than silently, which is the better failure —
/// but naming them here is cheaper than reading an OIDC error during a release.
///
/// The `.sha256` files are deliberately not subjects. They are checksums *of* the tarballs,
/// so attesting the tarball covers what they assert; they go on answering integrity for
/// `install.sh`, the tap and `[yidam-build]`, and this answers origin.
#[test]
fn the_release_publishes_build_provenance_for_what_it_ships() {
    let release = read(".github/workflows/release.yml");

    // The same reading `the_cross_compile_check_mirrors_the_release_build` takes of ci.yml:
    // job boundaries by name, then assert the slice is the job it claims to be.
    let publish = release
        .split("\n  publish:")
        .nth(1)
        .expect("release.yml has a publish job")
        .split("\n  tap:")
        .next()
        .unwrap();
    // Comments stripped, for the reason two tests above give and one below learned the hard
    // way: the comment explaining why `contents: write` is repeated here *contains*
    // `contents: write`, so deleting the permission left this test green. Verified by
    // deleting it. Trailing comments survive — it is whole prose lines that answer for code.
    let publish: String = publish
        .lines()
        .filter(|l| !l.trim_start().starts_with('#'))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        publish.contains("gh release create"),
        "the slice taken for release.yml's publish job does not create a release, so it is \
         not that job and every assertion below it is vacuous"
    );

    assert!(
        publish.contains("actions/attest-build-provenance"),
        "release.yml publishes tarballs and attests nothing. mise announces \"verify GitHub \
         artifact attestations\" on every install of this binary and finds nothing there, \
         which reads to a user exactly as verification passing."
    );
    // Scoped to the attest step's own subject list rather than matched against the whole
    // job, because `gh release create` names the same glob and would answer for this. The
    // spelling of the list is not pinned — it became a block scalar when the `.mcpb` bundles
    // joined it (#421), and a test that fails on YAML style rather than on coverage is one
    // that gets edited without being read.
    let subjects = publish
        .split("subject-path:")
        .nth(1)
        .expect("the attestation step names no subjects")
        .split("\n      - ")
        .next()
        .unwrap();
    assert!(
        subjects.contains("dist/*.tar.gz"),
        "the attestation does not cover `dist/*.tar.gz` — the assets every channel actually \
         downloads. An attestation over something else is the absence with extra steps. \
         Subjects: {subjects:?}"
    );
    for permission in ["id-token: write", "attestations: write"] {
        assert!(
            publish.contains(permission),
            "release.yml's publish job attests build provenance without granting \
             `{permission}`; the step cannot mint or store an attestation without it"
        );
    }
    // `contents: write` is workflow-level, and a job that declares `permissions` REPLACES
    // the workflow's rather than adding to it — so scoping the two above to this job silently
    // removed what `gh release create` needs unless it is repeated here.
    assert!(
        publish.contains("contents: write"),
        "release.yml's publish job declares `permissions` but not `contents: write`; a \
         job-level block replaces the workflow-level one, so `gh release create` would be \
         publishing without permission to write the release"
    );
}

/// The notes for a release must start from the previous release **of its own layer**.
///
/// `nothing_resolves_a_layer_release_through_the_repository_latest` is the same finding at a
/// different call site, and this is the fourth. `releases/generate-notes` picks a base when
/// `previous_tag_name` is omitted, and on `cli/v0.9.0` it picked `cli/v0.7.0` — past
/// `cli/v0.8.0`, which exists, is a release, is an annotated tag on `main`, and is an ancestor
/// of the tag being cut. The published notes listed 37 pull requests where 16 belonged to the
/// release; everything from #497 on had already shipped (#555).
///
/// It is a content defect rather than a broken link, and the least alarming shape one can
/// take: a changelog that is too long reads like a busy release.
#[test]
fn the_release_notes_start_from_this_layers_previous_release() {
    let workflow = read(".github/workflows/release.yml");
    let commands = workflow
        .lines()
        .filter(|l| !l.trim_start().starts_with('#'))
        .collect::<Vec<_>>()
        .join("\n");

    assert!(
        commands.contains("previous_tag_name"),
        "release.yml generates notes without `previous_tag_name`, so GitHub chooses the base. \
         It chose one release too far back on cli/v0.9.0 and repeated a whole release's \
         changelog."
    );
    assert!(
        commands.contains("previous-release-tag.sh"),
        "release.yml must resolve the previous tag per layer. A hardcoded one is a second \
         place the version lives, and omitting it is the defect above."
    );
}

/// The resolution itself, run against fixtures rather than the network.
///
/// The workflow's copy is exercised by cutting a release, which is a poor place to learn that
/// a shell expansion was wrong. This drives the same script the workflow calls.
///
/// The fixture interleaves layers in *creation* order the way this repository's really does,
/// because that ordering is what the first version of the script got wrong: it took the newest
/// release of the layer that was not the tag itself, which is correct only while the tag being
/// cut is the newest one. Asked about a historical tag it answered with a *later* release.
/// That had no reachable symptom in the release flow — it only ever cuts the newest — and
/// would have grown one the first time somebody re-cut an old tag.
#[test]
fn the_previous_release_resolution_answers_per_layer() {
    use std::io::Write;
    use std::process::{Command, Stdio};

    let script = repo_root().join(".github/scripts/previous-release-tag.sh");
    assert!(script.is_file(), "{} is missing", script.display());

    // Newest-first, as the releases API returns them, and deliberately not in version order.
    const RELEASES: &str = r#"[
      {"tag_name": "cli/v0.10.0"},
      {"tag_name": "v0.3.0"},
      {"tag_name": "cli/v0.9.0"},
      {"tag_name": "sdk/rust/v0.4.0"},
      {"tag_name": "cli/v0.8.0"},
      {"tag_name": "editor/v0.2.0"},
      {"tag_name": "editor/v0.1.0"}
    ]"#;

    let cases: &[(&str, &str)] = &[
        // `0.10.0` sorts above `0.9.0` by version and below it as a string.
        ("cli/v0.10.0", "cli/v0.9.0"),
        // A tag that is not the newest release of its layer: the answer is the one below it,
        // never the one above.
        ("cli/v0.9.0", "cli/v0.8.0"),
        // The layer's earliest release has no predecessor, and "" is the answer rather than
        // an error — the caller omits `previous_tag_name` on it.
        ("cli/v0.8.0", ""),
        ("editor/v0.2.0", "editor/v0.1.0"),
        ("editor/v0.1.0", ""),
        // Layers with exactly one release, and a tag whose prefix is a prefix of nothing else.
        ("sdk/rust/v0.4.0", ""),
        ("v0.3.0", ""),
        // A tag with no release yet — the ordinary case, since notes are generated before the
        // release is created.
        ("cli/v0.11.0", "cli/v0.10.0"),
    ];

    for (tag, expected) in cases {
        let mut child = Command::new(&script)
            .arg(tag)
            .current_dir(repo_root())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("the resolution script runs");
        child
            .stdin
            .take()
            .expect("stdin is piped")
            .write_all(RELEASES.as_bytes())
            .expect("the fixture is written");
        let out = child.wait_with_output().expect("the script terminates");
        assert!(
            out.status.success(),
            "resolving {tag} failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        let got = String::from_utf8_lossy(&out.stdout).trim().to_string();
        assert_eq!(
            &got, expected,
            "previous release of {tag}: expected {expected:?}, got {got:?}"
        );
    }
}
