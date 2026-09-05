//! What the report says, from a diff and a vocabulary. No repository, no subprocess.

use super::*;
use crate::cmd::lint::checks::{Class, ClassEdge, ClassProperty};

fn class(name: &str, properties: &[&str], edges: &[&str]) -> Class {
    Class {
        rel: format!(".yidam/corpus/{name}.ont.yml"),
        description: String::new(),
        name: name.to_string(),
        properties: properties
            .iter()
            .map(|p| ClassProperty {
                name: p.to_string(),
                r#type: "string".to_string(),
                // These fixtures are about which properties a class declares across two
                // commits, not about whether an instance must carry them.
                required: false,
            })
            .collect(),
        edges: edges
            .iter()
            .map(|e| ClassEdge {
                relationship: e.to_string(),
                target: String::new(),
                direction: None,
            })
            .collect(),
        edge_policy: Default::default(),
        max_lines: None,
        implemented_by: None,
        foundational_type: None,
        dead_alignment_fields: vec![],
    }
}

fn ontology() -> BTreeSet<String> {
    extract::declared(&[
        class("bill", &["session_law"], &["sponsored-by"]),
        class("chamber", &[], &[]),
    ])
}

fn diff(lines: &str) -> String {
    format!("+++ b/crates/dossier/src/lib.rs\n@@ -0,0 +1,9 @@\n{lines}")
}

fn report(lines: &str, a: &Authorship) -> CheckDiffReport {
    build("main..HEAD", &diff(lines), &ontology(), a)
}

#[test]
fn a_type_the_ontology_names_is_counted_and_not_reported() {
    let r = report("+pub struct Bill;\n", &Authorship::default());
    assert_eq!((r.introduced, r.aligned), (1, 1));
    assert!(r.findings.is_empty());
}

/// All three, and not classes alone: a concept can arrive in the ontology as a property of
/// something else or as the name of a relationship.
#[test]
fn a_property_name_and_a_relationship_name_both_count_as_declared() {
    let r = report(
        "+pub struct SessionLaw;\n+pub struct SponsoredBy;\n",
        &Authorship::default(),
    );
    assert_eq!(r.aligned, 2, "{:?}", r.findings);
    assert!(r.findings.is_empty());
}

#[test]
fn a_type_nothing_names_is_a_warn_and_carries_the_kebab_form() {
    let r = report("+pub struct AgendaItem;\n", &Authorship::default());
    assert_eq!(r.findings.len(), 1);
    let f = &r.findings[0];
    assert_eq!(f.check, CHECK);
    assert_eq!(f.severity, "warn");
    assert_eq!(f.concept, "AgendaItem");
    assert_eq!(f.name, "agenda-item");
    assert_eq!(f.file, "crates/dossier/src/lib.rs");
    assert!(f.region.is_none());
}

/// The register: the check cannot tell a gap from a helper, and must not phrase itself as
/// though it could.
#[test]
fn the_finding_is_a_question_and_offers_both_answers() {
    let r = report("+pub struct AgendaItem;\n", &Authorship::default());
    let q = &r.findings[0].question;
    assert!(q.contains('?'), "{q}");
    assert!(q.contains("agenda-item"), "{q}");
    assert!(q.contains("no reason to know about"), "{q}");
}

/// RFC-0022's worked example, end to end: the reader is handed the name they were about to
/// go looking for.
#[test]
fn a_near_miss_is_carried_on_the_finding_and_named_in_the_question() {
    let r = report("+pub struct Sponsorship;\n", &Authorship::default());
    let f = &r.findings[0];
    let n = f.nearest.as_ref().expect("`sponsored-by` shares a root");
    assert_eq!(n.name, "sponsored-by");
    assert_eq!(n.shared, "sponsor");
    assert!(f.question.contains("`sponsored-by`"), "{}", f.question);
    assert!(f.question.contains("root `sponsor`"), "{}", f.question);
}

/// **It annotates a finding that exists either way.** This is the whole reason no threshold
/// needed calibrating: a candidate cannot add a row, so a wrong one costs a bad lead and
/// never a false finding.
#[test]
fn a_near_miss_adds_no_row_and_does_not_count_as_alignment() {
    let r = report("+pub struct Sponsorship;\n", &Authorship::default());
    assert_eq!((r.introduced, r.aligned, r.findings.len()), (1, 0, 1));
}

