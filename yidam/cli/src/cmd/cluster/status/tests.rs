//! `status_of` over `Workflow` objects shaped as Argo writes them, one per outcome.

use serde_json::{json, Value};

use super::*;

const SHA: &str = "1111111111111111111111111111111111111111";
const PIN: &str = "2222222222222222222222222222222222222222";

const KEEP: Retention = Retention {
    after_success: 86_400,
    after_failure: 604_800,
};

/// A `Workflow` under construction: its DAG tasks, in order, and its nodes.
struct Wf {
    name: String,
    labels: Value,
    annotations: Value,
    phase: &'static str,
    started: &'static str,
    tasks: Vec<Value>,
    nodes: serde_json::Map<String, Value>,
}

impl Wf {
    fn new(name: &str, started: &'static str) -> Self {
        Self {
            name: name.into(),
            labels: json!({ "yidam.dev/corpus": "streamflow" }),
            annotations: json!({}),
            phase: "Succeeded",
            started,
            tasks: vec![],
            nodes: Default::default(),
        }
    }

    fn admitted(mut self, record: Value) -> Self {
        self.tasks
            .push(json!({ "name": "admit", "template": "admit" }));
        self.node("admit", "Pod", "Succeeded", Some(record), None)
    }

    fn step_task(mut self, step: &str) -> Self {
        self.tasks.push(json!({
            "name": format!("step-{step}"),
            "template": "step",
            "arguments": { "parameters": [{ "name": "step", "value": step }] },
        }));
        self.tasks
            .push(json!({ "name": format!("land-{step}"), "template": "land" }));
        self
    }

    fn node(
        mut self,
        display: &str,
        kind: &str,
        phase: &str,
        record: Option<Value>,
        message: Option<&str>,
    ) -> Self {
        let id = format!("{}-{}", self.name, self.nodes.len());
        let mut n = json!({ "id": id, "displayName": display, "type": kind, "phase": phase });
        if let Some(r) = record {
            n["outputs"] = json!({ "parameters": [{ "name": "output", "value": r.to_string() }] });
        }
        if let Some(m) = message {
            n["message"] = json!(m);
        }
        self.nodes.insert(id, n);
        self
    }

    fn build(self) -> Value {
        json!({
            "kind": "Workflow",
            "metadata": {
                "name": self.name,
                "labels": self.labels,
                "annotations": self.annotations,
                "creationTimestamp": self.started,
            },
            "spec": { "templates": [{ "name": "run", "dag": { "tasks": self.tasks } }] },
            "status": {
                "phase": self.phase,
                "startedAt": self.started,
                "nodes": self.nodes,
            },
        })
    }
}

fn admission(admitted: bool) -> Value {
    json!({
        "format_version": 1,
        "admitted": admitted,
        "because": if admitted { "1 clock due, 0 steps stale" } else { "nothing is owed" },
        "owed": if admitted { json!(["catalog"]) } else { json!([]) },
        "stale": [],
        "open_proposals": 1,
        "max_open_proposals": 3,
    })
}

fn step_record(step: &str, outcome: &str) -> Value {
    let ran = outcome == "ran";
    json!({
        "format_version": 1,
        "step": step,
        "outcome": outcome,
        "sha": ran.then_some(SHA),
        "class": "operational",
        "verb": "compute",
        "receipt": format!(".yidam/runs/{step}.yml"),
        "input": PIN,
        "bundle": ran.then_some("sha256:aa"),
    })
}

fn landed(step: &str, target: Option<&str>) -> Value {
    json!({
        "format_version": 1,
        "step": step,
        "landed": target.map(|_| SHA),
        "target": target,
        "class": "operational",
        "reparented": false,
        "attempts": 1,
        "next": { "format_version": 1, "branch": "main", "sha": SHA, "bundle": "sha256:bb" },
    })
}

fn list(items: Vec<Value>) -> Value {
    json!({ "kind": "List", "items": items })
}

fn status(items: Vec<Value>) -> StatusReport {
    status_of(&list(items), "streamflow", 5, KEEP, None)
}

/// A step whose pod and lander both succeeded with these records.
fn stepped(w: Wf, step: &str, outcome: &str, target: Option<&str>) -> Wf {
    w.step_task(step)
        .node(
            &format!("step-{step}"),
            "Retry",
            "Succeeded",
            Some(step_record(step, outcome)),
            None,
        )
        .node(
            &format!("land-{step}"),
            "Pod",
            "Succeeded",
            Some(landed(step, target)),
            None,
        )
}

