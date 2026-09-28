//! The ontology: `<class>.ont.yml` parsed once, and the two derivations over `direction:`.

use std::collections::HashSet;
use std::path::Path;

/// Whether a class's `edges:` list bounds what may be said about it, or merely describes it.
///
/// The distinction the ontology could not previously make, and the reason `unlicensed-edge`
/// reported 210 errors against a corpus that was doing nothing wrong. A non-empty `edges:`
/// says *these relationships exist*; on its own it does not say *and no others may*. Reading
/// it as the second is the same over-reading [`source_classes`] refuses when it
/// declines to treat an empty `edges:` as a contract — one field further in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EdgePolicy {
    /// `edges:` names what the class is *defined by*. A relationship outside it is a
    /// deliberate coinage, and not reported: the corpus was asked the question and answered
    /// it. In the corpus that prompted this, 107 distinct undeclared relationships carried
    /// 210 edges and `bears-on` alone carried 16 — a vocabulary, not a typo pile.
    Characteristic,
    /// The vocabulary is closed. Anything outside `edges:` is an error, because the class
    /// said it would be.
    Exhaustive,
    /// The class has not said, which is every class written before the field existed.
    ///
    /// Reported, and does not gate. The typo case is real and worth seeing; gating on it
    /// would enforce a contract nobody wrote. Measured in both directions before choosing:
    /// of the three derived corpora, the two that declare no policy trip this check zero
    /// times either way, and the one that declares `characteristic` on all 18 of its
    /// classes drops from 210 errors to nothing.
    #[default]
    Unstated,
}

