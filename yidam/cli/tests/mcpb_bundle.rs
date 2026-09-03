//! The `.mcpb` bundle — the one channel that does not begin in a terminal (#421).
//!
//! Every other route to a corpus ends with a binary on a `PATH` and an `mcpServers` block to
//! hand-write, and `docs/mcp-server.md` then spends a subsection on the `sh -c 'cd … && exec
//! …'` form needed to say *which* corpus you meant. An `.mcpb` is a zip holding a manifest
//! and that same binary, installed by dragging it onto Claude Desktop, and its manifest turns
//! that shell incantation into a directory the installer asks for.
//!
//! Which makes this file's central case an end-to-end one rather than a schema check. A
//! manifest that parses and names a flag the binary does not have is a green build and a
//! stranger double-clicking a file that does nothing — the exact failure mode
//! `install-channels.yml` exists for, one channel later. So
//! [`the_bundle_serves_the_corpus_its_manifest_names`] performs the substitution Claude
//! Desktop performs, from a working directory that is not the corpus, and makes the server
//! say which corpus it loaded.

use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn renderer() -> PathBuf {
    repo_root().join("render-manifest.sh")
}

fn render(version: &str, platform: &str) -> std::process::Output {
    Command::new(renderer())
        .args([version, platform])
        .current_dir(repo_root())
        .output()
        .expect("running render-manifest.sh")
}

fn manifest(version: &str, platform: &str) -> Value {
    let out = render(version, platform);
    assert!(
        out.status.success(),
        "render-manifest.sh {version} {platform} failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    serde_json::from_slice(&out.stdout).unwrap_or_else(|e| {
        panic!(
            "render-manifest.sh emitted invalid JSON ({e}):\n{}",
            String::from_utf8_lossy(&out.stdout)
        )
    })
}

fn release_workflow() -> serde_yaml::Value {
    let text = std::fs::read_to_string(repo_root().join(".github/workflows/release.yml"))
        .expect("reading release.yml");
    serde_yaml::from_str(&text).expect("release.yml must parse as YAML")
}

/// The targets the release builds, paired with the MCPB platform each is bundled for.
fn release_targets() -> Vec<(String, Option<String>)> {
    let wf = release_workflow();
    let include = wf["jobs"]["build"]["strategy"]["matrix"]["include"]
        .as_sequence()
        .expect("release.yml has a build matrix")
        .clone();
    assert!(
        !include.is_empty(),
        "the build matrix is empty; this test reads nothing"
    );
    include
        .iter()
        .map(|e| {
            (
                e["target"]
                    .as_str()
                    .expect("a matrix entry with no target")
                    .to_string(),
                e["mcpb_platform"].as_str().map(str::to_string),
            )
        })
        .collect()
}

// ── the manifest ────────────────────────────────────────────────────────────

/// The fields MCPB declares required. A bundle missing one does not install, and the only
/// place that shows up otherwise is a person's Extensions pane.
#[test]
fn the_manifest_carries_every_field_mcpb_requires() {
    let m = manifest("1.2.3", "darwin");
    for key in [
        "manifest_version",
        "name",
        "version",
        "description",
        "author",
        "server",
    ] {
        assert!(
            m.get(key).is_some(),
            "manifest has no {key:?}, which MCPB requires"
        );
    }
    assert!(
        m["author"]["name"].is_string(),
        "`author` must carry a `name`; MCPB requires it even though `author` itself is an object"
    );
    assert_eq!(
        m["server"]["type"], "binary",
        "the bundle carries a compiled yidam, not a runtime and a script"
    );
    assert!(
        m["server"]["entry_point"].is_string(),
        "a binary server must name its entry_point"
    );
}

#[test]
fn the_rendered_version_is_the_one_asked_for() {
    assert_eq!(manifest("1.2.3", "darwin")["version"], "1.2.3");
    assert_eq!(manifest("0.9.0", "darwin")["version"], "0.9.0");
}

/// The corpus is a question the installer asks, which is the whole repair. A bundle whose
/// manifest declares no `directory` config is one that inherits a working directory again.
#[test]
fn the_corpus_is_a_directory_the_installer_asks_for() {
    let m = manifest("1.2.3", "darwin");
    let corpus = &m["user_config"]["corpus"];
    assert_eq!(
        corpus["type"], "directory",
        "the corpus must be a directory picker"
    );
    assert_eq!(
        corpus["required"], true,
        "an optional corpus is the load-bearing working directory with extra steps: unset, \
         the server falls back to wherever Claude Desktop happened to start it"
    );
    let args = m["server"]["mcp_config"]["args"]
        .as_array()
        .expect("mcp_config.args");
    assert!(
        args.iter().any(|a| a == "${user_config.corpus}"),
        "the args never substitute the directory the installer asked for: {args:?}"
    );
}

