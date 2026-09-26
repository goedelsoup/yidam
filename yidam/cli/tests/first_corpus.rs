//! `docs/first-corpus-by-hand.md` is a corpus, and it is run as one.
//!
//! The page exists because the only documented route to a corpus of your own was the
//! bootstrap dialogue — 8,642 words of agent prompt, reachable only from a checkout of this
//! template (#951). The route it documents instead is six files in an editor, so the page is
//! not describing a procedure: it **is** the corpus, transcribed. Every byte a reader types
//! is in a fenced block on it.
//!
//! That makes the ordinary documentation failure — prose that was true against a binary two
//! releases ago — checkable in a way an English page never is. This file lifts the six files
//! out of the page, builds the repository the page says to build, and runs the commands the
//! page prints, against the output the page prints beside them.
//!
//! # Discovered, not listed
//!
//! Nothing here names a file of the corpus. [`page_files`] reads whatever the page writes,
//! and [`expected_runs`] reads whatever the page claims those commands answer. A seventh file
//! added to the page is covered on the day it lands, and a command whose output is edited
//! without being re-run fails here rather than on a reader's terminal.
//!
//! Which is also how a scanning gate fails open, so both parsers carry a vacuity guard and
//! [`the_page_is_parsed_as_the_corpus_it_claims_to_be`] is the test to fix first if this file
//! ever goes quiet.
//!
//! # Mutations it was checked against
//!
//! Each was applied to the page by hand and the named test went red:
//!
//! | mutation | caught by |
//! |---|---|
//! | `across 2 classes` edited to `across 4 classes` | `every_command_the_page_prints_answers_the_way_the_page_says` |
//! | `[orphan-out]` renamed in the failure transcript | `deleting_the_links_block_fails_the_gate_the_way_the_page_says` |
//! | the fixture citation repointed elsewhere | `the_page_cites_a_corpus_this_repository_still_maintains` |
//! | a file's bolded path label unbolded | all three, plus `the_page_is_parsed_as_the_corpus_it_claims_to_be` |
//!
//! # Why not `examples/`
//!
//! A worked example is gated by `example_corpus`, and the standard there is the right one for
//! a corpus this repository maintains. This is not one. It is the *smallest* thing that
//! passes, and its value is that a reader can hold all of it at once — an example that grew a
//! seventh node to demonstrate something would stop being the thing the page is for.

use std::path::{Path, PathBuf};
use std::process::Command;

mod common;

use common::git::git;
use common::repo_root;

/// The page under test, repo-relative. Named once; every parser below takes its text.
const PAGE: &str = "docs/first-corpus-by-hand.md";

/// The worked minimum the page cites, and the reason it cites one.
///
/// A page that invented its own corpus shape would drift from the fixture the report goldens
/// are generated from, and neither would know. The page names this directory, so the two rot
/// together: a fixture restructured out from under the page fails
/// [`the_page_cites_a_corpus_this_repository_still_maintains`].
const FIXTURE: &str = "yidam/prelude/sdks/parity/fixtures/reports/basic/repo/.yidam";

fn read(rel: &str) -> String {
    let p = repo_root().join(rel);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{} is unreadable ({e})", p.display()))
}

