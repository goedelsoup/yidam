//! Documents that say what a run may declare, against what a run actually declares (#873).
//!
//! For the first slice of `yidam run`, RFC-0026 §2's second sentence was held by a refusal: a
//! capability declaring an epistemic verb did not load. `d8161ad` replaced that with a
//! destination — every verb loads, an operational one advances the branch, an epistemic one
//! lands on `propose/<head>` — and touched `manifest.rs` and `mod.rs` and no prose.
//!
//! **Three documents went on describing the mechanism that was gone.** The vendored
//! `guidelines/directories.md`, which a derived repository cannot edit and reads to decide what
//! a capability layer could do; `docs/cli-reference.md`; and the `run` doc comment in `main.rs`,
//! which ships as `yidam run --help`. A derived repository concluded from the first that the
//! epistemic half of the vocabulary was undeclarable for a run, and found otherwise only by
//! reading `manifest.rs` — which it has no copy of. The prose reversed the affordance: `yidam
//! run` **can** open a question, safely, and the document that reaches every derivation said it
//! could not.
//!
//! This is the third restatement of the run/verb rule, which is the count `yidam/cli/Cargo.toml`
//! reached about the light build before `light_build.rs` stopped accepting a fourth.
//!
//! # The verdict is probed, not written down here
//!
//! A test asserting "no document may say an epistemic verb is refused" would be a fourth
//! restatement — right today, and wrong on the day somebody restores a refusal for a good
//! reason. So [`the_binary_loads_an_epistemic_verb_and_routes_it_to_a_proposal`] asks the real
//! binary what happens, and the prose scans are held to *that* answer. If the behaviour ever
//! flips back, this file inverts with it instead of needing an edit.
//!
//! # It reads the assertive form only
//!
//! `a capability declaring an epistemic verb does not load` is a claim about the binary.
//! `manifest.rs`'s own *"for as long as only the first half existed, the second was enforced by
//! refusing the verb"*, and RFC-0026's past-tense account of the same, are history being
//! recorded, and a guard that could not tell those apart would forbid writing down why the
//! design changed — which four sites in this repository do, deliberately. So the pattern is a
//! **finite present-tense** predicate of refusal with `epistemic` in the same sentence, and
//! nothing wider; [`asserts_a_refusal`] has the tense argument. A document that invents some
//! new way to assert the refusal will slip through; the sentence this exists for is the one
//! that was actually written, three times, and
//! [`the_predicate_catches_the_claim_and_spares_the_history`] keeps all three in the
//! population. That self-test is also why [`DEFINITION_SITE`] is the one path the scan skips:
//! this file quotes the claim in order to match it, and a scan that read its own definition
//! could never pass.

mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

use common::{capability_kinds, examples, kind_spellings, repo_root, Example};

// ── what the binary actually does ─────────────────────────────────────────────

/// What a run does with a capability declaring an epistemic verb, asked of the real binary.
#[derive(Debug, PartialEq, Eq)]
enum Observed {
    /// It loads, it runs, and the commit lands somewhere that is not the invoking branch.
    RoutedToProposal,
    /// It is refused before anything runs — the holding action `d8161ad` replaced.
    Refused,
}

