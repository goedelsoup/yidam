//! Counting the evidence markers in a node.
//!
//! Distinct from `yidam_core::corpus::extract_claims`, which parses whole *claims* —
//! line-oriented, one tag per line, returning the claim text. This counts *markers*
//! wherever they appear, which is a different question with a different answer.
//!
//! Properties are included because the corpus routinely records absence there rather than
//! in prose: `estimate: "[open] — not computed"` is a live open claim that a
//! description-only count misses entirely.
//!
//! Markers are matched as exact bracketed tokens. That is deliberately narrow — corpus
//! prose is dense with markdown links, and a looser match would read `[open questions](…)`
//! as an open claim. The three tokens never appear as link text, so exact matching has no
//! false positives to trade against.
//!
//! # A mention is not a claim
//!
//! Exact was not narrow enough. A node that *discusses* the evidence vocabulary — a sentence
//! saying a claim is not verified, with the token in backticks as code — was counted as
//! carrying that claim. The count was correct about the bytes and wrong about the corpus.
//!
//! This is not a hypothetical for any corpus whose subject touches its own evidentiary
//! apparatus, and such a node has good reason to name the tags and no way to do so without
//! being counted. Reported from a derived repository where `yidam status` published **1
//! verified claim against a true 0** for four commits, inside a `REGEN` block in `README.md`
//! — which no human writes and everyone therefore trusts. It was found because a reader
//! happened to hold an expectation about the number, not by any check.
//!
//! So the text scan reads [`crate::markdown::mask_code`] first. Inline code is markdown's
//! conventional signal for mention-rather-than-use, and it costs a corpus nothing to say
//! what it means. The frozen MCP contract already described the predicate this way — "the
//! body contains an `[open]` **claim**" — so this makes the implementation agree with the
//! words rather than changing what they promise.
//!
//! The opposite failure is left to `lint`. A tag with its citation folded inside the
//! brackets — `[verified — <source>]` — matches no token and is counted as *nothing*, just
//! as silently; a reader who writes that has plainly intended a tag. Counting it would
//! guess, so `claim-tag-malformed` reports it instead.
//!
//! # And a node may say so structurally
//!
//! A bracketed token in serialized text is a property of a node's *serialization*, not of
//! the node. A corpus whose evidence markers are structured — `claim_tag: open` in a
//! property, because it has a typed claim vocabulary and stores the tag rather than prose
//! about the tag — was invisible to every count here.
//!
//! Measured, by a consumer running the real binary over its own mirror: `open-questions`
//! returned **2 nodes against that repository's own count of 26**. Not a rounding
//! difference. The other 24 were open, said so in a machine-readable field, and were
//! counted as nothing. It closed the gap by serializing `claim_tag: "[open]"` — a data-model
//! decision made to satisfy a `contains()` call, with every reader normalizing it back.
//!
//! So a class may now declare which of its properties carry an evidence tag, and a value in
//! one of those is read as the tag it is. The text scan stays, for prose-authored corpora
//! and for the markers that legitimately live in a sentence.
//!
//! **The ontology declares it and no key name is blessed.** Matching a bare `open` under any
//! key would read `status: open` on a ballot measure as an open claim, which is a false
//! positive a corpus cannot opt out of. A declared field has none: the corpus said which
//! field it is.

/// How much of a node is measured against how much is supposed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ClaimCounts {
    pub verified: usize,
    pub inference: usize,
    pub open: usize,
}

impl ClaimCounts {
    pub fn total(&self) -> usize {
        self.verified + self.inference + self.open
    }

    pub fn add(&mut self, other: ClaimCounts) {
        self.verified += other.verified;
        self.inference += other.inference;
        self.open += other.open;
    }

    /// Compact rendering for an index cell: `12v / 3i / 1o`, or `—` when untagged.
    pub fn cell(&self) -> String {
        if self.total() == 0 {
            return "—".to_string();
        }
        format!("{}v / {}i / {}o", self.verified, self.inference, self.open)
    }
}

/// The tokens defined by `prelude/guidelines/agent-conduct.md`.
pub const VERIFIED: &str = "[verified]";
pub const INFERENCE: &str = "[inference]";
pub const OPEN: &str = "[open]";

/// The ontology property type that marks a field as carrying an evidence tag.
pub const CLAIM_PROPERTY_TYPE: &str = "claim";

