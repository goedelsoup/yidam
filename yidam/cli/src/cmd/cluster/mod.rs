//! `yidam cluster` — the run invariant as a permission (RFC-0026 §7, #475).
//!
//! `cmd/run` holds this sentence as a code path:
//!
//! > A run authors operational commits directly. Every epistemic commit it produces goes to a
//! > proposal branch, and nothing merges itself.
//!
//! On one machine that is enough, because the process that builds the commit is the process
//! that writes the ref, and the route is a total function of the verb. On a cluster it is not
//! enough, and the reason is the one that makes a cluster worth having: the process that
//! computes is not the process that lands. Once those are two pods, the invariant is either
//! restated in every pod — in which case it is still a code path, now in several places — or
//! it is made into something a pod **cannot** do rather than something it does not do.
//!
//! # The permission
//!
//! Five subcommands, and only one of them can move a ref on the remote:
//!
//! - `pin` reads the branch and puts a git bundle of it in the vault, by digest.
//! - `step <name>` reads a pinned bundle, invokes one capability against it, builds the
//!   commit **as an object**, puts a bundle of that commit in the vault, and emits
//!   `{sha, class, verb, receipt, bundle}`. It has no `--remote`. There is no flag, no
//!   environment variable and no code path by which it names a ref on the corpus's remote,
//!   and the pod it runs in holds no credential for one.
//! - `land` fetches that bundle, classifies the commit by its message — not by the pod's
//!   claim — chooses the ref the class permits, and writes it with a compare-and-swap. It is
//!   the only template in the generated manifest that mounts the write credential.
//! - `admit` decides whether a workflow is submitted at all.
//! - `workflow` writes the Argo manifest that arranges the four above.
//!
//! What the step pod emits is a sha, and a sha is not a permission. The step could compute
//! the wrong class, lie about it in its output, or write `refs/heads/main` in its own scratch
//! clone, and the corpus's remote would not move: the lander recomputes the class from the
//! commit, and the pod's scratch clone dies with the pod. `tests/cluster_run.rs` holds the
//! boundary by trying to cross it — a valid commit sha in a process with no ref credential —
//! and asserts the failure is git's, not a check of ours.
//!
//! # What crosses between pods, and why it is a git bundle
//!
//! #460 decision 6: a pod gets a pinned read-only bundle, never a shared volume holding a
//! checkout. The `.yiz` bundle `export` writes is the wrong object for this — it is re-rooted,
//! carries no history, and omits `.yidam/capabilities/**`, so a step could not build a commit
//! on it and could not find its own script in it. A **git bundle** at the input sha has every
//! property decision 6 asks for: it is pinned (one sha), read-only (a file in a content-
//! addressed store), fetched by digest, and holds nothing a working tree would. The vault is
//! the transport, which is RFC-0023's sentence one more time: the vault stores bytes, git
//! stores the record of them, and the record here is the receipt in the commit.
//!
//! # Pod logs are not provenance
//!
//! Nothing a pod prints is read by anything. The receipt is in the commit, the commit is on
//! the ref, and the ref is the record — so a workflow whose logs were garbage-collected an
//! hour after it ran has lost nothing the corpus relies on. A step's stderr is passed
//! through to the pod's own, for whoever is debugging it, and recorded nowhere.

pub mod admit;
pub mod builtin;
mod bundle;
pub mod gather;
pub mod land;
pub mod pin;
pub mod step;
pub mod workflow;

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::{Args, Subcommand};
use serde::{Deserialize, Serialize};

use crate::report::Format;
use crate::vault::{Store, VaultConfig};

/// The version of the JSON a pod emits and the next pod reads.
///
/// From the first commit, for `receipt.rs`'s reason: a consumer that reads a field the
/// producer never wrote fails at the reader, in a cluster whose pods are all fine.
pub const CONTRACT_VERSION: u32 = 1;

/// The ref a step pod points at the commit it built, in its own scratch clone only.
///
/// Not `refs/heads/*`, deliberately: nothing that reads a bundle should mistake it for a
/// branch, and a pod that wrote a branch name would be writing the one word it may not.
const OUT_REF: &str = "refs/yidam/out";

