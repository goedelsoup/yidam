//! `cluster status` — what the recent runs of a corpus did, read from records (#1236).
//!
//! An operator asking *"did last night's run land anything, and if not, why not?"* had three
//! places to look: the Argo UI, pod logs, or `git log` on the remote. None shows admitted,
//! refused and landed in one place, with the refusal's reason.
//!
//! # Where the records are
//!
//! Every pod writes its record to a file, and Argo reads that file into the node's `output`
//! parameter. That parameter is kept in the `Workflow` object's status, beside the phase Argo
//! gave the node. So the records of a run are in its `Workflow`, for as long as the workflow
//! exists. They are not in the vault, which holds bundles by digest and cannot be listed.
//!
//! This reads `kubectl get workflows,cronworkflows -o json`, or a file holding that output.
//! It never reads a log. When a record is missing, the outcome is `unknown` and the reason
//! names what is missing. A run whose `Workflow` was deleted is seen only through its
//! `CronWorkflow`'s last scheduled time, and it is reported as unknown for that reason.
//!
//! This is operational status, not provenance. Nothing it prints is committed, and nothing it
//! reads is cited. The record of a run is the commit the lander pushed and its receipt.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context, Result};
use serde::Serialize;
use serde_json::Value;

use super::egress::CORPUS_LABEL;
use super::{Admission, Landed, StepOutcome, StepOutput};
use crate::cmd::gather::Outcome as PeerOutcome;
use crate::git::Git;
use crate::paths::{repo_root, require_yidam_repo};
use crate::report::Format;

/// The label Argo puts on every `Workflow` a `CronWorkflow` creates.
const CRON_LABEL: &str = "workflows.argoproj.io/cron-workflow";
/// The annotation holding the tick a cron-created `Workflow` was scheduled for.
const SCHEDULED: &str = "workflows.argoproj.io/scheduled-time";

pub(super) struct Overrides {
    pub workflows: Option<PathBuf>,
    pub namespace: Option<String>,
    pub context: Option<String>,
    pub remote: Option<String>,
    pub branch: Option<String>,
    pub corpus: Option<String>,
    pub limit: usize,
}

/// How long Argo keeps a finished `Workflow`, in seconds.
#[derive(Debug, Clone, Copy)]
pub(super) struct Retention {
    pub after_success: u64,
    pub after_failure: u64,
}

// ── the report ────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub(super) struct StatusReport {
    pub corpus: String,
    /// Newest first.
    pub runs: Vec<RunStatus>,
    /// The branch and the proposal branches on the remote. `None` when no remote was named.
    pub refs: Option<Refs>,
}

#[derive(Debug, Serialize)]
pub(super) struct RunStatus {
    /// The `Workflow`'s name. `None` for a run whose `Workflow` no longer exists.
    pub workflow: Option<String>,
    /// Argo's phase for the run, or `unknown`.
    pub phase: String,
    /// The cron tick the run was created for. Absent for a run created any other way.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scheduled: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finished: Option<String>,
    pub admission: AdmissionStatus,
    /// In the order the workflow runs them.
    pub steps: Vec<StepStatus>,
    pub gathers: Vec<GatherStatus>,
    /// Why nothing below the run is known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unknown: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum AdmissionState {
    Admitted,
    NotAdmitted,
    /// A one-shot `Workflow`, which has no `admit` task.
    NotAsked,
    /// `admit` has not finished.
    Running,
    Unknown,
}