#[test]
fn an_operational_commit_is_landed_and_an_epistemic_one_proposed() {
    let w = Wf::new("yidam-streamflow-a", "2026-09-30T06:00:00Z").admitted(admission(true));
    let w = stepped(w, "travel-tier", "ran", Some("main"));
    let w = stepped(w, "survey", "ran", Some("propose/2222222"));
    let r = status(vec![w.build()]);

    let run = &r.runs[0];
    assert_eq!(run.admission.state, AdmissionState::Admitted);
    assert_eq!(run.admission.owed, ["catalog"]);
    assert_eq!(run.admission.open_proposals, Some(1));
    assert_eq!(run.admission.max_open_proposals, Some(3));
    let [tier, survey] = &run.steps[..] else {
        panic!("{:?}", run.steps)
    };
    assert_eq!(
        (tier.step.as_str(), tier.outcome),
        ("travel-tier", StepState::Landed)
    );
    assert_eq!(tier.target.as_deref(), Some("main"));
    assert_eq!(tier.commit.as_deref(), Some(SHA));
    assert_eq!(survey.outcome, StepState::Proposed);
    assert_eq!(survey.target.as_deref(), Some("propose/2222222"));
    assert_eq!(
        survey.still_open, None,
        "the refs were not read, so it is not said"
    );
}

#[test]
fn a_proposal_is_open_while_its_branch_is_on_the_remote() {
    let w = Wf::new("yidam-streamflow-a", "2026-09-30T06:00:00Z").admitted(admission(true));
    let w = stepped(w, "a", "ran", Some("propose/2222222"));
    let w = stepped(w, "b", "ran", Some("propose/3333333"));
    let refs = Refs {
        remote: "git@host:corpus.git".into(),
        branch: "main".into(),
        tip: Some(SHA.into()),
        proposals: vec!["propose/2222222".into()],
        unread: None,
    };
    let r = status_of(&list(vec![w.build()]), "streamflow", 5, KEEP, Some(refs));
    let open: Vec<_> = r.runs[0].steps.iter().map(|s| s.still_open).collect();
    assert_eq!(open, [Some(true), Some(false)]);

    let unread = Refs {
        unread: Some("could not read".into()),
        proposals: vec![],
        ..r.refs.unwrap()
    };
    let w = stepped(
        Wf::new("yidam-streamflow-a", "2026-09-30T06:00:00Z").admitted(admission(true)),
        "a",
        "ran",
        Some("propose/2222222"),
    );
    let r = status_of(&list(vec![w.build()]), "streamflow", 5, KEEP, Some(unread));
    assert_eq!(
        r.runs[0].steps[0].still_open, None,
        "refs that could not be read say nothing about a proposal"
    );
}

#[test]
fn a_refused_landing_reports_the_landers_reason() {
    let w = Wf::new("yidam-streamflow-a", "2026-09-30T06:00:00Z")
        .admitted(admission(true))
        .step_task("disclosure-envelope")
        .node(
            "step-disclosure-envelope",
            "Retry",
            "Succeeded",
            Some(step_record("disclosure-envelope", "ran")),
            None,
        )
        .node(
            "land-disclosure-envelope",
            "Pod",
            "Failed",
            // The lander's own type, so this fixture cannot drift from what it writes.
            Some(
                serde_json::to_value(super::super::land::Refused {
                    format_version: 1,
                    step: "disclosure-envelope".into(),
                    input: PIN.into(),
                    refused: "main moved and touched what it reads: \
                              .yidam/computed/travel-tier.yml"
                        .into(),
                })
                .unwrap(),
            ),
            Some("Error (exit code 1)"),
        )
        .step_task("travel-tier-typed")
        .node(
            "step-travel-tier-typed",
            "Skipped",
            "Omitted",
            None,
            Some("omitted: depends condition not met"),
        );
    let mut w = w;
    w.phase = "Failed";
    let r = status(vec![w.build()]);
    let steps = &r.runs[0].steps;
    assert_eq!(steps[0].outcome, StepState::Refused);
    assert!(
        steps[0]
            .reason
            .as_deref()
            .unwrap()
            .contains("touched what it reads"),
        "{:?}",
        steps[0]
    );
    assert_eq!(steps[1].outcome, StepState::NotReached);
    assert_eq!(steps[1].reason, None);
}

