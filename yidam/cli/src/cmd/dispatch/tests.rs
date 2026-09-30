//! #477's definition of done, one test per line of it where a line is testable here.

use std::fmt::Write as _;
use std::path::Path;

use super::*;
use crate::cmd::lint::{elector_receipt, independence};
use crate::cmd::sangha::Resolution;
use crate::git::fixture::{self, commit, git, git_out};

const MODEL: &str = "claude-opus-4-8";

/// An elector that writes a position naming what it was handed, and nothing else.
const ELECTOR: &str = "#!/bin/sh\nset -eu\nmkdir -p \"$YIDAM_OUT/$(dirname \"$YIDAM_POSITION\")\"\n\
                       printf '# %s on %s\\n\\nRan %s %s.\\n' \"$YIDAM_SEAT\" \"$YIDAM_QUESTION\" \
                       \"$YIDAM_MODEL\" \"$YIDAM_MODEL_VERSION\" > \"$YIDAM_OUT/$YIDAM_POSITION\"\n";

const DECLARATION: &str = "model = \"claude-opus-4-8\"\nversion = \"1\"\nrun = [\"sh\", \
                           \"elector.sh\"]\nreads = [\"elector.sh\", \".yidam/corpus/**\"]\nconfig \
                           = [\"elector.sh\"]\n";

/// The digest [`DECLARATION`] over `script` hashes to, computed the way the dispatcher does.
fn digest_of(script: &str) -> String {
    let decl: Declaration = toml::from_str(DECLARATION).unwrap();
    config_digest(
        &decl,
        &[File {
            path: "elector.sh".into(),
            sha256: receipt::sha256(script.as_bytes()),
        }],
    )
    .unwrap()
}

fn registry(rows: &[(&str, &str, &str, &str)]) -> String {
    let mut s = String::from(
        "# Electors\n\n| Name | Branch | Role | Kind | Model | Version | Config |\n\
         |------|--------|------|------|-------|---------|--------|\n",
    );
    for (name, kind, model, config) in rows {
        let _ = writeln!(
            s,
            "| `{name}` | `ma/{name}` | A seat. | {kind} | `{model}` | `1` | `{config}` |"
        );
    }
    s
}

/// A repository whose seats are branched from `main` after the registry and the declarations
/// are committed there, so each seat's tip holds both.
fn repo(rows: &[(&str, &str, &str, &str)]) -> tempfile::TempDir {
    let tmp = tempfile::TempDir::new().unwrap();
    let root = tmp.path();
    fixture::init(root);
    git(root, &["config", "commit.gpgsign", "false"]);
    fixture::write(root, ".yidam/corpus/topic.ont.yml", "class: topic\n");
    fixture::write(root, "elector.sh", ELECTOR);
    fixture::write(root, REGISTRY, &registry(rows));
    for (name, kind, _, _) in rows {
        if *kind == "agent" {
            fixture::write(root, &declaration_path(name), DECLARATION);
        }
    }
    commit(root, "genesis: a sangha");
    for (name, _, _, _) in rows {
        git(root, &["branch", &format!("ma/{name}")]);
    }
    tmp
}

fn opts(seats: &[&str]) -> Options {
    Options {
        question: "budget".into(),
        seats: seats.iter().map(|s| (*s).to_string()).collect(),
        dry_run: false,
        force: false,
        format: crate::report::Format::Text,
    }
}

fn rev(root: &Path, r: &str) -> String {
    git_out(root, &["rev-parse", r])
}

fn seat<'a>(r: &'a DispatchReport, name: &str) -> &'a SeatReport {
    r.seats.iter().find(|s| s.seat == name).unwrap()
}

/// Fast-forward a seat onto its proposal, as the person answering for it would.
fn accept(root: &Path, s: &SeatReport) {
    let proposal = s.proposal.as_deref().unwrap();
    git(
        root,
        &["update-ref", &format!("refs/heads/{}", s.branch), proposal],
    );
}

fn resolution_over(root: &Path, seats: &[&str]) -> Resolution {
    Resolution {
        file: ".yidam/sangha/resolutions/budget.md".into(),
        evolution: "e".into(),
        date: "2026-01-01".into(),
        tips: seats
            .iter()
            .map(|s| {
                format!(
                    "ma/{s}@{}",
                    git_out(root, &["rev-parse", "--short", &format!("ma/{s}")])
                )
            })
            .collect(),
        synthesized_by: vec![],
        independence: String::new(),
        rounds: String::new(),
        positions: vec![],
        branch_present: true,
    }
}

