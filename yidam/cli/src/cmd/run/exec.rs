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

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};

use anyhow::{bail, Context, Result};

use super::manifest::{Capability, Run};
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
    /// Where the resolved corpus was written, and its digest (#1080).
    ///
    /// **Its own scratch directory, and not a file inside [`Self::dir`].** The input tree
    /// holding *exactly* what `reads` resolves to is the property the whole arrangement
    /// exists for, and a file the run put there would be one the step was given and did not
    /// declare — visible to a `find`, indistinguishable from an input, and a thing the next
    /// reader of this module would have to be told about. The step reaches it by the name it
    /// is handed, `$YIDAM_GRAPH`, which is the same contract the other four variables are.
    ///
    /// `None` where the declared `reads` admit no corpus node: a connector against an
    /// external source is given no resolved corpus rather than an empty one, because an empty
    /// document and *a corpus with nothing in it* are different answers and a step should not
    /// have to tell them apart.
    pub resolved: Option<ResolvedCorpus>,
    /// The script a typed calculator applies, read out of the input tree (RFC-0042).
    ///
    /// Read here rather than at invocation, out of the same tree every other input came from. A
    /// typed calculator's program is an input like any other — [`super::manifest::validate_gluon`]
    /// refuses a declaration that does not say so — and reading it anywhere but the materialized
    /// tree would be the one file the step was handed that `reads` did not bound.
    ///
    /// No digest here: it is already in [`Self::files`], and
    /// [`super::receipt::Receipt::script_sha256`] is where it is taken from, once, for both the run
    /// and `doctor`.
    ///
    /// `None` for the shell arm, whose program is an argv the process resolves for itself.
    ///
    /// Behind the feature, unlike everything the *declaration* needs: the shape, the validation and
    /// the refusal are ungated because a light build reads manifests that declare this arm — see
    /// [`super::manifest::Run`] — but reading a program that build cannot evaluate is work with no
    /// consumer, and a field nothing reads is the shape a warning is right about.
    #[cfg(feature = "calculators-gluon")]
    pub script: Option<Script>,
}

/// A typed calculator's program, as the run read it.
#[cfg(feature = "calculators-gluon")]
pub struct Script {
    /// Repository-relative, as the manifest declared it.
    pub path: String,
    pub text: String,
}

/// The resolved corpus as it was handed over.
pub struct ResolvedCorpus {
    /// Kept so the directory outlives the step: dropping it removes the file. Underscored
    /// because nothing reads it and that is the point — it is a lifetime and not a value.
    _dir: Scratch,
    pub path: PathBuf,
    /// Recorded in the receipt, so a run states what it actually read and not only which
    /// bytes it read it from — see [`super::receipt::Input::resolved_graph_sha256`].
    pub sha256: String,
}

impl ResolvedCorpus {
    /// Build it from the materialized tree, checking existence against the whole commit.
    ///
    /// The nodes come from the scratch tree, so the document is bounded by `reads` without a
    /// second filter that could fall out of step with the first. Existence is asked of
    /// `tracked` — every path the commit holds — for the reason
    /// [`super::resolved::render`] gives: a link out of this step's view is not a broken
    /// edge, and answering it from the sliced tree would say it was.
    fn build(dir: &Scratch, tracked: &[&str]) -> Result<Option<Self>> {
        let read = crate::corpus::Corpus::open(dir.path());
        let held: BTreeSet<&str> = tracked.iter().copied().collect();
        // `keep` is everything: the tree this reads *is* the declaration resolved, so a second
        // filter here would be a second reading of `reads` that could fall out of step with
        // the `checkout-index` above.
        let Some(text) = super::resolved::build(&read, &|_| true, &|p| held.contains(p)) else {
            return Ok(None);
        };

        let out = Scratch::new("graph")?;
        let path = out.path().join("corpus.tsv");
        std::fs::write(&path, text.as_bytes())
            .with_context(|| format!("writing the resolved corpus to {}", path.display()))?;
        Ok(Some(Self {
            _dir: out,
            path,
            sha256: sha256(text.as_bytes()),
        }))
    }
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

