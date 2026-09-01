//! The benchmark ratchets.
//!
//! `bench-scaling` has run on every pull request since #264 and asserted nothing about what it
//! printed. `mise.toml` said so in its own comment — *"It ratchets nothing yet"* — and gave a
//! reason that has since expired: #261 landed, and the query executor it was waiting for
//! exists. What was left is a job that catches retrieval breaking outright and nothing in
//! between.
//!
//! # Why a golden and not a tolerance band
//!
//! #468 asks for "a committed bench baseline with a tolerance band". A band is the right
//! instrument for a *timing* benchmark, and this is not one. `--scaling` generates its
//! corpora from a seeded xorshift, reads no clock, and reports counts — nodes considered,
//! tokens read, precision, recall. Three consecutive runs here are byte-identical, and the
//! arithmetic is `+ - * /` on `f64` with no transcendental function and no hash iteration
//! anywhere in the path, so it is identical across platforms too, not merely across runs.
//!
//! Against a deterministic number a band is strictly worse than an exact check: it admits a
//! real regression up to the width of the band and protects against nothing an exact
//! comparison misses. So the baseline is exact, and what a band would have bought — a legible
//! answer to *"what moved, and is it worse?"* — is bought instead by [`compare`], which names
//! the arm, the size, the metric and the direction.
//!
//! `UPDATE_GOLDENS=1` refreshes it, the way every other golden here is refreshed. The diff is
//! the review, and for this file the diff is the benchmark result.
//!
//! # What is not ratcheted
//!
//! `bench-example`, the real-corpus arm. It needs `--features index`, a vector index and an
//! embedding run, it executes on main and the weekly schedule rather than on pull requests,
//! and whether an ANN search over embedded prose is reproducible enough to golden is a
//! question this phase did not measure. Guessing a band for it would be inventing the number
//! rather than finding it.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::Command;

fn golden_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/goldens/bench/scaling.json")
}

