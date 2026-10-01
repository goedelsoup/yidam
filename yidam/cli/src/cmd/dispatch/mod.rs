//! `yidam dispatch` — run an agent elector as a run, and propose what it wrote onto its seat (#477).
//!
//! A seat is a `ma/<name>` branch and its row in `electors.md`, and an agent seat's positions were
//! written by whatever an operator happened to launch: nothing recorded what ran, under which
//! configuration, against which commit. A cluster would have supplied that by accident — a pod has
//! an image and a digest — and a definition that arrived that way would be whatever the cluster
//! happened to record. This is the definition, written down first.
//!
//! # What an elector run is
//!
//! A seat declares its occupant in `.yidam/sangha/dispatch/<name>.toml` **on its own branch**: the
//! model and version the registry row names, the argv that runs it, what it reads, and which of
//! those files make up its operative configuration. Everything is read at the seat's tip — the
//! declaration, the registry row, the corpus the elector is handed — because that is *"the state
//! of the agent that held it"*, the same reading `independence:` is derived from.
//!
//! The elector runs through the run executor's shell arm (RFC-0026 §2): the declared `reads`
//! checked out of the tip into a scratch tree, a scratch output directory, and a refusal of any
//! write outside the position and the seat's commitments file. It is handed the seat, the
//! question and the path its position is expected at.
//!
//! # Registry first, receipt corroborates
//!
//! **The dispatcher refuses a seat whose row does not already describe the occupant it would
//! run.** `Model`, `Version` and `Config` must be filled in and must agree with the declaration —
//! `Config` against a digest computed here from the argv and the configuration files. A run never
//! writes the registry: a model bump is a registration update a person commits, and only then does
//! a dispatch under the new model go through. So the row a resolution's `independence:` is later
//! derived from is the row the run was checked against, and the receipt committed beside the
//! position records the same three values as corroboration and nothing more — see
//! [`crate::cmd::lint::elector_receipt`], which reports a tip where the two part.
//!
//! # What it refuses to do
//!
//! **A position is epistemic, so it is proposed.** RFC-0026 §2's invariant: a run authors
//! operational commits and nothing else, and routes everything epistemic through `propose/*`. The
//! position lands on `propose/elector/<name>/<question>/<tip>`, parented on the seat's tip, as the
//! `open:` or `revise:` commit PROTOCOL.md's step 1 asks for. It reaches `ma/<name>` only when
//! whoever answers for the seat fast-forwards it; nothing here moves a seat, transports a
//! position, authors `resolve:`, or merges anything.
//!
//! **No count of positions is reported.** Three seats under one configuration are one position
//! read three times — PROTOCOL.md: *"The resolution read fewer positions than it read tips."* So
//! the report lists the seats and derives `independence:` over them with the same function the
//! lint uses, and never says how many positions it produced.
//!
//! **A human seat is not dispatched.** A person answers for their own seat.

use std::fmt::Write as _;
use std::path::Path;

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

use crate::cmd::lint::elector_receipt::{config_matches, receipt_path};
use crate::cmd::lint::independence::{derive, SeatAtTip};
use crate::cmd::propose::write::{self, TempIndex};
use crate::cmd::run::exec;
use crate::cmd::run::manifest::{Capability, Kind, Run};
use crate::cmd::run::receipt::{self, File, Input, Receipt};
use crate::cmd::sangha::{parse_electors, ElectorRow};
use crate::paths::{repo_root, require_yidam_repo};

#[cfg(test)]
mod tests;

/// The author every dispatched commit carries — the tool wrote the commit; the elector wrote the
/// position; whoever answers for the seat decides whether it stands.
const AUTHOR_NAME: &str = "yidam dispatch";
const AUTHOR_EMAIL: &str = "dispatch@yidam";

const REGISTRY: &str = ".yidam/sangha/electors.md";

pub struct Options {
    /// The question, as the slug a position file is named by: `<elector>-<question>.md`.
    pub question: String,
    /// The seats to dispatch, by name — `auditor` for `ma/auditor`.
    pub seats: Vec<String>,
    /// Run every elector and report; write no branch.
    pub dry_run: bool,
    /// Replace an existing proposal at this tip whose content differs.
    pub force: bool,
    pub format: crate::report::Format,
}

