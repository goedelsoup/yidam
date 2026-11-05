//! `yidam cycle` — where this repository is in its loop, and what the next act is.
//!
//! Four surfaces already answer a quarter of this question each. `due` says what is owed,
//! `phases` says what is in flight, `lint` and `graph-check` say what is blocked. A person
//! opening a session runs none of them, or runs one, and the loop has no object: nothing in
//! this repository ever said *here is where you are, and here is what to do next*.
//!
//! So this composes rather than computes. Every number below is read from the command that
//! already owns it — [`crate::cmd::due::read_clocks`], [`crate::cmd::phases::collect_phases`],
//! [`crate::cmd::lint::run_checks`] against the baseline ratchet,
//! [`crate::cmd::corpus::graph_check_data`], and — for the manifest's steps —
//! [`crate::cmd::run::plan_and_write`] as a dry run, which is `yidam run --dry-run`'s reader. **Nothing here re-derives a fact a sibling
//! surface publishes**, which is `due`'s own rule about `phases` (`status` reported 26 active
//! phases against a true count of 1 for exactly as long as the classification lived in two
//! places), applied one level out over four surfaces instead of one.
//!
//! # It exits zero however much is owed
//!
//! `due`'s precedent, and its reasoning: a corpus that is owed is not a corpus that is
//! broken. `--strict` exits nonzero, for a scheduled job that wants a signal — and it signals
//! on the union of what this report composes, so it covers being *blocked* as well as being
//! owed. Those are different facts and the summary says which is which; a scheduled job
//! wanting one signal off this report should get one that does not go quiet because the half
//! that fired was the other one.
//!
//! # There is no declared phase order, and the "next" half says so rather than inventing one
//!
//! #576 specifies the fourth half as *"the kuten's declared phase order (what is next)"*.
//! **No kuten declares an order.** `kuten.yml`'s `phases` slot is
//! `{types: [...], commit_share: {low, high}}` — [`crate::kuten::Phases`] — and `types` is a
//! set of four names with a band beside it; nothing in the profile, the model, or
//! `PHASES.md`'s prose reads them as a sequence, and no consumer orders them. The shipped
//! `inquiry` profile is the only profile there is.
//!
//! Two things follow, and both are stated in the report rather than only here:
//!
//! - **The ranking below is this command's, not the kuten's.** Blocked before owed before in
//!   flight, because a blocked corpus cannot land what discharging a clock would produce, and
//!   a clock that is owed is a thing to do where a phase in flight is a thing already being
//!   done. That is an argument, and it is reversible; what it is not is a declaration read
//!   off a profile, and a report implying otherwise would be citing a field that does not
//!   exist.
//! - **What the kuten does declare is named, and named as what it is** — the phase *types* a
//!   phase opened now would be named from, and the kind of question this practice presses
//!   toward ([RFC-0028 §5](../../../../docs/rfcs/0028-kuten-layer.md): *"`cycle` (A4) may name
//!   it as a next act, and nothing writes"*). A repository holding no kuten is told that half
//!   needs a declaration, and still gets the other three.
//!
//! # It is not an MCP tool, and that is #576's whole argument
//!
//! RFC-0005 froze thirteen tool names and every one reads. A cycle report shipped as a
//! fourteenth would be *"a fourteenth read-only tool telling an agent what to do next while
//! it still cannot act"*. RFC-0029 §2.3 decides that `cycle` joins the `act` tier, coined with
//! `propose` under #474 — one frozen-contract event rather than two, and `cycle` never exists
//! as a read-only tool at any point. Nothing in this module reaches `cmd/serve`.
//!
//! # It authors nothing
//!
//! RFC-0026's invariant does not have to be argued for here: this command drafts no commit and
//! touches no file a commit could carry. It reports. The one file it can write is `--record`'s
//! round record under `.yidam/loop/`, which ignores itself and is local loop state rather than
//! provenance — see [`round`]. Without `--record` it opens no file for writing at all.

use anyhow::Result;
use std::fmt::Write as _;
use std::path::Path;

use crate::cmd::due::{Actor, Clock, State};
use crate::cmd::run::manifest::MANIFEST;
use crate::cmd::run::{plan_and_write, Freshness};
use crate::paths::require_yidam_repo;

pub mod round;

use round::{Finding, Round, SinceLast};

/// One inquiry ref in flight.
///
/// A projection of [`crate::cmd::phases::PhaseRow`] rather than the row itself: `phases`
/// answers *what refs exist and what state is each in*, and this half of the report is about
/// the ones that are unsettled. Carrying the whole row would put `settled` and `position`
/// rows into a list headed "in flight", which is the conflation `state` became a column to
/// undo.
#[derive(Debug, Clone, serde::Serialize)]
pub struct InFlight {
    pub name: String,
    pub ref_name: String,
    pub owner: String,
    pub started: String,
    pub commits: usize,
}