fn derived(root: &Path, seats: &[&str]) -> &'static str {
    let audits = independence::audit(root, &[resolution_over(root, seats)]);
    independence::derive(&audits[0].seats).value.as_str()
}

// ── the position goes through the proposal route ──────────────────────────────

#[test]
fn a_position_is_proposed_on_the_seats_tip_and_the_seat_does_not_move() {
    let d = digest_of(ELECTOR);
    let tmp = repo(&[("auditor", "agent", MODEL, &d)]);
    let root = tmp.path();
    let seat_before = rev(root, "ma/auditor");
    let main_before = rev(root, "main");

    let r = run_all(root, &opts(&["auditor"])).unwrap();
    let s = seat(&r, "auditor");
    assert_eq!(s.outcome, Outcome::Proposed, "{s:?}");
    let proposal = s.proposal.as_deref().unwrap();
    assert!(
        proposal.starts_with("propose/elector/auditor/budget/"),
        "{proposal}"
    );

    assert_eq!(
        rev(root, "ma/auditor"),
        seat_before,
        "the seat moved itself"
    );
    assert_eq!(rev(root, "main"), main_before);
    assert_eq!(rev(root, &format!("{proposal}^")), seat_before);

    let subject = git_out(root, &["log", "-1", "--format=%s", proposal]);
    assert_eq!(subject, "open: auditor on budget");
    let position = git_out(
        root,
        &[
            "show",
            &format!("{proposal}:.yidam/sangha/positions/auditor-budget.md"),
        ],
    );
    assert!(position.contains("Ran claude-opus-4-8 1."), "{position}");

    let receipt = git_out(
        root,
        &[
            "show",
            &format!("{proposal}:.yidam/runs/elector/auditor.yml"),
        ],
    );
    let occupant = elector_receipt::Occupant::parse(&receipt).unwrap();
    assert_eq!(occupant.model.as_deref(), Some(MODEL));
    assert_eq!(occupant.version.as_deref(), Some("1"));
    assert_eq!(occupant.config.as_deref(), Some(d.as_str()));
    assert!(receipt.contains("kind: elector"), "{receipt}");
}

#[test]
fn a_seat_that_binds_a_key_is_told_to_sign_the_position_rather_than_fast_forward_onto_it() {
    let d = digest_of(ELECTOR);
    let tmp = repo(&[("plain", "agent", MODEL, &d), ("keyed", "agent", MODEL, &d)]);
    let root = tmp.path();
    git(root, &["switch", "-q", "ma/keyed"]);
    fixture::write(
        root,
        REGISTRY,
        &format!(
            "| Name | Branch | Role | Kind | Model | Version | Config | Key |\n\
             |------|--------|------|------|-------|---------|--------|-----|\n\
             | `keyed` | `ma/keyed` | A seat. | agent | `{MODEL}` | `1` | `{d}` | \
             `ssh-ed25519 AAAAC3Nz` |\n"
        ),
    );
    commit(root, "update: keyed binds a key");
    git(root, &["switch", "-q", "main"]);

    let r = run_all(root, &opts(&["plain", "keyed"])).unwrap();
    let body = |name: &str| {
        let s = seat(&r, name);
        assert_eq!(s.outcome, Outcome::Proposed, "{s:?}");
        git_out(
            root,
            &["log", "-1", "--format=%B", s.proposal.as_deref().unwrap()],
        )
    };
    let plain = body("plain");
    assert!(
        plain.contains("git merge --ff-only propose/elector/plain/"),
        "{plain}"
    );
    assert!(!plain.contains("cherry-pick"), "{plain}");
    // An unsigned tip on a keyed seat is what `elector-signature-unverified` gates on.
    let keyed = body("keyed");
    assert!(
        keyed.contains("git cherry-pick -S propose/elector/keyed/"),
        "{keyed}"
    );
    assert!(!keyed.contains("--ff-only"), "{keyed}");
}

#[test]
fn a_repeat_is_unchanged_and_an_accepted_position_is_answered_with_revise() {
    let d = digest_of(ELECTOR);
    let tmp = repo(&[("auditor", "agent", MODEL, &d)]);
    let root = tmp.path();

    let first = run_all(root, &opts(&["auditor"])).unwrap();
    let again = run_all(root, &opts(&["auditor"])).unwrap();
    assert_eq!(seat(&again, "auditor").outcome, Outcome::Unchanged);

    accept(root, seat(&first, "auditor"));
    let next = run_all(root, &opts(&["auditor"])).unwrap();
    let s = seat(&next, "auditor");
    assert_eq!(s.outcome, Outcome::Proposed, "{s:?}");
    assert_eq!(s.verb.as_deref(), Some("revise"));
    let subject = git_out(
        root,
        &["log", "-1", "--format=%s", s.proposal.as_deref().unwrap()],
    );
    assert!(subject.starts_with("revise: "), "{subject}");
}

