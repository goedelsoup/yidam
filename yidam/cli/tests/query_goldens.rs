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
//! Since #283 both empty cases also carry an `absence`: *which kind* of empty, derived from
//! the ontology rather than guessed. `exact-matches-nothing` is the one worth reading — it is
//! the row RFC-0018's first draft shipped wrong, and the golden now shows the query answering
//! its own confusion by naming the values `regulated` actually holds.
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
    // Two commits, not one. A series golden needs a range, and `a..b` over a repository with
    // a single commit is either empty or not a range at all — so the ontology lands first and
    // the instances follow, which is also the order a corpus is actually written in.
    for args in [
        vec!["init", "-q"],
        vec!["config", "user.email", "query@yidam.test"],
        vec!["config", "user.name", "Query"],
        vec!["add", "-A", "--", ".yidam/corpus/*.ont.yml"],
        vec!["commit", "-qm", "chore: genesis — the ontology"],
        vec!["add", "-A"],
        vec!["commit", "-qm", "feat: the instances"],
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

/// A commit sha and a commit date belong to the run, not to the corpus.
///
/// Only a series golden carries either — `stage()` commits the example fresh, so both are new
/// on every run. Applied to every case anyway: no other golden holds a 40-character hex string
/// or an ISO-8601 instant, so a redaction that fired on one would itself be the finding.
fn redact_run(out: &str) -> String {
    let c: Vec<char> = out.chars().collect();
    let mut result = String::new();
    let mut i = 0;
    while i < c.len() {
        let fresh = i == 0 || !c[i - 1].is_ascii_alphanumeric();
        let hex = c[i..].iter().take_while(|x| x.is_ascii_hexdigit()).count();
        if fresh && hex == 40 {
            result.push_str("<SHA>");
            i += 40;
            continue;
        }
        // `%cI` prints `2026-08-25T22:27:21-04:00` — and `2026-08-25T22:27:21Z` where the
        // offset is zero, which is every CI runner and no developer machine. A mask that
        // spelled the offset as digits matched locally and matched nothing on the runner, so
        // the golden went green here and failed there on the one field it exists to redact.
        const INSTANT: &str = "dddd-dd-ddTdd:dd:dd";
        let matches = |mask: &str, at: usize| {
            c.len() - at >= mask.len()
                && mask.chars().zip(&c[at..]).all(|(m, &got)| match m {
                    'd' => got.is_ascii_digit(),
                    other => got == other,
                })
        };
        if fresh && matches(INSTANT, i) {
            let rest = i + INSTANT.len();
            let zone = match c.get(rest) {
                Some('Z') => Some(1),
                Some('+') | Some('-') if matches("dd:dd", rest + 1) => Some(6),
                _ => None,
            };
            if let Some(zone) = zone {
                result.push_str("<DATE>");
                i = rest + zone;
                continue;
            }
        }
        result.push(c[i]);
        i += 1;
    }
    result
}

struct Case {
    /// Golden filename stem, and what the case is for.
    name: &'static str,
    query: &'static str,
    /// Everything after the query, before `--format json`.
    args: &'static [&'static str],
    /// Exit code. 1 only for a rejected query — `query` never gates on the corpus.
    code: i32,
}

