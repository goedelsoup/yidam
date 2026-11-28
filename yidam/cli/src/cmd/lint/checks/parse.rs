use super::*;

/// The id of [`malformed_yaml`], named once so the suppression it licenses cannot come to
/// mean a check that no longer exists.
pub const MALFORMED_YAML: CheckSpec = CheckSpec {
    id: "malformed-yaml",
    title: "File that is not the YAML it is read as",
    severity: Severity::Error,
    rationale: "A file the parser rejects reaches every other check as an empty record, and an \
         empty record is not the same claim as an empty file. On an instance that reads as a \
         node declaring nothing, so the report contradicts the file it names. On a \
         `<class>.ont.yml` it reads as a class declaring no properties and licensing no \
         edges, which is indistinguishable from an ontology nobody has filled in — and five \
         checks that gate on those declarations find nothing to check and pass. That is a \
         false negative in a gate, produced by a typo, and nothing else in the report can \
         say it happened. On a catalog entry it reads as an entry with no TTL, no `used-by` \
         and no artifacts that never declared `obtained: false` — so it is read as obtained, \
         governed by nothing, and cited by nothing. On a decision record it reads as a record \
         with no `id:` and no `summary:`, which the log renders under its file stem with an em \
         dash, indistinguishable from a record nobody filled in. On \
         `.yidam/corpus/universal.yml` it reads as a corpus that declared nothing universal, \
         so every property that file permits is reported against the instances carrying it, \
         and the edge-tagging gate it turns on is off. Every other finding about the file is \
         suppressed: fix the parse error and they come back, correct this time.",
};

