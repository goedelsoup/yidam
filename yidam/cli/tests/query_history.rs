//! `yidam query --at` and `--between` against a real history (#262).
//!
//! An integration test rather than goldens, and rather than unit tests. Goldens would pin
//! commit shas and author dates, which vary per run and would have to be redacted until the
//! fixture pinned almost nothing. Unit tests cannot reach this at all: every assertion here is
//! about what git objects say, and the fixture's whole point is that there is a history.
//!
//! The corpus grows across three commits, so that each of the three things a historical query
//! can do has a commit where it is true and one where it is not:
//!
//! | | genesis | +gage | +concept |
//! |---|---|---|---|
//! | `gage` instances | 1 | 2 | 2 |
//! | `concept` class | absent | absent | present |
//! | `reach -measured-by-> gage` | 1 | 2 | 2 |

use std::path::Path;
use std::process::Command;

fn git(dir: &Path, args: &[&str]) {
    let status = Command::new("git")
        .current_dir(dir)
        .args(args)
        .status()
        .unwrap();
    assert!(status.success(), "git {args:?} failed");
}

fn write(dir: &Path, rel: &str, body: &str) {
    let path = dir.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, body).unwrap();
}

/// Three commits, each changing something a query can see.
fn history() -> tempfile::TempDir {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    git(dir, &["init", "-q", "-b", "main"]);
    git(dir, &["config", "user.email", "t@t.co"]);
    git(dir, &["config", "user.name", "T"]);

    write(
        dir,
        ".yidam/corpus/reach.ont.yml",
        "class: reach\nedges:\n  - relationship: measured-by\n    target: gage\n    \
         direction: out\n",
    );
    write(dir, ".yidam/corpus/gage.ont.yml", "class: gage\n");
    write(
        dir,
        ".yidam/corpus/reach/tailwater.yml",
        "class: reach\nlabel: Tailwater\nlinks:\n  - target: ../gage/canyon.yml\n    \
         relationship: measured-by\n",
    );
    write(
        dir,
        ".yidam/corpus/gage/canyon.yml",
        "class: gage\nlabel: Canyon Outlet\n",
    );
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-qm", "chore: genesis — history"]);

    write(
        dir,
        ".yidam/corpus/gage/valley.yml",
        "class: gage\nlabel: Valley Bridge\n",
    );
    write(
        dir,
        ".yidam/corpus/reach/tailwater.yml",
        "class: reach\nlabel: Tailwater\nlinks:\n  - target: ../gage/canyon.yml\n    \
         relationship: measured-by\n  - target: ../gage/valley.yml\n    relationship: \
         measured-by\n",
    );
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-qm", "feat: a second gage"]);

    write(dir, ".yidam/corpus/concept.ont.yml", "class: concept\n");
    write(
        dir,
        ".yidam/corpus/concept/hydropeaking.yml",
        "class: concept\nlabel: Hydropeaking\n",
    );
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-qm", "feat: a concept class"]);
    tmp
}

struct Run {
    stdout: String,
    code: i32,
}

fn query(dir: &Path, args: &[&str]) -> Run {
    let mut argv = vec!["query"];
    argv.extend_from_slice(args);
    let out = Command::new(env!("CARGO_BIN_EXE_yidam"))
        .args(&argv)
        .current_dir(dir)
        .output()
        .unwrap();
    Run {
        stdout: String::from_utf8_lossy(&out.stdout).to_string(),
        code: out.status.code().unwrap_or(-1),
    }
}

fn json(dir: &Path, args: &[&str]) -> serde_json::Value {
    let mut argv = args.to_vec();
    argv.extend_from_slice(&["--format", "json"]);
    let run = query(dir, &argv);
    serde_json::from_str(&run.stdout).unwrap_or_else(|e| panic!("{e}\n{}", run.stdout))
}

/// The headline: the same query, two commits, two answers.
#[test]
fn a_query_at_a_commit_answers_about_that_commit() {
    let repo = history();
    let dir = repo.path();

    let now = json(dir, &["reach -measured-by-> gage"]);
    assert_eq!(now["matched"], 2);
    assert_eq!(
        now["at"],
        serde_json::Value::Null,
        "no revision was asked for"
    );

    let then = json(dir, &["reach -measured-by-> gage", "--at", "HEAD~2"]);
    assert_eq!(then["matched"], 1);
    assert_eq!(then["results"][0]["node"], "gage/canyon.yml");
    assert!(then["at"]["commit"].as_str().unwrap().len() == 40);
    assert_eq!(then["at"]["rev"], "HEAD~2");
    // The cost block is about that commit too, or a series would compare answers against a
    // corpus size that was never true at the same time.
    assert_eq!(then["cost"]["corpus_nodes"], 2);
}