#[derive(Debug, Serialize)]
pub(super) struct AdmissionStatus {
    pub state: AdmissionState,
    /// The admission record's own `because`, or why there is none.
    pub because: String,
    /// The `due` clocks that were owed, by id.
    pub owed: Vec<String>,
    /// The steps whose receipt did not match the tip.
    pub stale: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub open_proposals: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_open_proposals: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum StepState {
    /// An operational commit, on the branch.
    Landed,
    /// An epistemic commit, on `propose/*`.
    Proposed,
    /// Its receipt matched its input, so it was not invoked.
    Fresh,
    /// Invoked, and it built the tree it was given.
    Unchanged,
    /// The lander refused the commit, and its record says why.
    Refused,
    /// A pod failed without a record.
    Failed,
    Running,
    NotReached,
    Unknown,
}

#[derive(Debug, Serialize)]
pub(super) struct StepStatus {
    pub step: String,
    pub outcome: StepState,
    /// The ref the commit landed on.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    /// The commit that landed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commit: Option<String>,
    /// For a proposal, whether its branch is still on the remote. Absent when the refs were
    /// not read, or the step proposed nothing.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub still_open: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Debug, Serialize)]
pub(super) struct GatherStatus {
    pub gather: String,
    /// The gather's landing, read the way a step's is.
    #[serde(flatten)]
    pub landing: StepStatus,
    /// Every peer the survey decided on or an asker answered for.
    pub peers: Vec<PeerStatus>,
}

#[derive(Debug, Serialize)]
pub(super) struct PeerStatus {
    pub peer: String,
    /// The gather's own outcome word, or `unknown` for an asker that left no record.
    pub outcome: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct Refs {
    pub remote: String,
    pub branch: String,
    /// The branch's tip. `None` when the branch is not on the remote or it could not be read.
    pub tip: Option<String>,
    /// Every `propose/*` branch, without `refs/heads/`.
    pub proposals: Vec<String>,
    /// Why the refs could not be read.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unread: Option<String>,
}

// ── the command ───────────────────────────────────────────────────────────────

pub(super) fn run(o: &Overrides, format: Format) -> Result<()> {
    let root = repo_root()?;
    require_yidam_repo(&root)?;
    let cfg = crate::config::load_yidam_config(&root)?;
    let corpus = o
        .corpus
        .clone()
        .unwrap_or_else(|| super::workflow::slug(&root));
    let list = match &o.workflows {
        Some(path) => {
            let text = std::fs::read_to_string(path)
                .with_context(|| format!("reading {}", path.display()))?;
            serde_json::from_str(&text).with_context(|| {
                format!("{} is not `kubectl get -o json` output", path.display())
            })?
        }
        None => kubectl(
            o.namespace.as_deref().or(cfg.cluster.namespace.as_deref()),
            o.context.as_deref(),
        )?,
    };
    let retention = super::workflow::retention(&cfg.cluster.cleanup);
    let refs = o
        .remote
        .clone()
        .or(cfg.cluster.remote.clone())
        .map(|remote| {
            let branch = o.branch.clone().unwrap_or(cfg.cluster.branch.clone());
            read_refs(&root, &remote, &branch)
        });
    let report = status_of(&list, &corpus, o.limit, retention, refs);
    crate::report::finish(&root, format, report, |r| print!("{}", render(r)))
}

/// Every `Workflow` and `CronWorkflow` in the namespace, as one `List`.
fn kubectl(namespace: Option<&str>, context: Option<&str>) -> Result<Value> {
    let mut cmd = Command::new("kubectl");
    if let Some(c) = context {
        cmd.args(["--context", c]);
    }
    if let Some(ns) = namespace {
        cmd.args(["--namespace", ns]);
    }
    cmd.args([
        "get",
        "workflows.argoproj.io,cronworkflows.argoproj.io",
        "-o",
        "json",
    ]);
    let out = cmd
        .output()
        .context("running kubectl; pass `--workflows <file>` to read its output from a file")?;
    if !out.status.success() {
        bail!(
            "kubectl get workflows failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    serde_json::from_slice(&out.stdout).context("kubectl's output is not JSON")
}

/// The branch's tip and the proposal branches, by `ls-remote`. A remote that cannot be read is
/// reported, not an error: the runs are still worth showing.
fn read_refs(root: &Path, remote: &str, branch: &str) -> Refs {
    let head = format!("refs/heads/{branch}");
    let listed = Git::new(root)
        .args(["ls-remote", "--heads", remote])
        .arg(&head)
        .arg("refs/heads/propose/*")
        .run();
    let mut refs = Refs {
        remote: remote.to_string(),
        branch: branch.to_string(),
        tip: None,
        proposals: vec![],
        unread: None,
    };
    match listed {
        Ok(text) => {
            for line in text.lines() {
                let Some((sha, name)) = line.split_once('\t') else {
                    continue;
                };
                if name == head {
                    refs.tip = Some(sha.to_string());
                } else if let Some(p) = name.strip_prefix("refs/heads/") {
                    refs.proposals.push(p.to_string());
                }
            }
        }
        Err(e) => refs.unread = Some(format!("{e:#}")),
    }
    refs
}

// ── reading the records ───────────────────────────────────────────────────────

/// The recent runs of `corpus` in `list`, newest first, with what each step did.
pub(super) fn status_of(
    list: &Value,
    corpus: &str,
    limit: usize,
    retention: Retention,
    refs: Option<Refs>,
) -> StatusReport {
    let cron_name = format!("yidam-{corpus}");
    let items = list["items"].as_array().map(Vec::as_slice).unwrap_or(&[]);
    let kind = |v: &Value| v["kind"].as_str().unwrap_or("").to_string();
    let label = |v: &Value, k: &str| v["metadata"]["labels"][k].as_str().map(str::to_string);

    let mut workflows: Vec<&Value> = items
        .iter()
        .filter(|v| kind(v) == "Workflow")
        .filter(|v| {
            label(v, CORPUS_LABEL).as_deref() == Some(corpus)
                || label(v, CRON_LABEL).as_deref() == Some(cron_name.as_str())
        })
        .collect();
    workflows.sort_by_key(|w| std::cmp::Reverse(began(w)));

    let mut runs = Vec::new();
    let cron = items
        .iter()
        .find(|v| kind(v) == "CronWorkflow" && v["metadata"]["name"] == cron_name.as_str());
    if let Some(last) = cron.and_then(|c| c["status"]["lastScheduledTime"].as_str()) {
        let kept = workflows.iter().any(|w| {
            label(w, CRON_LABEL).as_deref() == Some(cron_name.as_str())
                && (w["metadata"]["annotations"][SCHEDULED].as_str() == Some(last)
                    || w["metadata"]["creationTimestamp"]
                        .as_str()
                        .is_some_and(|t| t >= last))
        });
        if !kept {
            runs.push(deleted(last, &cron_name, retention));
        }
    }
    let refs_ref = refs.as_ref();
    runs.extend(
        workflows
            .into_iter()
            .take(limit)
            .map(|w| run_status(w, refs_ref)),
    );
    StatusReport {
        corpus: corpus.to_string(),
        runs,
        refs,
    }
}

fn began(w: &Value) -> String {
    w["status"]["startedAt"]
        .as_str()
        .or(w["metadata"]["creationTimestamp"].as_str())
        .unwrap_or("")
        .to_string()
}

fn deleted(scheduled: &str, cron: &str, r: Retention) -> RunStatus {
    let reason = format!(
        "`{cron}` last created a run for {scheduled}, and no Workflow for it remains. Argo \
         deletes a run {}s after it succeeds and {}s after it fails ([cluster.cleanup]), and a \
         run deleted by hand looks the same",
        r.after_success, r.after_failure
    );
    RunStatus {
        workflow: None,
        phase: "unknown".into(),
        scheduled: Some(scheduled.to_string()),
        started: None,
        finished: None,
        admission: AdmissionStatus::without(AdmissionState::Unknown, reason.clone()),
        steps: vec![],
        gathers: vec![],
        unknown: Some(reason),
    }
}

impl AdmissionStatus {
    fn without(state: AdmissionState, because: String) -> Self {
        Self {
            state,
            because,
            owed: vec![],
            stale: vec![],
            open_proposals: None,
            max_open_proposals: None,
        }
    }
}

/// The nodes of a run, by the display name Argo gives a task.
struct Nodes<'a> {
    all: Vec<&'a Value>,
    running: bool,
}

impl<'a> Nodes<'a> {
    /// The node for a top-level task. A retried task is a `Retry` node with one child per
    /// attempt, and the parent carries the outputs, so the exact name is the one to read.
    fn task(&self, name: &str) -> Option<&'a Value> {
        self.all.iter().copied().find(|n| n["displayName"] == name)
    }

    /// The nodes a fan-out task made, one per item.
    fn children(&self, name: &str) -> Vec<&'a Value> {
        let Some(group) = self.task(name) else {
            return vec![];
        };
        let ids: Vec<&str> = group["children"]
            .as_array()
            .map(|c| c.iter().filter_map(Value::as_str).collect())
            .unwrap_or_default();
        ids.iter()
            .filter_map(|id| self.all.iter().copied().find(|n| n["id"] == *id))
            .collect()
    }
}

fn phase(node: &Value) -> &str {
    node["phase"].as_str().unwrap_or("")
}

/// The record a pod wrote, as Argo holds it: the `output` parameter's value.
fn record(node: &Value) -> Option<&str> {
    node["outputs"]["parameters"]
        .as_array()?
        .iter()
        .find(|p| p["name"] == "output")?["value"]
        .as_str()
}

/// A named parameter's value from a `{parameters: [{name, value}]}` object: a DAG task's
/// `arguments`, or a node's `inputs`.
fn param<'a>(params: &'a Value, name: &str) -> Option<&'a str> {
    params["parameters"]
        .as_array()?
        .iter()
        .find(|p| p["name"] == name)?["value"]
        .as_str()
}