/// Every case, with what it is here to pin.
const CASES: &[Case] = &[
    // The entry step alone.
    Case {
        name: "class-only",
        query: "reach",
        args: &[],
        code: 0,
    },
    // One typed hop, and the traversal `neighbors` could not express.
    Case {
        name: "one-hop",
        query: "reach -measured-by-> gage",
        args: &[],
        code: 0,
    },
    // Two hops through two different relationships.
    Case {
        name: "two-hops",
        query: "reach -measured-by-> gage -sources-from-> concept",
        args: &[],
        code: 0,
    },
    // The same edge read from the far end.
    Case {
        name: "backward-hop",
        query: "concept <-exhibits- reach",
        args: &[],
        code: 0,
    },
    // `*` narrowed by a property predicate, with the skipped classes named.
    Case {
        name: "star-filter",
        query: "*[claim_tag=open]",
        args: &[],
        code: 0,
    },
    // The row that shipped wrong in RFC-0018's first draft: `regulated` holds prose, so `=`
    // is correct to match nothing and `~` is the working form. Both are pinned so the pair
    // cannot drift apart.
    //
    // And this is where #283 pays for itself in one line. The empty result now carries
    // `predicate-unsatisfied` with the values `regulated` does hold — which is exactly the
    // fact whose absence made the draft ship wrong in the first place. A reader of the golden
    // is told the shape of the data rather than left to infer it from a zero.
    Case {
        name: "exact-matches-nothing",
        query: "reach[regulated=yes]",
        args: &[],
        code: 0,
    },
    Case {
        name: "contains-matches",
        query: "reach[regulated~yes]",
        args: &[],
        code: 0,
    },
    // #261's acceptance, both halves.
    Case {
        name: "unknown-class",
        query: "gauge",
        args: &[],
        code: 1,
    },
    Case {
        name: "unlicensed-hop",
        query: "reach -measured-by-> concept",
        args: &[],
        code: 1,
    },
    // The near miss: `sourced-from` is authored (into the catalog) and `sources-from` is
    // declared. It runs, returns nothing, and says which is which — in the diagnostic, and
    // now also in `absence`, which reports `no-edge-from-here` rather than blaming the class:
    // the relationship is in use in this corpus, and not from any node that reached step 1.
    Case {
        name: "near-miss-empty",
        query: "gage -sourced-from-> concept",
        args: &[],
        code: 0,
    },
    // #263's mechanism, end to end: enter by meaning, leave by typed edge. The example ships
    // no vector index, so this pins the *degraded* path — which is the one almost every
    // reader of this repository will run, and the one whose cost block has to stay honest.
    // `nodes_read` is 5 of 8 and not 2: a keyword anchor reads every candidate concept to
    // score it, so the narrowing arrives with the index rather than with the syntax.
    Case {
        name: "anchored-entry",
        query: r#"concept~"hydropeaking below a dam" <-exhibits- reach"#,
        args: &[],
        code: 0,
    },
    // An anchor enters, and only the first step is entered. Frozen because the tempting fix
    // is to reinterpret this as a similarity filter over the landing set, which is a
    // different operation wearing the same syntax.
    Case {
        name: "anchor-not-entry",
        query: r#"reach -exhibits-> concept~"low flow""#,
        args: &[],
        code: 1,
    },
    // #262's other report shape, which had no golden at all: a series flattens into the same
    // RFC-0016 envelope at the same `format_version` as everything above it, and `kind` is
    // what tells a consumer which one it is holding. The row carries `changed` and
    // `unschematised` — the two fields a reader of the *count* alone cannot reconstruct.
    Case {
        name: "series",
        query: "reach",
        args: &["--between", "HEAD~1..HEAD"],
        code: 0,
    },
    // And the refusal that belongs to the series rather than to a row. It is taken once,
    // before any tree is reconstructed, and it gates — unlike `unknown-class`, which is the
    // ordinary answer for a class the corpus grew into.
    Case {
        name: "series-rejected",
        query: "reach -->",
        args: &["--between", "HEAD~1..HEAD"],
        code: 1,
    },
];

fn argv<'a>(case: &'a Case, extra: &[&'a str]) -> Vec<&'a str> {
    let mut argv = vec!["query", case.query];
    argv.extend_from_slice(case.args);
    argv.extend_from_slice(extra);
    argv
}

fn run_case(dir: &Path, case: &Case) -> (String, i32) {
    let out = Command::new(env!("CARGO_BIN_EXE_yidam"))
        .args(argv(case, &["--format", "json"]))
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

        let actual = redact_run(&redact(&stdout, dir.path()));
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
            .args(argv(case, &[]))
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