/// One epistemic verb, put through `yidam run` in a real corpus.
///
/// `capability_run.rs` walks the whole family and holds the invariant. This wants one fact —
/// does a declaration load — so it takes the first example that declares a capability and the
/// first epistemic verb, and reads the exit code.
fn observe() -> (Observed, String) {
    let verb = yidam_core::git::EPISTEMIC_VERBS
        .first()
        .expect("the epistemic family is non-empty");

    // A capability nothing else declares itself `after`. The manifest refuses a step that
    // waits for an epistemic one — what a proposal branch holds is not in the tree a dependent
    // would read — so mutating an upstream would make this probe observe `Refused` for a
    // reason that is the dependency rule rather than the route.
    let Some((example, step)) = examples().into_iter().find_map(|name| {
        let e = Example::materialize(&name);
        let text = std::fs::read_to_string(e.path().join(".yidam/capabilities.toml")).ok()?;
        let m: toml::Value = toml::from_str(&text).ok()?;
        let caps = m.get("capability")?.as_table()?;
        let waited_for: Vec<&str> = caps
            .values()
            .filter_map(|c| c.get("after")?.as_array())
            .flat_map(|a| a.iter().filter_map(toml::Value::as_str))
            .collect();
        let step = caps
            .keys()
            .find(|k| !waited_for.contains(&k.as_str()))?
            .clone();
        Some((name, step))
    }) else {
        panic!("no example declares a terminal capability, so this file asserts nothing");
    };

    let e = Example::materialize(&example);
    let manifest = e.path().join(".yidam/capabilities.toml");
    let before = std::fs::read_to_string(&manifest).unwrap();

    // That step's own table, and nothing past it. A `replace` over the file would move every
    // capability's verb at once, which is the pair the manifest refuses.
    let head = format!("[capability.{step}]");
    let at = before.find(&head).expect("the step is declared");
    let rest = &before[at + head.len()..];
    let end = at + head.len() + rest.find("\n[").unwrap_or(rest.len());
    let table = before[at..end].replace("verb   = \"compute\"", &format!("verb   = \"{verb}\""));
    let after = format!("{}{table}{}", &before[..at], &before[end..]);
    assert_ne!(
        before,
        after,
        "the manifest at {} was not mutated, so the probe below declares `compute` and \
         observes nothing about the epistemic family",
        manifest.display()
    );
    std::fs::write(&manifest, after).unwrap();

    let head = |dir: &Path| common::git::out(dir, &["rev-parse", "HEAD"]);
    // Settle the corpus under the edited manifest before measuring anything.
    //
    // The edit changes the manifest digest, which is in every capability's input state, so it
    // makes every step stale — including the operational ones this probe is not about. Left
    // unsettled, the measured run would bring a dependency up to date and advance the branch,
    // and the guard below would report the invariant broken by a commit that is exactly what
    // the invariant permits.
    //
    // A refusal at load stops this run too, which is the answer this probe is asking for.
    let (out, err, code) = e.run(&["run"]);
    if code != 0 {
        return (Observed::Refused, format!("{out}{err}"));
    }
    // Settling ran the epistemic step as well and its result is committed on a proposal
    // branch, so the next run would find it fresh. Deleting that branch is what a person does
    // to reject a proposal, and it is what leaves the step owed for the run below.
    let refs = common::git::out(
        &e.path(),
        &["for-each-ref", "--format=%(refname)", "refs/heads/propose/"],
    );
    for name in refs.lines().filter(|l| !l.trim().is_empty()) {
        common::git::git(&e.path(), &["update-ref", "-d", name]);
    }

    let start = head(&e.path());
    let (out, err, code) = e.run(&["run", &step]);
    let transcript = format!("{out}{err}");

    if code != 0 {
        return (Observed::Refused, transcript);
    }
    assert_eq!(
        start,
        head(&e.path()),
        "`{verb}` advanced the branch in {example} — the invariant `capability_run.rs` holds \
         is broken, and every claim this file checks is the wrong question:\n{transcript}"
    );
    (Observed::RoutedToProposal, transcript)
}

/// The fact every scan below is held to.
///
/// Stated as its own test so a change in behaviour fails *here*, naming the behaviour, rather
/// than as three prose failures that read like a documentation problem.
#[test]
fn the_binary_loads_an_epistemic_verb_and_routes_it_to_a_proposal() {
    let (observed, transcript) = observe();
    assert_eq!(
        observed,
        Observed::RoutedToProposal,
        "a capability declaring an epistemic verb no longer loads. If that is intended, the \
         documents this file guards should say so again and the expectation here should move \
         with them — do not delete the scans:\n{transcript}"
    );
    assert!(
        transcript.contains("propose/"),
        "the run reported no proposal branch, so a reader is told the branch did not move and \
         not where the commit went:\n{transcript}"
    );
}