/// Most unmatched types have no near-miss, and those findings must read exactly as they did
/// before Phase B.
#[test]
fn a_finding_with_no_candidate_keeps_phase_as_question_and_omits_the_field() {
    let r = report("+pub struct AgendaItem;\n", &Authorship::default());
    let f = &r.findings[0];
    assert!(f.nearest.is_none());
    assert_eq!(
        f.question,
        "nothing the ontology declares is named `agenda-item`. Is it a concept this corpus \
         should model, or a helper the ontology has no reason to know about?"
    );
}

/// A shared root is a fact about two strings, and the report has to keep saying that — but
/// only where it offered one.
#[test]
fn the_report_explains_a_candidate_only_when_it_offered_one() {
    let with = render(&report(
        "+pub struct Sponsorship;\n",
        &Authorship::default(),
    ));
    assert!(with.contains("sharing a root and nothing more"), "{with}");
    assert!(with.contains("No model read"), "{with}");

    let without = render(&report("+pub struct AgendaItem;\n", &Authorship::default()));
    assert!(
        !without.contains("sharing a root"),
        "a report that suggested nothing must not explain how it would have:\n{without}"
    );
}

fn manifest(body: &str) -> Authorship {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(tmp.path().join(".yidam")).unwrap();
    std::fs::write(tmp.path().join(crate::authorship::MANIFEST), body).unwrap();
    Authorship::load(tmp.path()).unwrap()
}

/// Still reported, and reported to somebody: the defect is upstream's, not this run's.
#[test]
fn a_type_inside_an_imported_region_is_info_and_names_who_can_act() {
    let a =
        manifest("imported:\n  - path: crates/dossier/\n    from: acme/gis at the fork point\n");
    let r = report("+pub struct AgendaItem;\n", &a);
    assert_eq!(r.introduced, 1);
    assert_eq!(r.findings[0].severity, "info");
    let region = r.findings[0].region.as_deref().unwrap();
    assert!(region.contains("acme/gis at the fork point"), "{region}");
}

/// `excluded` is the one kind that means *do not look* — and a type nobody may be asked
/// about is not one this run saw, so it is not in the denominator either.
#[test]
fn an_excluded_region_is_silent_and_uncounted() {
    let a = manifest("excluded:\n  - path: crates/dossier/\n    why: a scratch crate\n");
    let r = report("+pub struct AgendaItem;\n+pub struct Bill;\n", &a);
    assert_eq!((r.introduced, r.aligned), (0, 0));
    assert!(r.findings.is_empty());
}

#[test]
fn a_diff_that_declares_nothing_says_so_rather_than_printing_a_zero_of_zero() {
    let r = build("main..HEAD", "", &ontology(), &Authorship::default());
    assert_eq!(r.introduced, 0);
    let text = render(&r);
    assert!(text.contains("No type declaration"), "{text}");
    assert!(!text.contains("0 of 0"), "{text}");
}

/// A count, and never a finding: one row per correctly implemented concept is a permanently
/// non-empty report by construction.
#[test]
fn alignment_is_a_number_in_the_summary_and_not_a_row() {
    let r = report(
        "+pub struct Bill;\n+pub struct Chamber;\n",
        &Authorship::default(),
    );
    let text = render(&r);
    assert_eq!(r.aligned, 2);
    assert!(text.contains("2 of 2"), "{text}");
    assert!(
        !text.contains("Bill"),
        "an aligned type is a count, not a row:\n{text}"
    );
    assert!(text.contains("Nothing here is unmodelled"), "{text}");
}

/// The one thing a shallow matcher must keep saying about itself.
#[test]
fn the_report_states_that_matching_is_by_name_alone() {
    let r = report("+pub struct AgendaItem;\n", &Authorship::default());
    let text = render(&r);
    assert!(text.contains("by name alone"), "{text}");
    assert!(text.contains("no class was created"), "{text}");
}

