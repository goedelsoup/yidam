//! A property whose value is a verbatim span of a catalogued document — RFC-0046.
//!
//! # What was missing
//!
//! A corpus can say a node rests on a span of another node ([`super::local_citations`]) or of
//! a node in a dependency ([`super::citations`]). It could not say that a value **is** a span of
//! a document it catalogued, checked against the bytes it fetched. `ohio-136th-assembly-hb-96`
//! keeps 364 quotations of the Governor's veto message as `anchors`, declared `type: text`, and
//! five of them were retyped from the wrong revision of the bill in one phase. Every one still
//! read like a quotation, and nothing in the ontology could say it was not one (#1070).
//!
//! `type: quotation` is that declaration. The value names the catalog entry the words come
//! from and the words themselves:
//!
//! ```yaml
//! anchors:
//!   - of: dewine-hb96-veto-messages
//!     span: "(4) If a school district is affected"
//!     sha256: 2a985bfe…   # optional; required when the entry holds more than one artifact
//! ```
//!
//! # Three checks, one partition
//!
//! - **`quotation-unresolved`** — `of:` names no catalog entry, the entry records no artifact,
//!   the pin names none of its artifacts, or it holds several and nothing says which.
//! - **`quotation-span-drift`** — the artifact's bytes are here, and the span is not in them.
//! - **`quotation-unchecked`** — the quotation resolves and its bytes cannot be read here: not
//!   in this machine's vault cache, or not text. Info.
//!
//! # Why the bytes are read from the cache, and what that costs
//!
//! RFC-0023's constraint is that *a vault stores bytes and git stores the record of them*, so
//! the only bytes a lint run can reach are the ones this machine has fetched. A CI runner has
//! none, and there every quotation is `quotation-unchecked`. That is reported rather than
//! passed, because a quotation nobody compared and one that was compared and held are
//! different facts, and rather than failed, because an empty cache is not a defect in the
//! corpus. The check that matters runs where the bytes are: on the machine that fetched them,
//! which is the machine where a quotation gets written.
//!
//! The cached bytes are hashed before they are read. A cache file that no longer hashes to its
//! name is not the artifact the entry records, and a drift found in it would be a finding
//! about the cache, so it is reported unchecked.
//!
//! # Text media only
//!
//! A span is compared with the bytes as stored, markup included, after the whitespace
//! normalization [`super::citations::flatten`] gives every span check here. A PDF has no such
//! reading. The case that motivated the type is a PDF, and a deterministic text reading of one
//! is its own work (#1172), so a quotation of one is reported unchecked, naming its media type.
//!
//! # Why the pin is optional
//!
//! An entry holding one artifact has one revision, and a quotation of it can only mean that
//! one. An entry holding several is the hb-96 case: the words were read from one revision,
//! and a check that passed when *any* revision held them would pass the retyping that
//! motivated this. So the pin is required exactly where there is a choice to record.
//!
//! # A quotation cites its entry
//!
//! A quotation's `of:` names a catalog entry the way an edge's `source:` does, so the citation
//! count reads it too (#1174): an entry quoted and linked from nowhere is not uncited, and a
//! `[verified]` claim resting on a quotation is not unsourced. It counts whether or not the span
//! was found. A bad pin, drift and missing bytes are each reported here, and an edge's
//! `source:` is not held to its bytes either. Both readers take the quotations from
//! [`Declared`], so a quotation these checks skip is one the count skips.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::citations::{flatten, truncate};
use super::model::{Check, Severity, Violation};
use crate::corpus::{normalize, Class, Node, Source};
use crate::parse::CatalogArtifact;
use crate::vault::{Cache, ContentHash};

/// The declared type, named once. `property_type_violation` and the schema compilers
/// dispatch on this string, and a second spelling would be a type one of them had not heard of.
pub const QUOTATION_PROPERTY_TYPE: &str = "quotation";

/// The three check ids, named once, for the reason [`super::local_citations`] names its four.
pub const UNRESOLVED: &str = "quotation-unresolved";
pub const SPAN_DRIFT: &str = "quotation-span-drift";
pub const UNCHECKED: &str = "quotation-unchecked";

