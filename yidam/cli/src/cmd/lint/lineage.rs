//! Which evolution a position is measured against.
//!
//! An elector's `ma/*` branch stands at some point in the settled line, and until RFC-0010 there
//! was no way to say which — the answer was left to git's merge-base. Measurement is what turned
//! that from a gap into a defect.
//!
//! # The inference does not work
//!
//! Asked *which evolution does this branch diverge from*, merge-base against the `rigpa/*` tips
//! answers, in the one repository that has run a sangha:
//!
//! | branch | commits ahead of `main` | merge-base says | actually holds through |
//! |---|---:|---|---|
//! | `ma/advocate` | 38 | `challenger-filings` (#23 of 29) | `claims-that-travel` (#29) |
//! | `ma/auditor` | 42 | `challenger-filings` (#23 of 29) | `claims-that-travel` (#29) |
//! | `ma/goedelsoup` | 46 | `challenger-filings` (#23 of 29) | `claims-that-travel` (#29) |
//!
//! The same answer for three branches dozens of commits apart, and **six resolutions stale for
//! every one of them**. The cause is that electors adopt through `main` — one `adopt:` commit
//! against 72 `merge main` across 126 elector commits — so merge-base is measuring against refs
//! nobody merges.
//!
//! # So this reads the settlements, not the branches
//!
//! A resolution's settlement is the commit that added its record, which is on the baseline and
//! which [`super::scope::adding_commits`] already finds for Article V. *Holds through* is the
//! latest such commit a branch contains. That is derivable today, for every branch, with no
//! convention adopted and nothing declared — which is why [`baseline_undeclared`] can tell an
//! elector what to write rather than only that something is missing.
//!
//! # What a declaration adds that the derivation cannot
//!
//! *Holding* a settlement and *being measured against* it are different facts, and separating them
//! is the whole content of RFC-0010. An elector who merges `main` picks up every settlement on it,
//! including resolutions they abstained from — 9 of 9 such cases in that repository — so the
//! derived answer says what arrived, never what the elector means to stand on. Only a declaration
//! says the second, and [`baseline_unmet`] is what makes it a claim that can be wrong.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use super::model::{Check, Severity, Violation};
use crate::cmd::sangha::Resolution;
use crate::git::RefKind;

/// One elector branch's standing against the settled line.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Standing {
    /// `ma/<elector>`.
    pub branch: String,
    /// The evolution named by the branch's most recent `Baseline:` trailer.
    pub declared: Option<String>,
    /// Whether any resolution record carries that evolution.
    pub known: bool,
    /// Whether the branch contains the declared evolution's settlement.
    pub held: bool,
    /// The latest evolution whose settlement this branch holds.
    pub through: Option<String>,
    /// How many settlements the branch does not hold.
    pub behind: usize,
    /// Resolutions this branch holds, did not take part in, and its declaration does not reach.
    ///
    /// Sorted as the line runs, so a reader gets them in the order they settled.
    pub absorbed: Vec<String>,
}

// ── the checks ────────────────────────────────────────────────────────────────

pub(crate) fn baseline_unmet(standings: &[Standing]) -> Check {
    let violations = standings
        .iter()
        .filter_map(|s| {
            let declared = s.declared.as_ref()?;
            let why = match (s.known, s.held) {
                (false, _) => format!("`{declared}` — no resolution record carries that evolution"),
                (true, false) => format!(
                    "`{declared}` — the branch does not contain that resolution's settlement"
                ),
                (true, true) => return None,
            };
            let holds = match &s.through {
                Some(e) => format!("; it holds through `{e}`"),
                None => "; it holds no settlement at all".to_string(),
            };
            Some(Violation::new(
                s.branch.clone(),
                format!("declares a baseline it does not stand on: {why}{holds}"),
            ))
        })
        .collect();
    Check::new(
        "elector-baseline-unmet",
        "Elector branch declares a baseline it does not stand on",
        Severity::Error,
        "A declared baseline is a claim — *this position is measured against that evolution* — and \
         a claim about the branch's own history is one the branch either satisfies or does not. \
         There is no state of the world in which an elector is measured against a settlement their \
         branch has never seen, so this gates. What it deliberately does not gate on is a baseline \
         that is merely *old*: divergence from the baseline is what an elector's branch is for, and \
         Article VI says so in as many words, so a position several resolutions behind is a \
         position doing its job.",
        violations,
    )
}

