//! Golden fixtures for `yidam query`, against `examples/streamflow` (#261, RFC-0018).
//!
//! The example is the fixture on purpose: its ontology is three classes and six
//! relationships, small enough that every expected answer below can be checked by hand
//! against the corpus, and it is the corpus the benchmark's goal set is written for. A
//! golden recorded against a synthetic fixture would be checking the query engine against
//! itself.
//!
//! Two of these cases are the acceptance criteria #261 names, and they are the reason the
//! file exists rather than a unit test:
//!
//! - **an unknown name fails with a diagnosis, never as an empty result** — `unknown-class`
//!   and `unlicensed-hop` below, both of which come back with a payload and exit 1;
//! - **an empty result is distinguishable from an unknown name** — `sourced-from` runs,
//!   returns nothing, and says in the diagnostic that the name differs by one character from
//!   one the class declares.
//!
//! Run with `UPDATE_GOLDENS=1` to rewrite the expected files after an intended change. The
//! diff is the review.

mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

use common::{repo_root, tracked_under};

const EXAMPLE: &str = "examples/streamflow/";

fn goldens_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/goldens/query")
}

/// `examples/streamflow` as a standalone repository.
///
/// From `git ls-files`, matching every other suite here: a directory walk picks up
/// `.DS_Store` and any local scratch, and the test would then be measuring the maintainer's
/// working directory.
fn stage() -> tempfile::TempDir {
    let root = repo_root();
    let dir = tempfile::tempdir().unwrap();
    for tracked in tracked_under(&root, EXAMPLE) {
        let to = dir.path().join(tracked.strip_prefix(EXAMPLE).unwrap());
        std::fs::create_dir_all(to.parent().unwrap()).unwrap();
        std::fs::copy(root.join(&tracked), &to).unwrap();
    }
    for args in [
        vec!["init", "-q"],
        vec!["config", "user.email", "query@yidam.test"],
        vec!["config", "user.name", "Query"],
        vec!["add", "-A"],
        vec!["commit", "-qm", "chore: genesis — query"],
    ] {
        assert!(Command::new("git")
            .args(&args)
            .current_dir(dir.path())
            .status()
            .unwrap()
            .success());
    }
    dir
}

/// Replace what belongs to the run rather than to the corpus.
///
/// The same three redactions `report_goldens` makes, for the same reasons: the absolute
/// root, the crate version and the build commit vary by machine, and the feature set varies
/// by job — `ci (cli · full features)` runs `--all-features` against goldens recorded from
/// the light build.
fn redact(out: &str, root: &Path) -> String {
    let canonical = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    let out = out
        .replace(&canonical.display().to_string(), "<ROOT>")
        .replace(&root.display().to_string(), "<ROOT>")
        .replace(
            &format!("\"version\": \"{}\"", env!("CARGO_PKG_VERSION")),
            "\"version\": \"<VERSION>\"",
        )
        .replace(
            &format!("\"commit\": \"{}\"", env!("YIDAM_BUILD_COMMIT")),
            "\"commit\": \"<COMMIT>\"",
        );
    let Some(start) = out.find("\"features\": [") else {
        return out;
    };
    let open = start + "\"features\": [".len();
    let Some(close) = out[open..].find(']') else {
        return out;
    };
    format!(
        "{}\"features\": [\n      \"<FEATURES>\"\n    {}",
        &out[..start],
        &out[open + close..]
    )
}

struct Case {
    /// Golden filename stem, and what the case is for.
    name: &'static str,
    query: &'static str,
    /// Exit code. 1 only for a rejected query — `query` never gates on the corpus.
    code: i32,
}