    // A step's own implementation is a file it depends on, so it is a file it must declare.
    //
    // The process stands in the scratch tree, so a relative `run` argument resolves there and
    // nowhere else — which is the isolation the whole arrangement exists for, and which makes
    // an undeclared script a certain failure rather than a risk. Left to the invocation it
    // surfaces as `sh: no such file or directory` and exit 127, blaming the shell for a
    // declaration the manifest is missing.
    //
    // Decided by what is tracked rather than by what looks like a path: a token is checked
    // only when the commit itself holds a file by that name, so `cargo`, `-p` and `sh` are not
    // candidates and no rule about which arguments are paths has to be invented.
    let tracked: Vec<&str> = listed.split('\0').filter(|p| !p.is_empty()).collect();
    for arg in cap.run.file_candidates() {
        if tracked.contains(&arg.as_str()) && !wanted.iter().any(|w| w == arg) {
            bail!(
                "`{arg}` is in this repository and is not in this capability's `reads` ({}).\n  \
                 A step is invoked in a tree holding exactly what it declares, so it would \
                 stand in a directory that does not contain the file it was told to run.\n  \
                 Declare it: a step's own implementation is an input, and reading it there is \
                 also what makes editing it change the input state.",
                cap.reads.join(", ")
            );
        }
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
    let resolved = ResolvedCorpus::build(&dir, &tracked)?;
    #[cfg(feature = "calculators-gluon")]
    let script = cap
        .run
        .gluon()
        .map(|(path, _)| -> Result<Script> {
            let text = std::fs::read_to_string(dir.path().join(path)).with_context(|| {
                format!(
                    "reading the calculator script {path} out of the materialized tree — it is \
                     covered by this capability's `reads`, so it should be there"
                )
            })?;
            Ok(Script {
                path: path.to_string(),
                text,
            })
        })
        .transpose()?;
    Ok(Inputs {
        dir,
        files,
        resolved,
        #[cfg(feature = "calculators-gluon")]
        script,
    })
}

/// What a step wrote, and what it said while doing it.
pub struct Produced {
    pub outputs: Vec<(String, Vec<u8>)>,
    pub stderr: String,
}

/// Invoke the capability, by whichever arm it declared.
///
/// The two arms differ in what the step is handed and in nothing else the executor does: the same
/// input tree is materialized, the same `writes` declaration is enforced against what came back,
/// the same receipt is written. RFC-0042's claim is that the difference is *what the step could
/// have read*, and that is where it lives — a process is handed a directory and can read the whole
/// of it, a typed calculator is handed a value and there is nothing else.
pub fn invoke(cap: &Capability, inputs: &Inputs, step: &str, commit: &str) -> Result<Produced> {
    match &cap.run {
        Run::Argv(argv) => process(argv, inputs, step, commit),
        // The budget is resolved inside, where the default lives — see [`super::manifest::Run`].
        Run::Gluon { calls, .. } => typed::evaluate(cap, inputs, step, *calls),
    }
}

/// The shell arm: a process, standing in the input tree, writing into a scratch output tree.
///
/// The contract is five environment variables and a working directory, and it is deliberately
/// small enough to implement in a shell script — the first calculator is one, because a
/// vertical slice that needed a build system to demonstrate would be demonstrating the build
/// system.
///
/// `YIDAM_GRAPH` is the fifth (#1080), and it is the only one that is sometimes absent: it
/// names the resolved corpus, and a step whose `reads` admit no corpus node is handed no
/// resolved corpus at all. A script that wants it tests for it — `[ -n "${YIDAM_GRAPH:-}" ]`
/// — rather than assuming the file is there, which is the same shape as the other four and is
/// what the manifest doc states.
fn process(argv: &[String], inputs: &Inputs, step: &str, commit: &str) -> Result<Produced> {
    let out = Scratch::new("out")?;
    let mut command = Command::new(&argv[0]);
    command
        .args(&argv[1..])
        .current_dir(inputs.dir.path())
        .env("YIDAM_IN", inputs.dir.path())
        .env("YIDAM_OUT", out.path())
        .env("YIDAM_STEP", step)
        .env("YIDAM_INPUT_COMMIT", commit)
        .stdin(Stdio::null());
    // Removed and not merely left unset, because the process inherits this environment: a
    // `YIDAM_GRAPH` the caller happened to export would otherwise point a step at a file this
    // run did not write, which is the one thing the isolation exists to make impossible.
    match &inputs.resolved {
        Some(r) => command.env("YIDAM_GRAPH", &r.path),
        None => command.env_remove("YIDAM_GRAPH"),
    };
    let status = command
        .output()
        .with_context(|| format!("invoking `{}`", argv.join(" ")))?;

    let stderr = String::from_utf8_lossy(&status.stderr).trim().to_string();
    if !status.status.success() {
        bail!(
            "`{}` exited {}{}",
            argv.join(" "),
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

/// The typed arm (RFC-0042): a script applied to the corpus as a value, in this process.
///
/// # Two implementations, and the light one refuses rather than panics
///
/// `calculators-gluon` is outside the default set, so the real module is compiled out of the binary
/// `install.sh` downloads. The plan pre-pass in [`super::plan_and_write`] refuses a typed step
/// before anything is materialized — [`super::manifest::Run::unrunnable_because`] is where that
/// answer comes from — so this function is unreachable in a light build.
///
/// It refuses anyway, rather than panicking. "Unreachable because a check two modules away is in
/// place" is a claim about code that changes, and the cost of being wrong about it is a panic in a
/// released binary instead of the sentence that check was already going to print. It asks the
/// declaration for that sentence rather than repeating it, so the two cannot come apart.
#[cfg(feature = "calculators-gluon")]
mod typed {
    use anyhow::{Context, Result};

    use super::{Capability, Inputs, Produced};
    use crate::gluon_arm::{budget, marshal};

    pub fn evaluate(
        _cap: &Capability,
        inputs: &Inputs,
        step: &str,
        calls: Option<usize>,
    ) -> Result<Produced> {
        let script = inputs
            .script
            .as_ref()
            .context("a typed calculator was invoked with no script read — see `materialize`")?;

        // The corpus is read from the materialized tree, which is the tree `ResolvedCorpus` parses
        // and the tree the shell arm's process stands in. That is what makes the two arms answer
        // about one corpus: `reads` bounds the value exactly as it bounds the directory, with no
        // second reading of the declaration that could fall out of step with the first.
        let read = crate::corpus::Corpus::open(inputs.dir.path());
        let corpus = marshal::project(read.nodes(), read.classes());
        let nodes = corpus.nodes.len();

        let budget = calls.unwrap_or(budget::DEFAULT_CALLS);
        let outcome = crate::gluon_arm::evaluate(step, &script.text, corpus, budget)?;

        let rel = format!(".yidam/computed/{step}.yml");
        let text = marshal::render(step, &script.path, &outcome.computed)
            .with_context(|| format!("rendering what `{step}` computed as {rel}"))?;

        // The one line a run of this arm has to say about itself, on the channel a shell
        // calculator's own account arrives on. Not written into the committed file: the number of
        // calls a VM charged is a fact about this gluon and not about the corpus, and a committed
        // file carrying it would churn on an upgrade that computed the identical answer.
        let stderr = format!(
            "{step}: {nodes} nodes, {} signal rows, {} of {budget} calls",
            outcome.computed.signals.len(),
            outcome.calls
        );
        Ok(Produced {
            outputs: vec![(rel, text.into_bytes())],
            stderr,
        })
    }
}

#[cfg(not(feature = "calculators-gluon"))]
mod typed {
    use anyhow::{bail, Result};

    use super::{Capability, Inputs, Produced};

    pub fn evaluate(
        cap: &Capability,
        _inputs: &Inputs,
        step: &str,
        _calls: Option<usize>,
    ) -> Result<Produced> {
        bail!(
            "`{step}` declares `{}` and nothing was run.\n  {}.\n  \
             A plan holding this step is refused before anything is materialized, so reaching here \
             means that pre-pass was bypassed — which is a defect in this binary and not in the \
             manifest.",
            cap.run.declared(),
            cap.unrunnable_because()
                .map(|u| u.because)
                .unwrap_or("the typed calculator arm is compiled out of this binary")
        );
    }
}