/// Claude Desktop runs on macOS and Windows. `linux` is not a platform a bundle can name, and
/// the failure of naming it is invisible — the manifest is valid JSON and installs nowhere.
#[test]
fn a_platform_claude_desktop_does_not_run_on_is_refused() {
    let out = render("1.2.3", "linux");
    assert!(
        !out.status.success(),
        "render-manifest.sh accepted `linux`; Claude Desktop does not run there, so the \
         bundle would install nowhere and nothing would say so"
    );
    for platform in ["darwin", "win32"] {
        assert!(
            render("1.2.3", platform).status.success(),
            "render-manifest.sh refused {platform:?}, which Claude Desktop does run on"
        );
    }
}

// ── the release ─────────────────────────────────────────────────────────────

/// Every bundled target names a platform the manifest can carry, and no target names one it
/// cannot. Read from the matrix rather than re-derived, so adding a target cannot quietly
/// bundle it for a platform that does not exist.
#[test]
fn every_bundled_target_names_a_platform_a_bundle_can_install_on() {
    let targets = release_targets();
    let bundled: Vec<_> = targets.iter().filter(|(_, p)| p.is_some()).collect();
    assert!(
        !bundled.is_empty(),
        "no target in the release matrix is bundled, so `cli/v*` publishes no .mcpb and \
         #421's channel does not exist"
    );
    for (target, platform) in &targets {
        let Some(platform) = platform else { continue };
        assert!(
            render("1.2.3", platform).status.success(),
            "{target} is bundled for platform {platform:?}, which render-manifest.sh refuses"
        );
        assert!(
            !target.contains("linux"),
            "{target} is bundled, and Claude Desktop does not run on Linux"
        );
    }
}

/// A bundle that is built and not uploaded is a release without the asset. The tarball
/// patterns cannot speak for it — `if-no-files-found: error` is satisfied by them alone.
#[test]
fn the_release_uploads_the_bundle_it_builds() {
    let wf = release_workflow();
    let steps = wf["jobs"]["build"]["steps"].as_sequence().unwrap();
    let upload = steps
        .iter()
        .find(|s| {
            s["uses"]
                .as_str()
                .is_some_and(|u| u.starts_with("actions/upload-artifact"))
        })
        .expect("the build job uploads its artifacts");
    let paths = upload["with"]["path"]
        .as_str()
        .expect("upload has a path list");
    for want in [".mcpb", ".mcpb.sha256"] {
        assert!(
            paths.contains(want),
            "the build job packages an .mcpb and uploads {paths:?}, which does not carry \
             {want} — so `publish` never sees it"
        );
    }
}

/// Uploaded is not published. `publish` names the assets it attaches one glob at a time —
/// `dist/*.tar.gz dist/*.sha256 dist/SHA256SUMS` — and the bundle matched none of them, so
/// the whole packaging step above would have run green on every release and shipped nothing.
/// It is also a subject of the provenance attestation, on the tarballs' own grounds: it
/// carries a binary, and it is the one channel installed by a person dragging a downloaded
/// file onto an application.
#[test]
fn the_release_publishes_and_attests_the_bundle() {
    let wf = release_workflow();
    let steps = wf["jobs"]["publish"]["steps"].as_sequence().unwrap();

    let create = steps
        .iter()
        .filter_map(|s| s["run"].as_str())
        .find(|r| r.contains("gh release create"))
        .expect("the publish job creates a release");
    assert!(
        create.contains("dist/*.mcpb"),
        "`gh release create` attaches no .mcpb, so the bundle is built, uploaded and never \
         published:\n{create}"
    );

    let attest = steps
        .iter()
        .find(|s| {
            s["uses"]
                .as_str()
                .is_some_and(|u| u.starts_with("actions/attest-build-provenance"))
        })
        .expect("the publish job attests its artifacts");
    let subjects = attest["with"]["subject-path"].as_str().unwrap_or_default();
    assert!(
        subjects.contains("*.mcpb"),
        "the bundle carries a binary and is not a provenance subject: {subjects:?}"
    );
}

// ── the bundle, run the way Claude Desktop runs it ──────────────────────────

