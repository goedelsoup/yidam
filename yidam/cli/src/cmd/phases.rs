use anyhow::Result;
use std::path::Path;

use crate::cmd::phase::record::{self, Record};

/// One row of the `yidam phases` table: an inquiry ref backed by a `ma/*`, `rigpa/*` or
/// `phase/*` branch.
///
/// `phase/*` was absent from both this table and the status count until the namespace was
/// typed — see [`crate::git::RefKind`]. A table that lists settled evolutions beside standing
/// positions and calls the lot "active phases" is why `state` is a column rather than a
/// filter: every ref is shown, and each says what it is.
#[derive(serde::Serialize)]
pub(crate) struct PhaseRow {
    pub name: String,
    /// One of [`crate::git::REF_STATES`], decided by [`state_of`] from two ranked sources.
    pub state: String,
    /// The type declared at `yidam phase start`, where the ref carries a phase record.
    /// `None` for every phase opened by hand and for every phase that predates the record —
    /// which is all of them in every repository today, and forever in the eighteen A0
    /// measured. A column that filled this in from somewhere would be inventing it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r#type: Option<String>,
    /// Which evidence decided `state`: `record` or `ref`. Reported rather than left implicit,
    /// because the two answer different questions and a reader comparing rows needs to know
    /// which they are reading — an inferred `active` is *this ref has not landed*, and a
    /// recorded one is *this phase's plan completed*.
    pub source: &'static str,
    pub ref_name: String,
    pub owner: String,
    pub started: String,
    pub commits: usize,
}

impl PhaseRow {
    /// Whether this row is bounded work that has not landed.
    ///
    /// **The predicate, and the reason it is one rather than a comparison at each call site.**
    /// `cmd/cycle.rs` and `cmd/due.rs` both filtered on `state == "active"`, which was the
    /// whole of "in flight" while `active` was the only unsettled state a row could carry.
    /// Adding `interrupted` would have made both of them silently drop the phases most in
    /// need of reporting — a run that stopped partway is more in flight than one nobody
    /// touched today, and `due`'s phase clock exists to ask how long that has been true.
    pub fn is_in_flight(&self) -> bool {
        matches!(self.state.as_str(), "active" | "interrupted")
    }
}

/// What a ref is, from the two sources RFC-0028 §3 ranks rather than collapses.
///
/// > `RefKind` answers *what is this ref*; the run record answers *what happened in this run*.
/// > Collapsing them recreates #272's actual defect — two surfaces free to disagree — one
/// > level up.
///
/// So: **one classifier, two evidence sources, ranked.** `ref_state` is asked first and its
/// answer stands wherever it is about the baseline — `position`, `settled`, `rewritten`. Those
/// are facts a record cannot hold: a record is committed on the phase's own branch, strictly
/// before the merge that settles it, so it could only claim settlement by predicting one.
///
/// The record is authoritative for exactly the case the ref cannot see. `active` from ref
/// shape means *this ref has commits the baseline lacks*, which is true of a phase whose run
/// died halfway through and of one opened this morning. Where a record exists it separates
/// them, and `source` says which reading the row is.
///
/// Where none exists the answer stays ref-derived, and that arm is not a fallback to be
/// removed later. RFC-0028 §3: *"refs without run records exist forever"* — every phase in
/// every existing repository, and every phase a person opens with `git switch -c`, which is
/// `PHASES.md`'s own documented flow.
fn state_of(
    root: &Path,
    phase: &crate::git::PhaseRef,
    base: Option<&str>,
    record: Option<&Record>,
) -> (String, &'static str) {
    let inferred = crate::git::ref_state(root, phase, base);
    match record {
        Some(rec) if inferred == "active" => (rec.state().to_string(), "record"),
        _ => (inferred.to_string(), "ref"),
    }
}

fn git_stdout(root: &Path, args: &[&str]) -> Option<String> {
    crate::git::Git::new(root).args(args).try_run()
}

/// "substrate-survey" → "Substrate survey"
fn humanize(slug: &str) -> String {
    let spaced = slug.replace(['-', '_'], " ");
    let mut chars = spaced.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => spaced,
    }
}