/// The property #262 names, tested the only way that can fail: make the working tree
/// disagree with every commit, then ask about a commit.
///
/// A checkout-based implementation passes every other test in this file and fails this one —
/// and would fail it by *destroying the user's edit*, which is why the assertion is on the
/// file's bytes and not only on the answer.
#[test]
fn a_historical_query_neither_reads_nor_writes_the_working_tree() {
    let repo = history();
    let dir = repo.path();

    let node = dir.join(".yidam/corpus/gage/canyon.yml");
    let edited = "class: gage\nlabel: UNCOMMITTED EDIT\n";
    std::fs::write(&node, edited).unwrap();

    // `--select body` is the one projection that used to reach for the file on disk.
    let then = json(dir, &["gage", "--select", "node,body", "--at", "HEAD~2"]);
    let body = then["results"][0]["body"].as_str().unwrap();
    assert!(
        body.contains("Canyon Outlet") && !body.contains("UNCOMMITTED EDIT"),
        "the historical body came from the working tree: {body}"
    );

    assert_eq!(
        std::fs::read_to_string(&node).unwrap(),
        edited,
        "the working tree was modified by a read-only query"
    );
}

/// A class that did not exist yet is an `unknown-class` rejection — the historical ontology
/// decides, because it is the schema that commit's data obeys — and the report says the
/// verdict would differ today rather than leaving the reader to think they mistyped.
#[test]
fn an_ontology_that_has_since_grown_is_named_rather_than_silently_picked() {
    let repo = history();
    let dir = repo.path();

    let then = json(dir, &["concept", "--at", "HEAD~2"]);
    assert_eq!(then["rejected"]["code"], "unknown-class");
    let moved = &then["diagnostics"][0];
    assert_eq!(moved["code"], "ontology-moved");
    assert_eq!(moved["level"], "info");
    assert!(
        moved["message"].as_str().unwrap().contains("HEAD"),
        "{moved}"
    );

    // And the text path says it too. It returned early on the rejection and swallowed this.
    let run = query(dir, &["concept", "--at", "HEAD~2"]);
    assert_eq!(run.code, 1);
    assert!(run.stdout.contains("ontology-moved") || run.stdout.contains("HEAD's ontology"));
}

/// The same query at HEAD, where nothing has moved, must not invent a divergence.
#[test]
fn an_unchanged_ontology_produces_no_note() {
    let repo = history();
    let then = json(repo.path(), &["gage", "--at", "HEAD"]);
    assert_eq!(then["diagnostics"].as_array().unwrap().len(), 0);
    assert_eq!(then["matched"], 2);
}

/// One row per commit that touched the corpus, oldest first, and the row where the answer
/// changed is the one the reader is looking for.
#[test]
fn a_series_has_a_row_per_commit_that_touched_the_corpus() {
    let repo = history();
    let dir = repo.path();
    let genesis = String::from_utf8_lossy(
        &Command::new("git")
            .current_dir(dir)
            .args(["rev-list", "--max-parents=0", "HEAD"])
            .output()
            .unwrap()
            .stdout,
    )
    .trim()
    .to_string();

    let series = json(dir, &["concept", "--between", &format!("{genesis}..HEAD")]);
    let rows = series["series"].as_array().unwrap();
    // `a..b` excludes `a`, as it does everywhere else in git: two rows, not three.
    assert_eq!(rows.len(), 2, "{series}");
    assert_eq!(rows[0]["rejected"]["code"], "unknown-class");
    assert_eq!(rows[1]["rejected"], serde_json::Value::Null);
    assert_eq!(rows[1]["matched"], 1);
    assert_eq!(series["range"], format!("{genesis}..HEAD"));

    let run = query(dir, &["concept", "--between", &format!("{genesis}..HEAD")]);
    assert!(run.stdout.contains("← changed"), "{}", run.stdout);
    // A series does not gate: a rejected row is the ordinary case for a class the corpus
    // grew into, and exiting 1 would make the flag unusable on exactly those queries.
    assert_eq!(run.code, 0, "{}", run.stdout);
}