/// A corpus file whose bytes are not the document the model reads them as.
///
/// **The check that has to run before the others can be believed.** Every other check in this
/// module reads an instance or a class through a parsed record, and until #676 a file that
/// did not parse produced an *empty* record — so the rest of the report described a file
/// nobody had read, in whichever direction the empty record happened to point.
///
/// It failed both ways, and the second is the serious half. One unclosed quote in an instance
/// reported eight findings across six checks, opening with `missing-class` against a file
/// whose first line is `class: gage`. The same typo in that class's `<class>.ont.yml` reported
/// **nothing**: an unreadable class declares no properties and licenses no edges, so
/// `missing-property`, `undeclared-property`, `property-type`, `edge-target-class` and
/// `unlicensed-edge` each found nothing to check and passed. Measured on a controlled pair
/// whose instances carried an identical defect, the sound schema reported one finding and the
/// broken one exited 0.
///
/// # Error, and the only finding the file gets
///
/// Error, because the class arm is a gate reporting clean on a corpus it did not read, and
/// there is no legacy-debt argument to weigh against it — a file that is not YAML is not a
/// judgement call, and no corpus ever declared it acceptable.
///
/// The suppression is the other half and lives in [`super::super::suppress_unparsed`]: every other
/// check's findings about an unparsed file are dropped. Without it the instance arm keeps its
/// eight contradictory findings and merely gains a ninth, and the ninth is the only one worth
/// reading. What is *not* suppressed is a finding whose subject is a different file, even
/// where the unreadable one is why it fired — a catalog entry whose `used-by` list names a
/// node that can no longer be shown to cite it still says so. See there for why.
///
/// # All four records, since #1056
///
/// #676 covered the two records `lint` had a model for, and the other two kept the defect
/// verbatim. A catalog entry's frontmatter read through `unwrap_or_default()` to a
/// `Frontmatter::default()`, which is not an entry declaring nothing — it is `obtained:`
/// absent and so read as *true*, with no TTL, no `used-by`, no locations and no artifacts. A
/// decision record read to a `Decision` with no `id:` and no `summary:`, which the log renders
/// under its file stem with an em dash, exactly as it renders a record nobody has filled in.
///
/// One corpus lost a catalog entry's TTL for its whole history and a decision record that
/// governed a phase, to one unclosed quote each. `grep` saw both; nothing in this binary did,
/// and the class arm's reason applies unchanged: these are gates and a renderer reporting
/// clean over files they did not read. `catalog-artifact-malformed` is not that report — it
/// fires only on entries that *declare* `artifacts:`, and an entry whose header did not parse
/// declares none.
///
/// # And `universal.yml`, since #1081
///
/// The fifth file, and not a fifth record: [`crate::universal::Universal`] is one document per
/// corpus rather than a type the walk produces many of, so it arrives here as itself and is
/// named by [`crate::universal::REL`] rather than by a `rel` it carries.
///
/// It is the class arm exactly. `.yidam/corpus/universal.yml` declares the properties any
/// class may carry, the top-level keys that are prose, and whether an edge must name its
/// standing — and [`crate::universal::Universal::parse`] read it through
/// `unwrap_or_default()`, so a corpus whose declarations did not parse declared *none of
/// them*. `undeclared-property` then reports every property the file permits, `property-type`
/// checks none of them, `node-too-long` measures a node without the prose keys the corpus
/// named, and `edge-untagged` is switched off entirely — `edge_claims.required` read as absent,
/// and absent means no. One of those four is noise and three are gates reporting clean, and
/// nothing anywhere said the file had not been read.
///
/// **Reported, not suppressed around.** The findings that go wrong here are about the
/// *instances*, which is the case [`super::super::suppress_unparsed`] deliberately leaves alone: a
/// finding whose subject is a different file stands even where an unreadable one is why it
/// fired. What was missing is this finding — the one that says why the rest of the report is
/// not to be trusted.
pub fn malformed_yaml(
    nodes: &[Node],
    classes: &[Class],
    sources: &[Source],
    decisions: &[DecisionRecord],
    universal: &crate::universal::Universal,
) -> Check {
    // Instances, classes, catalog, decisions — the order [`prose_views`] reads the corpus in,
    // extended the way the walk is. `universal.yml` goes last: it is one file rather than a
    // record type, so appending it leaves every finding a report already had where it was.
    let violations = nodes
        .iter()
        .map(|n| (n.rel.as_str(), n.malformed.as_deref()))
        .chain(
            classes
                .iter()
                .map(|c| (c.rel.as_str(), c.malformed.as_deref())),
        )
        .chain(
            sources
                .iter()
                .map(|s| (s.rel.as_str(), s.malformed.as_deref())),
        )
        .chain(
            decisions
                .iter()
                .map(|d| (d.rel.as_str(), d.malformed.as_deref())),
        )
        .chain(std::iter::once((
            crate::universal::REL,
            universal.malformed(),
        )))
        .filter_map(|(rel, why)| {
            Some(Violation::new(
                rel,
                format!("does not parse as YAML: {}", why?),
            ))
        })
        .collect();
    Check::new(&MALFORMED_YAML, violations)
}

/// The id of [`findings_malformed`], named once beside the check it licenses nothing for — it
/// is spelled here for the reason [`MALFORMED_YAML`] is, so the two read as the pair they are.
pub const FINDINGS_MALFORMED: CheckSpec = CheckSpec {
    id: "findings-malformed",
    title: "Document whose findings block is not the record it is read as",
    severity: Severity::Error,
    rationale:
        "The `yidam:` block is where `yidam propose` records the questions it has opened, and a \
         block the parser rejects was read as no findings at all — so one entry missing its \
         `check:` lost every finding on the document and nothing said so. What follows is not a \
         gap in a report: a document with no readable block looks like a document with nothing \
         outstanding, so `propose` opens every finding on it again with a fresh `opened_at`, and \
         the ids an author has already closed come back. The file itself parses — unknown \
         top-level keys are carried, so neither the instance nor the catalog entry reports a \
         parse failure — which is why this is reported apart from `malformed-yaml`. Nothing but \
         `propose` writes under this key, so a block of the wrong shape is never something a \
         corpus chose.",
};

