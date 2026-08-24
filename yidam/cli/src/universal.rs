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
pub struct Universal(Vec<UniversalProperty>);

#[derive(Default, serde::Deserialize)]
struct File {
    #[serde(default)]
    properties: Vec<Declared>,
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
        Self(Vec::new())
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
        Self(
            file.properties
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
                .collect(),
        )
    }

    pub fn load(root: &Path) -> Self {
        Self::parse(&std::fs::read_to_string(Self::path(root)).unwrap_or_default())
    }

    /// The type declared for this property name, if any class may carry it.
    pub fn declared_type(&self, key: &str) -> Option<&str> {
        self.0
            .iter()
            .find(|p| p.matches(key))
            .map(|p| p.r#type.as_str())
    }

    /// Whether any class may carry this property name.
    pub fn covers(&self, key: &str) -> bool {
        self.0.iter().any(|p| p.matches(key))
    }

    /// The exact-named declarations, for the schema compiler to fold into each class.
    pub fn named(&self) -> impl Iterator<Item = (&str, &str)> {
        self.0
            .iter()
            .filter_map(|p| Some((p.name.as_deref()?, p.r#type.as_str())))
    }

    /// The pattern declarations, as `(regex source, type)` — JSON Schema's
    /// `patternProperties` is keyed by the pattern itself.
    pub fn patterns(&self) -> impl Iterator<Item = (&str, &str)> {
        self.0
            .iter()
            .filter_map(|p| Some((p.pattern.as_ref()?.as_str(), p.r#type.as_str())))
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
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
}
