//! A scheduled watch must not read an unreachable source as an unchanged one.
//!
//! `sadhana/github/workflows/watch.yml` runs a derived repository's own round of checking and
//! appends one row to `.github/watch/heartbeats.tsv`; the `watch` job in `ci.yml` reads that
//! ledger and fails on silence or blindness. A derived repository ran the first half without
//! the second for twenty-four rounds: its source was unreachable, the job saw no diff, and the
//! build stayed green.
//!
//! Unlike the other workflow tests, these run the shell. The round step and the gate are
//! extracted from the two workflows and executed against scripted rounds and fixture ledgers,
//! because every property worth pinning here is about what the shell does with an edge case —
//! a crash, an empty line, the word `Unchanged` — and none of it is visible in the text.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const LEDGER: &str = ".github/watch/heartbeats.tsv";

fn workflow(name: &str) -> serde_yaml::Value {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../sadhana/github/workflows")
        .join(name);
    let text =
        std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{} unreadable: {e}", p.display()));
    serde_yaml::from_str(&text).unwrap_or_else(|e| panic!("{name} parses: {e}"))
}

fn step(name: &str, job: &str, step_name: &str) -> serde_yaml::Value {
    workflow(name)["jobs"][job]["steps"]
        .as_sequence()
        .unwrap_or_else(|| panic!("{name} has a {job} job with steps"))
        .iter()
        .find(|s| s["name"].as_str() == Some(step_name))
        .unwrap_or_else(|| panic!("{name}'s {job} job has no step named {step_name:?}"))
        .clone()
}

fn run_of(name: &str, job: &str, step_name: &str) -> String {
    step(name, job, step_name)["run"]
        .as_str()
        .unwrap_or_else(|| panic!("{step_name:?} runs a script"))
        .to_string()
}

fn bash(script: &str, cwd: &Path, env: &[(&str, &str)]) -> Output {
    // Outside `cwd`: the guard step reads the working tree, and would see the script.
    let holder = tempfile::tempdir().unwrap();
    let file = holder.path().join("step.sh");
    std::fs::write(&file, script).unwrap();
    let mut cmd = Command::new("bash");
    cmd.arg(&file).current_dir(cwd).env_remove("WATCH_TODAY");
    for (k, v) in env {
        cmd.env(k, v);
    }
    cmd.output().expect("bash runs")
}

