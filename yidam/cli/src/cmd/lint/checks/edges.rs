use super::*;

pub const DANGLING_EDGE: CheckSpec = CheckSpec {
    id: "dangling-edge",
    title: "Edge pointing at nothing",
    severity: Severity::Error,
    rationale: "A broken edge is worse than a missing one: it asserts a relationship to something \
         that is not there, so a reader following it learns nothing and a traversal counting \
         it overstates the graph's connectivity. Renaming a node severs every edge into it, \
         which is why node filenames are meant to be stable.",
};

/// An edge whose target is not there.
///
/// Decided on the resolved graph (#925) rather than on a `dir.join(target)` written here, so
/// this check and `orphan-in` mean the same thing by *resolves* — they used to write the
/// join out separately, and only one of them normalized.
///
/// **A node the corpus holds is never dangling, whether or not it is a file.** That is the
/// `to.is_some()` arm, and it is the difference between a gate and an editor: `serve --lsp`
/// reads unsaved buffers as nodes, so a link to a node somebody has typed and not yet saved
/// resolves to a path that does not exist. On disk the two arms agree — a node came from the
/// walk — so nothing about a `lint` run changes.
pub fn dangling_edge(nodes: &[Node], edges: &Edges) -> Check {
    let mut violations = Vec::new();
    for (i, n) in nodes.iter().enumerate() {
        // The resolved edges, in the order the file wrote them, with the targetless links
        // skipped — which is exactly the ones reported below. Advanced by `index` rather
        // than zipped, so the two orders cannot slide past each other.
        let mut resolved = edges.out(i).iter().peekable();
        for (index, link) in n.inst.links.iter().flatten().enumerate() {
            let Some(target) = link.target.as_deref() else {
                violations.push(Violation::new(&n.rel, "link entry with no `target:`"));
                continue;
            };
            let Some(edge) = resolved.next_if(|e| e.index == index) else {
                continue;
            };
            if edge.to.is_none() && !edge.resolved.exists() {
                violations.push(Violation::new(
                    &n.rel,
                    format!("target does not exist: {target}"),
                ));
            }
        }
    }
    Check::new(&DANGLING_EDGE, violations).spanning_links()
}