impl EdgePolicy {
    /// Parse the declared value. An unrecognized one is [`EdgePolicy::Unstated`] rather
    /// than an error, for the reason [`crate::cmd::lint::checks::property_type_violation`] leaves an unknown type
    /// alone: a check that failed on vocabulary it had not heard of would make coining any
    /// impossible. `yidam schema` publishes the enum, so a typo is underlined in the editor
    /// where it can be fixed as it is typed.
    fn parse(raw: Option<&str>) -> Self {
        match raw.map(str::trim) {
            Some("characteristic") => Self::Characteristic,
            Some("exhaustive") => Self::Exhaustive,
            _ => Self::Unstated,
        }
    }
}
/// A class definition parsed once — `<class>.ont.yml`.
pub struct Class {
    pub rel: String,
    /// The file's bytes, as they were read.
    ///
    /// Kept for the reason [`crate::corpus::Node::text`] is kept, and against the same defect: `load_classes`
    /// already had this string in hand and threw it away, so a check over a class's *prose*
    /// had nothing to read but the fields serde happened to keep.
    ///
    /// **The bytes rather than the fields, because the fields are not all of the prose.**
    /// A class writes prose in four places, and measured across 17 corpora the 21 evidence-tag
    /// sites in class files fall in all four: 9 in `description`, 6 in `properties[].description`,
    /// 2 in `edges[].description`, and 4 in `analytic_note`. [`ClassProperty`] and [`ClassEdge`]
    /// declare no `description` field at all — serde parses two of those four straight out of
    /// existence — and `analytic_note` is not on [`ClassFields`] either. A scan over the text
    /// reaches all four at once, and cannot fall behind the next prose field somebody adds.
    pub text: String,
    /// The `description` field: the text that says what kind of thing an instance is.
    /// Deliberately the only field [`crate::cmd::lint::checks::class_asserts_purpose`] reads; see there.
    pub description: String,
    /// The short human name the class gives itself — `Person`, as written.
    ///
    /// Serialised by `graph` and read by nothing else. It is on the model because it was
    /// the field `cmd/graph.rs` could not get from here, and so the reason that module kept
    /// a second parse of every class file (#1116).
    pub label: String,
    /// The class name — `person` for `person.ont.yml`.
    ///
    /// **The file stem, and never the `class:` field inside.** The stem is what governs: an
    /// instance lives in `corpus/<stem>/`, [`crate::cmd::lint::checks::class_of`] resolves a
    /// node's class by reading that directory, and [`super::Corpus::defined_classes`] is the
    /// stem for the same reason. A class file that declares a different `class:` has not
    /// renamed itself; it has written a field nothing resolves by.
    ///
    /// **Four readers used to prefer the declared field, and this is where they stopped.**
    /// `claims`, `prose`, `retrievable` and `cmd/graph.rs` each parsed the class file
    /// themselves and keyed on `class:` where it was present, while `pack`, `score` and the
    /// lint checks keyed on the stem — one corpus, two answers to what a class is called,
    /// and no check reporting the gap. #1116 routed all four through this field.
    ///
    /// **The change is inert on every corpus measured.** 18 corpora, 204 class files, 0 that
    /// declare a `class:` differing from their stem and 0 that omit the field — so no lookup
    /// that resolved before resolves differently. The population is one corpus per
    /// independent checkout on hand: no vendored `.vendor/` or `tonpa/` copy, no
    /// `.claude/worktrees/` or second branch of a repository already counted, and not this
    /// repository's own `examples/` or test fixtures. `yidam schema` requires `class:` on a
    /// class file, which is why the field is always there: a conformant file states the name
    /// its stem already is. **Nothing reports the disagreement**, so a corpus that starts
    /// writing one would be silently re-keyed rather than told; the decision on #1116 was to
    /// document the rule rather than add a check nobody had a case for.
    pub name: String,
    /// The typed fields the class declares, in declaration order.
    pub properties: Vec<ClassProperty>,
    /// The relationships the class licenses, from whichever end authors them.
    pub edges: Vec<ClassEdge>,
    /// Whether [`Self::edges`] is a bound or a description. See [`EdgePolicy`].
    pub edge_policy: EdgePolicy,
    /// The longest an instance's `description` may be, in lines. `None` when the class has
    /// not said, which is every class written before the field existed — and no check runs.
    ///
    /// **Lines of prose, not lines of file.** `node-too-long` counted the bytes as read until
    /// #588 showed what that charges for: frontmatter, properties, and a `claim_tag` and
    /// `source` on every link, so a node documenting where its edges come from paid for the
    /// provenance out of a budget written to stop descriptions sprawling. A corpus that
    /// raised this number to absorb structural lines can lower it again.
    ///
    /// **There is no default, and that is a measurement rather than a shrug.** The bootstrap
    /// rubric's S7 fixes 40 lines, and across 410 nodes in five real corpora **335 of them
    /// exceed it** — 86%, 86% and 97% in the three mature ones. The same corpora at their
    /// genesis commits run to a median of 35, where 40 is right for three of the four. So 40
    /// is a genesis norm that a corpus grows out of, the growth is what a corpus doing its
    /// job looks like, and there is no knee in the distribution to put a steady-state number
    /// at: it runs smoothly from 20 to 534.
    ///
    /// A class knows what its instances are. One holding statutory obligations quoting the
    /// text they arise from is not the same length as one holding a person, and the corpus
    /// is where that is known. Declaring the number is how it becomes checkable; declining
    /// to declare it leaves the corpus exactly as checked as it was.
    pub max_lines: Option<usize>,
    /// Top-level keys this class's instances carry as prose, beyond `description`.
    ///
    /// Empty is every class written before the field existed, and reads as *this class said
    /// nothing about prose* rather than as *this class has none* — `description` is prose
    /// whatever anything declares. See [`crate::prose`].
    pub prose: Vec<String>,
    /// The type in `crates/` that implements this class — `Intervention`, as written.
    ///
    /// **`None` is the overwhelming default and no check runs, because the ontology is not
    /// a specification of the code.** Measured over twelve derived corpora: 129 of their
    /// 157 declared classes have no `struct` or `enum` bearing their name, and widening the
    /// match to traits, aliases and every language in the tree makes it *worse* — 165 of
    /// 186, 88%. Five of those corpora match nothing at all. The reason is not that they
    /// are behind: their ontologies model a research domain and their `crates/` model the
    /// pipeline that gathers evidence about it, and there is no expectation that the two
    /// share a name. An unconditional check would call 88% of every ontology debt, which is
    /// exactly the permanently non-empty report [`crate::cmd::check_diff`] was
    /// diff-scoped to avoid.
    ///
    /// So the class says it, or nothing is said — the shape [`Self::max_lines`] and
    /// [`ClassProperty::required`] already have. A class that writes this has made a
    /// statement about the tree, and a tree that contradicts it is the ontology being
    /// contradicted rather than an omission, which is why [`crate::cmd::lint::checks::unimplemented_class`] gates
    /// where `missing-property` does not.
    pub implemented_by: Option<String>,
    /// The declared foundational alignment, or `None` when the corpus chose none — which is
    /// every corpus this repository ships, and a legitimate answer the bootstrap dialogue
    /// offers by name.
    pub foundational_type: Option<FoundationalType>,
    /// Dead alignment spellings this class file carries, as written. Empty for almost every
    /// class; [`crate::cmd::lint::checks::foundational_field_misspelled`] is the only reader.
    pub dead_alignment_fields: Vec<&'static str>,
    /// Why the bytes did not parse, when they did not. See [`super::parse_or_default`].
    ///
    /// **The arm this field exists for.** An unreadable instance produces findings that
    /// contradict the file it names; an unreadable *class* produces no findings at all,
    /// because every check that reads a declaration reads an empty list and has nothing to
    /// say. Nothing else on this struct can tell a class that declares no properties from a
    /// class nobody could read, and the two are opposite: the first is an ontology that has
    /// not been filled in, the second is a gate that has been switched off.
    pub malformed: Option<String>,
}
/// One typed field a class declares.
#[derive(Default, serde::Deserialize)]
pub struct ClassProperty {
    #[serde(default)]
    pub name: String,
    /// `string`, `text`, `date`, `number`, `ref`, `claim` — or anything else, which is
    /// unchecked.
    #[serde(default)]
    pub r#type: String,
    /// What the property is for, as the class writes it.
    ///
    /// Required by `yidam schema`'s class shape, and prose: [`Class::text`] explains why the
    /// *scan* for evidence tags reads the bytes instead, and this field is the structural
    /// half — `graph` serialises it per property so an editor can show it. It is declared
    /// here rather than parsed a second time in `cmd/graph.rs`, which is #1116.
    #[serde(default)]
    pub description: String,
    /// Whether every instance of the class must carry this property (#301).
    ///
    /// **Absent means false.** Every corpus predating this field was written under a schema
    /// where the question could not be asked, so defaulting to `true` would gate every class
    /// in every derived repository on a declaration nobody made — a gate arriving in a
    /// corpus that never agreed to it, which is #257 from the other direction.
    #[serde(default)]
    pub required: bool,
    /// Whether this property's value is prose (#746).
    ///
    /// **A fifth of what a corpus writes is here and nothing that reads prose could see it.**
    /// Measured over sixteen corpora and 2,763 nodes: 1,895 of them — 68.6% — carry a block
    /// scalar nested inside another key, 14.5% of all node bytes, and 83% of that sits under
    /// `properties`. `properties.method` alone is a block scalar on 341 nodes across 214 KB.
    /// [`crate::claims`] scans the file's bytes and has always counted claims in there; every
    /// check that reads *prose* read the top level only, so `node-too-long` measured a fifth
    /// less than the node, `missing-description` answered *no prose* about a node whose whole
    /// substance is a `properties.verbatim` transcription, and `embed` put none of it in an
    /// embedding.
    ///
    /// **Absent means false**, for [`Self::required`]'s reason exactly. It is also why this is
    /// a flag beside `type` rather than a type of its own: prose-ness is orthogonal to what a
    /// value *is* — `method` and `identifier` are both strings — and a `type: prose` would
    /// change what `compile_class_schema` emits, which is a parity function in three SDKs.
    #[serde(default)]
    pub prose: bool,
    /// Whether this property's value belongs in the node's embedding though it is not prose.
    ///
    /// `embed` is the only reader — see [`crate::retrievable`], which held the only parse of
    /// this field until #1116 and now reads it from here.
    ///
    /// **Absent means false**, for [`Self::required`]'s reason.
    #[serde(default)]
    pub retrievable: bool,
}
/// One relationship a class declares.
#[derive(Default, serde::Deserialize)]
pub struct ClassEdge {
    #[serde(default)]
    pub relationship: String,
    /// The class at the *other* end, whichever end authors the link.
    #[serde(default)]
    pub target: String,
    /// `out` when instances of this class author the link, `in` when the other side does.
    #[serde(default)]
    pub direction: Option<String>,
    /// What the relationship means, as the class writes it. `graph` serialises it; nothing
    /// else reads it. Here rather than in `cmd/graph.rs` for [`ClassProperty::description`]'s
    /// reason.
    #[serde(default)]
    pub description: String,
}
/// One class's edge declarations, which is all the source-class derivation reads.
///
/// A view rather than a `&[Class]` because the derivation has two callers holding different
/// things: the checks hold parsed [`Class`]es from disk, and [`crate::cmd::lint::history`] holds class
/// blobs replayed out of git. One derivation over a shape both can produce is what stops
/// them disagreeing about which classes are exempt — which they would, silently, and the
/// replay's own doc comment already promised they would not.
pub struct EdgeView<'a> {
    pub name: &'a str,
    pub edges: &'a [ClassEdge],
}
/// Classes the ontology says nothing points at.
///
/// Instances of such a class have no inbound edges *by design*, so reporting them as orphans
/// is reporting the ontology working. In a derived repository this was 17 of 35 `orphan-in`
/// findings — every `person` and every `boundary-case` — and the noise is why the check's own
/// rationale had already conceded it was "worth seeing, not worth blocking on". The corpus
/// was not the thing that needed to change.
///
/// # Both ends of the edge, which is the correction
///
/// This used to read one class at a time: a source class was one declaring edges, none of
/// them `direction: in`. That reads half the ontology. `B: {relationship: r, target: A,
/// direction: out}` is a declaration that instances of `B` point at instances of `A` — the
/// same fact as `A: {..., direction: in}`, stated from the authoring end, and
/// [`ClassEdge::target`] is documented as "the class at the *other* end, **whichever end
/// authors the link**".
///
/// Reading only a class's own list therefore treated its silence about inbound edges as a
/// positive declaration that nothing points at it, while the ontology said elsewhere that
/// something does. That is the inverse of the over-read `GRAPH.md` warns about, and it was
/// measured: in `examples/streamflow` all three classes derived as source classes, so
/// `orphan-in` could not fire anywhere in the corpus yidam ships to teach people — while
/// `gage` declared `sources-from → concept, direction: out` the whole time.
///
/// # What it does not change
///
/// **A class that declares no edges at all is still not a source class.** It has said
/// nothing about its shape, and reading silence as a declaration would exempt every instance
/// in a corpus whose ontology has not been filled in — silencing the check exactly where
/// there is least reason to trust the graph.
///
/// **A declaration with no `direction` exempts neither end.** It says a relationship exists
/// and not which way it runs, and exempting on it would be reading an ambiguous declaration
/// in the one direction that silences findings. No measured corpus has one — A, B and C
/// declare a direction on all 254 edges between them — so this costs nothing today and is
/// the safe reading when it stops being free.
///
/// **A self-edge does not make a class pointed at.** `reach -downstream-of-> reach` says
/// instances relate to each other; it cannot say every instance is cited, because any
/// acyclic self-relation has an endpoint that is not. Reading it either way is wrong in one
/// direction, so it is read neither way — which is also exactly what the one-sided
/// derivation did, making this correction change only the cross-class case it is about.
/// Measured: with self-edges counted, the terminal reach of `examples/streamflow`'s
/// `downstream-of` chain becomes a finding, and every river has one.
pub fn source_classes(view: &[EdgeView<'_>]) -> HashSet<String> {
    let pointed = pointed_classes(view);
    view.iter()
        .filter(|v| !v.edges.is_empty() && !pointed.contains(v.name))
        .map(|v| v.name.to_string())
        .collect()
}

/// Every class the ontology says something points at, from whichever end said it.
///
/// The reading of `direction:` lives here and nowhere else. It is the half of
/// [`source_classes`] that is a fact about the *ontology* rather than about which classes are
/// exempt from a check, and it has a second caller — `history::expectations_of`,
/// which asks the same question of a class that declares no edges of its own.
///
/// **It is one function because it was two.** The replay used to answer "is anything pointed
/// at this class" with `edges.iter().any(|e| e.target == name)`, which is direction-blind: a
/// class `C` named as the `target` of `D`'s `direction: in` declaration read as targeted,
/// when what that declaration says is that `C` points at `D`. `C` then scored against
/// `uncited == 0` — the exact state `meets_expectation: null` exists to represent, on a class
/// that had declared nothing at all (#659). The two readings could drift because the rule was
/// written twice; it is now written once.
///
/// The three arms are argued for on [`source_classes`], and they are the same arms: `in` on a
/// class points at that class, `out` points at its target, and a declaration with no
/// direction says a relationship exists without saying which way it runs, so it names both
/// ends. A self-edge names neither.
pub fn pointed_classes<'a>(view: &[EdgeView<'a>]) -> HashSet<&'a str> {
    let mut pointed: HashSet<&str> = HashSet::new();
    for v in view {
        for e in v.edges.iter().filter(|e| e.target != v.name) {
            match e.direction.as_deref() {
                Some("in") => {
                    pointed.insert(v.name);
                }
                Some("out") => {
                    pointed.insert(e.target.as_str());
                }
                _ => {
                    pointed.insert(v.name);
                    pointed.insert(e.target.as_str());
                }
            }
        }
    }
    pointed
}

