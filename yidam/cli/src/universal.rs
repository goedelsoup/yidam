//! Properties any class may carry, whatever its own ontology declares.
//!
//! `.yidam/corpus/universal.yml` is the corpus speaking about itself rather than about one
//! of its classes, and it exists because the alternative was measured and rejected. A
//! derived repository carries `seeded_because` — *why this node is in the corpus at all* —
//! on nodes of six different classes. Declaring it per class would have been sixteen copies
//! of one decision, and a seventeenth class would silently not have it.
//!
//! The same file answers the other half. That corpus also carries `fy2024_profile`,
//! `fy2021_idea_part_b`, `fy2027_scale`: a fiscal year's figures pasted onto the node they
//! describe. They recur, so they are not typos, but declaring them by name would mean
//! editing an ontology every July to permit next year's. The year in the name is what makes
//! them self-describing, so a `pattern:` matches them and a `name:` matches the first kind.
//!
//! Between them these were 29 of that corpus's 29 `undeclared-property` findings — every
//! one of which was the corpus working as designed.
//!
//! # The same file says which relationships assert something
//!
//! `edge_claims:` is the other thing a corpus knows about itself and cannot say per class. An
//! edge is a claim written as structure (`guidelines/agent-conduct.md`, *An edge is a claim*),
//! and a corpus that tags its edges has to name the relationships that are **bookkeeping** —
//! `instance-of`, `concerns`, `subject-of` — because a tag on one of those would assert a
//! standing about nothing. Those relationships are authored by many classes at once, so
//! declaring them per class is the same sixteen copies of one decision the property list
//! exists to avoid.
//!
//! The exemption list and the gate are **separate keys**, and that is deliberate. A corpus
//! naming three structural verbs is exempting them; it is not asking to have every other edge
//! in the corpus reported. `required: true` is the ask, written once and on purpose.
//!
//! # Not a way to stop declaring things
//!
//! This is deliberately *not* `property_policy: characteristic`, the property-side twin of
//! [`crate::cmd::lint::checks::EdgePolicy`]. That corpus measured its own property
//! vocabulary at 94% declared and its relationship vocabulary at 68%, and concluded the
//! first was effectively closed and worth gating on. It wants `undeclared-property`; what
//! it lacked was a way to say that two specific shapes are apparatus rather than schema.
//! A blanket opt-out would have thrown away the gate that catches the next real typo.

use std::path::Path;

use regex::Regex;

/// One universally-permitted property: matched by exact name, or by a pattern.
pub struct UniversalProperty {
    /// The exact property name, when the declaration named one.
    pub name: Option<String>,
    /// A compiled anchor-free regex over the property name, when it declared a pattern.
    /// A declaration whose pattern does not compile is dropped rather than fatal — see
    /// [`Universal::load`].
    pub pattern: Option<Regex>,
    /// The declared type, checked by `property-type` exactly as a class's own would be.
    /// Universal does not mean untyped: `seeded_because` is prose and a fiscal-year
    /// snapshot is prose, and a `claim` written into either is still counted as no claim.
    pub r#type: String,
}

impl UniversalProperty {
    fn matches(&self, key: &str) -> bool {
        self.name.as_deref() == Some(key) || self.pattern.as_ref().is_some_and(|p| p.is_match(key))
    }
}

/// Every universal property this corpus declares. Empty when the file is absent, which is
/// every corpus that has not needed one.
#[derive(Default)]
pub struct Universal {
    properties: Vec<UniversalProperty>,
    /// Top-level keys any class's instances may carry as prose.
    ///
    /// The same argument the property list makes, one axis over: `summary` is apparatus that
    /// applies to every class, and declaring it per class would be sixteen copies of one
    /// decision with a seventeenth class silently missing it. See [`crate::prose`].
    prose: Vec<String>,
    /// What the corpus has said about tagging its edges. See [`EdgeClaims`].
    edge_claims: EdgeClaims,
}

/// What a corpus has said about the standing of its own edges (#587).
///
/// An edge is a claim written as structure, and until this existed the corpus's most
/// load-bearing assertions were the only ones exempt from its own evidence discipline: a
/// `claim_tag` on a link survived the parse and nothing read it.
///
/// **Two keys and not one**, because they answer two different questions. `structural:` says
/// which relationships assert nothing about the world; `required:` says that everything else
/// does and must name its standing. A corpus could want the first and not the second — the
/// exemption is a fact about its vocabulary — and turning a gate on as a side effect of
/// recording that fact is the shape this repository has rejected for `edges:` and for
/// `properties:` both.
#[derive(Default)]
pub struct EdgeClaims {
    /// Whether an edge outside [`Self::structural`] must carry `claim_tag`.
    ///
    /// **Absent means no, and the default cannot be otherwise.** Every corpus written before
    /// the field existed tags none of its edges, so a default of `true` would open with one
    /// finding per empirical edge — 1,270 of them in the repository that filed #587, and the
    /// whole graph in one that has never heard of the practice. That is the gate arriving in
    /// a corpus that never agreed to it, which is what `required:` on a class property and
    /// `edge_policy:` on a class both exist to avoid.
    required: bool,
    /// Relationships that are bookkeeping rather than empirical — exempt from tagging.
    ///
    /// Held as written, compared as written: a relationship is already a token the ontology
    /// declares and the instances spell, and normalizing here would be a second opinion about
    /// what `unlicensed-edge` matches literally.
    structural: Vec<String>,
}