/// One thing that would fail a gate today.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Blocked {
    /// Which gate says so — `lint` or `graph-check`. A consumer keys on this.
    pub gate: &'static str,
    /// The check or condition, as that gate names it.
    pub check: String,
    /// The node or file, repo-relative, where there is one.
    pub node: Option<String>,
    pub detail: String,
    /// What discharges it.
    pub remedy: String,
}

/// Where a next act came from.
///
/// The value is the *half of this report* that produced the act, which
/// is what a reader needs to know to argue with the ranking — not a severity, and not a
/// priority number, because this command's ordering is an argument rather than a measurement
/// and a number would present it as the second thing.
///
/// **Spelled by [`Source::name`], not by a serde rename rule.** This enum used
/// `rename_all = "lowercase"`, which turns `InFlight` into `"inflight"`, while
/// `report.schema.json` declares `"in_flight"` (the same spelling as the report's own `in_flight`
/// half). So a validating consumer rejected every `cycle` report that had a phase in flight. No
/// golden could see it, because the fixture has nothing in flight (see `UNREACHED` in
/// `report_goldens.rs`). The tests here compared Rust variants, so they could not see it either.
/// [`CYCLE_SOURCES`] is now the roster the schema is held to, in both directions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    Blocked,
    Owed,
    InFlight,
    Practice,
}

impl serde::Serialize for Source {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.name())
    }
}

/// Every `next[].source` a report can carry: the roster that
/// `properties.next.items.properties.source.enum` is compared against.
///
/// Built by walking a successor `match` with no wildcard arm, so a fifth variant is a compile
/// error here and the fixed `[_; 4]` width fails to evaluate. This is the same construction as
/// `COMMIT_KINDS`. An array literal would have been a second copy of the vocabulary.
pub const CYCLE_SOURCES: [&str; 4] = {
    let mut out = [""; 4];
    let mut cur = Some(Source::Blocked);
    let mut i = 0;
    while let Some(k) = cur {
        out[i] = k.name();
        cur = match k {
            Source::Blocked => Some(Source::Owed),
            Source::Owed => Some(Source::InFlight),
            Source::InFlight => Some(Source::Practice),
            Source::Practice => None,
        };
        i += 1;
    }
    out
};

impl Source {
    /// The value a report carries. Snake case, because the schema and the `in_flight` half both
    /// spell it that way.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Blocked => "blocked",
            Self::Owed => "owed",
            Self::InFlight => "in_flight",
            Self::Practice => "practice",
        }
    }

    fn heading(self) -> &'static str {
        match self {
            Self::Blocked => "blocked",
            Self::Owed => "owed",
            Self::InFlight => "in flight",
            Self::Practice => "practice",
        }
    }
}

/// One act the report names, with where it came from and why.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Act {
    pub source: Source,
    /// The command or act, as a reader would run or perform it.
    pub act: String,
    /// What makes it the thing to do — the finding, clock or declaration behind it.
    pub why: String,
    /// `kind`, and `argv` on a tool act: what a loop switches on, where [`Self::act`] is what a
    /// reader is told (#1442). An owed clock's is `due`'s, copied — not re-derived here.
    #[serde(flatten)]
    pub actor: Actor,
    /// What the act is about: the nodes a finding names, the ref in flight, the steps a run
    /// would invoke, the nodes a clock lists. Empty when the act is about the corpus as a whole.
    pub targets: Vec<String>,
}

/// What this corpus declared its work is aimed at.
///
/// Read from the kuten, and it carries no order because no profile declares one — see this
/// module's header. [`Self::undeclared`] is the sentence #576's definition of done asks for:
/// a repository holding no kuten gets the other three halves and is told this one needs a
/// declaration.
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct Practice {
    pub kuten: Option<String>,
    pub revision: Option<u32>,
    /// The phase types the profile declares. **A set, not a sequence.**
    pub phase_types: Vec<String>,
    /// The kind of question this practice presses toward — `epistemic`, or `coverage`, which
    /// is reserved and unimplemented.
    pub question_pressure: Option<String>,
    /// Why this half is short, when it is. `None` when a kuten is held and read.
    pub undeclared: Option<String>,
}