#[test]
fn a_proposal_landed_again_is_still_proposed_with_no_new_commit() {
    let w = Wf::new("yidam-streamflow-a", "2026-09-30T06:00:00Z").admitted(admission(true));
    let mut again = landed("a", Some("propose/2222222"));
    again["landed"] = Value::Null;
    let w = w
        .step_task("a")
        .node(
            "step-a",
            "Retry",
            "Succeeded",
            Some(step_record("a", "ran")),
            None,
        )
        .node("land-a", "Pod", "Succeeded", Some(again), None);
    let r = status(vec![w.build()]);
    let s = &r.runs[0].steps[0];
    assert_eq!(s.outcome, StepState::Proposed);
    assert_eq!(s.commit, None);
    assert!(render(&r).contains("proposed on propose/2222222, which already held it"));
}

#[test]
fn a_step_that_landed_nothing_says_whether_it_was_fresh_or_unchanged() {
    let w = Wf::new("yidam-streamflow-a", "2026-09-30T06:00:00Z").admitted(admission(true));
    let w = stepped(w, "a", "fresh", None);
    let w = stepped(w, "b", "unchanged", None);
    let outcomes: Vec<_> = status(vec![w.build()]).runs[0]
        .steps
        .iter()
        .map(|s| s.outcome)
        .collect();
    assert_eq!(outcomes, [StepState::Fresh, StepState::Unchanged]);
}

#[test]
fn a_run_not_admitted_names_why_and_reaches_no_step() {
    let w = Wf::new("yidam-streamflow-a", "2026-09-30T06:00:00Z")
        .admitted(admission(false))
        .step_task("a");
    let w = Wf {
        tasks: {
            let mut t = w.tasks.clone();
            t.insert(1, json!({ "name": "pin", "template": "pin" }));
            t
        },
        ..w
    }
    .node(
        "pin",
        "Skipped",
        "Skipped",
        None,
        Some("when 'false' evaluated false"),
    );
    let run = &status(vec![w.build()]).runs[0];
    assert_eq!(run.admission.state, AdmissionState::NotAdmitted);
    assert_eq!(run.admission.because, "nothing is owed");
    assert_eq!(run.steps[0].outcome, StepState::NotReached);
}

#[test]
fn a_one_shot_run_was_not_asked_for_admission() {
    let w = stepped(
        Wf::new("yidam-streamflow-a", "2026-09-30T06:00:00Z"),
        "a",
        "ran",
        Some("main"),
    );
    let run = &status(vec![w.build()]).runs[0];
    assert_eq!(run.admission.state, AdmissionState::NotAsked);
    assert_eq!(run.steps[0].outcome, StepState::Landed);
}

#[test]
fn a_pod_that_failed_without_a_record_is_failed_with_argos_message_and_no_more() {
    let w = Wf::new("yidam-streamflow-a", "2026-09-30T06:00:00Z")
        .admitted(admission(true))
        .step_task("a")
        .node(
            "step-a",
            "Retry",
            "Failed",
            None,
            Some("Error (exit code 1)"),
        )
        .step_task("b")
        .node(
            "step-b",
            "Retry",
            "Succeeded",
            Some(step_record("b", "ran")),
            None,
        )
        // A lander from an image before the refusal record: it failed and wrote nothing.
        .node("land-b", "Pod", "Failed", None, Some("Error (exit code 1)"));
    let steps = &status(vec![w.build()]).runs[0].steps;
    assert_eq!(steps[0].outcome, StepState::Failed);
    assert_eq!(
        steps[0].reason.as_deref(),
        Some("the step failed and left no record (Error (exit code 1))")
    );
    assert_eq!(steps[1].outcome, StepState::Failed);
    assert!(steps[1]
        .reason
        .as_deref()
        .unwrap()
        .starts_with("the lander failed"));
}

#[test]
fn a_succeeded_node_without_its_record_is_unknown_not_guessed() {
    let w = Wf::new("yidam-streamflow-a", "2026-09-30T06:00:00Z")
        .admitted(admission(true))
        .step_task("a")
        .node(
            "step-a",
            "Retry",
            "Succeeded",
            Some(step_record("a", "ran")),
            None,
        )
        .node("land-a", "Pod", "Succeeded", None, None);
    let s = &status(vec![w.build()]).runs[0].steps[0];
    assert_eq!(s.outcome, StepState::Unknown);
    assert_eq!(
        s.reason.as_deref(),
        Some("the lander ended Succeeded and left no record")
    );
}

