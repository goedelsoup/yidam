//! `yidam due` — the four clocks, read together, on a schedule.
//!
//! Four clocks exist in a derived repository and nothing read them as a set: index
//! staleness, a catalog entry's TTL, how long a question has gone unanswered, and how long a
//! phase has been in flight. Each had a home — `doctor`, `lint`, `status`, `phases` — and
//! each answered when somebody already suspected something.
//!
//! A practice is not performed because you suspect something. It is performed because it is
//! time, and nothing here said it was time.
//!
//! # Why this is not `doctor`, and must not become it
//!
//! `doctor` answers *is this setup sound now*. It writes nothing, does no network, and exits
//! nonzero on what is wrong — three properties worth keeping, and the wrong three for a
//! practice. A diagnostic is read under suspicion; a schedule is read on a cadence.
//!
//! The distinction that matters most is in the verdicts. **What is due and what is wrong are
//! different reports.** A corpus with three expired sources is not unhealthy — nothing about
//! it is broken, no traversal will lie, and the gate is green. It is *owed*. Folding that
//! into `doctor`'s `warn` would tell a reader that a repository doing exactly what it is
//! meant to do has a problem, and the reader would learn to ignore the line.
//!
//! So [`State`] is its own enum rather than a reuse of `doctor`'s `Verdict`, and this command
//! exits zero however much is due unless `--strict` asks otherwise.
//!
//! # Intervals are declared, and three of the four were not
//!
//! A clock is an age and an interval, and the interval is never compiled in — the argument is
//! [`crate::config::LintConfig::escalate_after`]'s and applies unchanged: a number in the
//! binary is one corpus's judgement arriving in another that never agreed to it.
//!
//! One of the four already had its interval declared in the right place: a source's TTL, per
//! entry or as `[catalog] ttl_days`. This command reads it there rather than restating it.
//! The other three are `[due]` keys, absent by default, and a clock with no interval reports
//! what it measured and calls nothing due.
//!
//! # What discharges a clock, checked rather than assumed
//!
//! #289 asks for a pass that reports what is due "and, under E4 (#269), proposes the commits
//! that would discharge them." Reading the four against what `propose` actually drafts
//! corrects that for three of them:
//!
//! | Clock | What discharges it |
//! |---|---|
//! | index | `yidam index-build`. Not a commit `propose` can draft — it is a build. |
//! | catalog | `yidam propose`, which already drafts an `open:` per expired source. |
//! | questions | A person. Deciding a question is answered is the resolution event Article V confines to a sangha, and `propose` says so. |
//! | phases | A person. Merging a phase, or abandoning it, is not a mechanical consequence of a finding. |
//!
//! One of four. That is the honest surface, and the remedy column says so per clock rather
//! than this command growing a `--propose` that would shell into a command a reader can run.

use anyhow::Result;
use std::path::Path;

use crate::paths::{repo_root, require_yidam_repo, yidam_catalog_dir, yidam_corpus_dir};

/// What one clock concluded.
///
/// Serialized lowercase, so a consumer switching on `"due"` need not know Rust's
/// capitalization habits.
///
/// Deliberately **not** `doctor`'s `Verdict`. The two enums would have the same shape and
/// mean different things: `Warn` says *this is probably wrong*, and `Due` says *this is
/// exactly right and it is time*. Sharing the type is how the two reports come to be read as
/// one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum State {
    /// The interval is declared and something is past it.
    Due,
    /// The interval is declared and nothing is past it.
    Ok,
    /// No interval is declared, so nothing here can be due. What was measured is still
    /// reported: a clock nobody has set is not a clock that reads zero.
    Undeclared,
    /// The interval is declared and there is nothing to measure against — no history, no
    /// date, no index metadata.
    Unmeasurable,
}

impl State {
    fn tag(self) -> &'static str {
        match self {
            State::Due => "due",
            State::Ok => "ok",
            State::Undeclared => "—",
            State::Unmeasurable => "?",
        }
    }
}