// ── the declaration ───────────────────────────────────────────────────────────

/// `.yidam/sangha/dispatch/<name>.toml`, as the seat's branch holds it.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Declaration {
    /// The model the elector runs. Must equal the row's `Model`.
    pub model: String,
    /// The model's version. Must equal the row's `Version`.
    pub version: String,
    /// The argv that runs the elector, from the root of its input tree.
    pub run: Vec<String>,
    /// Globs, relative to the corpus root: exactly what the elector is handed.
    pub reads: Vec<String>,
    /// Globs naming the files, among `reads`, that make up the operative configuration — a
    /// system prompt, a harness config. Their digests and the argv are the `Config` hash.
    #[serde(default)]
    pub config: Vec<String>,
}

pub fn declaration_path(name: &str) -> String {
    format!(".yidam/sangha/dispatch/{name}.toml")
}

pub fn position_path(name: &str, question: &str) -> String {
    format!(".yidam/sangha/positions/{name}-{question}.md")
}

pub fn commitments_path(name: &str) -> String {
    format!(".yidam/sangha/commitments/{name}.md")
}

pub fn branch_for(name: &str, question: &str, short_tip: &str) -> String {
    format!("propose/elector/{name}/{question}/{short_tip}")
}

fn valid_slug(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        && !s.starts_with('-')
}

/// What the `Config` column is a hash of: the argv and the configuration files, by digest.
///
/// **Model and version are not in it.** They have columns of their own, and folding them in would
/// make a model bump change three columns where it changed two — a `Config` that moved for a
/// reason the configuration did not.
#[derive(Serialize)]
struct Operative<'a> {
    run: &'a [String],
    files: Vec<&'a File>,
}

/// The configuration digest, over the materialized files the `config` globs select.
///
/// A glob that selects nothing is refused rather than contributing nothing: a declaration naming a
/// system prompt the tree does not hold would otherwise hash to the same value as one that names
/// no prompt at all, and two different occupants would read as one configuration.
pub(crate) fn config_digest(decl: &Declaration, files: &[File]) -> Result<String> {
    for glob in &decl.config {
        if !files
            .iter()
            .any(|f| crate::kuten::glob_covers(glob, &f.path))
        {
            bail!(
                "`config` names `{glob}`, which covers nothing the elector is handed — a \
                 configuration file must also be in `reads`"
            );
        }
    }
    let selected: Vec<&File> = files
        .iter()
        .filter(|f| {
            decl.config
                .iter()
                .any(|g| crate::kuten::glob_covers(g, &f.path))
        })
        .collect();
    let text = serde_yaml::to_string(&Operative {
        run: &decl.run,
        files: selected,
    })
    .context("serializing the operative configuration")?;
    Ok(receipt::sha256(text.as_bytes()))
}

/// Registry first: why this row does not already describe the occupant the declaration would run.
///
/// Every column must be filled in and must agree. A blank is refused rather than filled by the
/// run, because the registry is what `independence:` is derived from and a run that wrote it would
/// be an occupant describing itself.
pub(crate) fn row_disagrees(row: &ElectorRow, decl: &Declaration, digest: &str) -> Option<String> {
    let mut why = Vec::new();
    for (column, cell, declared) in [
        ("Model", row.model.trim(), decl.model.as_str()),
        ("Version", row.version.trim(), decl.version.as_str()),
    ] {
        if cell.is_empty() {
            why.push(format!(
                "`{column}` is blank and the declaration runs `{declared}`"
            ));
        } else if cell != declared {
            why.push(format!(
                "`{column}` is `{cell}` and the declaration runs `{declared}`"
            ));
        }
    }
    let cell = row.config.trim();
    if cell.is_empty() {
        why.push(format!(
            "`Config` is blank and the declaration hashes to `{digest}`"
        ));
    } else if !config_matches(cell, digest) {
        why.push(format!(
            "`Config` is `{cell}` and the declaration hashes to `{digest}`"
        ));
    }
    (!why.is_empty()).then(|| {
        format!(
            "the registry row at this tip does not describe what would run: {} — commit a \
             registration update to `{REGISTRY}` on `{}` first; the registry is read, the run \
             only corroborates it",
            why.join("; "),
            row.branch
        )
    })
}