#[test]
fn findings_are_grouped_under_the_file_that_holds_them() {
    let d = "+++ b/crates/a/src/lib.rs\n@@ -0,0 +1,1 @@\n+pub struct Canvass;\n\
             +++ b/crates/b/src/lib.rs\n@@ -0,0 +1,1 @@\n+pub struct Caucus;\n";
    let text = render(&build("main..HEAD", d, &ontology(), &Authorship::default()));
    assert!(text.contains("  crates/a/src/lib.rs\n"), "{text}");
    assert!(text.contains("  crates/b/src/lib.rs\n"), "{text}");
}

/// #389 — what a bare `check-diff` compares, and where it refuses.
///
/// Every case here is a git position that costs a repository to reproduce and one line to
/// describe, which is why [`default_range`] takes the position rather than reading it.
mod default_range {
    use super::*;

    fn at(
        base: Option<&str>,
        base_tip: Option<&str>,
        merge_base: Option<&str>,
        head: &str,
    ) -> Position {
        Position {
            base: base.map(str::to_string),
            base_tip: base_tip.map(str::to_string),
            merge_base: merge_base.map(str::to_string),
            head: Some(head.to_string()),
        }
    }

    /// The ordinary case: branched from `main`, which has not moved. The endpoint *is* the
    /// merge-base, so the legible two-dot range is also the exactly correct one.
    #[test]
    fn a_branch_off_an_unmoved_baseline_reads_as_the_branch_name() {
        let p = at(Some("main"), Some("aaa"), Some("aaa"), "bbb");
        assert_eq!(default_range(&p).unwrap(), "main..HEAD");
    }

    /// Once the baseline moves on, `main..HEAD` and the merge-base are different commits and
    /// a two-dot range would compare the wrong pair — it would report `main`'s own new work
    /// as though this branch had introduced it. Only the sha states what is compared.
    #[test]
    fn a_baseline_that_moved_on_is_named_by_its_merge_base_and_not_by_the_branch() {
        let p = at(Some("main"), Some("ccc"), Some("aaa"), "bbb");
        assert_eq!(default_range(&p).unwrap(), "aaa..HEAD");
    }

    /// `master` is the baseline wherever `main` is not, exactly as `git::base_branch` decides
    /// it, and the message has to name the one that was actually looked for.
    #[test]
    fn the_baseline_may_be_master() {
        let p = at(Some("master"), Some("aaa"), Some("aaa"), "bbb");
        assert_eq!(default_range(&p).unwrap(), "master..HEAD");
    }

    /// The case the measurement made load-bearing: 8 of 13 derived corpora sit here. An empty
    /// report would be indistinguishable from *your branch introduced no types*.
    #[test]
    fn standing_on_the_baseline_refuses_rather_than_reporting_an_empty_range() {
        let p = at(Some("main"), Some("aaa"), Some("aaa"), "aaa");
        let e = default_range(&p).unwrap_err().to_string();
        assert!(e.contains("you are on `main`"), "{e}");
        assert!(e.contains("check-diff HEAD~5"), "{e}");
    }

    /// Pre-genesis. A repository mid-bootstrap has no baseline to have branched from.
    #[test]
    fn a_repository_with_no_baseline_says_so_and_does_not_guess() {
        let p = at(None, None, None, "aaa");
        let e = default_range(&p).unwrap_err().to_string();
        assert!(e.contains("no baseline to compare against"), "{e}");
    }

    /// An unborn HEAD, or unrelated histories: git names no merge-base and neither does this.
    #[test]
    fn no_common_ancestor_is_refused_by_name() {
        let p = at(Some("main"), Some("aaa"), None, "bbb");
        let e = default_range(&p).unwrap_err().to_string();
        assert!(e.contains("no common ancestor"), "{e}");
    }

    /// Detached HEAD is not a failure case, though #389 was filed believing it was:
    /// `merge-base main HEAD` resolves there like anywhere else, and the position it produces
    /// is the ordinary one. Pinned so the belief cannot come back as a special case.
    #[test]
    fn a_detached_head_is_an_ordinary_position_and_not_an_error() {
        let p = at(Some("main"), Some("aaa"), Some("aaa"), "bbb");
        assert!(default_range(&p).is_ok());
    }
}
