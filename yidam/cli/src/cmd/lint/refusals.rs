//! A node's declared refusals, held to its own prose (RFC-0045 §3.3), and proposed where a corpus
//! asks (RFC-0057).
//!
//! `refuses:` quotes the sentence in which a node declines an inference. `derive check` owes an
//! answer to every refusal in a paragraph an argument cites, and finds that paragraph by the
//! quote — so a quote that no longer matches the prose owes nothing to anyone, and the argument
//! that routed around the refusal passes. `refusal-span-drift` keeps the declaration and the prose
//! saying the same thing. It reads the node alone; no artifact needs to exist for it to fire.
//!
//! `not_refusals:` is the other direction of a ruling: a paragraph a person read and ruled
//! declines nothing. The same check holds it to the prose. A ruling is per paragraph, so a span on
//! either list must sit inside exactly one, and one paragraph cannot be ruled both ways.
//!
//! `refusal-undeclared` runs `derive`'s proposer over every paragraph of every node, not only the
//! cited ones, and names each flagged paragraph that neither list rules on. It is Info and opt-in:
//! the proposer is wrong four times in ten, and deciding is a person's ruling.

use std::collections::BTreeSet;

use crate::cmd::lint::citations::{flatten, truncate};
use crate::cmd::lint::model::{Check, Severity, Violation};
use crate::corpus::Node;
use crate::derive::{
    in_prose, not_refusal_spans, paragraphs_holding, proposes_refusal, prose_fields, refusal_spans,
    ruled_paragraph, Field,
};
use crate::universal::Universal;

pub const SPAN_DRIFT: &str = "refusal-span-drift";
pub const UNDECLARED: &str = "refusal-undeclared";

/// What one declared span says about the prose it quotes, or `None` when it sits in one paragraph.
fn span_problem(fields: &[Field], span: Option<&str>, list: &str) -> Option<String> {
    let span = span.map(str::trim).unwrap_or_default();
    if span.is_empty() {
        return Some(match list {
            "refuses" => "declares a refusal with no `span:` — quote the sentence that refuses, \
                          or `derive check` has no paragraph to hold an argument to"
                .to_string(),
            _ => "declares a ruled non-refusal with no `span:` — quote the paragraph's sentence, \
                  or the ruling silences nothing"
                .to_string(),
        });
    }
    if !in_prose(fields, span) {
        return Some(match list {
            "refuses" => format!(
                "declares a refusal its own prose no longer says — reread the node and decide \
                 whether it still refuses; re-quoting whatever is there now to clear this \
                 discards the refusal silently. Span: {}",
                truncate(span)
            ),
            _ => format!(
                "declares a ruled non-refusal its own prose no longer says — reread the paragraph \
                 and rule on it again; the proposer will flag it until a span holds. Span: {}",
                truncate(span)
            ),
        });
    }
    let held: BTreeSet<String> = paragraphs_holding(fields, span).into_iter().collect();
    match held.len() {
        0 => Some(format!(
            "quotes a `{list}:` span that crosses a blank line — a ruling is per paragraph, so \
             quote a sentence inside one. Span: {}",
            truncate(span)
        )),
        1 => None,
        n => Some(format!(
            "quotes a `{list}:` span found in {n} paragraphs — a ruling is per paragraph, so \
             quote enough of the sentence to name one; until then it rules on none. Span: {}",
            truncate(span)
        )),
    }
}