#[test]
fn a_running_run_reports_its_steps_as_running_or_not_yet_reached() {
    let mut w = Wf::new("yidam-streamflow-a", "2026-09-30T06:00:00Z")
        .admitted(admission(true))
        .step_task("a")
        .node(
            "step-a",
            "Retry",
            "Succeeded",
            Some(step_record("a", "ran")),
            None,
        )
        .step_task("b");
    w.phase = "Running";
    let steps = &status(vec![w.build()]).runs[0].steps;
    assert_eq!(steps[0].outcome, StepState::Running, "landing next");
    assert_eq!(steps[1].outcome, StepState::NotReached);
}

#[test]
fn a_gather_reports_every_peers_outcome_including_a_refused_one() {
    let survey = json!({
        "format_version": 1,
        "gather": "flows",
        "input": PIN,
        "bundle": "sha256:cc",
        "asking": 2,
        "asks": [],
        "peers": [{
            "package": "gauges",
            "outcome": "missing",
            "reason": "named by the gather, not installed",
            "matched": 0,
            "cited": 0,
            "unquotable": [],
        }],
    });
    let ask = |peer: &str, outcome: &str| {
        json!({
            "format_version": 1,
            "gather": "flows",
            "peer": peer,
            "outcome": outcome,
            "record": "sha256:dd",
        })
    };
    let mut w = Wf::new("yidam-streamflow-a", "2026-09-30T06:00:00Z").admitted(admission(true));
    w.tasks.push(json!({
        "name": "survey-flows",
        "template": "survey",
        "arguments": { "parameters": [{ "name": "gather", "value": "flows" }] },
    }));
    let w = w
        .node("survey-flows", "Retry", "Succeeded", Some(survey), None)
        .node(
            "ask-flows(0:a)",
            "Pod",
            "Succeeded",
            Some(ask("basin", "answered")),
            None,
        )
        .node(
            "ask-flows(1:b)",
            "Pod",
            "Succeeded",
            Some(ask("reach", "refused")),
            None,
        )
        .node("ask-flows(2:c)", "Pod", "Error", None, Some("pod deleted"));
    let mut w = w;
    // The asker that died: Argo kept its input, which names the peer.
    let dead = w
        .nodes
        .values_mut()
        .find(|n| n["displayName"] == "ask-flows(2:c)")
        .unwrap();
    dead["inputs"] = json!({ "parameters": [{ "name": "ask", "value": r#"{"peer":"weirs"}"# }] });
    let children: Vec<Value> = w
        .nodes
        .values()
        .filter(|n| n["displayName"].as_str().unwrap().starts_with("ask-flows("))
        .map(|n| n["id"].clone())
        .collect();
    let w = w
        .node("ask-flows", "TaskGroup", "Failed", None, None)
        .node(
            "gather-flows",
            "Retry",
            "Succeeded",
            Some(json!({
                "format_version": 1, "step": "gather/flows", "outcome": "ran", "sha": SHA,
                "class": "epistemic", "verb": "open", "receipt": "", "input": PIN,
                "bundle": "sha256:ee",
            })),
            None,
        )
        .node(
            "land-gather-flows",
            "Pod",
            "Succeeded",
            Some(landed("gather/flows", Some("propose/gather/flows/2222222"))),
            None,
        );
    let mut w = w;
    let group = w
        .nodes
        .values_mut()
        .find(|n| n["displayName"] == "ask-flows")
        .unwrap();
    group["children"] = json!(children);

    let run = &status(vec![w.build()]).runs[0];
    let g = &run.gathers[0];
    assert_eq!(g.gather, "flows");
    assert_eq!(g.landing.outcome, StepState::Proposed);
    let peers: Vec<(&str, &str)> = g
        .peers
        .iter()
        .map(|p| (p.peer.as_str(), p.outcome.as_str()))
        .collect();
    assert_eq!(
        peers,
        [
            ("basin", "answered"),
            ("gauges", "missing"),
            ("reach", "refused"),
            ("weirs", "unknown"),
        ]
    );
    let weirs = &g.peers[3];
    assert!(
        weirs
            .reason
            .as_deref()
            .unwrap()
            .contains("left no record (pod deleted)"),
        "{weirs:?}"
    );
}

#[test]
fn a_run_argo_deleted_is_unknown_and_says_how_long_runs_are_kept() {
    let cron = json!({
        "kind": "CronWorkflow",
        "metadata": { "name": "yidam-streamflow" },
        "status": { "lastScheduledTime": "2026-09-30T06:00:00Z" },
    });
    let mut kept =
        Wf::new("yidam-streamflow-1727589600", "2026-09-29T06:00:02Z").admitted(admission(false));
    kept.labels = json!({ CRON_LABEL: "yidam-streamflow" });
    kept.annotations = json!({ SCHEDULED: "2026-09-29T06:00:00Z" });

    let r = status(vec![cron.clone(), kept.build()]);
    assert_eq!(r.runs.len(), 2);
    let gone = &r.runs[0];
    assert_eq!(gone.workflow, None);
    assert_eq!(gone.phase, "unknown");
    assert_eq!(gone.scheduled.as_deref(), Some("2026-09-30T06:00:00Z"));
    let why = gone.unknown.as_deref().unwrap();
    assert!(
        why.contains("86400s after it succeeds and 604800s after it fails"),
        "{why}"
    );
    assert!(gone.steps.is_empty(), "no step outcome is invented");
    assert_eq!(gone.admission.state, AdmissionState::Unknown);

    // The tick's own run is still there: nothing is reported missing.
    let mut today =
        Wf::new("yidam-streamflow-1727676000", "2026-09-30T06:00:01Z").admitted(admission(false));
    today.labels = json!({ CRON_LABEL: "yidam-streamflow" });
    today.annotations = json!({ SCHEDULED: "2026-09-30T06:00:00Z" });
    let r = status(vec![cron, today.build()]);
    assert_eq!(r.runs.len(), 1);
    assert!(r.runs[0].unknown.is_none());
}

#[test]
fn a_run_whose_node_status_was_offloaded_is_unknown() {
    let mut w = Wf::new("yidam-streamflow-a", "2026-09-30T06:00:00Z")
        .admitted(admission(true))
        .build();
    w["status"].as_object_mut().unwrap().remove("nodes");
    w["status"]["offloadNodeStatusVersion"] = json!("fnv:123");
    let run = &status(vec![w]).runs[0];
    assert!(run.unknown.as_deref().unwrap().contains("offloaded"));
    assert!(run.steps.is_empty());
}

#[test]
fn only_this_corpus_runs_are_read_newest_first_and_limited() {
    let ours = |name: &str, at: &'static str| Wf::new(name, at).admitted(admission(false)).build();
    let mut other = Wf::new("yidam-other-a", "2026-09-30T09:00:00Z");
    other.labels = json!({ "yidam.dev/corpus": "other" });
    let mut by_cron = Wf::new("yidam-streamflow-c", "2026-09-30T08:00:00Z");
    by_cron.labels = json!({ CRON_LABEL: "yidam-streamflow" });
    let items = vec![
        ours("yidam-streamflow-a", "2026-09-28T06:00:00Z"),
        other.build(),
        by_cron.build(),
        ours("yidam-streamflow-b", "2026-09-29T06:00:00Z"),
    ];
    let names = |r: StatusReport| -> Vec<String> {
        r.runs.into_iter().filter_map(|r| r.workflow).collect()
    };
    assert_eq!(
        names(status(items.clone())),
        [
            "yidam-streamflow-c",
            "yidam-streamflow-b",
            "yidam-streamflow-a"
        ]
    );
    assert_eq!(
        names(status_of(&list(items), "streamflow", 2, KEEP, None)),
        ["yidam-streamflow-c", "yidam-streamflow-b"]
    );
}

#[test]
fn the_text_names_each_outcome_and_the_refusal() {
    let w = Wf::new("yidam-streamflow-a", "2026-09-30T06:00:00Z").admitted(admission(true));
    let w = stepped(w, "travel-tier", "ran", Some("main"))
        .step_task("b")
        .node(
            "step-b",
            "Retry",
            "Succeeded",
            Some(step_record("b", "ran")),
            None,
        )
        .node(
            "land-b",
            "Pod",
            "Failed",
            Some(json!({ "format_version": 1, "step": "b", "input": PIN, "refused": "no" })),
            None,
        );
    let text = render(&status(vec![w.build()]));
    assert!(
        text.contains("admitted: 1 clock due, 0 steps stale"),
        "{text}"
    );
    assert!(
        text.contains("travel-tier  landed on main at 1111111111"),
        "{text}"
    );
    assert!(text.contains("b            refused: no"), "{text}");
    assert!(text.contains("refs: not read"), "{text}");
}