impl Practice {
    /// Read the declaration and the profile it names.
    ///
    /// Three ways this half can be short, and they are different facts rather than one
    /// absence: no decision record at all (the supported no-kuten state, which every
    /// repository is in today), a record naming a profile this repository does not hold (a
    /// re-vendor that did not happen), and a record or profile that did not parse. The last
    /// is an error a person should see and is reported as its own sentence rather than
    /// silently becoming the first.
    fn read(root: &Path) -> Self {
        let declaration = match crate::kuten::read_declaration(root) {
            Ok(d) => d,
            Err(e) => {
                return Self {
                    undeclared: Some(format!("this repository's kuten record is unreadable: {e}")),
                    ..Default::default()
                }
            }
        };
        let Some(declaration) = declaration else {
            return Self {
                undeclared: Some(
                    "this repository holds no kuten, so nothing here declares what its work is \
                     aimed at — `yidam kuten adopt <name>` declares one, and holding none is a \
                     supported state"
                        .to_string(),
                ),
                ..Default::default()
            };
        };
        let profile = match crate::kuten::read_profile(root, &declaration.name) {
            Ok(p) => p,
            Err(e) => {
                return Self {
                    kuten: Some(declaration.name.clone()),
                    revision: Some(declaration.revision),
                    undeclared: Some(format!(
                        "the vendored `{}` profile is unreadable: {e}",
                        declaration.name
                    )),
                    ..Default::default()
                }
            }
        };
        let Some(profile) = profile else {
            return Self {
                kuten: Some(declaration.name.clone()),
                revision: Some(declaration.revision),
                undeclared: Some(format!(
                    "`{}` is declared and not vendored here — re-vendor the prelude, or record a \
                     superseding decision",
                    declaration.name
                )),
                ..Default::default()
            };
        };
        Self {
            kuten: Some(declaration.name),
            revision: Some(declaration.revision),
            phase_types: profile
                .phases
                .as_ref()
                .map(|p| p.types.clone())
                .unwrap_or_default(),
            question_pressure: profile
                .question_pressure
                .as_ref()
                .map(|q| q.kind.name().to_string()),
            undeclared: None,
        }
    }
}

/// Where this repository is in its loop.
#[derive(Debug, serde::Serialize)]
pub struct CycleReport {
    /// What is owed — `due`'s five clocks, read through `due`'s own reader and unchanged.
    pub owed: Vec<Clock>,
    /// The manifest's steps whose committed receipt does not stand at the tip — owed as surely
    /// as a due clock, and the half `due` cannot report (#1440).
    ///
    /// **`stale_steps` and not `stale`**, for `blocking`'s reason below: the schema's flat
    /// namespace already gives `stale` to `regen --check`, as an array of REGEN blocks.
    pub stale_steps: Vec<StaleStep>,
    /// Why the step half could not be read, when it could not: the planner's own refusal,
    /// verbatim. `None` both when it was read and when there is no manifest to read — a corpus
    /// declaring no capabilities has asked for nothing, which is not a gap in this report.
    pub stale_steps_unread: Option<String>,
    /// What is in flight — the unsettled inquiry refs `phases` lists.
    pub in_flight: Vec<InFlight>,
    /// What is blocked — what `lint`'s ratchet and `graph-check` would fail on today.
    ///
    /// **`blocking` on the wire and not `blocked`**, and the rename is not cosmetic.
    /// `report.schema.json`'s property namespace is flat across every report, and `blocked`
    /// is already `rename`'s — an array of *strings* saying why a rename cannot proceed. One
    /// name cannot be an array of strings in one report and an array of findings in another;
    /// that is the defect the contract's own `rejected.code` set was repaired for, where
    /// three lint check ids had been borrowed into a vocabulary about rejected steps.
    /// Widening the schema would have kept the name and cost `rename` its type.
    ///
    /// The word survives where it belongs: the text report still heads this half *blocked*,
    /// because that is the question a reader asks. `blocking` is what the list holds — the
    /// findings doing it.
    #[serde(rename = "blocking")]
    pub blocked: Vec<Blocked>,
    /// What is next, in this command's ranking. See the module header: the ranking is an
    /// argument made here, not an order read off a kuten.
    pub next: Vec<Act>,
    pub practice: Practice,
    /// Clocks past their interval. `due`'s count, carried so a consumer need not re-derive it.
    pub due: usize,
    pub strict: bool,
    /// What moved since the last round recorded on this branch (#1443). **Absent unless asked
    /// for** with `--since-last`, and absent on the MCP tool, which takes no arguments: a field
    /// that is absent was not asked, where a null would have read as asked and empty.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub since_last: Option<SinceLast>,
    /// Why there is no delta when one was asked for — no round recorded on this branch, or a
    /// record this binary cannot read. Exactly one of this and [`Self::since_last`] is present
    /// on a `--since-last` run, so a missing record is never reported as nothing having moved.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub since_last_unread: Option<String>,
    /// Every gating finding behind [`Self::blocked`], accepted ones included: what a round
    /// records, and not on the wire — the blocking half is what the report says about them.
    #[serde(skip)]
    pub findings: Vec<Finding>,
    /// Whether the run exits zero. True unless `--strict` and something is owed — a due clock or
    /// a stale step — or blocked.
    /// Being owed is not a defect, and this field exists so a consumer does not infer one.
    pub passed: bool,
}

// ── the four halves ───────────────────────────────────────────────────────────

