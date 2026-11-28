use super::*;

/// Can every corpus file be read at all?
///
/// **The question under all the others.** A file that does not parse reaches a reader as an
/// *empty record*, and everything computed from it is then a statement about the emptiness
/// rather than about the file. `lint` learned this in #676 and `graph-check` in #721; this is
/// the same defect on the surface a person runs first when something is wrong, and where it
/// showed up as saying nothing at all.
///
/// **It does not re-report what `lint` reports.** One line and a count, with `yidam lint` as
/// the remedy — that command names the files and is the place the finding belongs. What
/// `doctor` adds is that the question gets *asked* here, so a corpus nothing can read is not
/// something you discover by noticing that a different command has gone quiet.
///
/// **All four records, since #1081.** It counted instances and classes, and said
/// `N file(s), all readable` on a repository with an unreadable catalog entry or decision
/// record — which is the defect it exists to report, committed by the report. A count that
/// leaves a population out is a clean bill of health over that population, and this line is the
/// one a person reads first. The wording moved with it: `corpus file(s)`, said only about what
/// is counted.
///
/// Silent on a corpus with no files in it, which is every repository between `bootstrap` and
/// the first node: `Ok` with `no corpus files yet` says the question was put and had no
/// subject, which is not the same as a clean bill of health over nothing.
pub(super) fn check_corpus(root: &Path) -> Answer {
    let read = crate::corpus::Corpus::open(root);
    // All four, because the line says `corpus file(s)` and those are the four records a yidam
    // repository is written in. Counting two of them said `all readable` over a corpus with an
    // unreadable catalog entry or decision record — the #1081 half of the same defect, one
    // level up: a count that excludes a population is a clean bill of health over it.
    let read_outcomes: Vec<bool> = read
        .nodes()
        .iter()
        .map(|n| n.malformed.is_some())
        .chain(read.classes().iter().map(|c| c.malformed.is_some()))
        .chain(read.sources().iter().map(|s| s.malformed.is_some()))
        .chain(read.decisions().iter().map(|d| d.malformed.is_some()))
        .collect();
    let total = read_outcomes.len();
    if total == 0 {
        return Answer::ok("no corpus files yet");
    }
    let unreadable = read_outcomes.iter().filter(|bad| **bad).count();

    match unreadable {
        0 => Answer::ok(format!("{total} corpus file(s), all readable")),
        n => Answer::fail(
            format!("{n} of {total} corpus file(s) do not parse"),
            Some("yidam lint"),
        ),
    }
}

/// Has the ontology said what its classes require (#1078)?
///
/// **The whole ontology, not one class.** Every contract check takes its severity from the
/// class's own declaration, which is right per check: `missing-property` gates only on
/// `required: true`, and `unlicensed-edge` only on `edge_policy: exhaustive`. Many classes
/// require nothing, and saying so per class would be a report that is never empty. But when
/// *no* class in an ontology writes `required:`, and *no* class writes `edge_policy:`, both
/// checks can warn there and never gate. Nothing said so. That is an ontology nobody asked,
/// and it is usually one written before the fields existed.
///
/// **Writing the key is the answer, whatever its value.** `required: false` on every property
/// is a decision, so this reads [`crate::corpus::ClassProperty::declared_required`] and not
/// `required()`. `edge_policy: characteristic` is a decision for the same reason. The two
/// questions are reported apart, because a corpus can answer one and not the other.
///
/// Measured on 2026-09-29 over the 16 derived ontologies on one host: 11 answered neither
/// question, 3 answered only `required:`, and 2 answered both. ohio-education-funding, the
/// corpus that reported this, answered only `edge_policy:` until it wrote `required:` by hand
/// that morning. A `warn` and not a `fail`: nothing is broken, and no derived repository runs
/// `doctor` in CI.
pub(super) fn check_contract(root: &Path) -> Answer {
    let corpus = crate::corpus::Corpus::open(root);
    contract_answer(corpus.classes())
}