/// Argo's message for a node, which is not a log line: `Error (exit code 1)` and the like.
fn message(node: &Value) -> String {
    match node["message"].as_str() {
        Some(m) if !m.is_empty() => format!(" ({m})"),
        _ => String::new(),
    }
}

fn run_status(w: &Value, refs: Option<&Refs>) -> RunStatus {
    let s = |v: &Value| v.as_str().map(str::to_string);
    let mut run = RunStatus {
        workflow: s(&w["metadata"]["name"]),
        phase: s(&w["status"]["phase"]).unwrap_or_else(|| "Pending".into()),
        scheduled: s(&w["metadata"]["annotations"][SCHEDULED]),
        started: s(&w["status"]["startedAt"]),
        finished: s(&w["status"]["finishedAt"]),
        admission: AdmissionStatus::without(AdmissionState::Unknown, String::new()),
        steps: vec![],
        gathers: vec![],
        unknown: None,
    };

    // A cron's or a sensor's run carries the whole spec; Argo also stores a copy in status.
    let tasks = [
        &w["spec"]["templates"],
        &w["status"]["storedWorkflowTemplateSpec"]["templates"],
    ]
    .into_iter()
    .filter_map(Value::as_array)
    .flatten()
    .find(|t| t["name"] == "run")
    .and_then(|t| t["dag"]["tasks"].as_array());
    let Some(tasks) = tasks else {
        let why = "the Workflow carries no `run` DAG, so its tasks cannot be named".to_string();
        run.admission.because = why.clone();
        run.unknown = Some(why);
        return run;
    };

    let all: Vec<&Value> = match w["status"]["nodes"].as_object() {
        Some(nodes) => nodes.values().collect(),
        None if w["status"]["offloadNodeStatusVersion"].is_string() => {
            let why = "Argo offloaded this run's node status to its persistence database, \
                       which `kubectl` does not read"
                .to_string();
            run.admission.because = why.clone();
            run.unknown = Some(why);
            return run;
        }
        None => vec![],
    };
    let nodes = Nodes {
        all,
        running: matches!(run.phase.as_str(), "Pending" | "Running"),
    };

    run.admission = admission(tasks, &nodes);
    for t in tasks {
        let name = t["name"].as_str().unwrap_or("");
        let template = t["template"].as_str().unwrap_or("");
        if template.starts_with("step") {
            let Some(task) = name.strip_prefix("step-") else {
                continue;
            };
            let step = param(&t["arguments"], "step").unwrap_or(task).to_string();
            run.steps.push(step_status(
                step,
                &nodes,
                &format!("step-{task}"),
                &format!("land-{task}"),
                refs,
            ));
        } else if template == "survey" {
            let Some(task) = name.strip_prefix("survey-") else {
                continue;
            };
            let gather = param(&t["arguments"], "gather").unwrap_or(task).to_string();
            run.gathers.push(gather_status(gather, task, &nodes, refs));
        }
    }
    run
}