/// One quotation, as written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Quotation {
    /// The catalog entry, by stem or by path from the node's directory — the two spellings
    /// an edge's `source:` admits, read by the same function.
    pub of: String,
    pub span: String,
    /// The artifact the span was read from.
    pub sha256: Option<String>,
}

/// Every quotation a value holds, or why it holds none.
///
/// **The one reading of the shape.** `property-type` reports the `Err` and the checks below
/// read the `Ok`, so the two cannot disagree about what a quotation is — `iso_date_parts`'s
/// reason, one type over. A value is one mapping or a non-empty list of them, and a mapping
/// carries `of:` and `span:`, optionally `sha256:`, and nothing else: a fourth key is a typo
/// of one of those three more often than a field, and the compiled schema refuses it too.
pub(crate) fn read(value: &serde_yaml::Value) -> Result<Vec<Quotation>, String> {
    match value {
        serde_yaml::Value::Mapping(m) => one(m).map(|q| vec![q]),
        serde_yaml::Value::Sequence(items) if items.is_empty() => {
            Err("is an empty list — a quotation names what it quotes".to_string())
        }
        serde_yaml::Value::Sequence(items) => items
            .iter()
            .enumerate()
            .map(|(i, item)| match item {
                serde_yaml::Value::Mapping(m) => {
                    one(m).map_err(|why| format!("entry {} {why}", i + 1))
                }
                _ => Err(format!(
                    "entry {} is not a quotation — write `of:` and `span:` beneath it",
                    i + 1
                )),
            })
            .collect(),
        serde_yaml::Value::Null => Err("is empty".to_string()),
        _ => Err("is not a quotation — write `of:` and `span:` beneath it".to_string()),
    }
}

fn one(m: &serde_yaml::Mapping) -> Result<Quotation, String> {
    let mut of = None;
    let mut span = None;
    let mut sha256 = None;
    for (k, v) in m {
        let key = k.as_str().unwrap_or_default();
        let slot = match key {
            "of" => &mut of,
            "span" => &mut span,
            "sha256" => &mut sha256,
            _ => {
                return Err(format!(
                    "has `{key}:`, which a quotation does not — write `of:`, `span:`, and \
                     optionally `sha256:`"
                ))
            }
        };
        match v {
            serde_yaml::Value::String(s) if !s.trim().is_empty() => *slot = Some(s.clone()),
            serde_yaml::Value::String(_) | serde_yaml::Value::Null => {}
            _ => return Err(format!("has `{key}:` holding something that is not text")),
        }
    }
    let of = of.ok_or("has no `of:` — name the catalog entry the span is quoted from")?;
    let span = span.ok_or("has no `span:` — the words, as the document has them")?;
    if let Some(pin) = &sha256 {
        ContentHash::parse(pin).map_err(|e| format!("has a `sha256:` that is {e}"))?;
    }
    Ok(Quotation {
        of: of.trim().to_string(),
        span,
        sha256: sha256.map(|s| s.trim().to_string()),
    })
}

/// Which properties hold quotations, class by class.
///
/// **The one reading of the declaration, and two things ask it.** [`checks`] holds each
/// quotation to the bytes it names. [`super::checks::citations`] counts its `of:` as a citation
/// of the entry (#1174). If each walked the ontology on its own, a quotation could be counted
/// and never checked, and a `[verified]` claim would rest on a value lint never read.
///
/// The class first, then `universal.yml`, which is `property-type`'s order, so a property read
/// here is one that check holds to the same type. A node whose class the corpus does not define
/// declares no quotation, as `property-type` checks nothing on it. The same precedence is
/// written out in three other places (#1186).
pub struct Declared<'a> {
    classes: HashMap<&'a str, &'a Class>,
    universal: &'a crate::universal::Universal,
}

