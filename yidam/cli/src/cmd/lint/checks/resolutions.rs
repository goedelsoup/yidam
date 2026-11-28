use super::*;

// ── Resolution annotations ───────────────────────────────────────────────────
//
// A resolution record is dated, and its `What remains open` describes the world on that
// date. The world moves, so PROTOCOL allows a dated annotation to be appended beneath an
// item that has moved — never an edit to the original.
//
// The risk that convention carries is not that an annotation is wrong. It is what an
// annotation structurally *is*: a place where one elector, in one commit, having read no
// `ma/*` tip and transported nothing, could perform a resolution in a file the protocol
// never routes through one. Article V puts synthesis in resolution events, so an
// annotation may record movement and never outcome.

/// One `> **Moved …**` annotation found in a resolution record.
pub struct Annotation {
    pub file: String,
    pub line: usize,
    /// The `## ` heading it sits under, or empty if it precedes the first one.
    pub section: String,
    pub text: String,
}

/// Find every annotation in a resolution record, with the section it sits under.
pub fn annotations_in(file: &str, text: &str) -> Vec<Annotation> {
    let mut section = String::new();
    let mut out = Vec::new();
    for (i, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if let Some(rest) = line.strip_prefix("## ") {
            section = rest.trim().to_string();
            continue;
        }
        // The annotation form PROTOCOL prescribes. Matched on the block-quote marker plus
        // the bold `Moved` opener, so ordinary prose mentioning the word is not a finding.
        if line.starts_with("> **Moved") {
            out.push(Annotation {
                file: file.to_string(),
                line: i + 1,
                section: section.clone(),
                text: line.to_string(),
            });
        }
    }
    out
}

/// `> **Moved YYYY-MM-DD.**` — an ISO date immediately after the word.
fn carries_iso_date(text: &str) -> bool {
    let Some(rest) = text.strip_prefix("> **Moved") else {
        return false;
    };
    let rest = rest.trim_start();
    let date: String = rest.chars().take(10).collect();
    if date.len() != 10 {
        return false;
    }
    let b = date.as_bytes();
    b[4] == b'-'
        && b[7] == b'-'
        && [0, 1, 2, 3, 5, 6, 8, 9]
            .iter()
            .all(|&i| b[i].is_ascii_digit())
}

pub const RESOLUTION_ANNOTATION_MALFORMED: CheckSpec = CheckSpec {
    id: "resolution-annotation-malformed",
    title: "Resolution annotation is malformed or in the wrong section",
    severity: Severity::Error,
    rationale: "Both halves are structural rather than a judgement about wording, which is why \
         this gates where its sibling does not. An annotation with no date cannot be read \
         against the record it extends — the whole point is that the record speaks for its \
         own date and the annotation for a later one. An annotation under `What was \
         resolved` or `What changed` edits history instead of extending it, and those \
         sections are the ones a reader trusts to describe a settled past.",
};

pub fn resolution_annotation_malformed(annotations: &[Annotation]) -> Check {
    let violations = annotations
        .iter()
        .filter_map(|a| {
            let node = format!("{}:{}", a.file, a.line);
            if !carries_iso_date(&a.text) {
                return Some(Violation::new(
                    node,
                    "no ISO date after `Moved` — an annotation carries the date of the \
                     movement, not of the resolution",
                ));
            }
            // Annotating anything but the open items edits the record rather than
            // extending it.
            if a.section != "What remains open" {
                return Some(Violation::new(
                    node,
                    format!(
                        "sits under `{}` — annotate `What remains open` and nowhere else",
                        if a.section.is_empty() {
                            "(no section)"
                        } else {
                            &a.section
                        }
                    ),
                ));
            }
            None
        })
        .collect();
    Check::new(&RESOLUTION_ANNOTATION_MALFORMED, violations)
}

/// Words that announce a decision rather than report a movement.
const DECIDING_WORDS: &[&str] = &[
    "resolved",
    "settled",
    "decided",
    "concluded",
    "agreed",
    "closes",
    "closed",
    "adopted",
    "rejected",
    "supersedes",
    "superseded",
];