/// A commit that changed nothing under `.yidam/corpus` gets no row. A series is read by
/// scanning for the commit where the answer moved, and rows that cannot have moved it are
/// what makes that scan fail.
#[test]
fn a_commit_that_did_not_touch_the_corpus_is_not_a_row() {
    let repo = history();
    let dir = repo.path();
    std::fs::write(dir.join("README.md"), "# not the corpus\n").unwrap();
    git(dir, &["add", "-A"]);
    git(
        dir,
        &["commit", "-qm", "docs: a commit the corpus cannot see"],
    );

    let series = json(dir, &["gage", "--between", "HEAD~2..HEAD"]);
    assert_eq!(series["series"].as_array().unwrap().len(), 1);
}

/// The index is built from one commit's text. Anchoring as of another would enter through
/// today's embeddings and walk that commit's edges, and degrading to keyword search would be
/// a different retrieval than the same query gets at HEAD.
#[test]
fn a_similarity_anchor_cannot_be_resolved_as_of_a_past_commit() {
    let repo = history();
    let run = query(repo.path(), &["reach~\"below the dam\"", "--at", "HEAD~1"]);
    assert_eq!(run.code, 1);
    assert!(run.stdout.contains("anchor-at-revision"), "{}", run.stdout);
}

/// A revision that is not one, and a range that is not one. Both name what was wrong with
/// the input rather than reporting an empty corpus.
#[test]
fn a_bad_revision_says_so_rather_than_answering_about_nothing() {
    let repo = history();
    let dir = repo.path();

    let run = query(dir, &["gage", "--at", "no-such-ref"]);
    assert_ne!(run.code, 0);

    let run = query(dir, &["gage", "--between", "HEAD"]);
    assert_ne!(run.code, 0);
}

/// `--at` and `--between` are two different questions and clap refuses both at once —
/// exit 2, the pre-dispatch usage code `binary_pin.rs` freezes.
#[test]
fn at_and_between_are_mutually_exclusive() {
    let repo = history();
    let run = query(
        repo.path(),
        &["gage", "--at", "HEAD", "--between", "HEAD~1..HEAD"],
    );
    assert_eq!(run.code, 2, "{}", run.stdout);
}

/// A historical answer that is empty says why, and does **not** point at today's packages.
///
/// The diagnosis itself travels to a past commit intact: it is read off the ontology
/// reconstructed at that commit rather than off the working tree. What must not travel is the
/// offer to look next door — **a dependency set has no history.** What this repository holds
/// is whatever bundle is unpacked now, so naming it to explain a corpus from a year ago would
/// be an anachronism dressed as a lead. It is the same argument that refuses `--across` and
/// `--at` together, one field further in.
///
/// Its own history rather than [`history`]'s: this needs a commit where a class is declared
/// and empty, and adding one to the shared fixture would change what every other test in this
/// file is looking at.
#[test]
fn an_empty_historical_answer_is_diagnosed_and_offered_no_dependency() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    git(dir, &["init", "-q", "-b", "main"]);
    git(dir, &["config", "user.email", "t@t.co"]);
    git(dir, &["config", "user.name", "T"]);
    // Declared and empty, at every commit there is.
    write(dir, ".yidam/corpus/note.ont.yml", "class: note\n");
    write(dir, ".yidam/corpus/gage.ont.yml", "class: gage\n");
    write(
        dir,
        ".yidam/corpus/gage/canyon.yml",
        "class: gage\nlabel: Canyon\n",
    );
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-qm", "chore: genesis — absence"]);

    // An installed package holding exactly what the query asks for. On a present-tense run
    // this is what `elsewhere` names — the first assertion below is what makes the second
    // one mean something.
    write(
        dir,
        ".yidam/tonpa/upstream/manifest.yml",
        "commit: \"abc1234\"\n",
    );
    write(
        dir,
        ".yidam/tonpa/upstream/corpus/note.ont.yml",
        "class: note\n",
    );
    write(
        dir,
        ".yidam/tonpa/upstream/corpus/note/upstream-note.yml",
        "class: note\nlabel: Upstream note\n",
    );

    let now = json(dir, &["note"]);
    assert_eq!(now["matched"], 0);
    assert_eq!(now["absence"]["code"], "class-unpopulated");
    assert_eq!(now["absence"]["elsewhere"], serde_json::json!(["upstream"]));

    let past = json(dir, &["note", "--at", "HEAD"]);
    assert_eq!(past["matched"], 0);
    assert_eq!(
        past["absence"]["code"], "class-unpopulated",
        "a historical empty answer is still an empty answer: {past}"
    );
    assert_eq!(
        past["absence"]["elsewhere"],
        serde_json::json!([]),
        "a dependency set has no history, so it cannot explain a past commit's silence"
    );
    assert!(
        !past["absence"]["message"]
            .as_str()
            .unwrap()
            .contains("--across"),
        "{past}"
    );
}
