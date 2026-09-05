//! The invariants `yidam lint` enforces.
//!
//! Each function returns a [`Check`] whether or not it found anything — a check that
//! reports nothing when it passes cannot be distinguished from a check that did not run,
//! and the difference matters when someone is deciding whether the gate covers a case.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use crate::parse::{parse_frontmatter, CorpusInstance, CATALOG_LOCATION_KINDS};

use super::model::{Check, Severity, Violation};

/// A corpus instance parsed once, with the paths needed to talk about it.
pub struct Node {
    pub path: PathBuf,
    pub rel: String,
    pub inst: CorpusInstance,
    /// The file's bytes, as they were read.
    ///
    /// Kept rather than dropped after parsing, because three callers were reading the same
    /// file again from disk to get it back: `query`'s `--select body`, the keyword arm of a
    /// similarity anchor, and — the one that makes this a correctness fix rather than a
    /// tidy-up — a query at a past commit, where `path` names a file whose *current*
    /// contents are the wrong answer and which may not exist at all. `load_nodes` already
    /// had this string in hand and threw it away.
    pub text: String,
}

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
    /// than an error, for the reason [`property_type_violation`] leaves an unknown type
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
    /// The `description` field: the text that says what kind of thing an instance is.
    /// Deliberately the only field [`class_asserts_purpose`] reads; see there.
    pub description: String,
    /// The class name — `person` for `person.ont.yml`.
    pub name: String,
    /// The typed fields the class declares, in declaration order.
    pub properties: Vec<ClassProperty>,
    /// The relationships the class licenses, from whichever end authors them.
    pub edges: Vec<ClassEdge>,
    /// Whether [`Self::edges`] is a bound or a description. See [`EdgePolicy`].
    pub edge_policy: EdgePolicy,
    /// The longest an instance of this class may be, in lines. `None` when the class has
    /// not said, which is every class written before the field existed — and no check runs.
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
    /// exactly the permanently non-empty report [`super::super::check_diff`] was
    /// diff-scoped to avoid.
    ///
    /// So the class says it, or nothing is said — the shape [`Self::max_lines`] and
    /// [`ClassProperty::required`] already have. A class that writes this has made a
    /// statement about the tree, and a tree that contradicts it is the ontology being
    /// contradicted rather than an omission, which is why [`unimplemented_class`] gates
    /// where `missing-property` does not.
    pub implemented_by: Option<String>,
}

/// One typed field a class declares.
#[derive(Default, serde::Deserialize)]
pub struct ClassProperty {
    #[serde(default)]
    pub name: String,
    /// `string`, `text`, `date`, `ref`, `claim` — or anything else, which is unchecked.
    #[serde(default)]
    pub r#type: String,
    /// Whether every instance of the class must carry this property (#301).
    ///
    /// **Absent means false.** Every corpus predating this field was written under a schema
    /// where the question could not be asked, so defaulting to `true` would gate every class
    /// in every derived repository on a declaration nobody made — a gate arriving in a
    /// corpus that never agreed to it, which is #257 from the other direction.
    #[serde(default)]
    pub required: bool,
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
}

/// One class's edge declarations, which is all the source-class derivation reads.
///
/// A view rather than a `&[Class]` because the derivation has two callers holding different
/// things: the checks hold parsed [`Class`]es from disk, and [`super::history`] holds class
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
    // Every class the ontology says something points at, from whichever end said it.
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
    view.iter()
        .filter(|v| !v.edges.is_empty() && !pointed.contains(v.name))
        .map(|v| v.name.to_string())
        .collect()
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
pub(crate) struct ClassFields {
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
    implemented_by: Option<String>,
}

impl Class {
    /// Build one from a parsed class file and the path it came from.
    ///
    /// Shared with `query::at`, which reconstructs classes from git blobs rather than from a
    /// walk. Two builders would be two answers to what a class *is* — and the one used less
    /// often is the one that would quietly stop reading `edge_policy`, which is the field the
    /// whole typecheck ladder turns on.
    pub(crate) fn from_fields(rel: impl Into<String>, fields: ClassFields) -> Class {
        let rel = rel.into();
        Class {
            name: Path::new(&rel)
                .file_name()
                .map(|f| f.to_string_lossy().replace(".ont.yml", ""))
                .unwrap_or_default(),
            rel,
            description: fields.description.unwrap_or_default(),
            properties: fields.properties,
            edges: fields.edges,
            edge_policy: EdgePolicy::parse(fields.edge_policy.as_deref()),
            max_lines: fields.max_lines,
            // Trimmed, and an empty declaration read as none: `implemented_by: ""` is a
            // field somebody started and did not finish, and gating a build on it would
            // report a class against a type name that cannot match anything.
            implemented_by: fields
                .implemented_by
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty()),
        }
    }
}

pub fn load_classes(root: &Path, paths: &[PathBuf], overlay: &super::Overlay) -> Vec<Class> {
    paths
        .iter()
        .map(|p| {
            let text = overlay.read(p);
            Class::from_fields(
                rel_of(root, p),
                serde_yaml::from_str(&text).unwrap_or_default(),
            )
        })
        .collect()
}

/// A catalog entry parsed once.
pub struct Source {
    pub rel: String,
    /// Absolute path on disk. A citation is a link that resolves to *this file*, which is
    /// what replaced the slug this struct used to carry — the slug existed only to be
    /// searched for in a node's bytes, and nothing else ever needed it.
    pub path: PathBuf,
    pub obtained: bool,
    pub used_by: Vec<String>,
    pub locations: Vec<crate::parse::CatalogLocation>,
    /// When the entry says it was last fetched, verbatim. See [`super::ttl`].
    pub retrieved: Option<String>,
    /// The entry's own TTL, which beats the corpus default.
    pub ttl_days: Option<u32>,
    /// What this entry says it has obtained, by content address.
    ///
    /// Empty on every entry written before RFC-0023, and the checks that read it are written
    /// so that an empty list is silent. A corpus adopting the field opts into the checks; a
    /// corpus that has not adopted it sees no new findings at all.
    pub artifacts: Vec<crate::parse::CatalogArtifact>,
}

fn rel_of(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .to_string()
}

pub fn load_nodes(root: &Path, paths: &[PathBuf], overlay: &super::Overlay) -> Vec<Node> {
    paths
        .iter()
        .map(|p| {
            let text = overlay.read(p);
            Node {
                path: p.clone(),
                rel: rel_of(root, p),
                inst: serde_yaml::from_str(&text).unwrap_or_default(),
                text,
            }
        })
        .collect()
}

pub fn load_sources(root: &Path, paths: &[PathBuf], overlay: &super::Overlay) -> Vec<Source> {
    paths
        .iter()
        .map(|p| {
            let text = overlay.read(p);
            let fm = parse_frontmatter(&text);
            Source {
                rel: rel_of(root, p),
                path: p.clone(),
                // Absent means obtained. Only an explicit `false` claims otherwise.
                obtained: fm.obtained.unwrap_or(true),
                used_by: fm.used_by.unwrap_or_default(),
                locations: fm.location.unwrap_or_default(),
                retrieved: fm.retrieved,
                ttl_days: fm.ttl_days,
                artifacts: fm.artifacts.unwrap_or_default(),
            }
        })
        .collect()
}

// ── class definitions ─────────────────────────────────────────────────────────

/// Phrasings that assert a reason rather than describe a kind.
///
/// Each is a purpose construction: it says the thing was done *in order to* bring something
/// about, which is a claim about somebody's intent. Kept small on purpose — a longer list
/// buys recall at the cost of the check being switched off.
const PURPOSE_PHRASES: &[&str] = &[
    "designed to",
    "deployed to",
    "intended to",
    "calculated to",
    "engineered to",
    "contrived to",
    "orchestrated to",
    "in order to",
    "so as to",
    "for the purpose of",
    "with the aim of",
    "with the intent",
];

/// Class definitions whose `description` asserts a purpose.
///
/// **Why this check exists where no instance-level check could.** Every other rule in a
/// yidam corpus runs on claim tags, and a tag attaches to a claim somebody makes on a node.
/// A class definition is not that: it is the meaning every instance takes on by being filed
/// under the class — asserted identically, silently, untagged, and for each. So a class
/// defined as "a procedural mechanism *deployed to obtain* an outcome the ordinary path
/// would not yield" makes every one of its instances assert a purpose, and no amount of
/// tagging on the instances can reach it. That definition is real: it survived five
/// resolutions and three separate arguments about its instances in a derived repository,
/// because every safeguard that repository had was pointed at instances.
///
/// **Only `description` is read.** That field says what kind of thing an instance is, and
/// is what an instance silently asserts. A class may perfectly well *discuss* purpose in
/// `analytic_note` — the corrected version of the class above does exactly that, quoting
/// "designed to produce this outcome" in order to forbid it — and reading commentary would
/// flag the discussion of the rule as a violation of it.
///
/// **Warn, and a prompt rather than a proof.** This is a check over wording. It cannot see
/// a purpose asserted in words that are not on the list, and a description can name a
/// purpose someone else attributed without asserting it. Read the finding; do not clear it.
///
/// **The finding quotes the sentence, not just the phrase.** The commonest false positive
/// is a description that carries the phrase inside a clause *disclaiming* purpose — "does
/// not record that a project was pursued in order to obtain anything" — which is the same
/// shape `analytic_note` is exempted for, one field over. Deciding automatically which
/// negations defuse a purpose claim is a judgement this check should not make, so it makes
/// the judgement cheap for the reader instead: the clause is printed with the finding, and
/// a case like that resolves at a glance without opening the file.
/// A decision this repository decided for itself.
///
/// **Not a defect, and reported anyway.** RFC-0024 settled that a repository's policy is
/// authoritative — a local rule decides, including by permitting what the default refused.
/// That is a decision the repository is entitled to make, so this never gates and is `Info`:
/// reporting it as probably-wrong would train people to ignore the check, which is the failure
/// mode `Severity` already argues about one file over.
///
/// What it exists to prevent is the *silent* version. `.yidam/private-paths` was built because
/// a repository's privacy was an assumption — true, load-bearing, and unenforced — and the
/// guideline states the rule this inherits:
///
/// > An assumption about access control that looks enforced and is not is worse than one
/// > everybody knows is manual, because nobody checks the second kind by hand.
///
/// The remedy there was not prohibition; it was making the declaration explicit. So an
/// override appears in `lint --format json`, in `serve --lsp`, and therefore in the editor —
/// where somebody reviewing a diff will actually meet it.
///
/// **It reports that the rule is local, and does not claim to know which way it moved.** That
/// is a question about every possible input; `yidam policy test` answers the part of it that
/// can be answered, by running the inherited cases against the local rule.
pub fn policy_override(overrides: &[(String, String)]) -> Check {
    let violations = overrides
        .iter()
        .map(|(decision, source)| {
            Violation::new(
                source,
                format!(
                    "`{decision}` is decided by this repository's own rule rather than the \
                     one `yidam` ships. Run `yidam policy test` to see which inherited \
                     expectations it no longer meets"
                ),
            )
            .at(Severity::Info)
        })
        .collect();
    Check::new(
        "policy-override",
        "A disclosure decision is this repository's own rule",
        Severity::Info,
        "A policy in `.yidam/policy/` supersedes the default `yidam` carries, and may permit \
         what the default refused — that is the authoritative model RFC-0024 settled on, and a \
         repository is entitled to use it. This check does not object to that. It exists so \
         that the choice cannot be made quietly: a guard loosened in a file nobody reads is the \
         shape `.yidam/private-paths` was built to end, where access control looked enforced \
         and was not. Record why in `.yidam/decisions/`, and use `yidam policy test` to see \
         exactly which inherited expectations your rule changed.",
        violations,
    )
}

pub fn class_asserts_purpose(classes: &[Class]) -> Check {
    let violations = classes
        .iter()
        .filter_map(|c| {
            let (hit, sentence) = first_purpose_phrase(&c.description)?;
            Some(Violation::new(
                &c.rel,
                format!("`description` says \"{hit}\" — a purpose, not a kind: \"{sentence}\""),
            ))
        })
        .collect();
    Check::new(
        "class-asserts-purpose",
        "Class definition asserts a purpose rather than describing a kind",
        Severity::Warn,
        "A claim tag attaches to a claim somebody makes on a node. A class definition is \
         not that — it is the meaning every instance takes on by being filed under the \
         class, asserted identically and untagged for each. So a class whose description \
         states a reason ('deployed to obtain an outcome the ordinary path would not \
         yield') puts that assertion beyond the reach of every other check here, which all \
         run on tags. The case this was written from survived five resolutions and three \
         arguments about its instances. Rewrite the description to say what an instance IS \
         — the observable shape, the admission conditions — and move any characterization \
         of why to a field that attributes it, or to `analytic_note`, which this check does \
         not read.",
        violations,
    )
}

/// The longest a quoted sentence gets before the middle is elided.
///
/// Long enough for the negating clause that motivates the quoting to survive on either
/// side of a phrase sitting mid-sentence; short enough to stay one line of terminal.
const SENTENCE_BUDGET: usize = 160;

/// The first purpose phrase in `description`, with the sentence carrying it.
///
/// Matching runs per sentence rather than over the whole description, so the phrase and
/// the clause reported for it cannot come from different places, and no index has to be
/// mapped back from the lowercased copy — `to_lowercase` is not length-preserving.
fn first_purpose_phrase(description: &str) -> Option<(&'static str, String)> {
    sentences(description).into_iter().find_map(|s| {
        let lowered = s.to_lowercase();
        let hit = PURPOSE_PHRASES.iter().find(|p| lowered.contains(**p))?;
        Some((*hit, elide(&s, hit)))
    })
}

/// Splits prose into whitespace-collapsed sentences on terminal punctuation.
///
/// Collapsing first means a phrase broken across lines by a literal YAML block still
/// matches, which the previous whole-description `contains` did not manage. Splitting
/// after cannot lose a match either: no phrase on the list contains a `.`, `!`, or `?`,
/// so a break — including a spurious one after an abbreviation — always falls outside a
/// phrase and at worst costs the reader some context in the quote.
fn sentences(text: &str) -> Vec<String> {
    let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut out = Vec::new();
    let mut start = 0;
    let bytes = flat.as_bytes();
    for (i, ch) in flat.char_indices() {
        let ends = matches!(ch, '.' | '!' | '?')
            && bytes.get(i + 1).is_none_or(|b| b.is_ascii_whitespace());
        if ends {
            let end = i + 1;
            if !flat[start..end].trim().is_empty() {
                out.push(flat[start..end].trim().to_string());
            }
            start = end;
        }
    }
    if !flat[start..].trim().is_empty() {
        out.push(flat[start..].trim().to_string());
    }
    out
}

/// Trims a sentence to `SENTENCE_BUDGET` characters, keeping `hit` inside the window.
///
/// A long sentence is elided around the phrase rather than from the end, because the
/// words that decide whether the finding is real — a negation, an attribution — are
/// usually the ones next to it.
fn elide(sentence: &str, hit: &str) -> String {
    let chars: Vec<char> = sentence.chars().collect();
    if chars.len() <= SENTENCE_BUDGET {
        return sentence.to_string();
    }
    let at = sentence.to_lowercase().find(hit).unwrap_or(0);
    let at_char = sentence[..at].chars().count();
    let half = (SENTENCE_BUDGET - hit.chars().count().min(SENTENCE_BUDGET)) / 2;
    let hi = (at_char + hit.chars().count() + half).clamp(SENTENCE_BUDGET, chars.len());
    let lo = hi - SENTENCE_BUDGET;
    let mut out = String::new();
    if lo > 0 {
        out.push('…');
    }
    out.extend(&chars[lo..hi]);
    if hi < chars.len() {
        out.push('…');
    }
    out
}

// ── corpus structure ──────────────────────────────────────────────────────────

pub fn missing_class(nodes: &[Node]) -> Check {
    let violations = nodes
        .iter()
        .filter(|n| n.inst.class.is_none())
        .map(|n| Violation::new(&n.rel, "no `class:` field"))
        .collect();
    Check::new(
        "missing-class",
        "Instance declaring no class",
        Severity::Error,
        "An instance's class is what binds it to a schema. Without one there is nothing to \
         validate the node against and nothing to render it as — it is a YAML file in a \
         corpus directory, not a node.",
        violations,
    )
}

pub fn unknown_class(nodes: &[Node], defined: &HashSet<String>) -> Check {
    // With no .ont.yml files at all, every class is "unknown" and the check would report
    // the whole corpus. That is a repository without a schema layer, which is a different
    // problem than a mistyped class name.
    let violations = if defined.is_empty() {
        Vec::new()
    } else {
        nodes
            .iter()
            .filter_map(|n| {
                let class = n.inst.class.as_ref()?;
                (!defined.contains(class)).then(|| {
                    Violation::new(&n.rel, format!("class `{class}` has no {class}.ont.yml"))
                })
            })
            .collect()
    };
    Check::new(
        "unknown-class",
        "Instance of a class that has no schema",
        Severity::Error,
        "Either the class file was never written or the instance misspells it. Both leave \
         the node unvalidatable, and a misspelling additionally splits one class into two \
         that no query will join.",
        violations,
    )
}

pub fn missing_label(nodes: &[Node]) -> Check {
    let violations = nodes
        .iter()
        .filter(|n| n.inst.label.is_none())
        .map(|n| Violation::new(&n.rel, "no `label:` field"))
        .collect();
    Check::new(
        "missing-label",
        "Instance with no human-readable label",
        Severity::Warn,
        "The label is what every index, export, and graph rendering shows. Without it a \
         reader gets a filename, which is an identifier chosen for stability rather than \
         for saying what the thing is.",
        violations,
    )
}

pub fn missing_description(nodes: &[Node]) -> Check {
    let violations = nodes
        .iter()
        .filter(|n| n.inst.description.is_none())
        .map(|n| Violation::new(&n.rel, "no `description:` field"))
        .collect();
    Check::new(
        "missing-description",
        "Instance with no description",
        Severity::Warn,
        "The description is the node's content. An instance with properties and links but \
         nothing said about it records that something exists without recording what is \
         known about it.",
        violations,
    )
}

pub fn orphan_out(nodes: &[Node]) -> Check {
    let violations = nodes
        .iter()
        .filter(|n| n.inst.links.as_deref().unwrap_or(&[]).is_empty())
        .map(|n| Violation::new(&n.rel, "no outgoing links"))
        .collect();
    Check::new(
        "orphan-out",
        "Node connected to nothing",
        Severity::Error,
        "Every node must carry at least one outgoing edge. A node with none is unreachable \
         by traversal from anywhere, which in a graph whose value is its connectivity means \
         it is not in the graph at all.",
        violations,
    )
}

pub fn dangling_edge(nodes: &[Node]) -> Check {
    let mut violations = Vec::new();
    for n in nodes {
        let dir = n.path.parent().unwrap_or(&n.path);
        for link in n.inst.links.as_deref().unwrap_or(&[]) {
            match &link.target {
                None => violations.push(Violation::new(&n.rel, "link entry with no `target:`")),
                Some(target) => {
                    if !dir.join(target).exists() {
                        violations.push(Violation::new(
                            &n.rel,
                            format!("target does not exist: {target}"),
                        ));
                    }
                }
            }
        }
    }
    Check::new(
        "dangling-edge",
        "Edge pointing at nothing",
        Severity::Error,
        "A broken edge is worse than a missing one: it asserts a relationship to something \
         that is not there, so a reader following it learns nothing and a traversal counting \
         it overstates the graph's connectivity. Renaming a node severs every edge into it, \
         which is why node filenames are meant to be stable.",
        violations,
    )
}

/// The class an instance belongs to, from its path: `.yidam/corpus/person/x.yml` → `person`.
pub(crate) fn class_of(n: &Node) -> String {
    n.path
        .parent()
        .and_then(|d| d.file_name())
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default()
}

pub fn orphan_in(nodes: &[Node], classes: &[Class]) -> Check {
    let mut targeted: HashSet<PathBuf> = HashSet::new();
    for n in nodes {
        let dir = n.path.parent().unwrap_or(&n.path);
        for link in n.inst.links.as_deref().unwrap_or(&[]) {
            if let Some(t) = &link.target {
                // Normalize so `../class/x.yml` and `class/x.yml` compare equal.
                targeted.insert(normalize(&dir.join(t)));
            }
        }
    }
    // Classes the ontology says nothing points at. Their instances are exempt: an orphan
    // there is the model holding, not the corpus failing.
    let exempt = source_classes(&edge_views(classes));

    let violations = nodes
        .iter()
        .filter(|n| !targeted.contains(&normalize(&n.path)))
        .filter(|n| !exempt.contains(&class_of(n)))
        .map(|n| Violation::new(&n.rel, "nothing links to this node"))
        .collect();
    Check::new(
        "orphan-in",
        "Node nothing points to",
        Severity::Info,
        "Instances of a class the ontology declares no inbound edge for — from either end \
         of the edge — are exempt: nothing is meant to point at them, so an orphan there is \
         the model working. \
         What remains is a class whose *other* instances are cited and this one is not, \
         which is the asymmetry worth reading. Still reported rather than gated: a node \
         authored this morning legitimately has no inbound edges yet.",
        violations,
    )
}

/// Resolve `.` and `..` without touching the filesystem.
pub fn normalize(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            std::path::Component::ParentDir => {
                out.pop();
            }
            std::path::Component::CurDir => {}
            other => out.push(other),
        }
    }
    out
}