pub const RESOLUTION_ANNOTATION_DECIDES: CheckSpec = CheckSpec {
    id: "resolution-annotation-decides",
    title: "Resolution annotation announces an outcome",
    severity: Severity::Warn,
    rationale: "An annotation may say a question was proposed, retrieved, measured or superseded \
         in fact; it may not say it was settled. Closing an open item is synthesis, and \
         Article V puts synthesis in resolution events — so an annotation that decides \
         something is a resolution performed by one elector, in one commit, in a file the \
         protocol never routes through a resolution. Warn rather than Error because this \
         is a heuristic over wording: it cannot see an outcome announced in words that are \
         not on the list, and gating on it would make every false positive a blocked \
         commit and the check would be switched off within a week.",
};

pub fn resolution_annotation_decides(annotations: &[Annotation]) -> Check {
    let violations = annotations
        .iter()
        .filter_map(|a| {
            let lower = a.text.to_lowercase();
            let hit = DECIDING_WORDS.iter().find(|w| {
                // Word-boundary-ish: avoid matching inside a longer word.
                lower
                    .split(|c: char| !c.is_ascii_alphabetic())
                    .any(|t| t == **w)
            })?;
            Some(Violation::new(
                format!("{}:{}", a.file, a.line),
                format!(
                    "`{hit}` announces an outcome — an annotation records movement only; \
                     an open item is closed by a later resolution"
                ),
            ))
        })
        .collect();
    Check::new(&RESOLUTION_ANNOTATION_DECIDES, violations)
}

pub const RESOLUTION_ELECTOR_UNREGISTERED: CheckSpec = CheckSpec {
    id: "resolution-elector-unregistered",
    title: "Resolution names a ma/* branch no elector row registers",
    severity: Severity::Error,
    rationale: "A resolution's authority rests on the seats it names, so a name the registry does \
         not carry is the one part of the record that cannot be read charitably — either the \
         seat was never registered, or it was removed and the records resting on it were \
         left behind. This gates where its sibling warns because it is set membership \
         between two committed files rather than a judgement about a missing field: there is \
         no state of the world in which the answer is arguable.",
};

/// A `ma/*` reference a resolution makes that `electors.md` does not register.
///
/// Both directions of the record point at seats: `tips:` says whose positions were read, and
/// `synthesized-by:` says who did the reading. Either naming a branch the registry does not
/// carry is a resolution standing on an elector that does not exist, and it is decidable —
/// set membership between a table and a frontmatter list, with no judgement in it.
///
/// Measured against the only derived repository running a sangha: 70 of 70 tips name a
/// registered branch, so this lands green there. It is a guard rather than a finding, and the
/// case it guards is a seat quietly leaving `electors.md` while the records that rest on it
/// stay. A registry is expected to keep historical seats for exactly that reason.
pub fn resolution_elector_unregistered(
    records: &[crate::cmd::sangha::Resolution],
    registered: &[String],
) -> Check {
    let mut violations = Vec::new();
    for r in records {
        // A tip is `ma/<elector>@<hash>`; the seat is the part before the `@`.
        let named = r
            .tips
            .iter()
            .map(|t| (t.split('@').next().unwrap_or(t), "read as a tip"))
            .chain(
                r.synthesized_by
                    .iter()
                    .map(|s| (s.as_str(), "named as the executor")),
            );
        for (branch, how) in named {
            if branch.is_empty() || registered.iter().any(|b| b == branch) {
                continue;
            }
            violations.push(Violation::new(
                r.file.clone(),
                format!("`{branch}` is {how} and `electors.md` registers no such branch"),
            ));
        }
    }
    Check::new(&RESOLUTION_ELECTOR_UNREGISTERED, violations)
}

