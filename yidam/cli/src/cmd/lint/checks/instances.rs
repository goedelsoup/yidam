use super::*;

pub const MISSING_CLASS: CheckSpec = CheckSpec {
    id: "missing-class",
    title: "Instance declaring no class",
    severity: Severity::Error,
    rationale: "An instance's class is what binds it to a schema. Without one there is nothing to \
         validate the node against and nothing to render it as — it is a YAML file in a \
         corpus directory, not a node.",
};

pub fn missing_class(nodes: &[Node]) -> Check {
    let violations = nodes
        .iter()
        .filter(|n| n.inst.class.is_none())
        .map(|n| Violation::new(&n.rel, "no `class:` field"))
        .collect();
    Check::new(&MISSING_CLASS, violations)
}

pub const UNKNOWN_CLASS: CheckSpec = CheckSpec {
    id: "unknown-class",
    title: "Instance of a class that has no schema",
    severity: Severity::Error,
    rationale: "Either the class file was never written or the instance misspells it. Both leave \
         the node unvalidatable, and a misspelling additionally splits one class into two \
         that no query will join.",
};

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
    Check::new(&UNKNOWN_CLASS, violations)
}

pub const MISSING_LABEL: CheckSpec = CheckSpec {
    id: "missing-label",
    title: "Instance with no human-readable label",
    severity: Severity::Warn,
    rationale: "The label is what every index, export, and graph rendering shows. Without it a \
         reader gets a filename, which is an identifier chosen for stability rather than \
         for saying what the thing is.",
};

pub fn missing_label(nodes: &[Node]) -> Check {
    let violations = nodes
        .iter()
        .filter(|n| n.inst.label.is_none())
        .map(|n| Violation::new(&n.rel, "no `label:` field"))
        .collect();
    Check::new(&MISSING_LABEL, violations)
}

pub const MISSING_DESCRIPTION: CheckSpec = CheckSpec {
    id: "missing-description",
    title: "Instance with no description",
    severity: Severity::Warn,
    rationale: "The prose is the node's content. An instance with properties and links but \
         nothing said about it records that something exists without recording what is \
         known about it. What counts as prose is what the ontology declares — a node \
         carrying a `summary` and no `description` has said something, and reporting it \
         here would report the corpus using a vocabulary it declared.",
};

pub fn missing_description(nodes: &[Node], prose: &crate::prose::ProseFields) -> Check {
    let violations = nodes
        .iter()
        .filter(|n| crate::prose::of(&n.inst, prose, &class_of(n)).is_empty())
        .map(|n| {
            let fields = prose.for_class(&class_of(n));
            Violation::new(
                &n.rel,
                if fields.len() == 1 {
                    "no `description:` field".to_string()
                } else {
                    format!("no prose in any of `{}`", fields.join("`, `"))
                },
            )
        })
        .collect();
    Check::new(&MISSING_DESCRIPTION, violations)
}

pub const ORPHAN_OUT: CheckSpec = CheckSpec {
    id: "orphan-out",
    title: "Node connected to nothing",
    severity: Severity::Error,
    rationale: "Every node must carry at least one outgoing edge. A node with none is unreachable \
         by traversal from anywhere, which in a graph whose value is its connectivity means \
         it is not in the graph at all. A class whose instances legitimately end a path — a \
         root, a glossary entry, a terminal source — says so with `outgoing: optional`.",
};

/// A node with no outgoing link, unless its class declares `outgoing: optional` (RFC-0064 §6).
pub fn orphan_out(nodes: &[Node], classes: &[Class]) -> Check {
    let exempt: HashSet<&str> = classes
        .iter()
        .filter(|c| c.outgoing_optional)
        .map(|c| c.name.as_str())
        .collect();
    let violations = nodes
        .iter()
        .filter(|n| n.inst.links.as_deref().unwrap_or(&[]).is_empty())
        .filter(|n| !exempt.contains(class_of(n).as_str()))
        .map(|n| Violation::new(&n.rel, "no outgoing links"))
        .collect();
    Check::new(&ORPHAN_OUT, violations)
}

/// The class an instance belongs to, from its path: `.yidam/corpus/person/x.yml` → `person`.
///
/// A [`crate::paths::class_of_path`] over the node's path, and deliberately nothing more —
/// `yidam embed` stamps the same rule onto every index row, and the two bodies drifting apart
/// is the hazard [`crate::paths::class_of_path`] documents.
pub(crate) fn class_of(n: &Node) -> String {
    crate::paths::class_of_path(&n.path)
}

/// Whether a name is a slug: lowercase ASCII words joined by single hyphens.
///
/// **Re-exported, not reimplemented.** The rule lives in [`yidam_core::uri`], where
/// `reference_conforms` is built on it and the parity fixtures hold three languages to one
/// answer. A second copy here would be a rule two surfaces could disagree about — the shape
/// that put four copies of the open-question predicate in this crate before one of them was
/// found under-reporting a consumer's corpus 26 to 2.
///
/// Deliberately strict, and it is what the mature corpora already write — but not what *every*
/// corpus writes, which is the correction below. Across the sixteen tracked corpora on disk
/// (2,764 instance nodes, 2026-09-09), **fourteen have no finding** and the only characters in
/// use beyond `a-z` are `-` and the digits. Two do:
///
/// | Corpus | Commits | Findings |
/// |---|---|---|
/// | `yidam-proto-001d` | 8 | 6 class names (`DischargePoint`, `Reach`, …) |
/// | `yidam-proto-002c` | 3 | 10 class names, and 6 node names (`DIS-2024-0447`, `TI-7842`) |
///
/// Both are the two youngest corpora measured, and both took their class names from their
/// domain's own vocabulary in its own case — a manufacturing corpus writes `NonConformance`, a
/// water-quality one writes `DischargePoint`. So this is a convention the corpora *converge* on
/// with maturity, not one they start with, and the rule is being imposed on the beginning of a
/// corpus's life even though it is descriptive of the rest of it.
pub(crate) use yidam_core::uri::is_slug;