// ── the report ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Outcome {
    /// Ran, and its position is on a `propose/elector/*` branch.
    Proposed,
    /// Ran, and the proposal branch — or the seat itself — already holds exactly this.
    Unchanged,
    /// Ran under `--dry-run`; nothing written.
    DryRun,
    /// Not run: no branch, no row, a human seat, no declaration, or a row that does not
    /// describe what would run.
    Refused,
    /// Ran, and exited non-zero, wrote outside its declaration, or wrote no position.
    Failed,
}

impl Outcome {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Proposed => "proposed",
            Self::Unchanged => "unchanged",
            Self::DryRun => "dry-run",
            Self::Refused => "refused",
            Self::Failed => "failed",
        }
    }

    fn ran(self) -> bool {
        matches!(self, Self::Proposed | Self::Unchanged | Self::DryRun)
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct SeatReport {
    pub seat: String,
    pub branch: String,
    pub outcome: Outcome,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// The seat's tip the run was read from, abbreviated.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tip: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config: Option<String>,
    /// `open` or `revise`, by whether the seat already held a position on this question.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verb: Option<String>,
    /// Where the position was written, repository-relative.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<String>,
    /// The proposal branch, where one was written or already held this.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proposal: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commit: Option<String>,
    /// The elector's stderr, where it said anything.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stderr: Option<String>,
    /// The row the run was checked against. Kept for the derivation, not reported.
    #[serde(skip)]
    row: Option<ElectorRow>,
}

impl SeatReport {
    fn new(seat: &str) -> Self {
        Self {
            seat: seat.to_string(),
            branch: format!("ma/{seat}"),
            outcome: Outcome::Refused,
            reason: None,
            tip: None,
            model: None,
            version: None,
            config: None,
            verb: None,
            position: None,
            proposal: None,
            commit: None,
            stderr: None,
            row: None,
        }
    }

    fn refused(mut self, reason: impl Into<String>) -> Self {
        self.outcome = Outcome::Refused;
        self.reason = Some(reason.into());
        self
    }

    fn failed(mut self, reason: impl Into<String>) -> Self {
        self.outcome = Outcome::Failed;
        self.reason = Some(reason.into());
        self
    }
}

/// `independence:` over the seats that ran, as `yidam lint` would derive it from their rows.
#[derive(Debug, Clone, Serialize)]
pub struct IndependenceReport {
    pub value: &'static str,
    pub because: String,
}

#[derive(Debug, Serialize)]
pub struct DispatchReport {
    /// Always `dispatch`.
    pub kind: &'static str,
    pub question: String,
    pub seats: Vec<SeatReport>,
    /// `null` when no seat ran.
    pub independence: Option<IndependenceReport>,
    pub dry_run: bool,
}

impl DispatchReport {
    /// Whether every seat named was run. A refusal is a seat the person asked for that did not
    /// answer, so it fails the command as a crash would.
    pub fn passed(&self) -> bool {
        self.seats.iter().all(|s| s.outcome.ran())
    }
}

// ── reading the tip ───────────────────────────────────────────────────────────

/// A file's bytes at a revision, untrimmed — the declaration's digest is of the file as
/// committed, so the terminal that trims stdout is the wrong one here.
fn show(root: &Path, rev: &str, path: &str) -> Option<String> {
    let out = crate::git::Git::new(root)
        .args(["cat-file", "blob"])
        .rev(format!("{rev}:{path}"))
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

fn exists(root: &Path, rev: &str, path: &str) -> bool {
    crate::git::Git::new(root)
        .args(["cat-file", "-e"])
        .rev(format!("{rev}:{path}"))
        .succeeded()
}

fn tree_of(root: &Path, rev: &str) -> Option<String> {
    crate::git::Git::new(root)
        .args(["rev-parse", "--verify", "--quiet"])
        .rev(format!("{rev}^{{tree}}"))
        .try_run()
}

/// The ref a seat is read from — local where there is one, else remote-tracking.
fn seat_ref(root: &Path, name: &str) -> Option<String> {
    let branch = format!("ma/{name}");
    crate::git::phase_refs(root)
        .into_iter()
        .find(|r| r.name == branch)
        .map(|r| r.git_ref)
}

// ── one seat ──────────────────────────────────────────────────────────────────

/// Dispatch one seat. Never an `Err`: every way a seat does not answer is an outcome in the
/// report, because a dispatch that stopped at the first refusal would report fewer seats than
/// were named and read as a smaller sangha than was asked for.
pub(crate) fn dispatch_seat(root: &Path, question: &str, name: &str, opts: &Options) -> SeatReport {
    let report = SeatReport::new(name);
    if !valid_slug(name) {
        return report.refused(format!(
            "`{name}` is not a seat name — lowercase letters, digits and `-` only"
        ));
    }
    let Some(git_ref) = seat_ref(root, name) else {
        return report.refused(format!(
            "no `ma/{name}` branch — a seat is a branch and a row, and this one has no branch"
        ));
    };
    let Some(tip) = crate::git::Git::new(root)
        .args(["rev-parse", "--verify", "--quiet"])
        .rev(format!("{git_ref}^{{commit}}"))
        .try_run()
    else {
        return report.refused(format!("`{git_ref}` does not name a commit"));
    };
    match prepare(root, question, name, &tip, report) {
        Ok(p) => run(root, question, name, &tip, p, opts),
        Err(r) => *r,
    }
}

/// Everything checked before the elector is invoked.
struct Prepared {
    report: SeatReport,
    decl: Declaration,
    decl_text: String,
    cap: Capability,
    inputs: exec::Inputs,
    digest: String,
}

fn prepare(
    root: &Path,
    question: &str,
    name: &str,
    tip: &str,
    mut report: SeatReport,
) -> std::result::Result<Prepared, Box<SeatReport>> {
    report.tip = Some(write::short_of(root, tip));
    let branch = report.branch.clone();

    let rows = show(root, tip, REGISTRY)
        .map(|t| parse_electors(&t))
        .unwrap_or_default();
    let Some(row) = rows.into_iter().find(|r| r.branch == branch) else {
        return Err(Box::new(report.refused(format!(
            "the registry at `{branch}`'s tip carries no row for it — register the seat first"
        ))));
    };
    if row.kind.eq_ignore_ascii_case("human") {
        return Err(Box::new(report.refused(
            "a human seat — a person answers for their own seat, and nothing dispatches one",
        )));
    }

    let path = declaration_path(name);
    let Some(decl_text) = show(root, tip, &path) else {
        return Err(Box::new(report.refused(format!(
            "no `{path}` at `{branch}`'s tip — the seat has not declared what runs it"
        ))));
    };
    let decl: Declaration = match toml::from_str(&decl_text) {
        Ok(d) => d,
        Err(e) => {
            return Err(Box::new(
                report.refused(format!("`{path}` does not parse: {e}")),
            ))
        }
    };
    if decl.run.is_empty() || decl.reads.is_empty() {
        return Err(Box::new(report.refused(format!(
            "`{path}` must name a `run` argv and at least one `reads` glob"
        ))));
    }
    report.model = Some(decl.model.clone());
    report.version = Some(decl.version.clone());

    let position = position_path(name, question);
    let cap = Capability {
        // A connector: its answer depends on something the repository does not hold — the
        // model — which is the case the connector kind exists to name.
        kind: Kind::Connector,
        run: Run::Argv(decl.run.clone()),
        reads: decl.reads.clone(),
        writes: vec![position.clone(), commitments_path(name)],
        verb: if exists(root, tip, &position) {
            "revise"
        } else {
            "open"
        }
        .to_string(),
        after: vec![],
        ageing_days: None,
        cluster: None,
    };
    let inputs = match exec::materialize(root, tip, &cap) {
        Ok(i) => i,
        Err(e) => return Err(Box::new(report.refused(format!("{path}: {e:#}")))),
    };
    let digest = match config_digest(&decl, &inputs.files) {
        Ok(d) => d,
        Err(e) => return Err(Box::new(report.refused(format!("{path}: {e:#}")))),
    };
    report.config = Some(digest.clone());
    if let Some(why) = row_disagrees(&row, &decl, &digest) {
        return Err(Box::new(report.refused(why)));
    }
    report.row = Some(row);
    Ok(Prepared {
        report,
        decl,
        decl_text,
        cap,
        inputs,
        digest,
    })
}

fn run(
    root: &Path,
    question: &str,
    name: &str,
    tip: &str,
    p: Prepared,
    opts: &Options,
) -> SeatReport {
    let Prepared {
        mut report,
        decl,
        decl_text,
        cap,
        inputs,
        digest,
    } = p;
    let position = position_path(name, question);
    let step = format!("elector/{name}");
    let produced = match exec::invoke_argv(
        &decl.run,
        &inputs,
        &step,
        tip,
        &[
            ("YIDAM_SEAT", name),
            ("YIDAM_QUESTION", question),
            ("YIDAM_POSITION", &position),
            ("YIDAM_MODEL", &decl.model),
            ("YIDAM_MODEL_VERSION", &decl.version),
            ("YIDAM_CONFIG", &digest),
        ],
    ) {
        Ok(p) => p,
        Err(e) => return report.failed(format!("{e:#}")),
    };
    if !produced.stderr.is_empty() {
        report.stderr = Some(produced.stderr.clone());
    }
    if let Err(e) = exec::check_declared(&cap, &produced.outputs) {
        return report.failed(format!("{e:#}"));
    }
    if !produced.outputs.iter().any(|(p, _)| p == &position) {
        return report.failed(format!(
            "the elector exited 0 and wrote no `{position}` — `$YIDAM_POSITION` names where its \
             position goes, under `$YIDAM_OUT`"
        ));
    }
    report.verb = Some(cap.verb.clone());
    report.position = Some(position);

    let receipt = match build_receipt(
        root,
        tip,
        &step,
        &decl,
        &decl_text,
        &cap,
        &inputs,
        &produced.outputs,
        &digest,
    ) {
        Ok(r) => r,
        Err(e) => return report.failed(format!("{e:#}")),
    };
    if opts.dry_run {
        report.outcome = Outcome::DryRun;
        return report;
    }
    match land(
        root,
        name,
        question,
        tip,
        &cap.verb,
        &produced.outputs,
        &receipt,
        report.row.as_ref().is_some_and(|r| !r.key.is_empty()),
        opts.force,
    ) {
        Ok(l) => {
            report.outcome = if l.unchanged {
                Outcome::Unchanged
            } else {
                Outcome::Proposed
            };
            report.proposal = Some(l.branch);
            report.commit = l.commit;
            report
        }
        Err(e) => report.failed(format!("{e:#}")),
    }
}

#[allow(clippy::too_many_arguments)]
fn build_receipt(
    root: &Path,
    tip: &str,
    step: &str,
    decl: &Declaration,
    decl_text: &str,
    cap: &Capability,
    inputs: &exec::Inputs,
    outputs: &[(String, Vec<u8>)],
    digest: &str,
) -> Result<Receipt> {
    // The declaration is this run's manifest, and the corpus config is read at the same tip as
    // everything else.
    let manifest_sha256 = receipt::sha256(decl_text.as_bytes());
    let config_sha256 = receipt::sha256(
        show(root, tip, ".yidam/config.toml")
            .unwrap_or_default()
            .as_bytes(),
    );
    let resolved = inputs.resolved.as_ref().map(|r| r.sha256.as_str());
    let input_state = Receipt::input_state(
        cap,
        &manifest_sha256,
        &config_sha256,
        &inputs.files,
        resolved,
        None,
    )?;
    Ok(Receipt {
        format_version: receipt::FORMAT_VERSION,
        step: step.to_string(),
        kind: "elector",
        verb: cap.verb.clone(),
        run: cap.run.clone(),
        input_state,
        input: Input {
            commit: tip.to_string(),
            manifest_sha256,
            config_sha256,
            reads: cap.reads.clone(),
            files: inputs.files.clone(),
            resolved_graph_sha256: resolved.map(str::to_string),
            script_sha256: None,
        },
        writes: cap.writes.clone(),
        outputs: outputs
            .iter()
            .map(|(path, bytes)| File {
                path: path.clone(),
                sha256: receipt::sha256(bytes),
            })
            .collect(),
        model: Some(decl.model.clone()),
        version: Some(decl.version.clone()),
        config: Some(digest.to_string()),
        image_digest: None,
    })
}

// ── landing ───────────────────────────────────────────────────────────────────

struct Landed {
    branch: String,
    commit: Option<String>,
    unchanged: bool,
}

/// How a seat takes a proposed position. This commit is authored by the tool and unsigned, so a
/// seat whose row binds a key cannot fast-forward onto it: its tip would carry no signature and
/// `elector-signature-unverified` would gate on it. It re-commits the same tree and message under
/// its own key instead — the proposal is parented on the tip, so the pick is exact.
fn accept_command(keyed: bool, branch: &str) -> String {
    if keyed {
        format!("git cherry-pick -S {branch}")
    } else {
        format!("git merge --ff-only {branch}")
    }
}

fn accept_how(keyed: bool) -> &'static str {
    if keyed {
        "signs it onto the branch — this row binds a key and this commit is unsigned"
    } else {
        "fast-forwards it"
    }
}

#[allow(clippy::too_many_arguments)]
fn land(
    root: &Path,
    name: &str,
    question: &str,
    tip: &str,
    verb: &str,
    outputs: &[(String, Vec<u8>)],
    receipt: &Receipt,
    keyed: bool,
    force: bool,
) -> Result<Landed> {
    let short = write::short_of(root, tip);
    let branch = branch_for(name, question, &short);
    let scratch = TempIndex::new(root, "dispatch")?;
    let index = scratch.path().to_path_buf();
    write::git(root, Some(&index), &["read-tree", tip], None)?;

    let receipt_text = receipt.to_yaml()?;
    let mut files: Vec<(String, String)> = Vec::new();
    for (path, bytes) in outputs {
        let text = String::from_utf8(bytes.clone())
            .with_context(|| format!("the elector wrote {path}, which is not UTF-8"))?;
        files.push((path.clone(), text));
    }
    files.push((receipt_path(name), receipt_text));
    for (path, content) in &files {
        let blob = write::git(
            root,
            Some(&index),
            &["hash-object", "-w", "--stdin"],
            Some(content),
        )?;
        write::git(
            root,
            Some(&index),
            &[
                "update-index",
                "--add",
                "--cacheinfo",
                &format!("100644,{blob},{path}"),
            ],
            None,
        )?;
    }
    let tree = write::git(root, Some(&index), &["write-tree"], None)?;

    let existing = tree_of(root, &format!("refs/heads/{branch}"));
    if tree_of(root, tip).as_deref() == Some(&tree) || existing.as_deref() == Some(&tree) {
        return Ok(Landed {
            branch,
            commit: None,
            unchanged: true,
        });
    }
    if existing.is_some() && !force {
        bail!(
            "{branch} already exists and holds a different position — the elector answered \
             differently this time, or the branch was edited. Review it with `git log -p \
             ma/{name}..{branch}`, delete it, or re-run with --force to replace it."
        );
    }

    let message = format!(
        "{verb}: {name} on {question}\n\nDispatched by `yidam dispatch {question} --seat {name}`: \
         {position}\n\nProposed, not held. It stands on ma/{name} only once whoever answers for \
         the seat {how}:\n\n    git switch ma/{name} && {accept}\n\n\
         Seat: ma/{name}\nModel: {model}\nVersion: {version}\nConfig: {config}\nProposed-from: \
         {short}\n",
        how = accept_how(keyed),
        accept = accept_command(keyed, &branch),
        position = position_path(name, question),
        model = receipt.model.as_deref().unwrap_or_default(),
        version = receipt.version.as_deref().unwrap_or_default(),
        config = receipt.config.as_deref().unwrap_or_default(),
    );
    let sha = write::commit_tree(root, &tree, tip, &message, (AUTHOR_NAME, AUTHOR_EMAIL))?;
    write::git(
        root,
        None,
        &["update-ref", &format!("refs/heads/{branch}"), &sha],
        None,
    )?;
    Ok(Landed {
        branch,
        commit: Some(write::short_of(root, &sha)),
        unchanged: false,
    })
}

// ── the command ───────────────────────────────────────────────────────────────

pub(crate) fn run_all(root: &Path, opts: &Options) -> Result<DispatchReport> {
    if !valid_slug(&opts.question) {
        bail!(
            "`{}` is not a question slug — lowercase letters, digits and `-` only; it names \
             `positions/<elector>-<question>.md`",
            opts.question
        );
    }
    if opts.seats.is_empty() {
        bail!("name at least one seat with --seat <name>");
    }
    let mut seen = std::collections::BTreeSet::new();
    let seats: Vec<SeatReport> = opts
        .seats
        .iter()
        .filter(|s| seen.insert(s.as_str()))
        .map(|s| dispatch_seat(root, &opts.question, s, opts))
        .collect();

    let at_tips: Vec<SeatAtTip> = seats
        .iter()
        .filter(|s| s.outcome.ran())
        .map(|s| SeatAtTip {
            branch: s.branch.clone(),
            tip: format!(
                "{}@{}",
                s.branch,
                s.commit.as_deref().or(s.tip.as_deref()).unwrap_or_default()
            ),
            row: s.row.clone(),
        })
        .collect();
    let independence = (!at_tips.is_empty()).then(|| {
        let d = derive(&at_tips);
        IndependenceReport {
            value: d.value.as_str(),
            because: d.because,
        }
    });
    Ok(DispatchReport {
        kind: "dispatch",
        question: opts.question.clone(),
        seats,
        independence,
        dry_run: opts.dry_run,
    })
}

pub fn dispatch(opts: Options) -> Result<()> {
    let root = repo_root()?;
    require_yidam_repo(&root)?;
    let report = run_all(&root, &opts)?;
    let passed = report.passed();
    crate::report::gate(&root, opts.format, report, passed, |r| {
        println!("{}", render(r));
    })
}

pub fn render(r: &DispatchReport) -> String {
    let mut out = format!("dispatch {}\n", r.question);
    for s in &r.seats {
        let _ = write!(out, "\n  {:<10} {}", s.outcome.as_str(), s.branch);
        if let Some(tip) = &s.tip {
            let _ = write!(out, "@{tip}");
        }
        if let (Some(m), Some(v)) = (&s.model, &s.version) {
            let _ = write!(out, "  {m} {v}");
        }
        if let Some(c) = &s.config {
            let _ = write!(out, " config {}", &c[..c.len().min(12)]);
        }
        if let Some(p) = &s.proposal {
            let _ = write!(
                out,
                "\n             {} {}",
                s.verb.as_deref().unwrap_or("open"),
                p
            );
        }
        if let Some(reason) = &s.reason {
            let _ = write!(out, "\n             {reason}");
        }
    }
    out.push('\n');
    if let Some(i) = &r.independence {
        let _ = write!(out, "\n  independence: {} — {}", i.value, i.because);
    }
    if r.dry_run {
        out.push_str("\n  dry run — nothing written");
    } else if r.seats.iter().any(|s| s.outcome == Outcome::Proposed) {
        out.push_str(
            "\n  proposed, not held — a seat takes its position when whoever answers for it \
             runs `git switch ma/<seat> && git merge --ff-only <proposal>`, or \
             `git cherry-pick -S <proposal>` for a seat whose row binds a key",
        );
    }
    out
}