#[derive(Subcommand)]
pub enum ClusterCommand {
    /// Write the Argo Workflow that runs this corpus's capability manifest on a cluster
    ///
    /// One `pin`, then a `step` and a `land` for each built-in catalog step (`catalog-fetch`,
    /// `catalog-extract`, `catalog-reconcile`) and each capability in dependency order. A
    /// chain rather than a wider DAG because a run is a chain: each step is invoked against
    /// the commit the one before it landed. `--cron` writes a `CronWorkflow` whose first
    /// task is `admit`, and nothing below it runs unless admission says so.
    ///
    /// Reads `[cluster]` and `[vault.<name>]` from `.yidam/config.toml`; every flag below
    /// overrides the matching key, so a corpus with no config can still be generated for.
    Workflow {
        /// Write a CronWorkflow on this schedule (five-field cron), gated by `admit`
        #[arg(long, value_name = "SCHEDULE")]
        cron: Option<String>,
        /// The container image every pod runs; overrides `[cluster] image`
        #[arg(long, value_name = "IMAGE")]
        image: Option<String>,
        /// The git remote the lander writes; overrides `[cluster] remote`
        #[arg(long, value_name = "URL")]
        remote: Option<String>,
        /// The branch a run advances; overrides `[cluster] branch`
        #[arg(long, value_name = "BRANCH")]
        branch: Option<String>,
        /// The vault url bundles travel through; overrides `[vault.<name>] url`
        #[arg(long = "vault-url", value_name = "URL")]
        vault_url: Option<String>,
    },
    /// Read the branch tip and put a git bundle of it in the vault — the input every step reads
    ///
    /// The first task of a workflow. Needs a credential that can read the remote and nothing
    /// that can write it.
    Pin {
        #[command(flatten)]
        remote: RemoteArgs,
        #[command(flatten)]
        vault: VaultArgs,
        /// Write the pin record — `{branch, sha, bundle}` — to this file, for the next task
        #[arg(long, value_name = "FILE")]
        out: Option<PathBuf>,
        /// Output format. `json` emits the machine-readable report contract (RFC-0016)
        #[arg(long, value_enum, default_value_t = Format::Text)]
        format: Format,
    },
    /// Invoke one capability against a pinned bundle and emit the commit it built, unlanded
    ///
    /// Reads the bundle from the vault, stands the capability in a tree holding exactly
    /// what it declares it reads, builds the outputs and the receipt into a commit object
    /// on the pinned sha, and puts a bundle of that commit back in the vault. It emits
    /// `{sha, class, verb, receipt, bundle}` and nothing else.
    ///
    /// This command has no `--remote`. It cannot name a ref on the corpus, and the pod it
    /// runs in holds no credential for one: what it emits is a sha, and a sha is not a
    /// permission.
    Step {
        /// The capability to invoke, as `.yidam/capabilities.toml` names it
        name: String,
        /// The vault digest of the bundle to run against — a `pin` or a `land` wrote it
        #[arg(long, value_name = "DIGEST")]
        bundle: String,
        /// The image reference this pod was started from. Recorded in the receipt only when it
        /// pins a digest (`…@sha256:<hex>`); a tag is not read and records nothing
        #[arg(long, value_name = "REF")]
        image: Option<String>,
        #[command(flatten)]
        vault: VaultArgs,
        /// Write the step record to this file, for the lander
        #[arg(long, value_name = "FILE")]
        out: Option<PathBuf>,
        /// Output format. `json` emits the machine-readable report contract (RFC-0016)
        #[arg(long, value_enum, default_value_t = Format::Text)]
        format: Format,
    },
    /// Land a step's commit on the ref its class permits — the only command that writes a ref
    ///
    /// Fetches the step's bundle, classifies the commit by its own message, and writes the
    /// branch for an operational commit or `propose/<input>` for an epistemic one, with a
    /// compare-and-swap on the ref. When the ref moved since the step ran it re-parents the
    /// commit and tries again, unless the move touched what the step reads — then it lands
    /// nothing and says so, and the next admission runs the step over.
    ///
    /// The step's own `class` is read and checked, never trusted: a claim in a pod's output
    /// is not a permission either.
    Land {
        /// The step record — JSON inline, or `@path` to read it from a file
        #[arg(long = "step-output", value_name = "JSON|@FILE")]
        step_output: String,
        #[command(flatten)]
        remote: RemoteArgs,
        #[command(flatten)]
        vault: VaultArgs,
        /// Write the landing record — what moved, and the next pin — to this file
        #[arg(long, value_name = "FILE")]
        out: Option<PathBuf>,
        /// Output format. `json` emits the machine-readable report contract (RFC-0016)
        #[arg(long, value_enum, default_value_t = Format::Text)]
        format: Format,
    },
    /// Plan a gather at a pinned bundle: who is asked, and the query in each peer's words
    ///
    /// Reads `.yidam/gathers/<name>.toml` and `tonpa.lock` from the pin and emits one ask per
    /// peer the gather can put its question to, already translated — so the correspondence
    /// is read here and nowhere else — and every other peer with the reason it is not asked.
    /// No `--remote`, and no git credential.
    Survey {
        /// The gather, as `.yidam/gathers/<name>.toml` names it
        gather: String,
        /// The vault digest of the pinned bundle to plan from
        #[arg(long, value_name = "DIGEST")]
        bundle: String,
        #[command(flatten)]
        vault: VaultArgs,
        /// Write the plan to this file, for the askers
        #[arg(long, value_name = "FILE")]
        out: Option<PathBuf>,
        /// Output format. `json` emits the machine-readable report contract (RFC-0016)
        #[arg(long, value_enum, default_value_t = Format::Text)]
        format: Format,
    },
    /// Ask one peer one planned question, from the peer's own locked bundle
    ///
    /// Takes the bundle from the vault when the vault has it and fetches it by its
    /// `tonpa.lock` url otherwise, checking it against the lock's sha256 either way. Always
    /// emits a record — a peer that could not be asked is `refused` with the reason — and
    /// puts it in the vault for `gather`. Handed no corpus, no `--remote`, and no git
    /// credential.
    Ask {
        /// One ask, as `survey` emits it — JSON inline, or `@path`
        #[arg(long, value_name = "JSON|@FILE")]
        ask: String,
        /// The image reference this pod was started from. Recorded in the peer's receipt only
        /// when it pins a digest (`…@sha256:<hex>`)
        #[arg(long, value_name = "REF")]
        image: Option<String>,
        #[command(flatten)]
        vault: VaultArgs,
        /// Write the ask record to this file, for `gather`
        #[arg(long, value_name = "FILE")]
        out: Option<PathBuf>,
        /// Output format. `json` emits the machine-readable report contract (RFC-0016)
        #[arg(long, value_enum, default_value_t = Format::Text)]
        format: Format,
    },
    /// Settle the askers' records into one roll call and build the gather's commits, unlanded
    ///
    /// Re-plans at the pin and holds every record to that plan: a peer planned and not
    /// answered for is `refused`, never dropped. Builds what `yidam gather` builds over the
    /// same pins — one `open:` commit per question node, with each peer's receipt — as
    /// objects on the pin, and emits a step record for `land`.
    Gather {
        /// The gather, as `.yidam/gathers/<name>.toml` names it
        gather: String,
        /// The vault digest of the pinned bundle `survey` planned from
        #[arg(long, value_name = "DIGEST")]
        bundle: String,
        /// The askers' records, as a JSON array — inline, or `@path`
        #[arg(long, value_name = "JSON|@FILE")]
        asked: String,
        #[command(flatten)]
        vault: VaultArgs,
        /// Write the step record to this file, for the lander
        #[arg(long, value_name = "FILE")]
        out: Option<PathBuf>,
        /// Output format. `json` emits the machine-readable report contract (RFC-0016)
        #[arg(long, value_enum, default_value_t = Format::Text)]
        format: Format,
    },
    /// Decide whether a workflow is owed — `due`'s clocks, stale steps, and the proposal cap
    ///
    /// The admission gate #460 decision 8 names. Admits when something is owed and the
    /// count of open `propose/*` branches is under `[cluster] max_open_proposals`; a corpus
    /// with nothing owed submits no workflow. Exits zero either way — the verdict is in the
    /// record, which is what the CronWorkflow's `when:` reads.
    Admit {
        #[command(flatten)]
        remote: RemoteArgs,
        /// Write the admission record to this file, for the workflow's `when:`
        #[arg(long, value_name = "FILE")]
        out: Option<PathBuf>,
        /// Output format. `json` emits the machine-readable report contract (RFC-0016)
        #[arg(long, value_enum, default_value_t = Format::Text)]
        format: Format,
    },
}