/// A manifest step that `yidam run` would invoke.
#[derive(Debug, serde::Serialize)]
pub struct StaleStep {
    pub step: String,
    /// `stale`, or `unknown` when a step earlier in the plan would write what this one reads.
    /// Both are owed: a run invokes the first and then decides the second.
    pub freshness: Freshness,
    /// The planner's reason, verbatim.
    pub because: String,
}

/// The manifest's half of what is owed, read the way `cluster admit` reads it.
///
/// `due` has no clock for this, because a stale step is stale rather than stale after N days,
/// and `due`'s table is a table of declared intervals — `admit`'s argument for asking it
/// beside `due` rather than inside it, and this command's for the same. Before #1440 `cycle`
/// did not ask it at all, so a corpus whose manifest was a commit behind reported nothing owed
/// and `next` said the loop was quiet while `cluster admit` admitted a run.
///
/// **Local refs only.** `admit` fetches `propose/*` first because it reads a single-branch
/// clone; this reads the checkout it stands in, so a proposed result is fresh here exactly
/// when `yidam run --dry-run` would say it is, and `cycle` stays offline.
///
/// **A refusal is reported, not raised.** The planner refuses a detached HEAD and a plan this
/// binary cannot invoke; either would otherwise turn a report about four halves into an
/// error about one, and the other three are still true.
fn stale_steps(root: &Path) -> (Vec<StaleStep>, Option<String>) {
    if !root.join(MANIFEST).exists() {
        return (Vec::new(), None);
    }
    match plan_and_write(root, None, true) {
        Ok(plan) => (
            plan.steps
                .into_iter()
                .filter(|s| s.freshness != Freshness::Fresh)
                .map(|s| StaleStep {
                    step: s.step,
                    freshness: s.freshness,
                    because: s.because,
                })
                .collect(),
            None,
        ),
        Err(e) => (Vec::new(), Some(format!("{e:#}"))),
    }
}

/// The unsettled inquiry refs.
///
/// `phases`' own rows, filtered on the state it publishes. Reading the refs directly would be
/// the third implementation of one predicate this repository has had to consolidate.
fn in_flight(root: &Path) -> Vec<InFlight> {
    crate::cmd::phases::collect_phases(root)
        .unwrap_or_default()
        .into_iter()
        .filter(|r| r.is_in_flight())
        .map(|r| InFlight {
            name: r.name,
            ref_name: r.ref_name,
            owner: r.owner,
            started: r.started,
            commits: r.commits,
        })
        .collect()
}

/// What would fail a gate today.
///
/// **`lint`'s findings are read through the baseline ratchet, not off the raw run**, and the
/// difference is the whole meaning of this half. A corpus carrying two hundred blessed
/// error-severity findings is not blocked by them: it agreed to them, `yidam lint` is green,
/// and listing them here would tell a reader that a repository doing exactly what it agreed
/// to has two hundred things to fix. What blocks is what `lint` itself gates on — a finding
/// introduced against the baseline, a baseline entry that has outlived its expiry, and a
/// stale entry the run no longer reproduces.
///
/// `graph-check` has no ratchet and needs none: it gates on `passed`, and every issue it
/// lists is one.
fn blocked(root: &Path) -> (Vec<Blocked>, Vec<Finding>) {
    let mut out = Vec::new();
    let mut findings = Vec::new();

    let opts = crate::cmd::lint::Options::default();
    let checks = crate::cmd::lint::run_checks(root, &opts);
    let baseline = crate::cmd::lint::baseline::Baseline::load(root).unwrap_or_default();
    let corpus_commits = crate::cmd::lint::history::corpus_commits(root);
    let unasked = crate::cmd::lint::unasked(&checks);
    let diff = crate::cmd::lint::baseline::diff(&checks, &baseline, &corpus_commits, &unasked);

    // What a round records: every violation the gate would count, accepted or not, by the
    // baseline's own identity. Not only the blocking ones, because a round that fixes a finding
    // the baseline forgave has still fixed it — and the blocking half would report that fix as
    // a *new* blocker, "the baseline is stale".
    for check in &checks {
        for v in check.violations.iter().filter(|v| check.gates(v)) {
            findings.push(Finding {
                gate: "lint".to_string(),
                check: check.id.to_string(),
                node: v.node.clone(),
            });
        }
    }

    for (check, node) in &diff.introduced {
        out.push(Blocked {
            gate: "lint",
            check: check.clone(),
            node: Some(node.clone()),
            detail: "introduced against the baseline".to_string(),
            remedy: format!("yidam lint --explain  (fix it, or `--bless` to accept it: {check})"),
        });
    }
    for e in &diff.expired {
        out.push(Blocked {
            gate: "lint",
            check: e.check.clone(),
            node: Some(e.node.clone()),
            detail: format!(
                "baseline entry past its expiry, {} commit(s) old",
                e.commits
            ),
            remedy: "yidam lint  (the repository agreed to deal with this and has not)".to_string(),
        });
    }
    for (check, node) in &diff.resolved {
        out.push(Blocked {
            gate: "lint",
            check: check.clone(),
            node: Some(node.clone()),
            detail: "in the baseline and no longer found — the baseline is stale".to_string(),
            remedy: "yidam lint --bless  (record that it is fixed)".to_string(),
        });
    }

    let gc = crate::cmd::corpus::graph_check_data(&crate::corpus::Corpus::open(root));
    if !gc.passed {
        for n in gc.classes_with_issues.iter().chain(&gc.nodes_with_issues) {
            out.push(Blocked {
                gate: "graph-check",
                check: "graph".to_string(),
                node: Some(n.node.clone()),
                detail: n.issues.join("; "),
                remedy: "yidam graph-check".to_string(),
            });
            findings.push(Finding {
                gate: "graph-check".to_string(),
                check: "graph".to_string(),
                node: n.node.clone(),
            });
        }
    }

    (out, findings)
}

