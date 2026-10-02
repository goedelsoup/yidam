//! What a node says, once its class has declared which keys say it.
//!
//! `description` was the only key anything read, and corpora write more than one — `summary`,
//! `findings`, `revisions`, `unfilled`. Reading one field measured 21 lines of a node that
//! says 118 (#674), and across the population 21.8% of a node's bytes against 50.5% (#713).
//! So which keys carry prose is **declared**, per class, and this module reads a node against
//! a declaration somebody else resolved.
//!
//! # The resolution is not here, and that is the division
//!
//! `<class>.ont.yml` names a class's own prose keys, `universal.yml` names the apparatus every
//! class may carry, and the effective set is their union with `description` always in it.
//! Walking those files, applying that union and falling back for a class nothing declared is a
//! question about a *corpus on disk* — about a directory layout, a filename convention and a
//! git revision — and an SDK that answered it would be answering a question the ontology owns.
//!
//! What is here is the half that has to be identical in three languages: given the resolved
//! key list, what does this node say? That half is what [`crate::embed::compose_embed_text`]
//! also needs, and having it in one place is what stops the embedder and the length checks
//! reading the same node differently — which is exactly what they did until #746.
//!
//! # Two axes, because they are read out of different places
//!
//! `keys` are top-level keys, read off the document. `properties` are names under
//! `properties:` that the class flagged `prose: true`, read out of that mapping. One list
//! could not hold both: nothing forbids a corpus writing a top-level `method` and a property
//! `method`, and a lookup over a merged list could not say which it meant. So the property
//! keys come back **qualified** — `properties.method` — and a caller rendering a name in a
//! finding can say which one it is naming.
//!
//! # A key the node does not carry, or does not carry as text, yields nothing
//!
//! A declared `findings:` holding a list is a real state and not prose. Guessing at a
//! rendering for it would put words in the corpus's mouth, and an empty entry would be a blank
//! line in whatever is composed downstream.

use crate::corpus::CorpusInstance;

/// The prose key every node carries whatever anything declares.
///
/// It is read off [`CorpusInstance::description`] rather than out of `extra`, which is the
/// one asymmetry in [`of`]: `description` is a named field on the struct and every other
/// declared key is a coined one. A caller that resolved the key list without this in it would
/// silently stop measuring the field every corpus writes, so the resolver puts it in
/// unconditionally and this constant is the name both sides use for it.
pub const ALWAYS: &str = "description";

/// This node's prose: the declared top-level keys, then the flagged properties.
///
/// Both lists are taken in the order given. The caller resolved them, and re-sorting here
/// would make what a node says depend on a rule nobody wrote down.
pub fn of<'a>(
    inst: &'a CorpusInstance,
    keys: &[String],
    properties: &[String],
) -> Vec<(String, &'a str)> {
    let mut out: Vec<(String, &'a str)> = keys
        .iter()
        .filter_map(|key| {
            let value = match key.as_str() {
                ALWAYS => inst.description.as_deref(),
                other => inst.extra.get(other).and_then(serde_yaml::Value::as_str),
            }?;
            (!value.trim().is_empty()).then_some((key.clone(), value))
        })
        .collect();

    let props = inst.properties.as_ref();
    out.extend(properties.iter().filter_map(|name| {
        let value = props?.get(name.as_str())?.as_str()?;
        (!value.trim().is_empty()).then_some((format!("properties.{name}"), value))
    }));
    out
}

/// The node's prose as one block, the declared fields joined in the order [`of`] returns them.
///
/// What a reader of the whole node reads, which is what a length ceiling is about. Each value
/// is trimmed at the end only: leading indentation inside a block scalar is the author's, and
/// a trailing newline is YAML's.
pub fn text(inst: &CorpusInstance, keys: &[String], properties: &[String]) -> String {
    of(inst, keys, properties)
        .into_iter()
        .map(|(_, v)| v.trim_end().to_string())
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::corpus::parse_instance;

    fn names(v: &[String]) -> Vec<String> {
        v.to_vec()
    }

    fn list(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| (*s).to_string()).collect()
    }

    #[test]
    fn description_is_read_off_the_field_and_other_keys_out_of_extra() {
        let node = parse_instance("description: What it is.\nfindings: What was found.\n");
        assert_eq!(
            of(&node, &list(&["description", "findings"]), &[]),
            [
                ("description".to_string(), "What it is."),
                ("findings".to_string(), "What was found."),
            ]
        );
    }

    #[test]
    fn an_undeclared_key_is_not_prose() {
        let node = parse_instance("description: What it is.\nfindings: What was found.\n");
        assert_eq!(
            of(&node, &list(&["description"]), &[]),
            [("description".to_string(), "What it is.")]
        );
    }

    #[test]
    fn a_flagged_property_is_qualified_so_a_same_named_key_is_told_apart() {
        let node = parse_instance("method: at the top\nproperties:\n  method: in the bag\n");
        assert_eq!(
            of(&node, &list(&["method"]), &list(&["method"])),
            [
                ("method".to_string(), "at the top"),
                ("properties.method".to_string(), "in the bag"),
            ]
        );
    }

    #[test]
    fn a_declared_key_the_node_omits_or_writes_as_a_list_yields_nothing() {
        let node = parse_instance("description: Only this.\nfindings:\n  - one\n  - two\n");
        assert_eq!(
            text(&node, &list(&["description", "findings", "absent"]), &[]),
            "Only this."
        );
    }

    #[test]
    fn a_blank_value_is_not_prose() {
        let node = parse_instance("description: \"   \"\nfindings: Real.\n");
        assert_eq!(
            text(&node, &list(&["description", "findings"]), &[]),
            "Real."
        );
    }

    #[test]
    fn values_are_trimmed_at_the_end_only() {
        let node = parse_instance("description: |\n  A block.\nfindings: |\n  Another.\n");
        assert_eq!(
            text(&node, &list(&["description", "findings"]), &[]),
            "A block.\nAnother."
        );
    }

    /// The order is the caller's, both within a list and between the two.
    #[test]
    fn the_declared_order_is_the_order() {
        let node = parse_instance("description: Third.\nfindings: First.\nsummary: Second.\n");
        assert_eq!(
            names(
                &of(&node, &list(&["findings", "summary", "description"]), &[])
                    .into_iter()
                    .map(|(k, _)| k)
                    .collect::<Vec<_>>()
            ),
            list(&["findings", "summary", "description"])
        );
    }
}
