//! RFC-0032's reference grammar: one name for a thing, and one parser that reads it.
//!
//! ```text
//! identifier   yidam://<corpus>/<kind>/<path>[@<rev>][#<property-path>]
//! relative     <kind>/<path>  |  <path>          resolved against the containing corpus
//! kind         node | crate | catalog | skill | decision
//! ```
//!
//! A corpus node had eleven string forms across the yidam repository before this module. Two
//! could say which corpus a node came from, one could say which revision, and none could say
//! both. They were not alternatives a caller chose between — they were what different surfaces
//! invented independently because there was no first form to reuse.
//!
//! **The parser is total.** RFC-0032 was written on the claim that `name-not-a-slug` made a
//! name's character set an invariant. Measured against the sixteen corpora with tracked nodes,
//! fourteen conform and two do not, and `yidam lint --bless` is the supported way to be one of
//! the two (#777). So this module parses structurally for any input and answers
//! [`reference_conforms`] separately, rather than refusing what it was handed. Refusing would
//! leave six of one corpus's ten nodes unnameable and grow a fallback path in every consumer,
//! which is the per-surface improvisation the grammar exists to end.

/// What a reference names. The `<kind>` slot of the grammar.
///
/// A closed vocabulary: a kind is a thing the repository addresses, not an open set. `Node` is the
/// default for a relative reference, because every relative form in the population it replaced
/// named a node.
///
/// `Issue` was added on measurement (#783). Across the corpora that write evidence-tag details,
/// **252 of the 496 mechanically resolvable references are issue references** — `[verified — #362]`
/// — which is the single largest kind in the population and had no form here. The corpus writing
/// them had already paid for their absence: its own `.yidam/corpus/README.md` carries a warning to
/// quote any `properties:` value containing one, because YAML reads an unquoted ` #` as a comment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Kind {
    Node,
    Crate,
    Catalog,
    Skill,
    Decision,
    /// An issue in the corpus's own tracker: `issue/362`.
    Issue,
}

impl Kind {
    /// The word this kind is written as, in every rendering.
    pub fn as_str(&self) -> &'static str {
        match self {
            Kind::Node => "node",
            Kind::Crate => "crate",
            Kind::Catalog => "catalog",
            Kind::Skill => "skill",
            Kind::Decision => "decision",
            Kind::Issue => "issue",
        }
    }

    /// The kind a segment names, or `None` if it names no kind.
    pub fn from_word(s: &str) -> Option<Kind> {
        match s {
            "node" => Some(Kind::Node),
            "crate" => Some(Kind::Crate),
            "catalog" => Some(Kind::Catalog),
            "skill" => Some(Kind::Skill),
            "decision" => Some(Kind::Decision),
            "issue" => Some(Kind::Issue),
            _ => None,
        }
    }

    /// How many segments this kind's `<path>` has.
    ///
    /// Two for a node — `<class>/<name>` — and one for everything else. This is not a
    /// tidiness rule: it is what makes the relative form unambiguous. `node/concept/foo` has
    /// three segments, so reading `node` as the kind leaves a two-segment path (valid) while
    /// reading it as a class leaves three (invalid), and only one reading survives. See
    /// [`parse_reference`].
    pub fn path_arity(&self) -> usize {
        match self {
            Kind::Node => 2,
            _ => 1,
        }
    }
}

/// One reference, parsed.
///
/// `rev` is a pin and not identity — RFC-0032 §4.3. `x` and `x@abc` denote the same node in two
/// states, which is the split [`ExternalCitation`](crate::corpus::ExternalCitation) already made
/// by holding `node` and `commit` as two fields. Folding the revision into identity would
/// silently change what every id in every existing corpus refers to.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Reference {
    /// The naming corpus, or `None` for a reference relative to the containing one.
    pub corpus: Option<String>,
    pub kind: Kind,
    /// The `<path>` segments, joined by `/`. `<class>/<name>` for a node.
    pub path: String,
    /// A revision pin: a commit or a tag.
    pub rev: Option<String>,
    /// A declared property path.
    ///
    /// **Never a claim.** RFC-0008 measured 22 claims entering corpora in resolution commits
    /// and 0 matching byte-identically at any participating tip, because a sentence search that
    /// ends at `\n` extracts one claim as two different strings in two hard-wrapped nodes.
    /// Identity by surface form is identity by line wrapping, so the fragment names something
    /// that has a name.
    pub fragment: Option<String>,
}