/// The [`EdgeView`]s of a parsed ontology.
pub fn edge_views(classes: &[Class]) -> Vec<EdgeView<'_>> {
    classes
        .iter()
        .map(|c| EdgeView {
            name: c.name.as_str(),
            edges: &c.edges,
        })
        .collect()
}
#[derive(Default, serde::Deserialize)]
struct ClassFields {
    #[serde(default)]
    label: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    properties: Vec<ClassProperty>,
    #[serde(default)]
    edges: Vec<ClassEdge>,
    #[serde(default)]
    edge_policy: Option<String>,
    #[serde(default)]
    max_lines: Option<usize>,
    #[serde(default)]
    prose: Vec<String>,
    #[serde(default)]
    implemented_by: Option<String>,
    #[serde(default)]
    foundational_type: Option<FoundationalType>,
    /// The three spellings that were never read by anything. Deserialized only so
    /// [`crate::cmd::lint::checks::foundational_field_misspelled`] can see them — `bootstrap.md` told authors to write
    /// `bfo_type:` for its whole life, and `additionalProperties: true` on the class body
    /// meant a corpus that believed it never heard otherwise. See #613.
    #[serde(default)]
    bfo_type: Option<serde_yaml::Value>,
    #[serde(default)]
    ufo_type: Option<serde_yaml::Value>,
    #[serde(default)]
    bfo_anchor: Option<String>,
}
/// A class's foundational alignment: which upper ontology, which type in it, and optionally
/// the IRI that type has there.
///
/// `iri` is optional because the alignment is worth stating even when nobody has looked the
/// IRI up, and because deriving one from `type` would mean shipping a BFO and gUFO term table
/// in the binary — a second copy of somebody else's vocabulary, wrong the moment they revise
/// it. When it is present `export-rdf` emits `skos:exactMatch`; when it is absent the
/// ontology and type still reach RDF as literals, which is what `bfo_anchor:` never did for
/// a UFO-aligned corpus.
#[derive(Default, Clone, serde::Deserialize)]
pub struct FoundationalType {
    #[serde(default)]
    pub ontology: String,
    #[serde(default, rename = "type")]
    pub ty: String,
    #[serde(default)]
    pub iri: Option<String>,
}
impl Class {
    /// Build one from a class file's bytes and the path it came from.
    ///
    /// Shared with `query::at`, which reconstructs classes from git blobs rather than from a
    /// walk. Two builders would be two answers to what a class *is* — and the one used less
    /// often is the one that would quietly stop reading `edge_policy`, which is the field the
    /// whole typecheck ladder turns on.
    ///
    /// It takes the *text* and deserializes here rather than taking a parsed [`ClassFields`],
    /// so that [`Self::text`] and the fields cannot come from different strings. A caller
    /// holding both could pass a mismatched pair, and nothing would say so.
    /// The class a `<class>.ont.yml` path names: its file stem.
    ///
    /// Split out of [`Self::parse`] because `lint::history` needs the answer for a class blob
    /// it is *removing* from the ontology, where there is no text to parse and so no `Class`
    /// to read [`Self::name`] off. That call site derived the stem itself until #1116 — a
    /// second expression for the question this whole type exists to answer once.
    pub(crate) fn name_of(rel: &str) -> String {
        Path::new(rel)
            .file_name()
            .map(|f| f.to_string_lossy().replace(".ont.yml", ""))
            .unwrap_or_default()
    }