/// One clock: what it measures, what this corpus said about it, and what it found.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Clock {
    /// Stable identifier. A consumer keys on this; the prose is free to change.
    pub id: &'static str,
    /// The question in the form a person would ask it.
    pub question: &'static str,
    pub state: State,
    /// The declaration that sets this clock's interval, as a reader would write it —
    /// `[due] questions_after`, or `ttl_days` for the one declared per entry. `None` when
    /// this corpus has not set it.
    pub interval: Option<String>,
    /// How many subjects are past the interval — expired sources, unanswered questions,
    /// refs in flight, changed corpus files. Zero on every state but [`State::Due`].
    ///
    /// One case is a count of one rather than of a measurement: an index that was never
    /// built. It is not stale by any number of files, and a corpus that declared the
    /// interval has said it wants one, so the thing owed is the index itself.
    pub overdue: usize,
    /// What was actually measured, whether or not an interval applies.
    pub detail: String,
    /// The command or act that discharges it. `None` when nothing is due.
    pub remedy: Option<String>,
}

impl Clock {
    const INDEX: &'static str = "index";
    const CATALOG: &'static str = "catalog";
    const QUESTIONS: &'static str = "questions";
    const PHASES: &'static str = "phases";

    fn new(
        id: &'static str,
        question: &'static str,
        state: State,
        detail: impl Into<String>,
    ) -> Self {
        Self {
            id,
            question,
            state,
            interval: None,
            overdue: 0,
            detail: detail.into(),
            remedy: None,
        }
    }

    fn at(mut self, interval: impl Into<String>) -> Self {
        self.interval = Some(interval.into());
        self
    }

    fn owing(mut self, overdue: usize, remedy: &str) -> Self {
        self.overdue = overdue;
        self.remedy = Some(remedy.to_string());
        self
    }

    /// The line a corpus that has not set this clock should read.
    ///
    /// A remedy on an [`State::Undeclared`] clock, deliberately: the thing to do about a
    /// clock nobody set is to set it, and a report that measured something and then said
    /// nothing about how to act on it is the shape of finding this roadmap keeps closing.
    fn unset(mut self, where_: &str) -> Self {
        self.remedy = Some(format!("declare {where_}"));
        self
    }
}

#[derive(Debug, serde::Serialize)]
pub struct DueReport {
    pub clocks: Vec<Clock>,
    /// Clocks past their interval.
    pub due: usize,
    /// Clocks this corpus has not set an interval for.
    pub undeclared: usize,
    pub strict: bool,
    /// Whether the run exits zero. True unless `--strict` and something is due — being owed
    /// is not a failure, and this field exists so a consumer does not infer one.
    pub passed: bool,
}

impl DueReport {
    fn new(clocks: Vec<Clock>, strict: bool) -> Self {
        let due = clocks.iter().filter(|c| c.state == State::Due).count();
        let undeclared = clocks
            .iter()
            .filter(|c| c.state == State::Undeclared)
            .count();
        Self {
            passed: !strict || due == 0,
            clocks,
            due,
            undeclared,
            strict,
        }
    }
}

// ── the four clocks ───────────────────────────────────────────────────────────

/// Is the index built against the corpus as it stands?
///
/// The only clock whose subject is a build rather than a fact about the corpus, and the only
/// one whose absence is itself the finding: an index that was never built cannot be stale,
/// and a repository that declared an interval has said it wants one.
fn clock_index(root: &Path, after: Option<usize>) -> Clock {
    const Q: &str = "Is the index built against the corpus as it stands?";
    let data = crate::cmd::index_status_data(root);
    let remedy = "yidam index-build  (needs the `index` feature)";

    let measured = match (data.index_present, data.meta_present) {
        (false, _) => "no index has been built".to_string(),
        (true, false) => "index present, carrying no readable meta.json".to_string(),
        (true, true) => format!(
            "built {}, {} corpus file(s) changed since",
            data.built.clone().unwrap_or_default(),
            data.stale_nodes
        ),
    };

    let Some(after) = after else {
        return Clock::new(Clock::INDEX, Q, State::Undeclared, measured)
            .unset("`[due] index_after` in .yidam/config.toml");
    };
    let interval = format!("[due] index_after = {after}");
    if !data.index_present {
        return Clock::new(Clock::INDEX, Q, State::Due, measured)
            .at(interval)
            .owing(1, remedy);
    }
    if !data.meta_present {
        return Clock::new(Clock::INDEX, Q, State::Unmeasurable, measured).at(interval);
    }
    if data.stale_nodes >= after {
        return Clock::new(Clock::INDEX, Q, State::Due, measured)
            .at(interval)
            .owing(data.stale_nodes, remedy);
    }
    Clock::new(Clock::INDEX, Q, State::Ok, measured).at(interval)
}