/// What to do next, in this command's ranking.
///
/// **Blocked, then owed, then in flight, then practice.** The argument is in the module
/// header, and the ranking is this command's rather than a kuten's, because no kuten declares
/// one. Each half contributes its own acts; a half that found nothing contributes none, and a
/// report that names no act at all says so rather than printing an empty heading.
///
/// An owed clock contributes the remedy `due` already computed for it — the five remedies
/// that are not this command's to invent, and four of the five are not commands at all. A
/// clock in any other state contributes nothing here: an unset clock's remedy is to set it,
/// which is a thing to do about the configuration rather than about the corpus, and `due` is
/// where that is said.
fn next_acts(
    blocked: &[Blocked],
    owed: &[Clock],
    stale_steps: &[StaleStep],
    in_flight: &[InFlight],
    practice: &Practice,
) -> Vec<Act> {
    let mut out = Vec::new();

    // One act per gate, not one per finding. A reader with forty introduced findings has one
    // thing to do, and forty rows under "what is next" is the shape of list people skim.
    for gate in ["lint", "graph-check"] {
        let n = blocked.iter().filter(|b| b.gate == gate).count();
        if n == 0 {
            continue;
        }
        // Each node once, in the order the gate reported it.
        let mut targets: Vec<String> = Vec::new();
        for node in blocked
            .iter()
            .filter(|b| b.gate == gate)
            .filter_map(|b| b.node.clone())
        {
            if !targets.contains(&node) {
                targets.push(node);
            }
        }
        out.push(Act {
            source: Source::Blocked,
            act: format!("yidam {gate}"),
            why: format!("{n} finding(s) would fail this gate today"),
            // A person act, though the prose names a command: the command lists the findings
            // and fixing them is an edit. See [`Actor`].
            actor: Actor::Person,
            targets,
        });
    }

    for c in owed.iter().filter(|c| c.state == State::Due) {
        let Some(remedy) = &c.remedy else { continue };
        out.push(Act {
            source: Source::Owed,
            act: remedy.clone(),
            why: format!("the {} clock is due — {}", c.id, c.detail),
            // `due` sets the two together; a remedy without one is a builder that forgot, and
            // the safe reading of a missing kind is the one a consumer is told to use.
            actor: c.actor.clone().unwrap_or(Actor::Person),
            targets: c.subjects.clone(),
        });
    }

    // One act for the whole manifest, for the gates' reason above: `yidam run` invokes every
    // stale step in plan order, so a reader with three stale steps has one thing to do.
    if !stale_steps.is_empty() {
        let names: Vec<&str> = stale_steps.iter().map(|s| s.step.as_str()).collect();
        out.push(Act {
            source: Source::Owed,
            act: "yidam run".to_string(),
            why: format!(
                "{} step(s) have no receipt matching the tip — {}",
                names.len(),
                names.join(", ")
            ),
            actor: Actor::tool(&["yidam", "run"]),
            targets: names.iter().map(|n| n.to_string()).collect(),
        });
    }

    // Not a remedy, because a phase in flight is not a thing owed: it is work already being
    // done, and the act is to finish it. `due`'s phases clock says when one has been in
    // flight too long, and that arrives through the owed half above rather than twice.
    for f in in_flight {
        out.push(Act {
            source: Source::InFlight,
            act: format!("settle or continue {}", f.ref_name),
            why: format!(
                "in flight since {}, {} commit(s) ahead of the baseline",
                f.started, f.commits
            ),
            actor: Actor::Person,
            targets: vec![f.ref_name.clone()],
        });
    }

    // RFC-0028 §5: the pressure slot creates pressure toward a kind of question; `cycle` may
    // name it as a next act, and nothing writes. What is named is the declaration, never a
    // measurement of how far this corpus is from it — that is `yidam kuten check`, and
    // computing it here would be this report's second implementation of a question it does
    // not own.
    if let Some(kind) = &practice.question_pressure {
        out.push(Act {
            source: Source::Practice,
            act: format!("open a {kind} question, or bound the next work into a phase"),
            why: match practice.phase_types.as_slice() {
                [] => format!(
                    "this corpus's practice presses toward {kind} questions — `yidam kuten \
                     check` reports where its history and its declaration disagree"
                ),
                types => format!(
                    "this corpus's practice presses toward {kind} questions, and names its phase \
                     types as {} — a set, in no declared order",
                    types.join(", ")
                ),
            },
            actor: Actor::Person,
            targets: Vec::new(),
        });
    }

    out
}