impl<'a> Declared<'a> {
    pub fn new(classes: &'a [Class], universal: &'a crate::universal::Universal) -> Self {
        Self {
            classes: classes.iter().map(|c| (c.name.as_str(), c)).collect(),
            universal,
        }
    }

    /// Every property an instance of `class` declares `type: quotation`, with its reading.
    pub(crate) fn in_node<'n>(
        &self,
        class: &str,
        inst: &'n crate::parse::CorpusInstance,
    ) -> Vec<(&'n str, Result<Vec<Quotation>, String>)> {
        let Some(class) = self.classes.get(class) else {
            return Vec::new();
        };
        inst.properties
            .iter()
            .flatten()
            .filter_map(|(k, value)| {
                let key = k.as_str()?;
                let declared = class
                    .properties
                    .iter()
                    .find(|p| p.name == key)
                    .map(|p| p.r#type.as_str())
                    .or_else(|| self.universal.declared_type(key));
                (declared == Some(QUOTATION_PROPERTY_TYPE)).then(|| (key, read(value)))
            })
            .collect()
    }

    /// The `of:` of every quotation an instance of `class` holds, as written.
    ///
    /// A malformed value names nothing. `property-type` reports it and [`checks`] skips it, so
    /// a claim cannot rest on a value lint could not read.
    pub(crate) fn named_entries(
        &self,
        class: &str,
        inst: &crate::parse::CorpusInstance,
    ) -> Vec<String> {
        self.in_node(class, inst)
            .into_iter()
            .filter_map(|(_, quotations)| quotations.ok())
            .flatten()
            .map(|q| q.of)
            .collect()
    }
}

/// A media type whose bytes are text a span can be found in.
///
/// Absent is not refused here: an entry written by hand may not record one, and the bytes
/// are then tried as UTF-8, which is the question the type would have answered.
fn is_text(media: &str) -> bool {
    let m = media.split(';').next().unwrap_or_default().trim();
    let m = m.to_ascii_lowercase();
    m.starts_with("text/")
        || m.ends_with("+xml")
        || m.ends_with("+json")
        || matches!(
            m.as_str(),
            "application/json" | "application/xml" | "application/yaml" | "application/x-yaml"
        )
}

/// What reading one artifact came to, kept per digest so a document quoted 364 times is
/// hashed once.
#[derive(Clone)]
enum Bytes {
    Text(String),
    /// Why these bytes cannot be compared here, as the tail of a sentence.
    Unreadable(String),
}

fn load(cache: Option<&Cache>, artifact: &CatalogArtifact, hash: &ContentHash) -> Bytes {
    if let Some(media) = artifact.media_type.as_deref().filter(|m| !is_text(m)) {
        return Bytes::Unreadable(format!(
            "is `{media}`, which lint cannot read as text (#1172)"
        ));
    }
    let Some(cache) = cache else {
        return Bytes::Unreadable(
            "cannot be looked for: this machine has no vault cache — set YIDAM_VAULT_CACHE"
                .to_string(),
        );
    };
    let Ok(raw) = std::fs::read(cache.path_of(hash)) else {
        return Bytes::Unreadable(
            "is not in this machine's vault cache — `yidam vault pull` fetches it".to_string(),
        );
    };
    let found = ContentHash::of_bytes(&raw);
    if &found != hash {
        return Bytes::Unreadable(format!(
            "is corrupt in this machine's vault cache: the file hashes to {}",
            found.as_str()
        ));
    }
    match String::from_utf8(raw) {
        Ok(text) => Bytes::Text(flatten(&text)),
        Err(_) => Bytes::Unreadable("is not UTF-8 text".to_string()),
    }
}

/// One finding, before it is filed under a check.
struct Finding {
    check: &'static str,
    rel: String,
    message: String,
}

