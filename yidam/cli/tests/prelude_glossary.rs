//! `prelude/GLOSSARY.md` — read zero, and the six ways it rots (#933).
//!
//! # What was measured
//!
//! The minimum path through a fresh clone before an agent writes its first node is 14,670
//! words: `.claude/CLAUDE.md` and `BOOTSTRAP.md` (323), `skills/bootstrap.md` (8,642),
//! `guidelines/agent-conduct.md` (4,723), `CONSTITUTION.md` (982). Not one of them defines
//! *rigpa*, *ma*, *samudaya*, *sadhana*, *kuten*, *tonpa* or *sangha*. The file that does is
//! `SCRIPTURE.md`, and step 1 forbids reading it by name — so the agent scaffolded a
//! `rigpa/`-aware repository having never been told what rigpa meant, and `tonpa` appeared
//! in the mandatory set exactly twice, both times as a path component.
//!
//! The glossary is the cheap half of that finding: 306 words that make the other 14,000
//! parse. The expensive half — splitting the normative sentences in `GRAPH.md`,
//! `directories.md` and `agent-conduct.md` from the essays that justify them — is #954, and
//! is not what this file gates.
//!
//! # Why these six
//!
//! Nothing here judges whether a gloss is *good*; that is a reading, and a test cannot take
//! one. What a test can hold is the six mechanical ways a glossary stops working:
//!
//! - it stops being **read zero** — step 1 lists it late, or lists it and miscounts itself;
//! - it stops being **complete** — a branch namespace `GRAPH.md` declares has no entry;
//! - it stops being **live** — an entry glosses a word the read set no longer uses;
//! - it stops being **short** — which is the whole of why it was allowed into step 1;
//! - it stops being **true** — a row names a path that does not exist, which read zero gets
//!   acted on before anything can contradict it;
//! - it stops **agreeing with `docs/vocabulary.md`**, which glossed six of these words first
//!   and in a document no agent reads.
//!
//! The count above is stated, which is the thing `step_one_lists_the_files_it_promises` exists
//! to catch one file over. It said *four* over six until somebody counted.
//!
//! Each population is discovered rather than listed: the read set is parsed out of step 1,
//! the namespaces out of `GRAPH.md`'s encoding table, the reading routes out of every tracked
//! document that sends an agent to `IDENTITY.md` and `GRAPH.md` together. That last rule is
//! not decoration — written as a list of the two `AGENTS.md` files, it would have missed
//! `yidam/README.md`, which calls the prelude "a curriculum. An agent reads it in order", and
//! `sadhana/root/README.md`, which is the first thing a derived repository shows anybody.

use std::collections::BTreeSet;

mod common;

use common::{repo_root, step_one_read_list, tracked_under, BOOTSTRAP_SKILL as SKILL};

const GLOSSARY: &str = "yidam/prelude/GLOSSARY.md";
const GRAPH: &str = "yidam/prelude/GRAPH.md";

fn read(rel: &str) -> String {
    let p = repo_root().join(rel);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{} is unreadable ({e})", p.display()))
}

/// The terms the glossary defines: the bolded cell opening each table row.
///
/// A trailing `/` marks a branch prefix and is part of how the file writes the term, not part
/// of the word — `**ma/**` is the entry for `ma`.
fn glossed_terms() -> BTreeSet<String> {
    read(GLOSSARY)
        .lines()
        .filter_map(|l| {
            let rest = l.trim().strip_prefix("| **")?;
            Some(rest.split("**").next()?.trim_end_matches('/').to_string())
        })
        .collect()
}