/// Every case, with what it is here to pin.
const CASES: &[Case] = &[
    // The entry step alone.
    Case {
        name: "class-only",
        query: "reach",
        code: 0,
    },
    // One typed hop, and the traversal `neighbors` could not express.
    Case {
        name: "one-hop",
        query: "reach -measured-by-> gage",
        code: 0,
    },
    // Two hops through two different relationships.
    Case {
        name: "two-hops",
        query: "reach -measured-by-> gage -sources-from-> concept",
        code: 0,
    },
    // The same edge read from the far end.
    Case {
        name: "backward-hop",
        query: "concept <-exhibits- reach",
        code: 0,
    },
    // `*` narrowed by a property predicate, with the skipped classes named.
    Case {
        name: "star-filter",
        query: "*[claim_tag=open]",
        code: 0,
    },
    // The row that shipped wrong in RFC-0018's first draft: `regulated` holds prose, so `=`
    // is correct to match nothing and `~` is the working form. Both are pinned so the pair
    // cannot drift apart.
    Case {
        name: "exact-matches-nothing",
        query: "reach[regulated=yes]",
        code: 0,
    },
    Case {
        name: "contains-matches",
        query: "reach[regulated~yes]",
        code: 0,
    },
    // #261's acceptance, both halves.
    Case {
        name: "unknown-class",
        query: "gauge",
        code: 1,
    },
    Case {
        name: "unlicensed-hop",
        query: "reach -measured-by-> concept",
        code: 1,
    },
    // The near miss: `sourced-from` is authored (into the catalog) and `sources-from` is
    // declared. It runs, returns nothing, and says which is which.
    Case {
        name: "near-miss-empty",
        query: "gage -sourced-from-> concept",
        code: 0,
    },
];

fn run_case(dir: &Path, case: &Case) -> (String, i32) {
    let out = Command::new(env!("CARGO_BIN_EXE_yidam"))
        .args(["query", case.query, "--format", "json"])
        .current_dir(dir)
        .output()
        .unwrap();
    (
        String::from_utf8_lossy(&out.stdout).to_string(),
        out.status.code().unwrap_or(-1),
    )
}

#[test]
fn every_case_matches_its_golden() {
    let dir = stage();
    let update = std::env::var("UPDATE_GOLDENS").is_ok();
    std::fs::create_dir_all(goldens_dir()).unwrap();

    let mut failures = Vec::new();
    for case in CASES {
        let (stdout, code) = run_case(dir.path(), case);
        assert_eq!(code, case.code, "{}: exit code\n{stdout}", case.name);

        let actual = redact(&stdout, dir.path());
        let path = goldens_dir().join(format!("{}.json", case.name));
        if update {
            std::fs::write(&path, &actual).unwrap();
            continue;
        }
        let expected = std::fs::read_to_string(&path).unwrap_or_else(|_| {
            panic!("no golden for `{}` — run with UPDATE_GOLDENS=1", case.name)
        });
        if expected != actual {
            failures.push(format!(
                "--- {} ---\nexpected:\n{expected}\nactual:\n{actual}",
                case.name
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}

/// A golden nobody runs looks exactly like one that is doing work. The rule runs both ways,
/// as it does for the parity fixtures.
#[test]
fn every_golden_has_a_case_that_reads_it() {
    let names: Vec<String> = CASES.iter().map(|c| format!("{}.json", c.name)).collect();
    let orphans: Vec<String> = std::fs::read_dir(goldens_dir())
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().to_string())
        .filter(|f| f.ends_with(".json") && !names.contains(f))
        .collect();
    assert!(orphans.is_empty(), "goldens no case reads: {orphans:?}");
}

/// The property the goldens cannot state, because they are one run each: the text and JSON
/// paths must agree about whether the query was rejected. A gate that gates differently
/// depending on how you asked for the answer is not a gate.
#[test]
fn the_text_and_json_paths_agree_on_the_verdict() {
    let dir = stage();
    for case in CASES {
        let text = Command::new(env!("CARGO_BIN_EXE_yidam"))
            .args(["query", case.query])
            .current_dir(dir.path())
            .output()
            .unwrap();
        assert_eq!(
            text.status.code().unwrap_or(-1),
            case.code,
            "{}: text path disagreed with json",
            case.name
        );
        let rendered = String::from_utf8_lossy(&text.stdout);
        assert_eq!(
            rendered.contains("rejected ("),
            case.code == 1,
            "{}: {rendered}",
            case.name
        );
    }
}