/// What the round step recorded for a round with this body: (row, `verdict=` output).
fn round(body: &str) -> (String, String) {
    let dir = tempfile::tempdir().unwrap();
    let temp = dir.path().join("runner-temp");
    std::fs::create_dir(&temp).unwrap();
    let script = dir.path().join("round");
    std::fs::write(&script, format!("#!/usr/bin/env bash\n{body}\n")).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    let output = dir.path().join("github-output");
    let out = bash(
        &run_of("watch.yml", "round", "Run one round"),
        dir.path(),
        &[
            ("ROUND", script.to_str().unwrap()),
            ("RUNNER_TEMP", temp.to_str().unwrap()),
            ("GITHUB_OUTPUT", output.to_str().unwrap()),
        ],
    );
    assert!(
        out.status.success(),
        "the round step itself must not fail on any round:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let row = std::fs::read_to_string(temp.join("row.tsv")).expect("a row is always written");
    let verdict = std::fs::read_to_string(output).unwrap();
    (row, verdict.trim().to_string())
}

fn fields(row: &str) -> Vec<String> {
    let row = row.strip_suffix('\n').expect("one newline-terminated row");
    assert!(!row.contains('\n'), "exactly one row: {row:?}");
    row.split('\t').map(str::to_string).collect()
}

/// The three words, each with a detail, are recorded as they were said.
#[test]
fn a_round_that_answers_is_recorded_as_it_answered() {
    for (body, verdict, detail) in [
        ("echo unchanged", "unchanged", ""),
        (
            "printf 'changed\\t3 new filings\\n'",
            "changed",
            "3 new filings",
        ),
        (
            "echo 'fetching…'; printf 'unreachable\\thost refused\\n'",
            "unreachable",
            "host refused",
        ),
    ] {
        let (row, out) = round(body);
        let f = fields(&row);
        assert_eq!(f.len(), 3, "date, outcome, detail: {row:?}");
        assert!(
            f[0].len() == 10 && f[0].as_bytes()[4] == b'-' && f[0].as_bytes()[7] == b'-',
            "the first column is an ISO day: {row:?}"
        );
        assert_eq!((f[1].as_str(), f[2].as_str()), (verdict, detail), "{body}");
        assert_eq!(out, format!("verdict={verdict}"));
    }
}

/// **The defect this workflow exists for.** Every way a round can fail to look is filed
/// `unreachable`, never `unchanged` — including a round that says `unchanged` and then exits
/// nonzero, because a round that crashed did not finish looking.
#[test]
fn a_round_that_did_not_look_is_unreachable_and_never_unchanged() {
    for (body, why) in [
        ("exit 3", "a crash with no output"),
        (
            "echo unchanged; exit 1",
            "a verdict followed by a failing exit",
        ),
        ("true", "no output at all"),
        ("echo ''", "an empty last line"),
        (
            "echo Unchanged",
            "a word that is not exactly one of the three",
        ),
        ("echo unchanged-ish", "a word that merely starts with one"),
        ("echo 'no change'", "prose"),
        (
            "echo unchanged; echo done",
            "a verdict that is not the last line",
        ),
    ] {
        let (row, out) = round(body);
        let f = fields(&row);
        assert_eq!(f[1], "unreachable", "{why} must be unreachable: {row:?}");
        assert!(!f[2].is_empty(), "{why} must say why: {row:?}");
        assert_eq!(out, "verdict=unreachable", "{why}");
    }
}

/// A detail cannot break the ledger's columns.
#[test]
fn a_detail_with_tabs_stays_in_its_column() {
    let (row, _) = round("printf 'changed\\ta\\tb\\tc\\r\\n'");
    let f = fields(&row);
    assert_eq!(
        (f[1].as_str(), f[2].as_str()),
        ("changed", "a b c"),
        "{row:?}"
    );
}

/// `unchanged` contains `changed`. The derived repository this came from tested its row with
/// `contains`, and its first quiet round opened an issue announcing a change.
#[test]
fn only_a_change_opens_an_issue_and_the_test_is_exact() {
    let issue = step("watch.yml", "round", "Open an issue on a change");
    assert_eq!(
        issue["if"].as_str(),
        Some("steps.round.outputs.verdict == 'changed'"),
        "the issue condition must compare the verdict exactly"
    );
}

/// The steps that write run in this order: round, append, guard, commit. A guard after the
/// commit guards nothing.
#[test]
fn the_guard_stands_between_the_append_and_the_commit() {
    let names: Vec<String> = workflow("watch.yml")["jobs"]["round"]["steps"]
        .as_sequence()
        .unwrap()
        .iter()
        .filter_map(|s| s["name"].as_str().map(str::to_string))
        .collect();
    let at = |n: &str| {
        names
            .iter()
            .position(|x| x == n)
            .unwrap_or_else(|| panic!("no step {n:?} in {names:?}"))
    };
    assert!(at("Run one round") < at("Append the row"));
    assert!(at("Append the row") < at("Guard — nothing but the ledger changed"));
    assert!(at("Guard — nothing but the ledger changed") < at("Commit the row"));
}

/// The guard passes a tree whose only change is the ledger, and refuses a modified file or an
/// untracked one beside it.
#[test]
fn the_watch_may_write_only_its_ledger() {
    let guard = run_of(
        "watch.yml",
        "round",
        "Guard — nothing but the ledger changed",
    );
    let dir = tempfile::tempdir().unwrap();
    let setup = format!(
        "set -e\n\
         git init -q .\n\
         mkdir -p .github/watch\n\
         printf '# armed: 2026-01-01\\n' > {LEDGER}\n\
         echo corpus > node.yml\n\
         git add -A\n\
         git -c user.name=t -c user.email=t@t -c commit.gpgsign=false commit -qm genesis\n"
    );
    assert!(bash(&setup, dir.path(), &[]).status.success());
    let guarded = |extra: &str| {
        bash(
            &format!("{extra}\n{guard}"),
            dir.path(),
            &[("LEDGER", LEDGER)],
        )
        .status
        .success()
    };
    assert!(
        guarded(&format!(
            "printf '2026-01-02\\tunchanged\\t\\n' >> {LEDGER}"
        )),
        "a tree changing only the ledger passes"
    );
    assert!(
        !guarded("echo claim >> node.yml"),
        "a modified corpus file is refused"
    );
    assert!(
        !guarded("git checkout -q node.yml; touch fetched.json"),
        "an untracked file is refused"
    );
}

fn gate(ledger: &str, today: &str) -> (bool, String) {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".github/watch")).unwrap();
    std::fs::write(dir.path().join(LEDGER), ledger).unwrap();
    let job = &workflow("ci.yml")["jobs"]["watch"];
    let max_blind = job["env"]["MAX_BLIND_ROUNDS"].as_u64().unwrap().to_string();
    let max_silence = job["env"]["MAX_SILENCE_DAYS"].as_u64().unwrap().to_string();
    let out = bash(
        &run_of("ci.yml", "watch", "The ledger says the watch runs and sees"),
        dir.path(),
        &[
            ("WATCH_TODAY", today),
            ("MAX_BLIND_ROUNDS", &max_blind),
            ("MAX_SILENCE_DAYS", &max_silence),
        ],
    );
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    (out.status.success(), text)
}