/// Have any source records aged past what this corpus said they may?
///
/// The interval is read from `[catalog] ttl_days` and each entry's own `ttl_days:`, which is
/// where it already lived. Reading it from a `[due]` key instead would be a second place to
/// set one number.
///
/// **No network, and none is possible from here.** Every age is computed from what is
/// recorded — the entry's `retrieved:`, or the commit that last touched its file. An expiry
/// does not claim the upstream changed. It claims nobody has looked.
fn clock_catalog(root: &Path, today: i64) -> Clock {
    const Q: &str = "Have any source records aged past their TTL?";
    let dir = yidam_catalog_dir(root);
    let sources = crate::cmd::lint::checks::load_sources(
        root,
        &crate::walk::walk_md_files(&dir),
        &Default::default(),
    );
    let default_ttl = crate::config::load_yidam_config(root)
        .map(|c| c.catalog.ttl_days)
        .unwrap_or_default();
    let iso = crate::cmd::export::unix_to_iso(today as u64 * 86_400);
    let ages = crate::cmd::lint::ttl::ages(
        &sources,
        &crate::cmd::lint::ttl::committed_dates(root, &dir),
        default_ttl,
        iso.split('T').next().unwrap_or_default(),
    );

    let governed = ages.iter().filter(|a| a.ttl_days.is_some()).count();
    if governed == 0 {
        return Clock::new(
            Clock::CATALOG,
            Q,
            State::Undeclared,
            format!("{} source(s), none under a TTL", ages.len()),
        )
        .unset(
            "`ttl_days:` on an entry, or `[catalog] ttl_days` in .yidam/config.toml for all \
             of them",
        );
    }

    let interval = match default_ttl {
        Some(d) => format!("[catalog] ttl_days = {d}, and each entry's own"),
        None => "each entry's own `ttl_days:`".to_string(),
    };
    let expired: Vec<&crate::cmd::lint::ttl::Age> =
        ages.iter().filter(|a| a.overdue_days().is_some()).collect();
    let undatable = ages.iter().filter(|a| a.undatable()).count();

    // Reported as its own state rather than folded into the expired count. A corpus that
    // asked to be told when a record aged, and cannot be told, has a gap in its bookkeeping
    // and not a stale source — and calling the second one due would assert something nobody
    // knows.
    let mut measured = format!("{governed} of {} source(s) under a TTL", ages.len());
    if undatable > 0 {
        measured.push_str(&format!("; {undatable} with no date to measure against"));
    }

    let Some(worst) = expired
        .iter()
        .max_by_key(|a| a.overdue_days().unwrap_or_default())
    else {
        if undatable > 0 {
            return Clock::new(
                Clock::CATALOG,
                Q,
                State::Unmeasurable,
                format!("none expired; {measured}"),
            )
            .at(interval);
        }
        return Clock::new(
            Clock::CATALOG,
            Q,
            State::Ok,
            format!("none expired; {measured}"),
        )
        .at(interval);
    };

    Clock::new(
        Clock::CATALOG,
        Q,
        State::Due,
        format!(
            "{} expired, worst {} day(s) past ({}); {measured}",
            expired.len(),
            worst.overdue_days().unwrap_or_default(),
            worst.entry
        ),
    )
    .at(interval)
    .owing(
        expired.len(),
        "yidam propose  (drafts an `open:` against each expired source)",
    )
}

