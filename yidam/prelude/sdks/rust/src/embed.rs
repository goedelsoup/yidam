//! One canonical embed text, and the reason there has to be exactly one.
//!
//! An index is built out of a string per node, and *which* string is a judgement: a label, the
//! prose the class declared, the identifiers a query is actually typed in, the names of the
//! things this node points at. Two implementations of that judgement over one corpus produce
//! two different vector spaces from the same weights, and nothing about either looks wrong.
//!
//! That is not hypothetical. RFC-0007 was filed about two assemblers over one corpus — this
//! repository's, composing `label · description · Related: <stems>.`, and a downstream
//! consumer's, composing `label · description · class · <curated meta>` because the SDK
//! shipped no opinion and it had to invent one. Each was reasonable. Neither could be
//! compared to the other, because neither was written down anywhere both could read.
//!
//! So the assembler is a parity function, held to a shared fixture, and
//! [`compose_embed_text`] is it.
//!
//! # What goes in, and who decides
//!
//! A node, and the three field lists its class declared:
//!
//! - **`prose_keys`** — top-level keys that carry prose, resolved by the caller out of
//!   `<class>.ont.yml` and `universal.yml`. See [`crate::prose`] for why that resolution is
//!   the caller's and this is not.
//! - **`prose_properties`** — names under `properties:` the class flagged `prose: true`.
//! - **`retrievable_properties`** — names under `properties:` the class flagged
//!   `retrievable: true`.
//!
//! The third list is the one that makes this an assembler and not a formatter. A gage's
//! `parameter: "00060"` and `units: cubic feet per second` are not prose in any sense a length
//! check or a missing-description check would accept — flagging them `prose` would make
//! `node-too-long` count a two-token code and `missing-description` accept a node that says
//! nothing but `00060` — and they are exactly the strings a query is typed in (#717).
//! *Which fields carry meaning* is the judgement the downstream consumer was making by hand in
//! a curated list; here the ontology answers it declaratively, and this function is where the
//! two axes are unioned.
//!
//! # Prose is retrievable, and the union is taken here
//!
//! A class need not flag a prose property `retrievable` as well: the set is *prose, then the
//! flagged properties prose did not already carry*, and a property flagged both appears once.
//! The implication runs one way — an embedding is built out of what a node says, so prose is
//! retrievable; a retrievable identifier is not thereby prose, which is the whole distinction
//! above.
//!
//! # Absent means false
//!
//! A class declaring nothing composes exactly its prose, so a corpus written before either
//! flag existed embeds byte-identically to how it always did. The corollary is the contract a
//! corpus takes on when it does flag one: node text is what an index is built from, so
//! flagging a property means re-embedding. That is the change rather than a side effect of it.

use crate::corpus::{CorpusInstance, CorpusLink};

/// What one class declared about which of its fields belong in an embedding.
///
/// A named struct rather than three positional slices, for [`crate::prose::of`]'s reason one
/// level down: the three lists are not interchangeable and a caller passing them in the wrong
/// order would compile. It is also what makes the *union* impossible to forget — the mistake
/// `embed` made for as long as it asked `prose` alone, and missed every flagged identifier in
/// the corpus.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EmbedFields {
    /// Top-level keys that carry prose, in the order the caller resolved them.
    pub prose_keys: Vec<String>,
    /// Property names the class flagged `prose: true`, in declaration order.
    pub prose_properties: Vec<String>,
    /// Property names the class flagged `retrievable: true`, in declaration order.
    pub retrievable_properties: Vec<String>,
}