// ── assembly ──────────────────────────────────────────────────────────────────

/// Read every half against `root`.
///
/// `today` is passed rather than read, for `due`'s reason: three of the five clocks count days,
/// and a report whose own tests depend on the day they run is the failure `lint::today_iso`
/// describes.
pub(crate) fn read_cycle(root: &Path, strict: bool, today: i64) -> Result<CycleReport> {
    let cfg = crate::config::load_yidam_config(root)?;
    let owed = crate::cmd::due::read_clocks(root, &cfg.due, today);
    let due = owed.iter().filter(|c| c.state == State::Due).count();
    let (stale_steps, stale_steps_unread) = stale_steps(root);
    let in_flight = in_flight(root);
    let (blocked, findings) = blocked(root);
    let practice = Practice::read(root);
    let next = next_acts(&blocked, &owed, &stale_steps, &in_flight, &practice);

    Ok(CycleReport {
        passed: !strict || (due == 0 && stale_steps.is_empty() && blocked.is_empty()),
        owed,
        stale_steps,
        stale_steps_unread,
        in_flight,
        blocked,
        next,
        practice,
        due,
        strict,
        since_last: None,
        since_last_unread: None,
        findings,
    })
}

/// What a round asks of `cycle` beyond the report.
#[derive(Debug, Clone, Copy, Default)]
pub struct RoundOpts {
    /// Compare against the last round recorded on this branch.
    pub since_last: bool,
    /// Record this round, after any comparison.
    pub record: bool,
    /// Render the token-budgeted brief instead of the full text report, with this budget.
    pub brief: Option<usize>,
}

/// The default `--budget` for `--brief`, in `pack`'s tokens (one per four bytes).
///
/// Sized to open a session without crowding it: about two thousand bytes, which holds a
/// header, five acts, and a few lines of each half.
pub const BRIEF_BUDGET: usize = 500;

/// Compare and record, in that order, and return the round this run saw.
///
/// **Compare first.** `--since-last --record` is the loop's step — what did the last round
/// leave, then remember this one — and recording first would compare a round with itself.
pub(crate) fn with_round(root: &Path, report: &mut CycleReport, opts: RoundOpts) -> Result<Round> {
    let now = Round::of(root, report);
    if opts.since_last || opts.brief.is_some() {
        match round::load(root, now.branch.as_deref()) {
            Ok(Some(last)) => report.since_last = Some(SinceLast::between(&last, &now)),
            Ok(None) => report.since_last_unread = Some(round::no_record(now.branch.as_deref())),
            Err(why) => report.since_last_unread = Some(why),
        }
    }
    if opts.record {
        round::store(root, &now)?;
    }
    Ok(now)
}

/// `yidam cycle`. Read-only, offline, and exits zero however much is owed unless `--strict`.
pub fn cycle(
    root: Option<&std::path::Path>,
    strict: bool,
    rounds: RoundOpts,
    format: crate::report::Format,
) -> Result<()> {
    let root = crate::paths::resolve_root(root)?;
    require_yidam_repo(&root)?;
    if rounds.brief.is_some() && format.is_json() {
        anyhow::bail!(
            "`--brief` is a text rendering, cut to a budget — `--format json` already carries every \
             field it summarizes, uncut. Pass one or the other."
        );
    }
    let mut report = read_cycle(&root, strict, crate::dates::today_days())?;
    let now = with_round(&root, &mut report, rounds)?;
    let passed = report.passed;

    crate::report::gate(&root, format, report, passed, |r| match rounds.brief {
        Some(budget) => println!("{}", render_brief(r, &now, budget)),
        None => println!("{}", render(r, &root)),
    })
}