fn admission(tasks: &[Value], nodes: &Nodes) -> AdmissionStatus {
    if !tasks.iter().any(|t| t["name"] == "admit") {
        return AdmissionStatus::without(
            AdmissionState::NotAsked,
            "a one-shot Workflow has no `admit` task; it runs when it is created".into(),
        );
    }
    let Some(node) = nodes.task("admit") else {
        return match nodes.running {
            true => {
                AdmissionStatus::without(AdmissionState::Running, "admit has not started".into())
            }
            false => AdmissionStatus::without(
                AdmissionState::Unknown,
                "the run has no `admit` node".into(),
            ),
        };
    };
    match record(node).map(serde_json::from_str::<Admission>) {
        Some(Ok(a)) => AdmissionStatus {
            state: match a.admitted {
                true => AdmissionState::Admitted,
                false => AdmissionState::NotAdmitted,
            },
            because: a.because,
            owed: a.owed,
            stale: a.stale,
            open_proposals: Some(a.open_proposals),
            max_open_proposals: a.max_open_proposals,
        },
        Some(Err(e)) => AdmissionStatus::without(
            AdmissionState::Unknown,
            format!("admit's record is not an admission record: {e}"),
        ),
        None if matches!(phase(node), "Pending" | "Running") => {
            AdmissionStatus::without(AdmissionState::Running, "admit is running".into())
        }
        None => AdmissionStatus::without(
            AdmissionState::Unknown,
            format!(
                "admit ended {} and left no record{}",
                phase(node),
                message(node)
            ),
        ),
    }
}