pub const RESOLUTION_EXECUTOR_UNRECORDED: CheckSpec = CheckSpec {
    id: "resolution-executor-unrecorded",
    title: "Resolution record does not name the seat that executed it",
    // The declared level. `resolution_executor_unrecorded` raises it to Error where
    // `keys_bind_seats` holds; see that function for why the condition is the one it is.
    severity: Severity::Warn,
    rationale:
        "Article II governs weight and Article III governs record, and they answer different \
         questions. Naming the executor grants it nothing — no standing, no tiebreak, no \
         priority in any later resolution — which is why recording it costs the article \
         nothing. What it buys is that the most consequential actor in the event stops being \
         the one actor the provenance omits. The record already names the tips that were \
         read; the reader was the gap.",
};

/// A resolution record that does not say which seat executed it.
///
/// `synthesized-by` is the only field naming a *seat* rather than a branch that was read, and
/// it is the only thing that can distinguish one elector from another in the record. Nothing
/// in git can: in the repository that has run this protocol, all 126 commits across three
/// elector branches carry the operator's git author, and none of the 1,070 commits in it is
/// signed. The auditor's position and the owner's are told apart by a branch name and nothing
/// else.
///
/// **Warn, and the condition for escalating is written down rather than left to taste.** Every
/// record written before the field existed is this finding — 29 of 29 in that repository — and
/// they cannot be fixed. A resolution record is a dated document; retrofitting one means
/// somebody recalling which seat held the pen a month ago, and the corpus cannot recover it
/// mechanically because the git author is the same for every seat. Gating on a debt that
/// cannot be paid is how a gate gets switched off. This becomes an Error when that is no
/// longer true — when `electors.md` binds a distinct signing key per seat (RFC-0012), the
/// executor is recoverable from the commit and a missing field is a choice rather than an
/// inheritance.
///
/// **`keys_bind_seats` is that condition, decided rather than promised.** It is
/// [`super::super::attest::binds_distinct_key_per_seat`] — every registered seat binds a key and no
/// two bind the same one — so the escalation is an event with a commit that causes it, not a
/// sentence in a doc comment. It is false in every repository that has not opted into
/// collective mode, which is nearly all of them, and false in the measured one: the registry
/// binds no keys, so the 29 records written before the field existed stay a warning.
pub fn resolution_executor_unrecorded(
    records: &[crate::cmd::sangha::Resolution],
    keys_bind_seats: bool,
) -> Check {
    let violations = records
        .iter()
        .filter(|r| r.synthesized_by.is_empty())
        .map(|r| {
            Violation::new(
                r.file.clone(),
                "no `synthesized-by:` — the record names which tips were read and not who \
                 read them",
            )
        })
        .collect();
    let mut check = Check::new(&RESOLUTION_EXECUTOR_UNRECORDED, violations);
    if keys_bind_seats {
        check.severity = Severity::Error;
    }
    check
}

pub const RESOLUTION_DELIBERATION_UNRECORDED: CheckSpec = CheckSpec {
    id: "resolution-deliberation-unrecorded",
    title: "Resolution record does not name its rounds or the positions it read",
    severity: Severity::Warn,
    rationale:
        "Article III asks for ancestry, and ancestry is not only which commits were read: it \
         is which claims were contested and by whom. `positions:` names them and `rounds:` \
         says whether the electors read each other before the synthesis. A record carrying \
         both can be audited by someone who was not there; one without them can only be \
         believed. `rounds: 1` is a complete loop, so the fields cost nothing to write. A \
         record older than the fields warns, because nobody can recover a round count \
         mechanically. A record added after the repository's own PROTOCOL.md asked for them \
         gates, because there the omission was a choice.",
};