/// Which properties of each class carry an evidence tag.
///
/// Built once from the `.ont.yml` files and passed to the counters, because the answer is a
/// property of the class rather than of the file being read.
#[derive(Debug, Default, Clone)]
pub struct ClaimFields(std::collections::BTreeMap<String, Vec<String>>);

#[derive(Default, serde::Deserialize)]
struct OntologyProperties {
    #[serde(default)]
    class: Option<String>,
    #[serde(default)]
    properties: Vec<OntologyProperty>,
}

#[derive(serde::Deserialize)]
struct OntologyProperty {
    name: String,
    #[serde(default)]
    r#type: String,
}

impl ClaimFields {
    /// Read every class definition in a corpus.
    pub fn load(corpus: &std::path::Path) -> Self {
        let mut map = std::collections::BTreeMap::new();
        for path in crate::walk::walk_ont_files(corpus) {
            let text = std::fs::read_to_string(&path).unwrap_or_default();
            let ont: OntologyProperties = serde_yaml::from_str(&text).unwrap_or_default();
            let class = ont.class.unwrap_or_else(|| {
                path.file_name()
                    .and_then(|n| n.to_str())
                    .and_then(|n| n.strip_suffix(".ont.yml"))
                    .unwrap_or_default()
                    .to_string()
            });
            let fields: Vec<String> = ont
                .properties
                .into_iter()
                .filter(|p| p.r#type == CLAIM_PROPERTY_TYPE)
                .map(|p| p.name)
                .collect();
            if !fields.is_empty() {
                map.insert(class, fields);
            }
        }
        Self(map)
    }

    pub fn for_class(&self, class: &str) -> &[String] {
        self.0.get(class).map(Vec::as_slice).unwrap_or(&[])
    }
}

/// The tag a declared field's value carries, if any.
///
/// Both spellings are read: `open` is what a typed vocabulary stores, and `[open]` is what a
/// corpus writes after being told the scan needs brackets. Accepting both means nobody has
/// to reshape their data a second time when this lands.
fn tag_of(value: &str) -> Option<&'static str> {
    match value.trim().trim_matches(|c| c == '[' || c == ']') {
        "verified" => Some(VERIFIED),
        "inference" => Some(INFERENCE),
        "open" => Some(OPEN),
        _ => None,
    }
}

/// Every value in the document held under one of `fields`, at any depth.
///
/// Depth-first over the whole document rather than only `properties`, because a projected
/// corpus may put its typed fields anywhere and the key name was declared by the ontology —
/// there is no false positive to guard against once the corpus has named the field.
fn structural_values(value: &serde_yaml::Value, fields: &[String], out: &mut Vec<String>) {
    match value {
        serde_yaml::Value::Mapping(map) => {
            for (k, v) in map {
                if let Some(key) = k.as_str() {
                    if fields.iter().any(|f| f == key) {
                        match v {
                            serde_yaml::Value::String(s) => out.push(s.clone()),
                            // A list of tags is one claim each — a node carrying two is
                            // making two statements about itself.
                            serde_yaml::Value::Sequence(items) => out.extend(
                                items.iter().filter_map(|i| i.as_str().map(str::to_string)),
                            ),
                            _ => {}
                        }
                        continue;
                    }
                }
                structural_values(v, fields, out);
            }
        }
        serde_yaml::Value::Sequence(items) => {
            for item in items {
                structural_values(item, fields, out);
            }
        }
        _ => {}
    }
}

/// Markers a node declares structurally, in fields its class named.
pub fn count_structural(text: &str, fields: &[String]) -> ClaimCounts {
    let mut counts = ClaimCounts::default();
    if fields.is_empty() {
        return counts;
    }
    let Ok(doc) = serde_yaml::from_str::<serde_yaml::Value>(text) else {
        return counts;
    };
    let mut values = Vec::new();
    structural_values(&doc, fields, &mut values);
    for v in values {
        match tag_of(&v) {
            Some(VERIFIED) => counts.verified += 1,
            Some(INFERENCE) => counts.inference += 1,
            Some(OPEN) => counts.open += 1,
            _ => {}
        }
    }
    counts
}

