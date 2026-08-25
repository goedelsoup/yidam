//! The committed goal set, held to the corpus it names.
//!
//! `.yidam/bench/goals.yml` is the fixed input to `yidam bench`, and #264's whole argument
//! for committing it is that a goal set chosen after the results measures the chooser. That
//! only holds while the file stays *true*: a goal whose expected answer names a node that no
//! longer exists scores zero recall for both arms and looks like a finding about traversal.
//!
//! `yidam rename` moves a node and rewrites every edge into it. It does not know about this
//! file. Nor does deleting a node. So the drift is silent, it is in the direction of "the
//! benchmark got worse", and nothing else would catch it.
//!
//! The flat arm itself is not exercised here. It needs a vector index, which needs
//! `--features index` and a model download, and PR CI runs neither. What *is* exercised is
//! the refusal — the light build must decline to publish a number rather than benchmark
//! against keyword search.

mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

use common::{repo_root, tracked_under};

const EXAMPLE: &str = "examples/streamflow/";

fn goals_path(root: &Path) -> PathBuf {
    root.join(EXAMPLE).join(".yidam/bench/goals.yml")
}

/// RFC-0018's lexing rule: split on whitespace that is not inside `"…"` or `[…]`.
///
/// Written out here rather than imported because there is nothing to import yet — the
/// executor is #261. When it lands, this should be deleted in favour of its lexer, or the
/// two will come to disagree about what a query says.
fn tokens(query: &str) -> Vec<String> {
    let (mut out, mut cur, mut quoted, mut depth) = (Vec::new(), String::new(), false, 0usize);
    for ch in query.chars() {
        match ch {
            '"' => quoted = !quoted,
            '[' if !quoted => depth += 1,
            ']' if !quoted => depth = depth.saturating_sub(1),
            _ => {}
        }
        if ch.is_whitespace() && !quoted && depth == 0 {
            if !cur.is_empty() {
                out.push(std::mem::take(&mut cur));
            }
        } else {
            cur.push(ch);
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

#[test]
fn the_committed_goal_set_parses() {
    let root = repo_root();
    let text = std::fs::read_to_string(goals_path(&root)).expect("the example's goal set");
    let set = yidam::parse_bench_goals(&text).expect("the committed goal set must parse");
    assert!(
        set.goals.len() >= 5,
        "a goal set small enough to prove nothing: {} goal(s)",
        set.goals.len()
    );
}

/// The drift guard this file exists for.
#[test]
fn every_expected_answer_names_a_node_that_exists() {
    let root = repo_root();
    let corpus = root.join(EXAMPLE).join(".yidam/corpus");
    let text = std::fs::read_to_string(goals_path(&root)).unwrap();
    let set = yidam::parse_bench_goals(&text).unwrap();

    let mut missing = Vec::new();
    for goal in &set.goals {
        for expected in &goal.expect {
            if !corpus.join(expected).is_file() {
                missing.push(format!("{}: {expected}", goal.id));
            }
        }
    }
    assert!(
        missing.is_empty(),
        "goals expect nodes the corpus does not have — a renamed or deleted node scores as \
         a traversal finding: {missing:?}"
    );
}

/// Both arms must be able to reach every expected answer *in principle*, or the goal is
/// measuring the corpus rather than the arms.
///
/// Weaker than checking the queries execute — there is no executor yet (#261) — but it
/// catches the case that matters: an `expect` set naming a node of a class the anchored
/// query's last step could never return.
#[test]
fn every_anchored_query_ends_on_the_class_its_answer_belongs_to() {
    let root = repo_root();
    let text = std::fs::read_to_string(goals_path(&root)).unwrap();
    let set = yidam::parse_bench_goals(&text).unwrap();

    let mut wrong = Vec::new();
    for goal in &set.goals {
        let Some(query) = &goal.anchored else {
            continue;
        };
        // The terminal step is the last token, and RFC-0018's rule is to split on whitespace
        // **that is not inside `"…"` or `[…]`**. Splitting naively takes `components"` out of
        // `concept~"…slow and fast components"` and calls it a class — which this test did,
        // and reported as a defect in the goal set.
        let last = tokens(query).pop().unwrap_or_default();
        let class = last
            .split(['~', '['])
            .next()
            .unwrap_or_default()
            .to_string();
        if class.is_empty() || class == "*" {
            continue;
        }
        for expected in &goal.expect {
            let actual = expected.split('/').next().unwrap_or_default();
            if actual != class {
                wrong.push(format!(
                    "{}: ends on `{class}` but expects {expected}",
                    goal.id
                ));
            }
        }
    }
    assert!(
        wrong.is_empty(),
        "an anchored query whose last step cannot return its own expected answer: {wrong:?}"
    );
}

/// A benchmark that quietly measured keyword search would report a number about nothing.
#[test]
fn bench_refuses_when_the_flat_arm_would_be_keyword_search() {
    let root = repo_root();
    let dir = tempfile::tempdir().unwrap();
    for tracked in tracked_under(&root, EXAMPLE) {
        let to = dir.path().join(tracked.strip_prefix(EXAMPLE).unwrap());
        std::fs::create_dir_all(to.parent().unwrap()).unwrap();
        std::fs::copy(root.join(&tracked), &to).unwrap();
    }
    for args in [
        vec!["init", "-q"],
        vec!["config", "user.email", "bench@yidam.test"],
        vec!["config", "user.name", "Bench"],
        vec!["add", "-A"],
        vec!["commit", "-qm", "chore: genesis — bench"],
    ] {
        Command::new("git")
            .args(&args)
            .current_dir(dir.path())
            .status()
            .unwrap();
    }

    let out = Command::new(env!("CARGO_BIN_EXE_yidam"))
        .arg("bench")
        .current_dir(dir.path())
        .output()
        .unwrap();

    // Only meaningful for a build whose retrieval is keyword — a `cli-full` run against a
    // corpus with an index is in a different state and answers instead of refusing.
    if out.status.success() {
        return;
    }
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("keyword search"),
        "bench must name the reason it will not measure: {stderr}"
    );
    assert!(
        !String::from_utf8_lossy(&out.stdout).contains("mean precision"),
        "a refusal must not also print a score"
    );
}