/// A resolution record that does not say how its deliberation went: no `rounds:`, or no
/// `positions:`.
///
/// PROTOCOL.md names both fields as what makes Article III *checkable rather than asserted*.
/// `tips:` says which commits were read; `positions:` says which claims were contested and by
/// whom, and `rounds:` says whether anyone read anyone else before the synthesis. Without them
/// a later reader can audit the record only by trusting its prose. **The fields' presence is
/// the point, not their size** — `rounds: 1` is a complete loop, and this check never asks for
/// more than one.
///
/// A `rounds:` that is not a count of at least one is reported as well. A loop that ran zero
/// times did not run, and `rounds: two` is a value no reader can compare; both are kept
/// verbatim by the parser so this finding can say *malformed* rather than *missing*.
///
/// **Warn, and Error for a record written after the repository's own PROTOCOL.md asked for
/// them.** `asked` is that set of records, decided by ancestry in
/// [`super::super::deliberation::asked`]: the commit that added the record descends from the commit
/// that put `rounds:` into the protocol's record format. Omitting the fields there was a
/// choice rather than an inheritance, so that finding gates. Every other record is the debt
/// `resolution-executor-unrecorded` describes: 32 of 32 in the repository that has run this
/// protocol, written from a vendored PROTOCOL.md without the fields. They cannot be fixed
/// mechanically — a round count is somebody's recollection — and gating on them is how a gate
/// gets switched off.
///
/// **Not decided by the record's other fields.** A record carrying `synthesized-by:` or
/// `independence:` looks newer than these two fields, but both can be added to an old record
/// afterward — `resolution-independence-mismatch` asks for exactly that. A tell a repair can
/// produce would turn an Info repair into an Error.
pub fn resolution_deliberation_unrecorded(
    records: &[crate::cmd::sangha::Resolution],
    asked: &std::collections::BTreeSet<String>,
) -> Check {
    let violations = records
        .iter()
        .filter_map(|r| {
            let mut gaps = Vec::new();
            if r.rounds.is_empty() {
                gaps.push("no `rounds:`".to_string());
            } else if !r.rounds.parse::<u32>().is_ok_and(|n| n >= 1) {
                gaps.push(format!(
                    "`rounds: {}` is not a count of at least one",
                    r.rounds
                ));
            }
            if r.positions.is_empty() {
                gaps.push("no `positions:`".to_string());
            }
            if gaps.is_empty() {
                return None;
            }
            let knew = asked.contains(&r.file);
            let why = if knew {
                " — the record was added after this repository's PROTOCOL.md asked for them"
            } else {
                " — the record names which tips were read and not how the deliberation went"
            };
            let v = Violation::new(r.file.clone(), format!("{}{why}", gaps.join(" and ")));
            Some(if knew { v.at(Severity::Error) } else { v })
        })
        .collect();
    Check::new(&RESOLUTION_DELIBERATION_UNRECORDED, violations)
}

#[cfg(test)]
mod resolution_record_tests {
    use super::*;
    use crate::cmd::sangha::Resolution;

    fn record(file: &str, by: &[&str], tips: &[&str]) -> Resolution {
        Resolution {
            file: file.to_string(),
            evolution: "e".to_string(),
            date: "2026-01-01".to_string(),
            tips: tips.iter().map(|t| t.to_string()).collect(),
            synthesized_by: by.iter().map(|b| b.to_string()).collect(),
            independence: String::new(),
            rounds: "1".to_string(),
            positions: vec!["positions/auditor-e.md".to_string()],
            branch_present: true,
        }
    }

    /// A record in the shape every one in the repository that has run this protocol takes:
    /// `evolution`, `date` and `tips`, and nothing that says how the deliberation went.
    fn predating(file: &str) -> Resolution {
        Resolution {
            rounds: String::new(),
            positions: vec![],
            ..record(file, &[], &["ma/auditor@a", "ma/advocate@b"])
        }
    }

    fn none() -> std::collections::BTreeSet<String> {
        std::collections::BTreeSet::new()
    }

    fn registered() -> Vec<String> {
        vec!["ma/auditor".to_string(), "ma/advocate".to_string()]
    }

    #[test]
    fn a_record_naming_registered_seats_passes_both() {
        let r = [record(
            "resolutions/e.md",
            &["ma/auditor"],
            &["ma/auditor@aaaaaaa", "ma/advocate@bbbbbbb"],
        )];
        assert!(resolution_elector_unregistered(&r, &registered()).passed());
        assert!(resolution_executor_unrecorded(&r, false).passed());
    }