pub(crate) fn baseline_undeclared(standings: &[Standing]) -> Check {
    let violations = standings
        .iter()
        .filter(|s| s.declared.is_none())
        .map(|s| {
            let detail = match (&s.through, s.behind) {
                (Some(e), 0) => format!(
                    "no `Baseline:` trailer — it holds every settlement, through `{e}`"
                ),
                (Some(e), n) => format!(
                    "no `Baseline:` trailer — it holds through `{e}`, and {n} settlement(s) it does not"
                ),
                (None, _) => "no `Baseline:` trailer, and it holds no settlement to name".to_string(),
            };
            Violation::new(s.branch.clone(), detail)
        })
        .collect();
    Check::new(
        "elector-baseline-undeclared",
        "Elector branch does not say which evolution it is measured against",
        Severity::Info,
        "Info, and it carries the answer rather than only the complaint: every branch is \
         undeclared until somebody starts writing the trailer, so a finding that merely said \
         *missing* would be one line of noise per elector forever. What the branch holds through \
         is derivable from the settlements, so this reports it — which is both the useful half \
         today and, exactly, the thing to write down. It is deliberately not the same fact: \
         holding a settlement is what a `merge main` brings you, and being measured against one is \
         a choice. An elector who has absorbed a resolution they abstained from holds it and may \
         well not mean to stand on it.",
        violations,
    )
}

/// Holding a resolution you did not take part in and have not adopted.
///
/// **The rule this reports against was decided by measurement, and it went the other way from
/// practice.** RFC-0011 asks whether a resolution produced by a subset binds the electors who sat
/// it out. The documents cut both ways — Article VI's minimum authority and *"divergence is normal
/// and expected; it is not a violation"* against PROTOCOL's *"the active baseline"* — and while
/// they cut both ways, practice decided: in the repository that has run this protocol, **all nine
/// partial resolutions are held by the elector who abstained**, absorbed through `merge main`,
/// with no adoption act and nothing recording that it happened.
///
/// The settlement is that **the declaration binds, not the merge**. Merging `main` is how a corpus
/// stays shared and step 3 requires it mid-loop, so it cannot be the thing that binds; the
/// `Baseline:` trailer is the adoption act. Holding a settlement and being measured against one
/// are two facts, and this check is what makes the gap between them visible.
///
/// Which is why it is Info. Every one of those nine is this finding today, and none of them is
/// wrong — an elector who has absorbed a resolution and not declared it has done nothing the
/// protocol forbids. It is a prompt to decide, not an accusation, and it clears by declaring.
pub(crate) fn holds_unadopted(standings: &[Standing]) -> Check {
    let violations = standings
        .iter()
        .filter(|s| !s.absorbed.is_empty())
        .map(|s| {
            // One finding per elector rather than per resolution. Before anybody declares a
            // baseline every held resolution qualifies, and nine lines saying the same thing
            // about two branches is a report nobody reads to the end.
            let named = s.absorbed.join("`, `");
            Violation::new(
                s.branch.clone(),
                format!(
                    "holds {} resolution(s) it took no part in and has not adopted: `{named}`",
                    s.absorbed.len()
                ),
            )
        })
        .collect();
    Check::new(
        "elector-holds-unadopted",
        "Elector branch holds a resolution it took no part in and has not adopted",
        Severity::Info,
        "A resolution binds the electors whose tips it read. Everyone else may read it, cite it,          adopt it or build on it, and until they declare it as their baseline it is not what          their position is measured against — Article VI, which says the sangha exercises the          minimum authority needed and that divergence is not a violation. Merging `main` cannot          be what binds: it is how the corpus stays shared, and the protocol requires it mid-loop.          So this reports the gap between what a branch has absorbed and what it says it stands          on, at Info, because the gap is a legitimate state rather than a defect. What it is not          is invisible, which is what it was: nine of nine abstainers were bound in practice with          nothing recording it.",
        violations,
    )
}

pub(crate) fn checks(standings: &[Standing]) -> [Check; 3] {
    [
        baseline_unmet(standings),
        baseline_undeclared(standings),
        holds_unadopted(standings),
    ]
}

// ── reading the repository ────────────────────────────────────────────────────

/// The evolution a `Baseline:` trailer names.
///
/// `Baseline: rigpa/<evolution>@<short-hash>` is the documented form and the hash is optional
/// here. It pins which commit of the evolution was adopted, which matters for a branch that
/// moved; what this check asks is *which evolution*, and a record names an evolution rather than
/// a commit.
fn declared_evolution(message: &str) -> Option<String> {
    message.lines().find_map(|line| {
        let rest = line.trim().strip_prefix("Baseline:")?.trim();
        let evolution = rest.strip_prefix("rigpa/")?;
        let evolution = evolution.split('@').next().unwrap_or(evolution).trim();
        (!evolution.is_empty()).then(|| evolution.to_string())
    })
}

fn git(root: &Path, args: &[&str]) -> String {
    std::process::Command::new("git")
        .current_dir(root)
        .args(args)
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .unwrap_or_default()
}