/// The remote a pod reads or writes. Absent from `step` by construction.
#[derive(Args, Debug, Clone)]
pub struct RemoteArgs {
    /// The corpus's git remote — an SSH or HTTPS url, or a path
    #[arg(long, value_name = "URL")]
    pub remote: String,
    /// The branch a run advances
    #[arg(long, value_name = "BRANCH", default_value = "main")]
    pub branch: String,
}

/// The vault bundles travel through.
///
/// Spelled out as flags rather than read from `.yidam/config.toml`, because a pod has no
/// config until it has fetched the bundle the config is in. The workflow generator writes
/// them from the config, so a person never types them; credentials come from the
/// environment, as they do everywhere else — `YIDAM_VAULT_<NAME>_ACCESS_KEY_ID`, with the
/// ambient `AWS_*` honoured only for `default`.
#[derive(Args, Debug, Clone)]
pub struct VaultArgs {
    /// The vault's name, which decides which credential variables are read
    #[arg(long, value_name = "NAME", default_value = "default")]
    pub vault: String,
    /// The vault url — `file:///path` or `s3://bucket/prefix`
    #[arg(long = "vault-url", value_name = "URL")]
    pub vault_url: String,
    /// The S3 region, where the url is an S3 one
    #[arg(long = "vault-region", value_name = "REGION")]
    pub vault_region: Option<String>,
    /// An S3-compatible endpoint, for a store that is not AWS
    #[arg(long = "vault-endpoint", value_name = "URL")]
    pub vault_endpoint: Option<String>,
    /// Address the bucket in the path rather than the host, for endpoints that need it
    #[arg(long = "vault-path-style")]
    pub vault_path_style: bool,
}

