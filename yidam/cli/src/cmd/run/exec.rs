//! Materializing a step's inputs, invoking it, and collecting what it produced.
//!
//! # The step never stands in the working tree
//!
//! This is what makes *the working tree is untouched* a structural property rather than a
//! promise. The declared `reads` are checked out of `HEAD` into a scratch directory with
//! `git checkout-index` against a temporary index, the process runs **there**, and it writes
//! into a second scratch directory it is handed by name. Nothing it does can reach the
//! checkout, because it is never pointed at one.
//!
//! Two things follow that were worth having anyway. A run's input state is a commit rather
//! than "whatever was on disk", which is the equality check RFC-0026 §1 asks for. And
//! `reads` becomes load-bearing on the same terms as `writes`: a step given only what it
//! declared cannot quietly depend on a file it did not name, so the declaration is checked by
//! the run rather than by review.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};

use anyhow::{bail, Context, Result};

use super::manifest::Capability;
use super::receipt::{sha256, File};
use crate::cmd::propose::write::{git, TempIndex};
use crate::kuten::glob_covers;

/// A scratch directory removed when the run ends, wherever it ends.
///
/// In the system temp directory and deliberately **not** inside `$GIT_DIR`, which is where
/// `TempIndex` puts its own file for a reason that does not transfer: an index must share a
/// filesystem with the object store, and a scratch tree a third-party process is pointed at
/// must not share a directory with it. The step's working directory is somewhere a misfiring
/// calculator can do no damage.
pub struct Scratch(PathBuf);

impl Scratch {
    fn new(what: &str) -> Result<Self> {
        // Named rather than random: a leftover after a crash is attributable, which is the
        // same argument `TempIndex` makes about its own name. Unique per process, because
        // two runs in one repository is an ordinary thing to do.
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "yidam-run-{what}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path)
            .with_context(|| format!("creating the scratch directory {}", path.display()))?;
        Ok(Self(path))
    }

    pub fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// What the step was given and where it stood while it ran.
pub struct Inputs {
    pub dir: Scratch,
    pub files: Vec<File>,
}

/// Check the declared `reads` out of `commit` into a scratch directory.
///
/// `git ls-files` against a temporary index seeded from the commit, filtered by the
/// declaration, then `git checkout-index` of exactly that set. Two plumbing calls and no
/// checkout: `.git/index` is never read or written, so this is safe mid-edit for the same
/// reason `propose` is.
pub fn materialize(root: &Path, commit: &str, cap: &Capability) -> Result<Inputs> {
    let scratch = TempIndex::new(root, "run")?;
    let index = scratch.path().to_path_buf();
    git(root, Some(&index), &["read-tree", commit], None)?;

    let listed = git(root, Some(&index), &["ls-files", "-z"], None)?;
    let mut wanted: Vec<String> = listed
        .split('\0')
        .filter(|p| !p.is_empty())
        .filter(|p| cap.reads.iter().any(|g| glob_covers(g, p)))
        .map(str::to_string)
        .collect();
    wanted.sort();

    if wanted.is_empty() {
        bail!(
            "nothing at {} matches this capability's `reads` ({}) — a step with no input \
             would compute over an empty tree and commit the result",
            &commit[..commit.len().min(12)],
            cap.reads.join(", ")
        );
    }

    let dir = Scratch::new("in")?;
    // `--prefix` wants a trailing separator, and the paths arrive on stdin so that a corpus
    // with more files than an argv can hold is not a different case.
    let prefix = format!("{}/", dir.path().display());
    git(
        root,
        Some(&index),
        &["checkout-index", "-f", "--stdin", "-z", "--prefix", &prefix],
        Some(&wanted.join("\0")),
    )?;

    let mut files = Vec::new();
    for path in wanted {
        let bytes = std::fs::read(dir.path().join(&path))
            .with_context(|| format!("reading back the materialized {path}"))?;
        files.push(File {
            sha256: sha256(&bytes),
            path,
        });
    }
    Ok(Inputs { dir, files })
}

/// What a step wrote, and what it said while doing it.
pub struct Produced {
    pub outputs: Vec<(String, Vec<u8>)>,
    pub stderr: String,
}

/// Invoke the capability, standing in its input tree, writing into a scratch output tree.
///
/// The contract is four environment variables and a working directory, and it is deliberately
/// small enough to implement in a shell script — the first calculator is one, because a
/// vertical slice that needed a build system to demonstrate would be demonstrating the build
/// system.
pub fn invoke(cap: &Capability, inputs: &Inputs, step: &str, commit: &str) -> Result<Produced> {
    let out = Scratch::new("out")?;
    let status = Command::new(&cap.run[0])
        .args(&cap.run[1..])
        .current_dir(inputs.dir.path())
        .env("YIDAM_IN", inputs.dir.path())
        .env("YIDAM_OUT", out.path())
        .env("YIDAM_STEP", step)
        .env("YIDAM_INPUT_COMMIT", commit)
        .stdin(Stdio::null())
        .output()
        .with_context(|| format!("invoking `{}`", cap.run.join(" ")))?;

    let stderr = String::from_utf8_lossy(&status.stderr).trim().to_string();
    if !status.status.success() {
        bail!(
            "`{}` exited {}{}",
            cap.run.join(" "),
            status
                .status
                .code()
                .map_or_else(|| "on a signal".to_string(), |c| c.to_string()),
            if stderr.is_empty() {
                String::new()
            } else {
                format!("\n{stderr}")
            }
        );
    }

    let mut outputs = BTreeMap::new();
    for entry in walkdir::WalkDir::new(out.path()).into_iter().flatten() {
        if !entry.file_type().is_file() {
            continue;
        }
        let rel = entry
            .path()
            .strip_prefix(out.path())
            .context("a file the step wrote is not under the directory it was given")?
            .to_string_lossy()
            .replace('\\', "/");
        let bytes = std::fs::read(entry.path())
            .with_context(|| format!("reading the step's output {rel}"))?;
        outputs.insert(rel, bytes);
    }
    Ok(Produced {
        outputs: outputs.into_iter().collect(),
        stderr,
    })
}

/// Refuse a step that wrote outside what it declared.
///
/// Fails closed and names both sides. RFC-0026 §4: `writes` *"is what lets the executor
/// refuse a step that wrote outside its declaration"* — a rule that only ever describes is
/// the surface-with-no-consumer shape one level down.
pub fn check_declared(cap: &Capability, produced: &[(String, Vec<u8>)]) -> Result<()> {
    let undeclared: Vec<&str> = produced
        .iter()
        .map(|(p, _)| p.as_str())
        .filter(|p| !cap.writes.iter().any(|g| glob_covers(g, p)))
        .collect();
    if undeclared.is_empty() {
        return Ok(());
    }
    bail!(
        "the step wrote outside its declaration and nothing was committed.\n  \
         wrote:    {}\n  declared: {}",
        undeclared.join(", "),
        cap.writes.join(", ")
    );
}