    /// The tip carries `@<hash>`; the seat is what precedes it. Comparing the whole string
    /// would report every tip in every repository as unregistered.
    #[test]
    fn a_tip_is_matched_without_its_hash() {
        let r = [record("resolutions/e.md", &[], &["ma/auditor@aaaaaaa"])];
        let c = resolution_elector_unregistered(&r, &registered());
        assert!(c.passed(), "{c:#?}");
    }

    #[test]
    fn an_unregistered_executor_is_an_error() {
        let r = [record(
            "resolutions/e.md",
            &["ma/stranger"],
            &["ma/auditor@a"],
        )];
        let c = resolution_elector_unregistered(&r, &registered());
        assert_eq!(c.violations.len(), 1, "{c:#?}");
        assert!(c.violations[0].detail.contains("ma/stranger"), "{c:#?}");
        assert!(
            c.violations[0].detail.contains("executor"),
            "the finding does not say which half named it: {:?}",
            c.violations[0].detail
        );
    }

    /// Both halves of the record point at seats, and a check that read only one would pass a
    /// resolution standing on a position nobody registered.
    #[test]
    fn an_unregistered_tip_is_an_error_too() {
        let r = [record("resolutions/e.md", &["ma/auditor"], &["ma/ghost@a"])];
        let c = resolution_elector_unregistered(&r, &registered());
        assert_eq!(c.violations.len(), 1, "{c:#?}");
        assert!(c.violations[0].detail.contains("tip"), "{c:#?}");
    }

    /// A joint synthesis names every seat, and every one of them is checked.
    #[test]
    fn each_seat_of_a_joint_synthesis_is_checked() {
        let r = [record(
            "resolutions/e.md",
            &["ma/auditor", "ma/stranger"],
            &[],
        )];
        let c = resolution_elector_unregistered(&r, &registered());
        assert_eq!(c.violations.len(), 1, "{c:#?}");
        assert!(c.violations[0].detail.contains("ma/stranger"));
    }

    /// Every record written before the field existed. It warns and does not gate — see the
    /// rationale on the check for the condition under which that changes.
    #[test]
    fn a_record_naming_no_executor_warns_and_does_not_gate() {
        let r = [record("resolutions/e.md", &[], &["ma/auditor@a"])];
        let c = resolution_executor_unrecorded(&r, false);
        assert_eq!(c.violations.len(), 1, "{c:#?}");
        assert_eq!(c.severity, Severity::Warn);
        // …and the *other* check is silent about it: a record that names no seat has named
        // no unregistered one, and reporting it twice would double-count one gap.
        assert!(resolution_elector_unregistered(&r, &registered()).passed());
    }

    /// The escalation RFC-0012 arms: the same records, the same finding, and it gates once
    /// the registry binds a distinct key per seat — because the executor is then recoverable
    /// from the commit and a missing field is a choice rather than an inheritance.
    ///
    /// The condition is [`super::super::super::attest::binds_distinct_key_per_seat`]; this asserts
    /// only that the severity follows it, which is the half that lives here.
    #[test]
    fn a_registry_binding_a_key_per_seat_makes_it_gate() {
        let r = [record("resolutions/e.md", &[], &["ma/auditor@a"])];
        let c = resolution_executor_unrecorded(&r, true);
        assert_eq!(c.violations.len(), 1, "{c:#?}");
        assert_eq!(c.severity, Severity::Error, "{c:#?}");
    }

    /// A record naming its rounds and positions passes, and one round is enough: the
    /// protocol's loop ends when a round adds nothing, and the check is not a quota.
    #[test]
    fn a_record_naming_one_round_and_its_positions_passes() {
        let c =
            resolution_deliberation_unrecorded(&[record("resolutions/e.md", &[], &[])], &none());
        assert!(c.passed(), "{c:#?}");
    }