/// Step 1 states how many files it lists, and then lists them.
///
/// The stated count is the half a reader trusts and stops checking, and
/// `bootstrap_skill.rs`'s `step_nine_lists_the_sections_it_promises` documents the specific way it
/// goes wrong: a count incremented when an item is added carries the old error forward.
/// Step 1 had no such gate for as long as it said "six", and adding a seventh file is exactly
/// the edit that would have broken it silently.
#[test]
fn step_one_lists_the_files_it_promises() {
    let text = read(SKILL);
    let (_, after) = text
        .split_once("Read these ")
        .expect("step 1's read preamble");
    let stated_word = after.split_whitespace().next().unwrap_or("");
    let stated = match stated_word {
        "four" => 4,
        "five" => 5,
        "six" => 6,
        "seven" => 7,
        "eight" => 8,
        "nine" => 9,
        "ten" => 10,
        other => panic!("step 1 promises {other:?} files and that is not a number this test knows"),
    };

    let listed = step_one_read_list();
    assert_eq!(
        stated,
        listed.len(),
        "step 1 promises {stated} files and numbers {}: {listed:?}",
        listed.len()
    );

    // The count is written twice more in the same step, and a fix that updates one is the
    // fix that was applied to step 9.
    let step_one = after.split("\n### ").next().unwrap_or(after);
    for phrase in [
        format!("**only** these {stated_word} files"),
        format!("in these {stated_word} files"),
        format!("After reading all {stated_word}"),
    ] {
        assert!(
            step_one.contains(&phrase),
            "step 1 says {stated_word:?} once and then disagrees with itself: {phrase:?} is absent"
        );
    }

    for p in &listed {
        assert!(
            repo_root().join(p).is_file(),
            "step 1 sends the agent to {p}, which does not exist"
        );
    }
}

/// The glossary is first, or it is not read zero.
///
/// A glossary placed after the documents that need it is a document nobody reaches in time —
/// the agent has already parsed `rigpa/` as noise by then, which is the state #933 measured.
#[test]
fn the_glossary_is_read_zero() {
    let listed = step_one_read_list();
    assert_eq!(
        listed.first().map(String::as_str),
        Some(GLOSSARY),
        "step 1's first read is {:?}; the glossary has to be first or it is not read zero",
        listed.first()
    );
}

/// Every branch namespace `GRAPH.md`'s encoding table declares has an entry.
///
/// Discovered from the table's `refs/heads/<ns>/` rows, which is the closed set an agent meets
/// as a ref name and cannot look up anywhere else. `phase/` and `propose/` are English and are
/// glossed anyway: the exemption a "borrowed words only" rule would need is a judgement, and a
/// namespace with no entry is the failure whichever language it came from.
#[test]
fn every_branch_namespace_is_glossed() {
    let graph = read(GRAPH);
    let declared: BTreeSet<String> = graph
        .lines()
        .filter_map(|l| {
            let (_, rest) = l.split_once("`refs/heads/")?;
            let ns = rest.split('/').next()?;
            (!ns.is_empty() && !ns.contains('`')).then(|| ns.to_string())
        })
        .collect();

    assert!(
        declared.len() >= 4,
        "GRAPH.md's encoding table declares {} branch namespaces; the scan broke rather than \
         the table emptying: {declared:?}",
        declared.len()
    );

    let glossed = glossed_terms();
    let missing: Vec<&String> = declared.difference(&glossed).collect();
    assert!(
        missing.is_empty(),
        "GRAPH.md declares {missing:?} as a branch namespace and GLOSSARY.md does not define it. \
         An agent meets these as ref names with nothing to look them up in."
    );
}

/// No entry glosses a word the rest of the read set has stopped using.
///
/// The glossary's cost is paid at read zero by every bootstrapping agent, so an entry has to
/// earn it. Checked against step 1's own list — the files the glossary exists to make
/// parseable — rather than the whole repository, where anything would match something.
#[test]
fn no_glossary_entry_is_dead() {
    let corpus: String = step_one_read_list()
        .iter()
        .filter(|p| p.as_str() != GLOSSARY)
        .map(|p| read(p))
        .collect::<Vec<_>>()
        .join("\n");

    let terms = glossed_terms();
    assert!(
        terms.len() >= 8,
        "GLOSSARY.md defines {} terms; the row scan broke rather than the file emptying",
        terms.len()
    );

    let unused: Vec<&String> = terms
        .iter()
        .filter(|t| !contains_word(&corpus, t))
        .collect();
    assert!(
        unused.is_empty(),
        "GLOSSARY.md defines {unused:?}, which step 1's other reads never use. Read zero is \
         charged to every bootstrap run; an entry nobody meets is weight for nothing."
    );
}