// ── the documents ─────────────────────────────────────────────────────────────

/// This file, which is the one place the claim appears as a quotation rather than an assertion.
///
/// `REFUSALS` *is* the sentence, spelled out; the module doc quotes it to say what the scan is
/// for; and [`the_predicate_catches_the_claim_and_spares_the_history`] holds all three reported
/// wordings on purpose, so that the predicate cannot go quiet. A scan that read its own
/// definition would report every one of them and could never pass.
const DEFINITION_SITE: &str = "yidam/cli/tests/run_route_claims.rs";

/// Every authored document that could carry the claim, plus the help text that ships.
///
/// Discovered rather than listed, for the reason `cli_reference.rs` gives: a roster of three
/// paths would stop covering a fourth site without ever going red, and a fourth site is exactly
/// what this defect was.
///
/// Reading `git ls-files` rather than walking the tree has a trap worth naming, because this
/// file fell into it: an uncommitted file is not listed, so the scan passed locally while the
/// gate was untracked and failed the moment it was committed. The `docs.len()` floor below is
/// what catches the general form of that — a listing that returns nothing reads exactly like a
/// repository with nothing to say.
fn authored_prose() -> Vec<(String, String)> {
    let root = repo_root();
    let mut docs: Vec<(String, String)> = common::git::out(&root, &["ls-files", "*.md", "*.rs"])
        .lines()
        .filter(|p| !p.starts_with("yidam/tests/results/"))
        .filter(|p| *p != DEFINITION_SITE)
        .filter_map(|p| {
            let text = std::fs::read_to_string(root.join(p)).ok()?;
            Some((p.to_string(), text))
        })
        .collect();
    assert!(
        docs.len() > 50,
        "only {} files were read, so these scans cover almost nothing",
        docs.len()
    );
    docs.push(("yidam run --help".into(), run_help()));
    docs
}

fn run_help() -> String {
    let out = Command::new(env!("CARGO_BIN_EXE_yidam"))
        .args(["run", "--help"])
        .output()
        .expect("running `yidam run --help`");
    String::from_utf8_lossy(&out.stdout).to_string()
}

/// One paragraph's worth of text, so a claim split over a line break is still one sentence.
fn sentences(text: &str) -> Vec<String> {
    text.replace('\n', " ")
        .split(". ")
        .map(|s| s.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|s| !s.is_empty())
        .collect()
}

/// Whether a sentence asserts, in the present tense, that such a declaration is turned away.
///
/// Tense is the whole of the distinction, and it has to be carried by the predicate rather
/// than by a keyword. `undeclarable` was the first spelling of this list and it matched four
/// sites, all of them this repository recording why the design changed — *"it made the whole
/// epistemic half of the vocabulary undeclarable"* in `manifest.rs`, `run/mod.rs`,
/// `capability_run.rs` and RFC-0026's amendment. Every one of those is a past participle under
/// `made`, and a guard that could not tell them from a claim would forbid writing the history
/// down, which all four do deliberately.
///
/// So each entry is a **finite verb in the present tense**: `is undeclarable` is a claim about
/// the binary, `made … undeclarable` is an account of a binary that is gone.
fn asserts_a_refusal(sentence: &str) -> bool {
    let s = sentence.to_lowercase();
    if !s.contains("epistemic") {
        return false;
    }
    const REFUSALS: &[&str] = &[
        "does not load",
        "doesn't load",
        "do not load",
        "cannot be declared",
        "can not be declared",
        "is refused",
        "are refused",
        "is undeclarable",
        "are undeclarable",
    ];
    REFUSALS.iter().any(|r| s.contains(r))
}