/// [`check_contract`] over classes already read, so the reading can be tested without a disk.
///
/// A class that did not parse reads as one with no properties and no edges, so it adds to
/// neither question and cannot make either one warn. `corpus` is the line that fails on it.
pub(super) fn contract_answer(classes: &[crate::corpus::Class]) -> Answer {
    use crate::corpus::EdgePolicy;
    const WHERE: &str = "see `The class contract` in .yidam/.vendor/prelude/GRAPH.md";

    if classes.is_empty() {
        return Answer::ok("no classes yet");
    }
    let properties: Vec<_> = classes.iter().flat_map(|c| &c.properties).collect();
    let answered_required = properties
        .iter()
        .filter(|p| p.declared_required.is_some())
        .count();
    // Only a class with edges has a policy to state: `unlicensed-edge` reads nothing else.
    let with_edges: Vec<_> = classes.iter().filter(|c| !c.edges.is_empty()).collect();
    let answered_policy = with_edges
        .iter()
        .filter(|c| c.edge_policy != EdgePolicy::Unstated)
        .count();

    let (p, e) = (properties.len(), with_edges.len());
    // A question with no subject is not asked, so it is neither open nor adopted.
    let mut offers = vec![];
    if p > 0 {
        offers.push(if answered_required > 0 {
            Offered::adopted("contract-required")
        } else {
            Offered::open(
                "contract-required",
                format!(
                    "no property says `required:` ({p} declared) — `missing-property` can warn \
                     here and never gate"
                ),
                format!("write `required:` on each property; {WHERE}"),
            )
        });
    }
    if e > 0 {
        offers.push(if answered_policy > 0 {
            Offered::adopted("contract-edge-policy")
        } else {
            Offered::open(
                "contract-edge-policy",
                format!(
                    "no class says `edge_policy:` ({e} with edges) — `unlicensed-edge` can warn \
                     here and never gate"
                ),
                format!("write `edge_policy:` on each class; {WHERE}"),
            )
        });
    }
    let held = format!(
        "`required:` on {answered_required} of {p} property declaration(s), \
         `edge_policy:` on {answered_policy} of {e} class(es) with edges"
    );
    if offers.is_empty() {
        return Answer::ok(held);
    }
    Answer::offers(held, offers)
}

/// Have any source records aged past what the corpus said they may?
///
/// **No network, and none is possible from here.** This reads the entry's own `retrieved:`,
/// or the commit that last touched its file, against a TTL the corpus declared. It cannot
/// say the upstream changed and does not claim to — it says nobody has looked.
///
/// Silent where no TTL applies, which is every corpus that has not asked. A `doctor` line
/// saying "0 expired" on a repository that never declared a TTL would read as a clean bill of
/// health on a question nobody put.
pub(super) fn check_catalog(root: &Path, today: i64) -> Answer {
    let dir = crate::paths::yidam_catalog_dir(root);
    let sources =
        crate::corpus::load_sources(root, &crate::walk::walk_md_files(&dir), &Default::default());
    let default_ttl = crate::config::load_yidam_config(root)
        .map(|c| c.catalog.ttl_days)
        .unwrap_or_default();
    let iso = crate::cmd::export::unix_to_iso(today as u64 * 86_400);
    let ages = crate::cmd::lint::ttl::ages(
        &sources,
        &crate::cmd::lint::ttl::committed_dates(root, &dir),
        default_ttl,
        iso.split('T').next().unwrap_or_default(),
    );

    let governed = ages.iter().filter(|a| a.ttl_days.is_some()).count();
    if governed == 0 {
        return Answer::ok(format!(
            "no TTL declared — {} source(s) never expire. Set `[catalog] ttl_days` or declare \
             `ttl_days:` on an entry.",
            ages.len()
        ));
    }
    let expired: Vec<&crate::cmd::lint::ttl::Age> =
        ages.iter().filter(|a| a.overdue_days().is_some()).collect();
    let undatable = ages.iter().filter(|a| a.undatable()).count();
    if expired.is_empty() && undatable == 0 {
        return Answer::ok(format!("{governed} source(s) under a TTL, none expired"));
    }
    let mut detail = Vec::new();
    if let Some(worst) = expired
        .iter()
        .max_by_key(|a| a.overdue_days().unwrap_or_default())
    {
        detail.push(format!(
            "{} of {governed} source(s) expired, worst {} day(s) past ({})",
            expired.len(),
            worst.overdue_days().unwrap_or_default(),
            worst.entry
        ));
    }
    if undatable > 0 {
        detail.push(format!(
            "{undatable} under a TTL with no date to measure against"
        ));
    }
    Answer::warn(
        detail.join("; "),
        Some("yidam lint  # catalog-expired names each one"),
    )
}

#[cfg(test)]
mod tests {
    use super::super::tests::*;
    use super::*;
    use tempfile::TempDir;

    // ── #721: a corpus file that does not parse ─────────────────────────────────