/// Every file the page tells a reader to write, as (repo-relative path, contents).
///
/// The convention is a bolded path on its own line, then a blank line, then the fence that
/// holds what goes in it. Starlight renders no filename header of its own — this deployment
/// runs plain Shiki, and a `title=` on the info string is dropped without a word — so the
/// label a reader sees and the label this parser reads are necessarily the same one.
///
/// A fence with no path above it is prose — the shell blocks, the failure transcript — and is
/// skipped. The label is what a reader sees, so a file whose label went missing stops being a
/// file here too, which is the correct reading: an unlabelled block is a block nobody knows
/// where to put.
fn page_files(text: &str) -> Vec<(String, String)> {
    let lines: Vec<&str> = text.lines().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let Some(path) = lines[i]
            .strip_prefix("**`")
            .and_then(|r| r.strip_suffix("`**"))
            .filter(|p| p.starts_with(".yidam/"))
        else {
            i += 1;
            continue;
        };
        // The fence is the next non-blank line, or this label introduces no file.
        let mut j = i + 1;
        while j < lines.len() && lines[j].trim().is_empty() {
            j += 1;
        }
        if j >= lines.len() || !lines[j].starts_with("```") {
            i += 1;
            continue;
        }
        let start = j + 1;
        let Some(end) = (start..lines.len()).find(|&k| lines[k].starts_with("```")) else {
            panic!("{PAGE}: the block under `{path}` is never closed");
        };
        out.push((
            path.to_string(),
            format!("{}\n", lines[start..end].join("\n")),
        ));
        i = end + 1;
    }
    out
}

/// Every command the page prints an answer for, as (arguments, the answer it promises).
///
/// A shell block on this page is either a command a reader runs blind — `mkdir`, the commit —
/// or a `yidam` call with its output commented under it. Only the second kind is a claim, and
/// only the second kind is collected: the `# ` lines following a `yidam` line, with the
/// marker stripped.
///
/// The comparison downstream is a prefix rather than an equality, because `status` ends in the
/// genesis date and the page cannot print the date a reader's corpus will be born on. Every
/// other run matches to the last character.
fn expected_runs(text: &str) -> Vec<(Vec<String>, String)> {
    let mut out = Vec::new();
    let mut in_sh = false;
    let mut pending: Option<(Vec<String>, Vec<String>)> = None;
    for line in text.lines() {
        if line.starts_with("```") {
            if in_sh {
                if let Some((args, said)) = pending.take() {
                    out.push((args, said.join("\n")));
                }
            }
            in_sh = line == "```sh";
            continue;
        }
        if !in_sh {
            continue;
        }
        if let Some(said) = line.strip_prefix("# ") {
            if let Some((_, lines)) = pending.as_mut() {
                lines.push(said.to_string());
            }
            continue;
        }
        if let Some((args, said)) = pending.take() {
            if !said.is_empty() {
                out.push((args, said.join("\n")));
            }
        }
        if let Some(rest) = line.strip_prefix("yidam ") {
            pending = Some((
                rest.split_whitespace().map(str::to_string).collect(),
                vec![],
            ));
        }
    }
    if let Some((args, said)) = pending {
        if !said.is_empty() {
            out.push((args, said.join("\n")));
        }
    }
    out
}

/// The corpus the page describes, in a temp repository with a history.
///
/// `git init` and a genesis commit, because `lint` dates `orphan-in` against the history and
/// reads its scope from it — run over a tree with no commits it is measuring something else.
/// The subject is the one the page tells the reader to write, lifted from the page for the
/// same reason as everything else here.
struct Handwritten {
    dir: tempfile::TempDir,
}

impl Handwritten {
    fn materialize(text: &str) -> Self {
        let dir = tempfile::tempdir().unwrap();
        for (rel, body) in page_files(text) {
            let to = dir.path().join(&rel);
            std::fs::create_dir_all(to.parent().unwrap()).unwrap();
            std::fs::write(&to, body).unwrap_or_else(|e| panic!("write {rel}: {e}"));
        }
        let subject = genesis_subject(text);
        for args in [
            vec!["init", "-q"],
            vec!["config", "user.email", "hand@yidam.test"],
            vec!["config", "user.name", "By Hand"],
            vec!["add", "-A"],
            vec!["commit", "-q", "-m", &subject],
        ] {
            git(dir.path(), &args);
        }
        Self { dir }
    }

    fn path(&self) -> &Path {
        self.dir.path()
    }

    fn run(&self, args: &[String]) -> (String, i32) {
        let out = Command::new(env!("CARGO_BIN_EXE_yidam"))
            .current_dir(self.path())
            .args(args)
            .output()
            .unwrap();
        (
            format!(
                "{}{}",
                String::from_utf8_lossy(&out.stdout),
                String::from_utf8_lossy(&out.stderr)
            ),
            out.status.code().unwrap_or(-1),
        )
    }
}