#[derive(Default, serde::Deserialize)]
struct File {
    #[serde(default)]
    properties: Vec<Declared>,
    #[serde(default)]
    prose: Vec<String>,
    #[serde(default)]
    edge_claims: DeclaredEdgeClaims,
}

#[derive(Default, serde::Deserialize)]
struct DeclaredEdgeClaims {
    #[serde(default)]
    required: bool,
    #[serde(default)]
    structural: Vec<String>,
}

#[derive(serde::Deserialize)]
struct Declared {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    pattern: Option<String>,
    #[serde(default)]
    r#type: String,
    #[serde(default)]
    #[allow(dead_code)]
    description: String,
}

impl Universal {
    /// A corpus that declares nothing universal, without touching a disk.
    pub const fn empty() -> Self {
        Self {
            properties: Vec::new(),
            prose: Vec::new(),
            edge_claims: EdgeClaims {
                required: false,
                structural: Vec::new(),
            },
        }
    }

    pub fn path(root: &Path) -> std::path::PathBuf {
        crate::paths::yidam_corpus_dir(root).join("universal.yml")
    }

    /// Read the declarations. `text` is the file's contents — the caller supplies it so the
    /// editor can lint an unsaved buffer, as it does for every class.
    ///
    /// **A declaration that does not compile is dropped, not fatal.** This runs inside the
    /// gate, and a half-typed pattern must degrade to *this property is not universal* —
    /// which reports the property — rather than taking the whole lint run down and leaving
    /// the corpus unchecked. `yidam schema` publishes the file's shape, so the malformed
    /// pattern is underlined where it is being typed.
    pub fn parse(text: &str) -> Self {
        let file: File = serde_yaml::from_str(text).unwrap_or_default();
        let properties = file
            .properties
            .into_iter()
            .filter_map(|d| {
                let pattern = match d.pattern.as_deref() {
                    None => None,
                    Some(p) => Some(Regex::new(p).ok()?),
                };
                (d.name.is_some() || pattern.is_some()).then_some(UniversalProperty {
                    name: d.name,
                    pattern,
                    r#type: d.r#type,
                })
            })
            .collect();
        Self {
            properties,
            prose: file
                .prose
                .into_iter()
                .map(|k| k.trim().to_string())
                .filter(|k| !k.is_empty())
                .collect(),
            edge_claims: EdgeClaims {
                required: file.edge_claims.required,
                structural: file
                    .edge_claims
                    .structural
                    .into_iter()
                    .map(|r| r.trim().to_string())
                    .filter(|r| !r.is_empty())
                    .collect(),
            },
        }
    }

    /// Whether an edge outside the structural list must declare its standing.
    pub fn edge_claims_required(&self) -> bool {
        self.edge_claims.required
    }

    /// Whether this relationship is bookkeeping, and so asserts nothing to tag.
    pub fn is_structural_relationship(&self, relationship: &str) -> bool {
        self.edge_claims
            .structural
            .iter()
            .any(|r| r == relationship)
    }

    /// The structural relationships, as declared — what the schema and the report echo back.
    pub fn structural_relationships(&self) -> &[String] {
        &self.edge_claims.structural
    }

    /// The top-level prose keys every class may carry.
    pub fn prose(&self) -> &[String] {
        &self.prose
    }

    pub fn load(root: &Path) -> Self {
        Self::parse(&std::fs::read_to_string(Self::path(root)).unwrap_or_default())
    }