#[test]
fn a_dry_run_runs_the_elector_and_writes_no_branch() {
    let d = digest_of(ELECTOR);
    let tmp = repo(&[("auditor", "agent", MODEL, &d)]);
    let root = tmp.path();
    let mut o = opts(&["auditor"]);
    o.dry_run = true;
    let r = run_all(root, &o).unwrap();
    assert_eq!(seat(&r, "auditor").outcome, Outcome::DryRun);
    assert!(git_out(root, &["branch", "--list", "propose/*"]).is_empty());
}

// ── registry first ────────────────────────────────────────────────────────────

#[test]
fn a_row_that_does_not_describe_the_occupant_is_refused_before_anything_runs() {
    let d = digest_of(ELECTOR);
    let tmp = repo(&[
        ("stale", "agent", "claude-opus-3", &d),
        ("blank", "agent", MODEL, ""),
        ("wrongcfg", "agent", MODEL, "decafbad"),
        ("person", "human", "", ""),
    ]);
    let root = tmp.path();
    let r = run_all(
        root,
        &opts(&["stale", "blank", "wrongcfg", "person", "ghost"]),
    )
    .unwrap();
    for (name, says) in [
        ("stale", "`Model` is `claude-opus-3`"),
        ("blank", "`Config` is blank"),
        ("wrongcfg", "`Config` is `decafbad`"),
        ("person", "human seat"),
        ("ghost", "no `ma/ghost` branch"),
    ] {
        let s = seat(&r, name);
        assert_eq!(s.outcome, Outcome::Refused, "{name}: {s:?}");
        assert!(
            s.reason.as_deref().unwrap_or_default().contains(says),
            "{name}: {s:?}"
        );
    }
    assert!(!r.passed());
    assert!(
        r.independence.is_none(),
        "no seat ran, so there is nothing to derive"
    );
    assert!(git_out(root, &["branch", "--list", "propose/*"]).is_empty());
}

/// Everything is read at the seat's tip: a declaration the seat's own branch withdrew is gone,
/// whatever `main` still holds.
#[test]
fn the_declaration_is_read_at_the_seats_tip_and_not_at_head() {
    let d = digest_of(ELECTOR);
    let tmp = repo(&[("auditor", "agent", MODEL, &d)]);
    let root = tmp.path();
    git(root, &["switch", "-q", "ma/auditor"]);
    git(root, &["rm", "-q", &declaration_path("auditor")]);
    commit(root, "update: auditor withdraws its declaration");
    git(root, &["switch", "-q", "main"]);

    let r = run_all(root, &opts(&["auditor"])).unwrap();
    let s = seat(&r, "auditor");
    assert_eq!(s.outcome, Outcome::Refused, "{s:?}");
    assert!(
        s.reason.as_deref().unwrap().contains("has not declared"),
        "{s:?}"
    );
}

#[test]
fn an_elector_that_writes_outside_its_seat_or_writes_nothing_fails() {
    for (script, says) in [
        (
            "#!/bin/sh\nmkdir -p \"$YIDAM_OUT/.yidam/corpus\"\necho x > \
             \"$YIDAM_OUT/.yidam/corpus/x.yml\"\n",
            "x.yml",
        ),
        ("#!/bin/sh\nexit 0\n", "wrote no"),
        ("#!/bin/sh\necho refusing >&2\nexit 3\n", "exited 3"),
    ] {
        let d = digest_of(script);
        let tmp = repo(&[("auditor", "agent", MODEL, &d)]);
        let root = tmp.path();
        fixture::write(root, "elector.sh", script);
        commit(root, "update: a different elector");
        git(root, &["branch", "-f", "ma/auditor"]);

        let r = run_all(root, &opts(&["auditor"])).unwrap();
        let s = seat(&r, "auditor");
        assert_eq!(s.outcome, Outcome::Failed, "{s:?}");
        assert!(s.reason.as_deref().unwrap().contains(says), "{says}: {s:?}");
        assert!(!r.passed());
    }
}

// ── independence: derived from the registry, reported without a count ──────────