/// The commit subject the page tells the reader to write.
///
/// `export` and `bundle` read the domain name off it, so a page that printed a subject naming
/// no domain would be teaching the one thing `quickstart` §3 warns about.
fn genesis_subject(text: &str) -> String {
    let marker = "git commit -m '";
    let rest = text
        .split_once(marker)
        .unwrap_or_else(|| panic!("{PAGE} no longer prints a commit for the reader to run"))
        .1;
    let subject = rest
        .split_once('\'')
        .unwrap_or_else(|| panic!("{PAGE}'s commit line is unterminated"))
        .0
        .to_string();
    assert!(
        subject.starts_with("genesis: ") && subject.len() > "genesis: ".len(),
        "{PAGE} prints `{subject}` as the first commit. It must be `genesis: <domain>` — the \
         domain name is read off it, and a subject naming none falls back to the directory"
    );
    subject
}

/// Neither parser is reading an empty page.
///
/// Every assertion in this file is quantified over what these two return, so both returning
/// nothing is a green suite that checks a document it never opened. This is the shape that
/// makes a documentation gate worthless, and the only defence is to state the population.
#[test]
fn the_page_is_parsed_as_the_corpus_it_claims_to_be() {
    let text = read(PAGE);
    let files = page_files(&text);
    let paths: Vec<&str> = files.iter().map(|(p, _)| p.as_str()).collect();

    assert!(
        files.len() >= 5,
        "{PAGE} writes {} labelled file(s): {paths:?}. A corpus needs at least one class and \
         one instance of it, and the page promises rather more than that",
        files.len()
    );
    let classes = paths.iter().filter(|p| p.ends_with(".ont.yml")).count();
    let instances = paths
        .iter()
        .filter(|p| p.starts_with(".yidam/corpus/") && !p.ends_with(".ont.yml"))
        .count();
    assert!(
        classes >= 2 && instances >= classes,
        "{PAGE} writes {classes} class(es) and {instances} instance(s): {paths:?}. One class \
         cannot show what an edge between two kinds of thing is, which is the page's subject"
    );
    assert!(
        files.iter().all(|(_, body)| !body.trim().is_empty()),
        "{PAGE} labels a path with an empty block: {paths:?}"
    );

    let runs = expected_runs(&text);
    assert!(
        runs.len() >= 3,
        "{PAGE} prints output for {} command(s). The page's claim is that these commands \
         answer on a hand-written tree, and a page that prints no answers claims nothing",
        runs.len()
    );
}

/// Every command the page prints an answer for answers that way.
///
/// This is the whole point of the file. The page is a transcript, and a transcript is the one
/// kind of documentation that can be re-run — so it is, on every commit, against the binary
/// this tree builds rather than the one it was written against.
#[test]
fn every_command_the_page_prints_answers_the_way_the_page_says() {
    let text = read(PAGE);
    let repo = Handwritten::materialize(&text);

    for (args, said) in expected_runs(&text) {
        let (got, code) = repo.run(&args);
        assert_eq!(
            code,
            0,
            "`yidam {}` exited {code} on the corpus {PAGE} describes, and the page prints it \
             as an ordinary step:\n{got}",
            args.join(" ")
        );
        assert!(
            got.trim_start().starts_with(said.trim()),
            "`yidam {}` no longer answers the way {PAGE} prints it.\n\nthe page says:\n{said}\n\n\
             the binary says:\n{got}",
            args.join(" ")
        );
    }
}