/// Everything this node says that belongs in its embedding: its prose, then the properties
/// flagged `retrievable` that prose did not already carry.
///
/// Keys come back qualified the way [`crate::prose::of`] qualifies them — `properties.units`.
/// That qualification is also what makes the de-duplication exact rather than a guess: a
/// property flagged both `prose` and `retrievable` arrives under one key from both sides and
/// is emitted once, while a top-level `parameter` and a property `parameter` stay two things.
pub fn of<'a>(inst: &'a CorpusInstance, fields: &EmbedFields) -> Vec<(String, &'a str)> {
    let mut out = crate::prose::of(inst, &fields.prose_keys, &fields.prose_properties);
    let already: std::collections::BTreeSet<String> = out.iter().map(|(k, _)| k.clone()).collect();

    let props = inst.properties.as_ref();
    out.extend(fields.retrievable_properties.iter().filter_map(|name| {
        let key = format!("properties.{name}");
        if already.contains(&key) {
            return None;
        }
        let value = props?.get(name.as_str())?.as_str()?;
        (!value.trim().is_empty()).then_some((key, value))
    }));
    out
}

/// The file stem of a link target, with hyphens read as spaces.
///
/// **Written out rather than taken from the platform's path type**, and that is the fix rather
/// than a style preference. The Rust reference reached this through `Path::file_stem`, which
/// splits on `\` as well as `/` when the target happens to be compiled for Windows — so the
/// same corpus composed two different strings depending on where the binary was built, and no
/// fixture could have said which was right. The rule is four lines and belongs where all three
/// languages can read it:
///
/// 1. the segment after the last `/`, ignoring a trailing one;
/// 2. never `.` or `..`, which name a directory and not a node;
/// 3. cut at the last `.` that is not the first character, so `plan.v2.yml` is `plan.v2` and a
///    dotfile keeps its name;
/// 4. `-` reads as a space, because `lower-canyon` is two words to an embedder and one token
///    to nobody.
fn link_stem(target: &str) -> Option<String> {
    let name = target.trim_end_matches('/').rsplit('/').next()?;
    if name.is_empty() || name == "." || name == ".." {
        return None;
    }
    let stem = match name.rfind('.') {
        Some(i) if i > 0 => &name[..i],
        _ => name,
    };
    Some(stem.replace('-', " "))
}

/// The names of the things this node points at, in the order it wrote them.
///
/// A class definition is not one of them. `.ont.yml` is the contract an instance is written
/// *under*, not a thing it relates to, and every instance of a class carries the same one — so
/// admitting it would add one constant phrase to every vector in the class and distinguish
/// nothing.
fn related(links: &[CorpusLink]) -> Vec<String> {
    links
        .iter()
        .filter_map(|l| l.target.as_deref())
        .filter(|t| !t.ends_with(".ont.yml"))
        .filter_map(link_stem)
        .collect()
}