/// A lander's record when it refuses: the step, its input, and the reason.
#[derive(serde::Deserialize)]
struct Refusal {
    refused: String,
}

fn step_status(
    step: String,
    nodes: &Nodes,
    step_task: &str,
    land_task: &str,
    refs: Option<&Refs>,
) -> StepStatus {
    let mut out = StepStatus {
        step,
        outcome: StepState::NotReached,
        target: None,
        commit: None,
        still_open: None,
        reason: None,
    };
    let stepped = nodes.task(step_task);
    let step_record = stepped
        .and_then(record)
        .and_then(|r| serde_json::from_str::<StepOutput>(r).ok());

    if let Some(land) = nodes.task(land_task) {
        if let Some(text) = record(land) {
            if let Ok(r) = serde_json::from_str::<Refusal>(text) {
                out.outcome = StepState::Refused;
                out.reason = Some(r.refused);
                return out;
            }
            match serde_json::from_str::<Landed>(text) {
                Ok(l) => {
                    match (l.landed, l.target) {
                        // `landed` is absent when the ref already held the result, as a
                        // proposal does when the same step is landed twice.
                        (landed, Some(target)) => {
                            out.outcome = match target.starts_with("propose/") {
                                true => StepState::Proposed,
                                false => StepState::Landed,
                            };
                            if out.outcome == StepState::Proposed {
                                out.still_open = refs
                                    .filter(|r| r.unread.is_none())
                                    .map(|r| r.proposals.contains(&target));
                            }
                            out.commit = landed;
                            out.target = Some(target);
                        }
                        _ => match step_record.map(|s| s.outcome) {
                            Some(StepOutcome::Fresh) => out.outcome = StepState::Fresh,
                            Some(StepOutcome::Unchanged) => out.outcome = StepState::Unchanged,
                            _ => {
                                out.outcome = StepState::Unknown;
                                out.reason = Some(
                                    "the lander landed nothing and the step's record does not \
                                     say why"
                                        .into(),
                                );
                            }
                        },
                    }
                }
                Err(e) => {
                    out.outcome = StepState::Unknown;
                    out.reason = Some(format!("the lander's record is not a landing record: {e}"));
                }
            }
            return out;
        }
        return from_phase(out, land, "the lander", nodes.running);
    }
    match stepped {
        None => out,
        Some(node) if phase(node) == "Succeeded" => match nodes.running {
            true => StepStatus {
                outcome: StepState::Running,
                ..out
            },
            false => StepStatus {
                outcome: StepState::Unknown,
                reason: Some("the step succeeded and no landing task was created".into()),
                ..out
            },
        },
        Some(node) => from_phase(out, node, "the step", nodes.running),
    }
}

/// The outcome of a task whose node left no record, from the phase Argo gave it.
fn from_phase(mut out: StepStatus, node: &Value, who: &str, running: bool) -> StepStatus {
    let p = phase(node);
    out.outcome = match p {
        "Pending" | "Running" => StepState::Running,
        "Skipped" | "Omitted" => StepState::NotReached,
        "Failed" | "Error" => {
            out.reason = Some(format!("{who} failed and left no record{}", message(node)));
            StepState::Failed
        }
        "" if running => StepState::Running,
        _ => {
            out.reason = Some(format!(
                "{who} ended {p} and left no record{}",
                message(node)
            ));
            StepState::Unknown
        }
    };
    out
}

fn gather_status(gather: String, task: &str, nodes: &Nodes, refs: Option<&Refs>) -> GatherStatus {
    let mut peers = Vec::new();
    if let Some(survey) = nodes
        .task(&format!("survey-{task}"))
        .and_then(record)
        .and_then(|r| serde_json::from_str::<super::gather::Survey>(r).ok())
    {
        for p in survey.peers {
            peers.push(PeerStatus {
                peer: p.package,
                outcome: p.outcome.as_str().to_string(),
                reason: p.reason,
            });
        }
    }
    for child in nodes.children(&format!("ask-{task}")) {
        match record(child).map(serde_json::from_str::<super::gather::AskOutput>) {
            Some(Ok(a)) => peers.push(PeerStatus {
                peer: a.peer,
                outcome: a.outcome.as_str().to_string(),
                reason: None,
            }),
            _ => {
                let peer = param(&child["inputs"], "ask")
                    .and_then(|a| serde_json::from_str::<Value>(a).ok())
                    .and_then(|a| a["peer"].as_str().map(str::to_string))
                    .unwrap_or_else(|| child["displayName"].as_str().unwrap_or("?").to_string());
                peers.push(PeerStatus {
                    peer,
                    outcome: "unknown".into(),
                    reason: Some(format!(
                        "the asker ended {} and left no record{}; `gather` counts it {}",
                        phase(child),
                        message(child),
                        PeerOutcome::Refused.as_str()
                    )),
                });
            }
        }
    }
    peers.sort_by(|a, b| a.peer.cmp(&b.peer));
    GatherStatus {
        landing: step_status(
            gather.clone(),
            nodes,
            &format!("gather-{task}"),
            &format!("land-gather-{task}"),
            refs,
        ),
        gather,
        peers,
    }
}