/// The predicate matches the sentences that were actually written, and not the history.
///
/// A scan whose predicate matches nothing passes everything, silently, and looks exactly like
/// a scan that is doing work — so the three sentences #873 reported are kept here as the
/// population it must still catch, alongside the four it must not.
#[test]
fn the_predicate_catches_the_claim_and_spares_the_history() {
    for claim in [
        // `guidelines/directories.md`, `main.rs` and `cli-reference.md`, as written at 1d81685.
        "A capability declaring an epistemic verb does not load.",
        "A run authors operational commits and nothing else: a capability declaring an \
         epistemic verb does not load, so no run can author a node.",
        "A capability declaring an epistemic verb does not load at all.",
    ] {
        assert!(
            asserts_a_refusal(claim),
            "the predicate no longer catches the sentence this file exists for: {claim}"
        );
    }
    for history in [
        "That was the right holding action and it made the whole epistemic half of the \
         vocabulary undeclarable.",
        "manifest.rs refused an epistemic verb at load, so nothing could produce an epistemic \
         commit at all.",
        "For the first slice the second half was enforced by refusing the epistemic verb.",
        "An epistemic verb lands on propose/<head> and the branch does not move.",
    ] {
        assert!(
            !asserts_a_refusal(history),
            "the predicate reads an account of the superseded design, or the current \
             behaviour, as a stale claim: {history}"
        );
    }
}

/// No document says a declaration is turned away while the binary accepts one.
#[test]
fn no_document_claims_an_epistemic_verb_is_refused() {
    let (observed, transcript) = observe();
    if observed == Observed::Refused {
        // The behaviour is what the prose describes. Nothing to refuse.
        return;
    }

    let mut stale = Vec::new();
    for (path, text) in authored_prose() {
        for sentence in sentences(&text) {
            if asserts_a_refusal(&sentence) {
                stale.push(format!("  {path}\n    {sentence}"));
            }
        }
    }
    assert!(
        stale.is_empty(),
        "these say a capability declaring an epistemic verb is turned away. It is not — it \
         loads, it runs, and its commit lands on a proposal branch. Say where the commit goes \
         instead; `Capability::route` is the behaviour and RFC-0026 §2 is the argument.\n\n{}\n\n\
         What the binary did:\n{transcript}",
        stale.join("\n")
    );
}

/// `yidam run --help` tells an operator where an epistemic run's commit lands.
///
/// This is the affordance the stale prose hid rather than merely misdescribed. A reader told
/// only that their branch did not move has no way to learn that `propose/<head>` is where to
/// look, and `--help` is what they read before writing a manifest.
#[test]
fn run_help_names_the_proposal_destination() {
    let (observed, _) = observe();
    if observed == Observed::Refused {
        return;
    }
    let help = run_help();
    assert!(
        help.contains("propose/"),
        "`yidam run --help` does not name the proposal branch, so an operator writing a \
         manifest is not told where an epistemic run's commit goes:\n{help}"
    );
}

/// The vendored copy is the one that cost something, so it is held to more than the others.
///
/// A derived repository reads `guidelines/directories.md` to decide what a capability layer
/// could do and cannot edit it. It gets the route, the destination, and the argument — that
/// last being the part that survived `d8161ad` unchanged and is why there is no field to set.
#[test]
fn the_vendored_guideline_states_the_route_and_keeps_the_argument() {
    let rel = "yidam/prelude/guidelines/directories.md";
    let path: PathBuf = repo_root().join(rel);
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{} is unreadable ({e})", path.display()));
    let flat = text.replace('\n', " ");

    for (needle, why) in [
        (
            "propose/<head>",
            "a derived repo otherwise has no way to learn where an epistemic run's commit went",
        ),
        (
            "no config value",
            "the argument that outlived the refusal: a destination a repository could declare \
             would make the safety argument a config value",
        ),
    ] {
        assert!(
            flat.contains(needle),
            "{rel} does not say `{needle}` — {why}"
        );
    }
}