impl VaultArgs {
    /// Open the store these flags describe.
    pub(crate) fn open(&self) -> Result<Box<dyn Store>> {
        let cfg = VaultConfig {
            url: self.vault_url.clone(),
            audience: None,
            holds: None,
            region: self.vault_region.clone(),
            endpoint: self.vault_endpoint.clone(),
            path_style: self.vault_path_style.then_some(true),
        };
        crate::vault::open(&self.vault, &cfg)
    }
}

pub fn run(sub: ClusterCommand) -> Result<()> {
    match sub {
        ClusterCommand::Workflow {
            cron,
            image,
            remote,
            branch,
            vault_url,
        } => workflow::run(&workflow::Overrides {
            cron,
            image,
            remote,
            branch,
            vault_url,
        }),
        ClusterCommand::Pin {
            remote,
            vault,
            out,
            format,
        } => pin::run(&remote, &vault, out.as_deref(), format),
        ClusterCommand::Step {
            name,
            bundle,
            image,
            vault,
            out,
            format,
        } => step::run(
            &name,
            &bundle,
            image.as_deref(),
            &vault,
            out.as_deref(),
            format,
        ),
        ClusterCommand::Land {
            step_output,
            remote,
            vault,
            out,
            format,
        } => land::run(&step_output, &remote, &vault, out.as_deref(), format),
        ClusterCommand::Survey {
            gather: name,
            bundle,
            vault,
            out,
            format,
        } => gather::survey(&name, &bundle, &vault, out.as_deref(), format),
        ClusterCommand::Ask {
            ask,
            image,
            vault,
            out,
            format,
        } => gather::ask(&ask, image.as_deref(), &vault, out.as_deref(), format),
        ClusterCommand::Gather {
            gather: name,
            bundle,
            asked,
            vault,
            out,
            format,
        } => gather::gather(&name, &bundle, &asked, &vault, out.as_deref(), format),
        ClusterCommand::Admit {
            remote,
            out,
            format,
        } => admit::run(&remote, out.as_deref(), format),
    }
}

// ── the contract between pods ─────────────────────────────────────────────────

/// What a `pin` or a `land` hands the next `step`: the branch, its tip, and where the bytes are.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pinned {
    pub format_version: u32,
    pub branch: String,
    pub sha: String,
    /// The vault digest of a git bundle whose `refs/heads/<branch>` is `sha`.
    pub bundle: String,
}

/// How a step ended, in the three ways it can.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StepOutcome {
    /// It was invoked and built a commit.
    Ran,
    /// Its committed receipt matches this input state, so it was not invoked.
    Fresh,
    /// It was invoked and produced the tree already at its input, so there is no commit.
    Unchanged,
}