/// The text report.
///
/// Four headed halves in the order the question is asked — where am I, and what is next —
/// and a closing sentence carrying `due`'s argument, because this report composes `due` and
/// inherits the reading it has to prevent: a reader shown four lists with counts on them
/// will reach for the reading they already have, that a report with numbers in it is a list
/// of problems.
pub(crate) fn render(report: &CycleReport, root: &Path) -> String {
    let mut out = format!("yidam cycle — {}\n\n", root.display());

    let _ = writeln!(out, "owed");
    if report.owed.iter().all(|c| c.state != State::Due) && report.stale_steps.is_empty() {
        let _ = writeln!(out, "  nothing is due");
    }
    for c in report.owed.iter().filter(|c| c.state == State::Due) {
        let _ = writeln!(out, "  {:<10} {}", c.id, c.detail);
    }
    for s in &report.stale_steps {
        let _ = writeln!(out, "  {:<10} step {} — {}", "run", s.step, s.because);
    }
    if let Some(why) = &report.stale_steps_unread {
        let _ = writeln!(
            out,
            "  (the manifest's steps could not be planned, so whether one is owed is unread: {why})"
        );
    }
    let unset = report
        .owed
        .iter()
        .filter(|c| c.state == State::Undeclared)
        .count();
    if unset > 0 {
        let _ = writeln!(
            out,
            "  ({unset} clock(s) have no interval declared and can never come due — `yidam due`)"
        );
    }

    let _ = writeln!(out, "\nin flight");
    if report.in_flight.is_empty() {
        let _ = writeln!(out, "  nothing in flight");
    }
    for f in &report.in_flight {
        let _ = writeln!(
            out,
            "  {:<10} {} — {} commit(s) since {}",
            f.ref_name, f.name, f.commits, f.started
        );
    }

    let _ = writeln!(out, "\nblocked");
    if report.blocked.is_empty() {
        let _ = writeln!(out, "  nothing is blocked");
    }
    for b in &report.blocked {
        let _ = writeln!(
            out,
            "  {:<10} {} {}",
            b.gate,
            b.node.as_deref().unwrap_or(""),
            b.detail
        );
    }

    if report.since_last.is_some() || report.since_last_unread.is_some() {
        let _ = writeln!(out, "\nsince the last round");
    }
    if let Some(why) = &report.since_last_unread {
        let _ = writeln!(out, "  {why}");
    }
    if let Some(d) = &report.since_last {
        let _ = writeln!(out, "  (recorded at {}; + arrived, - left)", d.short_head());
        let lines = d.lines();
        if lines.is_empty() {
            let _ = writeln!(out, "  nothing moved");
        }
        for l in lines {
            let _ = writeln!(out, "  {l}");
        }
    }

    let _ = writeln!(out, "\nnext");
    for a in &report.next {
        let _ = writeln!(out, "  {:<10} {}", a.source.heading(), a.act);
        let _ = writeln!(out, "  {:<10} → {}", "", a.why);
    }
    // The declaration half, said where a reader will look for it and said as what it is. A
    // repository holding no kuten is told this half needs one; it is not told that it is
    // broken, and it still got the three halves above.
    //
    // **Before the quiet line and not after**, which is where it was: a report that says
    // "nothing to do" and then prints a line about the practice is telling a reader two
    // things in the wrong order, and the one that reads as the conclusion is the one that
    // is not.
    if let Some(why) = &report.practice.undeclared {
        let _ = writeln!(out, "  {:<10} {why}", "practice");
    }
    if report.next.is_empty() {
        let _ = writeln!(out, "  nothing to do — the loop is quiet");
    }

    let owed = match (report.due, report.stale_steps.len()) {
        (0, 0) => None,
        (d, 0) => Some(format!("{d} clock(s) due")),
        (0, s) => Some(format!("{s} step(s) stale")),
        (d, s) => Some(format!("{d} clock(s) due and {s} step(s) stale")),
    };
    let head = match (owed, report.blocked.len()) {
        (None, 0) => "Nothing is due and nothing is blocked.".to_string(),
        (None, b) => format!("Nothing is due; {b} finding(s) would fail a gate."),
        (Some(o), 0) => format!("{o}, nothing blocked."),
        (Some(o), b) => format!("{o}, and {b} finding(s) would fail a gate."),
    };
    // Stated on every run, including the quiet one, for `due`'s reason: it is the sentence
    // that keeps the report from being read as a gate, and the reader who only ever sees the
    // clean run is exactly the reader who needs to be told what the clean run means.
    let line = if report.strict && !report.passed {
        "Being owed is not being broken — `--strict` is exiting nonzero on what is owed or \
         blocked, because you asked it to."
    } else {
        "Being owed is not being broken. `yidam doctor` answers what is wrong, and `yidam \
         lint` is the gate."
    };
    // The ranking is an argument, and a reader who is about to act on it is owed the fact
    // that no profile declared it. Said once, at the bottom, rather than beside every act.
    let ranking = "The order under `next` is this command's — blocked, owed, in flight, \
                   practice — and not a kuten's: no profile declares a phase order.";
    let _ = write!(out, "\n{head}\n{line}\n{ranking}");
    out
}