/// Every marker in a node: prose and structure both.
///
/// A structural tag whose value already carries brackets is counted once, not twice — the
/// text scan would otherwise see the same tag in the serialized bytes. That is the case a
/// corpus lands in after reshaping its data to satisfy the old scan, which is most of the
/// corpora this feature exists for.
pub fn count_in_node(text: &str, fields: &[String]) -> ClaimCounts {
    let mut counts = count_in_source(text);
    let structural = count_structural(text, fields);
    let bracketed = count_bracketed_structural(text, fields);
    counts.add(structural);
    // Subtract what both passes saw.
    counts.verified -= bracketed.verified.min(counts.verified);
    counts.inference -= bracketed.inference.min(counts.inference);
    counts.open -= bracketed.open.min(counts.open);
    counts
}

/// Structural values that are *also* visible to the text scan, i.e. already bracketed.
fn count_bracketed_structural(text: &str, fields: &[String]) -> ClaimCounts {
    let mut counts = ClaimCounts::default();
    if fields.is_empty() {
        return counts;
    }
    let Ok(doc) = serde_yaml::from_str::<serde_yaml::Value>(text) else {
        return counts;
    };
    let mut values = Vec::new();
    structural_values(&doc, fields, &mut values);
    for v in values {
        let trimmed = v.trim();
        if !trimmed.starts_with('[') {
            continue;
        }
        match tag_of(trimmed) {
            Some(VERIFIED) => counts.verified += 1,
            Some(INFERENCE) => counts.inference += 1,
            Some(OPEN) => counts.open += 1,
            _ => {}
        }
    }
    counts
}

/// **The** open-question predicate.
///
/// One function, because it was three: inlined at three call sites in `cmd/` and again in
/// the MCP server, each spelling `label.starts_with('?') || text.contains("[open]")` for
/// itself. RFC-0006 names the copies, and a fourth is where the next divergence goes.
pub fn is_open_question(label: &str, text: &str, fields: &[String]) -> bool {
    label.trim_start().starts_with('?')
        // Masked, for the same reason the counter is: a node explaining what `[open]` means
        // is not thereby an open question.
        || crate::markdown::mask_code(text).contains(OPEN)
        || count_structural(text, fields).open > 0
}

fn tally(text: &str, counts: &mut ClaimCounts) {
    counts.verified += text.matches(VERIFIED).count();
    counts.inference += text.matches(INFERENCE).count();
    counts.open += text.matches(OPEN).count();
}

/// Count markers in a whole instance file.
///
/// Reads the raw YAML rather than the parsed struct so that markers in property values are
/// seen regardless of how the property is shaped — the corpus puts them in scalars, lists,
/// and nested maps, and a typed walk would have to anticipate each.
pub fn count_in_source(text: &str) -> ClaimCounts {
    let mut counts = ClaimCounts::default();
    tally(&crate::markdown::mask_code(text), &mut counts);
    counts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_each_marker_in_prose() {
        let c = count_in_source("A [verified] and a [inference] and an [open].");
        assert_eq!(c.verified, 1);
        assert_eq!(c.inference, 1);
        assert_eq!(c.open, 1);
        assert_eq!(c.total(), 3);
    }

    #[test]
    fn counts_markers_in_property_values() {
        // The single most load-bearing open claim in a real corpus lives here, not in prose.
        let yaml =
            "description: no markers here\nproperties:\n  estimate: \"[open] — not computed\"\n";
        assert_eq!(count_in_source(yaml).open, 1);
    }

    #[test]
    fn a_markdown_link_is_not_an_open_claim() {
        let c = count_in_source("see [open questions](../q/index.md) for more");
        assert_eq!(c.open, 0, "matching must be exact-token, not substring");
    }

    /// The reported case: a node that names the vocabulary rather than using it.
    ///
    /// `yidam status` published 1 verified claim against a true 0 for four commits on a
    /// sentence shaped like this one.
    #[test]
    fn a_backticked_token_is_a_mention_and_is_not_counted() {
        let c = count_in_source("This claim is not `[verified]`; nobody has checked it.");
        assert_eq!(c.verified, 0, "inline code is a mention, not a claim");
        assert_eq!(c.total(), 0);
    }

    /// Both halves, in one sentence, because a corpus that discusses its own vocabulary
    /// writes both — and masking the wrong one is how a fix becomes the next defect.
    #[test]
    fn a_mention_and_a_use_on_one_line_are_told_apart() {
        let c = count_in_source("Unlike `[open]`, this one is settled. [verified]");
        assert_eq!((c.verified, c.open), (1, 0));
    }

    /// A fenced example is shown, not said.
    #[test]
    fn a_fenced_example_is_not_counted() {
        let yaml = "description: |\n  Write it like this:\n  ```\n  estimate: \"[open]\"\n  ```\n";
        assert_eq!(count_in_source(yaml).open, 0);
    }

    /// A node explaining the vocabulary is not thereby an instance of it.
    #[test]
    fn a_node_naming_open_in_code_is_not_an_open_question() {
        let text = "class: c\ndescription: The `[open]` tag marks an unanswered claim.\n";
        assert!(!is_open_question("Evidence tags", text, &[]));
        // And the used form still is.
        assert!(is_open_question(
            "Evidence tags",
            "description: Unresolved. [open]\n",
            &[]
        ));
    }

    #[test]
    fn repeated_markers_all_count() {
        let c = count_in_source("[verified] one. [verified] two.");
        assert_eq!(c.verified, 2);
    }

    #[test]
    fn an_untagged_node_renders_as_a_dash() {
        assert_eq!(ClaimCounts::default().cell(), "—");
    }

    #[test]
    fn a_tagged_node_renders_compactly() {
        let c = ClaimCounts {
            verified: 12,
            inference: 3,
            open: 1,
        };
        assert_eq!(c.cell(), "12v / 3i / 1o");
    }

    #[test]
    fn source_counting_sees_markers_anywhere_in_the_file() {
        let yaml = "class: c\ndescription: |\n  A [verified] fact.\nproperties:\n  estimate: \"[open] — not computed\"\n";
        let c = count_in_source(yaml);
        assert_eq!(c.verified, 1);
        assert_eq!(c.open, 1);
    }

    #[test]
    fn totals_accumulate() {
        let mut a = ClaimCounts::default();
        a.add(count_in_source("[verified]"));
        a.add(count_in_source("[open] [open]"));
        assert_eq!(a.verified, 1);
        assert_eq!(a.open, 2);
    }
}