/// The three checks, over one walk of the corpus.
///
/// `cache` is `None` where this machine has none to resolve, which is a reason every
/// resolving quotation is unchecked rather than a reason lint fails.
#[must_use]
pub fn checks(
    nodes: &[Node],
    declared: &Declared,
    sources: &[Source],
    catalog: &Path,
    cache: Option<&Cache>,
) -> [Check; 3] {
    let entries: HashMap<PathBuf, &Source> =
        sources.iter().map(|s| (normalize(&s.path), s)).collect();
    let mut read_once: HashMap<ContentHash, Bytes> = HashMap::new();
    let mut found = Vec::new();

    for n in nodes {
        let dir = n.path.parent().unwrap_or(&n.path);
        for (key, quotations) in declared.in_node(&super::checks::class_of(n), &n.inst) {
            // A malformed value is `property-type`'s finding, and one finding is enough.
            let Ok(quotations) = quotations else {
                continue;
            };
            for q in quotations {
                let mut report = |check, message: String| {
                    found.push(Finding {
                        check,
                        rel: n.rel.clone(),
                        message: format!("`{key}` quoting `{}`: {message}", q.of),
                    })
                };
                let Some(entry) = super::checks::source_targets(dir, catalog, &q.of)
                    .iter()
                    .find_map(|p| entries.get(p))
                else {
                    report(UNRESOLVED, "names no catalog entry".to_string());
                    continue;
                };
                let recorded: Vec<(&CatalogArtifact, ContentHash)> = entry
                    .artifacts
                    .iter()
                    .filter_map(|a| Some((a, ContentHash::parse(a.sha256.as_deref()?).ok()?)))
                    .collect();
                let artifact = match (&q.sha256, recorded.as_slice()) {
                    (_, []) => {
                        report(
                            UNRESOLVED,
                            format!(
                                "`{}` records no artifact, so there are no bytes to quote — \
                                 `yidam catalog fetch` records one",
                                entry.rel
                            ),
                        );
                        continue;
                    }
                    (Some(pin), _) => match recorded.iter().find(|(_, h)| h.as_str() == pin) {
                        Some(a) => a,
                        None => {
                            report(
                                UNRESOLVED,
                                format!(
                                    "pins `sha256: {pin}`, and `{}` records no artifact \
                                     under it",
                                    entry.rel
                                ),
                            );
                            continue;
                        }
                    },
                    (None, [only]) => only,
                    (None, many) => {
                        report(
                            UNRESOLVED,
                            format!(
                                "`{}` records {} artifacts and this quotation pins none — add \
                                 `sha256:` naming the one the span was read from",
                                entry.rel,
                                many.len()
                            ),
                        );
                        continue;
                    }
                };
                let (meta, hash) = artifact;
                let bytes = read_once
                    .entry(hash.clone())
                    .or_insert_with(|| load(cache, meta, hash));
                match bytes {
                    Bytes::Unreadable(why) => {
                        report(UNCHECKED, format!("the artifact {} {why}", short(hash)))
                    }
                    Bytes::Text(text) if text.contains(&flatten(&q.span)) => {}
                    Bytes::Text(_) => report(
                        SPAN_DRIFT,
                        format!(
                            "\"{}\" is not in artifact {}",
                            truncate(&q.span),
                            short(hash)
                        ),
                    ),
                }
            }
        }
    }

    let file = |id: &str| -> Vec<Violation> {
        found
            .iter()
            .filter(|f| f.check == id)
            .map(|f| Violation::new(&f.rel, f.message.clone()))
            .collect()
    };
    [
        quotation_unresolved(file(UNRESOLVED)),
        quotation_span_drift(file(SPAN_DRIFT)),
        quotation_unchecked(file(UNCHECKED)),
    ]
}

/// A digest short enough to sit in a finding — the twelve characters a person compares by
/// eye, which is all a finding needs when the entry it names holds the rest.
fn short(hash: &ContentHash) -> String {
    format!("{}…", &hash.as_str()[..12])
}

fn quotation_unresolved(violations: Vec<Violation>) -> Check {
    Check::new(
        UNRESOLVED,
        "A quotation names no artifact the catalog records",
        Severity::Error,
        "A property declared `type: quotation` states that its words are in a document this \
         corpus catalogued, and a quotation whose `of:` names no entry, or an entry that \
         records no artifact, names no bytes those words could be in. An entry holding \
         several artifacts is several revisions, and a quotation that does not pin one with \
         `sha256:` would be satisfied by whichever revision happens to hold the words — the \
         retyping from the wrong revision that motivated the type. Error: the population is \
         empty in a corpus that declares no quotation, so no repository that predates it is \
         put in debt.",
        violations,
    )
}