    /// The baseline #592 was filed on: 0 of 29 records carried either field, and 0 of 32 do
    /// now. Every one is named, each once, and none of them gates — the fields postdate the
    /// vendored protocol those records were written from.
    #[test]
    fn records_predating_the_fields_are_each_named_and_warn() {
        let r = [predating("resolutions/a.md"), predating("resolutions/b.md")];
        let c = resolution_deliberation_unrecorded(&r, &none());
        assert_eq!(c.violations.len(), 2, "{c:#?}");
        assert_eq!(c.severity, Severity::Warn);
        for v in &c.violations {
            assert_eq!(v.severity, None, "a predating record gated: {v:?}");
            assert!(v.detail.contains("no `rounds:`"), "{v:?}");
            assert!(v.detail.contains("no `positions:`"), "{v:?}");
        }
    }

    /// Either field alone is the finding, and the finding says which one.
    #[test]
    fn each_field_is_asked_for_on_its_own() {
        let no_positions = Resolution {
            positions: vec![],
            ..record("resolutions/p.md", &[], &[])
        };
        let no_rounds = Resolution {
            rounds: String::new(),
            ..record("resolutions/r.md", &[], &[])
        };
        let c = resolution_deliberation_unrecorded(&[no_positions, no_rounds], &none());
        assert_eq!(c.violations.len(), 2, "{c:#?}");
        assert!(
            c.violations[0].detail.starts_with("no `positions:`"),
            "{c:#?}"
        );
        assert!(!c.violations[0].detail.contains("rounds"), "{c:#?}");
        assert!(c.violations[1].detail.starts_with("no `rounds:`"), "{c:#?}");
    }

    /// A loop that ran zero times did not run, and a count that is not a number cannot be
    /// compared. Both are named as malformed rather than read as absent.
    #[test]
    fn a_rounds_value_that_is_not_a_count_is_named() {
        for bad in ["0", "two", "-1", "1.5"] {
            let r = Resolution {
                rounds: bad.to_string(),
                ..record("resolutions/e.md", &[], &[])
            };
            let c = resolution_deliberation_unrecorded(&[r], &none());
            assert_eq!(c.violations.len(), 1, "{bad}: {c:#?}");
            assert!(
                c.violations[0]
                    .detail
                    .contains(&format!("`rounds: {bad}` is not a count")),
                "{bad}: {c:#?}"
            );
        }
    }

    /// A record added after the repository's PROTOCOL.md asked for the fields gates; one added
    /// before warns. The check stays Warn, so a consumer must read the finding's own level.
    #[test]
    fn a_record_added_after_the_protocol_asked_gates() {
        let asked: std::collections::BTreeSet<String> = ["resolutions/new.md".to_string()].into();
        let c = resolution_deliberation_unrecorded(
            &[
                predating("resolutions/new.md"),
                predating("resolutions/old.md"),
            ],
            &asked,
        );
        assert_eq!(c.severity, Severity::Warn, "{c:#?}");
        let levels: Vec<_> = c.violations.iter().map(|v| v.severity).collect();
        assert_eq!(levels, [Some(Severity::Error), None], "{c:#?}");
        assert!(c.violations[0].detail.contains("asked for them"), "{c:#?}");
    }

    /// The tell that was rejected: a record carrying `synthesized-by:` and `independence:` is
    /// not thereby newer, because both can be added to an old record afterward. Only
    /// ancestry decides.
    #[test]
    fn a_newer_field_on_an_old_record_does_not_gate() {
        let retrofitted = Resolution {
            synthesized_by: vec!["ma/auditor".to_string()],
            independence: "distinct-seats".to_string(),
            ..predating("resolutions/old.md")
        };
        let c = resolution_deliberation_unrecorded(&[retrofitted], &none());
        assert_eq!(c.violations.len(), 1, "{c:#?}");
        assert_eq!(c.violations[0].severity, None, "{c:#?}");
    }

    /// A repository with no sangha has no records, and both checks report that they ran.
    #[test]
    fn no_records_is_not_a_finding() {
        assert!(resolution_deliberation_unrecorded(&[], &none()).passed());
        assert!(resolution_elector_unregistered(&[], &[]).passed());
        assert!(resolution_executor_unrecorded(&[], false).passed());
        // Escalated or not, a check with nothing to report reports nothing.
        assert!(resolution_executor_unrecorded(&[], true).passed());
    }
}