const HEAD: &str = "# armed: 2026-03-01\ndate\toutcome\tdetail\n";

fn ledger(rows: &[(&str, &str)]) -> String {
    let body: String = rows.iter().map(|(d, o)| format!("{d}\t{o}\t\n")).collect();
    format!("{HEAD}{body}")
}

/// `n` consecutive days of `outcome` starting on 2026-03-`from`.
fn days(from: u32, n: u32, outcome: &'static str) -> Vec<(String, &'static str)> {
    (from..from + n)
        .map(|d| (format!("2026-03-{d:02}"), outcome))
        .collect()
}

fn ledger_of(rows: &[(String, &str)]) -> String {
    ledger(
        &rows
            .iter()
            .map(|(d, o)| (d.as_str(), *o))
            .collect::<Vec<_>>(),
    )
}

/// The thresholds are the ones the header argues for. A change to either should be a
/// decision that moves this test, not a quiet edit.
#[test]
fn the_windows_are_a_week() {
    let job = &workflow("ci.yml")["jobs"]["watch"];
    assert_eq!(job["env"]["MAX_BLIND_ROUNDS"].as_u64(), Some(7));
    assert_eq!(job["env"]["MAX_SILENCE_DAYS"].as_u64(), Some(7));
}

/// **The gate this issue asked for.** Seven unreachable rounds in a row are weather; the
/// eighth is red — and the ledger that produced it was fresh every single day.
#[test]
fn a_run_of_unreachable_rounds_fails_although_every_row_is_fresh() {
    let mut rows = days(1, 2, "unchanged");
    rows.extend(days(3, 7, "unreachable"));
    let (ok, text) = gate(&ledger_of(&rows), "2026-03-09");
    assert!(ok, "seven blind rounds are inside the window:\n{text}");

    rows.extend(days(10, 1, "unreachable"));
    let (ok, text) = gate(&ledger_of(&rows), "2026-03-10");
    assert!(!ok, "the eighth blind round must fail:\n{text}");
    assert!(
        text.contains("BLIND: 8 unreachable rounds in a row, since 2026-03-03"),
        "{text}"
    );
    assert!(
        !text.contains("SILENT"),
        "the ledger is fresh; only sight fails:\n{text}"
    );
}

/// One round that sees ends the run.
#[test]
fn a_round_that_sees_resets_the_run() {
    let mut rows = days(1, 7, "unreachable");
    rows.extend(days(8, 1, "changed"));
    rows.extend(days(9, 7, "unreachable"));
    let (ok, text) = gate(&ledger_of(&rows), "2026-03-15");
    assert!(ok, "{text}");
    assert!(text.contains("7 unreachable in a row"), "{text}");
}

/// A watch that has never reached its source is named apart from one that stopped: the second
/// was once shown to work.
#[test]
fn a_watch_that_never_saw_is_named_apart_from_one_that_stopped() {
    let (ok, text) = gate(&ledger_of(&days(1, 8, "unreachable")), "2026-03-08");
    assert!(!ok, "{text}");
    assert!(
        text.contains("NEVER SEEN: 8 rounds since 2026-03-01"),
        "{text}"
    );
    assert!(!text.contains("BLIND"), "{text}");
}

/// Silence is measured from the newest row or the arming date, whichever is later — so an
/// armed watch that is never scheduled goes red on the same day as one that stops.
#[test]
fn silence_is_measured_from_the_newest_row_or_the_arming_date() {
    let armed_only = HEAD;
    assert!(
        gate(armed_only, "2026-03-08").0,
        "seven days after arming, no round yet"
    );
    let (ok, text) = gate(armed_only, "2026-03-09");
    assert!(
        !ok,
        "eight days after arming with no round is silence:\n{text}"
    );
    assert!(
        text.contains("SILENT: no round for 8 days, since 2026-03-01"),
        "{text}"
    );

    let stopped = ledger_of(&days(1, 3, "unchanged"));
    assert!(gate(&stopped, "2026-03-10").0);
    let (ok, text) = gate(&stopped, "2026-03-11");
    assert!(!ok, "{text}");
    assert!(text.contains("since 2026-03-03"), "{text}");

    let rearmed = format!(
        "{}# armed: 2026-04-01\n",
        ledger_of(&days(1, 3, "unchanged"))
    );
    assert!(
        gate(&rearmed, "2026-04-05").0,
        "a later arming date restarts the clock"
    );
}