// ── text ──────────────────────────────────────────────────────────────────────

fn render(r: &StatusReport) -> String {
    let mut s = String::new();
    if r.runs.is_empty() {
        let _ = writeln!(s, "{}: no run found", r.corpus);
    }
    for run in &r.runs {
        let name = run.workflow.as_deref().unwrap_or("(deleted)");
        let when = run
            .started
            .as_deref()
            .or(run.scheduled.as_deref())
            .unwrap_or("not started");
        let _ = writeln!(s, "{name}  {}  {when}", run.phase);
        if let Some(why) = &run.unknown {
            let _ = writeln!(s, "  unknown: {why}\n");
            continue;
        }
        let a = &run.admission;
        let state = match a.state {
            AdmissionState::Admitted => "admitted",
            AdmissionState::NotAdmitted => "not admitted",
            AdmissionState::NotAsked => "not asked",
            AdmissionState::Running => "admitting",
            AdmissionState::Unknown => "admission unknown",
        };
        let _ = writeln!(s, "  {state}: {}", a.because);
        if !a.owed.is_empty() {
            let _ = writeln!(s, "    due: {}", a.owed.join(", "));
        }
        if !a.stale.is_empty() {
            let _ = writeln!(s, "    stale: {}", a.stale.join(", "));
        }
        let width = run
            .steps
            .iter()
            .map(|st| st.step.len())
            .chain(run.gathers.iter().map(|g| g.gather.len() + 7))
            .max()
            .unwrap_or(0);
        for st in &run.steps {
            let _ = writeln!(s, "  {:width$}  {}", st.step, outcome(st));
        }
        for g in &run.gathers {
            let label = format!("gather {}", g.gather);
            let _ = writeln!(s, "  {label:width$}  {}", outcome(&g.landing));
            for p in &g.peers {
                let _ = match &p.reason {
                    Some(why) => writeln!(s, "    {}: {} — {why}", p.peer, p.outcome),
                    None => writeln!(s, "    {}: {}", p.peer, p.outcome),
                };
            }
        }
        s.push('\n');
    }
    match &r.refs {
        None => s.push_str("refs: not read (no --remote and no [cluster] remote)\n"),
        Some(refs) => match &refs.unread {
            Some(why) => {
                let _ = writeln!(s, "refs on {}: not read — {why}", refs.remote);
            }
            None => {
                let _ = writeln!(
                    s,
                    "refs on {}: {} at {}, {} proposal branch{} open",
                    refs.remote,
                    refs.branch,
                    refs.tip.as_deref().map_or("(absent)", short),
                    refs.proposals.len(),
                    if refs.proposals.len() == 1 { "" } else { "es" }
                );
            }
        },
    }
    s
}

fn short(sha: &str) -> &str {
    &sha[..sha.len().min(10)]
}

fn outcome(st: &StepStatus) -> String {
    let at = |verb: &str| {
        let target = st.target.as_deref().unwrap_or("?");
        match &st.commit {
            Some(sha) => format!("{verb} on {target} at {}", short(sha)),
            None => format!("{verb} on {target}, which already held it"),
        }
    };
    let mut line = match st.outcome {
        StepState::Landed => at("landed"),
        StepState::Proposed => {
            let open = match st.still_open {
                Some(true) => " (open)",
                Some(false) => " (gone from the remote)",
                None => "",
            };
            at("proposed") + open
        }
        StepState::Fresh => "fresh".into(),
        StepState::Unchanged => "unchanged".into(),
        StepState::Refused => "refused".into(),
        StepState::Failed => "failed".into(),
        StepState::Running => "running".into(),
        StepState::NotReached => "not reached".into(),
        StepState::Unknown => "unknown".into(),
    };
    if let Some(why) = &st.reason {
        let _ = write!(line, ": {why}");
    }
    line
}

#[cfg(test)]
mod tests;