    /// A corpus of one class and one instance, with control over both files' bytes.
    fn repo_with_corpus(instance: &str, schema: &str) -> TempDir {
        let tmp = derived_repo();
        let class = tmp.path().join(".yidam/corpus/gage");
        std::fs::create_dir_all(&class).unwrap();
        std::fs::write(tmp.path().join(".yidam/corpus/gage.ont.yml"), schema).unwrap();
        std::fs::write(class.join("canyon-outlet.yml"), instance).unwrap();
        tmp
    }

    const SOUND: &str = "class: gage\nlabel: canyon-outlet\n";
    /// The same bytes with one unclosed quote in `label:`.
    const BROKEN: &str = "class: gage\nlabel: \"canyon-outlet\n";

    /// **The arm that said nothing at all.** `doctor` is what a person runs first when
    /// something is wrong, and a corpus file nothing can read was invisible to it — the
    /// defect #676 fixed in `lint` and #721 in `graph-check`, on the surface where it read
    /// as a clean bill of health.
    #[test]
    fn a_corpus_file_that_does_not_parse_is_a_finding() {
        let sound = repo_with_corpus(SOUND, SOUND);
        let c = check_corpus(sound.path());
        assert_eq!(c.verdict, Verdict::Ok, "{}", c.detail);
        assert!(c.detail.contains('2'), "both files counted: {}", c.detail);

        for (instance, schema) in [(BROKEN, SOUND), (SOUND, BROKEN)] {
            let broken = repo_with_corpus(instance, schema);
            let c = check_corpus(broken.path());
            assert_eq!(c.verdict, Verdict::Fail, "{}", c.detail);
            assert!(c.detail.starts_with("1 of 2"), "{}", c.detail);
            // The remedy is the command that names the files. `doctor` counts; it does not
            // re-report what `lint` reports.
            assert_eq!(c.remedy.as_deref(), Some("yidam lint"));
        }
    }

