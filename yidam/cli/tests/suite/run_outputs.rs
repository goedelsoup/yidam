//! A step's outputs are read before they land (RFC-0056 §2.1).
//!
//! Each test adds one step to `examples/streamflow` that writes fixed bytes, commits it, and
//! runs it through the real binary. The bytes are the shapes the run check exists for: the
//! misindented block six commits of `ngsim-arterial.yml` carried, a signal table a later
//! reader refuses, a file under a spelling no reader lists. Its counterparts are the shapes it
//! must leave alone: a report artifact in any form, and a neighbour's defect that this step
//! did not write.

use std::fmt::Write as _;

use crate::common;

use common::git::out as git;
use common::Example;

const STEP: &str = "probe";

/// `examples/streamflow` with one more step, `probe`, which writes each `(path, text)` under
/// `$YIDAM_OUT` and nothing else. `committed` lands first, as files another step wrote.
fn with_probe(files: &[(&str, &str)], committed: &[(&str, &str)]) -> Example {
    let e = Example::materialize("streamflow");
    let root = e.path();
    let mut script = String::from("set -eu\n");
    for (path, text) in files {
        assert!(!text.contains("PROBE_EOF"), "the heredoc would end early");
        let _ = write!(
            script,
            "mkdir -p \"$(dirname \"$YIDAM_OUT/{path}\")\"\n\
             cat > \"$YIDAM_OUT/{path}\" <<'PROBE_EOF'\n{text}PROBE_EOF\n"
        );
    }
    std::fs::write(root.join(".yidam/capabilities/probe.sh"), script).unwrap();
    let manifest = root.join(".yidam/capabilities.toml");
    let mut text = std::fs::read_to_string(&manifest).unwrap();
    text.push_str(
        "\n[capability.probe]\nkind   = \"calculator\"\n\
         run    = [\"sh\", \".yidam/capabilities/probe.sh\"]\n\
         reads  = [\".yidam/capabilities/**\"]\nwrites = [\".yidam/computed/**\"]\n\
         verb   = \"compute\"\n",
    );
    std::fs::write(&manifest, text).unwrap();
    for (path, text) in committed {
        let at = root.join(path);
        std::fs::create_dir_all(at.parent().unwrap()).unwrap();
        std::fs::write(at, text).unwrap();
    }
    git(&root, &["add", "-A"]);
    git(
        &root,
        &["commit", "-qm", "fix: a step that writes fixed bytes"],
    );
    e
}

/// Whether the probe's receipt is at HEAD, which is whether anything it wrote landed.
fn landed(e: &Example) -> bool {
    common::git::succeeded(
        &e.path(),
        &["cat-file", "-e", &format!("HEAD:.yidam/runs/{STEP}.yml")],
    )
}

/// Run the probe, assert it was refused with nothing landed, and hand back the refusal.
fn refused(e: &Example) -> String {
    let before = git(&e.path(), &["rev-parse", "HEAD"]);
    let (out, err, code) = e.run(&["run", STEP]);
    assert_ne!(code, 0, "the probe's output was accepted:\n{out}{err}");
    assert!(err.contains("nothing was committed"), "{err}");
    assert!(!landed(e), "a refused probe committed its receipt");
    assert_eq!(
        git(&e.path(), &["rev-parse", "HEAD"]),
        before,
        "a refused probe moved the branch"
    );
    err
}

fn accepted(e: &Example) {
    let (out, err, code) = e.run(&["run", STEP]);
    assert_eq!(code, 0, "the probe was refused:\n{out}{err}");
    assert!(landed(e), "an accepted probe left no receipt:\n{out}{err}");
}

const GOOD: &str = "format_version: 1\nsignals:\n- node: gage/canyon-outlet\n  probe_signal: 1\n";

#[test]
fn a_block_that_does_not_parse_is_refused_with_its_line_quoted() {
    let e = with_probe(
        &[(
            ".yidam/computed/probe.yml",
            "format_version: 1\nqueue_discharge:\n  crossings: 4757\n  queues: 332\n  \
             - {direction: 2, queues: 106}\n",
        )],
        &[],
    );
    let err = refused(&e);
    assert!(
        err.contains(".yidam/computed/probe.yml does not parse as YAML"),
        "{err}"
    );
    assert!(
        err.contains("line 5:   - {direction: 2, queues: 106}"),
        "{err}"
    );
}

#[test]
fn a_report_artifact_that_does_not_parse_is_refused_too() {
    let e = with_probe(
        &[(
            ".yidam/computed/reports/probe.json",
            "{\"rows\": [1, 2,]}\n",
        )],
        &[],
    );
    let err = refused(&e);
    assert!(err.contains("probe.json does not parse as JSON"), "{err}");
    assert!(err.contains("line 1: {\"rows\": [1, 2,]}"), "{err}");
}

#[test]
fn a_signal_table_with_no_format_version_is_refused() {
    let e = with_probe(
        &[(
            ".yidam/computed/probe.yml",
            "signals:\n- node: gage/canyon-outlet\n  probe_signal: 1\n",
        )],
        &[],
    );
    let err = refused(&e);
    assert!(err.contains("a later reader refuses"), "{err}");
    assert!(err.contains("no `format_version`"), "{err}");
}

#[test]
fn a_row_keyed_on_a_node_the_corpus_does_not_hold_is_refused() {
    let e = with_probe(
        &[(
            ".yidam/computed/probe.yml",
            "format_version: 1\nsignals:\n- node: gage/nowhere\n  probe_signal: 1\n",
        )],
        &[],
    );
    let err = refused(&e);
    assert!(err.contains(".yidam/corpus/gage/nowhere.yml"), "{err}");
}

#[test]
fn a_signal_name_a_committed_table_already_claims_is_refused() {
    let e = with_probe(
        &[(".yidam/computed/probe.yml", GOOD)],
        &[(".yidam/computed/neighbour.yml", GOOD)],
    );
    let err = refused(&e);
    assert!(
        err.contains("`probe_signal` is declared by both"),
        "the collision with the committed table is not named:\n{err}"
    );
}

#[test]
fn a_table_under_the_spelling_no_reader_lists_is_refused_with_the_one_it_reads() {
    let e = with_probe(&[(".yidam/computed/probe.yaml", GOOD)], &[]);
    let err = refused(&e);
    assert!(
        err.contains("Write it as .yidam/computed/probe.yml"),
        "{err}"
    );
}

#[test]
fn a_report_artifact_keeps_every_form_it_has() {
    let e = with_probe(
        &[
            (".yidam/computed/probe.yml", GOOD),
            (".yidam/computed/probe.tsv", "node\tx\n- [00:00\tnot yaml\n"),
            (".yidam/computed/probe.txt", "- [unclosed\n"),
            (
                ".yidam/computed/reports/probe.yml",
                "free: form\nlist: [1, 2]\n",
            ),
            (".yidam/computed/reports/probe.json", "{\"rows\": [1, 2]}\n"),
        ],
        &[],
    );
    accepted(&e);
}

#[test]
fn a_neighbour_s_defect_does_not_block_a_step_that_did_not_write_it() {
    let e = with_probe(
        &[(".yidam/computed/probe.yml", GOOD)],
        &[(
            ".yidam/computed/neighbour.yml",
            "signals:\n- node: gage/canyon-outlet\n  other: 1\n",
        )],
    );
    accepted(&e);
}