/// How long has a question gone unanswered?
///
/// **The count comes from the working tree, the age from history**, which is
/// `orphan_in_dated`'s arrangement and is deliberate: the questions are the ones a reader
/// would see now, and the clock is `HEAD`, so an uncommitted question is counted and has no
/// age yet.
///
/// The replay is the expensive read here, and it runs only when this corpus declared an
/// interval *and* there is at least one question to date. A corpus that never set the clock
/// pays for a directory walk and nothing else.
fn clock_questions(root: &Path, after: Option<usize>) -> Clock {
    const Q: &str = "How long has a question gone unanswered?";
    let open = open_at_head(root);

    let Some(after) = after else {
        return Clock::new(
            Clock::QUESTIONS,
            Q,
            State::Undeclared,
            format!("{} open question(s)", open.len()),
        )
        .unset("`[due] questions_after` in .yidam/config.toml");
    };
    let interval = format!("[due] questions_after = {after}");
    if open.is_empty() {
        return Clock::new(Clock::QUESTIONS, Q, State::Ok, "no open questions").at(interval);
    }

    let ages = crate::cmd::lint::history::open_question_age(root);
    let mut overdue: Vec<(&String, usize)> = open
        .iter()
        .filter_map(|n| ages.get(n).map(|a| (n, a.commits)))
        .filter(|(_, commits)| *commits >= after)
        .collect();
    overdue.sort_by_key(|(_, commits)| std::cmp::Reverse(*commits));

    let Some((node, longest)) = overdue.first() else {
        // Every question is younger than the interval, or is too new to have a committed
        // age. Both are the same answer to the question asked: nothing is owed here.
        return Clock::new(
            Clock::QUESTIONS,
            Q,
            State::Ok,
            format!(
                "{} open question(s), none past {after} commit(s)",
                open.len()
            ),
        )
        .at(interval);
    };

    Clock::new(
        Clock::QUESTIONS,
        Q,
        State::Due,
        format!(
            "{} of {} open question(s) past {after} commit(s), longest {longest} ({node})",
            overdue.len(),
            open.len()
        ),
    )
    .at(interval)
    // Not `propose`, and the omission is the constitutional line rather than a gap.
    // Deciding a question is answered is a resolution event, and `cmd/sangha.rs` and
    // RFC-0020 both put that outside what a tool may perform.
    .owing(
        overdue.len(),
        "yidam open-questions  (answering one is a resolution event, not a drafted commit)",
    )
}

/// Every node that reads as an open question at HEAD, repo-relative.
///
/// `open-questions`' own walk, called rather than reproduced. The remedy this clock prints
/// sends a reader to that command, and a clock counting a set that command does not list
/// would be the third implementation of one predicate this repository has had to consolidate.
/// The near-miss it avoids is small and real: a node whose YAML does not parse still counts
/// there, because the tag is found in its prose.
fn open_at_head(root: &Path) -> Vec<String> {
    crate::cmd::corpus::open_questions_data(root, &yidam_corpus_dir(root))
        .open_questions
        .into_iter()
        .map(|q| q.node)
        .collect()
}

/// How long has a bounded inquiry been in flight?
///
/// Reads `yidam phases`' own rows rather than the refs directly, so the two commands cannot
/// disagree about which refs are active. That disagreement is not hypothetical: `status`
/// reported **26 active phases** against a true count of 1 for exactly as long as the
/// classification lived in more than one place (#272).
///
/// **Positions are excluded by construction.** A standing `ma/*` ref is meant to sit ahead
/// of the baseline forever, so asking how long it has been in flight is a category error
/// rather than a stale clock. `phases` already answers that with `state`, and this reads it.
fn clock_phases(root: &Path, after: Option<u32>, today: i64) -> Clock {
    const Q: &str = "How long has a bounded inquiry been in flight?";
    let rows = crate::cmd::phases::collect_phases(root).unwrap_or_default();
    let active: Vec<&crate::cmd::phases::PhaseRow> =
        rows.iter().filter(|r| r.state == "active").collect();

    let Some(after) = after else {
        return Clock::new(
            Clock::PHASES,
            Q,
            State::Undeclared,
            format!("{} inquiry ref(s) in flight", active.len()),
        )
        .unset("`[due] phases_after` in .yidam/config.toml");
    };
    let interval = format!("[due] phases_after = {after}");
    if active.is_empty() {
        return Clock::new(Clock::PHASES, Q, State::Ok, "nothing in flight").at(interval);
    }

    // A row whose start date git could not give (`—`) is skipped rather than counted as
    // zero. A ref with no readable history is unmeasured, and reporting it as fresh would
    // be the flattering direction to be wrong in.
    let mut aged: Vec<(&str, i64)> = active
        .iter()
        .filter_map(|r| {
            let started = crate::dates::days_from_civil_str(&r.started)?;
            Some((r.ref_name.as_str(), today - started))
        })
        .collect();
    aged.sort_by_key(|(_, days)| std::cmp::Reverse(*days));

    let unmeasured = active.len() - aged.len();
    let overdue: Vec<&(&str, i64)> = aged.iter().filter(|(_, d)| *d >= after as i64).collect();

    let Some((worst, days)) = overdue.first().copied() else {
        let mut detail = format!("{} in flight, none past {after} day(s)", active.len());
        if unmeasured > 0 {
            detail.push_str(&format!("; {unmeasured} with no readable start date"));
        }
        let state = match unmeasured {
            0 => State::Ok,
            _ => State::Unmeasurable,
        };
        return Clock::new(Clock::PHASES, Q, state, detail).at(interval);
    };

    Clock::new(
        Clock::PHASES,
        Q,
        State::Due,
        format!(
            "{} of {} in flight past {after} day(s), longest {days} ({worst})",
            overdue.len(),
            active.len()
        ),
    )
    .at(interval)
    // Neither settling a phase nor abandoning it is a mechanical consequence of a finding,
    // so there is nothing here for `propose` to draft. The remedy names where to look.
    .owing(
        overdue.len(),
        "yidam phases  (settle it, or say why it stands)",
    )
}