/// The day arithmetic crosses a month and a leap day. The awk has no `mktime`, so it is a
/// hand-written day count, and those break at February.
#[test]
fn silence_counts_calendar_days_across_february() {
    let at = |armed: &str, today: &str| gate(&format!("# armed: {armed}\n"), today).0;
    assert!(at("2024-02-27", "2024-03-05"), "7 days across 29 Feb 2024");
    assert!(!at("2024-02-26", "2024-03-05"), "8 days across 29 Feb 2024");
    assert!(at("2026-02-26", "2026-03-05"), "7 days across 28 Feb 2026");
    assert!(!at("2026-02-25", "2026-03-05"), "8 days across 28 Feb 2026");
    assert!(at("2025-12-29", "2026-01-05"), "7 days across a year");
    assert!(!at("2025-12-28", "2026-01-05"), "8 days across a year");
}

/// A row the gate cannot read fails rather than being skipped. Skipping would let a
/// misspelled `Unreachable` silently shorten a blind run.
#[test]
fn a_row_the_gate_cannot_read_fails_it() {
    for bad in [
        "2026-03-02\tUnreachable\t",
        "2026-03-02\tunchanged-ish\t",
        "03/02/2026\tunchanged\t",
        "2026-03-02",
    ] {
        let (ok, text) = gate(&format!("{HEAD}{bad}\n"), "2026-03-02");
        assert!(!ok, "{bad:?} must fail the gate:\n{text}");
        assert!(text.contains("line 3"), "{text}");
    }
    let (ok, text) = gate("# armed: soon\n", "2026-03-02");
    assert!(!ok && text.contains("armed date"), "{text}");
}

/// A ledger with neither rounds nor an arming date says nothing, and passing it would be the
/// gate asserting nothing.
#[test]
fn an_empty_ledger_fails() {
    let (ok, text) = gate("date\toutcome\tdetail\n", "2026-03-02");
    assert!(!ok, "{text}");
    assert!(text.contains("asserts nothing"), "{text}");
}

/// The producer and the consumer agree: rows the round step writes are rows the gate reads,
/// and eight blind ones from it turn the gate red.
#[test]
fn the_gate_reads_what_the_round_writes() {
    let (seen, _) = round("echo unchanged");
    let (blind, _) = round("exit 1");
    let today = fields(&seen)[0].clone();
    let (ok, text) = gate(&format!("# armed: {today}\n{seen}{blind}"), &today);
    assert!(ok, "{text}");
    let (ok, text) = gate(&format!("# armed: {today}\n{}", blind.repeat(8)), &today);
    assert!(!ok && text.contains("NEVER SEEN: 8 rounds"), "{text}");
}

/// The gate runs only where a ledger exists, and the two workflows name the same one.
#[test]
fn the_gate_is_armed_by_the_ledger_both_workflows_name() {
    let ci = workflow("ci.yml");
    let detect = serde_yaml::to_string(&ci["jobs"]["detect"]).unwrap();
    assert!(
        detect.contains(&format!("test -f {LEDGER}")) && detect.contains("watch=true"),
        "detect must arm the gate on the ledger existing:\n{detect}"
    );
    assert_eq!(
        ci["jobs"]["watch"]["if"].as_str(),
        Some("needs.detect.outputs.watch == 'true'")
    );
    assert!(run_of("ci.yml", "watch", "The ledger says the watch runs and sees").contains(LEDGER));
    assert_eq!(
        workflow("watch.yml")["env"]["LEDGER"].as_str(),
        Some(LEDGER)
    );
}

/// Installed at genesis, the watch must do nothing until a person arms it: no schedule, so
/// no derived repository runs a round it never wrote.
#[test]
fn the_installed_watch_is_not_scheduled() {
    let on = workflow("watch.yml")["on"]
        .as_mapping()
        .expect("watch.yml declares its triggers")
        .clone();
    let keys: Vec<&str> = on.keys().filter_map(|k| k.as_str()).collect();
    assert_eq!(
        keys,
        ["workflow_dispatch"],
        "watch.yml must ship unscheduled"
    );
}