/// Run the real binary over the committed config, as the gate does.
fn measure() -> serde_json::Value {
    let out = Command::new(env!("CARGO_BIN_EXE_yidam"))
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .args(["bench", "--scaling", "--format", "json"])
        .output()
        .expect("yidam runs");
    assert!(
        out.status.success(),
        "`yidam bench --scaling --format json` failed:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let mut value: serde_json::Value =
        serde_json::from_slice(&out.stdout).expect("the report is JSON");
    redact(&mut value);
    value
}

/// Drop the envelope fields that describe the build rather than the measurement.
///
/// And fail if they were not there to drop. A redaction that silently matches nothing turns a
/// golden into a test of the redactor — the assertion `report_goldens.rs` needed for the same
/// reason, and the one that keeps this from passing after the envelope stops being emitted.
fn redact(value: &mut serde_json::Value) {
    let yidam = value
        .get_mut("yidam")
        .expect("the report carries no `yidam` block; the envelope is gone");
    assert!(
        yidam["commit"].is_string() && yidam["version"].is_string(),
        "the `yidam` block no longer carries a version and a commit"
    );
    yidam["commit"] = serde_json::Value::String("<commit>".into());
    yidam["version"] = serde_json::Value::String("<version>".into());
    assert!(
        value["root"].as_str().is_some_and(|r| r.starts_with('/')),
        "`root` is not an absolute path"
    );
    value["root"] = serde_json::Value::String("<root>".into());
}

// ── reading a report as a flat set of measurements ───────────────────────────

/// Every number the report states, keyed by where it sits.
///
/// Flattened so a difference can be *named*. Comparing two documents tells a reader that
/// something moved; comparing two maps tells them that focused-scan at N=4096 now reads 40
/// more nodes, which is the sentence a benchmark exists to produce.
fn metrics(report: &serde_json::Value) -> BTreeMap<String, f64> {
    let mut out = BTreeMap::new();
    let rows = report["rows"].as_array().expect("the report has no rows");
    assert!(!rows.is_empty(), "the report has no rows at all");

    for row in rows {
        let n = row["corpus"]["nodes"].as_u64().expect("a row with no size");
        for (key, value) in [
            ("goals", &row["goals"]),
            ("mean_expected", &row["mean_expected"]),
            ("mean_hops", &row["mean_hops"]),
            ("narrowing_ceiling", &row["corpus"]["narrowing_ceiling"]),
        ] {
            if let Some(number) = value.as_f64() {
                out.insert(format!("n={n} {key}"), number);
            }
        }
        for arm in ["full_scan", "focused_scan", "anchored"] {
            let report = &row[arm];
            if report["ran"] != serde_json::Value::Bool(true) {
                continue;
            }
            for metric in ["candidates", "precision", "recall", "nodes_read", "tokens"] {
                if let Some(number) = report[metric].as_f64() {
                    out.insert(format!("n={n} {arm}.{metric}"), number);
                }
            }
        }
    }
    assert!(
        out.len() > 20,
        "only {} measurements parsed out of the report; the reader is looking at the wrong \
         shape and this comparison would be vacuous",
        out.len()
    );
    out
}

/// Which direction is worse for a metric.
///
/// The half a golden cannot express. `tokens` going up is a regression and `recall` going up
/// is not, and a benchmark that cannot say which has not asserted anything about its numbers —
/// it has asserted that they did not change.
fn higher_is_worse(metric: &str) -> Option<bool> {
    let name = metric.rsplit('.').next().unwrap_or(metric);
    match name {
        "candidates" | "nodes_read" | "tokens" => Some(true),
        "precision" | "recall" => Some(false),
        // Generator shape: `goals`, `mean_hops`, `narrowing_ceiling`. A change is a change to
        // the experiment, not a result, and calling it a regression would be a category error.
        _ => None,
    }
}

/// A human sentence per number that moved, worst first.
fn compare(baseline: &BTreeMap<String, f64>, measured: &BTreeMap<String, f64>) -> Vec<String> {
    let mut lines = Vec::new();

    for (key, was) in baseline {
        let Some(now) = measured.get(key) else {
            lines.push(format!("  {key}: gone — the report no longer states it"));
            continue;
        };
        if now == was {
            continue;
        }
        let verdict = match higher_is_worse(key) {
            Some(true) if now > was => "REGRESSED",
            Some(false) if now < was => "REGRESSED",
            Some(_) => "improved",
            None => "changed (experiment shape)",
        };
        let delta = now - was;
        let pct = if *was != 0.0 {
            format!("{:+.1}%", (delta / was) * 100.0)
        } else {
            "n/a".to_string()
        };
        lines.push(format!("  {key}: {was} → {now} ({pct}) — {verdict}"));
    }
    for key in measured.keys() {
        if !baseline.contains_key(key) {
            lines.push(format!(
                "  {key}: new — the report states it and the baseline does not"
            ));
        }
    }
    // Regressions first: a diff that buries the one bad number among nine good ones is a diff
    // that gets skimmed.
    lines.sort_by_key(|l| !l.contains("REGRESSED"));
    lines
}

/// The numbers the benchmark produced are the numbers it is committed to producing.
#[test]
fn the_scaling_benchmark_matches_its_baseline() {
    let measured = measure();
    let path = golden_path();

    if std::env::var("UPDATE_GOLDENS").is_ok() {
        std::fs::create_dir_all(path.parent().expect("a parent")).expect("goldens dir");
        let mut text = serde_json::to_string_pretty(&measured).expect("serializes");
        text.push('\n');
        std::fs::write(&path, text).expect("write golden");
        return;
    }

    let baseline: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap_or_else(|e| {
            panic!(
                "{} is unreadable ({e}). Run with UPDATE_GOLDENS=1 to record the first \
                 baseline.",
                path.display()
            )
        }))
        .expect("the baseline is JSON");

    let moved = compare(&metrics(&baseline), &metrics(&measured));
    assert!(
        moved.is_empty(),
        "the scaling benchmark moved:\n{}\n\nThe generator is seeded and reads no clock, so \
         this is a real change in what the corpus costs to answer over — not noise. If it is \
         intended, re-run with UPDATE_GOLDENS=1 and let the diff be the review.",
        moved.join("\n")
    );

    // The whole document, after the numbers, so a change to the *reasons* — why an arm did
    // not run, what the config assumed — is a reviewable diff rather than a silent edit.
    assert_eq!(
        serde_json::to_string_pretty(&measured).expect("serializes"),
        serde_json::to_string_pretty(&baseline).expect("serializes"),
        "the benchmark's numbers are unchanged but the report around them is not. Re-run \
         with UPDATE_GOLDENS=1 to accept it."
    );
}