/// What a step pod emits — the whole of it.
///
/// `class` and `verb` are here so a reader of the workflow can see what a step did without
/// opening the commit, and for no other reason: the lander recomputes both from the commit
/// and refuses when they disagree.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepOutput {
    pub format_version: u32,
    pub step: String,
    pub outcome: StepOutcome,
    /// The commit built, as an object nothing points at. Absent unless `outcome` is `ran`.
    pub sha: Option<String>,
    /// `operational` or `epistemic`, as the verb's family reads.
    pub class: String,
    pub verb: String,
    /// Where the receipt is in the commit's tree.
    pub receipt: String,
    /// The pinned commit the step ran against, which is the commit's parent.
    pub input: String,
    /// The vault digest of a bundle carrying the commit, with `input` as its prerequisite.
    pub bundle: Option<String>,
}

/// What the lander emits: what moved, and the pin the next step reads.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Landed {
    pub format_version: u32,
    pub step: String,
    /// The commit that landed — the step's, or its re-parented copy. Absent when nothing did.
    pub landed: Option<String>,
    /// The ref it landed on. The branch for an operational commit, `propose/<input>` for an
    /// epistemic one, and for an epistemic one never the branch.
    pub target: Option<String>,
    pub class: String,
    /// Whether the ref had moved since the step ran and the commit was rebuilt on its tip.
    pub reparented: bool,
    /// How many compare-and-swap attempts the ref write took.
    pub attempts: u32,
    /// The branch after landing, which is what the next step reads.
    pub next: Pinned,
}

/// What admission decided, and every input it decided on.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Admission {
    pub format_version: u32,
    pub admitted: bool,
    pub because: String,
    /// `due` clocks past their interval, by id.
    pub owed: Vec<String>,
    /// Steps whose committed receipt does not match the input state at the tip.
    pub stale: Vec<String>,
    pub open_proposals: usize,
    /// `[cluster] max_open_proposals`, or `None` where the corpus has not set one.
    pub max_open_proposals: Option<usize>,
}

/// The one epilogue every pod shares: the record to its file, the report to stdout.
///
/// The record is the contract; the file is what the next task reads, because Argo hands a
/// parameter from a file rather than from a stream. `None` writes nothing, which is how the
/// same command is run by a person at a terminal.
///
/// Under `--format json` the record sits under a `record` key rather than flattened into
/// the envelope. Every other report is flattened, and this one cannot be: the record
/// carries its own `format_version`, the envelope carries the report contract's, and
/// `receipt.rs` says why they are two numbers — *"they change for different reasons"*.
/// Flattened, the two would be one JSON key written twice.
pub(crate) fn deliver<T: Serialize>(
    root: &Path,
    format: Format,
    out: Option<&Path>,
    record: T,
    text: impl FnOnce(&T) -> String,
) -> Result<()> {
    if let Some(path) = out {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating {}", parent.display()))?;
        }
        let json = serde_json::to_string_pretty(&record).context("serializing the record")?;
        std::fs::write(path, json).with_context(|| format!("writing {}", path.display()))?;
    }
    #[derive(Serialize)]
    struct Report<T: Serialize> {
        record: T,
    }
    crate::report::finish(root, format, Report { record }, |r| {
        print!("{}", text(&r.record))
    })
}

/// A step record, from the argument `land` was given.
///
/// `@path` reads a file, which is how the tests hand one over; anything else is the JSON
/// itself, which is how Argo does — a task's output parameter arrives inline.
///
/// # Why an exact version match is safe here, and what would make it unsafe
///
/// The refusal is exact, not "this version or older", and that is only safe because of
/// who writes a step record and what is in it. The step pod and the lander run one image
/// (the workflow names it once), so the producer and the consumer of this record are the
/// same binary and a mismatch means a workflow assembled from two releases — which is worth
/// refusing loudly.
///
/// It is also safe because the record carries the receipt's *path*, not its fields. The
/// receipt is committed in the tree under its own `receipt::FORMAT_VERSION` and read by
/// every later binary that opens the corpus, and that number moves when what a run records
/// moves. `CONTRACT_VERSION` moves only when what one pod tells the next moves.
///
/// Copying a receipt field into [`StepOutput`] would break that. Every receipt bump would
/// then be a contract bump, and a lander one release behind its steps would refuse a record
/// it could otherwise have landed — the *"additive fields strand old producers"* failure,
/// in a spot where it strands a whole workflow. `the_step_record_carries_the_receipts_path_and_none_of_its_fields`
/// is what goes red if that happens.
pub(crate) fn read_step_output(arg: &str) -> Result<StepOutput> {
    let text = match arg.strip_prefix('@') {
        Some(path) => std::fs::read_to_string(path)
            .with_context(|| format!("reading the step record at {path}"))?,
        None => arg.to_string(),
    };
    let record: StepOutput =
        serde_json::from_str(&text).context("the step record is not the JSON `step` emits")?;
    if record.format_version != CONTRACT_VERSION {
        anyhow::bail!(
            "the step record is format_version {}, and this binary reads {CONTRACT_VERSION}",
            record.format_version
        );
    }
    Ok(record)
}

