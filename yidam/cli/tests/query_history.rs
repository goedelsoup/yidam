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

/// The same, at a fixed timestamp on both dates.
///
/// A branchy fixture needs its commits to be *datable* independently of the order they are
/// created in, or the ordering under test is the order the test happened to write them.
fn git_at(dir: &Path, args: &[&str], epoch: u64) {
    // Git's raw date format. Without the `@` it refuses the whole commit.
    let when = format!("@{epoch} +0000");
    let status = Command::new("git")
        .current_dir(dir)
        .args(args)
        .env("GIT_AUTHOR_DATE", &when)
        .env("GIT_COMMITTER_DATE", &when)
        .status()
        .unwrap();
    assert!(status.success(), "git {args:?} failed");
}

/// An empty repository with an identity, ready for a corpus.
fn repo() -> tempfile::TempDir {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    git(dir, &["init", "-q", "-b", "main"]);
    git(dir, &["config", "user.email", "t@t.co"]);
    git(dir, &["config", "user.name", "T"]);
    tmp
}

/// The node ids an answer holds, in order.
fn ids(report: &serde_json::Value) -> Vec<String> {
    report["results"]
        .as_array()
        .unwrap_or(&Vec::new())
        .iter()
        .map(|r| r["node"].as_str().unwrap_or_default().to_string())
        .collect()
}

fn write(dir: &Path, rel: &str, body: &str) {
    let path = dir.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, body).unwrap();
}