pub(crate) fn collect_phases(root: &Path) -> Result<Vec<PhaseRow>> {
    // Local *and* remote-tracking, via `crate::git::phase_refs`. This listed
    // `refs/heads/{ma,rigpa}` only, which is invisible in a fresh clone — see that
    // function for what it cost.
    let phases = crate::git::phase_refs(root);

    let base = crate::git::base_branch(root);
    let mut rows = Vec::new();

    for phase in &phases {
        // The ref to read is not the phase's name when the phase lives only on a remote.
        let ref_name = phase.git_ref.as_str();
        let slug = record::slug_of(&phase.name);

        let owner = git_stdout(root, &["log", "-1", "--format=%an", ref_name])
            .unwrap_or_else(|| "unknown".to_string());

        // Commits and start date are relative to the base branch: only work
        // unique to the phase counts. A phase with no unique commits yet
        // falls back to the tip commit's date.
        let (commits, started) = match &base {
            Some(b) if *b != ref_name => {
                let not_base = format!("^{b}");
                let count = git_stdout(root, &["rev-list", "--count", ref_name, &not_base])
                    .and_then(|s| s.parse::<usize>().ok())
                    .unwrap_or(0);
                let first_unique = git_stdout(
                    root,
                    &["log", "--reverse", "--format=%as", ref_name, &not_base],
                )
                .and_then(|s| s.lines().next().map(str::to_string))
                .filter(|s| !s.is_empty());
                let started = match first_unique {
                    Some(d) => d,
                    None => git_stdout(root, &["log", "-1", "--format=%as", ref_name])
                        .unwrap_or_else(|| "—".to_string()),
                };
                (count, started)
            }
            _ => {
                let count = git_stdout(root, &["rev-list", "--count", ref_name])
                    .and_then(|s| s.parse::<usize>().ok())
                    .unwrap_or(0);
                let started = git_stdout(root, &["log", "-1", "--format=%as", ref_name])
                    .unwrap_or_else(|| "—".to_string());
                (count, started)
            }
        };

        // Read once and used twice: the record decides `state` and carries `type`, and two
        // `git show` invocations for one file is the kind of thing a table of twenty-six refs
        // notices.
        let record = record::read_at(root, ref_name, slug);
        let (state, source) = state_of(root, phase, base.as_deref(), record.as_ref());
        rows.push(PhaseRow {
            name: humanize(slug),
            state,
            r#type: record.map(|rec| rec.r#type),
            source,
            ref_name: ref_name.to_string(),
            owner,
            started,
            commits,
        });
    }

    Ok(rows)
}

pub(crate) fn render_phases(rows: &[PhaseRow]) -> String {
    if rows.is_empty() {
        return "No inquiry refs (no ma/*, rigpa/* or phase/* branches).".to_string();
    }

    const N: usize = 7;
    let headers = [
        "Phase", "Type", "State", "Ref", "Owner", "Started", "Commits",
    ];
    let cells: Vec<[String; N]> = rows
        .iter()
        .map(|r| {
            [
                r.name.clone(),
                // Em dash rather than an empty cell: a blank reads as a value that failed to
                // render, and this is the ordinary state of every phase that predates the
                // record.
                r.r#type.clone().unwrap_or_else(|| "\u{2014}".to_string()),
                r.state.clone(),
                r.ref_name.clone(),
                r.owner.clone(),
                r.started.clone(),
                r.commits.to_string(),
            ]
        })
        .collect();

    let widths: Vec<usize> = (0..N)
        .map(|i| {
            cells
                .iter()
                .map(|c| c[i].chars().count())
                .chain(std::iter::once(headers[i].chars().count()))
                .max()
                .unwrap_or(0)
        })
        .collect();

    let mut lines = Vec::with_capacity(rows.len() + 2);
    let fmt_row = |cols: [&str; N]| -> String {
        cols.iter()
            .enumerate()
            .map(|(i, c)| format!("{c:<width$}", width = widths[i]))
            .collect::<Vec<_>>()
            .join("   ")
            .trim_end()
            .to_string()
    };

    lines.push(fmt_row(headers));
    lines.push(
        widths
            .iter()
            .map(|w| "\u{2500}".repeat(*w))
            .collect::<Vec<_>>()
            .join("   "),
    );
    for c in &cells {
        lines.push(fmt_row([&c[0], &c[1], &c[2], &c[3], &c[4], &c[5], &c[6]]));
    }

    // Said once, at the foot, rather than in a column. Every phase in every repository today
    // is ref-inferred (RFC-0028 §3), so a per-row marker would mark every row and tell a
    // reader nothing; what is worth saying is how many rows are an inference and what writes
    // the evidence instead.
    let inferred = rows
        .iter()
        .filter(|r| r.source == "ref" && r.is_in_flight())
        .count();
    if inferred > 0 {
        lines.push(String::new());
        lines.push(format!(
            "{inferred} in-flight row(s) carry no phase record, so their state is inferred \
             from the ref.\n`yidam phase start` snapshots what a phase begins from; \
             `yidam phase run` records its steps."
        ));
    }
    lines.join("\n")
}