fn quotation_span_drift(violations: Vec<Violation>) -> Check {
    Check::new(
        SPAN_DRIFT,
        "A quotation's span is not in the artifact it names",
        Severity::Error,
        "The artifact's bytes are in this machine's vault cache, they hash to the digest the \
         catalog records, and the span is not in them. Whitespace is normalized on both sides \
         and nothing else is, so a re-wrapped quotation compares equal and a retyped one does \
         not. A span read from a rendered page is compared with the bytes as stored, markup \
         included. The repair is to read the document again and copy the words from it; \
         editing the span until the finding clears is the one response that destroys what \
         the check is for.",
        violations,
    )
}

fn quotation_unchecked(violations: Vec<Violation>) -> Check {
    Check::new(
        UNCHECKED,
        "A quotation whose artifact cannot be read here",
        Severity::Info,
        "A vault stores bytes and git stores the record of them, so a lint run can compare \
         only the bytes this machine has fetched, and only when they are text. On a CI runner \
         every quotation lands here. Reported rather than passed, because a quotation nobody \
         compared and one that held are different facts; Info, because an empty cache is not \
         a defect in the corpus. `yidam vault pull` fetches the bytes, and a PDF waits on a \
         text reading of one (#1172).",
        violations,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::corpus::Overlay;
    use crate::walk::walk_corpus_instances;

    const CLASS: &str = "properties:\n  - name: anchors\n    type: quotation\n";

    const MESSAGE: &str = "Therefore, a veto of this item\n  is in the public interest.\n";

    fn yaml(s: &str) -> serde_yaml::Value {
        serde_yaml::from_str(s).unwrap()
    }

    /// A corpus with one `provision` node quoting `anchors`, a catalog entry recording the
    /// given artifacts, and a vault cache holding the given bytes.
    struct Fixture {
        dir: tempfile::TempDir,
    }

    impl Fixture {
        fn new(anchors: &str, artifacts: &str, cached: &[&[u8]]) -> Self {
            let dir = tempfile::tempdir().unwrap();
            let root = dir.path();
            std::fs::create_dir_all(root.join(".yidam/corpus/provision")).unwrap();
            std::fs::create_dir_all(root.join(".yidam/catalog")).unwrap();
            std::fs::write(root.join(".yidam/corpus/provision.ont.yml"), CLASS).unwrap();
            std::fs::write(
                root.join(".yidam/corpus/provision/item-12.yml"),
                format!("class: provision\nlabel: Item 12\nproperties:\n  anchors:\n{anchors}"),
            )
            .unwrap();
            std::fs::write(
                root.join(".yidam/catalog/veto-message.md"),
                format!("---\nname: veto-message\nartifacts:\n{artifacts}---\n\n# Veto\n"),
            )
            .unwrap();
            let cache = Cache::at(root.join("cache"));
            for bytes in cached {
                let hash = ContentHash::of_bytes(bytes);
                let path = cache.path_of(&hash);
                std::fs::create_dir_all(path.parent().unwrap()).unwrap();
                std::fs::write(path, bytes).unwrap();
            }
            Self { dir }
        }

        fn run(&self, with_cache: bool) -> [Check; 3] {
            let root = self.dir.path();
            let corpus = crate::paths::yidam_corpus_dir(root);
            let overlay = Overlay::default();
            let nodes = crate::corpus::load_nodes(root, &walk_corpus_instances(&corpus), &overlay);
            let classes =
                crate::corpus::load_classes(root, &[corpus.join("provision.ont.yml")], &overlay);
            let sources = crate::corpus::load_sources(
                root,
                &[root.join(".yidam/catalog/veto-message.md")],
                &overlay,
            );
            let cache = Cache::at(root.join("cache"));
            let universal = crate::universal::Universal::default();
            checks(
                &nodes,
                &Declared::new(&classes, &universal),
                &sources,
                &root.join(".yidam/catalog"),
                with_cache.then_some(&cache),
            )
        }
    }

    fn artifact(bytes: &[u8], media: &str) -> String {
        format!(
            "  - sha256: {}\n    media_type: {media}\n",
            ContentHash::of_bytes(bytes).as_str()
        )
    }

    /// Every finding, as `(check, detail)`.
    fn findings(checks: &[Check; 3]) -> Vec<(String, String)> {
        checks
            .iter()
            .flat_map(|c| {
                c.violations
                    .iter()
                    .map(|v| (c.id.to_string(), v.detail.clone()))
            })
            .collect()
    }

    fn only(checks: &[Check; 3]) -> (String, String) {
        let all = findings(checks);
        assert_eq!(all.len(), 1, "expected exactly one finding, got {all:?}");
        all.into_iter().next().unwrap()
    }

    #[test]
    fn a_span_the_cached_bytes_hold_reports_nothing_even_rewrapped() {
        let f = Fixture::new(
            "    of: veto-message\n    span: a veto of this item is in the public interest\n",
            &artifact(MESSAGE.as_bytes(), "text/plain"),
            &[MESSAGE.as_bytes()],
        );
        assert!(findings(&f.run(true)).is_empty());
    }

    #[test]
    fn a_retyped_span_is_drift() {
        let f = Fixture::new(
            "    - of: veto-message\n      span: a veto of this item is in the public good\n",
            &artifact(MESSAGE.as_bytes(), "text/plain"),
            &[MESSAGE.as_bytes()],
        );
        let (check, detail) = only(&f.run(true));
        assert_eq!(check, SPAN_DRIFT);
        assert!(detail.contains("public good"), "{detail}");
    }

    #[test]
    fn a_path_spelling_of_the_entry_resolves_like_a_stem() {
        let f = Fixture::new(
            "    of: ../../catalog/veto-message.md\n    span: veto of this item\n",
            &artifact(MESSAGE.as_bytes(), "text/plain"),
            &[MESSAGE.as_bytes()],
        );
        assert!(findings(&f.run(true)).is_empty());
    }

    #[test]
    fn an_entry_the_catalog_does_not_hold_is_unresolved() {
        let f = Fixture::new(
            "    of: veto-messages\n    span: veto\n",
            &artifact(MESSAGE.as_bytes(), "text/plain"),
            &[],
        );
        let (check, detail) = only(&f.run(true));
        assert_eq!(check, UNRESOLVED);
        assert!(detail.contains("names no catalog entry"), "{detail}");
    }

    #[test]
    fn an_entry_recording_no_artifact_is_unresolved() {
        let f = Fixture::new("    of: veto-message\n    span: veto\n", "  []\n", &[]);
        let (check, detail) = only(&f.run(true));
        assert_eq!(check, UNRESOLVED);
        assert!(detail.contains("records no artifact"), "{detail}");
    }

    /// The hb-96 case: two revisions, and the words are only in the one not read from.
    #[test]
    fn several_revisions_need_a_pin_and_the_pin_is_what_is_checked() {
        let enrolled = b"is in the public interest";
        let introduced = b"is in the public good";
        let both = format!(
            "{}{}",
            artifact(enrolled, "text/plain"),
            artifact(introduced, "text/plain")
        );
        let unpinned = Fixture::new(
            "    of: veto-message\n    span: public good\n",
            &both,
            &[enrolled, introduced],
        );
        let (check, detail) = only(&unpinned.run(true));
        assert_eq!(check, UNRESOLVED);
        assert!(detail.contains("records 2 artifacts"), "{detail}");

        let pinned = Fixture::new(
            &format!(
                "    of: veto-message\n    span: public good\n    sha256: {}\n",
                ContentHash::of_bytes(enrolled).as_str()
            ),
            &both,
            &[enrolled, introduced],
        );
        assert_eq!(only(&pinned.run(true)).0, SPAN_DRIFT);
    }

    #[test]
    fn a_pin_the_entry_does_not_record_is_unresolved() {
        let f = Fixture::new(
            &format!(
                "    of: veto-message\n    span: veto\n    sha256: {}\n",
                ContentHash::of_bytes(b"another document").as_str()
            ),
            &artifact(MESSAGE.as_bytes(), "text/plain"),
            &[MESSAGE.as_bytes()],
        );
        let (check, detail) = only(&f.run(true));
        assert_eq!(check, UNRESOLVED);
        assert!(detail.contains("records no artifact under it"), "{detail}");
    }

    #[test]
    fn bytes_this_machine_does_not_hold_are_unchecked_not_failed() {
        let f = Fixture::new(
            "    of: veto-message\n    span: anything at all\n",
            &artifact(MESSAGE.as_bytes(), "text/plain"),
            &[],
        );
        let (check, detail) = only(&f.run(true));
        assert_eq!(check, UNCHECKED);
        assert!(detail.contains("yidam vault pull"), "{detail}");
        assert_eq!(only(&f.run(false)).0, UNCHECKED);
    }

    #[test]
    fn a_pdf_is_unchecked_by_its_media_type() {
        let f = Fixture::new(
            "    of: veto-message\n    span: veto\n",
            &artifact(b"%PDF-1.7", "application/pdf"),
            &[b"%PDF-1.7"],
        );
        let (check, detail) = only(&f.run(true));
        assert_eq!(check, UNCHECKED);
        assert!(detail.contains("application/pdf"), "{detail}");
    }

    /// A cache file that no longer hashes to its name is not the artifact the entry records,
    /// and a drift found in it would be a finding about the cache.
    #[test]
    fn a_corrupt_cache_file_is_unchecked_not_drift() {
        let f = Fixture::new(
            "    of: veto-message\n    span: not in the corrupted file\n",
            &artifact(MESSAGE.as_bytes(), "text/plain"),
            &[],
        );
        let cache = Cache::at(f.dir.path().join("cache"));
        let path = cache.path_of(&ContentHash::of_bytes(MESSAGE.as_bytes()));
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, "something else entirely").unwrap();
        let (check, detail) = only(&f.run(true));
        assert_eq!(check, UNCHECKED);
        assert!(detail.contains("corrupt"), "{detail}");
    }

    #[test]
    fn the_shape_is_one_mapping_or_a_list_of_them() {
        assert_eq!(read(&yaml("of: a\nspan: b\n")).unwrap().len(), 1);
        assert_eq!(
            read(&yaml("- {of: a, span: b}\n- {of: c, span: d}\n"))
                .unwrap()
                .len(),
            2
        );
        for (written, says) in [
            ("just some words", "is not a quotation"),
            ("[]", "empty list"),
            ("span: b\n", "has no `of:`"),
            ("of: a\n", "has no `span:`"),
            ("of: a\nspan: '  '\n", "has no `span:`"),
            ("of: a\nspan: b\npage: 4\n", "has `page:`"),
            ("of: a\nspan: 7\n", "not text"),
            ("of: a\nspan: b\nsha256: ABC\n", "`sha256:`"),
            ("- {of: a, span: b}\n- {of: c}\n", "entry 2 has no `span:`"),
            (
                "- {of: a, span: b}\n- words\n",
                "entry 2 is not a quotation",
            ),
        ] {
            let why = read(&yaml(written)).unwrap_err();
            assert!(
                why.contains(says),
                "{written:?} said {why:?}, wanted {says:?}"
            );
        }
    }

    #[test]
    fn text_media_are_the_ones_read() {
        for m in [
            "text/plain",
            "text/html; charset=utf-8",
            "application/xhtml+xml",
            "application/json",
        ] {
            assert!(is_text(m), "{m}");
        }
        for m in ["application/pdf", "image/png", "application/octet-stream"] {
            assert!(!is_text(m), "{m}");
        }
    }
}