impl Reference {
    /// The `<path>` split into its segments.
    pub fn segments(&self) -> Vec<&str> {
        if self.path.is_empty() {
            Vec::new()
        } else {
            self.path.split('/').collect()
        }
    }
}

/// Whether a segment is a slug: lowercase ASCII words joined by single hyphens.
///
/// The one implementation. `yidam`'s `name-not-a-slug` check reads this rather than carrying its
/// own copy, because a rule with two implementations is a rule two surfaces can disagree about —
/// the shape that put four copies of the open-question predicate in the CLI before one of them
/// was found under-reporting a corpus 26 to 2.
///
/// It is what fourteen of the sixteen measured corpora already write, and what two do not: a
/// manufacturing corpus writes `NonConformance` and a water-quality one `DischargePoint`, both
/// taking class names from their domain's vocabulary in its own case. Kebab-case is a convention
/// corpora converge on with maturity, not one they start with (#777).
pub fn is_slug(s: &str) -> bool {
    !s.is_empty()
        && !s.starts_with('-')
        && !s.ends_with('-')
        && !s.contains("--")
        && s.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// Whether every segment of a reference is a slug, so it needs no escaping in any rendering.
///
/// The separate predicate RFC-0032 §4.6 calls for. A caller that must emit a URI acts on the
/// answer; the parser does not refuse on its behalf. This is the test
/// `export_rdf.rs` already applies to a foreign alignment IRI — check whether it is
/// dereferenceable and demote it when it is not — turned inward onto our own identifiers, which
/// §2 of that RFC complains it never was.
///
/// The corpus and the revision are checked too. A corpus name is the declared package name,
/// which is what every surface prints; a revision is a commit hex or a tag.
pub fn reference_conforms(r: &Reference) -> bool {
    r.corpus.as_deref().map(is_slug).unwrap_or(true)
        && !r.path.is_empty()
        && r.segments().iter().all(|s| is_slug(s))
        && r.segments().len() == r.kind.path_arity()
        && r.rev.as_deref().map(is_slug).unwrap_or(true)
        && r.fragment
            .as_deref()
            .map(|f| fragment_conforms(r.kind, f))
            .unwrap_or(true)
}

/// Whether a fragment names something, for the kind it is attached to.
///
/// **Two rules, because a fragment names a path into two different kinds of thing.** For every
/// kind but `Crate` it is a declared property path — dotted slugs — which is §4.4 unchanged. For a
/// `Crate` it names a code item or a file inside that crate, and those are named by a foreign
/// toolchain: `ohio_panel::equalization_by_year` and `tests/the_weights_behind_the_index.rs` are
/// both real references in a measured corpus, and neither is a dotted slug path.
///
/// §4.4's refusal survives this. What it refuses is a fragment naming a **claim**, and the reason
/// is measured rather than aesthetic: RFC-0008 found 22 claims entering corpora and 0 matching
/// byte-identically at any tip, because identity by surface form is identity by line wrapping. A
/// Rust item path has an identity a compiler enforces and a file path one the filesystem does; the
/// argument for refusing a claim does not reach either.
///
/// The crate rule deliberately admits uppercase and `.`, which `is_slug` does not. A type is
/// `CamelCase` and a file has an extension, and neither is this project's naming rule to make —
/// the same reason `export_rdf` tests a foreign alignment IRI for dereferenceability instead of
/// validating its shape.
pub fn fragment_conforms(kind: Kind, fragment: &str) -> bool {
    if fragment.is_empty() {
        return false;
    }
    if kind != Kind::Crate {
        return fragment.split('.').all(is_slug);
    }
    // `/` and `::` both separate; a segment is a filename or a Rust identifier.
    fragment
        .split('/')
        .flat_map(|part| part.split("::"))
        .all(|seg| {
            !seg.is_empty()
                && seg
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.')
        })
}

/// Parse a reference. `None` only when the input names no thing at all.
///
/// Accepts the canonical `yidam://<corpus>/<kind>/<path>` identifier, the relative forms, and
/// the legacy spellings the repository already wrote — a trailing `.yml`, a full
/// `.yidam/corpus/<class>/<name>.yml` path, `pkg::class/name`, and RFC-0005's frozen
/// `yidam://corpus/…`, `yidam://skills/…` and `yidam://decisions/…` resource URIs.
///
/// **The relative form is unambiguous, by arity.** `<kind>/<path>` and a bare `<path>` share a
/// shape, and a corpus may legitimately declare a class named `node`. The path's segment count
/// decides: reading the leading word as a kind must leave exactly that kind's arity, or the word
/// is a class name after all. `node/concept/foo` is the node `concept/foo`; `node/foo` is the
/// node `foo` in a class named `node`. Where both readings are valid — `skill/foo`, with a class
/// named `skill` — the kind wins, because the grammar puts a kind in that position; and
/// [`render_reference`] emits the explicit `node/skill/foo` for the other reading, so the round
/// trip is total and the collision costs nothing.
///
/// **`@` is split from the right and `#` from the left.** A conforming segment contains neither,
/// so the rule is only visible on input that already failed [`reference_conforms`].
pub fn parse_reference(input: &str) -> Option<Reference> {
    let s = input.trim();
    if s.is_empty() {
        return None;
    }

    // The fragment comes off first: it is the last component of the grammar, and a `#` inside a
    // revision is not a thing any producer writes.
    let (s, fragment) = match s.split_once('#') {
        Some((head, frag)) => (head, Some(frag.to_string())),
        None => (s, None),
    };
    // …and the revision from the right, so a path segment holding an `@` in a non-conforming
    // corpus still leaves the pin recoverable.
    let (body, rev) = match s.rsplit_once('@') {
        Some((head, rev)) if !head.is_empty() && !rev.is_empty() => (head, Some(rev.to_string())),
        _ => (s, None),
    };

    let (corpus, rest) = split_authority(body);
    let (kind, path) = split_kind(&rest)?;

    Some(Reference {
        corpus,
        kind,
        path,
        rev,
        fragment,
    })
}

/// Peel the naming corpus off the front, in whichever of the three ways it can be written.
///
/// `yidam://<corpus>/…` is the grammar's. `yidam://corpus/…` is RFC-0005's frozen authority,
/// where `corpus` is a collection kind rather than a corpus name — the defect RFC-0032 §1 is
/// about — and it means *this* corpus, so it maps to `None`. `pkg::class/name` is
/// `qualified_id`'s form, the only string in the eleven that could say which corpus.
fn split_authority(body: &str) -> (Option<String>, String) {
    if let Some(rest) = body.strip_prefix("yidam://") {
        let (authority, tail) = match rest.split_once('/') {
            Some((a, t)) => (a, t),
            None => (rest, ""),
        };
        return match authority {
            // RFC-0005's five URIs spend the authority slot on a collection kind. `corpus`
            // there means the serving repository's own corpus, so it is not a corpus name.
            "corpus" | "" => (None, tail.to_string()),
            // …and `skills`/`decisions` spend it on the *kind*, so the word moves into the
            // path where the kind split reads it, and the two aliases need no second reader.
            "skills" => (None, format!("skill/{tail}")),
            "decisions" => (None, format!("decision/{tail}")),
            other => (Some(other.to_string()), tail.to_string()),
        };
    }
    if let Some((pkg, rest)) = body.split_once("::") {
        if !pkg.is_empty() && !rest.is_empty() {
            return (Some(pkg.to_string()), rest.to_string());
        }
    }
    (None, body.to_string())
}

/// Split the kind off the front of a path, defaulting to `Node`, and normalise the legacy
/// spellings of a node path.
fn split_kind(rest: &str) -> Option<(Kind, String)> {
    let cleaned = normalise_node_path(rest);
    if cleaned.is_empty() {
        return None;
    }
    let count = cleaned.split('/').count();
    if let Some((head, tail)) = cleaned.split_once('/') {
        if let Some(kind) = Kind::from_word(head) {
            // The arity test. Reading `head` as a kind must leave exactly that kind's path, or
            // `head` was a class name and the whole string is the path.
            if count - 1 == kind.path_arity() {
                return Some((kind, tail.to_string()));
            }
        }
    }
    Some((Kind::Node, cleaned))
}

/// The node-path spellings the repository already wrote, reduced to `<class>/<name>`.
///
/// `find_node` tolerated three of these and no contract mentioned any of them. An ad-hoc
/// resolver is what an absent grammar looks like from the inside, so they are admitted here,
/// once, instead of in each surface.
fn normalise_node_path(rest: &str) -> String {
    let rest = rest.trim_start_matches('/');
    // A full repository path, as `find_node` accepted: `.yidam/corpus/<class>/<name>.yml`.
    let rest = match rest.find(".yidam/corpus/") {
        Some(at) => &rest[at + ".yidam/corpus/".len()..],
        None => rest,
    };
    // …and the web editor's route, `/node/<class>/<name>`, whose leading segment the kind split
    // reads on its own.
    let rest = rest.strip_suffix('/').unwrap_or(rest);
    rest.strip_suffix(".yml").unwrap_or(rest).to_string()
}

/// Render a reference. The only place an identifier is built.
///
/// A corpus renders the canonical `yidam://` identifier; its absence renders the relative form,
/// which is what a corpus's own files and its own tool arguments use. The locator — RFC-0032
/// §4.2's `https://` rendering, for anything a stranger has to follow — is derived per corpus
/// from a declared base and is not this function's job: an identifier must survive a `.yiz`
/// tarball, a private corpus and an offline clone, none of which have a host.
///
/// **A relative node reference names its kind when its class would otherwise be read as one.**
/// A node in a class called `skill` renders `node/skill/foo`, not `skill/foo`, so that
/// [`parse_reference`] returns what was rendered. Without that the round trip fails on exactly
/// the corpora that name a class after a kind, and silently.
pub fn render_reference(r: &Reference) -> String {
    let mut out = String::new();
    match &r.corpus {
        Some(corpus) => {
            out.push_str("yidam://");
            out.push_str(corpus);
            out.push('/');
            out.push_str(r.kind.as_str());
            out.push('/');
        }
        None => {
            let shadows_a_kind = r
                .path
                .split('/')
                .next()
                .is_some_and(|head| Kind::from_word(head).is_some());
            if r.kind != Kind::Node || shadows_a_kind {
                out.push_str(r.kind.as_str());
                out.push('/');
            }
        }
    }
    out.push_str(&r.path);
    if let Some(rev) = &r.rev {
        out.push('@');
        out.push_str(rev);
    }
    if let Some(fragment) = &r.fragment {
        out.push('#');
        out.push_str(fragment);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(path: &str) -> Reference {
        Reference {
            corpus: None,
            kind: Kind::Node,
            path: path.into(),
            rev: None,
            fragment: None,
        }
    }

    #[test]
    fn a_bare_class_and_name_is_a_relative_node() {
        assert_eq!(parse_reference("concept/foo"), Some(node("concept/foo")));
    }

    #[test]
    fn the_canonical_identifier_carries_the_corpus() {
        let r = parse_reference("yidam://producer/node/concept/weir").unwrap();
        assert_eq!(r.corpus.as_deref(), Some("producer"));
        assert_eq!(r.kind, Kind::Node);
        assert_eq!(r.path, "concept/weir");
    }

    /// The whole point of the grammar: a form that says corpus *and* revision. Before it, two
    /// forms could say the first, one could say the second, and none could say both.
    #[test]
    fn a_reference_can_say_which_corpus_at_which_revision() {
        let r = parse_reference("yidam://producer/node/concept/weir@abc123").unwrap();
        assert_eq!(r.corpus.as_deref(), Some("producer"));
        assert_eq!(r.rev.as_deref(), Some("abc123"));
        assert_eq!(r.path, "concept/weir");
    }

    #[test]
    fn a_fragment_names_a_property_path() {
        let r = parse_reference("concept/foo#properties.enacted").unwrap();
        assert_eq!(r.fragment.as_deref(), Some("properties.enacted"));
        assert_eq!(r.path, "concept/foo");
    }

    // ── the arity rule, which is what makes the relative form unambiguous ──────

    #[test]
    fn a_leading_kind_word_is_a_kind_when_the_arity_fits() {
        let r = parse_reference("node/concept/foo").unwrap();
        assert_eq!(r.kind, Kind::Node);
        assert_eq!(r.path, "concept/foo");
    }

    /// And a class name when it does not. `node/foo` cannot be the kind `node` — that would
    /// leave a one-segment node path — so it is the node `foo` in a class called `node`.
    #[test]
    fn a_leading_kind_word_is_a_class_when_the_arity_does_not_fit() {
        let r = parse_reference("node/foo").unwrap();
        assert_eq!(r.kind, Kind::Node);
        assert_eq!(r.path, "node/foo");
    }

    #[test]
    fn a_single_segment_kind_takes_a_single_segment_path() {
        for (input, kind) in [
            ("skill/bootstrap", Kind::Skill),
            ("decision/adr-1", Kind::Decision),
            ("crate/yidam-core", Kind::Crate),
            ("catalog/statute-book", Kind::Catalog),
        ] {
            let r = parse_reference(input).unwrap();
            assert_eq!(r.kind, kind, "{input}");
            assert_eq!(r.path.split('/').count(), 1, "{input}");
        }
    }

    /// The one genuinely ambiguous shape, and the rendering that costs nothing to resolve it.
    ///
    /// `skill/foo` is read as the skill `foo`, because the grammar puts a kind in that
    /// position. A node in a class named `skill` is therefore rendered with its kind named, and
    /// the round trip holds for both.
    #[test]
    fn a_class_named_after_a_kind_still_round_trips() {
        let shadowed = node("skill/foo");
        assert_eq!(render_reference(&shadowed), "node/skill/foo");
        assert_eq!(parse_reference("node/skill/foo"), Some(shadowed));

        let skill = Reference {
            kind: Kind::Skill,
            ..node("foo")
        };
        assert_eq!(render_reference(&skill), "skill/foo");
        assert_eq!(parse_reference("skill/foo"), Some(skill));
    }

    // ── the legacy spellings, admitted once here instead of in each surface ────

    #[test]
    fn the_spellings_find_node_tolerated_all_resolve_to_one_reference() {
        for input in [
            "concept/foo",
            "concept/foo.yml",
            ".yidam/corpus/concept/foo.yml",
            "/node/concept/foo",
        ] {
            assert_eq!(parse_reference(input), Some(node("concept/foo")), "{input}");
        }
    }

    #[test]
    fn a_qualified_id_names_its_corpus() {
        let r = parse_reference("producer::concept/weir").unwrap();
        assert_eq!(r.corpus.as_deref(), Some("producer"));
        assert_eq!(r.path, "concept/weir");
    }

    /// RFC-0005's frozen resource URIs, whose authority slot holds a collection kind. `corpus`
    /// there means the serving repository's own, so it is not a corpus name.
    #[test]
    fn the_frozen_resource_uris_map_into_the_new_shape() {
        let r = parse_reference("yidam://corpus/concept/knowledge-graph").unwrap();
        assert_eq!(r.corpus, None);
        assert_eq!(r.kind, Kind::Node);
        assert_eq!(r.path, "concept/knowledge-graph");

        let r = parse_reference("yidam://skills/my-skill").unwrap();
        assert_eq!(r.kind, Kind::Skill);
        assert_eq!(r.path, "my-skill");

        let r = parse_reference("yidam://decisions/adr-1").unwrap();
        assert_eq!(r.kind, Kind::Decision);
        assert_eq!(r.path, "adr-1");
    }

    #[test]
    fn nothing_at_all_is_not_a_reference() {
        for input in ["", "   ", "yidam://", "#frag"] {
            assert_eq!(parse_reference(input), None, "{input:?}");
        }
    }

    // ── conformance is reported, not required ─────────────────────────────────

    /// The two corpora #777 measured. Their ids parse — refusing would leave six of one
    /// corpus's ten nodes unnameable — and they are reported non-conforming.
    #[test]
    fn a_non_conforming_id_parses_and_says_so() {
        for input in [
            "Disposition/DIS-2024-0447",
            "DischargePoint/outfall-002",
            "Measurement/MEAS-0883-030-BORE",
        ] {
            let r = parse_reference(input).unwrap_or_else(|| panic!("refused {input}"));
            assert!(!reference_conforms(&r), "{input} reported conforming");
            assert_eq!(render_reference(&r), input, "{input} did not round trip");
        }
    }

    #[test]
    fn the_ids_fourteen_corpora_write_conform() {
        for input in [
            "concept/knowledge-graph",
            "person/matt-huffman",
            "fiscal-period/fy2024",
            "yidam://producer/node/concept/weir@abc123",
            "skill/bootstrap",
        ] {
            let r = parse_reference(input).unwrap();
            assert!(reference_conforms(&r), "{input} reported non-conforming");
        }
    }

    /// A path of the wrong arity for its kind does not conform, even when every segment is a
    /// slug. `skill/a/b` names no skill, and a checker that read only the character set would
    /// pass it.
    #[test]
    fn conformance_reads_the_arity_and_not_only_the_characters() {
        let wrong = Reference {
            kind: Kind::Skill,
            ..node("a/b")
        };
        assert!(!reference_conforms(&wrong));
        let bare = node("foo");
        assert!(!reference_conforms(&bare), "a node needs class and name");
    }

    // ── the round trip, which is the property the rendering was chosen for ────

    #[test]
    fn every_reference_survives_the_round_trip() {
        let cases = [
            node("concept/foo"),
            node("skill/foo"),
            node("node/foo"),
            Reference {
                kind: Kind::Skill,
                ..node("bootstrap")
            },
            Reference {
                corpus: Some("producer".into()),
                ..node("concept/weir")
            },
            Reference {
                rev: Some("abc123".into()),
                ..node("concept/foo")
            },
            Reference {
                fragment: Some("properties.enacted".into()),
                ..node("concept/foo")
            },
            Reference {
                corpus: Some("producer".into()),
                kind: Kind::Decision,
                path: "adr-1".into(),
                rev: Some("v1.2.0".into()),
                fragment: Some("status".into()),
            },
        ];
        for want in cases {
            let rendered = render_reference(&want);
            let got = parse_reference(&rendered)
                .unwrap_or_else(|| panic!("{rendered} did not parse back"));
            assert_eq!(got, want, "round trip via {rendered}");
        }
    }

    // ── the sixth kind, and the fragment that names code (#783) ───────────────

    /// The largest reference kind in the measured population, which had no form before.
    #[test]
    fn an_issue_reference_parses_renders_and_conforms() {
        let r = parse_reference("issue/362").expect("issue/362 is a reference");
        assert_eq!(r.kind, Kind::Issue);
        assert_eq!(r.path, "362");
        assert!(reference_conforms(&r));
        assert_eq!(render_reference(&r), "issue/362");

        let absolute = parse_reference("yidam://hegeomai/issue/362").unwrap();
        assert_eq!(absolute.corpus.as_deref(), Some("hegeomai"));
        assert_eq!(absolute.kind, Kind::Issue);
        assert_eq!(absolute.path, "362");
    }

    /// And a class named `issue` is still reachable, by the arity rule the other kinds use.
    #[test]
    fn a_class_named_issue_does_not_lose_to_the_new_kind() {
        let shadowed = Reference {
            kind: Kind::Node,
            ..node("issue/reopened")
        };
        assert_eq!(render_reference(&shadowed), "node/issue/reopened");
        assert_eq!(parse_reference("node/issue/reopened"), Some(shadowed));
        // …and the bare spelling reads as the kind, because the grammar puts one there.
        assert_eq!(parse_reference("issue/reopened").unwrap().kind, Kind::Issue);
    }

    /// A crate fragment names a code item or a file, and both are real in a measured corpus.
    #[test]
    fn a_crate_fragment_names_code_and_a_node_fragment_names_a_property() {
        for fragment in [
            "ohio_panel::equalization_by_year",
            "tests/the_weights_behind_the_index.rs",
            "OhioPanel::new",
            "identified",
        ] {
            assert!(
                fragment_conforms(Kind::Crate, fragment),
                "crate fragment rejected: {fragment}"
            );
        }
        // The node rule is §4.4's, unchanged: a declared property path of dotted slugs.
        assert!(fragment_conforms(Kind::Node, "properties.enacted"));
        for fragment in ["properties.Enacted", "tests/x.rs", "a::b", ""] {
            assert!(
                !fragment_conforms(Kind::Node, fragment),
                "node fragment accepted a non-property path: {fragment}"
            );
        }
        // …and neither rule admits an empty fragment or whitespace.
        assert!(!fragment_conforms(Kind::Crate, ""));
        assert!(!fragment_conforms(Kind::Crate, "a b"));
        assert!(!fragment_conforms(Kind::Crate, "a//b"));
    }

    /// The whole reference, as a corpus actually wrote it in a tag detail.
    #[test]
    fn the_crate_references_a_corpus_wrote_in_brackets_all_conform() {
        for input in [
            "crate/dispersion",
            "crate/dispersion#ohio_panel::equalization_by_year",
            "crate/dispersion#tests/the_weights_behind_the_index.rs",
            "crate/project#tests/a_draft_cannot_hide_what_it_did_not_price.rs",
        ] {
            let r = parse_reference(input).unwrap_or_else(|| panic!("refused {input}"));
            assert_eq!(r.kind, Kind::Crate, "{input}");
            assert!(reference_conforms(&r), "{input} reported non-conforming");
            assert_eq!(render_reference(&r), input, "{input} did not round trip");
        }
    }

    #[test]
    fn the_slug_rule_is_the_one_the_corpora_follow() {
        for name in ["knowledge-graph", "hb96", "section-3", "a", "2024"] {
            assert!(is_slug(name), "rejected a name in use: {name:?}");
        }
        for name in [
            "Disposition",
            "TI-7842",
            "two words",
            "a--b",
            "-lead",
            "trail-",
            "",
        ] {
            assert!(!is_slug(name), "accepted a name no URI survives: {name:?}");
        }
    }
}