/// `${__dirname}` and `${user_config.KEY}`, substituted as the MCPB spec defines them.
fn substitute(raw: &str, dirname: &Path, corpus: &Path) -> String {
    raw.replace("${__dirname}", &dirname.display().to_string())
        .replace("${user_config.corpus}", &corpus.display().to_string())
}

fn stage_corpus(into: &Path) {
    let from = repo_root().join("yidam/prelude/sdks/parity/mcp/corpus");
    assert!(from.is_dir(), "no fixture corpus at {}", from.display());
    for entry in walkdir::WalkDir::new(&from)
        .into_iter()
        .filter_map(Result::ok)
    {
        let rel = entry.path().strip_prefix(&from).unwrap();
        if rel.as_os_str().is_empty() {
            continue;
        }
        let to = into.join(rel);
        if entry.file_type().is_dir() {
            std::fs::create_dir_all(&to).unwrap();
        } else {
            std::fs::create_dir_all(to.parent().unwrap()).unwrap();
            std::fs::copy(entry.path(), &to).unwrap();
        }
    }
    let git = |args: &[&str]| {
        let ok = Command::new("git")
            .current_dir(into)
            .args(args)
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        assert!(ok, "git {args:?} failed staging the fixture corpus");
    };
    git(&["init", "-q", "-b", "main"]);
    git(&["config", "user.email", "t@t.co"]);
    git(&["config", "user.name", "Test"]);
    git(&["add", "."]);
    git(&["commit", "-q", "-m", "chore: genesis — mcpb fixture"]);
}

/// The case this file exists for.
///
/// Lay the bundle out as the archive does, substitute the manifest's own `command` and `args`
/// the way the client does, and run it **from a directory that is not the corpus** — which is
/// the condition the whole bundle exists to survive, and the one a test run from inside the
/// repository would never exercise.
#[test]
fn the_bundle_serves_the_corpus_its_manifest_names() {
    let m = manifest("1.2.3", "darwin");

    let bundle = tempfile::TempDir::new().unwrap();
    let corpus = tempfile::TempDir::new().unwrap();
    let elsewhere = tempfile::TempDir::new().unwrap();
    stage_corpus(corpus.path());

    // `entry_point` is where the archive puts the binary; the packaging step copies it there
    // and this must agree with it, or the manifest describes a layout no bundle has.
    let entry = m["server"]["entry_point"].as_str().unwrap();
    let placed = bundle.path().join(entry);
    std::fs::create_dir_all(placed.parent().unwrap()).unwrap();
    std::fs::copy(env!("CARGO_BIN_EXE_yidam"), &placed).unwrap();

    let cfg = &m["server"]["mcp_config"];
    let command = substitute(
        cfg["command"].as_str().expect("mcp_config.command"),
        bundle.path(),
        corpus.path(),
    );
    assert_eq!(
        Path::new(&command),
        placed,
        "`command` and `entry_point` name different files, so the bundle ships the binary \
         somewhere the manifest does not launch it from"
    );
    let args: Vec<String> = cfg["args"]
        .as_array()
        .expect("mcp_config.args")
        .iter()
        .map(|a| {
            substitute(
                a.as_str().expect("args must be strings"),
                bundle.path(),
                corpus.path(),
            )
        })
        .collect();

    let mut child = Command::new(&command)
        .args(&args)
        .current_dir(elsewhere.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|e| {
            panic!("spawning the bundle's own command line {command} {args:?}: {e}")
        });

    use std::io::{BufRead, BufReader, Write};
    {
        let stdin = child.stdin.as_mut().unwrap();
        writeln!(
            stdin,
            r#"{{"jsonrpc":"2.0","id":1,"method":"initialize","params":{{}}}}"#
        )
        .unwrap();
        stdin.flush().unwrap();
    }
    let mut line = String::new();
    BufReader::new(child.stdout.as_mut().unwrap())
        .read_line(&mut line)
        .unwrap();
    let _ = child.kill();
    let _ = child.wait();

    assert!(
        !line.trim().is_empty(),
        "the bundle's command line produced no handshake. Run from {}, it is the \
         working-directory failure the manifest's directory picker is supposed to end.",
        elsewhere.path().display()
    );
    let resp: Value = serde_json::from_str(&line)
        .unwrap_or_else(|e| panic!("handshake was not JSON ({e}): {line}"));
    let served = &resp["result"]["capabilities"]["yidam"]["corpus"];
    assert!(
        served["nodes"].as_u64().is_some_and(|n| n > 0),
        "the bundle served an empty corpus from {}: {served}. That is what a server \
         answering from the wrong directory looks like.",
        corpus.path().display()
    );
}