/// Three dispatches of one configuration are `shared-configuration` — in what the dispatcher
/// reports and in what `yidam lint` derives from the same tips — and neither says how many
/// positions there are.
#[test]
fn three_dispatches_of_one_configuration_are_shared_configuration_and_are_not_counted() {
    let d = digest_of(ELECTOR);
    let tmp = repo(&[
        ("auditor", "agent", MODEL, &d),
        ("advocate", "agent", MODEL, &d),
        ("archivist", "agent", MODEL, &d),
    ]);
    let root = tmp.path();
    let seats = ["auditor", "advocate", "archivist"];
    let r = run_all(root, &opts(&seats)).unwrap();
    assert!(r.passed(), "{:?}", r.seats);

    let reported = r.independence.as_ref().unwrap();
    assert_eq!(reported.value, "shared-configuration", "{reported:?}");

    for name in seats {
        accept(root, seat(&r, name));
    }
    assert_eq!(derived(root, &seats), "shared-configuration");

    // No count of positions, in either format.
    fn keys(v: &serde_json::Value, out: &mut Vec<String>) {
        match v {
            serde_json::Value::Object(m) => {
                for (k, v) in m {
                    out.push(k.clone());
                    keys(v, out);
                }
            }
            serde_json::Value::Array(a) => a.iter().for_each(|v| keys(v, out)),
            _ => {}
        }
    }
    let json = serde_json::to_value(&r).unwrap();
    let mut found = Vec::new();
    keys(&json, &mut found);
    for k in &found {
        assert!(
            !k.contains("count") && k != "positions" && k != "total",
            "the report carries `{k}`: {json}"
        );
    }
    let text = render(&r);
    let words: Vec<&str> = text.split_whitespace().collect();
    let counted = words
        .windows(2)
        .any(|w| w[0].parse::<usize>().is_ok() && w[1].starts_with("position"));
    assert!(!counted, "{text}");
}

#[test]
fn two_seats_under_different_configurations_report_distinct_seats() {
    let other = format!("{ELECTOR}# a different prompt\n");
    let tmp = repo(&[
        ("auditor", "agent", MODEL, &digest_of(ELECTOR)),
        ("advocate", "agent", MODEL, &digest_of(&other)),
    ]);
    let root = tmp.path();
    git(root, &["switch", "-q", "ma/advocate"]);
    fixture::write(root, "elector.sh", &other);
    commit(root, "update: advocate's own prompt");
    git(root, &["switch", "-q", "main"]);

    let r = run_all(root, &opts(&["auditor", "advocate"])).unwrap();
    assert!(r.passed(), "{:?}", r.seats);
    assert_eq!(r.independence.as_ref().unwrap().value, "distinct-seats");
}

/// Registry first, receipt corroborates: removing every receipt, or committing receipts that
/// disagree with the rows, leaves `independence:` exactly where the registry put it. The lint
/// is what sees the disagreement.
#[test]
fn receipts_never_move_the_derived_independence() {
    let d = digest_of(ELECTOR);
    let tmp = repo(&[
        ("auditor", "agent", MODEL, &d),
        ("advocate", "agent", MODEL, &d),
    ]);
    let root = tmp.path();
    let seats = ["auditor", "advocate"];
    let r = run_all(root, &opts(&seats)).unwrap();
    for name in seats {
        accept(root, seat(&r, name));
    }
    let with = derived(root, &seats);
    assert_eq!(with, "shared-configuration");
    assert!(
        elector_receipt::elector_receipt_disagrees(&elector_receipt::audit(root, &[]))
            .violations
            .is_empty(),
        "a dispatch the dispatcher accepted is one the lint accepts"
    );

    for name in seats {
        git(root, &["switch", "-q", &format!("ma/{name}")]);
        git(root, &["rm", "-q", &elector_receipt::receipt_path(name)]);
        commit(root, "update: drop the receipt");
    }
    git(root, &["switch", "-q", "main"]);
    assert_eq!(derived(root, &seats), with, "removing receipts moved it");

    for name in seats {
        git(root, &["switch", "-q", &format!("ma/{name}")]);
        fixture::write(
            root,
            &elector_receipt::receipt_path(name),
            &format!("model: other-{name}\nversion: '9'\nconfig: {name}{name}{name}\n"),
        );
        commit(root, "update: a receipt that says otherwise");
    }
    git(root, &["switch", "-q", "main"]);
    assert_eq!(derived(root, &seats), with, "disagreeing receipts moved it");
    let check = elector_receipt::elector_receipt_disagrees(&elector_receipt::audit(root, &[]));
    assert_eq!(check.violations.len(), 2, "{:?}", check.violations);
}