/// Print the table of inquiry refs (`ma/*`, `rigpa/*` and `phase/*` branches), each with the
/// state that distinguishes work in flight from work already settled.
#[derive(serde::Serialize)]
struct PhasesReport<'a> {
    phases: &'a [PhaseRow],
}

pub fn phases(root: Option<&std::path::Path>, format: crate::report::Format) -> Result<()> {
    let root = crate::paths::resolve_root(root)?;
    let rows = collect_phases(&root)?;
    crate::report::finish(&root, format, PhasesReport { phases: &rows }, |p| {
        println!("{}", render_phases(p.phases))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::TempDir;

    use crate::git::fixture::git;

    fn init_repo() -> (TempDir, PathBuf) {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().to_path_buf();
        crate::git::fixture::write(&root, "a.txt", "a");
        crate::git::fixture::repo(&root, "chore: genesis — test");
        (tmp, root)
    }

    /// A phase that exists only as a remote-tracking ref is found and read.
    ///
    /// This is the fresh-clone shape — the one CI has — and it used to score zero.
    /// Built by fetching from a second repository rather than by writing the ref by
    /// hand, so the ref is real and `git log` against it resolves.
    #[test]
    fn collect_phases_finds_a_remote_only_phase() {
        let (_origin_tmp, origin) = init_repo();
        git(&origin, &["checkout", "-q", "-b", "ma/auditor"]);
        std::fs::write(origin.join("b.txt"), "b").unwrap();
        git(&origin, &["add", "."]);
        git(
            &origin,
            &["commit", "-q", "-m", "establish: the auditor's first node"],
        );
        git(&origin, &["checkout", "-q", "main"]);

        let (_tmp, root) = init_repo();
        git(
            &root,
            &["remote", "add", "origin", origin.to_str().unwrap()],
        );
        git(&root, &["fetch", "-q", "origin"]);

        // No local ma/* branch exists here at all.
        let rows = collect_phases(&root).unwrap();
        assert_eq!(
            rows.len(),
            1,
            "{:?}",
            rows.iter().map(|r| &r.ref_name).collect::<Vec<_>>()
        );
        assert_eq!(rows[0].name, "Auditor");
        assert_eq!(rows[0].ref_name, "origin/ma/auditor");
        assert_eq!(rows[0].owner, crate::git::fixture::FIXTURE_AUTHOR);
        // A remote-only *position*, so it lands in `positions` — the assertion here was
        // `active == 1`, which is the conflation this split exists to undo.
        assert_eq!(rows[0].state, "position");
        assert_eq!(crate::git::phase_tally(&root).positions, 1);
    }

    /// The same phase locally and on the remote is one row, read from the local ref.
    ///
    /// The branch carries a commit of its own so that it is genuinely ahead of the baseline.
    /// Branched at `main` and left there it has nothing in flight, and `phase_tally` reads
    /// that — correctly — as settled, which would make this test about the wrong thing.
    #[test]
    fn a_phase_on_both_sides_is_one_row() {
        let (_origin_tmp, origin) = init_repo();
        git(&origin, &["checkout", "-q", "-b", "rigpa/schema-reach"]);
        std::fs::write(origin.join("c.txt"), "c").unwrap();
        git(&origin, &["add", "."]);
        git(
            &origin,
            &["commit", "-q", "-m", "revise: the schema reaches"],
        );
        git(&origin, &["checkout", "-q", "main"]);

        let (_tmp, root) = init_repo();
        git(
            &root,
            &["remote", "add", "origin", origin.to_str().unwrap()],
        );
        git(&root, &["fetch", "-q", "origin"]);
        git(
            &root,
            &["branch", "rigpa/schema-reach", "origin/rigpa/schema-reach"],
        );

        let rows = collect_phases(&root).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].ref_name, "rigpa/schema-reach");
        assert_eq!(crate::git::phase_tally(&root).active, 1);
    }

    #[test]
    fn collect_phases_reports_unique_commits() {
        let (_tmp, root) = init_repo();
        git(&root, &["checkout", "-q", "-b", "ma/substrate-survey"]);
        std::fs::write(root.join("b.txt"), "b").unwrap();
        git(&root, &["add", "."]);
        git(&root, &["commit", "-q", "-m", "establish: substrate map"]);
        git(&root, &["checkout", "-q", "main"]);

        let rows = collect_phases(&root).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].ref_name, "ma/substrate-survey");
        assert_eq!(rows[0].name, "Substrate survey");
        assert_eq!(rows[0].owner, crate::git::fixture::FIXTURE_AUTHOR);
        assert_eq!(rows[0].commits, 1);
        assert!(!rows[0].started.is_empty());
    }

    #[test]
    fn collect_phases_includes_rigpa_refs() {
        let (_tmp, root) = init_repo();
        git(&root, &["branch", "rigpa/glacial-review"]);

        let rows = collect_phases(&root).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].ref_name, "rigpa/glacial-review");
        assert_eq!(rows[0].commits, 0);
    }

    #[test]
    fn render_phases_aligns_columns() {
        let rows = vec![
            PhaseRow {
                name: "Substrate survey".into(),
                state: "position".into(),
                r#type: None,
                source: "ref",
                ref_name: "ma/substrate".into(),
                owner: "goedelsoup".into(),
                started: "2026-06-20".into(),
                commits: 12,
            },
            PhaseRow {
                name: "Glacial review".into(),
                state: "settled".into(),
                r#type: Some("Synthesis".into()),
                source: "ref",
                ref_name: "rigpa/glacial".into(),
                owner: "goedelsoup".into(),
                started: "2026-06-28".into(),
                commits: 4,
            },
        ];
        let out = render_phases(&rows);
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines.len(), 4);
        assert!(lines[0].starts_with("Phase"));
        assert!(lines[1].starts_with('\u{2500}'));
        assert!(lines[2].contains("ma/substrate"));
        assert!(lines[3].contains("rigpa/glacial"));
        // The state is the column that distinguishes the two, and it is rendered.
        assert!(lines[2].contains("position"), "{}", lines[2]);
        assert!(lines[3].contains("settled"), "{}", lines[3]);
    }

    #[test]
    fn render_phases_empty_state() {
        assert!(render_phases(&[]).contains("No inquiry refs"));
    }

    /// A phase merged into the baseline is settled, not active. This is the defect: a
    /// derived repository reported 26 active phases while holding exactly one, because
    /// nothing asked whether the work had already landed.
    #[test]
    fn a_merged_phase_is_settled_not_active() {
        let (_tmp, root) = init_repo();
        git(&root, &["checkout", "-q", "-b", "phase/outcome-axis"]);
        std::fs::write(root.join("b.txt"), "b").unwrap();
        git(&root, &["add", "."]);
        git(
            &root,
            &["commit", "-q", "-m", "establish: the outcome axis"],
        );
        git(&root, &["checkout", "-q", "main"]);
        git(
            &root,
            &[
                "merge",
                "--no-ff",
                "-q",
                "-m",
                "phase: outcome axis — one node",
                "phase/outcome-axis",
            ],
        );

        let tally = crate::git::phase_tally(&root);
        assert_eq!(tally.settled, 1, "{tally:?}");
        assert_eq!(tally.active, 0, "{tally:?}");
    }

    /// The one genuinely in-flight phase is the one that has not landed.
    #[test]
    fn an_unmerged_phase_is_active() {
        let (_tmp, root) = init_repo();
        git(&root, &["checkout", "-q", "-b", "phase/batchelder-lineage"]);
        std::fs::write(root.join("b.txt"), "b").unwrap();
        git(&root, &["add", "."]);
        git(
            &root,
            &["commit", "-q", "-m", "open: where the line starts"],
        );
        git(&root, &["checkout", "-q", "main"]);

        let tally = crate::git::phase_tally(&root);
        assert_eq!(tally.active, 1, "{tally:?}");
        assert_eq!(tally.settled, 0, "{tally:?}");
    }

    /// An elector position is neither. It sits ahead of the baseline permanently and is
    /// not work awaiting settlement — counting it as an active phase is the category error
    /// that made three standing positions read as three phases in flight.
    #[test]
    fn a_position_is_never_active_nor_settled() {
        let (_tmp, root) = init_repo();
        git(&root, &["checkout", "-q", "-b", "ma/auditor"]);
        std::fs::write(root.join("b.txt"), "b").unwrap();
        git(&root, &["add", "."]);
        git(
            &root,
            &["commit", "-q", "-m", "establish: the auditor reads"],
        );
        git(&root, &["checkout", "-q", "main"]);

        let tally = crate::git::phase_tally(&root);
        assert_eq!(tally.positions, 1, "{tally:?}");
        assert_eq!(tally.active, 0, "{tally:?}");
        assert_eq!(tally.settled, 0, "{tally:?}");

        // And still a position once its work is on the baseline — merging an elector's
        // position does not convert it into settled bounded work.
        git(
            &root,
            &[
                "merge",
                "--no-ff",
                "-q",
                "-m",
                "transport: the auditor",
                "ma/auditor",
            ],
        );
        let tally = crate::git::phase_tally(&root);
        assert_eq!(tally.positions, 1, "{tally:?}");
        assert_eq!(tally.settled, 0, "{tally:?}");
    }
}