/// The class a route's commits carry, in the word `classify_commit` uses.
pub(crate) fn class_of(route: crate::cmd::run::manifest::Route) -> &'static str {
    use crate::cmd::run::manifest::Route;
    match route {
        Route::Branch => "operational",
        Route::Proposal => "epistemic",
    }
}

/// Clone `remote`'s `branch` into `dest`, full history, that branch only.
///
/// Full history rather than shallow, because a bundle with a prerequisite can only be
/// fetched into a repository that holds the prerequisite, and a lander that cloned shallow
/// would then fail on the first step whose input had moved one commit back.
pub(crate) fn clone_branch(remote: &str, branch: &str, dest: &Path) -> Result<()> {
    let parent = dest.parent().unwrap_or(dest);
    crate::git::Git::new(parent)
        .args(["clone", "-q", "--branch", branch, "--single-branch", remote])
        .arg(dest)
        .run()
        .with_context(|| format!("cloning {branch} from {remote}"))?;
    Ok(())
}

/// The committer a pod's commits carry.
///
/// `cmd/run`'s author is `yidam run` and its committer is "whoever ran it", which on a
/// checkout is the person's git config. In a pod nobody ran it, and a scratch clone inherits
/// no config, so `commit-tree` would refuse for want of a committer. This names the pod in
/// the clone's local config, which is gone with the clone. `GIT_COMMITTER_NAME` and
/// `GIT_COMMITTER_EMAIL` in the pod's environment still win, which is how a deployment
/// names its own.
pub(crate) const COMMITTER_NAME: &str = "yidam cluster";
pub(crate) const COMMITTER_EMAIL: &str = "cluster@yidam";