/// The page's broken arm: the gate fails the way the page says it fails.
///
/// A page that only ever shows a passing command teaches that the gate is decoration. This
/// one tells the reader to delete the `links:` block and prints what comes back, so the
/// finding name and the exit code are both claims — and the finding name is the part a lint
/// registry change silently invalidates.
#[test]
fn deleting_the_links_block_fails_the_gate_the_way_the_page_says() {
    let text = read(PAGE);
    let repo = Handwritten::materialize(&text);

    let (_, code) = repo.run(&["graph-check".to_string()]);
    assert_eq!(code, 0, "the page's corpus must start clean");

    // The page names the file and the block. Both are lifted rather than restated: the point
    // of failing is that the reader sees this exact finding, from this exact edit.
    let target = repo.path().join(".yidam/corpus/concept/low-flow.yml");
    let body = std::fs::read_to_string(&target).unwrap();
    let cut = body
        .split_once("links:")
        .unwrap_or_else(|| {
            panic!(
                "{PAGE}'s `low-flow.yml` has no `links:` block, so its broken arm is unreachable"
            )
        })
        .0
        .to_string();
    std::fs::write(&target, &cut).unwrap();

    let (got, code) = repo.run(&["lint".to_string()]);
    assert_ne!(
        code, 0,
        "a node linking to nothing must fail `lint`:\n{got}"
    );

    let finding = expected_finding(&text);
    assert!(
        got.contains(&finding),
        "{PAGE} prints `{finding}` for a node with no outgoing links, and `lint` says:\n{got}"
    );
}

/// The check name the page shows in its failure transcript.
///
/// Read off the page's `text` block rather than written here, so renaming a check in the lint
/// registry fails against the document that quotes it.
fn expected_finding(text: &str) -> String {
    let open = text
        .split_once("\nERROR [")
        .unwrap_or_else(|| panic!("{PAGE} no longer prints a failing transcript"))
        .1;
    let name = open
        .split_once(']')
        .unwrap_or_else(|| panic!("{PAGE}'s failure transcript has an unterminated check name"))
        .0;
    assert!(
        !name.is_empty() && !name.contains(char::is_whitespace),
        "{PAGE} prints `[{name}]` where a check name goes"
    );
    format!("[{name}]")
}

/// The page cites a corpus this repository still maintains, and cites it truthfully.
///
/// #951 asked for the citation so the page and the fixture rot together. A citation nothing
/// checks rots in one direction only — the page keeps claiming a shape the fixture no longer
/// has — so the claim is held here: the directory exists, it is a corpus, and it is the same
/// kind of small the page is.
#[test]
fn the_page_cites_a_corpus_this_repository_still_maintains() {
    let text = read(PAGE);
    assert!(
        text.contains(FIXTURE),
        "{PAGE} no longer cites {FIXTURE}. The page's shape was taken from that fixture, and \
         the citation is what makes the correspondence checkable rather than remembered"
    );

    let fixture = repo_root().join(FIXTURE);
    let corpus = fixture.join("corpus");
    assert!(
        corpus.is_dir(),
        "{PAGE} cites {FIXTURE}, which has no corpus/ — the page is pointing at nothing"
    );

    let (classes, instances) = corpus_shape(&corpus);
    let page = page_files(&text);
    let page_classes = page.iter().filter(|(p, _)| p.ends_with(".ont.yml")).count();
    assert!(
        classes >= page_classes && instances >= 2,
        "the fixture declares {classes} class(es) over {instances} instance(s) and {PAGE} \
         writes {page_classes}. The page calls the fixture its worked minimum, and a minimum \
         smaller than the page is not one"
    );
}

/// (classes, instances) under a `.yidam/corpus/` directory.
fn corpus_shape(corpus: &PathBuf) -> (usize, usize) {
    let mut classes = 0;
    let mut instances = 0;
    for entry in std::fs::read_dir(corpus).into_iter().flatten().flatten() {
        let p = entry.path();
        if p.is_dir() {
            instances += std::fs::read_dir(&p)
                .into_iter()
                .flatten()
                .flatten()
                .filter(|e| e.path().extension().is_some_and(|x| x == "yml"))
                .count();
        } else if p.to_string_lossy().ends_with(".ont.yml") {
            classes += 1;
        }
    }
    (classes, instances)
}
