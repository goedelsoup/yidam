//! The parts of `yidam phase` that are decidable without a repository.
//!
//! Everything that needs one — the commits, the resumption, the ranking against ref shape —
//! is in `tests/phase_record.rs`, against the real binary. What is here is the three
//! judgements this module makes before it touches git: what a phase may be named, what a
//! kuten permits it to be, and what subject a person is handed to paste.

use super::*;

// ── the name, which is a ref, a file stem and an identity at once ─────────────

#[test]
fn a_phase_name_is_the_ref_it_would_become_or_it_is_refused() {
    for ok in [
        "outcome-axis",
        "the-local-half",
        "a",
        "s3-vectors",
        "phase2",
    ] {
        assert!(is_kebab(ok), "{ok}");
    }
    for bad in [
        "",
        "Outcome Axis",
        "outcome_axis",
        "-leading",
        "trailing-",
        "double--dash",
        "Investigation",
        "outcome/axis",
    ] {
        assert!(!is_kebab(bad), "{bad}");
    }
}

/// The suggestion a refusal names. It is a repair offered to a person, never a rename this
/// command performs — see [`start`].
#[test]
fn the_suggested_name_is_one_a_refusal_would_accept() {
    for input in [
        "Outcome Axis",
        "outcome_axis",
        "  outcome   axis  ",
        "Outcome--Axis",
        "The Local Half!",
    ] {
        let out = kebab(input);
        assert!(
            is_kebab(&out),
            "{input} → {out}, which would be refused too"
        );
    }
    assert_eq!(kebab("Outcome Axis"), "outcome-axis");
    assert_eq!(kebab("The Local Half!"), "the-local-half");
}

// ── the type, and the list it is held to ──────────────────────────────────────

fn held(types: Option<&[&str]>) -> Held {
    Held {
        name: Some("inquiry".into()),
        revision: Some(2),
        types: types.map(|t| t.iter().map(|s| s.to_string()).collect()),
    }
}

#[test]
fn a_type_the_vendored_kuten_declares_is_accepted_and_one_it_omits_is_not() {
    let h = held(Some(&["Investigation", "Synthesis"]));
    assert!(h.validate("Investigation").is_ok());
    assert!(h.validate("Synthesis").is_ok());

    let refused = h.validate("Extraction").unwrap_err().to_string();
    assert!(refused.contains("Investigation, Synthesis"), "{refused}");
    assert!(
        refused.contains("vendored"),
        "the refusal must say which list it read — RFC-0028 §2: {refused}"
    );
}

/// **There is no default list**, and the absence is RFC-0028 Erratum 1's finding rather than
/// a gap: implementing one is the only way to create the four-element phase-type list in the
/// binary that the section objects to. Nothing in any repository is typed — 60 `phase/*` refs
/// across eighteen derived corpora, 301 `phase:` subjects, and no file under any `.yidam/`
/// recording one — so a layer that refused an untyped phase would refuse all of them.
#[test]
fn a_repository_declaring_no_types_validates_against_nothing() {
    assert!(Held::default().validate("Excavation").is_ok());
    assert!(held(None).validate("Excavation").is_ok());
    assert!(
        held(Some(&[])).validate("Excavation").is_ok(),
        "an empty list is no list — a profile declaring `types: []` constrains nothing, and \
         reading it as a closed empty set would make every phase type invalid"
    );
}

// ── the merge subject, which a person pastes ──────────────────────────────────

/// The draft carries `phase`, which GRAPH.md closes and `lint --commits` reads. A subject this
/// tool hands somebody to paste is one it is answerable for.
#[test]
fn the_drafted_subject_is_in_the_closed_vocabulary() {
    let subject = merge_subject("outcome-axis", 4, 9);
    let verb = subject.split_once(": ").unwrap().0;
    assert_eq!(verb, "phase");
    assert!(yidam_core::git::is_recognized_verb(verb));
    assert!(
        yidam_core::git::EPISTEMIC_VERBS.contains(&verb),
        "and it is the epistemic one, which is why nothing here writes it"
    );
}

/// The verb stands alone — GRAPH.md: *everything before the first `: ` is the verb*, so
/// `phase(outcome-axis):` is read as a verb in no list, reported off-vocabulary and classified
/// as Epistemic by fallthrough rather than by recognition.
#[test]
fn the_drafted_subject_carries_no_conventional_commits_scope() {
    let subject = merge_subject("outcome-axis", 1, 1);
    let verb = subject.split_once(": ").unwrap().0;
    assert!(!verb.contains('('), "{subject}");
    assert!(subject.contains("outcome-axis"), "{subject}");
}

#[test]
fn the_drafted_subject_counts_in_the_plural_it_means() {
    assert!(merge_subject("x", 1, 1).contains("1 commit across 1 file"));
    assert!(merge_subject("x", 2, 3).contains("2 commits across 3 files"));
}