#[cfg(test)]
mod structural_tests {
    use super::*;
    use tempfile::TempDir;

    fn corpus_with(ont: &str) -> (TempDir, std::path::PathBuf) {
        let tmp = TempDir::new().unwrap();
        let corpus = tmp.path().join(".yidam/corpus");
        std::fs::create_dir_all(corpus.join("lead")).unwrap();
        std::fs::write(corpus.join("lead.ont.yml"), ont).unwrap();
        (tmp, corpus)
    }

    const TYPED: &str = "class: lead\nlabel: Lead\nproperties:\n  \
                         - name: claim_tag\n    type: claim\n    description: The tag.\n";

    /// The reported case, reproduced: a node whose tag is a structured value.
    ///
    /// `open-questions` returned 2 of 26 against a corpus shaped like this, because the
    /// bracketed token never appears in the bytes.
    #[test]
    fn a_structured_tag_is_seen() {
        let (_t, corpus) = corpus_with(TYPED);
        let fields = ClaimFields::load(&corpus);
        assert_eq!(fields.for_class("lead"), ["claim_tag"]);

        let node = "class: lead\nlabel: A lead\nproperties:\n  claim_tag: open\n";
        assert_eq!(count_in_source(node).open, 0, "the text scan sees nothing");
        assert!(is_open_question("A lead", node, fields.for_class("lead")));
        assert_eq!(count_in_node(node, fields.for_class("lead")).open, 1);
    }

    /// The workaround a consumer adopted to satisfy the old scan must not now count twice.
    ///
    /// `claim_tag: "[open]"` is visible to both passes. A corpus that reshaped its data to be
    /// seen at all should not be penalised for it the moment the fix lands.
    #[test]
    fn a_bracketed_structured_tag_counts_once() {
        let (_t, corpus) = corpus_with(TYPED);
        let fields = ClaimFields::load(&corpus);
        let node = "class: lead\nlabel: A lead\nproperties:\n  claim_tag: \"[open]\"\n";
        assert_eq!(count_in_source(node).open, 1, "the text scan sees it");
        assert_eq!(count_in_node(node, fields.for_class("lead")).open, 1);
    }

    /// Prose and structure both, on one node, are two claims.
    #[test]
    fn prose_and_structure_are_separate_claims() {
        let (_t, corpus) = corpus_with(TYPED);
        let fields = ClaimFields::load(&corpus);
        let node = "class: lead\nlabel: A lead\ndescription: Unresolved. [open]\n\
                    properties:\n  claim_tag: open\n";
        assert_eq!(count_in_node(node, fields.for_class("lead")).open, 2);
    }

