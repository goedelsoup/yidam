//! Committed baselines are golden fixtures for the checks that read them.
//!
//! A baseline is a real bootstrap run, captured whole: the corpus the agent produced, the
//! history it wrote, and the verdict the checks returned. The corpus in it never changes
//! again — so if a recomputed verdict differs from the recorded one, the checks changed, and
//! this test says which and how.
//!
//! This is the gate that makes `harness diff` mean something. Until a baseline existed, the
//! regression machinery was fully implemented and had never had two snapshots to compare;
//! `tests/results/` did not exist at all, while three documents described how regressions
//! would be visible as diffs inside it.
//!
//! It needs no model and no API key, which is the point: the expensive half of an eval is
//! producing the run, and that half is already paid for and committed.

use std::path::{Path, PathBuf};

use yidam_harness::{check, snapshot};

fn results_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../results")
}

/// Every directory holding a `structural.json`.
fn baselines() -> Vec<PathBuf> {
    let root = results_root();
    if !root.exists() {
        return vec![];
    }
    let mut found: Vec<PathBuf> = walkdir::WalkDir::new(&root)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file() && e.file_name() == "structural.json")
        .filter_map(|e| e.path().parent().map(Path::to_path_buf))
        .collect();
    found.sort();
    found
}

/// A test that passes because it found nothing to check is the failure this whole exercise
/// has been about. Three of the structural checks did exactly that for the whole of protocol
/// 0.1.0.
#[test]
fn there_is_at_least_one_baseline_to_check() {
    assert!(
        !baselines().is_empty(),
        "no baseline under {}. The regression gate below has nothing to assert, and passes \
         for that reason rather than because anything is right.",
        results_root().display()
    );
}

/// The corpus is fixed. A change in the verdict is a change in the checks.
#[test]
fn no_baseline_has_drifted() {
    let mut drifted = Vec::new();

    for dir in baselines() {
        let recorded = snapshot::load(&dir)
            .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
            .structural;
        let fresh = check::run_all(&dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display()));

        let lines = check::drift(&recorded, &fresh);
        if !lines.is_empty() {
            drifted.push(format!("{}:\n  {}", dir.display(), lines.join("\n  ")));
        }
    }

    assert!(
        drifted.is_empty(),
        "recorded baselines no longer produce the verdicts recorded with them:\n\n{}\n\n\
         The corpora in these directories have not changed, so the checks have. If the new \
         behaviour is right, re-record with `harness check` and say why in the commit — a \
         baseline updated without a reason is a baseline that follows the code rather than \
         holding it to anything.",
        drifted.join("\n\n")
    );
}

/// A baseline whose provenance is missing cannot be compared to a later run: `harness diff`
/// refuses to compare across protocol versions, and needs each side to say which it is.
#[test]
fn every_baseline_records_what_produced_it() {
    for dir in baselines() {
        let snap = snapshot::load(&dir).unwrap();
        assert!(
            snap.protocol_version.is_some(),
            "{} records no protocol version",
            dir.display()
        );
        let run = snap.run.unwrap_or_else(|| {
            panic!(
                "{} has no run record — it cannot say what model produced it",
                dir.display()
            )
        });
        assert!(
            run.model_resolved.is_some(),
            "{} does not record the model the session resolved to",
            dir.display()
        );
        assert!(
            !run.was_denied(),
            "{} was captured from a run the permission layer prevented ({}). Its verdicts \
             describe the harness, not a bootstrap, and it must not be a baseline.",
            dir.display(),
            run.permission_denials.join(", ")
        );
    }
}