pub(crate) fn commits_as_the_pod(root: &Path) -> Result<()> {
    for (key, value) in [
        ("user.name", COMMITTER_NAME),
        ("user.email", COMMITTER_EMAIL),
    ] {
        crate::git::Git::new(root)
            .args(["config", key, value])
            .run()
            .with_context(|| format!("naming the pod as {key} in the scratch clone"))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The thing the module doc says: the step surface cannot name a ref.
    ///
    /// Asserted on the clap definition rather than on `--help` text, so a flag added to
    /// `Step` with a remote in it fails here before it has help to read.
    #[test]
    fn the_step_command_has_no_remote_and_no_branch() {
        use clap::CommandFactory;
        #[derive(clap::Parser)]
        struct Cli {
            #[command(subcommand)]
            sub: ClusterCommand,
        }
        let cmd = Cli::command();
        // A gather's three pods are held to what a step is (#1217).
        for pod in ["step", "survey", "ask", "gather"] {
            let c = cmd
                .get_subcommands()
                .find(|c| c.get_name() == pod)
                .expect(pod);
            let flags: Vec<String> = c
                .get_arguments()
                .filter_map(|a| a.get_long().map(str::to_string))
                .collect();
            for forbidden in ["remote", "branch", "ref", "push", "land"] {
                assert!(
                    !flags.iter().any(|f| f.contains(forbidden)),
                    "`cluster {pod}` grew a `--{forbidden}` flag: {flags:?}. A pod that computes \
                     must have no way to name a ref — see the module doc"
                );
            }
        }
        // And the two that may name one, do.
        for named in ["pin", "land", "admit"] {
            let c = cmd
                .get_subcommands()
                .find(|c| c.get_name() == named)
                .expect(named);
            assert!(
                c.get_arguments().any(|a| a.get_long() == Some("remote")),
                "`cluster {named}` has no --remote"
            );
        }
    }

    #[test]
    fn a_step_record_round_trips_and_refuses_a_future_version() {
        let record = StepOutput {
            format_version: CONTRACT_VERSION,
            step: "travel-tier".into(),
            outcome: StepOutcome::Ran,
            sha: Some("a".repeat(40)),
            class: "operational".into(),
            verb: "compute".into(),
            receipt: ".yidam/runs/travel-tier.yml".into(),
            input: "b".repeat(40),
            bundle: Some("c".repeat(64)),
        };
        let json = serde_json::to_string(&record).unwrap();
        let back = read_step_output(&json).unwrap();
        assert_eq!(back.sha, record.sha);
        assert_eq!(back.outcome, StepOutcome::Ran);

        let future = json.replace("\"format_version\":1", "\"format_version\":2");
        assert_ne!(future, json);
        let err = read_step_output(&future).unwrap_err().to_string();
        assert!(err.contains("format_version 2"), "{err}");
    }

    /// Every key a value serializes to, at any depth.
    fn keys(v: &serde_json::Value, into: &mut std::collections::BTreeSet<String>) {
        match v {
            serde_json::Value::Object(m) => {
                for (k, v) in m {
                    into.insert(k.clone());
                    keys(v, into);
                }
            }
            serde_json::Value::Array(a) => a.iter().for_each(|v| keys(v, into)),
            _ => {}
        }
    }

    /// The step record names the receipt by path and copies none of what it records, so a
    /// receipt bump is never a contract bump — see [`read_step_output`].
    ///
    /// Both key sets are read off the serialized structs, so a field added to either side is
    /// in the comparison the day it compiles. What is written by hand is the overlap the
    /// contract *does* share with the receipt, and that is four names that identify the step,
    /// not a list of what to look for. The receipt is built with every optional field
    /// present, because a field skipped when absent is a key this would not see.
    #[test]
    fn the_step_record_carries_the_receipts_path_and_none_of_its_fields() {
        use crate::cmd::run::manifest::Run;
        use crate::cmd::run::receipt::{self, File, Input, Receipt};
        let file = || File {
            path: ".yidam/corpus/a.md".into(),
            sha256: "d".repeat(64),
        };
        let receipt = Receipt {
            format_version: receipt::FORMAT_VERSION,
            step: "travel-tier".into(),
            kind: "calculator",
            verb: "compute".into(),
            run: Run::Argv(vec!["sh".into(), "x.sh".into()]),
            input_state: "e".repeat(64),
            input: Input {
                commit: "b".repeat(40),
                manifest_sha256: "f".repeat(64),
                config_sha256: "f".repeat(64),
                reads: vec![".yidam/corpus/**".into()],
                files: vec![file()],
                resolved_graph_sha256: Some("g".repeat(64)),
                script_sha256: Some("h".repeat(64)),
            },
            writes: vec![".yidam/computed/**".into()],
            outputs: vec![file()],
            model: Some("m".into()),
            version: Some("v".into()),
            config: Some("c".into()),
            image_digest: Some(format!("sha256:{}", "i".repeat(64))),
        };
        let record = StepOutput {
            format_version: CONTRACT_VERSION,
            step: "travel-tier".into(),
            outcome: StepOutcome::Ran,
            sha: Some("a".repeat(40)),
            class: "operational".into(),
            verb: "compute".into(),
            receipt: receipt::Receipt::path("travel-tier"),
            input: "b".repeat(40),
            bundle: Some("c".repeat(64)),
        };

        let mut recorded = std::collections::BTreeSet::new();
        keys(&serde_json::to_value(&receipt).unwrap(), &mut recorded);
        let mut contract = std::collections::BTreeSet::new();
        keys(&serde_json::to_value(&record).unwrap(), &mut contract);
        assert!(
            recorded.len() > 10 && contract.len() > 5,
            "the key walk saw nothing: {recorded:?} / {contract:?}"
        );
        let shared: Vec<&str> = contract
            .intersection(&recorded)
            .map(String::as_str)
            .collect();
        assert_eq!(
            shared,
            ["format_version", "input", "step", "verb"],
            "the step record holds a field the receipt records. A receipt bump would then be a \
             contract bump, and a lander one release behind would refuse the record. Carry the \
             receipt's path, which the record already does, and read the field from the tree."
        );
        assert!(
            record.receipt.ends_with(".yml"),
            "the record names the receipt by path"
        );
    }

    #[test]
    fn the_class_words_are_the_classifiers() {
        use crate::cmd::run::manifest::Route;
        let op = yidam_core::git::classify_commit("x", "compute: a\n");
        let ep = yidam_core::git::classify_commit("x", "establish: a\n");
        assert_eq!(
            format!("{:?}", op.kind).to_lowercase(),
            class_of(Route::Branch)
        );
        assert_eq!(
            format!("{:?}", ep.kind).to_lowercase(),
            class_of(Route::Proposal)
        );
    }
}