/// `--brief`: the report cut to `budget` tokens, to open a session with (#1443).
///
/// Written to be pasted or injected as a session's first context — after a compaction most of
/// all: the 2026-11-04 scan found agents resuming mid-task, and in 1,681 calls after a
/// compaction not one asked where the loop stood. So the acts come first, then the delta, then the halves: a reader cut
/// short should lose the detail and keep what to do.
///
/// **The budget is `pack`'s arithmetic**, a token per four bytes, and it is a ceiling: lines
/// are added in that order while they fit, and the brief says how many it left out and where
/// they are. The header and the closing line are kept whatever the budget, because a brief
/// that does not say what it is will be read as the whole report.
pub(crate) fn render_brief(report: &CycleReport, now: &Round, budget: usize) -> String {
    /// Items shown per half before the rest is counted, so one long half cannot spend the
    /// whole budget.
    const PER_HALF: usize = 5;

    let header = format!(
        "yidam cycle --brief — {} at {}",
        now.branch
            .as_deref()
            .map_or_else(|| "detached HEAD".to_string(), |b| format!("branch {b}")),
        now.short_head()
    );
    let footer = "Being owed is not being broken. `yidam cycle` is the full report.";

    let mut body: Vec<String> = Vec::new();
    let capped = |items: Vec<String>, body: &mut Vec<String>| {
        let n = items.len();
        body.extend(items.into_iter().take(PER_HALF));
        if n > PER_HALF {
            body.push(format!("  …and {} more", n - PER_HALF));
        }
    };

    if report.next.is_empty() {
        body.push("next: nothing to do — the loop is quiet".to_string());
    } else {
        body.push("next:".to_string());
        capped(
            report
                .next
                .iter()
                .enumerate()
                .map(|(i, a)| {
                    format!(
                        "  {}. [{}] {} — {}",
                        i + 1,
                        a.actor.kind().name(),
                        a.act,
                        a.why
                    )
                })
                .collect(),
            &mut body,
        );
    }

    match (&report.since_last, &report.since_last_unread) {
        (Some(d), _) => {
            let lines = d.lines();
            if lines.is_empty() {
                body.push(format!(
                    "since the round at {}: nothing moved",
                    d.short_head()
                ));
            } else {
                body.push(format!(
                    "since the round at {} (+ arrived, - left):",
                    d.short_head()
                ));
                capped(
                    lines.into_iter().map(|l| format!("  {l}")).collect(),
                    &mut body,
                );
            }
        }
        (None, Some(why)) => body.push(format!("since the last round: {why}")),
        (None, None) => {}
    }

    let mut owed: Vec<String> = report
        .owed
        .iter()
        .filter(|c| c.state == State::Due)
        .map(|c| format!("  {} — {}", c.id, c.detail))
        .collect();
    owed.extend(
        report
            .stale_steps
            .iter()
            .map(|s| format!("  run step {} — {}", s.step, s.because)),
    );
    if owed.is_empty() {
        body.push("owed: nothing is due".to_string());
    } else {
        body.push("owed:".to_string());
        capped(owed, &mut body);
    }
    if let Some(why) = &report.stale_steps_unread {
        body.push(format!("  (steps unread: {why})"));
    }

    if report.blocked.is_empty() {
        body.push("blocking: nothing".to_string());
    } else {
        body.push("blocking:".to_string());
        capped(
            report
                .blocked
                .iter()
                .map(|b| {
                    format!(
                        "  {} {} {}",
                        b.gate,
                        b.check,
                        b.node.as_deref().unwrap_or("")
                    )
                })
                .collect(),
            &mut body,
        );
    }

    if report.in_flight.is_empty() {
        body.push("in flight: nothing".to_string());
    } else {
        body.push("in flight:".to_string());
        capped(
            report
                .in_flight
                .iter()
                .map(|f| format!("  {} — {} commit(s)", f.ref_name, f.commits))
                .collect(),
            &mut body,
        );
    }

    // Fill to the ceiling. The cut-off note's own length is reserved up front, at the longest
    // it can be, so adding it cannot push a brief that fit over the budget.
    let ceiling = budget.saturating_mul(4);
    let note = |left: usize| {
        format!("({left} more line(s) past the {budget}-token budget — `yidam cycle`)")
    };
    let reserved = header.len() + 1 + footer.len() + 1 + note(body.len()).len() + 1;
    let mut out = format!("{header}\n");
    let mut used = reserved;
    let mut kept = 0;
    for line in &body {
        if used + line.len() + 1 > ceiling {
            break;
        }
        used += line.len() + 1;
        out.push_str(line);
        out.push('\n');
        kept += 1;
    }
    if kept < body.len() {
        out.push_str(&note(body.len() - kept));
        out.push('\n');
    }
    out.push_str(footer);
    out
}

#[cfg(test)]
mod tests;