/// Every declared span on either list that is absent, gone from the prose, or across a blank
/// line, and every paragraph ruled both a refusal and not one.
pub fn refusal_span_drift(nodes: &[Node]) -> Check {
    let violations = nodes
        .iter()
        .flat_map(|n| {
            let refusals = n.inst.refuses.as_deref().unwrap_or_default();
            let ruled = n.inst.not_refusals.as_deref().unwrap_or_default();
            if refusals.is_empty() && ruled.is_empty() {
                return Vec::new();
            }
            let fields = prose_fields(&n.text);
            let mut out: Vec<Violation> = refusals
                .iter()
                .map(|r| span_problem(&fields, r.span.as_deref(), "refuses"))
                .chain(
                    ruled
                        .iter()
                        .map(|r| span_problem(&fields, r.span.as_deref(), "not_refusals")),
                )
                .flatten()
                .map(|detail| Violation::new(&n.rel, detail))
                .collect();
            // Each span's one paragraph: an ambiguous span is reported above and rules on none,
            // so a short span repeated elsewhere is not read as a second ruling there.
            let refused: BTreeSet<String> = refusal_spans(n)
                .into_iter()
                .filter_map(|s| ruled_paragraph(&fields, s))
                .collect();
            let both: BTreeSet<String> = not_refusal_spans(n)
                .into_iter()
                .filter_map(|s| ruled_paragraph(&fields, s))
                .filter(|p| refused.contains(p))
                .collect();
            out.extend(both.iter().map(|p| {
                Violation::new(
                    &n.rel,
                    format!(
                        "rules one paragraph both a refusal and not one — it holds a `refuses:` \
                         span and a `not_refusals:` span; keep the ruling that is true and drop \
                         the other. Paragraph: {}",
                        truncate(p)
                    ),
                )
            }));
            out
        })
        .collect();
    Check::new(
        SPAN_DRIFT,
        "A declared refusal or non-refusal quotes text its own node no longer holds",
        Severity::Error,
        "`agent-conduct.md`: an outbound claim does not route around a refusal in the block it \
         cites. `derive check` can only hold an argument to that when the refusal is declared, \
         and it finds the paragraph a refusal governs by the `refuses:` span — so a span the \
         prose no longer contains governs nothing, and every argument resting on that paragraph \
         is released from it with no finding anywhere. A `not_refusals:` span is held the same \
         way, because a ruling that quotes nothing silences nothing and the paragraph is proposed \
         again. A ruling is per paragraph, so a span across a blank line, a span found in more \
         than one paragraph, or one paragraph ruled both ways, is reported too (RFC-0057). Error, under the baseline ratchet: the repair is \
         in this corpus and is one edit. Whitespace is normalized and nothing else is, which is \
         `local-citation-span-drift`'s comparison. The population is empty in a corpus that \
         declares neither list, so this cannot fail a repository that predates it.",
        violations,
    )
}