// ── the class contract ────────────────────────────────────────────────────────
//
// `.ont.yml` states, per class, which properties an instance carries and which
// relationships it may enter into. Until these checks existed nothing read any of it: an
// instance could carry a property the class never declared, omit one it does, record a
// `claim` field as prose, or point a declared relationship at a node of the wrong class,
// and every gate reported the corpus clean. `graph-check` and `dangling-edge` check that a
// link *resolves*; nothing checked that it was *licensed*.
//
// **Silence is not a contract.** Each half of the declaration is read independently: a
// class with no `properties:` has said nothing about properties, and a class with no
// `edges:` has said nothing about edges. Reading either silence as "and therefore none are
// permitted" would flood every corpus whose ontology is not filled in — which is exactly
// the corpus with least reason to trust the graph, and the same trap `source_classes`
// documents one field over.
//
// **Only edges between instances are licensed edges.** A link to `../<class>.ont.yml` or to
// a catalog entry is a citation, not a relationship — the bootstrap skill says so in as
// many words, and an instance is required to carry the `instance-of` link that none of
// these classes declares. So the licensing checks read only links resolving to another
// corpus instance, which also means a broken edge is `dangling-edge`'s finding and not
// reported twice here.

/// The class that governs an instance, keyed by the directory name that actually governs.
fn classes_by_name(classes: &[Class]) -> HashMap<&str, &Class> {
    classes.iter().map(|c| (c.name.as_str(), c)).collect()
}

/// Every instance in the corpus, by its normalized path — what a link target resolves to.
pub(crate) fn nodes_by_path(nodes: &[Node]) -> HashMap<PathBuf, &Node> {
    nodes.iter().map(|n| (normalize(&n.path), n)).collect()
}

/// `(link, target node)` for each of `n`'s links that lands on another instance.
///
/// Everything else — the `instance-of` link to the class file, a citation into the catalog,
/// an edge to a file that is not there — is not an ontology edge and is not licensed here.
pub(crate) fn instance_links<'a>(
    n: &'a Node,
    by_path: &HashMap<PathBuf, &'a Node>,
) -> Vec<(&'a crate::parse::CorpusLink, &'a Node)> {
    let dir = n.path.parent().unwrap_or(&n.path);
    n.inst
        .links
        .as_deref()
        .unwrap_or(&[])
        .iter()
        .filter_map(|l| {
            let target = l.target.as_deref()?;
            let to = by_path.get(&normalize(&dir.join(target)))?;
            Some((l, *to))
        })
        .collect()
}

/// The properties an instance actually wrote, in file order.
fn instance_properties(n: &Node) -> Vec<(String, &serde_yaml::Value)> {
    n.inst
        .properties
        .iter()
        .flatten()
        .filter_map(|(k, v)| Some((k.as_str()?.to_string(), v)))
        .collect()
}

/// Instance properties the class never declared.
///
/// The commonest cause is a field renamed on one instance and not on the class — which
/// reads as data to a human and is invisible to every consumer that walks the ontology,
/// because the ontology does not mention it. The value is not lost; it is simply not part
/// of the class, and so no schema, export, or query will ever see it.
pub fn undeclared_property(
    nodes: &[Node],
    classes: &[Class],
    universal: &crate::universal::Universal,
) -> Check {
    let by_name = classes_by_name(classes);
    let mut violations = Vec::new();
    for n in nodes {
        let Some(class) = by_name.get(class_of(n).as_str()) else {
            continue;
        };
        if class.properties.is_empty() {
            continue;
        }
        for (key, _) in instance_properties(n) {
            if class.properties.iter().any(|p| p.name == key) || universal.covers(&key) {
                continue;
            }
            violations.push(Violation::new(
                &n.rel,
                format!(
                    "`{key}` is not declared by `{}` — declare it on the class or remove it",
                    class.rel
                ),
            ));
        }
    }
    Check::new(
        "undeclared-property",
        "Instance property the class never declared",
        Severity::Error,
        "A property absent from the class is invisible to everything that reads the corpus \
         through the ontology — schemas, exports, queries. It reads as data to a human and \
         as nothing to a machine, which is the failure mode worth gating on. A class that \
         declares no `properties:` has said nothing and is not checked, and a property \
         `.yidam/corpus/universal.yml` declares — by name or by pattern — may be carried by \
         any class without being declared on each.",
        violations,
    )
}

/// Declared properties an instance does not carry.
///
/// **Warn, where its four siblings are errors, and the difference is not a hedge.** Each of
/// the others reports a statement the ontology actually made being contradicted: a property
/// it never declared, a value contradicting a declared type, a relationship it does not
/// license, a target of the wrong class. An omission contradicts nothing. The property
/// declaration has no `required` field, so it cannot distinguish *every instance has this*
/// from *an instance may have this*, and gating on the first reading asserts a contract the
/// ontology never wrote.
///
/// The case that settled it is in the parity fixture and is not a corner: `claim_tag` is
/// declared on `concept`, and two of that fixture's three concepts deliberately do not
/// carry one — a node that makes no tagged claim is a real state, and it is the state the
/// fixture exists to exercise. A gate on omission would have failed the corpus for being
/// what it was written to be.
///
/// So this reports, and reports usefully: the commonest cause is a class that grew a field
/// its instances never did, which is invisible to every reader and obvious in a list. When
/// the ontology gains a way to say *required* — the schema compiler is the natural home —
/// this check can gate on the properties that say it.
/// An instance longer than the class said its instances get.
///
/// **Opt-in, and the opt-in is the finding.** The bootstrap rubric's S7 caps a node at 40
/// lines, and that number lives only in the harness — which the vendor step deletes — so a
/// derived repository has been scored against a norm it could not read or test for. The
/// obvious repair is to port 40 here. Measured against five real corpora, 335 of 410 nodes
/// exceed it: 86%, 86% and 97% in the three mature ones. A check arriving in an existing
/// corpus and calling four fifths of it debt is the check that gets switched off, which is
/// the argument `.yidam/lint-baseline.yml` exists to make and this would be the worst way to
/// test it.
///
/// The same corpora at genesis run to a median of 35 and 40 is right for three of the four —
/// so 40 is a genesis norm a corpus grows out of, and growing out of it is what a corpus
/// doing its job looks like. There is no knee to put a steady-state number at either: the
/// distribution runs smoothly from 20 to 534.
///
/// So the number is the class's. A class that declares `max_lines:` has said what its
/// instances are — statutory obligations quoting the text they arise from are not the length
/// of a person — and an instance over it contradicts a contract the corpus wrote. A class
/// that says nothing is not reported against, for the reason [`EdgePolicy::Unstated`] gives
/// one field over: gating there would enforce a contract nobody wrote.
///
/// Warn rather than Error even when declared. Length is editorial, the ratchet in
/// [`super::baseline`] already distinguishes inherited from new, and a node one line over is
/// not a corpus that has stopped being true.
pub fn node_too_long(nodes: &[Node], classes: &[Class]) -> Check {
    let by_name = classes_by_name(classes);
    let mut violations = Vec::new();
    for n in nodes {
        let Some(class) = by_name.get(class_of(n).as_str()) else {
            continue;
        };
        let Some(max) = class.max_lines else {
            continue;
        };
        // The bytes as read, so this counts what a reader scrolls past — the same thing the
        // harness's S7 counts, and the reason `Node::text` is kept after parsing.
        let lines = n.text.lines().count();
        if lines <= max {
            continue;
        }
        violations.push(Violation::new(
            &n.rel,
            format!("{lines} lines; `{}` declares `max_lines: {max}`", class.rel),
        ));
    }
    Check::new(
        "node-too-long",
        "Instance longer than its class allows",
        Severity::Warn,
        "A long node is usually two nodes, or a node carrying quoted source that belongs in \
         the catalog entry it cites. The ceiling is the class's own — a class that declares \
         no `max_lines:` is not checked, because the length an instance should be is a \
         question about that class and not about corpora in general. Measured before \
         choosing: a fixed ceiling of 40, which the bootstrap rubric uses at genesis, is \
         exceeded by 335 of 410 nodes across five real corpora once they have grown.",
        violations,
    )
}

pub fn missing_property(nodes: &[Node], classes: &[Class]) -> Check {
    let by_name = classes_by_name(classes);
    let mut violations = Vec::new();
    for n in nodes {
        let Some(class) = by_name.get(class_of(n).as_str()) else {
            continue;
        };
        if class.properties.is_empty() {
            continue;
        }
        let carried = instance_properties(n);
        for declared in &class.properties {
            if carried.iter().any(|(k, _)| *k == declared.name) {
                continue;
            }
            // The severity is a function of the DECLARATION, not a blanket judgement about
            // omissions — the shape `orphan-in` already has, where residence time rather
            // than the check's level decides. A class that said `required: true` has
            // written the contract this instance contradicts, and contradiction is what the
            // other four checks gate on. A class that said nothing has not, and gating
            // there would assert a contract the ontology never wrote.
            let violation = Violation::new(
                &n.rel,
                format!(
                    "`{}` is declared by `{}`{} and this instance does not carry it",
                    declared.name,
                    class.rel,
                    match declared.required {
                        true => " as `required: true`",
                        false => "",
                    }
                ),
            );
            violations.push(match declared.required {
                true => violation.at(Severity::Error),
                false => violation,
            });
        }
    }
    Check::new(
        "missing-property",
        "Declared property the instance omits",
        Severity::Warn,
        "The commonest cause is a class that grew a field its instances never did — \
         invisible to every reader and obvious in a list. A property declared \
         `required: true` GATES: the class wrote that contract, and an instance omitting it \
         contradicts the ontology, which is what this check's four siblings gate on. \
         Everything else is reported, because a class that said nothing about a property \
         cannot be read as demanding it — a node that makes no tagged claim is a real \
         state, not a defect, and gating on omission would assert a contract nobody wrote.",
        violations,
    )
}

/// One `struct` or `enum` the tree defines, and where.
///
/// Built by [`super::run_checks_with`] from a walk of `crates/`, and taken as an argument
/// rather than read here so the check stays pure and its logic is testable without a
/// repository — the same reason every other function in this module takes its facts in.
pub struct TypeIndex(HashMap<String, String>);

impl TypeIndex {
    /// Index `(repo-relative path, file text)` pairs by the type names each defines.
    ///
    /// **First declaration wins, and nothing here deduplicates across crates.** Five crates
    /// defining `Error` is five files and one name; the class asked whether the name exists,
    /// and one location is a better answer than a list nobody reads.
    pub fn build<'a>(files: impl IntoIterator<Item = (&'a str, &'a str)>) -> Self {
        let mut by_name = HashMap::new();
        for (rel, text) in files {
            for (name, line) in crate::cmd::check_diff::extract::declared_in(text) {
                by_name
                    .entry(name)
                    .or_insert_with(|| format!("{rel}:{line}"));
            }
        }
        Self(by_name)
    }

    fn locate(&self, name: &str) -> Option<&str> {
        self.0.get(name).map(String::as_str)
    }
}

/// A class naming an implementation the tree does not define.
///
/// **Error, where its omission-shaped sibling is a warning, and the difference is the same
/// one `missing-property` draws.** A class that declares `implemented_by: Intervention` has
/// stated a fact about the tree. A tree with no such type contradicts it, and contradiction
/// is what `undeclared-property`, `property-type` and an `exhaustive` `unlicensed-edge`
/// gate on. The reading that made this an omission — *the ontology declares a class and
/// nothing implements it* — is the one the measurement killed; see [`Class::implemented_by`]
/// for the 129-of-157 number and why it could never have shipped unconditionally.
///
/// **A rename is invisible here, which is the whole reason the finding moved.** #33 asked
/// this of a diff, where a rename arrives as a removal plus an addition and correlating the
/// two is the hard part. This scans a tree: a class implemented under a new name is found
/// under that name and nothing is reported. What remains reportable is the case worth
/// gating on — the type is *gone*, or the class was never right about it in the first place.
///
/// Matching is exact, on the name as the class wrote it. No kebab round trip, because the
/// class names the Rust type directly and a round trip would only reintroduce the guess the
/// field exists to remove: `HTTPServer` and `HttpServer` kebab to the same thing and are
/// two different types.
pub fn unimplemented_class(classes: &[Class], types: &TypeIndex) -> Check {
    let mut violations = Vec::new();
    for class in classes {
        let Some(declared) = class.implemented_by.as_deref() else {
            continue;
        };
        if types.locate(declared).is_some() {
            continue;
        }
        violations.push(Violation::new(
            &class.rel,
            format!(
                "`implemented_by: {declared}` names no `struct` or `enum` under `crates/` — \
                 restore it, point the class at the type that replaced it, or drop the field"
            ),
        ));
    }
    Check::new(
        "unimplemented-class",
        "Class naming an implementation the code does not define",
        Severity::Error,
        "A class declaring `implemented_by:` has stated a fact about `crates/`, and a tree \
         with no type of that name contradicts it — which is what this check's siblings \
         gate on, and an omission is not. A class that declares nothing is not checked, and \
         that silence is measured rather than timid: across twelve derived corpora 129 of \
         157 declared classes have no type bearing their name, and matching every language \
         in the tree raises it to 88%. Their ontologies model a domain and their code models \
         the pipeline that gathers evidence about it, so a class without a type is the \
         normal case and reporting it would call four fifths of every ontology debt. A \
         rename is invisible: the tree is scanned rather than a diff, so a class implemented \
         under a new name is simply found.",
        violations,
    )
}

/// The declared types this check knows how to test.
///
/// `pub(crate)` for `migrate`, which tests an instance's existing value against a *proposed*
/// type before performing a retype. That has to be this predicate rather than a second
/// reading of it: a migration disagreeing with the gate about what a valid value is would
/// be a migration into a failing build.
///
/// Anything else is left alone rather than reported: a corpus is free to coin a type, and a
/// check that failed on every type it had not heard of would make coining one impossible.
pub(crate) fn property_type_violation(declared: &str, value: &serde_yaml::Value) -> Option<String> {
    let scalar = |v: &serde_yaml::Value| match v {
        serde_yaml::Value::String(s) => Ok(s.clone()),
        serde_yaml::Value::Null => Err("is empty".to_string()),
        serde_yaml::Value::Sequence(_) => Err("is a list".to_string()),
        serde_yaml::Value::Mapping(_) => Err("is a mapping".to_string()),
        // The classic YAML surprise: `parameter: 00060` is the number 60, and the code the
        // corpus meant is gone by the time anything reads it.
        other => Err(format!(
            "is {}, not text — quote it",
            match other {
                serde_yaml::Value::Bool(_) => "a boolean",
                serde_yaml::Value::Number(_) => "a number",
                _ => "not text",
            }
        )),
    };
    let not_a_tag = |s: &str| {
        format!(
            "`{}` is not an evidence tag — write `verified`, `inference`, or `open`",
            s.trim()
        )
    };
    match declared {
        // A sequence is legal here and nowhere else: `count_structural` reads a list of
        // tags as one claim each, and a corpus writing `claim_tag: [open]` unquoted has
        // written a one-element list without meaning to. Rejecting it would put this check
        // in disagreement with the counter about the same bytes.
        crate::claims::CLAIM_PROPERTY_TYPE => match value {
            serde_yaml::Value::Sequence(items) => items.iter().find_map(|i| match i.as_str() {
                Some(s) if crate::claims::tag_of(s).is_some() => None,
                Some(s) => Some(not_a_tag(s)),
                None => Some("holds a value that is not text".to_string()),
            }),
            other => match scalar(other) {
                Err(why) => Some(why),
                Ok(s) if crate::claims::tag_of(&s).is_some() => None,
                Ok(s) => Some(not_a_tag(&s)),
            },
        },
        "date" => match scalar(value) {
            Err(why) => Some(why),
            Ok(s) if is_iso_date(s.trim()) => None,
            Ok(s) => Some(format!(
                "`{}` is not a date — write `YYYY-MM-DD`, or `YYYY-MM` or `YYYY` if that is \
                 the precision it is known to",
                s.trim()
            )),
        },
        "string" | "text" | "ref" => match scalar(value) {
            Err(why) => Some(why),
            Ok(s) if s.trim().is_empty() => Some("is empty".to_string()),
            Ok(_) => None,
        },
        _ => None,
    }
}

/// An ISO-8601 calendar date, structurally, **at whatever precision the corpus knows**:
/// `YYYY`, `YYYY-MM`, or `YYYY-MM-DD`.
///
/// Not a calendar — `2024-02-31` passes and is somebody else's finding. What this catches is
/// a date field carrying prose, and reduced precision is not prose. Requiring a full day
/// rejected `formed: "1985"` 71 times in one derived corpus and `stated: "1991-06"` in
/// another: a band formed in 1985, and the year is the precision the fact is actually known
/// to. Demanding a month and a day there does not make the record more accurate, it makes it
/// invented — and the check's own rationale had already said prose was the target.
///
/// The 45 findings that remain in that corpus are the ones worth having: `[open] No date.`,
/// `1993. [inference], via [musicbrainz](../../catalog/musicbrainz.md) life-span.`
fn is_iso_date(s: &str) -> bool {
    let mut parts = s.split('-');
    // Year, then optionally month, then optionally day. A trailing separator leaves an
    // empty part, which fails the width test rather than passing as absent — `1985-` is
    // not a year.
    let widths = [4, 2, 2];
    let mut seen = 0;
    for w in widths {
        match parts.next() {
            None => break,
            Some(p) if p.len() == w && p.chars().all(|c| c.is_ascii_digit()) => seen += 1,
            Some(_) => return false,
        }
    }
    seen > 0 && parts.next().is_none()
}