    /// The type declared for this property name, if any class may carry it.
    pub fn declared_type(&self, key: &str) -> Option<&str> {
        self.properties
            .iter()
            .find(|p| p.matches(key))
            .map(|p| p.r#type.as_str())
    }

    /// Whether any class may carry this property name.
    pub fn covers(&self, key: &str) -> bool {
        self.properties.iter().any(|p| p.matches(key))
    }

    /// The exact-named declarations, for the schema compiler to fold into each class.
    pub fn named(&self) -> impl Iterator<Item = (&str, &str)> {
        self.properties
            .iter()
            .filter_map(|p| Some((p.name.as_deref()?, p.r#type.as_str())))
    }

    /// The pattern declarations, as `(regex source, type)` — JSON Schema's
    /// `patternProperties` is keyed by the pattern itself.
    pub fn patterns(&self) -> impl Iterator<Item = (&str, &str)> {
        self.properties
            .iter()
            .filter_map(|p| Some((p.pattern.as_ref()?.as_str(), p.r#type.as_str())))
    }

    /// Whether the corpus declared nothing universal at all — properties, prose or edges.
    ///
    /// Every axis, deliberately. A reader asking whether this file says anything is asking
    /// about the file, and answering from the property list alone would report a corpus that
    /// declares only `prose:` — or only `edge_claims:` — as having declared nothing.
    pub fn is_empty(&self) -> bool {
        self.properties.is_empty()
            && self.prose.is_empty()
            && !self.edge_claims.required
            && self.edge_claims.structural.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const OHIO: &str = r#"
properties:
  - name: seeded_because
    type: text
    description: Why this node is in the corpus at all
  - pattern: '^fy\d{4}(_\d{2})?_[a-z0-9_]+$'
    type: text
    description: A fiscal year's figures, pasted onto the node they describe
"#;

    /// The two shapes that motivated the file, verbatim from the corpus that reported them.
    #[test]
    fn both_declaration_shapes_match_what_they_were_written_for() {
        let u = Universal::parse(OHIO);
        for key in [
            "seeded_because",
            "fy2024_profile",
            "fy2021_idea_part_b",
            "fy2027_scale",
        ] {
            assert!(u.covers(key), "{key} should be universal");
            assert_eq!(u.declared_type(key), Some("text"));
        }
    }

    /// A pattern is a licence for a *shape*, not for anything that starts the same way.
    /// `fy` alone is not a fiscal-year stamp, and a property misspelt into near-miss is
    /// exactly what `undeclared-property` is for.
    #[test]
    fn a_pattern_does_not_license_a_near_miss() {
        let u = Universal::parse(OHIO);
        for key in ["fyi_note", "fy24_profile", "seeded", "seeded_because_x"] {
            assert!(!u.covers(key), "{key} must still be reported");
        }
    }

    /// Absent is the common case and must read as *nothing is universal* rather than as an
    /// error — a corpus that never needed the file is not misconfigured.
    #[test]
    fn an_absent_or_empty_file_licenses_nothing() {
        for text in ["", "properties: []", "{}"] {
            let u = Universal::parse(text);
            assert!(u.is_empty());
            assert!(!u.covers("anything"));
        }
    }

    /// A pattern still being typed must not take the gate down with it. The property is
    /// reported — which is the safe direction — and the rest of the file still stands.
    #[test]
    fn a_pattern_that_does_not_compile_is_dropped_not_fatal() {
        let u = Universal::parse(
            "properties:\n  - pattern: '[unclosed'\n    type: text\n  - name: seeded_because\n    type: text\n",
        );
        assert!(!u.covers("anything"));
        assert!(
            u.covers("seeded_because"),
            "the rest of the file still stands"
        );
    }

    /// A declaration naming neither is not a declaration, and must not match every property
    /// by matching nothing.
    #[test]
    fn a_declaration_naming_neither_licenses_nothing() {
        let u = Universal::parse("properties:\n  - type: text\n    description: nothing\n");
        assert!(u.is_empty());
    }

    // ── edge claims (#587) ────────────────────────────────────────────────────

    const TAGGED_EDGES: &str = "edge_claims:\n  required: true\n  structural:\n    - instance-of\n    - concerns\n    - subject-of\n";

    #[test]
    fn the_declared_structural_relationships_are_exempt_and_nothing_else_is() {
        let u = Universal::parse(TAGGED_EDGES);
        assert!(u.edge_claims_required());
        for rel in ["instance-of", "concerns", "subject-of"] {
            assert!(u.is_structural_relationship(rel), "{rel} is declared");
        }
        for rel in ["located-in", "within", "flows-into", "instance", "concern"] {
            assert!(!u.is_structural_relationship(rel), "{rel} is not declared");
        }
        assert_eq!(u.structural_relationships().len(), 3);
    }

    /// The exemption list is a fact about the vocabulary; the gate is a separate ask. A
    /// corpus recording the first must not find the second switched on.
    #[test]
    fn naming_structural_relationships_does_not_turn_the_gate_on() {
        let u = Universal::parse("edge_claims:\n  structural:\n    - instance-of\n");
        assert!(!u.edge_claims_required());
        assert!(u.is_structural_relationship("instance-of"));
        assert!(!u.is_empty(), "the file said something");
    }

    /// And the gate without a list is legal: a corpus whose every relationship is empirical
    /// has nothing to exempt.
    #[test]
    fn the_gate_stands_alone_when_there_is_nothing_to_exempt() {
        let u = Universal::parse("edge_claims:\n  required: true\n");
        assert!(u.edge_claims_required());
        assert!(u.structural_relationships().is_empty());
    }
}