/// Every paragraph the proposer flags that no `refuses:` or `not_refusals:` span rules on — only
/// where `universal.yml` says `refusals: propose: true`.
pub fn refusal_undeclared(nodes: &[Node], universal: &Universal) -> Check {
    let violations = if universal.propose_refusals() {
        nodes
            .iter()
            .flat_map(|n| {
                let fields = prose_fields(&n.text);
                // The one paragraph each span names, not every paragraph repeating its words.
                let ruled: BTreeSet<String> = refusal_spans(n)
                    .into_iter()
                    .chain(not_refusal_spans(n))
                    .filter_map(|s| ruled_paragraph(&fields, s))
                    .collect();
                // A paragraph written twice — a coined `summary:` repeating the description's
                // first paragraph — is one paragraph to rule on, so it is named once.
                let mut seen = BTreeSet::new();
                fields
                    .iter()
                    .flat_map(|f| f.paragraphs.iter())
                    .map(|p| flatten(p))
                    .filter(|p| !ruled.contains(p))
                    .filter(|p| proposes_refusal(p))
                    .filter(|p| seen.insert(p.clone()))
                    .map(|p| {
                        Violation::new(
                            &n.rel,
                            format!(
                                "has a paragraph that reads like a refusal and is ruled neither \
                                 way — if it declines an inference, quote it under `refuses:`; \
                                 if not, quote it under `not_refusals:`. Paragraph: {}",
                                truncate(&p)
                            ),
                        )
                    })
                    .collect::<Vec<_>>()
            })
            .collect()
    } else {
        Vec::new()
    };
    Check::new(
        UNDECLARED,
        "A paragraph reads like a refusal and no span rules on it",
        Severity::Info,
        "`agent-conduct.md`: a refusal is declared where it is written, or `derive check` cannot \
         hold an argument to it. `derive check` proposes refusals only over paragraphs an artifact \
         cites, so a corpus with no artifacts is never asked. This runs the same proposer, \
         `proposes_refusal`, over every paragraph. Info and never more: the proposer's precision \
         is 0.593 against 62 human rulings, and deciding which paragraphs refuse is a person's \
         ruling. Opt-in through `universal.yml`'s `refusals: propose: true`, because the default \
         cannot be otherwise — across 20 corpora it flags 8,780 of 135,200 paragraphs, and that \
         is a gate arriving in a corpus that never agreed to it (RFC-0057).",
        violations,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(text: &str) -> Node {
        Node::parse(
            std::path::PathBuf::from("/r/.yidam/corpus/claim/a.yml"),
            ".yidam/corpus/claim/a.yml",
            text,
        )
    }

    const HELD: &str = "\
class: claim
description: |
  The census counts residents.

  [inference] The records do not say the period
  caused the depopulation.
refuses:
  - span: The records do not say the period caused the depopulation.
    inference: the period caused the depopulation
";

    #[test]
    fn a_refusal_its_prose_holds_is_silent() {
        let check = refusal_span_drift(&[node(HELD)]);
        assert!(check.violations.is_empty(), "{:?}", check.violations);
    }

    #[test]
    fn a_reworded_refusal_is_reported() {
        let drifted = HELD.replace("do not say the period\n", "never say the period\n");
        let check = refusal_span_drift(&[node(&drifted)]);
        assert_eq!(check.violations.len(), 1, "{:?}", check.violations);
        assert!(check.violations[0].detail.contains("no longer says"));
    }

    /// The declaration quotes itself, so a search of the raw file would always succeed.
    #[test]
    fn the_declaration_is_not_its_own_evidence() {
        let text = "\
class: claim
description: Something else entirely.
refuses:
  - span: The records do not say the period caused the depopulation.
";
        let check = refusal_span_drift(&[node(text)]);
        assert_eq!(check.violations.len(), 1, "{:?}", check.violations);
    }

    #[test]
    fn a_refusal_without_a_span_is_reported() {
        let text = "\
class: claim
description: Anything.
refuses:
  - inference: the period caused the depopulation
";
        let check = refusal_span_drift(&[node(text)]);
        assert_eq!(check.violations.len(), 1);
        assert!(check.violations[0].detail.contains("no `span:`"));
    }

    // ── not_refusals: and the paragraph rule (RFC-0057) ──────────────────────

    const RULED: &str = "\
class: claim
description: |
  [inference] The publisher does not label the motions;
  the Journal does.

  [open] The records do not say the period caused the depopulation.
not_refusals:
  - span: The publisher does not label the motions; the Journal does.
";

    #[test]
    fn a_ruled_non_refusal_its_prose_holds_is_silent() {
        let check = refusal_span_drift(&[node(RULED)]);
        assert!(check.violations.is_empty(), "{:?}", check.violations);
    }

    #[test]
    fn a_drifted_non_refusal_is_reported() {
        let drifted = RULED.replace(
            "does not label the motions;\n",
            "never labels the motions;\n",
        );
        let check = refusal_span_drift(&[node(&drifted)]);
        assert_eq!(check.violations.len(), 1, "{:?}", check.violations);
        assert!(check.violations[0].detail.contains("ruled non-refusal"));
    }

    #[test]
    fn a_span_across_a_blank_line_is_reported_on_either_list() {
        let across = "the Journal does. [open] The records do not say";
        for list in ["refuses", "not_refusals"] {
            let text = format!(
                "{}{list}:\n  - span: {across}\n",
                RULED.split("not_refusals:").next().unwrap_or_default()
            );
            let check = refusal_span_drift(&[node(&text)]);
            assert_eq!(check.violations.len(), 1, "{list}: {:?}", check.violations);
            assert!(
                check.violations[0].detail.contains("crosses a blank line"),
                "{list}: {:?}",
                check.violations
            );
            assert_eq!(check.severity, Severity::Error);
        }
    }

    #[test]
    fn one_paragraph_ruled_both_ways_is_reported() {
        // The same text on both lists, and two different sentences of one paragraph.
        let same = format!(
            "{RULED}refuses:\n  - span: The publisher does not label the motions; the Journal does.\n"
        );
        let different = format!("{RULED}refuses:\n  - span: the Journal does.\n");
        for text in [same, different] {
            let check = refusal_span_drift(&[node(&text)]);
            assert_eq!(check.violations.len(), 1, "{:?}", check.violations);
            assert!(check.violations[0]
                .detail
                .contains("both a refusal and not one"));
        }
    }

    /// A short `refuses:` span whose words recur in the paragraph a `not_refusals:` span rules
    /// is ambiguous, and is reported as that — not as one paragraph ruled both ways.
    #[test]
    fn a_span_in_two_paragraphs_is_ambiguous_not_a_conflict() {
        let text = "\
class: claim
description: |
  [inference] The survey does not say who owns it; the deed does.

  [open] The survey does not say.
refuses:
  - span: The survey does not say
not_refusals:
  - span: who owns it; the deed does.
";
        let check = refusal_span_drift(&[node(text)]);
        assert_eq!(check.violations.len(), 1, "{:?}", check.violations);
        assert!(
            check.violations[0].detail.contains("found in 2 paragraphs"),
            "{:?}",
            check.violations
        );
    }

    #[test]
    fn rulings_on_two_paragraphs_do_not_conflict() {
        let text = format!(
            "{RULED}refuses:\n  - span: The records do not say the period caused the depopulation.\n"
        );
        let check = refusal_span_drift(&[node(&text)]);
        assert!(check.violations.is_empty(), "{:?}", check.violations);
    }

    // ── refusal-undeclared ───────────────────────────────────────────────────

    fn asked() -> Universal {
        Universal::parse("refusals:\n  propose: true\n")
    }

    const UNRULED: &str = "\
class: claim
description: |
  [inference] The publisher does not label the motions.

  [open] The records do not say the period caused the depopulation.

  The census does not count visitors.
";

    #[test]
    fn a_flagged_paragraph_is_a_finding_only_when_asked() {
        let check = refusal_undeclared(&[node(UNRULED)], &asked());
        assert_eq!(check.violations.len(), 2, "{:?}", check.violations);
        assert_eq!(check.severity, Severity::Info);
        assert!(check.violations[0]
            .detail
            .contains("The publisher does not label"));
        let unasked = refusal_undeclared(&[node(UNRULED)], &Universal::empty());
        assert!(unasked.violations.is_empty(), "{:?}", unasked.violations);
    }

    #[test]
    fn a_span_on_either_list_silences_its_paragraph() {
        let text = format!(
            "{UNRULED}refuses:\n  - span: The records do not say the period\nnot_refusals:\n  - span: does not label the motions.\n"
        );
        let check = refusal_undeclared(&[node(&text)], &asked());
        assert!(check.violations.is_empty(), "{:?}", check.violations);
    }

    /// A span that names no single paragraph silences none — a two-word ruling does not
    /// switch the proposer off for every paragraph that repeats it.
    #[test]
    fn an_ambiguous_span_silences_nothing() {
        let text = format!("{UNRULED}not_refusals:\n  - span: does not\n");
        let check = refusal_undeclared(&[node(&text)], &asked());
        assert_eq!(check.violations.len(), 2, "{:?}", check.violations);
    }

    #[test]
    fn a_paragraph_written_twice_is_named_once() {
        let text = "\
class: claim
description: |
  [inference] The publisher does not label the motions.
summary: \"[inference] The publisher does not label the motions.\"
";
        let check = refusal_undeclared(&[node(text)], &asked());
        assert_eq!(check.violations.len(), 1, "{:?}", check.violations);
    }

    #[test]
    fn a_paragraph_without_a_weak_tag_is_never_a_finding() {
        let text = "\
class: claim
description: |
  [verified] The census does not count visitors. Nothing here is refused.
";
        let check = refusal_undeclared(&[node(text)], &asked());
        assert!(check.violations.is_empty(), "{:?}", check.violations);
    }

    /// The declarations quote the prose, so a reader that took them as prose would flag them too.
    #[test]
    fn the_declarations_are_not_proposed() {
        let text = "\
class: claim
description: Plain.
not_refusals:
  - span: \"[inference] This does not refuse anything.\"
";
        let check = refusal_undeclared(&[node(text)], &asked());
        assert!(check.violations.is_empty(), "{:?}", check.violations);
    }
}