// ── what a run may declare: the kinds ─────────────────────────────────────────
//
// The same defect as the route claims, in the other direction. There the prose described a
// mechanism the code had dropped; here the code lacked an arm the prose had published, and
// `Kind` carried two of the taxonomy's three for long enough that a crate implementing the
// third got `crates-index`'s em dash whatever it declared (#1027). Both are one document and
// one enum disagreeing with nobody comparing them, so both are compared here.
//
// Two directions, because one of them is the whole finding. A scan asking only "is every
// documented kind one the binary knows" would have passed throughout #1027: the documents named
// three kinds and showed one, and the arm that was missing was missing from the code.

/// Every `[capability.…]` this repository ships or documents names a kind the binary knows.
///
/// Over fenced examples as well as real manifests, because a `toml` block in a document is what
/// a corpus author copies — a documented example is a prediction about the binary, and this is
/// the assertion that it holds.
#[test]
fn every_documented_capability_names_a_kind_the_binary_knows() {
    let known = kind_spellings();
    let root = repo_root();
    let mut scanned: Vec<String> = Vec::new();
    let mut wrong: Vec<String> = Vec::new();

    for path in common::git::out(&root, &["ls-files", "*.md", "*.toml"]).lines() {
        // This file quotes a kind in order to check it, and `manifest.rs`'s own doc comment
        // records the rejected spellings. Neither is a declaration a corpus would copy.
        if path == DEFINITION_SITE {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(root.join(path)) else {
            continue;
        };
        for (step, kind) in capability_kinds(&text) {
            scanned.push(format!("{path} [capability.{step}] {kind}"));
            if !known.contains(&kind) {
                wrong.push(format!(
                    "  {path}\n    [capability.{step}] kind = \"{kind}\""
                ));
            }
        }
    }

    // Per site rather than a count, because a count is cleared by a scan that reads one file.
    // These two are the population the gate exists for: the shipped manifest, and the vendored
    // document a derived repository copies from.
    for site in [
        "examples/streamflow/.yidam/capabilities.toml",
        "yidam/prelude/guidelines/directories.md",
        "docs/cli-reference.md",
    ] {
        assert!(
            scanned.iter().any(|s| s.starts_with(site)),
            "no `[capability.…]` was read from {site}, so this scan does not cover it. Either \
             the block moved or the parse broke; a scan that reads nothing passes everything.\n\
             read: {scanned:?}"
        );
    }

    assert!(
        wrong.is_empty(),
        "these declare a `kind` the binary does not know, so the manifest does not parse at all \
         — not the named refusal a reader could act on, but a serde error against the whole \
         file. The kinds are {known:?}.\n\n{}",
        wrong.join("\n")
    );
}

/// The vendored guideline names every kind a corpus may declare.
///
/// The converse, and the reason #1027 was filed. `guidelines/directories.md` is what a derived
/// repository reads to decide what its capability layer can be, and it cannot edit its copy. For
/// as long as that document typed the domain computer as three kinds while the manifest comment
/// beside the example offered two, the third was a vocabulary a corpus could read and not write
/// — and the way to find that out was to read `manifest.rs`, which a derivation has no copy of.
///
/// Held to the vendored document specifically, for the reason
/// [`the_vendored_guideline_states_the_route_and_keeps_the_argument`] gives: it is the copy that
/// cost something.
#[test]
fn the_vendored_guideline_names_every_kind_a_corpus_may_declare() {
    let rel = "yidam/prelude/guidelines/directories.md";
    let text = std::fs::read_to_string(repo_root().join(rel))
        .unwrap_or_else(|e| panic!("{rel} is unreadable ({e})"));
    for kind in kind_spellings() {
        assert!(
            text.contains(&format!("`{kind}`")) || text.contains(&format!("\"{kind}\"")),
            "{rel} never names `{kind}`, which a corpus may declare. A derived repository reads \
             this document to decide what its capability layer can be and cannot edit its copy, \
             so a kind absent here is a kind nothing will declare — #1027."
        );
    }
}