/// `needle` occurs in `haystack` as a whole word.
///
/// Word-bounded because the shortest term is `ma`, and a substring match finds it in "may",
/// "make" and "format" — which would pass this test for a term the read set had dropped
/// entirely. A boundary is any non-alphanumeric byte, so `ma/` and `` `ma` `` both count and
/// `formatted` does not.
fn contains_word(haystack: &str, needle: &str) -> bool {
    let bytes = haystack.as_bytes();
    let n = needle.len();
    haystack.match_indices(needle).any(|(i, _)| {
        let before_ok = i == 0 || !bytes[i - 1].is_ascii_alphanumeric();
        let after_ok = i + n >= bytes.len() || !bytes[i + n].is_ascii_alphanumeric();
        before_ok && after_ok
    })
}

/// The glossary stays short, because short is the whole argument for letting it into step 1.
///
/// A ratchet, not a measurement. #933 asked for ~200 words; twelve rows carrying both an
/// etymology and a path came to 334, and the ceiling is 360 — enough headroom to fix a gloss
/// without renegotiating the budget, not enough to add a section. Lower it when the file gets
/// shorter. The floor is here for the reason every other population above has one: a file
/// emptied by a bad edit would otherwise pass.
///
/// 360 is the number because `the_glossary_points_at_the_argument` needs `SCRIPTURE.md` to stay
/// at least twice this file's length, and scripture is 990 words. The two ceilings are the same
/// decision from opposite ends.
#[test]
fn the_glossary_stays_short() {
    const CEILING: usize = 360;
    let words = read(GLOSSARY).split_whitespace().count();
    assert!(
        (150..=CEILING).contains(&words),
        "GLOSSARY.md is {words} words and the budget is 150..={CEILING}. Over the ceiling, it \
         has become the thing it was written to replace — lower the ceiling on purpose or cut \
         a row, but do not raise it to fit."
    );
}

/// Every document that routes an agent through the prelude routes it through the glossary.
///
/// A reading route is discovered, not listed: any tracked markdown that links `IDENTITY.md`
/// and `GRAPH.md` together is sending somebody through the prelude in order. Four documents
/// match, and the two a hand-written list would have named are the two `AGENTS.md` files — so
/// the list would have shipped missing `yidam/README.md` and `sadhana/root/README.md`, which
/// are the curriculum and the derived repository's front door.
#[test]
fn every_reading_route_names_the_glossary() {
    let root = repo_root();
    let routes: Vec<(String, String)> = tracked_under(&root, "*.md")
        .into_iter()
        .filter_map(|rel| {
            let text = std::fs::read_to_string(root.join(&rel)).ok()?;
            let is_route =
                text.contains("prelude/IDENTITY.md)") && text.contains("prelude/GRAPH.md)");
            is_route.then_some((rel, text))
        })
        .collect();

    assert!(
        routes.len() >= 4,
        "{} prelude reading routes found; the discovery rule broke rather than the routes \
         disappearing",
        routes.len()
    );

    let silent: Vec<&String> = routes
        .iter()
        .filter(|(_, text)| !text.contains("GLOSSARY.md)"))
        .map(|(rel, _)| rel)
        .collect();
    assert!(
        silent.is_empty(),
        "{silent:?} send an agent through IDENTITY.md and GRAPH.md and never mention \
         GLOSSARY.md. Every route that teaches the model has to hand over the vocabulary first."
    );
}

/// `SCRIPTURE.md` is still the long form, and the glossary still points at it.
///
/// The glossary is not a replacement for scripture and must not become one — it carries the
/// glosses and scripture carries the argument. The forward link is what keeps the argument
/// reachable from inside step 1's read set, which is the only place a bootstrapping agent
/// looks, and step 1 forbids opening scripture directly.
#[test]
fn the_glossary_points_at_the_argument() {
    let g = read(GLOSSARY);
    assert!(
        g.contains("(SCRIPTURE.md)"),
        "GLOSSARY.md does not link SCRIPTURE.md. Step 1 forbids reading scripture, so the \
         glossary is the only place the argument for these words stays reachable from."
    );
    let scripture = read("yidam/prelude/SCRIPTURE.md")
        .split_whitespace()
        .count();
    let glossary = g.split_whitespace().count();
    assert!(
        scripture > glossary * 2,
        "SCRIPTURE.md is {scripture} words and GLOSSARY.md {glossary}. The glossary has grown \
         into the document it was written to stand in for."
    );
}