/// Compose the text this node is embedded as.
///
/// Three parts, joined by a single space, each omitted when it is empty:
///
/// 1. the node's `label`;
/// 2. everything [`of`] returns, each value trimmed at the end and joined by newlines;
/// 3. `Related: <names>.` over the link targets.
///
/// **The class is not in the text, and that is a decision rather than an omission.** It is a
/// column beside the vector, where a filter can use it exactly; folded into the text it would
/// put one identical phrase in every vector of a class, which pulls the whole class toward one
/// point and distinguishes no member of it from any other. The consumer RFC-0007 was filed
/// about folded it in, which is one of the two places the two assemblers disagreed.
pub fn compose_embed_text(inst: &CorpusInstance, fields: &EmbedFields) -> String {
    let mut parts: Vec<String> = Vec::new();

    if let Some(label) = inst.label.as_deref() {
        if !label.is_empty() {
            parts.push(label.to_string());
        }
    }

    let said = of(inst, fields)
        .into_iter()
        .map(|(_, v)| v.trim_end().to_string())
        .collect::<Vec<_>>()
        .join("\n");
    if !said.is_empty() {
        parts.push(said);
    }

    let names = related(inst.links.as_deref().unwrap_or(&[]));
    if !names.is_empty() {
        parts.push(format!("Related: {}.", names.join(", ")));
    }

    parts.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::corpus::parse_instance;

    fn list(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| (*s).to_string()).collect()
    }

    fn fields(keys: &[&str], prose: &[&str], retrievable: &[&str]) -> EmbedFields {
        EmbedFields {
            prose_keys: list(keys),
            prose_properties: list(prose),
            retrievable_properties: list(retrievable),
        }
    }

    const GAGE: &str = "label: Tailwater gage\n\
        description: The station below the impoundment.\n\
        properties:\n\
        \x20 parameter: \"00060\"\n\
        \x20 units: cubic feet per second\n\
        \x20 claim_tag: inference\n\
        links:\n\
        \x20 - target: ../reach/lower-canyon.yml\n\
        \x20 - target: ../gage.ont.yml\n";

    /// #717 in one assertion. Both flagged strings are in the node's embedding, and the
    /// property nobody flagged is not.
    #[test]
    fn a_flagged_property_reaches_the_embedding() {
        let t = compose_embed_text(
            &parse_instance(GAGE),
            &fields(&["description"], &[], &["parameter", "units"]),
        );
        assert_eq!(
            t,
            "Tailwater gage The station below the impoundment.\n00060\ncubic feet per second \
             Related: lower canyon."
        );
        assert!(!t.contains("inference"), "claim_tag was not flagged: {t:?}");
    }

    /// Every corpus written before either flag existed: identical to its prose alone.
    #[test]
    fn a_class_flagging_nothing_composes_exactly_its_prose() {
        assert_eq!(
            compose_embed_text(&parse_instance(GAGE), &fields(&["description"], &[], &[])),
            "Tailwater gage The station below the impoundment. Related: lower canyon."
        );
    }

    /// Prose is already retrievable, and a property flagged both is emitted once.
    #[test]
    fn a_property_flagged_prose_and_retrievable_appears_once() {
        let node = parse_instance("properties:\n  method: How it was computed.\n");
        assert_eq!(
            of(&node, &fields(&[], &["method"], &["method"])),
            [("properties.method".to_string(), "How it was computed.")]
        );
    }

    /// A top-level key and a property of the same name are told apart, so the de-duplication
    /// against prose cannot collide them.
    #[test]
    fn a_top_level_key_and_a_property_of_the_same_name_are_told_apart() {
        let node = parse_instance("parameter: at the top\nproperties:\n  parameter: \"00060\"\n");
        assert_eq!(
            of(&node, &fields(&["parameter"], &[], &["parameter"])),
            [
                ("parameter".to_string(), "at the top"),
                ("properties.parameter".to_string(), "00060"),
            ]
        );
    }

    /// A flagged property the node omits, or writes as something other than a string, is not
    /// an empty line in the vector.
    #[test]
    fn a_flagged_property_that_is_absent_or_not_text_yields_nothing() {
        let node =
            parse_instance("description: Only this.\nproperties:\n  codes:\n    - \"00060\"\n");
        assert_eq!(
            compose_embed_text(
                &node,
                &fields(&["description"], &[], &["codes", "parameter"])
            ),
            "Only this."
        );
    }

    /// The class definition an instance is written under is not a thing it relates to.
    #[test]
    fn an_ontology_target_is_not_a_relationship() {
        let node = parse_instance("links:\n  - target: ../gage.ont.yml\n");
        assert_eq!(
            compose_embed_text(&node, &fields(&["description"], &[], &[])),
            ""
        );
    }

    /// The stem rule, including the two cases a naive split gets wrong.
    #[test]
    fn the_stem_is_the_last_segment_cut_at_its_last_dot() {
        assert_eq!(
            link_stem("../reach/lower-canyon.yml").as_deref(),
            Some("lower canyon")
        );
        assert_eq!(link_stem("plan.v2.yml").as_deref(), Some("plan.v2"));
        assert_eq!(link_stem("no-extension").as_deref(), Some("no extension"));
        assert_eq!(link_stem(".hidden").as_deref(), Some(".hidden"));
        assert_eq!(link_stem("a/b/").as_deref(), Some("b"));
        assert_eq!(link_stem(".."), None);
        assert_eq!(link_stem(""), None);
    }

    /// An empty node composes an empty string rather than a stray separator.
    #[test]
    fn a_node_that_says_nothing_composes_nothing() {
        assert_eq!(
            compose_embed_text(
                &parse_instance("class: gage\n"),
                &fields(&["description"], &[], &[])
            ),
            ""
        );
    }
}
