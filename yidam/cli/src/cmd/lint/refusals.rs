//! A node's declared refusals, held to its own prose (RFC-0045 §3.3).
//!
//! `refuses:` quotes the sentence in which a node declines an inference. `derive check` owes an
//! answer to every refusal in a paragraph an argument cites, and finds that paragraph by the
//! quote — so a quote that no longer matches the prose owes nothing to anyone, and the argument
//! that routed around the refusal passes. This is the check that keeps the declaration and the
//! prose saying the same thing. It reads the node alone; no artifact needs to exist for it to
//! fire.

use crate::cmd::lint::citations::truncate;
use crate::cmd::lint::model::{Check, Severity, Violation};
use crate::corpus::Node;
use crate::derive::{in_prose, prose_fields};

pub const SPAN_DRIFT: &str = "refusal-span-drift";

/// Every `refuses:` entry whose `span` is absent or is not in the node's own prose.
pub fn refusal_span_drift(nodes: &[Node]) -> Check {
    let violations = nodes
        .iter()
        .flat_map(|n| {
            let refusals = n.inst.refuses.as_deref().unwrap_or_default();
            let fields = (!refusals.is_empty()).then(|| prose_fields(&n.text));
            refusals.iter().filter_map(move |r| {
                let span = r.span.as_deref().map(str::trim).unwrap_or_default();
                let detail = if span.is_empty() {
                    "declares a refusal with no `span:` — quote the sentence that refuses, or \
                     `derive check` has no paragraph to hold an argument to"
                        .to_string()
                } else if !in_prose(fields.as_deref().unwrap_or_default(), span) {
                    format!(
                        "declares a refusal its own prose no longer says — reread the node and \
                         decide whether it still refuses; re-quoting whatever is there now to \
                         clear this discards the refusal silently. Span: {}",
                        truncate(span)
                    )
                } else {
                    return None;
                };
                Some(Violation::new(&n.rel, detail))
            })
        })
        .collect();
    Check::new(
        SPAN_DRIFT,
        "A declared refusal quotes text its own node no longer holds",
        Severity::Error,
        "`agent-conduct.md`: an outbound claim does not route around a refusal in the block it \
         cites. `derive check` can only hold an argument to that when the refusal is declared, \
         and it finds the paragraph a refusal governs by the `refuses:` span — so a span the \
         prose no longer contains governs nothing, and every argument resting on that paragraph \
         is released from it with no finding anywhere. Error, under the baseline ratchet: the \
         repair is in this corpus and is one edit. Whitespace is normalized and nothing else is, \
         which is `local-citation-span-drift`'s comparison. The population is empty in a corpus \
         that declares no refusals, so this cannot fail a repository that predates it.",
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
}