/// Property values that do not satisfy the type the class declares.
///
/// `claim` is the type that pays for this check on its own. `claims.rs` matches the three
/// evidence tokens exactly, so a field declared `type: claim` and filled with prose is
/// counted as **no claim at all** — a node that reads as evidenced to a human and as bare
/// assertion to every counter. That is the same silent-undercount failure
/// `claim-tag-malformed` reports in prose, one field over, and the matcher is reused rather
/// than re-derived so the two cannot disagree about what a tag is.
pub fn property_type(
    nodes: &[Node],
    classes: &[Class],
    universal: &crate::universal::Universal,
) -> Check {
    let by_name = classes_by_name(classes);
    let mut violations = Vec::new();
    for n in nodes {
        let Some(class) = by_name.get(class_of(n).as_str()) else {
            continue;
        };
        for (key, value) in instance_properties(n) {
            // The class first, then the corpus. Universal does not mean untyped — a
            // fiscal-year snapshot is prose and a `claim` written into one is still
            // counted as no claim — but a class naming the same property has said
            // something more specific about its own instances, and wins.
            let declared = class
                .properties
                .iter()
                .find(|p| p.name == key)
                .map(|p| p.r#type.as_str())
                .or_else(|| universal.declared_type(&key));
            let Some(declared) = declared else {
                continue;
            };
            if let Some(why) = property_type_violation(declared, value) {
                violations.push(Violation::new(
                    &n.rel,
                    format!("`{key}` is declared `{declared}` and {why}"),
                ));
            }
        }
    }
    Check::new(
        "property-type",
        "Property value that contradicts its declared type",
        Severity::Error,
        "A typed field the ontology declares is read by type, not by eye. A `claim` field \
         holding prose is counted as no claim at all; a `string` holding an unquoted number \
         has lost its leading zeros before anything reads it. Types the corpus coins for \
         itself are left alone — this reports only the ones it can test.",
        violations,
    )
}

/// Edges between instances that the authoring class does not license.
///
/// A relationship appearing in no declaration is either a typo or a verb the corpus coined
/// deliberately, and **the ontology is the only thing that can tell those apart**. A class
/// declaring `edge_policy: exhaustive` has said its vocabulary is closed, so the finding is
/// an error. A class declaring `characteristic` has said the opposite, and there is no
/// finding. A class that has said neither gets a warning: the typo is worth seeing, and
/// gating on it would enforce a contract nobody wrote.
///
/// The default was chosen by measuring rather than by taste. Reading a non-empty `edges:`
/// as closed put 210 errors on a corpus whose 18 classes all declare `characteristic`,
/// documented it, and enforce it in their own schema — 107 distinct relationships, of which
/// `bears-on` alone carried 16. The two derived corpora that declare no policy trip this
/// check zero times whichever way the default falls, so the permissive reading costs
/// nothing that the strict one was buying.
pub fn unlicensed_edge(nodes: &[Node], classes: &[Class]) -> Check {
    let by_name = classes_by_name(classes);
    let by_path = nodes_by_path(nodes);
    let mut violations = Vec::new();
    for n in nodes {
        let Some(class) = by_name.get(class_of(n).as_str()) else {
            continue;
        };
        // Silence about the *edges* is still silence, exactly as before: a class with no
        // `edges:` has named no relationships, and a policy on an empty list bounds nothing.
        if class.edges.is_empty() || class.edge_policy == EdgePolicy::Characteristic {
            continue;
        }
        for (link, _) in instance_links(n, &by_path) {
            let rel = link.relationship.as_deref().unwrap_or_default();
            if class.edges.iter().any(|e| e.relationship == rel) {
                continue;
            }
            let v = Violation::new(
                &n.rel,
                format!(
                    "`{}` is not a relationship `{}` declares{}",
                    if rel.is_empty() { "(none)" } else { rel },
                    class.rel,
                    match class.edge_policy {
                        EdgePolicy::Exhaustive => ", whose vocabulary is `exhaustive`",
                        // Names the way out, because the commonest reason to see this is a
                        // corpus that coins verbs on purpose and has never been asked to
                        // say so.
                        _ => " — declare it, or set `edge_policy:` on the class",
                    }
                ),
            );
            violations.push(match class.edge_policy {
                EdgePolicy::Exhaustive => v.at(Severity::Error),
                _ => v,
            });
        }
    }
    Check::new(
        "unlicensed-edge",
        "Relationship the class does not declare",
        // Warn is the level for a class that has not declared a policy, which is every
        // class written before the field existed. `exhaustive` raises its own findings to
        // Error per violation, so one corpus can hold both kinds at once.
        Severity::Warn,
        "An edge is licensed by the class that authors it, and `edge_policy:` says whether \
         that list bounds the vocabulary or describes it. `exhaustive` closes it and makes \
         anything outside an error; `characteristic` says an undeclared relationship is a \
         deliberate coinage, and none are reported. A class that has said neither is warned \
         about but not gated — a relationship in no declaration is worth seeing, because a \
         traversal that walks by relationship will not find it, but a non-empty `edges:` on \
         its own never claimed to be complete. Only links landing on another instance are \
         read: a link to the class file or into the catalog is a citation, not a \
         relationship. A class that declares no `edges:` has said nothing and is not \
         checked.",
        violations,
    )
}

/// Licensed edges that land on a node of the wrong class.
///
/// This is the finding no existing check could produce. `dangling-edge` catches an edge to
/// nothing; nothing caught an edge to the wrong thing, and an edge to the wrong thing
/// resolves, traverses, and exports — it is simply false.
pub fn edge_target_class(nodes: &[Node], classes: &[Class]) -> Check {
    let by_name = classes_by_name(classes);
    let by_path = nodes_by_path(nodes);
    let mut violations = Vec::new();
    for n in nodes {
        let Some(class) = by_name.get(class_of(n).as_str()) else {
            continue;
        };
        for (link, to) in instance_links(n, &by_path) {
            let rel = link.relationship.as_deref().unwrap_or_default();
            // Several declarations may share a relationship name; any one of them licenses
            // the target. A declaration with no `target` has named no class and licenses
            // every one of them.
            let declared: Vec<&str> = class
                .edges
                .iter()
                .filter(|e| e.relationship == rel)
                .map(|e| e.target.as_str())
                .collect();
            if declared.is_empty() || declared.iter().any(|t| t.is_empty()) {
                continue;
            }
            let actual = class_of(to);
            if declared.contains(&actual.as_str()) {
                continue;
            }
            violations.push(Violation::new(
                &n.rel,
                format!(
                    "`{rel}` is declared to target {} and this edge lands on `{}`, a {actual}",
                    declared
                        .iter()
                        .map(|t| format!("`{t}`"))
                        .collect::<Vec<_>>()
                        .join(" or "),
                    link.target.as_deref().unwrap_or_default()
                ),
            ));
        }
    }
    Check::new(
        "edge-target-class",
        "Edge that resolves to a node of the wrong class",
        Severity::Error,
        "`dangling-edge` catches an edge to nothing. This catches an edge to the wrong \
         thing — which resolves, traverses, and exports, and is simply false. The class \
         names the target class for each relationship it declares; a declaration that \
         names none licenses any target.",
        violations,
    )
}

/// Separators that turn a tag into a tag-plus-something.
///
/// A plain space is not among them, deliberately. `[open questions]` is ordinary prose and
/// reference-style link text; flagging it would put a permanent finding in front of every
/// reader who writes the phrase, and a permanently non-empty report is where a real finding
/// gets lost. What is reported is a tag followed by punctuation that reads as *and here is
/// the rest of it*.
const TAG_SEPARATORS: &[char] = &['—', '–', '-', ':', ';', ',', '|', '/', '='];

/// Bracketed forms that open with an evidence tag and are not one.
///
/// The counter matches exact tokens, so `[verified — Pearl 2009]` matches nothing and is
/// counted as **no claim at all** — the opposite of the mention-counted-as-use failure and
/// just as silent. A reader who writes that has plainly intended a tag, and the count
/// absorbs the authoring error rather than reporting it. Counting it would mean guessing
/// what was meant; saying so does not.
///
/// Warn rather than Error: the node is still readable and the fix is the author's to make.
pub fn claim_tag_malformed(nodes: &[Node]) -> Check {
    let mut violations = Vec::new();
    for n in nodes {
        // Masked, so a node explaining the vocabulary is not reported for naming it — the
        // same reason the counter masks.
        for (line, found) in near_miss_tags(&crate::markdown::mask_code(&n.text)) {
            violations.push(Violation::new(
                format!("{}:{}", n.rel, line),
                format!(
                    "`{found}` opens with an evidence tag and is not one, so it is counted \
                     as no claim at all — write the tag alone and put the citation beside it"
                ),
            ));
        }
    }
    Check::new(
        "claim-tag-malformed",
        "Evidence tag that counts as nothing",
        Severity::Warn,
        "The three tokens are matched exactly, which is what keeps `[open questions](…)` \
         from reading as an open claim. The cost is that a tag with its citation folded \
         inside the brackets matches nothing and is silently counted as untagged — a node \
         that looks tagged to a reader and reads as bare assertion to every counter. This \
         reports the near miss rather than guessing at it.",
        violations,
    )
}

/// `(line, text)` for each bracketed near miss. Links are not near misses.
fn near_miss_tags(text: &str) -> Vec<(usize, String)> {
    let tags = [
        crate::claims::VERIFIED,
        crate::claims::INFERENCE,
        crate::claims::OPEN,
    ];
    let mut out = Vec::new();
    for (i, line) in text.lines().enumerate() {
        let bytes = line.as_bytes();
        let mut j = 0;
        while j < bytes.len() {
            let Some(open) = line[j..].find('[') else {
                break;
            };
            let at = j + open;
            let Some(close_rel) = line[at..].find(']') else {
                break;
            };
            let close = at + close_rel;
            j = close + 1;
            // `[text](target)` and `[text][ref]` are links; their label is not a claim.
            if matches!(line[j..].chars().next(), Some('(') | Some('[')) {
                continue;
            }
            let inner = &line[at + 1..close];
            for tag in tags {
                let word = tag.trim_matches(|c| c == '[' || c == ']');
                if inner == word {
                    break; // the tag itself, exactly as intended
                }
                let Some(rest) = inner.strip_prefix(word) else {
                    continue;
                };
                if rest
                    .trim_start()
                    .starts_with(|c: char| TAG_SEPARATORS.contains(&c))
                {
                    out.push((i + 1, line[at..=close].to_string()));
                    break;
                }
            }
        }
    }
    out
}

// ── catalog ───────────────────────────────────────────────────────────────────

/// Every repository path a node points at: `links:` targets and markdown links alike, each
/// resolved from the node's own directory and normalized.
///
/// **The one answer to "what does this node link to?"** There were three matchers asking it,
/// and two of them asked it by searching the node's bytes for a slug: `lint`'s citation
/// counter and `catalog-audit`'s. Both reported a node that merely *named* a source as one
/// that cites it, which the conventions do not say and a reader chasing the finding does not
/// find. A third copy is where the next divergence goes.
pub fn linked_paths(node_path: &Path, rel: &str, text: &str) -> HashSet<PathBuf> {
    let dir = node_path.parent().unwrap_or(node_path);
    let inst: crate::parse::CorpusInstance = serde_yaml::from_str(text).unwrap_or_default();
    let mut out: HashSet<PathBuf> = inst
        .links
        .as_deref()
        .unwrap_or(&[])
        .iter()
        .filter_map(|l| l.target.as_ref())
        .map(|t| normalize(&dir.join(t)))
        .collect();
    out.extend(
        prose_links(rel, dir, text)
            .into_iter()
            .map(|l| normalize(&l.resolved)),
    );
    out
}

/// Corpus nodes citing each source, by repo-relative path.
///
/// **A citation is a link that resolves to the catalog file.** That is what the conventions
/// describe — "corpus nodes link to catalog nodes as edges […] writes
/// `[Pearl 2009](../../catalog/pearl-2009.md)`" — and it used to be a substring search for
/// the slug anywhere in the node's bytes, which admits every sentence that happens to
/// contain the word.
///
/// The reported case was a catalog entry carrying `obtained: false` whose slug named a
/// connector crate — the directory conventions recommend naming connectors after what they
/// fetch (`nwis`, `echo`, `census`), so a catalog entry for the source those connectors
/// fetch from naturally collides. A node mentioning the crate in backticks, linking nothing,
/// failed the build under `catalog-unobtained-but-cited`, which is Error severity and gates.
/// The finding named a file whose text contains no citation, so the diagnosis ran through
/// "which node cites this?" before arriving at "none of them do".
///
/// Both edge forms count: a `links:` entry and a markdown link in the prose. The prose scan
/// is [`prose_links`], the same function `broken-prose-link` uses, so a link shown as an
/// example in code is not read as a citation here either.
pub fn citations(sources: &[Source], nodes: &[Node]) -> Vec<Vec<String>> {
    // Resolved once per node rather than once per (node, source) pair, and from [`Node::text`]
    // rather than from a parallel `Vec<String>` the caller built by re-reading every instance.
    // The gate's hot path read the corpus twice and held two copies of it to run one check.
    let linked: Vec<HashSet<PathBuf>> = nodes
        .iter()
        .map(|n| linked_paths(&n.path, &n.rel, &n.text))
        .collect();

    sources
        .iter()
        .map(|s| {
            let target = normalize(&s.path);
            linked
                .iter()
                .zip(nodes)
                .filter(|(links, _)| links.contains(&target))
                .map(|(_, n)| n.rel.clone())
                .collect()
        })
        .collect()
}

/// A `[verified]` claim in a node that draws on no registered source.
///
/// The claim vocabulary is the epistemic core of the system and, until this check, its
/// correctness was unenforced in the one direction that matters. `claims.rs` records which
/// direction that is — it was measured against a mature corpus and every one of the eight
/// miscounts *promoted*:
///
/// > That is the flattering direction, and it is the one direction this vocabulary exists to
/// > prevent.
///
/// `catalog/` exists to record provenance, and the three checks that read it
/// (`catalog-uncited`, `catalog-unobtained-but-cited`, `catalog-used-by-drift`) check the
/// catalog's own bookkeeping — whether entries and citations agree with each other. None of
/// them asks the question the vocabulary is *for*: whether a claim asserted at the strongest
/// standing rests on anything at all.
///
/// `[verified]` means **supported by a committed primary source**. A node making one while
/// linking to no catalog entry has asserted a standing it cannot demonstrate from inside the
/// repository.
///
/// # What counts as resting on something
///
/// A link that resolves to a file under `.yidam/catalog/`. That is the same resolution
/// [`citations`] performs, so a node this check calls unsourced is exactly a node absent from
/// every entry's citation list — the two cannot come to disagree about what a citation is.
///
/// **A `cites:` into a dependency does not count**, and that is RFC-0019's rule rather than a
/// simplification. A foreign tag is the producer's tag: it records what *that* corpus's
/// electors accepted, travels with none of the apparatus that made it accountable, and is
/// recorded as an observation rather than folded into a local standing. `agent-conduct.md`
/// prescribes what to do instead — put what you took into a local node, at this corpus's
/// standard — and a local `[verified]` still owes a local source.
///
/// # Warn, not Error
///
/// The corpus is not malformed and traversal is unaffected; what is wrong is a claim about
/// evidence, and the fix is either a citation or a demotion — both of which are an author's
/// judgement rather than a mechanical repair. Gating would also make adopting this check a
/// build break in every corpus that predates it, which is the failure
/// `docs/post-genesis-measurement.md` recorded for a ratchet nobody could satisfy.
///
/// Measured across three instrumented derived repositories before choosing the level; the
/// figures are in the pull request that added it.
pub fn verified_unsourced(
    nodes: &[Node],
    sources: &[Source],
    fields: &crate::claims::ClaimFields,
) -> Check {
    let registered: HashSet<PathBuf> = sources.iter().map(|s| normalize(&s.path)).collect();
    let violations = nodes
        .iter()
        .filter_map(|n| {
            // Through the counter rather than by filtering `claims_in_node` on a spelling.
            // `ServedClaim::standing` is the bare word and `claims::VERIFIED` is the
            // bracketed marker, so comparing the two compares nothing — which is how the
            // first draft of this check reported zero findings against a corpus holding 456
            // verified claims, and looked exactly like a clean bill of health. The counter
            // is already held to agree with the served list tag for tag.
            let verified =
                crate::claims::count_in_node(&n.text, fields.for_class(&class_of(n))).verified;
            if verified == 0 {
                return None;
            }
            let links = linked_paths(&n.path, &n.rel, &n.text);
            if links.iter().any(|p| registered.contains(p)) {
                return None;
            }
            // Named separately because the fix differs: a node already citing a dependency
            // has *some* provenance recorded and needs it localized, and one citing nothing
            // has none.
            let external = n.inst.cites.as_deref().is_some_and(|c| !c.is_empty());
            let detail = match (verified, external) {
                (1, false) => "1 `[verified]` claim, and this node draws on no source".to_string(),
                (n, false) => format!("{n} `[verified]` claims, and this node draws on no source"),
                (1, true) => "1 `[verified]` claim resting only on a `cites:` into a dependency — \
                              a foreign tag is the producer's, and does not transfer"
                    .to_string(),
                (n, true) => format!(
                    "{n} `[verified]` claims resting only on a `cites:` into a dependency — a \
                     foreign tag is the producer's, and does not transfer"
                ),
            };
            Some(Violation::new(&n.rel, detail))
        })
        .collect();
    Check::new(
        "verified-unsourced",
        "A claim asserted at `[verified]` that rests on nothing",
        Severity::Warn,
        "`[verified]` means supported by a committed primary source. A node asserting one \
         while linking to no catalog entry has claimed a standing it cannot demonstrate. \
         This is the one direction the claim vocabulary exists to prevent: over-counting \
         evidence is the flattering error, and a mature corpus measured eight miscounts of \
         which all eight promoted. The catalog checks beside this one verify the catalog's \
         own bookkeeping; this is the only one that asks whether a claim rests on anything. \
         The fix is a citation or a demotion, and which of the two is the author's call — \
         nothing here proposes a promotion, ever.",
        violations,
    )
}

/// How many corpus nodes rest on a source, and which of them.
///
/// #270 asked for the expiry question to reach *"claims citing it"*, and the honest way to do
/// that is not to write the question into each of them. Measured across the three
/// instrumented repositories, one entry is cited by a median of 3, 5 and 3 nodes — and by a
/// maximum of **84**, which is a web service, exactly the kind of record a `ttl_days` is
/// declared on. `propose` writes one commit per proposal, so propagation would mean 84
/// commits for one aged file and 84 more to retire them. The finding names them instead.
///
/// It is also what E3 already does for the analogous case: `citations::Movement` carries the
/// **local** node whose claim is affected, *"because this report is about my graph and not
/// theirs"*.
///
/// Three names, then a count. The median entry in all three corpora is listed in full, and
/// the tail says how much more there is rather than printing it.
fn resting_clause(nodes: &[String]) -> String {
    if nodes.is_empty() {
        // Worth saying, and it changes what the reader does: an aged record nothing rests on
        // may want deleting rather than refreshing.
        return " Nothing cites it.".to_string();
    }
    let mut named: Vec<&str> = nodes.iter().map(|n| corpus_address(n)).collect();
    named.sort_unstable();
    let listed = named
        .iter()
        .take(RESTING_NAMED)
        .copied()
        .collect::<Vec<_>>()
        .join(", ");
    match named.len().saturating_sub(RESTING_NAMED) {
        0 => format!(" {} node(s) rest on it: {listed}.", named.len()),
        more => format!(
            " {} node(s) rest on it: {listed}, and {more} more.",
            named.len()
        ),
    }
}

/// How many citing nodes a finding names before it starts counting.
const RESTING_NAMED: usize = 3;

/// `.yidam/corpus/gage/canyon-outlet.yml` → `gage/canyon-outlet.yml`.
///
/// The address `rename` and `neighbors` take, rather than [`basename`]: two classes may hold
/// a node of the same name, and a list of bare stems would not say which one aged.
fn corpus_address(rel: &str) -> &str {
    rel.strip_prefix(".yidam/corpus/").unwrap_or(rel)
}

/// A catalog record that has stood longer than the corpus said it may.
///
/// `docs/domain-computer.md` specified "refreshed on TTL or on demand" and there was no TTL:
/// an entry recorded where something came from and never that the record had aged, so a
/// corpus resting on a source fetched a year ago read exactly like one fetched today.
///
/// **An aged source is a thing to look at, not an error.** Refreshing it is a knowledge
/// event a person should own, so this reports and does not gate — and it says only what it
/// knows. An expiry does not claim the upstream changed; nothing here can see upstream, and
/// `doctor` must keep doing no network. It claims that nobody has looked.
///
/// Two silences are kept apart, because the fix differs. An entry with no applicable TTL is
/// not reported at all: absent a declaration the corpus never asked to be told, which is
/// every corpus until someone turns it on. An entry that *does* carry a TTL and has no date
/// to measure against is reported as **undatable** rather than as expired — a gap in the
/// bookkeeping, not a stale source, and calling it stale would assert something nobody knows.
pub fn catalog_expired(
    ages: &[super::ttl::Age],
    sources: &[Source],
    cites: &[Vec<String>],
) -> Check {
    let resting: HashMap<&str, &Vec<String>> = sources
        .iter()
        .map(|s| s.rel.as_str())
        .zip(cites.iter())
        .collect();
    let empty: Vec<String> = Vec::new();
    let violations = ages
        .iter()
        .filter_map(|a| {
            let on = resting_clause(resting.get(a.entry.as_str()).copied().unwrap_or(&empty));
            if a.undatable() {
                return Some(Violation::new(
                    &a.entry,
                    format!(
                        "declares a {}-day TTL and nothing records when it was fetched — add \
                         `retrieved:`, or commit the entry so its date can be read.{on}",
                        a.ttl_days.unwrap_or(0)
                    ),
                ));
            }
            let over = a.overdue_days()?;
            Some(Violation::new(
                &a.entry,
                format!(
                    "retrieved {} ({}), {} day(s) ago against a {}-day TTL — {} day(s) past.{on}",
                    a.retrieved.as_deref().unwrap_or("?"),
                    a.dated.map(|d| d.as_str()).unwrap_or("?"),
                    a.age_days.unwrap_or(0),
                    a.ttl_days.unwrap_or(0),
                    over
                ),
            ))
        })
        .collect();
    Check::new(
        "catalog-expired",
        "A source record older than the corpus said it may be",
        Severity::Warn,
        "A catalog entry records where something came from; until it could also record how \
         long that record may stand, a source fetched a year ago read exactly like one \
         fetched today. Declare `ttl_days:` on the entry — a gauge record and a statute do \
         not age at the same rate — or a corpus-wide default under `[catalog]` in \
         `.yidam/config.toml`. Absent both, nothing expires. This reports rather than gates \
         because an aged record is a thing to look at, and refreshing it is a knowledge event \
         a person owns. It does not claim the source changed: nothing here reads upstream, \
         and `doctor` does no network. It claims nobody has looked. The finding names the \
         nodes resting on the record, because the reader who has to act is the one who wrote \
         them — and naming them here is what keeps the question in one file instead of \
         copied into every node that cites it.",
        violations,
    )
}

pub fn catalog_uncited(sources: &[Source], cites: &[Vec<String>]) -> Check {
    let violations = sources
        .iter()
        .zip(cites)
        .filter(|(s, c)| s.obtained && c.is_empty())
        .map(|(s, _)| Violation::new(&s.rel, "no corpus node draws on this source"))
        .collect();
    Check::new(
        "catalog-uncited",
        "Registered source nothing draws from",
        Severity::Info,
        "A catalog entry describes a source, not knowledge, so an uncited one is not an \
         error. It is either a source registered ahead of the extraction that will use it, \
         or a claim resting on evidence that never became a node. The two look identical \
         from here and are worth telling apart — which is what `obtained: false` declares. \
         Entries that have said they are the former are not reported.",
        violations,
    )
}

pub fn catalog_unobtained_but_cited(sources: &[Source], cites: &[Vec<String>]) -> Check {
    let violations = sources
        .iter()
        .zip(cites)
        .filter(|(s, c)| !s.obtained && !c.is_empty())
        .map(|(s, c)| {
            Violation::new(
                &s.rel,
                format!(
                    "`obtained: false` but {} node(s) cite it, e.g. {}",
                    c.len(),
                    c[0]
                ),
            )
        })
        .collect();
    Check::new(
        "catalog-unobtained-but-cited",
        "Source nobody has fetched, cited anyway",
        Severity::Error,
        "This is the price of the `obtained: false` exemption, which is otherwise the \
         cheapest way to silence `catalog-uncited`. A node drawing on a source the catalog \
         says was never retrieved is one of two things and both are defects: the flag is \
         stale and was not cleared when the source arrived, or the citation rests on \
         something nobody has read.",
        violations,
    )
}

/// How a declared `used-by` list disagrees with the citations, by basename.
///
/// Basenames rather than paths because the list is hand-written and a person writing one
/// writes `tailwater.yml`, not `.yidam/corpus/concept/tailwater.yml`. Comparing full paths
/// would report drift on every entry whose author spelled the node the way the convention
/// asks them to.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
pub struct UsedByDrift {
    /// Names the list claims, that no node citing this entry carries.
    pub claimed_not_citing: Vec<String>,
    /// Nodes that cite this entry, that the list omits.
    pub citing_not_claimed: Vec<String>,
}

/// The disagreement between a declared `used-by` list and the citations, or `None` when the
/// entry declares no list — **absent is not drift**, and that distinction is the whole
/// reason this returns an `Option` rather than an empty struct.
///
/// One function because two consumers ask the same question and must not answer it
/// differently: `catalog-used-by-drift` renders it as a gated violation, and
/// `catalog-audit` reports it as a field an editor navigates by. A second copy is exactly
/// how the two counts in this file's own history came to disagree.
pub fn used_by_drift(used_by: &[String], citing: &[String]) -> Option<UsedByDrift> {
    if used_by.is_empty() {
        return None;
    }
    let claimed: HashSet<&str> = used_by.iter().map(|u| basename(u)).collect();
    let found: HashSet<&str> = citing.iter().map(|a| basename(a)).collect();
    let mut claimed_not_citing: Vec<String> = claimed
        .difference(&found)
        .map(|s| (*s).to_string())
        .collect();
    let mut citing_not_claimed: Vec<String> = found
        .difference(&claimed)
        .map(|s| (*s).to_string())
        .collect();
    claimed_not_citing.sort_unstable();
    citing_not_claimed.sort_unstable();
    Some(UsedByDrift {
        claimed_not_citing,
        citing_not_claimed,
    })
}

pub fn catalog_used_by_drift(sources: &[Source], cites: &[Vec<String>]) -> Check {
    let mut violations = Vec::new();
    for (s, actual) in sources.iter().zip(cites) {
        // `None` is the optional list being absent, which is not drift.
        let Some(drift) = used_by_drift(&s.used_by, actual) else {
            continue;
        };
        let mut detail = Vec::new();
        if !drift.claimed_not_citing.is_empty() {
            detail.push(format!(
                "claims {} that do not cite it",
                drift.claimed_not_citing.join(", ")
            ));
        }
        if !drift.citing_not_claimed.is_empty() {
            detail.push(format!(
                "omits {} that do",
                drift.citing_not_claimed.join(", ")
            ));
        }
        if !detail.is_empty() {
            violations.push(Violation::new(&s.rel, detail.join("; ")));
        }
    }
    Check::new(
        "catalog-used-by-drift",
        "`used-by` list disagreeing with the citations",
        Severity::Warn,
        "The citations are authoritative — they cannot drift from the corpus, and a \
         hand-maintained list can. Both are kept so the disagreement is visible rather than \
         averaged away. The list is optional; an entry that declares one is asserting it is \
         current.",
        violations,
    )
}

fn basename(p: &str) -> &str {
    p.rsplit('/').next().unwrap_or(p)
}

/// An artifact record that cannot be read as one.
///
/// # Why this and not a parse failure
///
/// [`crate::parse::CatalogArtifact`] takes every field as `Option` so that one bad line does
/// not make the whole entry unreadable — an entry that fails to load takes its citations, its
/// TTL and its `used-by` down with it, which is a large penalty for a mistyped digest. The
/// cost of that choice is that malformedness has to be *reported*, and this is the report.
///
/// # What it does not check
///
/// **Whether the bytes exist, and whether they hash correctly.** Both are facts about the
/// machine running the check rather than about `HEAD`, and no check in this file has ever read
/// machine-local state — `lint` reads the working tree and nothing else. A gate whose verdict
/// depends on which machine ran it is one a corpus cannot reason about, so the cache lives
/// behind `yidam vault verify` where a per-machine answer is the point.
///
/// So this reports only what the repository can settle from its own files, and it fires only
/// on entries that declare `artifacts:` at all. A corpus that has not adopted the field gets
/// no findings from it.
pub fn catalog_artifact_malformed(sources: &[Source]) -> Check {
    let mut violations = Vec::new();
    for s in sources {
        for (i, a) in s.artifacts.iter().enumerate() {
            let mut problems = Vec::new();
            match a.sha256.as_deref().map(str::trim) {
                None | Some("") => problems.push("no `sha256`".to_string()),
                Some(h) if h.len() != 64 => {
                    problems.push(format!("`sha256` is {} characters, not 64: {h:?}", h.len()))
                }
                Some(h) if !h.chars().all(|c| matches!(c, '0'..='9' | 'a'..='f')) => {
                    // Uppercase named separately: hex is case-insensitive and a store is not,
                    // so the two spellings would be two keys for one artifact.
                    if h.chars().any(|c| c.is_ascii_uppercase()) {
                        problems.push(format!(
                            "`sha256` is uppercase; vault keys are lowercase hex — use {}",
                            h.to_ascii_lowercase()
                        ));
                    } else {
                        problems.push(format!("`sha256` is not hex: {h:?}"));
                    }
                }
                Some(_) => {}
            }
            // A `from:` index naming no location is the one cross-field error worth having:
            // it means the record cites a provenance the entry does not carry.
            if let Some(crate::parse::ArtifactOrigin::Location(n)) = &a.from {
                if *n >= s.locations.len() {
                    problems.push(format!(
                        "`from: {n}` names location {n} and this entry declares {}",
                        s.locations.len()
                    ));
                }
            }
            if !problems.is_empty() {
                violations.push(Violation::new(
                    &s.rel,
                    format!("artifacts[{i}]: {}", problems.join("; ")),
                ));
            }
        }
    }
    Check::new(
        "catalog-artifact-malformed",
        "Obtained-artifact record is missing or mistyped",
        Severity::Error,
        "An artifact record is what makes `obtained: true` demonstrable rather than asserted \
         — the digest names the bytes the entry claims to have. A record without a well-formed \
         digest names nothing, so it is a claim of retrieval with no more standing than the \
         flag it was added to support. Error, because the failure is in the record itself and \
         is settled entirely by files in this repository.",
        violations,
    )
}

/// An artifact routed to a vault this repository does not declare.
///
/// # Why this may gate
///
/// **Both sides are committed.** The record is in `.yidam/catalog/`, the vault is in
/// `.yidam/config.toml`, and the check compares one against the other — so it answers
/// identically in every clone, and a finding is a defect in the corpus rather than a fact
/// about your credentials or your network. Whether a *declared* vault can actually be reached
/// is a different question and belongs to `yidam vault status`.
///
/// `vault: none` is a route, not an absence: it says these bytes stay in the local cache. It
/// is spelled rather than omitted so that "nobody has decided" and "decided to keep it here"
/// are different states.
pub fn catalog_artifact_unroutable(sources: &[Source], declared: &[String]) -> Check {
    let mut violations = Vec::new();
    for s in sources {
        for (i, a) in s.artifacts.iter().enumerate() {
            let Some(name) = a.vault.as_deref().map(str::trim).filter(|v| !v.is_empty()) else {
                continue;
            };
            if name == "none" || declared.iter().any(|d| d == name) {
                continue;
            }
            let known = if declared.is_empty() {
                "this repository declares no vault".to_string()
            } else {
                format!(
                    "declared: {}",
                    declared
                        .iter()
                        .map(|d| format!("`{d}`"))
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            };
            violations.push(Violation::new(
                &s.rel,
                format!("artifacts[{i}]: `vault: {name}` names no declared vault ({known})"),
            ));
        }
    }
    Check::new(
        "catalog-artifact-unroutable",
        "Artifact routed to a vault nothing declares",
        Severity::Error,
        "A record naming a vault that `.yidam/config.toml` does not declare is a route to \
         nowhere: nothing will ever store or fetch those bytes, and the entry reads as though \
         something will. Both sides are committed, so this check answers identically in every \
         clone — it reports a defect in the corpus, never a fact about your credentials. \
         `vault: none` is a route and means the local cache and nowhere else.",
        violations,
    )
}

pub fn catalog_location_malformed(sources: &[Source]) -> Check {
    let mut violations = Vec::new();
    for s in sources {
        for (i, loc) in s.locations.iter().enumerate() {
            let mut problems = Vec::new();
            match loc.kind.as_deref() {
                None => problems.push("no `kind`".to_string()),
                Some(k) if !CATALOG_LOCATION_KINDS.contains(&k) => {
                    problems.push(format!(
                        "kind `{k}` is not one of {}",
                        CATALOG_LOCATION_KINDS.join(", ")
                    ));
                }
                Some(k) => {
                    // The type decides whether a reader is offered a link, so a value
                    // whose shape contradicts its type renders wrongly.
                    let v = loc.value.as_deref().unwrap_or("");
                    if k == "url" && !v.starts_with("http://") && !v.starts_with("https://") {
                        problems.push(format!("kind `url` but value is not a URL: {v}"));
                    }
                    if k == "url_template" && !v.contains('{') {
                        problems
                            .push("kind `url_template` but value has no `{…}` placeholder".into());
                    }
                }
            }
            if loc.value.as_deref().unwrap_or("").trim().is_empty() {
                problems.push("no `value`".to_string());
            }
            if s.locations.len() > 1 && loc.description.is_none() {
                problems.push("several locations and no `description` to tell them apart".into());
            }
            if !problems.is_empty() {
                violations.push(Violation::new(
                    &s.rel,
                    format!("location[{i}]: {}", problems.join("; ")),
                ));
            }
        }
    }
    Check::new(
        "catalog-location-malformed",
        "Catalog location missing or mistyped",
        Severity::Warn,
        "A `location` is a list of typed places. The type decides whether a reader is \
         offered a link, so a value whose shape contradicts its type renders wrongly; and \
         where an entry has several locations, the description is the only thing \
         distinguishing them.",
        violations,
    )
}

// ── presentation ──────────────────────────────────────────────────────────────

/// Cells of a markdown table row, ignoring the leading and trailing pipes.
fn table_cells(line: &str) -> Vec<&str> {
    let t = line.trim();
    let t = t.strip_prefix('|').unwrap_or(t);
    let t = t.strip_suffix('|').unwrap_or(t);
    t.split('|').collect()
}

fn is_delimiter_row(line: &str) -> bool {
    let cells = table_cells(line);
    !cells.is_empty()
        && cells
            .iter()
            .all(|c| !c.trim().is_empty() && c.trim().chars().all(|ch| ch == '-' || ch == ':'))
}

/// REGEN blocks that took lines which were not theirs.
///
/// `yidam regen` writes into a block by finding its open tag, its arrow and its close tag. If
/// any of the three is missing the write is a **silent no-op** — `update_regen` returns the
/// text unchanged — so the section simply stops updating, and the failure reads as "that part
/// never refreshes" rather than as a malformed file. Where a marker below it gets swallowed
/// as content, that marker's section stops updating too, and nothing anywhere says so.
///
/// The scan is `yidam_core::markers::scan_markers`, not a reader of this check's own. A
/// second scanner would be a second answer to "where does a block end", and the two would
/// drift without either going red — the argument `formal_specs.rs` makes about a workflow
/// that reproduces a task instead of running it. It is also this repository's first consumer
/// of a function that had three implementations, shared fixtures, and nothing calling it.
///
/// Warn rather than Error, and the reason is a limit of the scanner rather than a doubt about
/// the finding: it does not know about fenced code blocks, so a document explaining the marker
/// syntax with a deliberately unbalanced example is indistinguishable from a damaged file.
/// Gating on a check that cannot tell those apart is how `--warn-only` gets typed, which turns
/// every check off at once. Every markdown file in this repository passes it as written.
pub fn malformed_regen_block(files: &[(String, String)]) -> Check {
    use yidam_core::markers::Fault;
    let mut violations = Vec::new();
    for (rel, text) in files {
        for b in yidam_core::markers::scan_markers(text).malformed {
            let what = match b.fault {
                Fault::OpenArrowMissing => {
                    "its opening comment never closes: no line after it ends in `-->`, so \
                     everything below became part of the tag"
                }
                Fault::CloseTagMissing => {
                    "`<!-- /REGEN -->` never arrives, so the rest of the file became its body"
                }
                Fault::ClosedOnAnothersTag => {
                    "it closes on a `<!-- /REGEN -->` belonging to a block opened inside its \
                     own body — a close tag is missing between them"
                }
            };
            let lost = match b.swallowed_markers {
                0 => String::new(),
                n => format!(
                    ", {n} of which open{} a marker that is now content",
                    if n == 1 { "s" } else { "" }
                ),
            };
            violations.push(Violation::new(
                rel,
                format!(
                    "line {}: the `{}` block — {what}. It took the {} line(s) after it{lost}. \
                     `yidam regen` will not write to it.",
                    b.line, b.command, b.swallowed_lines
                ),
            ));
        }
    }
    Check::new(
        "malformed-regen-block",
        "REGEN block whose extent is not what it looks like",
        Severity::Warn,
        "A REGEN block is three markers: the open tag, the `-->` that ends it, and \
         `<!-- /REGEN -->`. `yidam regen` finds all three or writes nothing at all, without \
         saying so — the section goes stale and looks like a generator that stopped running. \
         Where the missing tag is a close tag, the block also swallows whatever is below it, \
         so the *next* section stops updating for the same invisible reason.",
        violations,
    )
}

/// Ragged markdown tables in any committed prose.
///
/// Applied to catalog entries and the corpus's own READMEs — anywhere a reader meets a
/// table. A table whose rows disagree on width does not render as a table; it renders as
/// a paragraph of pipes.
pub fn malformed_table(files: &[(String, String)]) -> Check {
    let mut violations = Vec::new();
    for (rel, text) in files {
        let lines: Vec<&str> = text.lines().collect();
        let mut i = 0;
        while i < lines.len() {
            // A table is a header row followed by a delimiter row.
            if !lines[i].trim_start().starts_with('|') || i + 1 >= lines.len() {
                i += 1;
                continue;
            }
            if !is_delimiter_row(lines[i + 1]) {
                i += 1;
                continue;
            }
            let width = table_cells(lines[i]).len();
            if table_cells(lines[i + 1]).len() != width {
                violations.push(Violation::new(
                    rel,
                    format!(
                        "line {}: header has {width} cells, delimiter has {} — renders as a \
                         paragraph of pipes",
                        i + 1,
                        table_cells(lines[i + 1]).len()
                    ),
                ));
            }
            let mut j = i + 2;
            let mut ragged = Vec::new();
            while j < lines.len() && lines[j].trim_start().starts_with('|') {
                if table_cells(lines[j]).len() != width {
                    ragged.push(j + 1);
                }
                j += 1;
            }
            if !ragged.is_empty() {
                violations.push(Violation::new(
                    rel,
                    format!(
                        "row(s) at line {} have a different width than the {width}-cell header",
                        ragged
                            .iter()
                            .map(|n| n.to_string())
                            .collect::<Vec<_>>()
                            .join(", ")
                    ),
                ));
            }
            i = j;
        }
    }
    Check::new(
        "malformed-table",
        "Markdown table whose rows disagree on width",
        Severity::Warn,
        "A ragged table does not render as a table. Every reader of that file sees a \
         paragraph of pipes instead of the thing the author was trying to show, and the \
         author usually never looks at the rendered output again.",
        violations,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A corpus declaring no universal properties — the state every corpus starts in, and
    /// what these cases are about. The universal path has its own tests below and in
    /// `crate::universal`.
    const NONE: crate::universal::Universal = crate::universal::Universal::empty();

    fn node(rel: &str, yaml: &str) -> Node {
        Node {
            path: PathBuf::from(rel),
            rel: rel.to_string(),
            inst: serde_yaml::from_str(yaml).unwrap(),
            text: yaml.to_string(),
        }
    }

    fn edge(relationship: &str, target: &str, direction: &str) -> ClassEdge {
        ClassEdge {
            relationship: relationship.into(),
            target: target.into(),
            direction: Some(direction.into()),
        }
    }

    /// `None` and an empty drift are different answers, and the report emits them as
    /// different values — `null` for an entry that never claimed anything, an object with two
    /// empty arrays for one whose claim holds. Collapsing them would make an entry that keeps
    /// its list current indistinguishable from one that has none.
    #[test]
    fn an_absent_used_by_list_is_not_drift_and_an_accurate_one_is_not_either() {
        assert_eq!(used_by_drift(&[], &["a.yml".into()]), None);
        assert_eq!(
            used_by_drift(&["a.yml".into()], &[".yidam/corpus/c/a.yml".into()]),
            Some(UsedByDrift::default()),
        );
    }

    /// Basenames, because the list is hand-written and a person writes `tailwater.yml`.
    /// Comparing full paths would report drift on every entry spelled the way the
    /// convention asks for.
    #[test]
    fn drift_is_reported_in_both_directions_and_compared_by_basename() {
        let drift = used_by_drift(
            &["mixing-zone.yml".into(), "low-flow.yml".into()],
            &[
                ".yidam/corpus/concept/low-flow.yml".into(),
                ".yidam/corpus/concept/tailwater.yml".into(),
            ],
        )
        .expect("a list was declared");
        assert_eq!(drift.claimed_not_citing, vec!["mixing-zone.yml"]);
        assert_eq!(drift.citing_not_claimed, vec!["tailwater.yml"]);
    }

    fn source(slug: &str, obtained: bool, used_by: &[&str]) -> Source {
        Source {
            rel: format!(".yidam/catalog/{slug}.md"),
            path: PathBuf::from(format!("/repo/.yidam/catalog/{slug}.md")),
            obtained,
            used_by: used_by.iter().map(|s| s.to_string()).collect(),
            locations: vec![],
            retrieved: None,
            ttl_days: None,
            artifacts: Vec::new(),
        }
    }

    /// The `type: claim` field a class declared, which is what the structural arm reads.
    ///
    /// Keyed on `concept` rather than on the node's `class:` field, because `class_of`
    /// reads the *directory* — instances live in `corpus/<class>/` and the two agree by
    /// convention. A fixture keyed on the YAML field silently exercises the prose arm only.
    fn claim_fields() -> crate::claims::ClaimFields {
        crate::claims::ClaimFields::from_declarations([(
            "concept".to_string(),
            vec!["claim_tag".to_string()],
        )])
    }

    /// The finding: a claim at the strongest standing, and nothing registered beneath it.
    #[test]
    fn a_verified_claim_with_no_citation_is_reported() {
        let nodes = vec![corpus_node(
            "a",
            "class: c\ndescription: |\n  A statement. [verified]\nlinks:\n  \
             - target: ../concept.ont.yml\n",
        )];
        let c = verified_unsourced(&nodes, &[catalog_source("nwis", true)], &claim_fields());
        assert_eq!(c.violations.len(), 1);
        assert_eq!(c.violations[0].node, ".yidam/corpus/concept/a.yml");
        assert!(
            c.violations[0].detail.contains("1 `[verified]` claim"),
            "{:?}",
            c.violations[0]
        );
    }

    /// A link that resolves to a catalog entry is what "rests on something" means, and it is
    /// the same resolution `citations` performs.
    #[test]
    fn a_verified_claim_citing_a_source_is_not_reported() {
        let nodes = vec![corpus_node(
            "a",
            "class: c\ndescription: |\n  A statement. [verified]\nlinks:\n  \
             - target: ../../catalog/nwis.md\n",
        )];
        let c = verified_unsourced(&nodes, &[catalog_source("nwis", true)], &claim_fields());
        assert!(c.passed(), "{:?}", c.violations);
    }

    /// The vocabulary's other two standings assert nothing this check is about. A corpus
    /// that is honest about what it does not know must not be reported for it.
    #[test]
    fn an_inference_or_an_open_question_is_not_reported() {
        let nodes = vec![
            corpus_node(
                "a",
                "class: c\ndescription: |\n  A conclusion. [inference]\nlinks: []\n",
            ),
            corpus_node(
                "b",
                "class: c\ndescription: |\n  A question. [open]\nlinks: []\n",
            ),
        ];
        let c = verified_unsourced(&nodes, &[catalog_source("nwis", true)], &claim_fields());
        assert!(c.passed(), "{:?}", c.violations);
    }

    /// A node-level standing declared in a `type: claim` property is a claim too — the
    /// structural arm, which a text-only scan misses entirely.
    #[test]
    fn a_structural_claim_tag_counts() {
        let nodes = vec![corpus_node(
            "a",
            "class: c\ndescription: |\n  Prose with no marker.\nproperties:\n  \
             claim_tag: verified\nlinks: []\n",
        )];
        let c = verified_unsourced(&nodes, &[catalog_source("nwis", true)], &claim_fields());
        assert_eq!(c.violations.len(), 1, "the structural arm read nothing");
    }

    /// A `cites:` into a dependency does not discharge a local `[verified]`. RFC-0019: a
    /// foreign tag is the producer's, records what *that* corpus's electors accepted, and
    /// travels without the apparatus that made it accountable.
    #[test]
    fn a_citation_into_a_dependency_does_not_count_as_a_source() {
        let nodes = vec![corpus_node(
            "a",
            "class: c\ndescription: |\n  A statement. [verified]\ncites:\n  \
             - package: upstream\n    node: concept/x.yml\n    tag: verified\nlinks: []\n",
        )];
        let c = verified_unsourced(&nodes, &[catalog_source("nwis", true)], &claim_fields());
        assert_eq!(c.violations.len(), 1);
        assert!(
            c.violations[0].detail.contains("does not transfer"),
            "the finding should say why a foreign tag is not enough: {:?}",
            c.violations[0]
        );
    }

    /// Several claims in one node are one finding. The baseline compares on `(check, node)`,
    /// so a node cannot carry two entries for the same check — and the count is what a
    /// reader needs, not five identical lines.
    #[test]
    fn several_verified_claims_in_one_node_are_one_finding() {
        let nodes = vec![corpus_node(
            "a",
            "class: c\ndescription: |\n  One. [verified] Two. [verified] Three. \
             [verified]\nlinks: []\n",
        )];
        let c = verified_unsourced(&nodes, &[catalog_source("nwis", true)], &claim_fields());
        assert_eq!(c.violations.len(), 1);
        assert!(
            c.violations[0].detail.starts_with("3 `[verified]` claims"),
            "{:?}",
            c.violations[0]
        );
    }

    /// A corpus with no catalog at all reports every verified claim, which is correct and is
    /// the state a corpus is in before it registers its first source.
    #[test]
    fn with_no_registered_sources_every_verified_claim_is_reported() {
        let nodes = vec![corpus_node(
            "a",
            "class: c\ndescription: |\n  A statement. [verified]\nlinks: []\n",
        )];
        let c = verified_unsourced(&nodes, &[], &claim_fields());
        assert_eq!(c.violations.len(), 1);
    }

    /// It reports and does not gate. Adopting it must not break the build of every corpus
    /// that predates it, and the fix — a citation or a demotion — is an author's judgement.
    #[test]
    fn the_check_reports_rather_than_gates() {
        let nodes = vec![corpus_node(
            "a",
            "class: c\ndescription: |\n  A statement. [verified]\nlinks: []\n",
        )];
        let c = verified_unsourced(&nodes, &[], &claim_fields());
        assert_eq!(c.severity, Severity::Warn);
        assert!(!c.gates(&c.violations[0]));
    }

    fn aged(
        entry: &str,
        retrieved: Option<&str>,
        dated: Option<super::super::ttl::Dated>,
        age: Option<i64>,
        ttl: Option<u32>,
    ) -> super::super::ttl::Age {
        super::super::ttl::Age {
            entry: entry.to_string(),
            retrieved: retrieved.map(str::to_string),
            dated,
            age_days: age,
            ttl_days: ttl,
        }
    }

    /// [`catalog_expired`] against entries nothing cites — the shape most of these tests
    /// are about, where the citation list is not the thing under test.
    fn expired(ages: &[super::super::ttl::Age]) -> Check {
        let none: Vec<Vec<String>> = ages.iter().map(|_| Vec::new()).collect();
        expired_cited(ages, &none)
    }

    /// [`catalog_expired`] with a citation list per entry, in the same order.
    fn expired_cited(ages: &[super::super::ttl::Age], cites: &[Vec<String>]) -> Check {
        let sources: Vec<Source> = ages
            .iter()
            .map(|a| Source {
                rel: a.entry.clone(),
                path: std::path::PathBuf::from(&a.entry),
                obtained: true,
                used_by: vec![],
                locations: vec![],
                retrieved: a.retrieved.clone(),
                ttl_days: a.ttl_days,
                artifacts: Vec::new(),
            })
            .collect();
        catalog_expired(ages, &sources, cites)
    }

    fn resting(nodes: &[&str]) -> Vec<String> {
        nodes.iter().map(|n| format!(".yidam/corpus/{n}")).collect()
    }

    /// A corpus that declared no TTL asked nothing, and is told nothing.
    #[test]
    fn an_entry_with_no_ttl_is_not_reported() {
        let c = expired(&[aged("a.md", Some("1999-01-01"), None, Some(9_000), None)]);
        assert!(c.passed(), "{:?}", c.violations);
    }

    #[test]
    fn an_entry_inside_its_ttl_is_not_reported() {
        let c = expired(&[aged("a.md", Some("2026-08-01"), None, Some(25), Some(180))]);
        assert!(c.passed(), "{:?}", c.violations);
    }

    /// The finding names the date, where the date came from, and how far past it is — the
    /// three things a maintainer needs before deciding whether to re-fetch.
    #[test]
    fn an_expired_entry_says_how_it_was_dated() {
        let c = expired(&[aged(
            "a.md",
            Some("2024-01-15"),
            Some(super::super::ttl::Dated::Declared),
            Some(954),
            Some(30),
        )]);
        assert_eq!(c.violations.len(), 1);
        let d = &c.violations[0].detail;
        assert!(d.contains("2024-01-15"), "{d}");
        assert!(d.contains("declared"), "{d}");
        assert!(d.contains("924 day(s) past"), "{d}");
    }

    /// A date read from git is labelled as such: it counts a typo fix as a refresh, so it
    /// errs in the flattering direction and a reader is owed the difference.
    #[test]
    fn a_git_dated_entry_says_so() {
        let c = expired(&[aged(
            "a.md",
            Some("2024-01-15"),
            Some(super::super::ttl::Dated::Committed),
            Some(954),
            Some(30),
        )]);
        assert!(
            c.violations[0].detail.contains("from git"),
            "{:?}",
            c.violations[0]
        );
    }

    /// A TTL with nothing to measure against is a gap in the bookkeeping, not a stale
    /// source. Calling it expired would assert something nobody knows.
    #[test]
    fn an_undatable_entry_is_reported_as_undatable_and_not_as_expired() {
        let c = expired(&[aged("a.md", None, None, None, Some(30))]);
        assert_eq!(c.violations.len(), 1);
        let d = &c.violations[0].detail;
        assert!(d.contains("nothing records when it was fetched"), "{d}");
        assert!(!d.contains("past"), "it must not read as an expiry: {d}");
    }

    /// It reports and does not gate. An aged record is a thing to look at, and refreshing it
    /// is a knowledge event a person owns.
    #[test]
    fn an_expired_entry_reports_rather_than_gates() {
        let c = expired(&[aged("a.md", Some("2024-01-15"), None, Some(954), Some(30))]);
        assert_eq!(c.severity, Severity::Warn);
        assert!(!c.gates(&c.violations[0]));
    }

    /// #270's gap, closed the cheap way: the question stays on the source and the nodes
    /// resting on it are named, rather than a copy of it being written into each of them.
    #[test]
    fn an_expired_entry_names_the_nodes_resting_on_it() {
        let c = expired_cited(
            &[aged("a.md", Some("2024-01-15"), None, Some(954), Some(30))],
            &[resting(&["gage/canyon-outlet.yml", "reach/tailwater.yml"])],
        );
        let d = &c.violations[0].detail;
        assert!(d.contains("2 node(s) rest on it"), "{d}");
        assert!(d.contains("gage/canyon-outlet.yml"), "{d}");
        assert!(d.contains("reach/tailwater.yml"), "{d}");
        assert!(!d.contains("more"), "two fits without a tail: {d}");
    }

    /// The address `rename` takes, not a bare stem: two classes may hold a node of the same
    /// name, and a list of stems would not say which one aged.
    #[test]
    fn a_resting_node_is_named_by_class_and_stem() {
        let c = expired_cited(
            &[aged("a.md", Some("2024-01-15"), None, Some(954), Some(30))],
            &[resting(&["gage/outlet.yml", "reach/outlet.yml"])],
        );
        let d = &c.violations[0].detail;
        assert!(d.contains("gage/outlet.yml, reach/outlet.yml"), "{d}");
        assert!(
            !d.contains(".yidam/corpus/"),
            "the prefix is noise here: {d}"
        );
    }

    /// The tail is counted, not printed. Measured: one entry in a real corpus is cited by
    /// 84 nodes, and a finding that listed them would be unreadable — which is the same
    /// reason the question is not written into each of them.
    #[test]
    fn a_long_citation_list_is_named_three_deep_and_then_counted() {
        let many: Vec<&str> = vec![
            "a/1.yml", "a/2.yml", "a/3.yml", "a/4.yml", "a/5.yml", "a/6.yml",
        ];
        let c = expired_cited(
            &[aged("a.md", Some("2024-01-15"), None, Some(954), Some(30))],
            &[resting(&many)],
        );
        let d = &c.violations[0].detail;
        assert!(d.contains("6 node(s) rest on it"), "{d}");
        assert!(d.contains("a/1.yml, a/2.yml, a/3.yml, and 3 more"), "{d}");
        assert!(!d.contains("a/4.yml"), "{d}");
    }

    /// Worth saying, and it changes what the reader does: an aged record nothing rests on
    /// may want deleting rather than refreshing.
    #[test]
    fn an_expired_entry_nothing_cites_says_so() {
        let c = expired(&[aged("a.md", Some("2024-01-15"), None, Some(954), Some(30))]);
        assert!(
            c.violations[0].detail.contains("Nothing cites it"),
            "{:?}",
            c.violations[0]
        );
    }

    /// The same stake, whichever silence it is. An entry whose bookkeeping is incomplete is
    /// one whose citing nodes cannot be told how old their evidence is either.
    #[test]
    fn an_undatable_entry_also_names_what_rests_on_it() {
        let c = expired_cited(
            &[aged("a.md", None, None, None, Some(30))],
            &[resting(&["gage/canyon-outlet.yml"])],
        );
        let d = &c.violations[0].detail;
        assert!(d.contains("1 node(s) rest on it"), "{d}");
        assert!(!d.contains("past"), "still not an expiry: {d}");
    }

    #[test]
    fn orphan_out_flags_a_node_with_no_links() {
        let nodes = vec![
            node("a.yml", "class: c\nlinks: []\n"),
            node("b.yml", "class: c\nlinks:\n  - target: a.yml\n"),
        ];
        let c = orphan_out(&nodes);
        assert_eq!(c.violations.len(), 1);
        assert_eq!(c.violations[0].node, "a.yml");
        assert_eq!(c.severity, Severity::Error);
    }

    #[test]
    fn unknown_class_is_silent_when_no_schema_layer_exists() {
        let nodes = vec![node("a.yml", "class: whatever\n")];
        assert!(unknown_class(&nodes, &HashSet::new()).passed());
    }

    #[test]
    fn unknown_class_fires_once_a_schema_layer_exists() {
        let nodes = vec![node("a.yml", "class: typo\n")];
        let defined: HashSet<String> = ["real".to_string()].into_iter().collect();
        assert_eq!(unknown_class(&nodes, &defined).violations.len(), 1);
    }

    #[test]
    fn orphan_in_sees_through_relative_traversal() {
        // `../reach/a.yml` from reach/ must match `reach/a.yml`, or every cross-class
        // edge would look like it points somewhere else.
        let a = Node {
            path: PathBuf::from("corpus/reach/a.yml"),
            rel: "corpus/reach/a.yml".into(),
            inst: serde_yaml::from_str("class: c\nlinks: []\n").unwrap(),
            text: "class: c\nlinks: []\n".to_string(),
        };
        let b = Node {
            path: PathBuf::from("corpus/other/b.yml"),
            rel: "corpus/other/b.yml".into(),
            inst: serde_yaml::from_str("class: c\nlinks:\n  - target: ../reach/a.yml\n").unwrap(),
            text: "class: c\nlinks:\n  - target: ../reach/a.yml\n".to_string(),
        };
        let c = orphan_in(&[a, b], &[]);
        let flagged: Vec<&str> = c.violations.iter().map(|v| v.node.as_str()).collect();
        assert_eq!(
            flagged,
            vec!["corpus/other/b.yml"],
            "only b is unpointed-at"
        );
    }

    /// A class declaring no `direction: in` edge says nothing points at its instances, so
    /// an orphan there is the ontology holding rather than the corpus failing.
    ///
    /// This was 17 of 35 findings in a derived repository — every `person` and every
    /// `boundary-case`. Both classes are richly out-linked; neither is ever a target. The
    /// signal was declared in the ontology the whole time and the check did not read it.
    #[test]
    fn instances_of_a_source_class_are_not_orphans() {
        let ont = |name: &str, dir: &str| Class {
            rel: format!(".yidam/corpus/{name}.ont.yml"),
            description: String::new(),
            name: name.into(),
            properties: vec![],
            edges: vec![edge("cited-by", "recording", dir)],
            edge_policy: EdgePolicy::default(),
            max_lines: None,
            implemented_by: None,
        };
        let node = |class: &str, file: &str| Node {
            path: PathBuf::from(format!("corpus/{class}/{file}.yml")),
            rel: format!("corpus/{class}/{file}.yml"),
            inst: serde_yaml::from_str("class: c\nlinks: []\n").unwrap(),
            text: "class: c\nlinks: []\n".to_string(),
        };

        // `person` declares only outbound edges; `recording` declares an inbound one.
        let classes = [ont("person", "out"), ont("recording", "in")];
        let c = orphan_in(
            &[node("person", "harris"), node("recording", "scum")],
            &classes,
        );

        let flagged: Vec<&str> = c.violations.iter().map(|v| v.node.as_str()).collect();
        assert_eq!(
            flagged,
            vec!["corpus/recording/scum.yml"],
            "a source-class instance is exempt; a citable one is not"
        );
    }

    /// An inbound relationship may be declared from the authoring end, and reading only a
    /// class's own list misses it (#336).
    ///
    /// This is the shape of `examples/streamflow` and it was entirely silent: `gage`
    /// declares `sources-from → concept, direction: out` — a statement that gages point at
    /// concepts — while `concept`'s own list held no `direction: in` entry, so `concept`
    /// derived as a class nothing points at and every orphaned concept was exempt.
    #[test]
    fn a_class_another_class_points_at_is_not_a_source_class() {
        let gage = Class {
            rel: ".yidam/corpus/gage.ont.yml".into(),
            description: String::new(),
            name: "gage".into(),
            properties: vec![],
            edges: vec![edge("sources-from", "concept", "out")],
            edge_policy: EdgePolicy::default(),
            max_lines: None,
            implemented_by: None,
        };
        let concept = Class {
            rel: ".yidam/corpus/concept.ont.yml".into(),
            description: String::new(),
            name: "concept".into(),
            properties: vec![],
            edges: vec![edge("refines", "concept", "out")],
            edge_policy: EdgePolicy::default(),
            max_lines: None,
            implemented_by: None,
        };
        let classes = [gage, concept];
        let sources = source_classes(&edge_views(&classes));

        assert!(
            !sources.contains("concept"),
            "`gage` declares it points at concepts, so concepts are pointed at"
        );
        assert!(
            sources.contains("gage"),
            "and nothing declares an edge at gages, so a gage is exempt"
        );
    }

    /// A self-edge says instances relate to each other, not that every instance is cited:
    /// any acyclic self-relation has an endpoint that is not.
    ///
    /// Measured on the worked example — counting self-edges makes the terminal reach of the
    /// `downstream-of` chain a finding, and every river has one.
    #[test]
    fn a_self_edge_does_not_make_a_class_pointed_at() {
        let reach = Class {
            rel: ".yidam/corpus/reach.ont.yml".into(),
            description: String::new(),
            name: "reach".into(),
            properties: vec![],
            edges: vec![edge("downstream-of", "reach", "out")],
            edge_policy: EdgePolicy::default(),
            max_lines: None,
            implemented_by: None,
        };
        let classes = [reach];
        assert!(source_classes(&edge_views(&classes)).contains("reach"));
    }

    /// A declaration that does not say which way it runs exempts neither end. Exempting on
    /// it would read an ambiguous declaration in the one direction that silences findings.
    #[test]
    fn a_directionless_declaration_exempts_neither_end() {
        let a = Class {
            rel: ".yidam/corpus/a.ont.yml".into(),
            description: String::new(),
            name: "a".into(),
            properties: vec![],
            edges: vec![ClassEdge {
                relationship: "relates-to".into(),
                target: "b".into(),
                direction: None,
            }],
            edge_policy: EdgePolicy::default(),
            max_lines: None,
            implemented_by: None,
        };
        let b = Class {
            rel: ".yidam/corpus/b.ont.yml".into(),
            description: String::new(),
            name: "b".into(),
            properties: vec![],
            edges: vec![edge("other", "c", "out")],
            edge_policy: EdgePolicy::default(),
            max_lines: None,
            implemented_by: None,
        };
        let classes = [a, b];
        assert!(source_classes(&edge_views(&classes)).is_empty());
    }

    /// Silence is not a declaration. A class that declares no edges has said nothing about
    /// its shape, and reading that as "nothing points at me" would exempt every instance in
    /// a corpus whose ontology is not filled in — switching the check off precisely where
    /// the graph is least trustworthy.
    #[test]
    fn a_class_declaring_no_edges_is_not_a_source_class() {
        let silent = Class {
            rel: ".yidam/corpus/concept.ont.yml".into(),
            description: String::new(),
            name: "concept".into(),
            properties: vec![],
            edges: vec![],
            edge_policy: EdgePolicy::default(),
            max_lines: None,
            implemented_by: None,
        };
        assert!(
            source_classes(&edge_views(std::slice::from_ref(&silent))).is_empty(),
            "a class that declared nothing was read as declaring nothing points at it"
        );

        let node = Node {
            path: PathBuf::from("corpus/concept/x.yml"),
            rel: "corpus/concept/x.yml".into(),
            inst: serde_yaml::from_str("class: c\nlinks: []\n").unwrap(),
            text: "class: c\nlinks: []\n".to_string(),
        };
        let c = orphan_in(&[node], &[silent]);
        assert_eq!(
            c.violations.len(),
            1,
            "an undeclared class is still checked"
        );
    }

    /// The reported shape: a tag with its citation folded inside the brackets.
    #[test]
    fn a_tag_with_its_citation_inside_is_a_near_miss() {
        let found = near_miss_tags("The estimate is [verified — Pearl 2009].");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0], (1, "[verified — Pearl 2009]".to_string()));
        // And it is counted as nothing, which is why it is worth reporting at all.
        assert_eq!(
            crate::claims::count_in_source("The estimate is [verified — Pearl 2009].").total(),
            0
        );
    }

    #[test]
    fn several_separators_all_read_as_a_folded_citation() {
        for inner in [
            "[open: pending review]",
            "[inference - weak]",
            "[verified, 2026]",
            "[open — nobody has looked]",
            "[verified/partial]",
        ] {
            assert_eq!(
                near_miss_tags(inner).len(),
                1,
                "{inner} should read as a near miss"
            );
        }
    }

    /// The forms that must stay silent. A warn nobody can act on is worse than no warn.
    #[test]
    fn the_tag_itself_and_ordinary_prose_are_not_reported() {
        for quiet in [
            "[verified]",
            "[open]",
            "[inference]",
            // A plain space is not a separator: this is prose, and link text.
            "See [open questions] below.",
            "See [open questions](../q/index.md) for more.",
            "A reference [open questions][q] link.",
            "[opening the valve] is not a tag",
            "[verification] is a different word",
            "nothing bracketed at all",
        ] {
            assert!(
                near_miss_tags(quiet).is_empty(),
                "{quiet} must not be reported"
            );
        }
    }

    /// A node that names the malformed shape in order to warn about it is not committing it.
    #[test]
    fn a_masked_mention_is_not_a_near_miss() {
        let node = Node {
            path: PathBuf::from("/tmp/x.yml"),
            rel: "x.yml".to_string(),
            inst: Default::default(),
            text: "description: Never write `[verified — source]`; the counter reads it as \
                   untagged.\n"
                .to_string(),
        };
        let c = claim_tag_malformed(std::slice::from_ref(&node));
        assert_eq!(c.violations.len(), 0, "{:?}", c.violations);
    }

    /// The violation names the line, because the point is to go and fix that line.
    #[test]
    fn a_near_miss_is_reported_against_its_line() {
        let node = Node {
            path: PathBuf::from("/tmp/x.yml"),
            rel: ".yidam/corpus/c/x.yml".to_string(),
            inst: Default::default(),
            text: "class: c\nlabel: X\ndescription: |\n  Settled [verified — Pearl 2009].\n"
                .to_string(),
        };
        let c = claim_tag_malformed(std::slice::from_ref(&node));
        assert_eq!(c.violations.len(), 1);
        assert!(
            c.violations[0].node.ends_with(":4"),
            "{}",
            c.violations[0].node
        );
    }

    /// A source at a known path, and a node in a directory that can reach it.
    fn catalog_source(slug: &str, obtained: bool) -> Source {
        Source {
            rel: format!(".yidam/catalog/{slug}.md"),
            path: PathBuf::from(format!("/repo/.yidam/catalog/{slug}.md")),
            obtained,
            used_by: vec![],
            locations: vec![],
            retrieved: None,
            ttl_days: None,
            artifacts: Vec::new(),
        }
    }

    // ── obtained-artifact records ────────────────────────────────────────────

    const GOOD: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

    fn with_artifacts(slug: &str, yaml: &str) -> Source {
        let mut s = catalog_source(slug, true);
        s.artifacts = serde_yaml::from_str(yaml).expect("test fixture parses");
        s
    }

    /// The upgrade story, and the reason both checks are written the way they are: a corpus
    /// that has not adopted `artifacts:` must see nothing new. Every entry in every existing
    /// corpus is this case on the day the field ships.
    #[test]
    fn an_entry_declaring_no_artifacts_produces_no_findings() {
        let sources = vec![catalog_source("pearl-2009", true)];
        assert!(catalog_artifact_malformed(&sources).violations.is_empty());
        assert!(catalog_artifact_unroutable(&sources, &[])
            .violations
            .is_empty());
    }

    #[test]
    fn a_well_formed_record_is_silent() {
        let sources = vec![with_artifacts(
            "pearl-2009",
            &format!("- sha256: {GOOD}\n  bytes: 4194304\n  media_type: application/pdf\n"),
        )];
        assert!(catalog_artifact_malformed(&sources).violations.is_empty());
    }

    #[test]
    fn a_record_without_a_digest_names_nothing_and_is_reported() {
        let sources = vec![with_artifacts("pearl-2009", "- bytes: 10\n")];
        let v = &catalog_artifact_malformed(&sources).violations;
        assert_eq!(v.len(), 1);
        assert!(v[0].detail.contains("no `sha256`"), "{:?}", v[0].detail);
    }

    /// Hex is case-insensitive and a content-addressed store is not, so an uppercase digest
    /// is a second name for one artifact. The message carries the fix.
    #[test]
    fn an_uppercase_digest_is_reported_with_the_lowercase_form() {
        let sources = vec![with_artifacts(
            "pearl-2009",
            &format!("- sha256: {}\n", GOOD.to_ascii_uppercase()),
        )];
        let v = &catalog_artifact_malformed(&sources).violations;
        assert_eq!(v.len(), 1);
        assert!(
            v[0].detail.contains(GOOD),
            "carries the fix: {:?}",
            v[0].detail
        );
    }

    #[test]
    fn a_digest_of_the_wrong_length_or_alphabet_is_reported() {
        let sources = vec![
            with_artifacts("short", "- sha256: abc123\n"),
            with_artifacts("nonhex", &format!("- sha256: {}\n", "z".repeat(64))),
        ];
        let v = &catalog_artifact_malformed(&sources).violations;
        assert_eq!(v.len(), 2);
        assert!(v[0].detail.contains("not 64"), "{:?}", v[0].detail);
        assert!(v[1].detail.contains("not hex"), "{:?}", v[1].detail);
    }

    /// A `from:` index naming no location means the record cites a provenance the entry does
    /// not carry — the one cross-field error worth having here.
    #[test]
    fn a_from_index_naming_no_location_is_reported() {
        let mut s = with_artifacts("pearl-2009", &format!("- sha256: {GOOD}\n  from: 2\n"));
        s.locations = vec![crate::parse::CatalogLocation {
            kind: Some("url".into()),
            value: Some("https://example.org".into()),
            description: None,
        }];
        let v = &catalog_artifact_malformed(&[s]).violations;
        assert_eq!(v.len(), 1);
        assert!(v[0].detail.contains("from: 2"), "{:?}", v[0].detail);
        assert!(v[0].detail.contains("declares 1"), "{:?}", v[0].detail);
    }

    #[test]
    fn a_from_url_is_not_an_index_and_is_never_out_of_range() {
        let s = with_artifacts(
            "pearl-2009",
            &format!("- sha256: {GOOD}\n  from: https://example.org/paper.pdf\n"),
        );
        assert!(catalog_artifact_malformed(&[s]).violations.is_empty());
    }

    /// Each malformed record is reported once, indexed, so an entry with several says which.
    #[test]
    fn several_problems_in_one_record_are_reported_as_one_violation() {
        let sources = vec![with_artifacts(
            "pearl-2009",
            "- sha256: abc\n  from: 5\n- bytes: 1\n",
        )];
        let v = &catalog_artifact_malformed(&sources).violations;
        assert_eq!(v.len(), 2, "one per record, not one per problem");
        assert!(v[0].detail.starts_with("artifacts[0]"), "{:?}", v[0].detail);
        assert!(v[1].detail.starts_with("artifacts[1]"), "{:?}", v[1].detail);
        assert!(
            v[0].detail.contains(';'),
            "both problems: {:?}",
            v[0].detail
        );
    }

    // ── routing ──────────────────────────────────────────────────────────────

    #[test]
    fn an_artifact_routed_to_a_declared_vault_is_silent() {
        let s = with_artifacts(
            "pearl-2009",
            &format!("- sha256: {GOOD}\n  vault: sources\n"),
        );
        let declared = vec!["default".to_string(), "sources".to_string()];
        assert!(catalog_artifact_unroutable(&[s], &declared)
            .violations
            .is_empty());
    }

    /// `none` is a route — the local cache and nowhere else — rather than an absence, so it
    /// resolves without any vault being declared at all.
    #[test]
    fn vault_none_is_a_route_and_needs_nothing_declared() {
        let s = with_artifacts("pearl-2009", &format!("- sha256: {GOOD}\n  vault: none\n"));
        assert!(catalog_artifact_unroutable(&[s], &[]).violations.is_empty());
    }

    #[test]
    fn an_artifact_naming_no_declared_vault_is_reported_with_what_is_declared() {
        let s = with_artifacts(
            "pearl-2009",
            &format!("- sha256: {GOOD}\n  vault: archive\n"),
        );
        let declared = vec!["default".to_string()];
        let v = &catalog_artifact_unroutable(&[s], &declared).violations;
        assert_eq!(v.len(), 1);
        assert!(
            v[0].detail.contains("`vault: archive`"),
            "{:?}",
            v[0].detail
        );
        assert!(
            v[0].detail.contains("`default`"),
            "names what is declared: {:?}",
            v[0].detail
        );
    }

    /// A corpus with records and no vault at all is a common half-configured state, and the
    /// message has to say that rather than listing an empty set.
    #[test]
    fn with_no_vault_declared_the_message_says_so_rather_than_listing_nothing() {
        let s = with_artifacts(
            "pearl-2009",
            &format!("- sha256: {GOOD}\n  vault: default\n"),
        );
        let v = &catalog_artifact_unroutable(&[s], &[]).violations;
        assert_eq!(v.len(), 1);
        assert!(
            v[0].detail.contains("declares no vault"),
            "{:?}",
            v[0].detail
        );
    }

    /// A record that does not name a vault has not been routed yet, which is not an error —
    /// the route falls back to what the vault itself declares it holds.
    #[test]
    fn a_record_naming_no_vault_is_not_unroutable() {
        let s = with_artifacts("pearl-2009", &format!("- sha256: {GOOD}\n"));
        assert!(catalog_artifact_unroutable(&[s], &[]).violations.is_empty());
    }

    fn corpus_node(name: &str, yaml: &str) -> Node {
        Node {
            path: PathBuf::from(format!("/repo/.yidam/corpus/concept/{name}.yml")),
            rel: format!(".yidam/corpus/concept/{name}.yml"),
            inst: serde_yaml::from_str(yaml).unwrap_or_default(),
            text: yaml.to_string(),
        }
    }

    /// The reported case. A catalog entry whose slug collides with a connector crate — the
    /// conventions recommend naming connectors after what they fetch — and a node that
    /// mentions the crate in prose and links nothing. It failed a build at Error severity on
    /// a node that cites nothing.
    #[test]
    fn naming_a_slug_in_prose_is_not_a_citation() {
        let sources = vec![catalog_source("nwis", false)];
        let node = corpus_node(
            "gauge-ingest",
            "class: concept\nlabel: Gauge ingest\ndescription: The `nwis` crate fetches the \
             series; nwis is also the source name.\nlinks:\n  - target: ../concept/other.yml\n",
        );
        let cites = citations(&sources, &[node]);
        assert_eq!(cites, vec![Vec::<String>::new()], "no link resolves to it");
        assert!(catalog_unobtained_but_cited(&sources, &cites).passed());
    }

    /// And a real citation still is one. Without this the fix is indistinguishable from
    /// deleting the check.
    #[test]
    fn a_markdown_link_that_resolves_to_the_entry_is_a_citation() {
        let sources = vec![catalog_source("pearl-2009", false)];
        let node = corpus_node(
            "confounding",
            "class: concept\nlabel: Confounding\ndescription: Draws on \
             [Pearl 2009](../../catalog/pearl-2009.md).\nlinks:\n  - target: ../concept/o.yml\n",
        );
        let cites = citations(&sources, &[node]);
        assert_eq!(
            cites[0],
            vec![".yidam/corpus/concept/confounding.yml".to_string()]
        );
        let c = catalog_unobtained_but_cited(&sources, &cites);
        assert_eq!(
            c.violations.len(),
            1,
            "an unfetched source, cited, still gates"
        );
    }

    /// A structured edge is a citation too — the corpus writes both forms.
    #[test]
    fn a_links_entry_pointing_at_the_entry_is_a_citation() {
        let sources = vec![catalog_source("pearl-2009", true)];
        let node = corpus_node(
            "confounding",
            "class: concept\nlabel: Confounding\ndescription: Draws on it.\nlinks:\n  \
             - target: ../../catalog/pearl-2009.md\n    relationship: cites\n",
        );
        let cites = citations(&sources, &[node]);
        assert_eq!(cites[0].len(), 1);
        assert!(catalog_uncited(&sources, &cites).passed());
    }

    /// A link shown as an example is not a citation, for the same reason it is not a link.
    #[test]
    fn a_citation_shown_in_code_is_not_a_citation() {
        let sources = vec![catalog_source("pearl-2009", false)];
        let node = corpus_node(
            "conventions",
            "class: concept\nlabel: How to cite\ndescription: Write \
             `[Pearl 2009](../../catalog/pearl-2009.md)` rather than a full \
             citation.\nlinks:\n  - target: ../concept/o.yml\n",
        );
        let cites = citations(&sources, &[node]);
        assert_eq!(cites, vec![Vec::<String>::new()]);
    }

    /// `..` in a target must not make two spellings of one file look like two files.
    #[test]
    fn a_citation_is_matched_through_a_normalized_path() {
        let sources = vec![catalog_source("pearl-2009", true)];
        let node = corpus_node(
            "confounding",
            "class: concept\nlabel: C\ndescription: See \
             [P](../../corpus/../catalog/pearl-2009.md).\nlinks:\n  - target: ../concept/o.yml\n",
        );
        assert_eq!(citations(&sources, &[node])[0].len(), 1);
    }

    #[test]
    fn an_unobtained_source_is_not_reported_as_uncited() {
        let s = vec![source("not-yet-fetched", false, &[])];
        assert!(catalog_uncited(&s, &[vec![]]).passed());
    }

    #[test]
    fn an_obtained_source_nothing_cites_is_reported() {
        let s = vec![source("fetched", true, &[])];
        assert_eq!(catalog_uncited(&s, &[vec![]]).violations.len(), 1);
    }

    #[test]
    fn citing_an_unfetched_source_is_an_error() {
        let s = vec![source("not-yet-fetched", false, &[])];
        let c = catalog_unobtained_but_cited(&s, &[vec!["corpus/x/y.yml".to_string()]]);
        assert_eq!(c.violations.len(), 1);
        assert_eq!(c.severity, Severity::Error);
    }

    #[test]
    fn used_by_drift_reports_both_directions() {
        let s = vec![source("src", true, &["../corpus/a/one.yml"])];
        let c = catalog_used_by_drift(&s, &[vec!["corpus/a/two.yml".to_string()]]);
        assert_eq!(c.violations.len(), 1);
        let d = &c.violations[0].detail;
        assert!(d.contains("one.yml"), "should name the stale claim: {d}");
        assert!(d.contains("two.yml"), "should name the omitted citer: {d}");
    }

    #[test]
    fn an_absent_used_by_list_is_not_drift() {
        let s = vec![source("src", true, &[])];
        assert!(catalog_used_by_drift(&s, &[vec!["corpus/a/x.yml".to_string()]]).passed());
    }

    #[test]
    fn a_url_location_must_be_a_url() {
        let mut s = source("src", true, &[]);
        s.locations = vec![crate::parse::CatalogLocation {
            kind: Some("url".into()),
            value: Some("see the reading room".into()),
            description: None,
        }];
        assert_eq!(catalog_location_malformed(&[s]).violations.len(), 1);
    }

    #[test]
    fn a_well_formed_location_passes() {
        let mut s = source("src", true, &[]);
        s.locations = vec![crate::parse::CatalogLocation {
            kind: Some("url".into()),
            value: Some("https://example.org/x".into()),
            description: None,
        }];
        assert!(catalog_location_malformed(&[s]).passed());
    }

    #[test]
    fn a_well_formed_regen_block_passes() {
        let text = "## Status\n\n<!-- REGEN: yidam status -->\n12 nodes\n<!-- /REGEN -->\n";
        assert!(malformed_regen_block(&[("f.md".into(), text.into())]).passed());
    }

    #[test]
    fn prose_with_no_markers_at_all_passes() {
        let text = "# A document\n\nNothing generated here.\n";
        assert!(malformed_regen_block(&[("f.md".into(), text.into())]).passed());
    }

    /// The shape a damaged file has in practice: two blocks, one missing close tag, and the
    /// scan reads it as a single long block that closed normally. The second section stops
    /// updating and nothing about the file looks wrong.
    #[test]
    fn a_block_that_swallows_the_next_one_is_reported_with_the_line_and_what_was_lost() {
        let text = "<!-- REGEN: yidam status -->\n12 nodes\n\n                    <!-- REGEN: yidam open-questions -->\n- q\n<!-- /REGEN -->\n";
        let c = malformed_regen_block(&[("README.md".into(), text.into())]);
        assert_eq!(c.violations.len(), 1, "{:#?}", c.violations);
        let d = &c.violations[0].detail;
        assert!(d.contains("line 1"), "{d}");
        assert!(d.contains("yidam status"), "{d}");
        assert!(d.contains("belonging to a block opened inside"), "{d}");
        assert!(
            d.contains("1 of which opens a marker that is now content"),
            "the finding must say a marker was lost, not just that lines were taken: {d}"
        );
        assert!(d.contains("will not write to it"), "{d}");
    }

    #[test]
    fn a_block_with_no_close_tag_at_the_end_of_a_file_is_reported() {
        let text = "## Status\n\n<!-- REGEN: yidam status -->\n12 nodes\n";
        let c = malformed_regen_block(&[("README.md".into(), text.into())]);
        assert_eq!(c.violations.len(), 1);
        assert!(
            c.violations[0].detail.contains("line 3"),
            "{:?}",
            c.violations[0].detail
        );
        assert!(
            c.violations[0].detail.contains("never arrives"),
            "{:?}",
            c.violations[0].detail
        );
    }

    #[test]
    fn an_open_tag_that_never_closes_is_reported_as_its_own_fault() {
        let text = "<!-- REGEN: yidam status\nFields: count.\n\nmore prose\n";
        let c = malformed_regen_block(&[("README.md".into(), text.into())]);
        assert_eq!(c.violations.len(), 1);
        assert!(
            c.violations[0]
                .detail
                .contains("opening comment never closes"),
            "{:?}",
            c.violations[0].detail
        );
    }

    /// The finding names the file it is in, so a report over a hundred of them is actionable.
    #[test]
    fn each_file_is_scanned_and_named() {
        let bad = "<!-- REGEN: a -->\nx\n";
        let good = "<!-- REGEN: b -->\ny\n<!-- /REGEN -->\n";
        let c = malformed_regen_block(&[
            ("good.md".into(), good.into()),
            ("bad.md".into(), bad.into()),
        ]);
        assert_eq!(c.violations.len(), 1);
        assert_eq!(c.violations[0].node, "bad.md");
    }

    #[test]
    fn a_well_formed_table_passes() {
        let text = "intro\n\n| A | B |\n|---|---|\n| 1 | 2 |\n| 3 | 4 |\n\nafter\n";
        assert!(malformed_table(&[("f.md".into(), text.into())]).passed());
    }

    #[test]
    fn a_ragged_body_row_is_flagged_with_its_line_number() {
        let text = "| A | B |\n|---|---|\n| 1 | 2 |\n| 3 |\n";
        let c = malformed_table(&[("f.md".into(), text.into())]);
        assert_eq!(c.violations.len(), 1);
        assert!(
            c.violations[0].detail.contains('4'),
            "should name line 4: {}",
            c.violations[0].detail
        );
    }

    #[test]
    fn a_header_and_delimiter_of_different_widths_are_flagged() {
        let text = "| A | B | C |\n|---|---|\n";
        let c = malformed_table(&[("f.md".into(), text.into())]);
        assert!(c.violations[0].detail.contains("paragraph of pipes"));
    }

    #[test]
    fn a_line_of_pipes_that_is_not_a_table_is_ignored() {
        // No delimiter row, so this is prose that happens to contain pipes.
        let text = "the grammar is `a | b | c` in this notation\n";
        assert!(malformed_table(&[("f.md".into(), text.into())]).passed());
    }

    // ── class-asserts-purpose ─────────────────────────────────────────────────

    fn class(rel: &str, description: &str) -> Class {
        Class {
            rel: rel.into(),
            description: description.into(),
            name: rel
                .rsplit('/')
                .next()
                .unwrap_or(rel)
                .replace(".ont.yml", ""),
            properties: vec![],
            // Declares an inbound edge, so instances are expected to be pointed at. The
            // source-class arm is exercised by `orphan_in`'s own tests.
            edges: vec![edge("cited-by", "concept", "in")],
            edge_policy: EdgePolicy::default(),
            max_lines: None,
            implemented_by: None,
        }
    }

    #[test]
    fn the_definition_that_motivated_the_check_is_caught() {
        // Verbatim from a derived repository's genesis. Every instance of this class
        // asserted a purpose by existing, and no instance-level tag could reach it.
        let c = class_asserts_purpose(&[class(
            ".yidam/corpus/maneuver.ont.yml",
            "A procedural mechanism deployed to obtain an outcome the ordinary path \
             would not yield.",
        )]);
        assert_eq!(c.violations.len(), 1);
        assert!(
            c.violations[0].detail.contains("deployed to"),
            "{}",
            c.violations[0].detail
        );
    }

    #[test]
    fn the_documentary_rewrite_of_that_definition_passes() {
        // The same class after it was redefined: an observable shape and its admission
        // conditions, with no claim about anyone's reason.
        assert!(class_asserts_purpose(&[class(
            ".yidam/corpus/maneuver.ont.yml",
            "A dated procedural sequence that departed from a baseline the record fixed \
             before the sequence and independently of it. The departure is the whole of \
             the class's content, and it is a comparison rather than a motive.",
        )])
        .passed());
    }

    #[test]
    fn commentary_outside_description_is_not_read() {
        // `analytic_note` is where a class discusses the purpose rule — often by quoting
        // the forbidden phrasing in order to forbid it. Reading it would flag the
        // discussion of the rule as a violation of the rule.
        let text = "class: maneuver\ndescription: A dated procedural sequence.\n\
                    analytic_note: |\n  \"designed to produce this outcome\" is an \
                    allegation, not a record.\n";
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("maneuver.ont.yml");
        std::fs::write(&p, text).unwrap();
        let loaded = load_classes(dir.path(), &[p], &crate::cmd::lint::Overlay::default());
        assert_eq!(loaded[0].description.trim(), "A dated procedural sequence.");
        assert!(class_asserts_purpose(&loaded).passed());
    }

    #[test]
    fn the_match_is_case_insensitive() {
        let c = class_asserts_purpose(&[class("x.ont.yml", "An instrument Designed To pass.")]);
        assert_eq!(c.violations.len(), 1);
    }

    #[test]
    fn the_finding_quotes_the_sentence_the_phrase_sits_in() {
        // Direction (3) of the report: the reader decides whether a hit is real, so the
        // clause is put in front of them rather than left in the file.
        let c = class_asserts_purpose(&[class(
            "x.ont.yml",
            "A dated procedural sequence. It was designed to obtain an outcome.",
        )]);
        assert_eq!(
            c.violations[0].detail,
            "`description` says \"designed to\" — a purpose, not a kind: \
             \"It was designed to obtain an outcome.\""
        );
    }

    #[test]
    fn the_disclaiming_sentence_is_quoted_with_its_negation() {
        // Verbatim shape from the report: the phrase occurs inside the clause refusing
        // to assert a purpose. Still a finding — but one a reader can resolve from the
        // message, because the words that defuse it are printed with it.
        let c = class_asserts_purpose(&[class(
            ".yidam/corpus/project.ont.yml",
            "It records what a body minuted, on what date, for how much. It does not \
             record that anyone intended an outcome, that a project was pursued in order \
             to obtain anything, or that a named person caused it to exist.",
        )]);
        assert_eq!(c.violations.len(), 1);
        let detail = &c.violations[0].detail;
        assert!(detail.contains("does not record"), "{detail}");
        assert!(detail.contains("in order to obtain anything"), "{detail}");
    }

    #[test]
    fn only_the_sentence_carrying_the_phrase_is_quoted() {
        let c = class_asserts_purpose(&[class(
            "x.ont.yml",
            "A minuted decision. An instrument intended to bind. Filed by date.",
        )]);
        assert!(
            c.violations[0]
                .detail
                .ends_with("\"An instrument intended to bind.\""),
            "{}",
            c.violations[0].detail
        );
    }

    #[test]
    fn a_phrase_broken_across_lines_is_still_found() {
        // A literal YAML block keeps its newlines. The old whole-description `contains`
        // missed a phrase straddling one; collapsing whitespace before matching does not.
        let c = class_asserts_purpose(&[class(
            "x.ont.yml",
            "A mechanism deployed\n   to obtain an outcome.",
        )]);
        assert_eq!(c.violations.len(), 1);
        assert!(
            c.violations[0].detail.ends_with(
                "\"A mechanism deployed to obtain an \
                 outcome.\""
            ),
            "{}",
            c.violations[0].detail
        );
    }

    #[test]
    fn a_long_sentence_is_elided_around_the_phrase() {
        let pad = "word ".repeat(60);
        let c = class_asserts_purpose(&[class(
            "x.ont.yml",
            &format!("{pad}not designed to bind {pad}"),
        )]);
        let detail = &c.violations[0].detail;
        let quoted = detail.split_once(": \"").unwrap().1.trim_end_matches('"');
        assert!(quoted.chars().count() <= SENTENCE_BUDGET + 2, "{quoted}");
        assert!(quoted.starts_with('…') && quoted.ends_with('…'), "{quoted}");
        // The negation next to the phrase is what makes the quote worth printing.
        assert!(quoted.contains("not designed to bind"), "{quoted}");
    }

    #[test]
    fn a_short_sentence_is_quoted_whole_and_unelided() {
        let c = class_asserts_purpose(&[class("x.ont.yml", "An instrument intended to bind.")]);
        assert!(!c.violations[0].detail.contains('…'));
    }

    #[test]
    fn a_class_with_no_description_passes() {
        assert!(class_asserts_purpose(&[class("x.ont.yml", "")]).passed());
    }

    #[test]
    fn the_check_warns_rather_than_gates() {
        // A heuristic over wording. Gating on it would make every false positive a
        // blocked commit, and the check would be switched off within a week.
        assert_eq!(class_asserts_purpose(&[]).severity, Severity::Warn);
    }

    // ── the class contract ────────────────────────────────────────────────────

    /// A class as the ontology writes it, parsed by the same deserializer `load_classes`
    /// uses and built by the same builder — so a test cannot pass against a shape the YAML
    /// could never produce, and cannot go on passing against a field the real builder has
    /// started reading and this one has not. It used to assemble the struct itself, which
    /// is the third answer to *what a class is* that [`Class::from_fields`] exists to
    /// prevent; it silently ignored `implemented_by:` the day the field was added.
    fn class_from(name: &str, yaml: &str) -> Class {
        Class::from_fields(
            format!(".yidam/corpus/{name}.ont.yml"),
            serde_yaml::from_str(yaml).unwrap(),
        )
    }

    // ── node-too-long ─────────────────────────────────────────────────────────

    /// A class of `name` whose instances may be `max` lines, or any length when `None`.
    fn capped(name: &str, max: Option<usize>) -> Class {
        Class {
            rel: format!(".yidam/corpus/{name}.ont.yml"),
            description: String::new(),
            name: name.into(),
            properties: vec![],
            edges: vec![],
            edge_policy: EdgePolicy::default(),
            max_lines: max,
            implemented_by: None,
        }
    }

    /// An instance of `class` that is `lines` lines long.
    fn sized(rel: &str, class: &str, lines: usize) -> Node {
        let mut yaml = format!("class: {class}\nlabel: L\ndescription: |\n");
        for _ in 3..lines {
            yaml.push_str("  filler\n");
        }
        assert_eq!(yaml.lines().count(), lines);
        node(rel, &yaml)
    }

    /// A class that declares no ceiling is not checked, however long its instances are.
    ///
    /// The whole design in one assertion. Porting the bootstrap rubric's 40 here would
    /// report against 335 of the 410 nodes in the five corpora this was measured on.
    #[test]
    fn a_class_that_declares_no_ceiling_is_not_checked() {
        let nodes = vec![sized("person/a.yml", "person", 200)];
        assert!(
            node_too_long(&nodes, &[capped("person", None)]).passed(),
            "a class that has not said its instances have a length cannot be contradicted"
        );
    }

    /// A class that declares one is checked against it, and the report names both numbers.
    #[test]
    fn a_declared_ceiling_is_enforced_and_says_by_how_much() {
        let nodes = vec![
            sized("person/long.yml", "person", 52),
            sized("person/short.yml", "person", 12),
        ];
        let c = node_too_long(&nodes, &[capped("person", Some(40))]);
        assert_eq!(c.violations.len(), 1, "only the long one is over");
        assert_eq!(c.violations[0].node, "person/long.yml");
        assert!(
            c.violations[0].detail.contains("52 lines")
                && c.violations[0].detail.contains("max_lines: 40"),
            "the finding must carry both numbers, not just a verdict: {}",
            c.violations[0].detail
        );
        assert_eq!(
            c.severity,
            Severity::Warn,
            "length is editorial; the baseline ratchet separates inherited from new"
        );
    }

    /// Exactly at the ceiling is not over it.
    #[test]
    fn the_ceiling_is_inclusive() {
        let nodes = vec![sized("person/a.yml", "person", 40)];
        assert!(node_too_long(&nodes, &[capped("person", Some(40))]).passed());
    }

    /// One class's ceiling says nothing about another's.
    #[test]
    fn a_ceiling_binds_only_the_class_that_declared_it() {
        let nodes = vec![sized("statute/a.yml", "statute", 60)];
        let classes = vec![capped("person", Some(10)), capped("statute", None)];
        assert!(
            node_too_long(&nodes, &classes).passed(),
            "statute declared nothing; person's number is not corpus-wide"
        );
    }

    // ── unimplemented-class ───────────────────────────────────────────────────

    /// A class of `name` that says `impl_by` implements it, or says nothing.
    fn implemented(name: &str, impl_by: Option<&str>) -> Class {
        Class {
            rel: format!(".yidam/corpus/{name}.ont.yml"),
            description: String::new(),
            name: name.into(),
            properties: vec![],
            edges: vec![],
            edge_policy: EdgePolicy::default(),
            max_lines: None,
            implemented_by: impl_by.map(str::to_string),
        }
    }

    fn tree(files: &[(&str, &str)]) -> TypeIndex {
        TypeIndex::build(files.iter().map(|(r, t)| (*r, *t)))
    }

    /// The measured default, and the one this check lives or dies by. Across twelve derived
    /// corpora 129 of 157 classes have no type of their name; if silence were a contract,
    /// every one of them would be a finding on adoption.
    #[test]
    fn a_class_that_declares_nothing_is_not_checked() {
        let types = tree(&[("crates/a/src/lib.rs", "pub struct Unrelated;\n")]);
        assert!(
            unimplemented_class(&[implemented("intervention", None)], &types).passed(),
            "an ontology is not a specification of the code"
        );
    }

    #[test]
    fn a_declared_implementation_the_tree_defines_is_silent() {
        let types = tree(&[("crates/a/src/model.rs", "pub struct Intervention {\n")]);
        assert!(
            unimplemented_class(&[implemented("intervention", Some("Intervention"))], &types)
                .passed()
        );
    }

    #[test]
    fn a_declared_implementation_the_tree_lacks_gates() {
        let types = tree(&[("crates/a/src/model.rs", "pub struct Treatment;\n")]);
        let c = unimplemented_class(&[implemented("intervention", Some("Intervention"))], &types);
        assert_eq!(c.violations.len(), 1);
        assert_eq!(
            c.violations[0].node, ".yidam/corpus/intervention.ont.yml",
            "the subject is the class, not a file of code — the ontology is what is wrong"
        );
        assert!(
            c.violations[0].detail.contains("Intervention"),
            "the finding must name the type that is missing: {}",
            c.violations[0].detail
        );
        assert_eq!(
            c.severity,
            Severity::Error,
            "the class stated a fact about the tree and the tree contradicts it, which is \
             what this check's siblings gate on"
        );
    }

    /// The reason the finding moved off `check-diff`. Asked of a diff, a rename is a removal
    /// plus an addition and correlating them is the hard part; asked of a tree, the class is
    /// simply found under the name it now declares.
    #[test]
    fn a_rename_the_class_followed_is_invisible() {
        let types = tree(&[("crates/a/src/model.rs", "pub struct Intervention;\n")]);
        assert!(
            unimplemented_class(&[implemented("intervention", Some("Intervention"))], &types)
                .passed(),
            "the tree defines it under the new name; there is no diff to be confused by"
        );
    }

    /// An `enum` implements a class as well as a `struct` does — the same two keywords
    /// `check-diff` reads, which is the point of sharing the reader.
    #[test]
    fn an_enum_implements_a_class() {
        let types = tree(&[("crates/a/src/lib.rs", "enum Chamber { House, Senate }\n")]);
        assert!(unimplemented_class(&[implemented("chamber", Some("Chamber"))], &types).passed());
    }

    /// A trait names a capability and an alias names a spelling, exactly as `check-diff`
    /// says. A class pointing at one has not been implemented.
    #[test]
    fn a_trait_is_not_an_implementation() {
        let types = tree(&[("crates/a/src/lib.rs", "pub trait Connector {}\n")]);
        assert!(
            !unimplemented_class(&[implemented("connector", Some("Connector"))], &types).passed()
        );
    }

    /// Exact, on the name the class wrote. `HTTPServer` and `HttpServer` kebab to one name
    /// and are two types, so a round trip would resolve a class against the wrong one.
    #[test]
    fn matching_is_exact_and_not_a_kebab_round_trip() {
        let types = tree(&[("crates/a/src/lib.rs", "pub struct HttpServer;\n")]);
        assert!(
            !unimplemented_class(&[implemented("http-server", Some("HTTPServer"))], &types)
                .passed()
        );
    }

    /// A field somebody started and did not finish must not gate a build against a name
    /// nothing can match.
    #[test]
    fn an_empty_declaration_is_read_as_no_declaration() {
        let fields: ClassFields = serde_yaml::from_str("implemented_by: '   '").unwrap();
        let class = Class::from_fields(".yidam/corpus/a.ont.yml", fields);
        assert_eq!(class.implemented_by, None);
        assert!(unimplemented_class(&[class], &tree(&[])).passed());
    }

    #[test]
    fn a_declaration_is_read_off_the_class_file_and_trimmed() {
        let fields: ClassFields =
            serde_yaml::from_str("implemented_by: '  Intervention  '").unwrap();
        assert_eq!(
            Class::from_fields(".yidam/corpus/a.ont.yml", fields).implemented_by,
            Some("Intervention".to_string())
        );
    }

    const GAGE: &str = "\
properties:
  - name: parameter
    type: string
  - name: claim_tag
    type: claim
edges:
  - relationship: sources-from
    target: concept
    direction: out
";

    fn gage_corpus(gage_yaml: &str) -> (Vec<Node>, Vec<Class>) {
        let nodes = vec![
            node(".yidam/corpus/gage/outlet.yml", gage_yaml),
            node(
                ".yidam/corpus/concept/hydropeaking.yml",
                "class: concept\nlinks: []\n",
            ),
        ];
        let classes = vec![
            class_from("gage", GAGE),
            class_from("concept", "properties: []\nedges: []\n"),
        ];
        (nodes, classes)
    }

    /// The severity is a function of the declaration (#301).
    ///
    /// Two omissions, one check, two severities. `parameter` is declared `required: true`,
    /// so leaving it out contradicts a contract the class wrote and gates like this check's
    /// four siblings do. `claim_tag` says nothing about being required, so leaving it out
    /// contradicts nothing and is reported — a node that makes no tagged claim is a real
    /// state, and the parity fixture depends on it staying one.
    #[test]
    fn missing_property_gates_on_required_and_reports_on_the_rest() {
        let declaring = "properties:\n  - name: parameter\n    type: string\n    required: \
                         true\n  - name: claim_tag\n    type: claim\nedges: []\n";
        let nodes = vec![node(
            ".yidam/corpus/gage/outlet.yml",
            "class: gage\nlinks: []\n",
        )];
        let classes = vec![class_from("gage", declaring)];

        let c = missing_property(&nodes, &classes);
        assert_eq!(c.violations.len(), 2, "{c:#?}");
        assert_eq!(
            c.severity,
            Severity::Warn,
            "the check's own level stays Warn; only the declared-required finding rises"
        );

        let by_name = |want: &str| {
            c.violations
                .iter()
                .find(|v| v.detail.contains(want))
                .unwrap_or_else(|| panic!("no finding mentions {want}: {c:#?}"))
        };
        assert_eq!(c.severity_of(by_name("parameter")), Severity::Error);
        assert_eq!(c.severity_of(by_name("claim_tag")), Severity::Warn);

        // The block heading must not read WARN above a finding that fails the build.
        assert_eq!(c.effective_severity(), Severity::Error);
    }

    /// Absent means false, and it has to be read off the file rather than assumed.
    ///
    /// Every corpus predating the field was written where the question could not be asked,
    /// so a parser defaulting the other way would gate every class in every derived
    /// repository on a declaration nobody made.
    #[test]
    fn a_property_that_says_nothing_about_being_required_does_not_gate() {
        let nodes = vec![node(
            ".yidam/corpus/gage/outlet.yml",
            "class: gage\nlinks: []\n",
        )];
        let classes = vec![class_from(
            "gage",
            "properties:\n  - name: parameter\n    type: string\nedges: []\n",
        )];

        let c = missing_property(&nodes, &classes);
        assert_eq!(c.violations.len(), 1);
        assert_eq!(c.severity_of(&c.violations[0]), Severity::Warn);
        assert!(
            !c.violations[0].detail.contains("required"),
            "a finding about a property nobody required must not mention requirement: {}",
            c.violations[0].detail
        );
    }

    /// The instance carries a field the class never named. It reads as data and is
    /// invisible to everything that reaches the corpus through the ontology.
    #[test]
    fn a_property_the_class_never_declared_is_reported() {
        let (nodes, classes) = gage_corpus(
            "class: gage\nproperties:\n  parameter: \"00060\"\n  claim_tag: verified\n  vendor: acme\nlinks: []\n",
        );
        let c = undeclared_property(&nodes, &classes, &NONE);
        assert_eq!(c.violations.len(), 1);
        assert!(c.violations[0].detail.contains("`vendor`"), "{c:#?}");
        // …and the declared ones are not reported as missing.
        assert!(missing_property(&nodes, &classes).passed());
    }

    #[test]
    fn a_declared_property_the_instance_omits_is_reported() {
        let (nodes, classes) =
            gage_corpus("class: gage\nproperties:\n  parameter: \"00060\"\nlinks: []\n");
        let c = missing_property(&nodes, &classes);
        assert_eq!(c.violations.len(), 1);
        assert!(c.violations[0].detail.contains("`claim_tag`"), "{c:#?}");
    }

    /// An instance with no `properties:` at all is missing every one of them — the case a
    /// corpus lands in when the class grew a field and the instances did not.
    #[test]
    fn an_instance_with_no_properties_block_is_missing_all_of_them() {
        let (nodes, classes) = gage_corpus("class: gage\nlinks: []\n");
        assert_eq!(missing_property(&nodes, &classes).violations.len(), 2);
    }

    /// The severity split is load-bearing and easy to lose in a refactor. Three checks
    /// report the ontology being contradicted and gate unconditionally; `missing-property`
    /// reports an omission, which contradicts nothing the declaration actually says;
    /// `unlicensed-edge` gates only where the class declared that it should, which is
    /// pinned by [`the_three_edge_policies_are_three_different_answers`].
    #[test]
    fn only_the_omission_check_declines_to_gate() {
        let (nodes, classes) = gage_corpus("class: gage\nlinks: []\n");
        assert_eq!(
            missing_property(&nodes, &classes).severity,
            Severity::Warn,
            "the property declaration has no `required` field to gate on"
        );
        for c in [
            undeclared_property(&nodes, &classes, &NONE),
            property_type(&nodes, &classes, &NONE),
            edge_target_class(&nodes, &classes),
        ] {
            assert_eq!(c.severity, Severity::Error, "{} must gate", c.id);
        }
        assert_eq!(
            unlicensed_edge(&nodes, &classes).severity,
            Severity::Warn,
            "an undeclared relationship gates only on a class that closed its vocabulary"
        );
    }

    /// **The collision this field exists to resolve.** One corpus, one undeclared
    /// relationship, three ontologies that differ in nothing but `edge_policy:` — and three
    /// different answers, because the three say different things.
    ///
    /// Reported against a real derived corpus, where reading a non-empty `edges:` as closed
    /// produced 210 errors across 107 distinct relationships on 18 classes that every one
    /// declare `characteristic`, document it, and enforce it in their own schema.
    #[test]
    fn the_three_edge_policies_are_three_different_answers() {
        let instance = "class: gage\nproperties:\n  parameter: \"00060\"\n  claim_tag: open\nlinks:\n  - target: ../concept/hydropeaking.yml\n    relationship: bears-on\n";
        let with = |policy: &str| {
            let (nodes, _) = gage_corpus(instance);
            let classes = vec![
                class_from("gage", &format!("{GAGE}{policy}")),
                class_from("concept", "properties: []\nedges: []\n"),
            ];
            let c = unlicensed_edge(&nodes, &classes);
            let gated = c.violations.iter().filter(|v| c.gates(v)).count();
            (c.violations.len(), gated)
        };

        assert_eq!(
            with("edge_policy: characteristic\n"),
            (0, 0),
            "`characteristic` says an undeclared relationship is a deliberate coinage — \
             the corpus answered the question and there is nothing to report"
        );
        assert_eq!(
            with("edge_policy: exhaustive\n"),
            (1, 1),
            "`exhaustive` closes the vocabulary, so the class asked for this gate"
        );
        assert_eq!(
            with(""),
            (1, 0),
            "silence names the relationships that exist without claiming the list is \
             complete — worth seeing, not a contract to gate on"
        );
    }

    /// `characteristic` licenses an *undeclared* relationship. It says nothing about where a
    /// **declared** one may land, and silencing that too would give up the one finding no
    /// other check can produce — two of which were real in the corpus that reported this.
    #[test]
    fn a_characteristic_class_still_has_its_declared_targets_checked() {
        let mut nodes = vec![
            node(
                ".yidam/corpus/gage/outlet.yml",
                "class: gage\nlinks:\n  - target: ../gage/bridge.yml\n    relationship: sources-from\n",
            ),
            node(".yidam/corpus/gage/bridge.yml", "class: gage\nlinks: []\n"),
        ];
        nodes.sort_by(|a, b| a.rel.cmp(&b.rel));
        let classes = vec![class_from(
            "gage",
            &format!("{GAGE}edge_policy: characteristic\n"),
        )];
        let c = edge_target_class(&nodes, &classes);
        assert_eq!(c.violations.len(), 1, "{c:#?}");
        assert!(c.gates(&c.violations[0]), "a false edge is still false");
    }

    /// A policy on an empty `edges:` bounds nothing. Silence about the relationships
    /// outranks a statement about how to read them, or `edge_policy: exhaustive` on a class
    /// that has named none would forbid every edge its instances draw.
    #[test]
    fn a_policy_without_edges_still_licenses_everything() {
        let nodes = vec![
            node(
                ".yidam/corpus/reach/alpha.yml",
                "class: reach\nlinks:\n  - target: ../reach/beta.yml\n    relationship: whatever\n",
            ),
            node(".yidam/corpus/reach/beta.yml", "class: reach\nlinks: []\n"),
        ];
        let classes = vec![class_from("reach", "edges: []\nedge_policy: exhaustive\n")];
        assert!(unlicensed_edge(&nodes, &classes).passed());
    }

    /// A policy the corpus coined is not one this check knows how to honour, and treating it
    /// as a closed vocabulary would gate on a misspelling. `yidam schema` publishes the enum,
    /// so the typo is underlined where it can be fixed as it is typed.
    #[test]
    fn an_unrecognized_policy_reads_as_unstated() {
        let (nodes, _) = gage_corpus(
            "class: gage\nlinks:\n  - target: ../concept/hydropeaking.yml\n    relationship: bears-on\n",
        );
        let classes = vec![
            class_from("gage", &format!("{GAGE}edge_policy: closed\n")),
            class_from("concept", "properties: []\nedges: []\n"),
        ];
        let c = unlicensed_edge(&nodes, &classes);
        assert_eq!(c.violations.len(), 1);
        assert!(
            !c.gates(&c.violations[0]),
            "a typo must not close a vocabulary"
        );
    }

    /// **Silence is not a contract.** A class declaring neither properties nor edges has
    /// said nothing, and reading that as "and therefore none are permitted" would flood
    /// every corpus whose ontology is not filled in.
    #[test]
    fn a_class_that_declares_nothing_checks_nothing() {
        let nodes = vec![node(
            ".yidam/corpus/reach/alpha.yml",
            "class: reach\nproperties:\n  anything: at all\nlinks:\n  - target: ../reach/beta.yml\n    relationship: whatever\n",
        ), node(".yidam/corpus/reach/beta.yml", "class: reach\nlinks: []\n")];
        let silent = vec![class_from("reach", "{}")];
        assert!(undeclared_property(&nodes, &silent, &NONE).passed());
        assert!(missing_property(&nodes, &silent).passed());
        assert!(property_type(&nodes, &silent, &NONE).passed());
        assert!(unlicensed_edge(&nodes, &silent).passed());
        assert!(edge_target_class(&nodes, &silent).passed());
    }

    /// Each half is read on its own: a class that declares properties and no edges has
    /// said nothing about edges, and must not have its links licensed against an empty
    /// list. This is the same trap `source_classes` documents, one field over.
    #[test]
    fn declaring_properties_says_nothing_about_edges() {
        let nodes = vec![
            node(
                ".yidam/corpus/reach/alpha.yml",
                "class: reach\nproperties:\n  datum: NAVD88\nlinks:\n  - target: ../reach/beta.yml\n    relationship: whatever\n",
            ),
            node(".yidam/corpus/reach/beta.yml", "class: reach\nlinks: []\n"),
        ];
        let classes = vec![class_from(
            "reach",
            "properties:\n  - name: datum\n    type: string\n",
        )];
        assert!(unlicensed_edge(&nodes, &classes).passed());
        assert!(edge_target_class(&nodes, &classes).passed());
        assert!(undeclared_property(&nodes, &classes, &NONE).passed());
    }

    /// **A universal property is not an undeclared one.** Both shapes, against the corpus
    /// that reported them: `seeded_because` is apparatus carried by six different classes,
    /// and `fy2024_profile` is a fiscal year's figures pasted onto the node they describe.
    /// Declaring the first per class would be sixteen copies of one decision; declaring the
    /// second by name would mean editing an ontology every July.
    #[test]
    fn a_universal_property_is_not_reported_as_undeclared() {
        let universal = crate::universal::Universal::parse(
            "properties:\n  - name: seeded_because\n    type: text\n  - pattern: '^fy\\d{4}_[a-z0-9_]+$'\n    type: text\n",
        );
        let (nodes, classes) = gage_corpus(
            "class: gage\nproperties:\n  parameter: \"00060\"\n  claim_tag: open\n  seeded_because: the anchor case\n  fy2024_profile: the figures\nlinks: []\n",
        );
        assert!(undeclared_property(&nodes, &classes, &universal).passed());
        // Reverting the declaration must bring both back, or this fixture pins nothing.
        assert_eq!(
            undeclared_property(&nodes, &classes, &NONE)
                .violations
                .len(),
            2
        );
        // …and they are not then reported as *missing* either: universal means any class
        // MAY carry it, and `missing-property` reads only what the class itself declared.
        assert!(missing_property(&nodes, &classes)
            .violations
            .iter()
            .all(|v| !v.detail.contains("seeded_because")));
    }

    /// Universal does not mean untyped. A `claim` field is counted by exact token wherever
    /// it was declared, so a universal one holding prose is the same silent undercount.
    #[test]
    fn a_universal_property_is_still_type_checked() {
        let universal =
            crate::universal::Universal::parse("properties:\n  - name: verdict\n    type: claim\n");
        let (nodes, classes) = gage_corpus(
            "class: gage\nproperties:\n  parameter: \"00060\"\n  verdict: mostly sure\nlinks: []\n",
        );
        let c = property_type(&nodes, &classes, &universal);
        assert_eq!(c.violations.len(), 1, "{c:#?}");
        assert!(c.violations[0].detail.contains("not an evidence tag"));
    }

    /// The class is the more specific statement about its own instances and wins. Otherwise
    /// a corpus could not narrow a universal property for one class without renaming it.
    #[test]
    fn a_class_declaring_the_same_property_outranks_the_universal_one() {
        let universal = crate::universal::Universal::parse(
            "properties:\n  - name: parameter\n    type: claim\n",
        );
        let (nodes, classes) = gage_corpus(
            "class: gage\nproperties:\n  parameter: \"00060\"\n  claim_tag: open\nlinks: []\n",
        );
        // `gage` declares `parameter` as `string`; "00060" satisfies that and not `claim`.
        assert!(property_type(&nodes, &classes, &universal).passed());
    }

    /// The type that pays for the check on its own: `claims.rs` matches the three tokens
    /// exactly, so prose in a `claim` field is counted as no claim at all.
    #[test]
    fn a_claim_field_holding_prose_is_reported() {
        let (nodes, classes) = gage_corpus(
            "class: gage\nproperties:\n  parameter: \"00060\"\n  claim_tag: mostly sure\nlinks: []\n",
        );
        let c = property_type(&nodes, &classes, &NONE);
        assert_eq!(c.violations.len(), 1);
        assert!(
            c.violations[0].detail.contains("not an evidence tag"),
            "{c:#?}"
        );
    }

    /// Both spellings the counter accepts pass here, including the flow sequence an
    /// unquoted `claim_tag: [open]` actually parses to. A check that disagreed with
    /// `count_structural` about the same bytes would be worse than no check.
    #[test]
    fn every_spelling_the_counter_accepts_is_accepted_here() {
        for value in ["verified", "\"[open]\"", "[open]", "[verified, inference]"] {
            let (nodes, classes) = gage_corpus(&format!(
                "class: gage\nproperties:\n  parameter: \"00060\"\n  claim_tag: {value}\nlinks: []\n"
            ));
            assert!(
                property_type(&nodes, &classes, &NONE).passed(),
                "claim_tag: {value} should be accepted"
            );
        }
    }

    /// The classic YAML surprise, one level down: a `string` field holding an unquoted
    /// number is a number by the time anything reads it.
    #[test]
    fn a_string_field_holding_a_bare_number_is_reported() {
        let (nodes, classes) = gage_corpus(
            "class: gage\nproperties:\n  parameter: 60\n  claim_tag: open\nlinks: []\n",
        );
        let c = property_type(&nodes, &classes, &NONE);
        assert_eq!(c.violations.len(), 1);
        assert!(c.violations[0].detail.contains("quote it"), "{c:#?}");
    }

    /// A type the corpus coined for itself is left alone. A check that failed on every
    /// type it had not heard of would make coining one impossible.
    #[test]
    fn a_type_this_check_does_not_know_is_left_alone() {
        let nodes = vec![node(
            ".yidam/corpus/reach/alpha.yml",
            "class: reach\nproperties:\n  extent:\n    from: 0\n    to: 9\nlinks: []\n",
        )];
        let classes = vec![class_from(
            "reach",
            "properties:\n  - name: extent\n    type: river-mile-range\n",
        )];
        assert!(property_type(&nodes, &classes, &NONE).passed());
    }

    #[test]
    fn a_date_field_holding_prose_is_reported() {
        let nodes = vec![node(
            ".yidam/corpus/reach/alpha.yml",
            "class: reach\nproperties:\n  surveyed: last spring\nlinks: []\n",
        )];
        let classes = vec![class_from(
            "reach",
            "properties:\n  - name: surveyed\n    type: date\n",
        )];
        assert_eq!(property_type(&nodes, &classes, &NONE).violations.len(), 1);

        let ok = vec![node(
            ".yidam/corpus/reach/alpha.yml",
            "class: reach\nproperties:\n  surveyed: 2024-04-01\nlinks: []\n",
        )];
        assert!(property_type(&ok, &classes, &NONE).passed());
    }

    /// **A date known to the year is a date.** Demanding a day where the corpus knows only
    /// the year does not make the record more accurate, it makes it invented — and this
    /// check's subject is prose in a date field, which `1985` is not. Requiring a full day
    /// put 72 errors on a derived corpus recording when bands formed and definitions were
    /// stated, every one of them the corpus being right.
    #[test]
    fn a_date_is_accepted_at_the_precision_it_is_known_to() {
        let classes = vec![class_from(
            "reach",
            "properties:\n  - name: surveyed\n    type: date\n",
        )];
        let surveyed = |v: &str| {
            vec![node(
                ".yidam/corpus/reach/alpha.yml",
                &format!("class: reach\nproperties:\n  surveyed: {v}\nlinks: []\n"),
            )]
        };
        for v in ["\"1985\"", "\"1991-06\"", "2024-04-01"] {
            assert!(
                property_type(&surveyed(v), &classes, &NONE).passed(),
                "{v} is a date at the precision it is known to"
            );
        }
        // Reduced precision is not a licence for a partial one. Each of these is a field
        // that was filled in wrong rather than filled in coarsely.
        for v in [
            "\"198\"",
            "\"1985-\"",
            "\"1985-6\"",
            "\"1985-06-\"",
            "last spring",
        ] {
            assert_eq!(
                property_type(&surveyed(v), &classes, &NONE)
                    .violations
                    .len(),
                1,
                "{v} is not a date"
            );
        }
    }

    #[test]
    fn a_relationship_the_class_does_not_declare_is_reported() {
        let (nodes, classes) = gage_corpus(
            "class: gage\nproperties:\n  parameter: \"00060\"\n  claim_tag: open\nlinks:\n  - target: ../concept/hydropeaking.yml\n    relationship: sourcs-from\n",
        );
        let c = unlicensed_edge(&nodes, &classes);
        assert_eq!(c.violations.len(), 1);
        assert!(c.violations[0].detail.contains("`sourcs-from`"), "{c:#?}");
    }

    /// A link to the class file or into the catalog is a citation, not a relationship —
    /// the bootstrap skill says so, and requires the `instance-of` link that no class
    /// declares. Licensing those would report every instance in every corpus.
    #[test]
    fn structural_links_are_not_licensed_edges() {
        let (nodes, classes) = gage_corpus(
            "class: gage\nproperties:\n  parameter: \"00060\"\n  claim_tag: open\nlinks:\n  - target: ../gage.ont.yml\n    relationship: instance-of\n  - target: ../../catalog/usgs.md\n    relationship: sourced-from\n",
        );
        assert!(unlicensed_edge(&nodes, &classes).passed());
        assert!(edge_target_class(&nodes, &classes).passed());
    }

    /// A broken edge is `dangling-edge`'s finding, reported once. It resolves to no node,
    /// so it is not licensed here and is not reported twice.
    #[test]
    fn a_broken_edge_is_not_also_reported_as_unlicensed() {
        let (nodes, classes) = gage_corpus(
            "class: gage\nproperties:\n  parameter: \"00060\"\n  claim_tag: open\nlinks:\n  - target: ../concept/gone.yml\n    relationship: sourcs-from\n",
        );
        assert!(unlicensed_edge(&nodes, &classes).passed());
        assert_eq!(dangling_edge(&nodes).violations.len(), 1);
    }

    /// The finding no existing check could produce. `dangling-edge` catches an edge to
    /// nothing; an edge to the wrong thing resolves, traverses, and exports, and is false.
    #[test]
    fn an_edge_to_a_node_of_the_wrong_class_is_reported() {
        let mut nodes = vec![
            node(
                ".yidam/corpus/gage/outlet.yml",
                "class: gage\nproperties:\n  parameter: \"00060\"\n  claim_tag: open\nlinks:\n  - target: ../gage/bridge.yml\n    relationship: sources-from\n",
            ),
            node(".yidam/corpus/gage/bridge.yml", "class: gage\nlinks: []\n"),
        ];
        nodes.sort_by(|a, b| a.rel.cmp(&b.rel));
        let classes = vec![class_from("gage", GAGE)];
        let c = edge_target_class(&nodes, &classes);
        assert_eq!(c.violations.len(), 1, "{c:#?}");
        assert!(c.violations[0].detail.contains("a gage"), "{c:#?}");
        // …and the relationship itself is declared, so it is not also unlicensed.
        assert!(unlicensed_edge(&nodes, &classes).passed());
    }

    /// A declaration with no `target` has named no class, so it licenses any of them.
    #[test]
    fn a_declaration_naming_no_target_licenses_anything() {
        let nodes = vec![
            node(
                ".yidam/corpus/reach/alpha.yml",
                "class: reach\nlinks:\n  - target: ../gage/x.yml\n    relationship: relates-to\n",
            ),
            node(".yidam/corpus/gage/x.yml", "class: gage\nlinks: []\n"),
        ];
        let classes = vec![class_from(
            "reach",
            "edges:\n  - relationship: relates-to\n    direction: out\n",
        )];
        assert!(edge_target_class(&nodes, &classes).passed());
    }
}

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
    Check::new(
        "resolution-annotation-malformed",
        "Resolution annotation is malformed or in the wrong section",
        Severity::Error,
        "Both halves are structural rather than a judgement about wording, which is why \
         this gates where its sibling does not. An annotation with no date cannot be read \
         against the record it extends — the whole point is that the record speaks for its \
         own date and the annotation for a later one. An annotation under `What was \
         resolved` or `What changed` edits history instead of extending it, and those \
         sections are the ones a reader trusts to describe a settled past.",
        violations,
    )
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
    Check::new(
        "resolution-annotation-decides",
        "Resolution annotation announces an outcome",
        Severity::Warn,
        "An annotation may say a question was proposed, retrieved, measured or superseded \
         in fact; it may not say it was settled. Closing an open item is synthesis, and \
         Article V puts synthesis in resolution events — so an annotation that decides \
         something is a resolution performed by one elector, in one commit, in a file the \
         protocol never routes through a resolution. Warn rather than Error because this \
         is a heuristic over wording: it cannot see an outcome announced in words that are \
         not on the list, and gating on it would make every false positive a blocked \
         commit and the check would be switched off within a week.",
        violations,
    )
}

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
    Check::new(
        "resolution-elector-unregistered",
        "Resolution names a ma/* branch no elector row registers",
        Severity::Error,
        "A resolution's authority rests on the seats it names, so a name the registry does \
         not carry is the one part of the record that cannot be read charitably — either the \
         seat was never registered, or it was removed and the records resting on it were \
         left behind. This gates where its sibling warns because it is set membership \
         between two committed files rather than a judgement about a missing field: there is \
         no state of the world in which the answer is arguable.",
        violations,
    )
}

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
pub fn resolution_executor_unrecorded(records: &[crate::cmd::sangha::Resolution]) -> Check {
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
    Check::new(
        "resolution-executor-unrecorded",
        "Resolution record does not name the seat that executed it",
        Severity::Warn,
        "Article II governs weight and Article III governs record, and they answer different \
         questions. Naming the executor grants it nothing — no standing, no tiebreak, no \
         priority in any later resolution — which is why recording it costs the article \
         nothing. What it buys is that the most consequential actor in the event stops being \
         the one actor the provenance omits. The record already names the tips that were \
         read; the reader was the gap.",
        violations,
    )
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
            branch_present: true,
        }
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
        assert!(resolution_executor_unrecorded(&r).passed());
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
        let c = resolution_executor_unrecorded(&r);
        assert_eq!(c.violations.len(), 1, "{c:#?}");
        assert_eq!(c.severity, Severity::Warn);
        // …and the *other* check is silent about it: a record that names no seat has named
        // no unregistered one, and reporting it twice would double-count one gap.
        assert!(resolution_elector_unregistered(&r, &registered()).passed());
    }

    /// A repository with no sangha has no records, and both checks report that they ran.
    #[test]
    fn no_records_is_not_a_finding() {
        assert!(resolution_elector_unregistered(&[], &[]).passed());
        assert!(resolution_executor_unrecorded(&[]).passed());
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

// ── Prose links ──────────────────────────────────────────────────────────────
//
// `graph-check` validates the corpus's own `links:` edges. Nothing read the markdown
// links in the prose beside them, and they break silently: a node cites a resolution, a
// resolution cites a position, a convention cites a guideline, and any of those can be
// moved or removed by a commit that never touches the citing file.
//
// In a derived repository the gap had eleven live instances — four citations to position
// files that had never left their author's branch, and seven naming files that are not
// there, including two pointing one directory short at the code behind a published figure.

/// One markdown link found in prose.
pub struct ProseLink {
    pub file: String,
    pub line: usize,
    /// The target exactly as written, for the message.
    pub target: String,
    /// Where it resolves from the directory of the file it sits in — which is what a
    /// reader's markdown client does, and the whole reason a repo-root-relative link in
    /// a nested file is broken.
    pub resolved: PathBuf,
    /// A `#L<n>` / `#L<n>-L<m>` fragment, when the link carries one. A heading anchor
    /// is `None`: the file is what exists, the heading is not ours to verify. A line
    /// range is ours — see [`super::line_citations`].
    pub fragment: Option<super::line_citations::LineFragment>,
    /// Byte columns of the whole `[label](target)` on its line, for finding the quoted
    /// passage beside it. Valid into the raw line: the mask that found the link keeps
    /// byte offsets.
    pub span: (usize, usize),
}

/// Whether a link target points outside the filesystem and is therefore not ours to check.
fn is_external(target: &str) -> bool {
    target.is_empty()
        || target.starts_with('#')
        || target.contains("://")
        || target.starts_with("mailto:")
        || target.starts_with("tel:")
        // Root-relative. In a repository these are ambiguous — repo root or site root —
        // and resolving them either way produces findings nobody can act on.
        || target.starts_with('/')
}

/// Strip a markdown title and any angle-bracket wrapper: `<a b> "title"` → `a b`, and
/// split off the fragment.
///
/// A heading anchor is dropped — the file is what exists, the heading is not ours to
/// verify. A `#L<n>` line fragment is different in kind: it names lines in a file this
/// repository owns and is decidable, so it is kept for the line-citation checks rather
/// than falling with the anchors (#563).
fn link_parts(raw: &str) -> (String, Option<super::line_citations::LineFragment>) {
    let t = raw.trim();
    let t = match (t.strip_prefix('<'), t.find('>')) {
        (Some(_), Some(end)) => &t[1..end],
        _ => t.split_whitespace().next().unwrap_or(t),
    };
    match t.split_once('#') {
        Some((path, frag)) => (
            path.trim().to_string(),
            super::line_citations::parse_fragment(frag),
        ),
        None => (t.trim().to_string(), None),
    }
}

/// Every checkable markdown link in one file.
///
/// `dir` is the directory the file sits in — links resolve against it, not against the
/// repository root.
///
/// **Fenced code blocks are skipped.** A path inside a fence is an example, a shell
/// command, or a directory tree drawing; these documents are full of them and every one
/// would be a false positive. Inline code spans are skipped for the same reason.
pub fn prose_links(file: &str, dir: &Path, text: &str) -> Vec<ProseLink> {
    let mut out = Vec::new();
    let mut fence: Option<String> = None;
    for (i, raw) in text.lines().enumerate() {
        let trimmed = raw.trim_start();
        // Fence open/close. The marker is repeated to allow ``` inside a ~~~ block.
        if let Some(open) = &fence {
            if trimmed.starts_with(open.as_str()) {
                fence = None;
            }
            continue;
        }
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            fence = Some(trimmed.chars().take(3).collect());
            continue;
        }
        // Blank out inline code spans so a `[a](b)` shown as code is not read as a link.
        let line = crate::markdown::mask_code_spans(raw);
        let bytes = line.as_bytes();
        let mut j = 0;
        while j < bytes.len() {
            let Some(open) = line[j..].find("](") else {
                break;
            };
            let at = j + open;
            j = at + 2;
            let Some(close_rel) = line[j..].find(')') else {
                break;
            };
            let target_raw = &line[j..j + close_rel];
            j += close_rel + 1;
            // Walk back to the `[` that opens the label, for the link's full extent.
            let label_start = {
                let mut depth = 1usize;
                let mut k = at;
                loop {
                    let Some(prev) = line[..k].rfind(['[', ']']) else {
                        break at;
                    };
                    k = prev;
                    match bytes[k] {
                        b']' => depth += 1,
                        _ => {
                            depth -= 1;
                            if depth == 0 {
                                break k;
                            }
                        }
                    }
                }
            };
            let (target, fragment) = link_parts(target_raw);
            if is_external(&target) {
                continue;
            }
            out.push(ProseLink {
                file: file.to_string(),
                line: i + 1,
                target: target.clone(),
                resolved: dir.join(&target),
                fragment,
                span: (label_start, j),
            });
        }
    }
    out
}