    /// The reason the ontology declares *which* field, rather than any bare `open` counting.
    ///
    /// A ballot measure with `status: open` is not an open claim, and a corpus cannot opt out
    /// of a false positive it never asked for.
    ///
    /// The class here declares **both** kinds of property, which is the version that bites: a
    /// first draft of this test used a class declaring nothing, so removing the `type: claim`
    /// filter entirely still passed it. A guarantee about which fields are read cannot be
    /// tested by a corpus with no fields.
    #[test]
    fn a_declared_non_claim_field_holding_the_word_open_is_not_a_claim() {
        let (_t, corpus) = corpus_with(
            "class: lead\nlabel: Lead\nproperties:\n  \
             - name: status\n    type: string\n    description: Procedural state.\n  \
             - name: claim_tag\n    type: claim\n    description: The tag.\n",
        );
        let fields = ClaimFields::load(&corpus);
        assert_eq!(
            fields.for_class("lead"),
            ["claim_tag"],
            "only the claim-typed one"
        );

        let node = "class: lead\nlabel: A measure\nproperties:\n  status: open\n";
        assert!(!is_open_question(
            "A measure",
            node,
            fields.for_class("lead")
        ));
        assert_eq!(count_in_node(node, fields.for_class("lead")).open, 0);
    }

    /// A corpus declaring no claim field at all reads exactly as it did before.
    #[test]
    fn a_class_declaring_no_claim_field_contributes_none() {
        let (_t, corpus) = corpus_with("class: lead\nlabel: Lead\n");
        // Zero declared fields is the common case, and means this reads exactly as it did
        // before — which is what makes the feature additive rather than a migration.
        assert!(ClaimFields::load(&corpus).for_class("lead").is_empty());
    }

    /// A corpus that declares nothing behaves exactly as it did. That is what makes this
    /// additive rather than a migration.
    #[test]
    fn a_prose_corpus_is_unaffected() {
        let node = "class: lead\nlabel: A lead\ndescription: Unresolved. [open]\n";
        assert!(is_open_question("A lead", node, &[]));
        assert_eq!(count_in_node(node, &[]), count_in_source(node));
        assert!(is_open_question("? A question", "class: lead\n", &[]));
    }

    /// Both spellings, so nobody reshapes their data a second time.
    #[test]
    fn either_spelling_of_the_value_is_read() {
        let (_t, corpus) = corpus_with(TYPED);
        let f = ClaimFields::load(&corpus);
        for value in ["open", "\"[open]\"", " open "] {
            let node = format!("class: lead\nlabel: L\nproperties:\n  claim_tag: {value}\n");
            assert!(
                is_open_question("L", &node, f.for_class("lead")),
                "{value:?}"
            );
        }
    }

    /// A node making two statements about itself is two claims.
    #[test]
    fn a_list_of_tags_is_one_claim_each() {
        let (_t, corpus) = corpus_with(TYPED);
        let f = ClaimFields::load(&corpus);
        let node = "class: lead\nlabel: L\nproperties:\n  claim_tag:\n    - verified\n    - open\n";
        let counts = count_in_node(node, f.for_class("lead"));
        assert_eq!((counts.verified, counts.open), (1, 1));
    }

    /// The field is found wherever the corpus put it. The key name was declared, so depth
    /// costs nothing — and a projected corpus does not have to flatten itself first.
    #[test]
    fn a_nested_declared_field_is_found() {
        let (_t, corpus) = corpus_with(TYPED);
        let f = ClaimFields::load(&corpus);
        let node = "class: lead\nlabel: L\nproperties:\n  evidence:\n    - claim_tag: open\n";
        assert!(is_open_question("L", node, f.for_class("lead")));
    }

    /// The class name comes from the filename when the field is absent, matching every other
    /// reader of an `.ont.yml`.
    #[test]
    fn a_class_is_named_by_its_file_when_it_does_not_say() {
        let (_t, corpus) = corpus_with(
            "label: Lead\nproperties:\n  - name: claim_tag\n    type: claim\n    description: x\n",
        );
        assert_eq!(ClaimFields::load(&corpus).for_class("lead"), ["claim_tag"]);
    }
}