// ── assembly ──────────────────────────────────────────────────────────────────

/// Read every clock against `root`.
///
/// `today` is passed rather than read, so a report over a fixture is the same report
/// tomorrow. Two of these clocks count days, and a wall-clock feature whose own tests depend
/// on the day they run is the failure the argument in `lint::today_iso` describes.
pub(crate) fn read_clocks(root: &Path, cfg: &crate::config::DueConfig, today: i64) -> Vec<Clock> {
    vec![
        clock_index(root, cfg.index_after),
        clock_catalog(root, today),
        clock_questions(root, cfg.questions_after),
        clock_phases(root, cfg.phases_after, today),
    ]
}

/// `yidam due`. Read-only, offline, and exits zero however much is owed unless `--strict`.
pub fn due(strict: bool, format: crate::report::Format) -> Result<()> {
    let root = repo_root()?;
    require_yidam_repo(&root)?;
    let cfg = crate::config::load_yidam_config(&root)?;
    let clocks = read_clocks(&root, &cfg.due, crate::dates::today_days());
    let report = DueReport::new(clocks, strict);
    let passed = report.passed;

    if format.is_json() {
        crate::report::emit(&root, report)?;
    } else {
        println!("{}", render(&report, &root));
    }
    if !passed {
        std::process::exit(1);
    }
    Ok(())
}

/// The text report.
///
/// The closing sentence is not decoration. A reader who has just been shown four lines with
/// counts on them will reach for the reading they already have — that a report with numbers
/// in it is a list of problems — and this report's whole argument is that it is not one.
pub(crate) fn render(report: &DueReport, root: &Path) -> String {
    let mut out = format!("yidam due — {}\n\n", root.display());
    for c in &report.clocks {
        out.push_str(&format!(
            "  {:<5} {:<10} {}\n",
            c.state.tag(),
            c.id,
            c.detail
        ));
        // Shown where it is owed, and on an unset clock — where the thing to do is to set
        // it. Printing one under every line is how a report becomes something people skim.
        if let Some(remedy) = &c.remedy {
            if matches!(c.state, State::Due | State::Undeclared) {
                out.push_str(&format!("  {:<5} {:<10} → {remedy}\n", "", ""));
            }
        }
    }
    out.push('\n');

    let head = match report.due {
        0 => "Nothing is due.".to_string(),
        n => format!("{n} clock(s) due."),
    };
    let unset = match report.undeclared {
        0 => String::new(),
        n => format!(
            " {n} clock(s) have no interval declared and can never come due — see \
             docs/configuration.md."
        ),
    };
    // Stated on every run, including the quiet one. It is the sentence that keeps this
    // report from being read as a second `doctor`, and a reader who only ever sees the clean
    // run is exactly the reader who needs to be told what the clean run means.
    let line = match (report.due, report.strict) {
        (0, _) => "Nothing here is wrong either — `yidam doctor` is the report that answers \
                   that."
            .to_string(),
        (_, false) => "None of this is a defect. Being owed is not being broken; `yidam \
                       doctor` answers what is wrong."
            .to_string(),
        (_, true) => "None of this is a defect — `--strict` is exiting nonzero on what is \
                      owed, because you asked it to."
            .to_string(),
    };
    out.push_str(&format!("{head}{unset}\n{line}"));
    out
}

#[cfg(test)]
mod tests;