/// The last path segment of a repo-relative path, with `.ont.yml` or `.yml` removed.
///
/// From `rel` rather than `path`, because `rel` is what the finding reports and what a node's
/// id is derived from — and because a caller constructing a `Node` for a test gives `rel` the
/// corpus-shaped value and `path` whatever the temp dir handed it.
fn stem_of(rel: &str) -> &str {
    let last = rel.rsplit('/').next().unwrap_or(rel);
    last.strip_suffix(".ont.yml")
        .or_else(|| last.strip_suffix(".yml"))
        .unwrap_or(last)
}

pub const NAME_NOT_A_SLUG: CheckSpec = CheckSpec {
    id: "name-not-a-slug",
    title: "Name that cannot be put in a URI",
    severity: Severity::Error,
    rationale:
        "A node's id is `<class>/<name>`, and that id is rendered as an MCP resource URI, an \
         LSP `file://` document URI, a web route and an RDF subject. The three that build the \
         string do it three different ways: the MCP surface interpolates raw and matches back \
         with `strip_prefix`, so a space produces a URI no client can round-trip; the LSP \
         escapes a space and nothing else; only the web editor encodes per segment. So a name \
         outside the slug set does not fail loudly, it fails as a resource that cannot be \
         fetched.\n\n\
         Measured across sixteen corpora and 2,764 instance nodes: fourteen have no finding, \
         and the only non-alphabetic characters those fourteen use are `-` and the digits. Two \
         do have findings — 6 and 16 — and both are the youngest corpora measured, both because \
         their class names came from their domain's vocabulary in its own case: a manufacturing \
         corpus writes `NonConformance`, a water-quality one writes `DischargePoint`. So this is \
         a convention corpora converge on with maturity rather than one they start with.\n\n\
         Error anyway, because the finding is true: a name outside this set names a resource no \
         client can fetch, and reporting that quietly would not make it fetchable. What makes it \
         survivable is the baseline — `yidam lint --bless` records the names a corpus already \
         has, and the check still stops the next one. Renaming is what costs, because an edge \
         into a node is written by its name.",
};

/// A node or class whose name cannot survive being put in a URI.
///
/// A name is half of a node's identity — `<class>/<name>` — and that identity is rendered into
/// four incompatible encodings today. The MCP resource surface interpolates it raw and matches
/// it back with `strip_prefix`, so a name carrying a space emits a malformed URI and a client
/// that correctly percent-encodes it gets *unknown resource*. The LSP escapes a space and
/// nothing else. The web editor is the only surface that encodes per segment. A name outside
/// this set therefore breaks two of the three consumers **silently**, which is why this is an
/// error rather than a note.
///
/// Chosen against the population rather than from taste, and **the population was measured
/// twice**. The first measurement excluded three prototype corpora on maturity grounds — 3, 8
/// and 24 commits against a floor of 73 — and reported 0 findings across the remaining thirteen,
/// from which this docstring concluded that no repository inherits a red gate. Two of the three
/// excluded corpora are the only ones that fire: 6 findings and 16 findings, `yidam lint` exiting
/// 1 in both. The exclusion was right for measuring *practice* and wrong for measuring *adoption
/// cost*, because a three-commit corpus is exactly what a repository looks like on the day it
/// adopts a check. See [`is_slug`] for the table.
///
/// Error is still the right severity, for a reason the empty population was standing in for: the
/// finding is **true**. `Disposition/DIS-2024-0447` cannot round-trip the MCP resource surface,
/// and saying so quietly would not make it fetchable. What makes that survivable is the baseline
/// ratchet — `--bless` records what a corpus already has, and the check still stops the next one.
///
/// The cost of waiting is what makes it worth landing at all — a name is load-bearing the moment
/// another node links to it, and `prelude/GRAPH.md` is explicit that renaming severs edges. A
/// rule that arrives after the names do is a rule nobody can apply.
///
/// Scope: an instance's own name and a class's declared name. A class *directory* is not read
/// separately — a directory that disagrees with its `.ont.yml` is what `unknown-class` reports,
/// and two checks answering one question is the drift this repository keeps finding.
pub fn name_not_a_slug(nodes: &[Node], classes: &[Class]) -> Check {
    let mut violations = Vec::new();
    let why = "a name is half a node's id and is rendered into URIs by three surfaces that \
               encode differently — lowercase letters, digits and single interior hyphens only";
    for n in nodes {
        let name = stem_of(&n.rel);
        if !is_slug(name) {
            violations.push(Violation::new(
                &n.rel,
                format!("node name `{name}` is not a slug — {why}"),
            ));
        }
    }
    for c in classes {
        if !is_slug(&c.name) {
            violations.push(Violation::new(
                &c.rel,
                format!("class name `{}` is not a slug — {why}", c.name),
            ));
        }
    }
    Check::new(&NAME_NOT_A_SLUG, violations)
}