/// Where each elector branch stands, and what it says about where it stands.
///
/// Returns nothing without touching git when there are no records — collective mode is opt-in and
/// a corpus with no resolutions has no line to stand in.
pub(crate) fn standings(root: &Path, records: &[Resolution]) -> Vec<Standing> {
    if records.is_empty() {
        return Vec::new();
    }
    let electors: Vec<crate::git::PhaseRef> = crate::git::phase_refs(root)
        .into_iter()
        .filter(|r| r.kind == RefKind::Position)
        .collect();
    if electors.is_empty() {
        return Vec::new();
    }

    // Where each settlement sits in the line. Ordered by position in the baseline's history
    // rather than by commit date: a date is a claim the committer makes and `main`'s order is
    // the one the corpus was actually built in.
    let adds = super::scope::adding_commits(root);
    let settlement: HashMap<String, String> = records
        .iter()
        .filter_map(|r| Some((r.evolution.clone(), adds.get(&r.file)?.clone())))
        .collect();
    let base = crate::git::base_branch(root).unwrap_or_else(|| "HEAD".to_string());
    let electors_md = crate::paths::yidam_sangha_dir(root)
        .strip_prefix(root)
        .unwrap_or(Path::new(".yidam/sangha"))
        .join("electors.md")
        .to_string_lossy()
        .replace('\\', "/");
    let rank: HashMap<String, usize> = git(root, &["rev-list", "--topo-order", "--reverse", &base])
        .lines()
        .enumerate()
        .map(|(i, sha)| (sha.to_string(), i))
        .collect();

    electors
        .iter()
        .map(|e| {
            let reachable: HashSet<String> = git(root, &["rev-list", &e.git_ref])
                .lines()
                .map(str::to_string)
                .collect();
            let held: Vec<(&String, usize)> = settlement
                .iter()
                .filter(|(_, sha)| reachable.contains(*sha))
                .filter_map(|(evo, sha)| rank.get(sha).map(|r| (evo, *r)))
                .collect();
            // Ties broken by name, and a tie is not exotic: one commit in the measured
            // repository added two resolution records, so two settlements genuinely share a
            // position in the line. Ranking on position alone left the answer to hash order,
            // which agreed with itself locally and disagreed in CI — the fixture's two records
            // are both added by its genesis commit, and the golden picked a different one on
            // each machine. Neither evolution is later than the other; what this owes a reader
            // is the same answer twice.
            let through = held
                .iter()
                .max_by(|a, b| a.1.cmp(&b.1).then_with(|| a.0.cmp(b.0)))
                .map(|(evo, _)| (*evo).clone());
            let declared = declared_evolution(&git(
                root,
                &[
                    "log",
                    "-n",
                    "1",
                    "--extended-regexp",
                    "--grep=^Baseline: rigpa/",
                    "--format=%B",
                    &e.git_ref,
                ],
            ));
            // When this seat was registered — the commit that first put its branch in
            // `electors.md`, which is what PROTOCOL defines becoming an elector as.
            //
            // **A resolution that settled before a seat existed is not one that seat sat out.**
            // Without this the report conflates two states: `ma/advocate` reads as holding 13
            // resolutions it "took no part in", and 8 of them predate its registration by days.
            // An elector inherits the corpus as it stands when they join; what is worth
            // reporting is what settled while they were there. Filtering on registration takes
            // the measured repository from 13 and 4 to 5 and 4 — which is exactly its count of
            // resolutions where a registered seat existed and was not read.
            //
            // A seat whose registration cannot be located reports everything, which fails
            // toward saying too much rather than too little.
            let seated_at = git(
                root,
                &[
                    "log",
                    "--reverse",
                    "--format=%H",
                    &format!("-S{}", e.name),
                    "--",
                    &electors_md,
                ],
            )
            .lines()
            .next()
            .and_then(|sha| rank.get(sha))
            .copied();

            // Where the declaration reaches. A declaration binds its holder to the line up to
            // and including that evolution; anything later is held and not adopted.
            let declared_rank = declared
                .as_ref()
                .and_then(|d| settlement.get(d))
                .and_then(|sha| rank.get(sha))
                .copied();
            let mut absorbed: Vec<(usize, String)> = records
                .iter()
                // A participant is an elector whose tip the record says was read. It is the
                // crisp definition and, measured, the only available one: nothing records who
                // was notified, so "notified and declined" cannot be told from "never asked".
                .filter(|r| {
                    !r.tips
                        .iter()
                        .any(|t| t.split('@').next().unwrap_or(t) == e.name)
                })
                .filter_map(|r| Some((r, settlement.get(&r.evolution)?)))
                .filter(|(_, sha)| reachable.contains(*sha))
                .filter_map(|(r, sha)| Some((*rank.get(sha)?, r.evolution.clone())))
                .filter(|(at, _)| declared_rank.is_none_or(|d| d < *at))
                .filter(|(at, _)| seated_at.is_none_or(|seated| seated < *at))
                .collect();
            absorbed.sort();
            Standing {
                branch: e.name.clone(),
                absorbed: absorbed.into_iter().map(|(_, e)| e).collect(),
                known: declared
                    .as_ref()
                    .is_some_and(|d| settlement.contains_key(d)),
                held: declared
                    .as_ref()
                    .and_then(|d| settlement.get(d))
                    .is_some_and(|sha| reachable.contains(sha)),
                declared,
                through,
                behind: settlement.len() - held.len(),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── the trailer ───────────────────────────────────────────────────────────

    #[test]
    fn a_trailer_names_the_evolution_with_or_without_a_hash() {
        assert_eq!(
            declared_evolution("revise: x\n\nBaseline: rigpa/tailwater@abc1234").as_deref(),
            Some("tailwater")
        );
        assert_eq!(
            declared_evolution("revise: x\n\nBaseline: rigpa/tailwater").as_deref(),
            Some("tailwater")
        );
    }

    /// The `rigpa/` prefix is required. A trailer naming something else is not a baseline
    /// declaration, and reading it as one would invent an evolution out of a passing mention.
    #[test]
    fn a_trailer_that_names_no_evolution_declares_nothing() {
        assert!(declared_evolution("revise: x\n\nBaseline: main").is_none());
        assert!(declared_evolution("revise: x\n\nBaseline: rigpa/").is_none());
        assert!(declared_evolution("revise: x\n\nno trailer here").is_none());
        assert!(declared_evolution("Baselines: rigpa/x").is_none());
    }

    // ── the pure readings ─────────────────────────────────────────────────────

    fn standing(
        declared: Option<&str>,
        known: bool,
        held: bool,
        through: Option<&str>,
        behind: usize,
    ) -> Standing {
        Standing {
            branch: "ma/one".into(),
            declared: declared.map(str::to_string),
            known,
            held,
            through: through.map(str::to_string),
            behind,
            absorbed: Vec::new(),
        }
    }

    #[test]
    fn a_declared_evolution_no_record_carries_is_reported() {
        let c = baseline_unmet(&[standing(Some("nowhere"), false, false, Some("thing"), 0)]);
        assert_eq!(c.violations.len(), 1);
        assert!(
            c.violations[0]
                .detail
                .contains("no resolution record carries"),
            "{}",
            c.violations[0].detail
        );
        assert_eq!(c.severity, Severity::Error);
    }

    #[test]
    fn a_declared_evolution_the_branch_does_not_hold_is_reported() {
        let c = baseline_unmet(&[standing(Some("later"), true, false, Some("earlier"), 3)]);
        assert_eq!(c.violations.len(), 1);
        assert!(
            c.violations[0]
                .detail
                .contains("does not contain that resolution's settlement"),
            "{}",
            c.violations[0].detail
        );
        assert!(c.violations[0].detail.contains("holds through `earlier`"));
    }

    /// **The arm a naive implementation gets wrong.** All three real electors are behind the head,
    /// and an elector's branch is *supposed* to be: Article VI says divergence is normal and not a
    /// violation. Only a declaration the branch cannot stand on is a finding.
    #[test]
    fn a_baseline_several_settlements_behind_is_not_a_finding() {
        let c = baseline_unmet(&[standing(Some("earlier"), true, true, Some("earlier"), 6)]);
        assert!(c.passed(), "{c:?}");
    }

    #[test]
    fn an_undeclared_branch_is_told_what_it_holds() {
        let c = baseline_undeclared(&[standing(None, false, false, Some("claims-that-travel"), 0)]);
        assert_eq!(c.violations.len(), 1);
        assert_eq!(c.severity, Severity::Info);
        assert!(
            c.violations[0]
                .detail
                .contains("holds every settlement, through `claims-that-travel`"),
            "{}",
            c.violations[0].detail
        );
    }

    #[test]
    fn an_undeclared_branch_behind_the_line_is_told_how_far() {
        let c = baseline_undeclared(&[standing(None, false, false, Some("earlier"), 4)]);
        assert!(
            c.violations[0]
                .detail
                .contains("holds through `earlier`, and 4 settlement(s)"),
            "{}",
            c.violations[0].detail
        );
    }

    #[test]
    fn a_declared_branch_is_not_reported_as_undeclared() {
        assert!(baseline_undeclared(&[standing(Some("x"), true, true, Some("x"), 0)]).passed());
    }

    #[test]
    fn a_repository_with_no_electors_reports_nothing() {
        assert!(baseline_unmet(&[]).passed());
        assert!(baseline_undeclared(&[]).passed());
    }

    // ── against a repository ──────────────────────────────────────────────────

    fn git(dir: &Path, args: &[&str]) {
        let ok = std::process::Command::new("git")
            .current_dir(dir)
            .args(args)
            .status()
            .unwrap()
            .success();
        assert!(ok, "git {args:?} failed");
    }

    fn commit(dir: &Path, msg: &str) {
        git(dir, &["add", "-A"]);
        let ok = std::process::Command::new("git")
            .current_dir(dir)
            .args(["commit", "-q", "--allow-empty", "--no-gpg-sign", "-m", msg])
            .env("GIT_AUTHOR_DATE", "2026-01-01T00:00:00Z")
            .env("GIT_COMMITTER_DATE", "2026-01-01T00:00:00Z")
            .status()
            .unwrap()
            .success();
        assert!(ok, "commit failed");
    }

    fn record(dir: &Path, evolution: &str) {
        let p = dir
            .join(".yidam/sangha/resolutions")
            .join(format!("{evolution}.md"));
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(
            p,
            format!("---\nevolution: {evolution}\ndate: 2026-01-01\ntips:\n  - ma/one@0000000\n---\n\n## What was resolved\n\nA thing.\n"),
        )
        .unwrap();
        commit(dir, &format!("synthesize: {evolution}"));
    }

    /// A baseline with three settlements and one elector branch parked after the second.
    ///
    /// **Two held settlements, not one.** With a branch holding a single settlement, "the latest
    /// one it holds" and "the earliest one it holds" are the same string, and a fixture that
    /// cannot tell those apart cannot tell whether `through` is computed at all.
    fn repo() -> tempfile::TempDir {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path();
        git(root, &["init", "-q", "-b", "main"]);
        git(root, &["config", "user.email", "t@t.com"]);
        git(root, &["config", "user.name", "T"]);
        git(root, &["config", "commit.gpgsign", "false"]);
        std::fs::create_dir_all(root.join(".yidam/sangha/electors")).ok();
        std::fs::write(
            root.join(".yidam/sangha/electors.md"),
            "| Name | Branch | Role |\n|---|---|---|\n| `one` | `ma/one` | Holds a position. |\n",
        )
        .unwrap();
        commit(root, "genesis: the sangha");
        record(root, "first");
        record(root, "second");
        git(root, &["branch", "ma/one"]);
        record(root, "third");
        tmp
    }

    fn run(root: &Path) -> [Check; 3] {
        let data = crate::cmd::sangha::sangha_data(root);
        checks(&standings(root, &data.resolutions))
    }

    /// The branch sits after `second` and before `third`, which is the ordinary state of an
    /// elector position. It must be told what it holds, and told nothing is wrong.
    ///
    /// `second`, not `first`: it holds both, and the answer is the *latest* of them.
    #[test]
    fn an_undeclared_branch_is_told_the_settlement_it_stands_on() {
        let tmp = repo();
        let [unmet, undeclared, _] = run(tmp.path());
        assert!(unmet.passed(), "{unmet:?}");
        assert_eq!(undeclared.violations.len(), 1, "{undeclared:?}");
        let d = &undeclared.violations[0].detail;
        assert!(d.contains("holds through `second`"), "{d}");
        assert!(d.contains("1 settlement(s) it does not"), "{d}");
    }

    /// **Stale but valid, end to end.** The branch declares `first` while holding through
    /// `second` and sitting behind `third`. Declared, held and behind are three different facts
    /// here, which is the point: only the middle one is a finding when it fails.
    #[test]
    fn a_branch_declaring_a_baseline_it_stands_on_is_green_however_far_behind() {
        let tmp = repo();
        let root = tmp.path();
        git(root, &["switch", "-q", "ma/one"]);
        commit(root, "revise: my position\n\nBaseline: rigpa/first@0000000");
        git(root, &["switch", "-q", "main"]);

        let [unmet, undeclared, _] = run(root);
        assert!(unmet.passed(), "{unmet:?}");
        assert!(undeclared.passed(), "{undeclared:?}");
    }

    /// The claim the check exists to falsify: a branch that says it is measured against a
    /// settlement it has never seen.
    #[test]
    fn a_branch_declaring_a_settlement_it_never_saw_is_caught() {
        let tmp = repo();
        let root = tmp.path();
        git(root, &["switch", "-q", "ma/one"]);
        commit(root, "revise: my position\n\nBaseline: rigpa/third@0000000");
        git(root, &["switch", "-q", "main"]);

        let [unmet, _, _] = run(root);
        assert_eq!(unmet.violations.len(), 1, "{unmet:?}");
        assert!(
            unmet.violations[0].detail.contains("does not contain"),
            "{}",
            unmet.violations[0].detail
        );
    }

    #[test]
    fn a_branch_declaring_an_evolution_that_does_not_exist_is_caught() {
        let tmp = repo();
        let root = tmp.path();
        git(root, &["switch", "-q", "ma/one"]);
        commit(
            root,
            "revise: my position\n\nBaseline: rigpa/invented@0000000",
        );
        git(root, &["switch", "-q", "main"]);

        let [unmet, _, _] = run(root);
        assert_eq!(unmet.violations.len(), 1, "{unmet:?}");
        assert!(
            unmet.violations[0]
                .detail
                .contains("no resolution record carries"),
            "{}",
            unmet.violations[0].detail
        );
    }

    /// The most recent declaration wins. An elector who adopts twice has said two things, and the
    /// second is the one that describes where they now stand.
    #[test]
    fn the_latest_declaration_is_the_one_read() {
        let tmp = repo();
        let root = tmp.path();
        git(root, &["switch", "-q", "ma/one"]);
        commit(root, "revise: early\n\nBaseline: rigpa/invented@0000000");
        commit(
            root,
            "adopt: the baseline after first\n\nBaseline: rigpa/first@0000000",
        );
        git(root, &["switch", "-q", "main"]);

        let [unmet, _, _] = run(root);
        assert!(
            unmet.passed(),
            "the superseded declaration still gates: {unmet:?}"
        );
    }

    /// And the other direction, which is the half that pins it. Above, reading the *older*
    /// declaration would have reported a finding; here it would hide one. A test that only ever
    /// asserts green cannot tell "the newest wins" from "any passing one wins".
    #[test]
    fn a_later_declaration_that_fails_is_not_saved_by_an_earlier_one_that_passed() {
        let tmp = repo();
        let root = tmp.path();
        git(root, &["switch", "-q", "ma/one"]);
        commit(
            root,
            "adopt: the baseline after first\n\nBaseline: rigpa/first@0000000",
        );
        commit(
            root,
            "revise: later, and wrong\n\nBaseline: rigpa/third@0000000",
        );
        git(root, &["switch", "-q", "main"]);

        let [unmet, _, _] = run(root);
        assert_eq!(
            unmet.violations.len(),
            1,
            "the earlier declaration masked it: {unmet:?}"
        );
    }

    /// Two settlements in one commit share a position in the line, and the answer must not
    /// depend on hash order. This is the defect CI found and a local run could not: the report
    /// fixture adds both its records in one commit, and the golden named a different one on each
    /// machine.
    #[test]
    fn two_settlements_in_one_commit_resolve_the_same_way_every_time() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path();
        git(root, &["init", "-q", "-b", "main"]);
        git(root, &["config", "user.email", "t@t.com"]);
        git(root, &["config", "user.name", "T"]);
        git(root, &["config", "commit.gpgsign", "false"]);
        std::fs::write(
            {
                let d = root.join(".yidam/sangha");
                std::fs::create_dir_all(d.join("resolutions")).unwrap();
                d.join("electors.md")
            },
            "| Name | Branch | Role |\n|---|---|---|\n| `one` | `ma/one` | Holds a position. |\n",
        )
        .unwrap();
        for evolution in ["zeta", "alpha"] {
            std::fs::write(
                root.join(".yidam/sangha/resolutions").join(format!("{evolution}.md")),
                format!("---\nevolution: {evolution}\ndate: 2026-01-01\ntips:\n  - ma/one@0000000\n---\n\n## What was resolved\n\nBoth at once.\n"),
            )
            .unwrap();
        }
        commit(root, "synthesize: two questions, one commit");
        git(root, &["branch", "ma/one"]);

        let answers: Vec<Option<String>> = (0..8)
            .map(|_| {
                let data = crate::cmd::sangha::sangha_data(root);
                standings(root, &data.resolutions)[0].through.clone()
            })
            .collect();
        assert!(
            answers.iter().all(|a| a == &answers[0]),
            "the answer moved between runs: {answers:?}"
        );
        assert_eq!(answers[0].as_deref(), Some("zeta"), "ties break by name");
    }

    // ── holding without adopting ──────────────────────────────────────────────

    fn absorbing(branch: &str, absorbed: &[&str]) -> Standing {
        Standing {
            branch: branch.into(),
            absorbed: absorbed.iter().map(|s| s.to_string()).collect(),
            ..Default::default()
        }
    }

    #[test]
    fn an_elector_holding_what_it_never_adopted_is_reported() {
        let c = holds_unadopted(&[absorbing("ma/one", &["alpha", "beta"])]);
        assert_eq!(c.violations.len(), 1);
        assert_eq!(c.severity, Severity::Info);
        let d = &c.violations[0].detail;
        assert!(d.contains("holds 2 resolution(s)"), "{d}");
        assert!(d.contains("`alpha`, `beta`"), "{d}");
    }

    /// One finding per elector, not per resolution. Before anybody declares a baseline every
    /// held resolution qualifies, and the measured repository would produce eight lines about
    /// two branches.
    #[test]
    fn an_elector_is_named_once_however_much_it_holds() {
        let c = holds_unadopted(&[absorbing("ma/one", &["a", "b", "c", "d"])]);
        assert_eq!(c.violations.len(), 1, "{c:?}");
    }

    #[test]
    fn an_elector_holding_nothing_unadopted_is_not_reported() {
        assert!(holds_unadopted(&[absorbing("ma/one", &[])]).passed());
        assert!(holds_unadopted(&[]).passed());
    }

    /// A sangha of two, where `two` is registered at the start and sits out the second
    /// resolution. `electors.md` is written before either settles, so registration is not what
    /// excludes anything here.
    fn sangha() -> tempfile::TempDir {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path();
        git(root, &["init", "-q", "-b", "main"]);
        git(root, &["config", "user.email", "t@t.com"]);
        git(root, &["config", "user.name", "T"]);
        git(root, &["config", "commit.gpgsign", "false"]);
        let dir = root.join(".yidam/sangha");
        std::fs::create_dir_all(dir.join("resolutions")).unwrap();
        std::fs::write(
            dir.join("electors.md"),
            "| Name | Branch | Role |\n|---|---|---|\n\
             | `one` | `ma/one` | Holds a position. |\n\
             | `two` | `ma/two` | Holds another. |\n",
        )
        .unwrap();
        commit(root, "genesis: the sangha");
        tmp
    }

    fn resolution(root: &Path, evolution: &str, tips: &[&str]) {
        let list: String = tips.iter().map(|t| format!("  - {t}@0000000\n")).collect();
        std::fs::write(
            root.join(".yidam/sangha/resolutions")
                .join(format!("{evolution}.md")),
            format!("---\nevolution: {evolution}\ndate: 2026-01-01\ntips:\n{list}---\n\n## What was resolved\n\nA thing.\n"),
        )
        .unwrap();
        commit(root, &format!("synthesize: {evolution}"));
    }

    /// The nine-of-nine shape, end to end: an elector whose tip was not read, whose branch holds
    /// the settlement anyway because it merged the baseline, and which has declared nothing.
    #[test]
    fn an_abstainer_that_merged_the_baseline_is_reported() {
        let tmp = sangha();
        let root = tmp.path();
        resolution(root, "both", &["ma/one", "ma/two"]);
        git(root, &["branch", "ma/one"]);
        git(root, &["branch", "ma/two"]);
        resolution(root, "without-two", &["ma/one"]);
        // `two` merges the baseline, which is how all nine were absorbed.
        git(root, &["switch", "-q", "ma/two"]);
        git(root, &["merge", "--no-ff", "--no-edit", "-q", "main"]);
        git(root, &["switch", "-q", "main"]);

        let [_, _, unadopted] = run(root);
        assert_eq!(unadopted.violations.len(), 1, "{unadopted:?}");
        assert_eq!(unadopted.violations[0].node, "ma/two");
        let d = &unadopted.violations[0].detail;
        assert!(d.contains("`without-two`"), "{d}");
        assert!(!d.contains("`both`"), "a participant's own resolution: {d}");
    }

    /// Declaring it is the adoption act, and it clears the finding. This is the whole content of
    /// the rule: the declaration binds, the merge does not.
    #[test]
    fn declaring_the_baseline_adopts_what_the_branch_holds() {
        let tmp = sangha();
        let root = tmp.path();
        resolution(root, "both", &["ma/one", "ma/two"]);
        git(root, &["branch", "ma/one"]);
        git(root, &["branch", "ma/two"]);
        resolution(root, "without-two", &["ma/one"]);
        git(root, &["switch", "-q", "ma/two"]);
        git(root, &["merge", "--no-ff", "--no-edit", "-q", "main"]);
        commit(
            root,
            "revise: my position\n\nBaseline: rigpa/without-two@0000000",
        );
        git(root, &["switch", "-q", "main"]);

        let [unmet, _, unadopted] = run(root);
        assert!(unmet.passed(), "{unmet:?}");
        assert!(unadopted.passed(), "{unadopted:?}");
    }

    /// Absorbed resolutions come back in the order they settled, and the same order every time.
    ///
    /// The list is built by filtering a map, so without an explicit sort it is hash order — the
    /// defect CI found in `through` (`c338a4d`), one field over. A single-element fixture cannot
    /// see it, which is why this one holds two.
    #[test]
    fn what_a_branch_absorbed_comes_back_in_settlement_order_every_time() {
        let tmp = sangha();
        let root = tmp.path();
        resolution(root, "both", &["ma/one", "ma/two"]);
        git(root, &["branch", "ma/one"]);
        git(root, &["branch", "ma/two"]);
        resolution(root, "zeta-without-two", &["ma/one"]);
        resolution(root, "alpha-without-two", &["ma/one"]);
        git(root, &["switch", "-q", "ma/two"]);
        git(root, &["merge", "--no-ff", "--no-edit", "-q", "main"]);
        git(root, &["switch", "-q", "main"]);

        let runs: Vec<Vec<String>> = (0..8)
            .map(|_| {
                let data = crate::cmd::sangha::sangha_data(root);
                standings(root, &data.resolutions)
                    .into_iter()
                    .find(|s| s.branch == "ma/two")
                    .unwrap()
                    .absorbed
            })
            .collect();
        assert!(
            runs.iter().all(|r| r == &runs[0]),
            "the order moved between runs: {runs:?}"
        );
        // Settlement order, not alphabetical: `zeta` settled first.
        assert_eq!(runs[0], ["zeta-without-two", "alpha-without-two"]);
    }

    /// **A resolution that settled before a seat existed is not one that seat sat out.** In the
    /// measured repository this is the difference between reporting 13 and reporting 4 for one
    /// elector: eight of the thirteen predate its registration by days.
    #[test]
    fn a_resolution_older_than_the_seat_is_inherited_rather_than_abstained_from() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path();
        git(root, &["init", "-q", "-b", "main"]);
        git(root, &["config", "user.email", "t@t.com"]);
        git(root, &["config", "user.name", "T"]);
        git(root, &["config", "commit.gpgsign", "false"]);
        let dir = root.join(".yidam/sangha");
        std::fs::create_dir_all(dir.join("resolutions")).unwrap();
        std::fs::write(
            dir.join("electors.md"),
            "| Name | Branch | Role |\n|---|---|---|\n| `one` | `ma/one` | First. |\n",
        )
        .unwrap();
        commit(root, "genesis: one elector");
        resolution(root, "before-two", &["ma/one"]);
        // `two` joins afterwards.
        std::fs::write(
            dir.join("electors.md"),
            "| Name | Branch | Role |\n|---|---|---|\n\
             | `one` | `ma/one` | First. |\n| `two` | `ma/two` | Joined later. |\n",
        )
        .unwrap();
        commit(root, "open: two joins the sangha");
        git(root, &["branch", "ma/two"]);
        resolution(root, "after-two", &["ma/one"]);
        git(root, &["switch", "-q", "ma/two"]);
        git(root, &["merge", "--no-ff", "--no-edit", "-q", "main"]);
        git(root, &["switch", "-q", "main"]);

        let [_, _, unadopted] = run(root);
        assert_eq!(unadopted.violations.len(), 1, "{unadopted:?}");
        let d = &unadopted.violations[0].detail;
        assert!(d.contains("`after-two`"), "{d}");
        assert!(
            !d.contains("`before-two`"),
            "it could not have taken part: {d}"
        );
    }

    /// Only `ma/*` is an elector. `rigpa/*` is a settled evolution and `phase/*` a bounded
    /// investigation; neither has a position to be measured, and reporting one as an undeclared
    /// elector would be the same conflation that once had a derived repository reporting 26 active
    /// phases while holding one.
    #[test]
    fn only_elector_branches_are_asked_where_they_stand() {
        let tmp = repo();
        let root = tmp.path();
        git(root, &["branch", "rigpa/third"]);
        git(root, &["branch", "phase/a-survey"]);

        let [_, undeclared, _] = run(root);
        let named: Vec<&str> = undeclared
            .violations
            .iter()
            .map(|v| v.node.as_str())
            .collect();
        assert_eq!(named, ["ma/one"], "{undeclared:?}");
    }

    /// Collective mode is opt-in, and a corpus with no resolutions has no line to stand in.
    #[test]
    fn a_corpus_with_no_resolutions_reports_nothing() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path();
        git(root, &["init", "-q", "-b", "main"]);
        git(root, &["config", "user.email", "t@t.com"]);
        git(root, &["config", "user.name", "T"]);
        git(root, &["config", "commit.gpgsign", "false"]);
        commit(root, "genesis: nothing collective");
        assert!(standings(root, &[]).is_empty());
    }
}