/// Read zero has to survive the vendor step, or it is read zero in this repository only.
///
/// `git ls-files` rather than a directory walk, for the reason `tracked_under` documents: the
/// vendor step copies out of a clone, so an untracked `GLOSSARY.md` reaches no derived
/// repository at all while every test here that reads the working tree goes green.
#[test]
fn the_glossary_is_tracked_and_therefore_vendored() {
    let tracked = tracked_under(&repo_root(), "yidam/prelude");
    assert!(
        tracked.iter().any(|p| p == GLOSSARY),
        "{GLOSSARY} is not tracked by git. The vendor step reads a clone, so it would install \
         into no derived repository while step 1 sends every agent to it."
    );
}

/// The two glossaries agree on which words are borrowed.
///
/// `docs/vocabulary.md` already glossed *rigpa*, *ma*, *samudaya*, *sadhana*, *sangha* and
/// *prelude* — and glossed none of them anywhere an agent reads, because `docs/` is not
/// vendored and step 1 never opens it. So read zero is a second list of the same words, which
/// is the shape that drifts: `kuten` and `tonpa` were absent from the docs page at the moment
/// this test was written, both of them terms the CLI has shipped a command for.
///
/// One direction only. The docs page is 878 words and carries the ontology, the gates and the
/// domain computer; read zero carries the borrowed words and has a budget. Everything the
/// prelude glosses has to be in the docs page, and the docs page may say more.
#[test]
fn the_docs_vocabulary_covers_every_borrowed_word() {
    let docs = read("docs/vocabulary.md");
    let terms = glossed_terms();

    let absent: Vec<&String> = terms
        .iter()
        .filter(|t| !docs.contains(&format!("| **{t}**")))
        .collect();
    assert!(
        absent.is_empty(),
        "GLOSSARY.md defines {absent:?} and docs/vocabulary.md has no row for them. Two lists          of the same words drift; the docs page is the public one and has to be the superset."
    );
}

/// No row invents a path.
///
/// A gloss is worth having because it binds a word to a place, which makes the place the part
/// that rots — and a wrong path in read zero is worse than no gloss, because the agent acts on
/// it. This shipped asserting `.yidam/kuten/`, a directory that has never existed in any
/// derived repository: a kuten is a profile under `.yidam/.vendor/prelude/kuten/`, selected in
/// `.yidam/decisions/kuten.yml`. Nothing else in the repository named it, which is exactly the
/// signal here.
///
/// The corroborating set is every tracked document under `yidam/prelude/` and `sadhana/` — the
/// prelude and the scaffold, which between them are what a derived repository has. Glossing a
/// path no other inherited document mentions is either a typo or a claim the rest of the
/// template does not make. `docs/` is deliberately out: it is not vendored, so a path attested
/// only there is attested nowhere the agent can reach.
#[test]
fn no_glossary_row_invents_a_path() {
    let root = repo_root();
    let corroborating: String = ["yidam/prelude", "sadhana"]
        .iter()
        .flat_map(|prefix| tracked_under(&root, prefix))
        .filter(|rel| rel.ends_with(".md") && rel != GLOSSARY)
        .filter_map(|rel| std::fs::read_to_string(root.join(&rel)).ok())
        .collect();

    // Backticked spans that look like a path into the layout: a `.yidam/…` span, or a
    // top-level directory written with a trailing slash.
    let paths: BTreeSet<String> = read(GLOSSARY)
        .split('`')
        .skip(1)
        .step_by(2)
        .filter(|sp| sp.starts_with(".yidam/") || (sp.ends_with('/') && !sp.contains(' ')))
        .map(str::to_string)
        .collect();

    assert!(
        paths.len() >= 5,
        "GLOSSARY.md names {} paths; the span scan broke rather than the rows losing them: \
         {paths:?}",
        paths.len()
    );

    let unattested: Vec<&String> = paths
        .iter()
        .filter(|p| !corroborating.contains(p.as_str()))
        .collect();
    assert!(
        unattested.is_empty(),
        "GLOSSARY.md sends an agent to {unattested:?}, which no other inherited document names. \
         Read zero is acted on before anything else, so a path invented here is acted on first."
    );
}