pub const REFERENCE_NOT_IN_THE_GRAMMAR: CheckSpec = CheckSpec {
    id: "reference-not-in-the-grammar",
    title: "A `references:` entry that is not a reference",
    severity: Severity::Warn,
    rationale:
        "`references:` is where a node names something that is not a node — an issue, a crate, a \
         catalog entry, a decision — and every entry is a reference in RFC-0032's grammar, written \
         as a string. `yidam_core::uri` is the one parser, so an entry it cannot resolve is an \
         entry no surface can: not the MCP resource namespace, not the RDF export, not a rendered \
         citation.\n\n\
         The finding names which rule the entry missed, because \"not a reference\" is not \
         actionable and the three causes have three different fixes: a segment count that does not \
         match the kind, a segment that cannot survive being put in a URI, or a fragment that does \
         not name what its kind's fragment names.\n\n\
         Warn rather than Error because the field is new and nothing writes it yet: 496 references \
         across five corpora are still inside evidence-tag brackets, which is the population \
         `yidam migrate references` moves. A finding here is a typo in a file somebody is already \
         editing, and the baseline is what keeps it from going quiet.",
};

/// A `references:` entry that is not a reference in the grammar.
///
/// The destination RFC-0032 gives a reference needs a reader, or it is a key a corpus can write and
/// nothing checks — which is how `[unentered]` happened, one layer up: a corpus coined a mark, the
/// renderer assigned it a meaning nobody chose, and the prose ended up denying what the badge
/// asserted. So this is the reader, and it lands with the field rather than after it.
///
/// Two failures, reported apart because they need different fixes. An entry that does not **parse**
/// names nothing at all — free text where a reference belongs. An entry that parses but does not
/// **conform** names something whose segments cannot survive a URI, which is
/// [`name_not_a_slug`]'s rule reaching a second population: the same check, on a reference a node
/// wrote rather than on the node's own name.
///
/// `Warn`, not `Error`, and for a reason the severity table makes available: no corpus writes this
/// field yet (#783 measured 496 references still inside brackets), so there is no population to
/// inherit a finding. When `yidam migrate references` starts writing entries, a wrong one is
/// somebody's typo in a file they are already editing — worth seeing, not worth stopping a build
/// over — and the baseline ratchet is what makes it stay visible.
pub fn reference_not_in_the_grammar(nodes: &[Node]) -> Check {
    let mut violations = Vec::new();
    for n in nodes {
        for entry in n.inst.references.as_deref().unwrap_or(&[]) {
            if let Some(why) = why_not_a_reference(entry) {
                violations.push(Violation::new(
                    &n.rel,
                    format!("`references:` entry {entry:?} {why}"),
                ));
            }
        }
    }
    Check::new(&REFERENCE_NOT_IN_THE_GRAMMAR, violations)
}

/// Why an entry is not a reference, or `None` when it is one.
///
/// **Named rather than derived from `parse_reference` returning `None`.** The parser is *total* by
/// design (#777): it answers for any input so that a corpus which blessed a non-slug name can still
/// address its own nodes. So `parse_reference` accepts `"not a reference at all"` — as a node with
/// a one-segment path — and "does it parse" is not a test a check can use. What separates an entry
/// a corpus meant from one it did not is `reference_conforms`, and this reports *which* of its
/// clauses failed so the finding says something the author can act on.
fn why_not_a_reference(entry: &str) -> Option<String> {
    let Some(r) = yidam_core::uri::parse_reference(entry) else {
        return Some("is empty".to_string());
    };
    if yidam_core::uri::reference_conforms(&r) {
        return None;
    }
    let kind = r.kind.as_str();
    let arity = r.kind.path_arity();
    let segments = r.path.split('/').count();
    if segments != arity {
        return Some(format!(
            "reads as a `{kind}` reference with {segments} path segment(s), and a `{kind}` takes \
             {arity}. A node is `<class>/<name>`; every other kind is one segment"
        ));
    }
    if let Some(bad) = r.path.split('/').find(|s| !yidam_core::uri::is_slug(s)) {
        return Some(format!(
            "has the segment {bad:?}, which cannot be put in a URI — lowercase letters, digits \
             and single interior hyphens only, the rule `name-not-a-slug` applies to a node's own \
             name"
        ));
    }
    if let Some(fragment) = r.fragment.as_deref() {
        return Some(format!(
            "has the fragment {fragment:?}, which is not what a `{kind}` fragment names — a \
             `crate` fragment names a code item or a file inside it, and every other kind's names \
             a declared property path"
        ));
    }
    if r.corpus
        .as_deref()
        .is_some_and(|c| !yidam_core::uri::is_slug(c))
    {
        return Some("names a corpus whose package name cannot be put in a URI".to_string());
    }
    if let Some(rev) = r.rev.as_deref() {
        return Some(format!(
            "pins the revision {rev:?}, which is not a commit or a tag"
        ));
    }
    // Every clause of `reference_conforms` is covered above. Reaching here means one was added
    // there and not here, and a finding with no reason is worse than none.
    Some("is not a reference the grammar can resolve".to_string())
}

pub const ORPHAN_IN: CheckSpec = CheckSpec {
    id: "orphan-in",
    title: "Node nothing points to",
    severity: Severity::Info,
    rationale: "Instances of a class the ontology declares no inbound edge for — from either end \
         of the edge — are exempt: nothing is meant to point at them, so an orphan there is \
         the model working. \
         What remains is a class whose *other* instances are cited and this one is not, \
         which is the asymmetry worth reading. Still reported rather than gated: a node \
         authored this morning legitimately has no inbound edges yet.",
};