    /// **The two records the count left out (#1081).** With `nodes` and `classes` sound, an
    /// unreadable catalog entry or decision record was counted by nothing, so the line read
    /// `2 file(s), all readable` on a corpus with a file in it that nothing could read. Asserted
    /// one population at a time, because a total that happens to move says nothing about which
    /// arm moved it — the shape [`crate::corpus::Source`] and
    /// [`crate::corpus::DecisionRecord`] had in `lint` before #1056.
    #[test]
    fn an_unreadable_catalog_entry_or_decision_record_is_counted() {
        // The sound corpus is two files; each case below adds one more.
        for (rel, bytes) in [
            (
                ".yidam/catalog/pearl-2009.md",
                "---\nttl_days: \"30\n---\nbody\n",
            ),
            (".yidam/decisions/0001-a.yml", "summary: \"unclosed\n"),
        ] {
            let tmp = repo_with_corpus(SOUND, SOUND);
            let path = tmp.path().join(rel);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, bytes).unwrap();

            let c = check_corpus(tmp.path());
            assert_eq!(c.verdict, Verdict::Fail, "{rel}: {}", c.detail);
            assert!(c.detail.starts_with("1 of 3"), "{rel}: {}", c.detail);
        }
    }

    /// And the negative control for the same widening: a catalog entry and a decision record
    /// that *do* read are counted and not reported. A count is only evidence if both answers
    /// move with the file.
    #[test]
    fn catalog_entries_and_decision_records_that_read_are_counted_and_silent() {
        let tmp = repo_with_corpus(SOUND, SOUND);
        for (rel, bytes) in [
            (
                ".yidam/catalog/pearl-2009.md",
                "---\nobtained: true\n---\nbody\n",
            ),
            (".yidam/decisions/0001-a.yml", "id: d1\nsummary: fine\n"),
        ] {
            let path = tmp.path().join(rel);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, bytes).unwrap();
        }
        let c = check_corpus(tmp.path());
        assert_eq!(c.verdict, Verdict::Ok, "{}", c.detail);
        assert_eq!(c.detail, "4 corpus file(s), all readable");
    }

    /// A repository between `bootstrap` and its first node has no corpus files, and that is
    /// not a clean bill of health over nothing — it is a question with no subject.
    #[test]
    fn a_corpus_with_no_files_yet_is_not_graded() {
        let tmp = derived_repo();
        let c = check_corpus(tmp.path());
        assert_eq!(c.verdict, Verdict::Ok);
        assert_eq!(c.detail, "no corpus files yet");
        assert!(c.remedy.is_none());
    }

    /// matt-huffman's shape: properties and edges, and neither question answered anywhere.
    #[test]
    fn an_ontology_that_answers_neither_question_warns_about_both() {
        let classes = [
            class("metric", &format!("{}{EDGE}", property(""))),
            class("place", "properties: []\n"),
        ];
        let a = contract_answer(&classes);
        assert_eq!(a.verdict, Verdict::Warn);
        assert!(a.detail.contains("`required:`"), "{}", a.detail);
        assert!(a.detail.contains("`edge_policy:`"), "{}", a.detail);
        let remedy = a.remedy.unwrap();
        assert!(remedy.contains("GRAPH.md"), "{remedy}");
    }

    /// ohio-education-funding before its `0870a3fd`: `characteristic` on every class and no
    /// `required:` anywhere. The issue's own proposal, "no class requires anything *and* no
    /// class states a policy", would have been silent here — on the corpus that reported it.
    #[test]
    fn the_two_questions_are_reported_apart() {
        let edges_answered = [class(
            "metric",
            &format!("{}{EDGE}edge_policy: characteristic\n", property("")),
        )];
        let a = contract_answer(&edges_answered);
        assert_eq!(a.verdict, Verdict::Warn);
        assert!(a.detail.contains("`missing-property`"), "{}", a.detail);
        assert!(!a.detail.contains("`unlicensed-edge`"), "{}", a.detail);

        let required_answered = [class(
            "metric",
            &format!("{}{EDGE}", property("    required: true\n")),
        )];
        let a = contract_answer(&required_answered);
        assert_eq!(a.verdict, Verdict::Warn);
        assert!(a.detail.contains("`unlicensed-edge`"), "{}", a.detail);
        assert!(!a.detail.contains("`missing-property`"), "{}", a.detail);
    }

    /// Writing the key is the answer. `required: false` gates nothing, the same as silence,
    /// but it is a decision — ohio-education-funding writes it on 7 of its 120 properties,
    /// each with a reason. Reading [`crate::corpus::ClassProperty::required`] here instead
    /// of what was written would warn at a corpus that did the work.
    #[test]
    fn required_false_is_an_answer() {
        let classes = [class(
            "metric",
            &format!(
                "{}{EDGE}edge_policy: characteristic\n",
                property("    required: false\n")
            ),
        )];
        let a = contract_answer(&classes);
        assert_eq!(a.verdict, Verdict::Ok, "{}", a.detail);
        assert!(a.remedy.is_none());
        assert!(!classes[0].properties[0].required());
    }

    /// One class answering is enough. The signal is the ontology, and a class that requires
    /// nothing beside one that requires something is a corpus that was asked.
    #[test]
    fn one_class_answering_answers_for_the_ontology() {
        let classes = [
            class(
                "metric",
                &format!("{}{EDGE}", property("    required: true\n")),
            ),
            class(
                "place",
                &format!("{}{EDGE}edge_policy: exhaustive\n", property("")),
            ),
        ];
        assert_eq!(contract_answer(&classes).verdict, Verdict::Ok);
    }

    /// A question with no subject is not unanswered. No property means nothing to require,
    /// and no edge means `unlicensed-edge` has nothing to read a policy for.
    #[test]
    fn a_question_with_no_subject_is_not_asked() {
        assert_eq!(contract_answer(&[]).verdict, Verdict::Ok);
        let bare = [class("place", "description: A place.\n")];
        assert_eq!(contract_answer(&bare).verdict, Verdict::Ok);
    }

    /// Adopted and declined at once: adopted wins, and the contradiction is noted.
    #[test]
    fn an_adopted_offer_that_is_also_declined_reads_ok_with_a_note() {
        let contract = [class(
            "metric",
            &format!(
                "{}{EDGE}edge_policy: characteristic\n",
                property("    required: true\n")
            ),
        )];
        let (_d, declines) = declining(&["0003-x"], &[("contract-required", "0003-x")], None);
        let a = contract_answer(&contract).settle(&declines);
        assert_eq!(a.verdict, Verdict::Ok, "{}", a.detail);
        assert!(
            a.detail
                .contains("`[doctor.declined] contract-required` is contradicted"),
            "{}",
            a.detail
        );
    }

    /// An undecided offer is dated, against the pin when there is one.
    #[test]
    fn an_open_offer_is_dated() {
        let a = contract_answer(&[class("metric", &property(""))]);
        assert!(
            a.detail.ends_with("offered since cli/v0.17.0"),
            "{}",
            a.detail
        );
    }
}