/// Three commits, each changing something a query can see.
fn history() -> tempfile::TempDir {
    let tmp = repo();
    let dir = tmp.path();

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
    let tmp = repo();
    let dir = tmp.path();
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

// ── the corpus the two paths must agree about ────────────────────────────────

/// `--at HEAD` on a clean tree is the identity.
///
/// The one property that makes every other assertion here mean something: if the
/// reconstruction and the walk disagree about what the corpus *is*, a series is comparing two
/// answers to two different questions. Three ways they came to differ, one fixture each.
fn agrees_with_the_working_tree(dir: &Path, query: &str) {
    let now = json(dir, &[query]);
    let head = json(dir, &[query, "--at", "HEAD"]);
    assert_eq!(
        ids(&now),
        ids(&head),
        "`--at HEAD` on a clean tree answered differently from the working tree\nnow:  {now}\nhead: {head}"
    );
    assert_eq!(
        now["cost"]["corpus_nodes"], head["cost"]["corpus_nodes"],
        "the two paths disagree about how many nodes the corpus holds"
    );
}

/// A node path git quotes. `ls-tree` without `-z` emits `".yidam/corpus/gage/caf\303\251.yml"`
/// — quotes included — and the node left the corpus at every revision, silently and at exit 0.
#[test]
fn a_node_whose_path_git_quotes_is_still_a_node() {
    let tmp = repo();
    let dir = tmp.path();
    write(dir, ".yidam/corpus/gage.ont.yml", "class: gage\n");
    write(
        dir,
        ".yidam/corpus/gage/canyon.yml",
        "class: gage\nlabel: C\n",
    );
    write(
        dir,
        ".yidam/corpus/gage/café.yml",
        "class: gage\nlabel: Café\n",
    );
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-qm", "chore: genesis — quoted"]);

    agrees_with_the_working_tree(dir, "gage");
    assert_eq!(json(dir, &["gage", "--at", "HEAD"])["matched"], 2);
}

/// A symlink is mode `120000` and type `blob`, so a filter on the type made its *link target*
/// a node — one that exists at every revision and in no working tree, because walkdir does not
/// follow links and `is_file()` excludes them.
#[test]
#[cfg(unix)]
fn a_symlink_in_the_corpus_is_not_a_node() {
    let tmp = repo();
    let dir = tmp.path();
    write(dir, ".yidam/corpus/gage.ont.yml", "class: gage\n");
    write(
        dir,
        ".yidam/corpus/gage/real.yml",
        "class: gage\nlabel: Real\n",
    );
    std::os::unix::fs::symlink("real.yml", dir.join(".yidam/corpus/gage/alias.yml")).unwrap();
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-qm", "chore: genesis — symlink"]);

    agrees_with_the_working_tree(dir, "gage");
    assert_eq!(
        json(dir, &["gage", "--at", "HEAD"])["matched"],
        1,
        "the link target was read as a node"
    );
}

/// The live walk takes instances at depth **two or more**; the historical test took exactly
/// two. So a node one directory further in existed at HEAD, was read by the gate, resolved as
/// a link target — and vanished at every revision.
#[test]
fn a_node_below_its_class_directory_exists_at_every_revision_too() {
    let tmp = repo();
    let dir = tmp.path();
    write(dir, ".yidam/corpus/gage.ont.yml", "class: gage\n");
    write(
        dir,
        ".yidam/corpus/gage/canyon.yml",
        "class: gage\nlabel: C\n",
    );
    write(
        dir,
        ".yidam/corpus/gage/upper/deep.yml",
        "class: gage\nlabel: Deep\n",
    );
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-qm", "chore: genesis — deep"]);

    agrees_with_the_working_tree(dir, "*");
}

// ── what git is asked, and what it does with the answer ──────────────────────

/// A revision beginning with `-` is an *option* to git wherever the argument lands.
///
/// `--at=--output=<path>` reached `git rev-list` as `--output` and truncated the named file to
/// zero bytes — a read-only command destroying a corpus node, and the falsification of both
/// this file's headline property and the module's own docstring. `--between` passed the
/// `contains("..")` guard and did the same at exit 0.
#[test]
fn a_revision_that_looks_like_an_option_is_not_one() {
    let repo = history();
    let dir = repo.path();
    let node = dir.join(".yidam/corpus/gage/canyon.yml");
    let before = std::fs::read_to_string(&node).unwrap();
    assert!(!before.is_empty());

    let target = node.display().to_string();
    for arg in [
        format!("--at=--output={target}"),
        format!("--between=--output={target}.."),
    ] {
        let run = query(dir, &["gage", &arg]);
        assert_ne!(run.code, 0, "`{arg}` was accepted: {}", run.stdout);
        assert_eq!(
            std::fs::read_to_string(&node).unwrap(),
            before,
            "`{arg}` wrote to the working tree"
        );
    }
}

/// A blob git cannot return as text is not an empty node.
///
/// `unwrap_or_default` on the read turned every I/O failure into a well-formed node with no
/// class, no label and no links — a corpus quietly missing whatever the failure covered, at
/// exit 0. On a class blob it is worse: the emptied schema looks bare, the query is rejected
/// as `unknown-class`, and the divergence note then attributes the failure to the ontology's
/// history.
#[test]
fn a_blob_that_cannot_be_read_stops_the_answer_rather_than_shrinking_it() {
    let tmp = repo();
    let dir = tmp.path();
    write(dir, ".yidam/corpus/gage.ont.yml", "class: gage\n");
    write(
        dir,
        ".yidam/corpus/gage/canyon.yml",
        "class: gage\nlabel: C\n",
    );
    // Not text, so `cat-file` returns bytes `read_blobs` cannot decode.
    std::fs::write(
        dir.join(".yidam/corpus/gage/broken.yml"),
        [0xff, 0xfe, 0x00, 0x01],
    )
    .unwrap();
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-qm", "chore: genesis — unreadable"]);

    let run = query(dir, &["gage", "--at", "HEAD", "--format", "json"]);
    let report: serde_json::Value = serde_json::from_str(&run.stdout).unwrap();
    assert_eq!(
        report["rejected"]["code"], "history-unreadable",
        "an unreadable object was answered around: {}",
        run.stdout
    );
    assert_eq!(run.code, 1);
}

/// A branchy history read in date order intermixes its branches, so a series shows nodes
/// appearing, disappearing and returning — changes that never happened, in a report read
/// precisely for where the answer changed.
#[test]
fn a_branchy_history_is_ordered_by_the_graph_and_not_by_the_clock() {
    let tmp = repo();
    let dir = tmp.path();
    write(dir, ".yidam/corpus/gage.ont.yml", "class: gage\n");
    write(dir, ".yidam/corpus/gage/a.yml", "class: gage\nlabel: A\n");
    git(dir, &["add", "-A"]);
    git_at(dir, &["commit", "-qm", "chore: genesis — branchy"], 1_000);

    // A branch adds two nodes, main adds one *between* them by the clock, and the two lines
    // are merged. In date order the rows read b, d, c; in graph order, b, c, d.
    git(dir, &["checkout", "-q", "-b", "x"]);
    write(dir, ".yidam/corpus/gage/b.yml", "class: gage\nlabel: B\n");
    git(dir, &["add", "-A"]);
    git_at(dir, &["commit", "-qm", "feat: b"], 1_100);
    write(dir, ".yidam/corpus/gage/c.yml", "class: gage\nlabel: C\n");
    git(dir, &["add", "-A"]);
    git_at(dir, &["commit", "-qm", "feat: c"], 1_300);

    git(dir, &["checkout", "-q", "main"]);
    write(dir, ".yidam/corpus/gage/d.yml", "class: gage\nlabel: D\n");
    git(dir, &["add", "-A"]);
    git_at(dir, &["commit", "-qm", "feat: d"], 1_200);
    git_at(
        dir,
        &["merge", "-q", "--no-ff", "-m", "chore: merge x", "x"],
        1_400,
    );

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
    let series = json(dir, &["gage", "--between", &format!("{genesis}..HEAD")]);
    let rows: Vec<Vec<String>> = series["series"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| {
            row["results"]
                .as_array()
                .unwrap()
                .iter()
                .map(|r| r["node"].as_str().unwrap().to_string())
                .collect()
        })
        .collect();
    let holding = |node: &str| {
        rows.iter()
            .position(|r| r.iter().any(|id| id == node))
            .unwrap_or_else(|| panic!("{node} is in no row: {series}"))
    };
    assert_eq!(
        holding("gage/c.yml"),
        holding("gage/b.yml") + 1,
        "the branch's two commits were separated by a commit from the other line: {rows:?}"
    );
}

/// `log.showSignature=true` makes git prepend verification lines to every commit, and the
/// parser took the first word of one — `Good` — for a sha and asked for the tree at it.
#[test]
fn a_signature_verifying_configuration_does_not_become_a_revision() {
    let tmp = repo();
    let dir = tmp.path();
    let key = dir.join("signing-key");
    let keygen = Command::new("ssh-keygen")
        .args(["-t", "ed25519", "-N", "", "-C", "t@t.co", "-f"])
        .arg(&key)
        .output();
    // A machine without `ssh-keygen`, or a git too old to sign with SSH, cannot exercise this.
    // Skipping is honest; asserting on the unsigned path would pass against the bug.
    if !keygen.map(|o| o.status.success()).unwrap_or(false) {
        ci_report::skipped("ssh-keygen is unavailable, so no signed commit can be made");
        return;
    }
    git(dir, &["config", "gpg.format", "ssh"]);
    git(
        dir,
        &["config", "user.signingkey", &key.display().to_string()],
    );
    git(dir, &["config", "log.showSignature", "true"]);

    write(dir, ".yidam/corpus/gage.ont.yml", "class: gage\n");
    write(
        dir,
        ".yidam/corpus/gage/canyon.yml",
        "class: gage\nlabel: C\n",
    );
    git(dir, &["add", "-A"]);
    if !Command::new("git")
        .current_dir(dir)
        .args(["commit", "-qS", "-m", "chore: genesis — signed"])
        .status()
        .unwrap()
        .success()
    {
        ci_report::skipped("this git cannot sign with SSH");
        return;
    }
    write(
        dir,
        ".yidam/corpus/gage/valley.yml",
        "class: gage\nlabel: V\n",
    );
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-qS", "-m", "feat: a second gage"]);

    let run = query(dir, &["gage", "--between", "HEAD~1..HEAD"]);
    assert_eq!(run.code, 0, "{}", run.stdout);
    assert!(
        run.stdout.contains("1 commit(s)"),
        "a verification line was read as a commit: {}",
        run.stdout
    );
}

// ── the report envelope ──────────────────────────────────────────────────────

/// A rejection about a past commit says which one — in both formats.
///
/// `rejected_report` hardcoded `at: null`, so a JSON consumer was handed `anchor-at-revision`
/// — a code only reachable when `--at` was supplied — beside the key that means *the working
/// tree*. In text mode the two runs were byte-identical, so a reader who asked about a tag and
/// mistyped a class name was told they had mistyped against today's ontology.
#[test]
fn a_rejected_historical_query_says_which_commit_it_is_about() {
    let repo = history();
    let dir = repo.path();

    let rejected = json(dir, &["gage~\"x\"", "--at", "HEAD~1"]);
    assert_eq!(rejected["rejected"]["code"], "anchor-at-revision");
    assert_eq!(rejected["at"]["rev"], "HEAD~1", "{rejected}");

    let now = query(dir, &["nosuchclass"]);
    let then = query(dir, &["nosuchclass", "--at", "HEAD"]);
    assert_eq!(now.code, 1);
    assert_eq!(then.code, 1);
    assert_ne!(
        now.stdout, then.stdout,
        "a refusal about a commit read exactly like a refusal about now"
    );
}

/// A failure to reconstruct the corpus is still an answer with a shape.
///
/// Returning `Err` printed `Error: …` on stderr with an empty stdout, which a `--format json`
/// consumer cannot distinguish from a crash or a truncated pipe — against this module's own
/// stated rule that a rejection emits its report and *then* exits 1.
#[test]
fn a_bad_revision_comes_back_inside_the_envelope() {
    let repo = history();
    let dir = repo.path();

    for args in [
        vec!["gage", "--at", "no-such-ref"],
        vec!["gage", "--between", "nope..alsonope"],
        vec!["gage", "--between", "HEAD"],
    ] {
        let mut argv = args.clone();
        argv.extend_from_slice(&["--format", "json"]);
        let run = query(dir, &argv);
        let report: serde_json::Value = serde_json::from_str(&run.stdout)
            .unwrap_or_else(|e| panic!("{args:?} emitted no envelope: {e}\n{}", run.stdout));
        assert_eq!(report["format_version"], "1", "{args:?}");
        assert_eq!(report["rejected"]["code"], "history-unreadable", "{args:?}");
        assert_eq!(run.code, 1, "{args:?}");
    }
}

/// A query that is wrong at every commit in a range is wrong, and `--between` returned
/// `Ok(())` for it. The second case is the one that hides: the query is never parsed, so a
/// syntax error printed `0 commit(s) touching the corpus` — what an empty range prints.
#[test]
fn a_malformed_query_is_refused_once_rather_than_per_row() {
    let repo = history();
    let dir = repo.path();

    for text in ["gage -->", "this is (( not valid"] {
        let run = query(dir, &[text, "--between", "HEAD~2..HEAD"]);
        assert_eq!(run.code, 1, "`{text}` exited 0: {}", run.stdout);
        assert!(
            run.stdout.starts_with("rejected (parse)"),
            "`{text}`: {}",
            run.stdout
        );

        let report = json(dir, &[text, "--between", "HEAD~2..HEAD"]);
        assert_eq!(report["kind"], "series");
        assert_eq!(report["rejected"]["code"], "parse");
        assert_eq!(
            report["series"].as_array().unwrap().len(),
            0,
            "the range was reconstructed to emit the same refusal per commit"
        );
    }

    // `unknown-class` is the exception and stays a *row*: a class the corpus grew into is the
    // ordinary case for a series, and exiting 1 for it would make the flag unusable.
    let run = query(dir, &["concept", "--between", "HEAD~2..HEAD"]);
    assert_eq!(run.code, 0, "{}", run.stdout);
}

/// The marker is the point of the report, and it compared the *rendered count*: a commit that
/// deleted one node and added another printed `2 result(s)` on both sides and carried no mark.
#[test]
fn a_row_that_changed_the_answer_without_changing_its_size_is_marked() {
    let tmp = repo();
    let dir = tmp.path();
    write(dir, ".yidam/corpus/gage.ont.yml", "class: gage\n");
    write(
        dir,
        ".yidam/corpus/gage/alpha.yml",
        "class: gage\nlabel: A\n",
    );
    write(
        dir,
        ".yidam/corpus/gage/beta.yml",
        "class: gage\nlabel: B\n",
    );
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-qm", "chore: genesis — swap"]);

    std::fs::remove_file(dir.join(".yidam/corpus/gage/alpha.yml")).unwrap();
    write(
        dir,
        ".yidam/corpus/gage/gamma.yml",
        "class: gage\nlabel: G\n",
    );
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-qm", "feat: alpha out, gamma in"]);

    // A third commit, so the range holds the two rows a comparison needs.
    write(
        dir,
        ".yidam/corpus/gage/delta.yml",
        "class: gage\nlabel: D\n",
    );
    std::fs::remove_file(dir.join(".yidam/corpus/gage/beta.yml")).unwrap();
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-qm", "feat: beta out, delta in"]);

    let series = json(dir, &["gage", "--between", "HEAD~2..HEAD"]);
    let rows = series["series"].as_array().unwrap();
    assert_eq!(rows.len(), 2, "{series}");
    // Same size at every row, different nodes — the case the marker exists for.
    assert_eq!(rows[0]["matched"], 2);
    assert_eq!(rows[1]["matched"], 2);
    assert_eq!(
        rows[0]["changed"], false,
        "the first row has nothing to differ from"
    );
    assert_eq!(
        rows[1]["changed"], true,
        "the commit that swapped one node for another was unmarked: {series}"
    );

    let run = query(dir, &["gage", "--between", "HEAD~2..HEAD"]);
    assert!(run.stdout.contains("← changed"), "{}", run.stdout);
}

/// A series over a class the corpus grew into is a column of `rejected (unknown-class)` with
/// nothing saying the name is a perfectly good class today. The note was computed per row and
/// thrown away by the renderer.
#[test]
fn a_series_prints_the_notes_it_computes() {
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

    let run = query(dir, &["concept", "--between", &format!("{genesis}..HEAD")]);
    assert_eq!(run.code, 0, "{}", run.stdout);
    assert!(
        run.stdout.contains("ontology-moved") || run.stdout.contains("HEAD's ontology"),
        "the rejected row explained nothing: {}",
        run.stdout
    );
}

/// The commit where the ontology *arrived* is the one a reader asks about, and it was the one
/// commit the comparison was silent on: `check` accepts every class name against a corpus with
/// no `.ont.yml`, so both verdicts came back `Ok` with no diagnostics and the answer read as
/// "nothing matched" rather than "nothing was checked".
#[test]
fn the_commit_before_the_ontology_existed_says_nothing_was_checked() {
    let tmp = repo();
    let dir = tmp.path();
    write(dir, "README.md", "# before the corpus\n");
    write(
        dir,
        ".yidam/corpus/gage/canyon.yml",
        "class: gage\nlabel: C\n",
    );
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-qm", "chore: genesis — unschematised"]);

    write(dir, ".yidam/corpus/gage.ont.yml", "class: gage\n");
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-qm", "feat: an ontology"]);

    let then = json(dir, &["gage", "--at", "HEAD~1"]);
    assert_eq!(then["unschematised"], true, "{then}");
    let moved = then["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["code"] == "ontology-moved")
        .unwrap_or_else(|| panic!("no divergence note where the ontology arrived: {then}"));
    assert!(
        moved["message"].as_str().unwrap().contains("no classes"),
        "{moved}"
    );

    // And at HEAD, where both sides are schematised, it stays quiet.
    let now = json(dir, &["gage", "--at", "HEAD"]);
    assert_eq!(now["diagnostics"].as_array().unwrap().len(), 0, "{now}");
}

/// The note says "HEAD", and HEAD is a commit. It compared against `Graph::load` — the working
/// tree — so an **untracked** `.ont.yml` made the note vanish with nothing else in the output
/// changing, and the historical path read the working tree against its one stated property.
#[test]
fn the_divergence_note_is_about_head_and_not_about_the_working_tree() {
    let repo = history();
    let dir = repo.path();

    let before = json(dir, &["concept", "--at", "HEAD~2"]);
    assert_eq!(before["rejected"]["code"], "unknown-class");

    // A class HEAD does not declare, present only on disk.
    write(dir, ".yidam/corpus/legacy.ont.yml", "class: legacy\n");
    let after = json(dir, &["concept", "--at", "HEAD~2"]);
    assert_eq!(
        before["diagnostics"], after["diagnostics"],
        "an uncommitted ontology edit changed a claim about HEAD"
    );
}