/// A document whose `yidam:` block is not the record this tool writes there.
///
/// The sibling of [`malformed_yaml`], and the reason it is a second check rather than a fifth
/// arm of that one: **the file parses**. [`crate::corpus::CorpusInstance`] carries unknown
/// top-level keys in a flattened mapping and a catalog entry's frontmatter ignores them, so a
/// `yidam:` block that is well-formed YAML of the wrong shape leaves `Node::malformed` and
/// `Source::malformed` both empty. Reporting it as "does not parse as YAML" would say something
/// false about the bytes.
///
/// # What is lost, which is a baseline rather than a field
///
/// The block is where `propose` records the questions it has opened, and
/// [`crate::findings::parse`] read it through `unwrap_or_default()` — so one entry missing its
/// `check:` lost every finding on the document, silently. A document whose block does not read
/// looks like a document with nothing outstanding, and the consequences run the wrong way:
/// `propose` opens every finding on it again with a fresh `opened_at`, and the ids an author
/// had already `close:`d come back. The corpus's own count of how many of its open questions
/// are the tool's — the whole reason the record is namespaced instead of spliced into prose —
/// is short by however many the document held.
///
/// Error, for [`malformed_yaml`]'s reason narrowed to a key this tool owns: nothing but
/// `propose` and `migrate-findings` write under `yidam:`, so there is no corpus whose authors
/// ever declared a block of this shape acceptable, and no judgement call to weigh.
///
/// # Both populations, because findings live on both
///
/// Instances and catalog entries: `propose` walks both, so a check over one of them would be
/// half a report. The catalog arm is what [`crate::corpus::Source::text`] was added for.
pub fn findings_malformed(nodes: &[Node], sources: &[Source]) -> Check {
    let violations = nodes
        .iter()
        .map(|n| (n.rel.as_str(), n.text.as_str()))
        .chain(sources.iter().map(|s| (s.rel.as_str(), s.text.as_str())))
        .filter_map(|(rel, text)| {
            let why = crate::findings::parse_reporting(text).1?;
            Some(Violation::new(
                rel,
                format!(
                    "`{}:` block does not read as findings: {why}",
                    crate::findings::CONTAINER
                ),
            ))
        })
        .collect();
    Check::new(&FINDINGS_MALFORMED, violations)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cmd::lint::testkit::*;

    // ── the parse outcome the model used to throw away (#676) ───────────────────

    /// The two constructors record why, and the reason reaches the finding.
    ///
    /// Both halves matter. A `Some` that carried no detail would report *that* a file is
    /// unreadable without saying where to look, and the parser's own message names the line
    /// and column the quote was opened at — the only part of this a person can act on.
    #[test]
    fn a_file_that_does_not_parse_carries_the_parsers_reason() {
        let n = Node::parse(
            PathBuf::from("c/x.yml"),
            "c/x.yml",
            "class: c\nlabel: \"unclosed\n",
        );
        let why = n.malformed.as_deref().expect("an unclosed quote");
        assert!(why.contains("line 2"), "{why}");
        // And the record it fell back to is the empty one, which is what every check
        // downstream reads and what the suppression exists to stop them reporting on.
        assert!(n.inst.class.is_none());

        let c = Class::parse(
            "c.ont.yml",
            "class: c\nproperties: [{name: p, type: string}]\n",
        );
        assert_eq!(c.malformed, None);
        assert_eq!(c.properties.len(), 1);
    }

    /// **An empty file is not a parse failure**, and the distinction is the whole reason the
    /// adjacent `Overlay::read` case stays out of scope: an unreadable file becomes `""`
    /// there, and reporting `""` as malformed YAML would fold a question about file
    /// permissions into a check about syntax.
    #[test]
    fn an_empty_document_is_the_absent_value_rather_than_a_failure() {
        for text in ["", "\n", "# a comment and nothing else\n"] {
            let n = Node::parse(PathBuf::from("c/x.yml"), "c/x.yml", text);
            assert_eq!(n.malformed, None, "{text:?}");
            assert!(
                Class::parse("c.ont.yml", text).malformed.is_none(),
                "{text:?}"
            );
        }
    }

    /// A catalog entry at `rel`, read the way [`crate::corpus::load_sources`] reads one — so a
    /// fixture cannot hand the check a `malformed` that disagrees with the bytes beside it.
    fn catalog(rel: &str, text: &str) -> Source {
        let (fm, malformed) = crate::parse::parse_frontmatter_reporting(text);
        Source {
            rel: rel.to_string(),
            path: PathBuf::from(rel),
            obtained: fm.obtained.unwrap_or(true),
            r#type: fm.r#type,
            used_by: fm.used_by,
            locations: fm.location.unwrap_or_default(),
            retrieved: fm.retrieved,
            ttl_days: fm.ttl_days,
            artifacts: fm.artifacts.unwrap_or_default(),
            columns: fm.columns,
            key: fm.key,
            text: text.to_string(),
            malformed,
        }
    }

    fn decision(rel: &str, text: &str) -> DecisionRecord {
        DecisionRecord::parse(PathBuf::from(rel), rel, text)
    }

    /// All four populations, in the order the corpus is read in.
    #[test]
    fn the_check_reports_every_record_a_repository_is_written_in() {
        let broken = Node::parse(PathBuf::from("c/x.yml"), "c/x.yml", "label: \"unclosed\n");
        let sound = node("c/y.yml", "class: c\n");
        let c = malformed_yaml(
            &[broken, sound],
            &[
                Class::parse("c.ont.yml", "class: c\n"),
                Class::parse("d.ont.yml", "label: \"unclosed\n"),
            ],
            &[
                catalog("cat/ok.md", "---\nobtained: true\n---\nbody\n"),
                catalog("cat/bad.md", "---\nttl_days: \"30\n---\nbody\n"),
            ],
            &[
                decision("dec/ok.yml", "id: d1\nsummary: fine\n"),
                decision("dec/bad.yml", "summary: \"unclosed\n"),
            ],
            &NONE,
        );
        let named: Vec<&str> = c.violations.iter().map(|v| v.node.as_str()).collect();
        assert_eq!(
            named,
            vec!["c/x.yml", "d.ont.yml", "cat/bad.md", "dec/bad.yml"]
        );
        assert_eq!(
            c.severity,
            Severity::Error,
            "a gate that cannot read a file"
        );
    }

    /// The #1081 arm. Asserted two ways round, because the defect was an absence: a corpus
    /// whose universal declarations do not parse has to be *named*, and one whose declarations
    /// are merely empty must not be — `Universal::empty()` is the state every corpus starts in,
    /// and reporting it would make this check fire on nearly every repository.
    #[test]
    fn declarations_that_do_not_parse_are_reported_and_an_absent_file_is_not() {
        let broken = crate::universal::Universal::parse("properties:\n  - name: \"unclosed\n");
        assert!(
            broken.malformed().is_some(),
            "the fixture has to be a parse failure for this case to be about anything"
        );
        let c = malformed_yaml(&[], &[], &[], &[], &broken);
        let named: Vec<&str> = c.violations.iter().map(|v| v.node.as_str()).collect();
        assert_eq!(named, vec![crate::universal::REL]);
        assert!(
            c.violations[0].detail.contains("does not parse as YAML"),
            "{:?}",
            c.violations[0].detail
        );

        // And the negative control, over the shapes a corpus legitimately has: no file at all,
        // a file declaring an empty list, and a file declaring something.
        for u in [
            crate::universal::Universal::empty(),
            crate::universal::Universal::parse(""),
            crate::universal::Universal::parse("properties: []\n"),
            crate::universal::Universal::parse(
                "properties:\n  - name: verdict\n    type: string\n",
            ),
        ] {
            let c = malformed_yaml(&[], &[], &[], &[], &u);
            assert!(c.violations.is_empty(), "{:?}", c.violations);
        }
    }

    /// The ordering promise the comment in [`malformed_yaml`] makes: `universal.yml` is
    /// appended, so adding it moved no finding a report already had.
    #[test]
    fn the_universal_arm_is_reported_last() {
        let c = malformed_yaml(
            &[Node::parse(
                PathBuf::from("c/x.yml"),
                "c/x.yml",
                "label: \"unclosed\n",
            )],
            &[],
            &[],
            &[decision("dec/bad.yml", "summary: \"unclosed\n")],
            &crate::universal::Universal::parse("properties:\n  - name: \"unclosed\n"),
        );
        let named: Vec<&str> = c.violations.iter().map(|v| v.node.as_str()).collect();
        assert_eq!(named, vec!["c/x.yml", "dec/bad.yml", crate::universal::REL]);
    }

    // ── findings-malformed ───────────────────────────────────────────────────

    /// Both populations, and the shape that makes this a second check: the two documents below
    /// parse — `Node::malformed` and `Source::malformed` are both `None` — so `malformed-yaml`
    /// reports neither of them.
    #[test]
    fn a_findings_block_that_does_not_read_is_reported_on_both_populations() {
        // An entry with an `id:` and nothing else is not a `Finding`.
        const BROKEN: &str = "yidam:\n  findings:\n    - id: 7f3a1c94b2e1\n";
        let bad_node = node("c/x.yml", &format!("class: c\n{BROKEN}"));
        let bad_entry = catalog(
            "cat/bad.md",
            &format!("---\nobtained: true\n{BROKEN}---\nb\n"),
        );
        assert_eq!(bad_node.malformed, None, "the instance parses");
        assert_eq!(bad_entry.malformed, None, "the entry parses");

        let c = findings_malformed(&[bad_node], &[bad_entry]);
        let named: Vec<&str> = c.violations.iter().map(|v| v.node.as_str()).collect();
        assert_eq!(named, vec!["c/x.yml", "cat/bad.md"]);
        assert!(
            c.violations[0].detail.starts_with("`yidam:` block"),
            "{:?}",
            c.violations[0].detail
        );
        assert_eq!(c.severity, Severity::Error);
    }

    /// The negative control, over the corpus every repository actually has: no block anywhere,
    /// and a block this tool wrote. A false positive here fires on every node in every corpus.
    #[test]
    fn a_corpus_whose_blocks_read_is_not_reported() {
        let written = crate::findings::add(
            "class: c\n",
            &crate::findings::Finding::open("orphan-in", "nothing links here", "4f2a1c9"),
        )
        .expect("the tool writes its own block");
        let c = findings_malformed(
            &[node("c/x.yml", "class: c\n"), node("c/y.yml", &written)],
            &[catalog("cat/ok.md", "---\nobtained: true\n---\nbody\n")],
        );
        assert!(c.violations.is_empty(), "{:?}", c.violations);
    }

    /// The state #1056 is about, asserted on the record rather than through the check: an
    /// unreadable header is not an entry that declared nothing, and every field it leaves
    /// behind reads as a claim the entry never made.
    #[test]
    fn an_unreadable_catalog_header_reads_as_obtained_and_governed_by_nothing() {
        let s = catalog(
            "cat/bad.md",
            "---\nobtained: false\nttl_days: \"30\n---\nbody\n",
        );
        assert!(s.malformed.is_some(), "the header does not parse");
        assert!(s.obtained, "`obtained: false` was written and is not read");
        assert_eq!(s.ttl_days, None, "the TTL the entry declared is gone");
    }

    /// A header that opens and never closes is the same empty record by a different route, and
    /// it is the one a truncated write leaves behind.
    #[test]
    fn an_unclosed_frontmatter_fence_is_reported() {
        let s = catalog("cat/cut.md", "---\nobtained: false\n");
        assert!(s.malformed.is_some());
        assert!(s.obtained);
    }

    /// No frontmatter at all is not a failure: the file has not contradicted anything, and the
    /// surfaces reading a skill or a seed through the same parser are entitled to that reading.
    #[test]
    fn a_file_with_no_frontmatter_is_not_malformed() {
        for text in ["", "\n", "# just prose\n", "Body with no header.\n"] {
            assert_eq!(catalog("cat/x.md", text).malformed, None, "{text:?}");
        }
    }

    /// A decision record whose bytes do not parse is the record the log renders under its file
    /// stem with an em dash — [`crate::corpus::DecisionRecord::id`] cannot tell the two apart,
    /// which is why the parse outcome has to be carried beside it.
    #[test]
    fn an_unreadable_decision_is_indistinguishable_from_an_unfilled_one() {
        let bad = decision("dec/phase-two.yml", "summary: \"unclosed\n");
        let empty = decision("dec/phase-three.yml", "");
        assert_eq!(bad.id(), "phase-two");
        assert_eq!(bad.decision.summary, None);
        assert_eq!(empty.malformed, None, "an empty file is the absent value");
        assert!(bad.malformed.is_some(), "and this one is not");
    }
}