/// The baseline covers the sizes the committed config asks for.
///
/// A baseline recorded from a run that silently measured two sizes instead of four would be a
/// green ratchet over a quarter of the experiment. The config is the source; the baseline is
/// checked against it rather than against a number written here.
#[test]
fn the_baseline_covers_every_size_the_config_declares() {
    let config: toml::Value = toml::from_str(
        &std::fs::read_to_string(
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/cmd/bench/scaling.toml"),
        )
        .expect("scaling.toml"),
    )
    .expect("scaling.toml parses");
    let sizes: Vec<u64> = config["sizes"]
        .as_array()
        .expect("no `sizes`")
        .iter()
        .map(|v| v.as_integer().expect("a size") as u64)
        .collect();
    assert!(!sizes.is_empty(), "the config declares no sizes");

    let baseline: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(golden_path()).expect("baseline"))
            .expect("the baseline is JSON");
    let recorded: Vec<u64> = baseline["rows"]
        .as_array()
        .expect("no rows")
        .iter()
        .map(|r| r["corpus"]["nodes"].as_u64().expect("a size"))
        .collect();

    assert_eq!(
        recorded, sizes,
        "the baseline records sizes {recorded:?} and scaling.toml declares {sizes:?}. A \
         baseline over fewer sizes than the experiment is a ratchet with holes in it."
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map(pairs: &[(&str, f64)]) -> BTreeMap<String, f64> {
        pairs.iter().map(|(k, v)| (k.to_string(), *v)).collect()
    }

    /// Cost going up is a regression; recall going up is not. The distinction a golden alone
    /// cannot draw, and the reason `compare` exists rather than a byte comparison.
    #[test]
    fn direction_decides_whether_a_change_is_a_regression() {
        let base = map(&[
            ("n=8 full_scan.tokens", 100.0),
            ("n=8 full_scan.recall", 1.0),
        ]);
        let worse = compare(
            &base,
            &map(&[
                ("n=8 full_scan.tokens", 120.0),
                ("n=8 full_scan.recall", 1.0),
            ]),
        );
        assert_eq!(worse.len(), 1);
        assert!(worse[0].contains("REGRESSED"), "{worse:?}");
        assert!(worse[0].contains("+20.0%"), "{worse:?}");

        let better = compare(
            &base,
            &map(&[
                ("n=8 full_scan.tokens", 80.0),
                ("n=8 full_scan.recall", 1.0),
            ]),
        );
        assert!(better[0].contains("improved"), "{better:?}");

        let lost_recall = compare(
            &base,
            &map(&[
                ("n=8 full_scan.tokens", 100.0),
                ("n=8 full_scan.recall", 0.5),
            ]),
        );
        assert!(lost_recall[0].contains("REGRESSED"), "{lost_recall:?}");
    }

    /// A metric that vanished is reported rather than passed over. An arm that stopped
    /// running would otherwise shrink the comparison to the arms that still do.
    #[test]
    fn a_measurement_that_disappears_is_a_finding() {
        let moved = compare(&map(&[("n=8 focused_scan.tokens", 10.0)]), &map(&[]));
        assert_eq!(moved.len(), 1);
        assert!(moved[0].contains("gone"), "{moved:?}");
    }

    #[test]
    fn regressions_are_listed_before_improvements() {
        let base = map(&[("a.tokens", 10.0), ("b.tokens", 10.0)]);
        let moved = compare(&base, &map(&[("a.tokens", 5.0), ("b.tokens", 20.0)]));
        assert!(moved[0].contains("REGRESSED"), "{moved:?}");
        assert!(moved[1].contains("improved"), "{moved:?}");
    }

    /// An identical run reports nothing. Without this the assertion above could be satisfied
    /// by a comparison that always finds something.
    #[test]
    fn an_unchanged_report_produces_no_lines() {
        let base = map(&[("n=8 full_scan.tokens", 100.0)]);
        assert!(compare(&base, &base).is_empty());
    }
}