    pub(crate) fn parse(rel: impl Into<String>, text: impl Into<String>) -> Self {
        let rel = rel.into();
        let text = text.into();
        let (fields, malformed): (ClassFields, _) = super::parse_or_default(&text);
        Self {
            name: Self::name_of(&rel),
            rel,
            text,
            label: fields.label.unwrap_or_default(),
            description: fields.description.unwrap_or_default(),
            properties: fields.properties,
            edges: fields.edges,
            edge_policy: EdgePolicy::parse(fields.edge_policy.as_deref()),
            max_lines: fields.max_lines,
            prose: fields
                .prose
                .into_iter()
                .map(|k| k.trim().to_string())
                .filter(|k| !k.is_empty())
                .collect(),
            // Trimmed, and an empty declaration read as none: `implemented_by: ""` is a
            // field somebody started and did not finish, and gating a build on it would
            // report a class against a type name that cannot match anything.
            implemented_by: fields
                .implemented_by
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty()),
            foundational_type: fields.foundational_type,
            dead_alignment_fields: [
                fields.bfo_type.is_some().then_some("bfo_type"),
                fields.ufo_type.is_some().then_some("ufo_type"),
                fields.bfo_anchor.is_some().then_some("bfo_anchor"),
            ]
            .into_iter()
            .flatten()
            .collect(),
            malformed,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **The stem governs, and it governs against a declared `class:`** rather than only in
    /// its absence.
    ///
    /// Four readers preferred the declared field until #1116 while `class_of`,
    /// `Corpus::defined_classes` and `graph-check` have always read the stem. No corpus on
    /// hand can tell the two apart — the measurement is on [`Class::name`] — so this is the
    /// only place the disagreement is constructed at all, and it is constructed so the rule
    /// has a falsifier rather than only a sentence.
    #[test]
    fn the_file_stem_names_the_class_when_the_declared_field_disagrees() {
        let c = Class::parse(
            ".yidam/corpus/gage.ont.yml",
            "class: measure\nlabel: Gage\n",
        );
        assert_eq!(c.name, "gage");
        // Only the name is the stem's. Everything else is read as the file wrote it.
        assert_eq!(c.label, "Gage");
    }

    /// The three fields #1116 widened this with, each of which a reader used to reach by
    /// opening the file a second time.
    #[test]
    fn a_class_carries_its_label_and_its_declared_descriptions() {
        let c = Class::parse(
            "gage.ont.yml",
            "class: gage\nlabel: Gage\ndescription: A station.\n\
             properties:\n  - name: datum\n    type: string\n    description: Which datum.\n\
             edges:\n  - relationship: reads\n    target: sensor\n    \
             description: The gage reads a sensor.\n",
        );
        assert_eq!(c.label, "Gage");
        assert_eq!(c.properties[0].description, "Which datum.");
        assert_eq!(c.edges[0].description, "The gage reads a sensor.");
    }

    /// Absent is empty, not unparseable. Every class file written before `label:` and the two
    /// `description:`s were read here still has to load, because this parse is now the only
    /// one and a reader that refused them would take the whole corpus with it.
    #[test]
    fn a_class_declaring_none_of_them_still_parses() {
        let c = Class::parse(
            "gage.ont.yml",
            "class: gage\nproperties:\n  - name: datum\n    type: string\n\
             edges:\n  - relationship: reads\n    target: sensor\n",
        );
        assert!(c.malformed.is_none(), "{:?}", c.malformed);
        assert!(c.label.is_empty());
        assert!(c.properties[0].description.is_empty());
        assert!(c.edges[0].description.is_empty());
    }
}