pub fn orphan_in(nodes: &[Node], edges: &Edges, classes: &[Class]) -> Check {
    // Classes the ontology says nothing points at. Their instances are exempt: an orphan
    // there is the model holding, not the corpus failing.
    let exempt = source_classes(&edge_views(classes));

    let violations = nodes
        .iter()
        .enumerate()
        .filter(|(i, _)| edges.incoming(*i).is_empty())
        .map(|(_, n)| n)
        .filter(|n| !exempt.contains(&class_of(n)))
        .map(|n| Violation::new(&n.rel, "nothing links to this node"))
        .collect();
    Check::new(&ORPHAN_IN, violations)
}

pub const ONLY_INSTANCE_OF: CheckSpec = CheckSpec {
    id: "only-instance-of",
    title: "Node linked only to its class",
    severity: Severity::Warn,
    rationale: "A node whose only link is to its own class records that something exists without \
         relating it to anything — a stub, or what a bulk import left behind. Instances of a \
         class the ontology names only as a target are exempt: a leaf there is the model \
         working. What remains is a class that declares relationships its instances author, \
         and a node carrying none of them.",
};

/// A node whose every link is its own class membership (#1072).
///
/// `orphan-out` passes it — it has a link — and `orphan-in` reports it only when nothing
/// points back. It is well-formed, it parses, and it records that something exists without
/// relating it to anything: a stub, or what a bulk import leaves behind. One derived corpus
/// wrote a local check for exactly this after finding a set of them.
///
/// **Scoped by the ontology, not switched off by default.** The issue asked for either, since
/// some classes are legitimately leaves. The ontology already says which ones: a class some
/// declaration names, and never as the author, is a [`sink_classes`] class, and its instances are
/// exempt for the reason a source class's are exempt from `orphan-in`. Measured over sixteen
/// derived corpora (2,772 nodes, 2026-09-29): 79 nodes link only to their class, 28 are
/// instances of a sink class, and every one of the remaining 51 is an instance of a class
/// whose ontology says it authors a relationship the node does not — a `venue` declaring
/// `operated-by → partner` with no operator. That is a contradiction of the ontology the
/// corpus wrote, not a leaf, and the finding names the relationships so the repair is in it.
///
/// **Class membership is a link resolving to a `.ont.yml`**, not a relationship named
/// `instance-of`: the target is what makes it membership, and the relationship label is
/// the corpus's own. A node with no links is `orphan-out`'s, and one with a link missing its
/// `target:` or pointing at nothing is `dangling-edge`'s; neither is reported twice here. A
/// citation into the catalog is a link to something other than the class, so a node that
/// cites a source is not reported.
///
/// Warn rather than Error: a node authored this morning may not have its edges yet, and the
/// ratchet in [`super::super::baseline`] already tells an inherited finding from a new one.
pub fn only_instance_of(nodes: &[Node], edges: &Edges, classes: &[Class]) -> Check {
    let view = edge_views(classes);
    let exempt = sink_classes(&view);
    let authored = authored_relationships(&view);

    let violations = nodes
        .iter()
        .enumerate()
        .filter(|(i, n)| {
            let out = edges.out(*i);
            !out.is_empty()
                && out.len() == n.inst.links.as_deref().unwrap_or_default().len()
                && out.iter().all(|e| {
                    e.resolved
                        .file_name()
                        .is_some_and(|f| f.to_string_lossy().ends_with(".ont.yml"))
                })
        })
        .map(|(_, n)| n)
        .filter_map(|n| {
            let class = class_of(n);
            if exempt.contains(&class) {
                return None;
            }
            let detail = match authored.get(class.as_str()) {
                Some(rels) => format!(
                    "links only to its class; `{class}` declares {}",
                    rels.iter()
                        .map(|r| format!("`{r}`"))
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
                None => "links only to its class".to_string(),
            };
            Some(Violation::new(&n.rel, detail))
        })
        .collect();
    Check::new(&ONLY_INSTANCE_OF, violations)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cmd::lint::testkit::*;

    /// The names the mature corpora write. If this ever reddens, the rule changed and not the
    /// corpora — fourteen of the sixteen measured have no finding at all.
    #[test]
    fn the_names_corpora_actually_use_are_slugs() {
        for name in [
            "knowledge-graph",
            "hb96",
            "fy2024-district-profile",
            "twenty-mill-floor",
            "section-3",
            "a",
            "2024",
        ] {
            assert!(is_slug(name), "rejected a name in use: {name:?}");
        }
    }

    // ── references: the destination, and its reader ───────────────────────────

    fn node_with_references(entries: &[&str]) -> Node {
        let list = entries
            .iter()
            .map(|e| format!("  - {e}\n"))
            .collect::<String>();
        node(
            ".yidam/corpus/coupling/first-and-last-mile.yml",
            &format!("class: coupling\nlabel: First and last mile\nreferences:\n{list}"),
        )
    }

    /// The shapes a corpus will actually write, once the migration writes them.
    ///
    /// Drawn from the measured population (#783): an issue, a crate file, a crate item, a catalog
    /// entry and a decision. If any of these starts reporting, the grammar moved and the corpora
    /// did not.
    #[test]
    fn the_references_the_migration_will_write_are_all_clean() {
        let node = node_with_references(&[
            "issue/362",
            "crate/dispersion",
            "crate/dispersion#tests/the_weights.rs",
            "crate/dispersion#ohio_panel::equalization_by_year",
            "catalog/lsc-greenbook",
            "decision/the-four-kinds-of-parameter",
            "yidam://hegeomai/issue/111",
        ]);
        let check = reference_not_in_the_grammar(std::slice::from_ref(&node));
        assert!(
            check.violations.is_empty(),
            "a reference the corpora write was reported: {:?}",
            check.violations
        );
        // …and the fixture is not vacuous: the field was read.
        assert_eq!(node.inst.references.as_deref().unwrap_or(&[]).len(), 7);
    }

    /// Free text in the field is reported, and so is a reference that cannot survive a URI.
    #[test]
    fn an_entry_that_is_not_a_reference_and_one_that_cannot_be_a_uri_are_both_reported() {
        let node = node_with_references(&[
            "not a reference at all",
            "issue/362",
            "Disposition/DIS-2024-0447",
        ]);
        let check = reference_not_in_the_grammar(std::slice::from_ref(&node));
        assert_eq!(check.violations.len(), 2, "{:?}", check.violations);
        let messages: String = check
            .violations
            .iter()
            .map(|v| v.detail.clone())
            .collect::<Vec<_>>()
            .join("\n");
        // Free text has the wrong segment count for a node; the proto-002c id has the wrong
        // character set. Two different author errors, and the finding says which.
        assert!(
            messages.contains("path segment(s)"),
            "the free-text entry was not diagnosed by arity: {messages}"
        );
        assert!(
            messages.contains("cannot be put in a URI"),
            "the non-slug entry was not diagnosed by charset: {messages}"
        );
    }

    /// Each clause of `reference_conforms` produces a reason a reader can act on.
    ///
    /// The parser is total, so "does it parse" is not the test — `reference_conforms` is, and a
    /// finding that only said *not a reference* would name none of these four fixes.
    #[test]
    fn every_way_an_entry_fails_gets_its_own_reason() {
        for (entry, expect) in [
            ("issue/362", None),
            ("crate/dispersion#ohio_panel::equalization_by_year", None),
            ("issue/362/extra", Some("path segment(s)")),
            ("concept/Foo", Some("cannot be put in a URI")),
            ("concept/foo#Properties.Enacted", Some("fragment")),
            ("yidam://Producer/node/concept/foo", Some("package name")),
        ] {
            let why = why_not_a_reference(entry);
            match expect {
                None => assert!(why.is_none(), "{entry} reported: {why:?}"),
                Some(fragment) => {
                    let why = why.unwrap_or_else(|| panic!("{entry} was not reported at all"));
                    assert!(
                        why.contains(fragment),
                        "{entry} was reported as {why:?}, which does not name {fragment:?}"
                    );
                }
            }
        }
    }

    /// A node with no `references:` is not a node with an empty one, and neither reports.
    #[test]
    fn a_node_that_writes_no_references_is_silent() {
        for yaml in [
            "class: coupling\nlabel: One\n",
            "class: coupling\nlabel: One\nreferences: []\n",
        ] {
            let n = node(".yidam/corpus/coupling/one.yml", yaml);
            let check = reference_not_in_the_grammar(std::slice::from_ref(&n));
            assert!(check.violations.is_empty(), "{yaml:?}");
        }
    }

    /// And the names two corpora write that this check rejects.
    ///
    /// Recorded as a test rather than as a sentence in the rationale, because a measurement
    /// kept only in prose is one nobody re-runs. These are the exact segments `yidam lint`
    /// reports in `yidam-proto-001d` (6 findings) and `yidam-proto-002c` (16) — the two
    /// youngest corpora measured, and the two that took class names from their domain's own
    /// vocabulary in its own case.
    ///
    /// If a later change makes any of these pass, the check stopped reporting a real corpus
    /// and the docstrings above are describing a population that moved.
    #[test]
    fn the_names_two_corpora_write_are_rejected_and_that_is_the_cost() {
        for class in [
            // yidam-proto-001d — a water-quality corpus
            "DischargePoint",
            "LandUseCatchment",
            "MonitoringStation",
            "Reach",
            // yidam-proto-002c — a manufacturing corpus
            "NonConformance",
            "ToolInstance",
            "Workpiece",
        ] {
            assert!(!is_slug(class), "class name in use now passes: {class:?}");
        }
        // Node names, in one of the two. An uppercase segment survives the URI and then fails
        // to match a lowercased id, which is why these are findings and not style notes.
        for name in [
            "DIS-2024-0447",
            "MEAS-0883-030-BORE",
            "TI-7842",
            "L-2024-1147",
        ] {
            assert!(!is_slug(name), "node name in use now passes: {name:?}");
        }
    }

    /// Each rejected shape breaks a different consumer, so they are listed rather than
    /// collapsed: a space breaks the MCP round trip, `#` truncates the URI at a fragment, `%`
    /// is read as an escape that is not one, and a capital survives the URI and then fails to
    /// match a lowercased id.
    #[test]
    fn a_name_that_cannot_be_put_in_a_uri_is_rejected() {
        for name in [
            "two words",
            "has#fragment",
            "has%20escape",
            "HasCapital",
            "café",
            "trailing-",
            "-leading",
            "double--hyphen",
            "under_score",
            "dot.separated",
            "",
        ] {
            assert!(!is_slug(name), "admitted an unsafe name: {name:?}");
        }
    }

    #[test]
    fn stem_of_drops_both_extensions_and_the_directories() {
        assert_eq!(
            stem_of(".yidam/corpus/concept/knowledge-graph.yml"),
            "knowledge-graph"
        );
        assert_eq!(stem_of(".yidam/corpus/concept.ont.yml"), "concept");
        assert_eq!(stem_of("bare"), "bare");
    }

    /// The check reports the node against its own path, because the fix is to rename that file.
    #[test]
    fn a_bad_node_name_is_reported_against_its_file() {
        let good = node(
            ".yidam/corpus/concept/knowledge-graph.yml",
            "class: concept\n",
        );
        let bad = node(".yidam/corpus/concept/two words.yml", "class: concept\n");
        let c = name_not_a_slug(&[good, bad], &[]);
        assert_eq!(c.violations.len(), 1, "{:?}", c.violations);
        assert_eq!(c.violations[0].node, ".yidam/corpus/concept/two words.yml");
        assert!(
            c.violations[0].detail.contains("`two words`"),
            "{}",
            c.violations[0].detail
        );
    }

    /// A class name is the other half of the id, so it is checked and reported the same way.
    #[test]
    fn a_bad_class_name_is_reported_against_its_class_file() {
        let c = name_not_a_slug(&[], &[class(".yidam/corpus/My Class.ont.yml", "d")]);
        assert_eq!(c.violations.len(), 1, "{:?}", c.violations);
        assert!(
            c.violations[0].detail.contains("class name"),
            "{}",
            c.violations[0].detail
        );
    }

    /// A corpus of good names produces nothing — the state every derived corpus is in, and the
    /// reason this can be an Error.
    #[test]
    fn a_corpus_of_slugs_reports_nothing() {
        let c = name_not_a_slug(
            &[
                node(".yidam/corpus/concept/base-flow.yml", "class: concept\n"),
                node(".yidam/corpus/gage/usgs-03271601.yml", "class: gage\n"),
            ],
            &[class(".yidam/corpus/concept.ont.yml", "d")],
        );
        assert!(c.passed(), "{:?}", c.violations);
        assert_eq!(c.severity, Severity::Error);
    }

    /// The check must read the names it is given. A version that looked at nothing would pass
    /// every test above except this one.
    #[test]
    fn the_check_reads_every_node_and_class_it_is_given() {
        let c = name_not_a_slug(
            &[
                node(".yidam/corpus/concept/bad name.yml", "class: concept\n"),
                node(".yidam/corpus/concept/Another Bad.yml", "class: concept\n"),
            ],
            &[
                class(".yidam/corpus/Bad Class.ont.yml", "d"),
                class(".yidam/corpus/other bad.ont.yml", "d"),
            ],
        );
        assert_eq!(
            c.violations.len(),
            4,
            "one per offending name: {:?}",
            c.violations
        );
    }

    fn edge(relationship: &str, target: &str, direction: &str) -> crate::corpus::ClassEdge {
        crate::corpus::ClassEdge {
            relationship: relationship.into(),
            target: target.into(),
            direction: Some(direction.into()),
            description: String::new(),
        }
    }

    #[test]
    fn orphan_out_flags_a_node_with_no_links() {
        let nodes = vec![
            node("a.yml", "class: c\nlinks: []\n"),
            node("b.yml", "class: c\nlinks:\n  - target: a.yml\n"),
        ];
        let c = orphan_out(&nodes, &[]);
        assert_eq!(c.violations.len(), 1);
        assert_eq!(c.violations[0].node, "a.yml");
        assert_eq!(c.severity, Severity::Error);
    }

    #[test]
    fn orphan_out_skips_a_class_whose_outgoing_is_optional() {
        let nodes = vec![
            node("corpus/site/a.yml", "class: site\nlinks: []\n"),
            node("corpus/gage/b.yml", "class: gage\nlinks: []\n"),
        ];
        let classes = [
            Class::parse("site.ont.yml", "class: site\noutgoing: optional\n"),
            // A typo is not an exemption: anything but `optional` is `required`.
            Class::parse("gage.ont.yml", "class: gage\noutgoing: optinal\n"),
        ];
        let c = orphan_out(&nodes, &classes);
        assert_eq!(c.violations.len(), 1);
        assert_eq!(c.violations[0].node, "corpus/gage/b.yml");
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
        let a = node("corpus/reach/a.yml", "class: c\nlinks: []\n");
        let b = node(
            "corpus/other/b.yml",
            "class: c\nlinks:\n  - target: ../reach/a.yml\n",
        );
        let nodes = [a, b];
        let c = orphan_in(&nodes, &edges_of(&nodes), &[]);
        let flagged: Vec<&str> = c.violations.iter().map(|v| v.node.as_str()).collect();
        assert_eq!(
            flagged,
            vec!["corpus/other/b.yml"],
            "only b is unpointed-at"
        );
    }

    // ── only-instance-of (#1072) ─────────────────────────────────────────────

    /// The flagged nodes of an `only_instance_of` run, as `(node, detail)`.
    fn only_instance_of_findings(nodes: &[Node], classes: &[Class]) -> Vec<(String, String)> {
        only_instance_of(nodes, &edges_of(nodes), classes)
            .violations
            .into_iter()
            .map(|v| (v.node, v.detail))
            .collect()
    }

    const VENUE: &str =
        "edges:\n  - relationship: operated-by\n    target: partner\n    direction: out\n";

    /// The reported shape: a `venue` whose ontology says it is operated by a partner, and
    /// which links to nothing but its class. The finding names the relationship it lacks.
    #[test]
    fn a_node_linked_only_to_its_class_is_reported_with_what_its_class_declares() {
        let nodes = [node(
            "corpus/venue/hall.yml",
            "class: venue\nlinks:\n  - target: ../venue.ont.yml\n    relationship: instance-of\n",
        )];
        let c = only_instance_of(&nodes, &edges_of(&nodes), &[class_from("venue", VENUE)]);
        assert_eq!(c.severity, Severity::Warn);
        assert_eq!(
            only_instance_of_findings(&nodes, &[class_from("venue", VENUE)]),
            vec![(
                "corpus/venue/hall.yml".to_string(),
                "links only to its class; `venue` declares `operated-by`".to_string()
            )]
        );
    }

    /// Any other link — to a node or into the catalog — is a relation to something, so the
    /// node is not reported. Two nodes, so the flagged one is told apart from the other by
    /// the rule and not by being the only node present.
    #[test]
    fn a_link_to_anything_but_the_class_clears_the_node() {
        let nodes = [
            node(
                "corpus/venue/hall.yml",
                "class: venue\nlinks:\n  - target: ../venue.ont.yml\n    relationship: instance-of\n  - target: ../partner/parks.yml\n    relationship: operated-by\n",
            ),
            node(
                "corpus/venue/barn.yml",
                "class: venue\nlinks:\n  - target: ../venue.ont.yml\n    relationship: instance-of\n  - target: ../../catalog/deed.md\n    relationship: sources-from\n",
            ),
            node(
                "corpus/venue/shed.yml",
                "class: venue\nlinks:\n  - target: ../venue.ont.yml\n    relationship: instance-of\n",
            ),
            node(
                "corpus/partner/parks.yml",
                "class: partner\nlinks:\n  - target: ../partner.ont.yml\n    relationship: instance-of\n  - target: ../venue/hall.yml\n    relationship: operates\n",
            ),
        ];
        let flagged: Vec<String> = only_instance_of_findings(&nodes, &[class_from("venue", VENUE)])
            .into_iter()
            .map(|(n, _)| n)
            .collect();
        assert_eq!(flagged, vec!["corpus/venue/shed.yml"]);
    }

    /// Membership is decided by the target, not the relationship's name: a corpus that calls
    /// it `is-a` has still linked only to its class.
    #[test]
    fn membership_is_the_target_not_the_relationship_name() {
        let nodes = [node(
            "corpus/venue/hall.yml",
            "class: venue\nlinks:\n  - target: ../venue.ont.yml\n    relationship: is-a\n",
        )];
        assert_eq!(
            only_instance_of_findings(&nodes, &[class_from("venue", VENUE)]).len(),
            1
        );
    }

    /// A class that declares edges and authors none of them is a leaf by design — here
    /// `jurisdiction`, which `fund` points at from its own side. Its instances are exempt,
    /// and `fund`, which does author an edge, is not.
    #[test]
    fn instances_of_a_sink_class_are_exempt() {
        let classes = [
            class_from(
                "jurisdiction",
                "edges:\n  - relationship: appropriated-in\n    target: fund\n    direction: in\n",
            ),
            class_from(
                "fund",
                "edges:\n  - relationship: appropriated-in\n    target: jurisdiction\n    direction: out\n",
            ),
        ];
        let nodes = [
            node(
                "corpus/jurisdiction/ohio.yml",
                "class: jurisdiction\nlinks:\n  - target: ../jurisdiction.ont.yml\n    relationship: instance-of\n",
            ),
            node(
                "corpus/fund/grf.yml",
                "class: fund\nlinks:\n  - target: ../fund.ont.yml\n    relationship: instance-of\n",
            ),
        ];
        assert_eq!(
            only_instance_of_findings(&nodes, &classes),
            vec![(
                "corpus/fund/grf.yml".to_string(),
                "links only to its class; `fund` declares `appropriated-in`".to_string()
            )]
        );
    }

    /// A class with no `edges:` has said nothing, and silence is not an exemption — the same
    /// reading `orphan-in` gives it. The finding cannot name a relationship, so it does not.
    #[test]
    fn a_class_that_declares_no_edges_is_not_exempt() {
        let nodes = [node(
            "corpus/note/a.yml",
            "class: note\nlinks:\n  - target: ../note.ont.yml\n    relationship: instance-of\n",
        )];
        assert_eq!(
            only_instance_of_findings(&nodes, &[class_from("note", "label: Note\n")]),
            vec![(
                "corpus/note/a.yml".to_string(),
                "links only to its class".to_string()
            )]
        );
    }

    /// No links is `orphan-out`'s finding and a link with no `target:` is `dangling-edge`'s;
    /// neither is reported a second time here.
    #[test]
    fn another_checks_finding_is_not_reported_twice() {
        let nodes = [
            node("corpus/venue/bare.yml", "class: venue\nlinks: []\n"),
            node(
                "corpus/venue/half.yml",
                "class: venue\nlinks:\n  - target: ../venue.ont.yml\n    relationship: instance-of\n  - relationship: operated-by\n",
            ),
        ];
        assert!(only_instance_of_findings(&nodes, &[class_from("venue", VENUE)]).is_empty());
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
            text: String::new(),
            description: String::new(),
            name: name.into(),
            properties: vec![],
            edges: vec![edge("cited-by", "recording", dir)],
            label: String::new(),
            edge_policy: EdgePolicy::default(),
            outgoing_optional: false,
            max_lines: None,
            interval: None,
            prose: Vec::new(),
            implemented_by: None,
            foundational_type: None,
            dead_alignment_fields: vec![],
            malformed: None,
        };
        let instance = |class: &str, file: &str| {
            node(
                &format!("corpus/{class}/{file}.yml"),
                "class: c\nlinks: []\n",
            )
        };

        // `person` declares only outbound edges; `recording` declares an inbound one.
        let classes = [ont("person", "out"), ont("recording", "in")];
        let nodes = [instance("person", "harris"), instance("recording", "scum")];
        let c = orphan_in(&nodes, &edges_of(&nodes), &classes);

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
            text: String::new(),
            description: String::new(),
            name: "gage".into(),
            properties: vec![],
            edges: vec![edge("sources-from", "concept", "out")],
            label: String::new(),
            edge_policy: EdgePolicy::default(),
            outgoing_optional: false,
            max_lines: None,
            interval: None,
            prose: Vec::new(),
            implemented_by: None,
            foundational_type: None,
            dead_alignment_fields: vec![],
            malformed: None,
        };
        let concept = Class {
            rel: ".yidam/corpus/concept.ont.yml".into(),
            text: String::new(),
            description: String::new(),
            name: "concept".into(),
            properties: vec![],
            edges: vec![edge("refines", "concept", "out")],
            label: String::new(),
            edge_policy: EdgePolicy::default(),
            outgoing_optional: false,
            max_lines: None,
            interval: None,
            prose: Vec::new(),
            implemented_by: None,
            foundational_type: None,
            dead_alignment_fields: vec![],
            malformed: None,
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
            text: String::new(),
            description: String::new(),
            name: "reach".into(),
            properties: vec![],
            edges: vec![edge("downstream-of", "reach", "out")],
            label: String::new(),
            edge_policy: EdgePolicy::default(),
            outgoing_optional: false,
            max_lines: None,
            interval: None,
            prose: Vec::new(),
            implemented_by: None,
            foundational_type: None,
            dead_alignment_fields: vec![],
            malformed: None,
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
            text: String::new(),
            description: String::new(),
            name: "a".into(),
            properties: vec![],
            edges: vec![crate::corpus::ClassEdge {
                relationship: "relates-to".into(),
                target: "b".into(),
                direction: None,
                description: String::new(),
            }],
            label: String::new(),
            edge_policy: EdgePolicy::default(),
            outgoing_optional: false,
            max_lines: None,
            interval: None,
            prose: Vec::new(),
            implemented_by: None,
            foundational_type: None,
            dead_alignment_fields: vec![],
            malformed: None,
        };
        let b = Class {
            rel: ".yidam/corpus/b.ont.yml".into(),
            text: String::new(),
            description: String::new(),
            name: "b".into(),
            properties: vec![],
            edges: vec![edge("other", "c", "out")],
            label: String::new(),
            edge_policy: EdgePolicy::default(),
            outgoing_optional: false,
            max_lines: None,
            interval: None,
            prose: Vec::new(),
            implemented_by: None,
            foundational_type: None,
            dead_alignment_fields: vec![],
            malformed: None,
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
            text: String::new(),
            description: String::new(),
            name: "concept".into(),
            properties: vec![],
            edges: vec![],
            label: String::new(),
            edge_policy: EdgePolicy::default(),
            outgoing_optional: false,
            max_lines: None,
            interval: None,
            prose: Vec::new(),
            implemented_by: None,
            foundational_type: None,
            dead_alignment_fields: vec![],
            malformed: None,
        };
        assert!(
            source_classes(&edge_views(std::slice::from_ref(&silent))).is_empty(),
            "a class that declared nothing was read as declaring nothing points at it"
        );

        let nodes = [node("corpus/concept/x.yml", "class: c\nlinks: []\n")];
        let c = orphan_in(&nodes, &edges_of(&nodes), &[silent]);
        assert_eq!(
            c.violations.len(),
            1,
            "an undeclared class is still checked"
        );
    }

    /// A node whose entire substance is a transcription in a property is not a node with
    /// nothing said about it — which is what `missing-description` reported before #746.
    #[test]
    fn a_node_whose_only_prose_is_a_flagged_property_is_not_missing_one() {
        let n = node(
            "measure/m.yml",
            "class: measure\nlabel: L\nproperties:\n  verbatim: |\n    What the record says.\n",
        );
        assert!(
            !missing_description(std::slice::from_ref(&n), &declaring("measure", &[])).passed(),
            "unflagged, the property is invisible and the node reads as saying nothing"
        );
        assert!(
            missing_description(
                std::slice::from_ref(&n),
                &declaring_with("measure", &[], &["verbatim"])
            )
            .passed(),
            "flagged, the corpus has said what it knows and reporting it would use a \
             vocabulary the corpus declared"
        );
    }

    /// A node whose substance is in a declared field is not a node with nothing said about
    /// it. This was a false positive on every node of a corpus that writes `summary`.
    #[test]
    fn a_node_with_declared_prose_and_no_description_is_not_missing_it() {
        const YAML: &str = "class: finding\nlabel: L\nsummary: |\n  It says this.\n";
        assert_eq!(
            missing_description(&[node("finding/a.yml", YAML)], &Default::default())
                .violations
                .len(),
            1,
            "undeclared, `summary` is not prose and the node has none"
        );
        assert!(
            missing_description(
                &[node("finding/a.yml", YAML)],
                &declaring("finding", &["summary"])
            )
            .passed(),
            "declared, the node has said something"
        );
    }

    /// The finding names what it looked in, so a reader is not left guessing which field the
    /// check wanted.
    #[test]
    fn the_missing_prose_finding_names_the_declared_fields() {
        let n = node("finding/a.yml", "class: finding\nlabel: L\n");
        let c = missing_description(&[n], &declaring("finding", &["summary"]));
        assert!(
            c.violations[0].detail.contains("summary")
                && c.violations[0].detail.contains("description"),
            "{}",
            c.violations[0].detail
        );
    }

    /// A declared key holding something that is not a string is not prose. Guessing at a
    /// rendering for a list would put words in the corpus's mouth.
    #[test]
    fn a_declared_key_that_is_not_a_string_is_not_prose() {
        let n = node(
            "finding/a.yml",
            "class: finding\nlabel: L\nfindings:\n  - one\n  - two\n",
        );
        assert_eq!(
            missing_description(&[n], &declaring("finding", &["findings"]))
                .violations
                .len(),
            1
        );
    }
}