pub fn broken_prose_link(links: &[ProseLink]) -> Check {
    let violations = links
        .iter()
        .filter(|l| !l.resolved.exists())
        .map(|l| {
            Violation::new(
                format!("{}:{}", l.file, l.line),
                format!("`{}` does not resolve from this file's directory", l.target),
            )
        })
        .collect();
    Check::new(
        "broken-prose-link",
        "A markdown link in prose does not resolve",
        Severity::Error,
        "A link resolves against the directory of the file it sits in — that is what a \
         reader's client does — so a path spelled from the repository root is broken in \
         every file that is not at the root. Error severity, which puts it under the \
         baseline ratchet: a link listed there and since repaired fails the build exactly \
         as a new break does, because a list permitted to be wrong drifts and one that \
         over-lists silently re-permits whatever it over-lists. Fenced and inline code are \
         not read, so an example path in a shell command is not a finding.",
        violations,
    )
}

/// A broken prose link inside a region the repository declared it did not author, paired
/// with the declaration that explains it.
pub struct UnauthoredLink<'a> {
    pub region: &'a crate::authorship::Region,
    pub link: ProseLink,
}

pub fn unauthored_prose_link(links: &[UnauthoredLink<'_>]) -> Check {
    let violations = links
        .iter()
        .filter(|u| !u.link.resolved.exists())
        .map(|u| {
            Violation::new(
                format!("{}:{}", u.link.file, u.link.line),
                format!(
                    "`{}` does not resolve — {}",
                    u.link.target,
                    u.region.explain()
                ),
            )
        })
        .collect();
    Check::new(
        "unauthored-prose-link",
        "A broken prose link in material this repository did not author",
        Severity::Info,
        "Error severity is the right verdict for a link somebody here can go and fix. It is \
         the wrong one for generated output, whose defect belongs to the generator and whose \
         file is rewritten by the next build, and for an unmodified import, where editing the \
         file to satisfy a linter falsifies the record it exists to keep. A consumer that met \
         43 broken links under `docs/` could act on 28: fifteen were inside a directory whose \
         own README says it is a frozen copy of an upstream project at a fork point. It \
         baselined them, which records `we accept this violation` when the truth is `this \
         file is not ours` — and a baselined entry that is later repaired fails the build, so \
         the gate came to depend on nobody ever re-syncing the import. Declared in \
         `.yidam/authorship.yml`. Reported rather than silenced, with the reason and whoever \
         can act on it, because these are real defects addressed to somebody else. Info \
         severity: never gates, never baselined.",
        violations,
    )
}

pub fn authorship_region_stale(stale: &[&crate::authorship::Region]) -> Check {
    let violations = stale
        .iter()
        .map(|r| {
            Violation::new(
                crate::authorship::MANIFEST,
                format!(
                    "`{}` is declared `{}` but matches nothing on disk",
                    r.path,
                    r.kind.as_str()
                ),
            )
        })
        .collect();
    Check::new(
        "authorship-region-stale",
        "A declared authorship region matches nothing on disk",
        Severity::Warn,
        "A manifest permitted to be wrong drifts, exactly as a lint baseline does, and the \
         entry that outlives the directory it describes is the one that quietly excuses a \
         path somebody later creates under the same name. `generated` regions are exempt: \
         they are written by a build and are frequently git-ignored, so absence on a fresh \
         clone carries no information. Warn rather than Error, unlike the baseline's own \
         staleness rule, and for the reason this check exists — an imported region is \
         re-synced by somebody upstream on their schedule, and gating on it would make the \
         build's colour depend on that.",
        violations,
    )
}

#[cfg(test)]
mod prose_link_tests {
    use super::*;

    fn scan(dir: &Path, text: &str) -> Vec<ProseLink> {
        prose_links("docs/x.md", dir, text)
    }

    fn fixture() -> tempfile::TempDir {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join("sub")).unwrap();
        std::fs::write(tmp.path().join("there.md"), "x").unwrap();
        std::fs::write(tmp.path().join("sub/deep.md"), "x").unwrap();
        tmp
    }

    /// The defect this exists for: a path spelled from the repository root, sitting in a
    /// file that is not at the root.
    #[test]
    fn a_root_relative_link_in_a_nested_file_is_broken() {
        let tmp = fixture();
        let links = scan(tmp.path(), "See [it](docs/there.md).");
        assert_eq!(links.len(), 1);
        let c = broken_prose_link(&links);
        assert_eq!(c.violations.len(), 1);
        assert!(c.violations[0].detail.contains("docs/there.md"));
    }

    #[test]
    fn a_link_relative_to_its_own_file_resolves() {
        let tmp = fixture();
        let links = scan(tmp.path(), "See [it](there.md) and [deep](sub/deep.md).");
        assert_eq!(links.len(), 2);
        assert!(broken_prose_link(&links).passed());
    }

    /// Every document here is full of example paths in shell blocks and tree drawings.
    /// Reading them would make the check unusable on its first run.
    #[test]
    fn fenced_code_is_not_read() {
        let tmp = fixture();
        let text = "\
Before.

```
git checkout ma/x -- [a](does/not/exist.md)
```

After [it](there.md).
";
        let links = scan(tmp.path(), text);
        assert_eq!(
            links.len(),
            1,
            "{:?}",
            links.iter().map(|l| &l.target).collect::<Vec<_>>()
        );
        assert_eq!(links[0].target, "there.md");
    }

    #[test]
    fn tilde_fences_and_nested_backticks_are_handled() {
        let tmp = fixture();
        let text = "~~~\n[a](nope.md)\n```\n[b](nope2.md)\n~~~\n[c](there.md)\n";
        let links = scan(tmp.path(), text);
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].target, "there.md");
    }

    #[test]
    fn inline_code_is_not_read() {
        let tmp = fixture();
        let links = scan(tmp.path(), "Write `[label](path/to/thing.md)` in the file.");
        assert!(
            links.is_empty(),
            "{:?}",
            links.iter().map(|l| &l.target).collect::<Vec<_>>()
        );
    }

    #[test]
    fn external_and_anchor_targets_are_skipped() {
        let tmp = fixture();
        let text = "[a](https://example.com/x) [b](#section) [c](mailto:x@y.z) [d](/abs/path)";
        assert!(scan(tmp.path(), text).is_empty());
    }

    /// The file is what exists; the heading is not ours to verify.
    #[test]
    fn an_anchor_is_stripped_before_resolving() {
        let tmp = fixture();
        let links = scan(tmp.path(), "[a](there.md#a-heading)");
        assert_eq!(links[0].target, "there.md");
        assert!(broken_prose_link(&links).passed());
    }

    #[test]
    fn titles_and_angle_brackets_are_stripped() {
        let tmp = fixture();
        let links = scan(tmp.path(), "[a](there.md \"Title\") and [b](<there.md>)");
        assert_eq!(links.len(), 2);
        assert!(broken_prose_link(&links).passed());
    }

    #[test]
    fn two_links_on_one_line_are_both_found() {
        let tmp = fixture();
        let links = scan(tmp.path(), "[a](there.md) then [b](gone.md)");
        assert_eq!(links.len(), 2);
        assert_eq!(broken_prose_link(&links).violations.len(), 1);
    }

    #[test]
    fn a_link_to_a_directory_resolves() {
        let tmp = fixture();
        let links = scan(tmp.path(), "[the subdir](sub/)");
        assert!(broken_prose_link(&links).passed());
    }

    #[test]
    fn no_links_is_clean() {
        assert!(broken_prose_link(&[]).passed());
    }
}