#[cfg(test)]
mod annotation_tests {
    use super::*;

    const WELL_FORMED: &str = "\
# local-infrastructure

## What was resolved

The project and site classes are adopted.

## What remains open

Whether `body` splits into fields.

> **Moved 2026-08-18.** A budget proposal now exists on `ma/goedelsoup`, measured over
> six commits. One branch holds it, so it is a proposal rather than a tension.

Whether block-quoted host error text should be indexed.
";

    fn annos(text: &str) -> Vec<Annotation> {
        annotations_in("resolutions/x.md", text)
    }

    #[test]
    fn a_well_formed_annotation_passes_both_checks() {
        let a = annos(WELL_FORMED);
        assert_eq!(a.len(), 1);
        assert_eq!(a[0].section, "What remains open");
        assert!(resolution_annotation_malformed(&a).passed());
        assert!(resolution_annotation_decides(&a).passed());
    }

    /// The date is what lets a reader tell the record's own date from the movement's.
    #[test]
    fn an_annotation_without_a_date_is_an_error() {
        let a = annos("## What remains open\n\n> **Moved.** Somebody proposed it.\n");
        let c = resolution_annotation_malformed(&a);
        assert_eq!(c.violations.len(), 1);
        assert!(
            c.violations[0].detail.contains("ISO date"),
            "{:?}",
            c.violations[0].detail
        );
    }

    #[test]
    fn a_non_iso_date_is_an_error() {
        let a = annos("## What remains open\n\n> **Moved 18/08/2026.** Proposed.\n");
        assert_eq!(resolution_annotation_malformed(&a).violations.len(), 1);
    }

    /// Annotating a settled section edits history rather than extending it.
    #[test]
    fn an_annotation_under_what_was_resolved_is_an_error() {
        let a = annos("## What was resolved\n\n> **Moved 2026-08-18.** It was reopened.\n");
        let c = resolution_annotation_malformed(&a);
        assert_eq!(c.violations.len(), 1);
        assert!(
            c.violations[0].detail.contains("What was resolved"),
            "{:?}",
            c.violations[0].detail
        );
    }

    /// The one that matters: an annotation performing a resolution.
    #[test]
    fn an_annotation_announcing_an_outcome_is_warned() {
        let a = annos(
            "## What remains open\n\n> **Moved 2026-08-18.** This is now settled: the \
             bodies stay indexed.\n",
        );
        let c = resolution_annotation_decides(&a);
        assert_eq!(c.violations.len(), 1);
        assert!(
            c.violations[0].detail.contains("settled"),
            "{:?}",
            c.violations[0].detail
        );
        // Structurally fine — it is the wording that offends.
        assert!(resolution_annotation_malformed(&a).passed());
    }

    /// Reporting a movement in fact is exactly what the convention is for, even when the
    /// movement happens to be a supersession that occurred elsewhere.
    #[test]
    fn reporting_a_movement_is_not_announcing_an_outcome() {
        let a = annos(
            "## What remains open\n\n> **Moved 2026-08-19.** Measured at 714 bytes per \
             document and recorded on `ma/auditor`.\n",
        );
        assert!(resolution_annotation_decides(&a).passed());
    }

    /// A deciding word inside a longer word is not a hit.
    #[test]
    fn the_word_list_respects_boundaries() {
        let a = annos("## What remains open\n\n> **Moved 2026-08-19.** Unresolvedness aside.\n");
        assert!(resolution_annotation_decides(&a).passed());
    }

    /// Prose that merely mentions the word is not an annotation.
    #[test]
    fn only_the_prescribed_form_is_read_as_an_annotation() {
        let a = annos("## What remains open\n\nNothing has moved since this was written.\n");
        assert!(a.is_empty());
    }

    /// A repository with no sangha has no annotations and no findings.
    #[test]
    fn no_annotations_is_clean() {
        assert!(resolution_annotation_malformed(&[]).passed());
        assert!(resolution_annotation_decides(&[]).passed());
    }
}
