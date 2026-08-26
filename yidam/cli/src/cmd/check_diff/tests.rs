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