pub const UNLICENSED_EDGE: CheckSpec = CheckSpec {
    id: "unlicensed-edge",
    title: "Relationship the class does not declare",
    // Warn is the level for a class that has not declared a policy, which is every
    // class written before the field existed. `exhaustive` raises its own findings to
    // Error per violation, so one corpus can hold both kinds at once.
    severity: Severity::Warn,
    rationale: "An edge is licensed by the class that authors it, and `edge_policy:` says whether \
         that list bounds the vocabulary or describes it. `exhaustive` closes it and makes \
         anything outside an error; `characteristic` says an undeclared relationship is a \
         deliberate coinage, and none are reported. A class that has said neither is warned \
         about but not gated — a relationship in no declaration is worth seeing, because a \
         traversal that walks by relationship will not find it, but a non-empty `edges:` on \
         its own never claimed to be complete. Only links landing on another instance are \
         read: a link to the class file or into the catalog is a citation, not a \
         relationship. A class that declares no `edges:` has said nothing and is not \
         checked.",
};

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
pub fn unlicensed_edge(nodes: &[Node], edges: &Edges, classes: &[Class]) -> Check {
    let by_name = classes_by_name(classes);
    let mut violations = Vec::new();
    for (i, n) in nodes.iter().enumerate() {
        let Some(class) = by_name.get(class_of(n).as_str()) else {
            continue;
        };
        // Silence about the *edges* is still silence, exactly as before: a class with no
        // `edges:` has named no relationships, and a policy on an empty list bounds nothing.
        if class.edges.is_empty() || class.edge_policy == EdgePolicy::Characteristic {
            continue;
        }
        for (edge, _) in edges.instance_links(i) {
            let rel = edge
                .written_as(n)
                .relationship
                .as_deref()
                .unwrap_or_default();
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
    Check::new(&UNLICENSED_EDGE, violations).spanning_links()
}

pub const EDGE_TARGET_CLASS: CheckSpec = CheckSpec {
    id: "edge-target-class",
    title: "Edge that resolves to a node of the wrong class",
    severity: Severity::Error,
    rationale: "`dangling-edge` catches an edge to nothing. This catches an edge to the wrong \
         thing — which resolves, traverses, and exports, and is simply false. The class \
         names the target class for each relationship it declares; a declaration that \
         names none licenses any target.",
};

/// Licensed edges that land on a node of the wrong class.
///
/// This is the finding no existing check could produce. `dangling-edge` catches an edge to
/// nothing; nothing caught an edge to the wrong thing, and an edge to the wrong thing
/// resolves, traverses, and exports — it is simply false.
pub fn edge_target_class(nodes: &[Node], edges: &Edges, classes: &[Class]) -> Check {
    let by_name = classes_by_name(classes);
    let mut violations = Vec::new();
    for (i, n) in nodes.iter().enumerate() {
        let Some(class) = by_name.get(class_of(n).as_str()) else {
            continue;
        };
        for (edge, t) in edges.instance_links(i) {
            let (link, to) = (edge.written_as(n), &nodes[t]);
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
    Check::new(&EDGE_TARGET_CLASS, violations).spanning_links()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cmd::lint::testkit::*;

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
            let c = unlicensed_edge(&nodes, &edges_of(&nodes), &classes);
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
        let c = edge_target_class(&nodes, &edges_of(&nodes), &classes);
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
        assert!(unlicensed_edge(&nodes, &edges_of(&nodes), &classes).passed());
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
        let c = unlicensed_edge(&nodes, &edges_of(&nodes), &classes);
        assert_eq!(c.violations.len(), 1);
        assert!(
            !c.gates(&c.violations[0]),
            "a typo must not close a vocabulary"
        );
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
        assert!(unlicensed_edge(&nodes, &edges_of(&nodes), &classes).passed());
        assert!(edge_target_class(&nodes, &edges_of(&nodes), &classes).passed());
        assert!(undeclared_property(&nodes, &classes, &NONE).passed());
    }

    #[test]
    fn a_relationship_the_class_does_not_declare_is_reported() {
        let (nodes, classes) = gage_corpus(
            "class: gage\nproperties:\n  parameter: \"00060\"\n  claim_tag: open\nlinks:\n  - target: ../concept/hydropeaking.yml\n    relationship: sourcs-from\n",
        );
        let c = unlicensed_edge(&nodes, &edges_of(&nodes), &classes);
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
        assert!(unlicensed_edge(&nodes, &edges_of(&nodes), &classes).passed());
        assert!(edge_target_class(&nodes, &edges_of(&nodes), &classes).passed());
    }

    /// A broken edge is `dangling-edge`'s finding, reported once. It resolves to no node,
    /// so it is not licensed here and is not reported twice.
    #[test]
    fn a_broken_edge_is_not_also_reported_as_unlicensed() {
        let (nodes, classes) = gage_corpus(
            "class: gage\nproperties:\n  parameter: \"00060\"\n  claim_tag: open\nlinks:\n  - target: ../concept/gone.yml\n    relationship: sourcs-from\n",
        );
        assert!(unlicensed_edge(&nodes, &edges_of(&nodes), &classes).passed());
        assert_eq!(dangling_edge(&nodes, &edges_of(&nodes)).violations.len(), 1);
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
        let c = edge_target_class(&nodes, &edges_of(&nodes), &classes);
        assert_eq!(c.violations.len(), 1, "{c:#?}");
        assert!(c.violations[0].detail.contains("a gage"), "{c:#?}");
        // …and the relationship itself is declared, so it is not also unlicensed.
        assert!(unlicensed_edge(&nodes, &edges_of(&nodes), &classes).passed());
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
        assert!(edge_target_class(&nodes, &edges_of(&nodes), &classes).passed());
    }
}
