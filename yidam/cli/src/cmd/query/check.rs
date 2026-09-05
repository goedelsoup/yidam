//! Typechecking a query against the ontology, per RFC-0018.
//!
//! The epic decision reads *"a query that does not typecheck against the schema is rejected
//! before it runs."* Applied literally that is the reading E1 **measured and rejected**: a
//! non-empty `edges:` says *these relationships exist*, not *and no others may*, and reading
//! it as the second put 210 errors on a corpus that was doing nothing wrong.
//!
//! So what is closed here is exactly what the ontology closed:
//!
//! | | |
//! |---|---|
//! | class names | closed — `unknown-class` is Error severity |
//! | relationships | closed only as far as `edge_policy` closed them |
//! | targets | closed where the class named one — `edge-target-class` is Error |
//! | property names | closed by the class, widened by `universal.yml` |
//! | property values | closed for `=` only; `!=` and `~` are satisfiable against values the type cannot hold |
//!
//! And the requirement that an unknown name never look like an empty result is met by a
//! **diagnostic**, not by a second rejection rule. A rejection rule strict enough to catch a
//! typo is also strict enough to refuse legal queries — the draft that tried it refused
//! relationships a class declares but no instance has authored yet, missed the near-miss it
//! existed for, and broke `--at`, where an earlier commit's vocabulary is strictly smaller.

use std::collections::{BTreeMap, BTreeSet};

use super::lang::{Dir, Op, Pred, Query, Step};
use crate::cmd::lint::checks::{Class, EdgePolicy};
use crate::universal::Universal;

/// A note about a step that ran anyway.
///
/// **Not lint's `Severity`.** A lint finding is keyed by a `&'static str` check id and a
/// corpus `node` path and carries `in_baseline`, which ties it to the baseline ratchet. A
/// query diagnostic has no check id, no node, and no baseline — it is about a *step*.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Diagnostic {
    /// `warn` or `info`. An error is not a diagnostic — it is the rejection.
    pub level: &'static str,
    pub step: usize,
    /// From a closed set, so a client can branch without matching prose.
    pub code: &'static str,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Rejection {
    pub step: Option<usize>,
    pub code: &'static str,
    pub message: String,
}

fn reject(code: &'static str, step: Option<usize>, message: impl Into<String>) -> Rejection {
    Rejection {
        step,
        code,
        message: message.into(),
    }
}

/// Everything the check reads about the corpus, gathered once.
pub struct Schema<'a> {
    pub classes: &'a [Class],
    pub universal: &'a Universal,
    /// Every `relationship:` the corpus authors, including citations and dangling links.
    ///
    /// Deliberately wider than the traversable set: a name written on a link into the
    /// catalog is still a name this corpus uses, and reporting it as "authored nowhere"
    /// would be false. `examples/streamflow` authors `instance-of` at the class file and
    /// `sourced-from` at the catalog, one character from the `sources-from` it declares.
    pub authored: BTreeMap<String, BTreeSet<String>>,
}

impl Schema<'_> {
    fn class(&self, name: &str) -> Option<&Class> {
        self.classes.iter().find(|c| c.name == name)
    }

    fn names(&self) -> Vec<&str> {
        self.classes.iter().map(|c| c.name.as_str()).collect()
    }

    /// Classes a step could match: one, or all of them for `*`.
    fn matching(&self, step: &Step) -> Vec<&Class> {
        match step.class.as_str() {
            "*" => self.classes.iter().collect(),
            name => self.class(name).into_iter().collect(),
        }
    }
}

/// The nearest declared name, for a diagnosis that names the near miss.
///
/// Edit distance 1, because the case it exists for — `sourced-from` against `sources-from` —
/// is exactly that, and a looser rule turns a useful note into noise.
fn near_miss<'a>(name: &str, mut candidates: impl Iterator<Item = &'a str>) -> Option<&'a str> {
    candidates.find(|c| edit_distance_1(name, c))
}

fn edit_distance_1(a: &str, b: &str) -> bool {
    if a == b {
        return false;
    }
    let (a, b): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    let (short, long) = match a.len() <= b.len() {
        true => (&a, &b),
        false => (&b, &a),
    };
    if long.len() - short.len() > 1 {
        return false;
    }
    let (mut i, mut j, mut budget) = (0usize, 0usize, 1i32);
    while i < short.len() && j < long.len() {
        if short[i] == long[j] {
            i += 1;
            j += 1;
            continue;
        }
        budget -= 1;
        if budget < 0 {
            return false;
        }
        if short.len() == long.len() {
            i += 1;
        }
        j += 1;
    }
    budget - ((long.len() - j) as i32) >= 0
}

fn nearest_class(name: &str, schema: &Schema) -> String {
    nearest(name, &schema.names())
}

/// The near-miss clause, over any set of declared names.
///
/// Shared with `retrieve`'s class filter (#334), which validates against the server's loaded
/// class stems rather than against a [`Schema`]. Two tools that reject the same misspelling
/// with two different sentences — or with two different notions of "near" — would be two
/// answers to one question, and the second copy is the one that stops matching the first.
pub(crate) fn nearest(name: &str, declared: &[&str]) -> String {
    match near_miss(name, declared.iter().copied()) {
        Some(near) => format!(" — did you mean `{near}`?"),
        None => {
            let mut names = declared.to_vec();
            names.sort_unstable();
            format!(" — declared classes: {}", names.join(", "))
        }
    }
}

// ── property predicates ───────────────────────────────────────────────────────

/// The declared type of a property on a class, or from `universal.yml`.
fn declared_type<'a>(class: &'a Class, prop: &str, universal: &'a Universal) -> Option<&'a str> {
    class
        .properties
        .iter()
        .find(|p| p.name == prop)
        .map(|p| p.r#type.as_str())
        .or_else(|| universal.declared_type(prop))
}

/// Check one predicate against one class.
///
/// `property_type_violation` answers *may the corpus store this*, not *may someone ask about
/// this*, and it takes no operator. Using it operator-blind rejects satisfiable predicates:
/// `reach[claim_tag!=maybe]` is satisfied by every reach in `examples/streamflow`, and
/// `reach[claim_tag~ope]` matches `open`.
fn check_pred(
    step_index: usize,
    class: &Class,
    pred: &Pred,
    universal: &Universal,
    out: &mut Vec<Diagnostic>,
) -> Result<(), Rejection> {
    let Some(declared) = declared_type(class, &pred.prop, universal) else {
        let mut names: Vec<&str> = class.properties.iter().map(|p| p.name.as_str()).collect();
        names.sort_unstable();
        return Err(reject(
            "undeclared-property",
            Some(step_index),
            format!(
                "`{}` is not a property `{}` declares{}",
                pred.prop,
                class.name,
                match names.is_empty() {
                    true => " — it declares none".to_string(),
                    false => format!(" — declared: {}", names.join(", ")),
                }
            ),
        ));
    };
    let as_yaml = serde_yaml::Value::String(pred.value.clone());
    let bad = crate::cmd::lint::checks::property_type_violation(declared, &as_yaml);
    match (pred.op, bad) {
        // `=` asks for a value the property can hold. Asking for one it cannot is a query
        // that could never match, which is a rejection and not an empty result.
        (Op::Eq, Some(why)) => Err(reject(
            "unsatisfiable-predicate",
            Some(step_index),
            format!("`{}` cannot hold that value: {why}", pred.prop),
        )),
        // `!=` excluding a value the type cannot hold is trivially true of every node that
        // carries the property. Legal, and worth saying.
        (Op::Ne, Some(why)) => {
            out.push(Diagnostic {
                level: "warn",
                step: step_index,
                code: "trivial-predicate",
                message: format!(
                    "`{} != {}` is true of every `{}` that carries the property: {why}",
                    pred.prop, pred.value, class.name
                ),
            });
            Ok(())
        }
        // `~` is substring over the serialized text and applies to every scalar type, so a
        // fragment that is not itself a legal value is exactly the point.
        _ => Ok(()),
    }
}

// ── hops ──────────────────────────────────────────────────────────────────────

/// What a class says about a relationship, once the empty-`edges:` case is separated out.
enum Licence {
    /// The class declares it, toward these classes. An empty entry licenses any target.
    Declared(Vec<String>),
    /// Silent: no `edges:` at all, so a policy bounds nothing.
    Silent,
    Undeclared(EdgePolicy),
}

fn licence(class: &Class, relationship: &str) -> Licence {
    let targets: Vec<String> = class
        .edges
        .iter()
        .filter(|e| e.relationship == relationship)
        .map(|e| e.target.clone())
        .collect();
    if !targets.is_empty() {
        return Licence::Declared(targets);
    }
    // The row that is easy to omit. `unlicensed_edge` short-circuits on an empty edge list
    // *before* it consults the policy (checks.rs:897), pinned by
    // `a_policy_without_edges_still_licenses_everything`. A table that consulted only the
    // policy would reject every hop out of a class that wrote `edge_policy: exhaustive` and
    // no `edges:` — a query engine stricter than the gate.
    match class.edges.is_empty() {
        true => Licence::Silent,
        false => Licence::Undeclared(class.edge_policy),
    }
}

/// Whether a declared target list admits the class at the other end.
fn target_admits(targets: &[String], other: &str) -> bool {
    // `*` on the query side is the twin of an empty `target:` in the ontology, and an empty
    // `target:` licenses every class exactly as `edge_target_class` reads it.
    other == "*" || targets.iter().any(|t| t.is_empty() || t == other)
}

/// Check one hop, from the authoring end.
///
/// A link lives on the node that wrote it, so `-rel->` is authored by the left class and
/// `<-rel-` by the right one. Which end authors decides whose `edges:` and whose
/// `edge_policy` govern.
#[allow(clippy::too_many_arguments)]
fn check_hop(
    hop_index: usize,
    relationship: &str,
    direction: Dir,
    from: &Step,
    to: &Step,
    schema: &Schema,
    out: &mut Vec<Diagnostic>,
) -> Result<(), Rejection> {
    let (author, other) = match direction {
        Dir::Out => (from, to),
        Dir::In => (to, from),
    };
    // The step a diagnostic hangs on is the one the hop leaves from, whichever end authors:
    // that is where a reader's eye is.
    let step_index = hop_index;
    let authors = schema.matching(author);
    if authors.is_empty() {
        return Ok(()); // an unschematised corpus; the class check already said so
    }

    let mut rejections = Vec::new();
    let mut licensed = false;
    let mut undeclared_by = Vec::new();
    for class in &authors {
        match licence(class, relationship) {
            Licence::Declared(targets) => match target_admits(&targets, &other.class) {
                true => licensed = true,
                false => rejections.push(format!(
                    "`{relationship}` is declared on `{}` toward {} and this hop asks for `{}`",
                    class.name,
                    targets
                        .iter()
                        .map(|t| format!("`{t}`"))
                        .collect::<Vec<_>>()
                        .join(" or "),
                    other.class
                )),
            },
            Licence::Silent => licensed = true,
            Licence::Undeclared(EdgePolicy::Exhaustive) => rejections.push(format!(
                "`{relationship}` is not declared by `{}`, whose vocabulary is `exhaustive`",
                class.name
            )),
            Licence::Undeclared(policy) => {
                licensed = true;
                if policy == EdgePolicy::Unstated {
                    undeclared_by.push(class.name.clone());
                }
            }
        }
    }

    if !licensed {
        // With `*` on the authoring side the hop is refused only when *every* class refuses
        // it; one class that admits the hop is enough to run.
        return Err(reject(
            "unlicensed-hop",
            Some(step_index),
            rejections.join("; "),
        ));
    }

    for class in undeclared_by {
        out.push(Diagnostic {
            level: "warn",
            step: step_index,
            code: "undeclared-relationship",
            message: format!(
                "`{relationship}` is not declared by `{class}` (edge_policy: unstated){}",
                authored_note(relationship, &class, schema)
            ),
        });
    }
    Ok(())
}

/// What the corpus actually does with a relationship the class does not declare.
///
/// The three situations #261 needs distinguishable from each other and from an empty result.
fn authored_note(relationship: &str, class: &str, schema: &Schema) -> String {
    let mut notes = Vec::new();

    // The near miss comes first, because it is the most useful thing anyone can be told and
    // because it is true whether or not the name is authored. `examples/streamflow` is
    // exactly that case: `sourced-from` is authored twice — into the catalog, so no
    // traversable edge carries it — while `gage` declares `sources-from`, one character
    // away. Reporting only the authorship leaves a reader with an empty result, a name the
    // corpus demonstrably uses, and no hint that the declared name is nearly the same.
    let declared: Vec<&str> = schema
        .class(class)
        .into_iter()
        .flat_map(|c| c.edges.iter().map(|e| e.relationship.as_str()))
        .collect();
    if let Some(near) = near_miss(relationship, declared.into_iter()) {
        notes.push(format!("`{near}` differs by one character"));
    }

    match schema.authored.get(relationship) {
        Some(classes) => {
            let mut names: Vec<&str> = classes.iter().map(String::as_str).collect();
            names.sort_unstable();
            notes.push(format!("it is authored by {}", names.join(", ")));
        }
        None => notes.push("no node in this corpus authors it".to_string()),
    }
    format!(", and {}", notes.join("; "))
}

// ── the check ─────────────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct Checked {
    pub diagnostics: Vec<Diagnostic>,
    /// Classes each step may match, after `*` narrowing. Index-aligned with the steps.
    pub narrowed: Vec<Vec<String>>,
    /// True when the corpus declared no classes, so no class name was checked at all.
    ///
    /// Carried on the *verdict* rather than read off the graph at each use site, because it
    /// is the one fact that makes an `Ok` mean two different things: a query that typechecked
    /// and a query nobody typechecked. `at::divergence` compares two verdicts, and without
    /// this it read the second as the first — reporting no divergence across the commit where
    /// the ontology arrived, which is where it moved most.
    pub unschematised: bool,
}

pub fn check(query: &Query, schema: &Schema) -> Result<Checked, Rejection> {
    let mut diagnostics = Vec::new();
    let mut narrowed = Vec::new();

    // A corpus with no `.ont.yml` at all has no schema layer, which is a different problem
    // from a misspelling — the carve-out `unknown_class` itself makes (checks.rs:383).
    let unschematised = schema.classes.is_empty();

    for (index, step) in query.steps.iter().enumerate() {
        if !unschematised && step.class != "*" && schema.class(&step.class).is_none() {
            return Err(reject(
                "unknown-class",
                Some(index),
                format!(
                    "`{}` is not a class this corpus declares{}",
                    step.class,
                    nearest_class(&step.class, schema)
                ),
            ));
        }

        // A property predicate narrows `*` to the classes that declare the property. If no
        // class declares it the query is rejected, exactly as a named class would be —
        // without this, `*[typo]` returns an empty result at exit 0 while `reach[typo]` is
        // rejected, which is the same typo with two answers.
        let mut classes: Vec<String> = schema
            .matching(step)
            .iter()
            .map(|c| c.name.clone())
            .collect();
        for pred in &step.filter {
            let mut kept = Vec::new();
            let mut last: Option<Rejection> = None;
            for name in &classes {
                let Some(class) = schema.class(name) else {
                    continue;
                };
                match check_pred(index, class, pred, schema.universal, &mut diagnostics) {
                    Ok(()) => kept.push(name.clone()),
                    Err(rejection) => last = Some(rejection),
                }
            }
            if kept.is_empty() {
                return Err(last.unwrap_or_else(|| {
                    reject(
                        "undeclared-property",
                        Some(index),
                        format!("no class declares `{}`", pred.prop),
                    )
                }));
            }
            if kept.len() < classes.len() && step.class == "*" {
                let mut skipped: Vec<&String> =
                    classes.iter().filter(|c| !kept.contains(c)).collect();
                skipped.sort();
                diagnostics.push(Diagnostic {
                    level: "info",
                    step: index,
                    code: "narrowed",
                    message: format!(
                        "`*` narrowed to the classes declaring `{}`; skipped {}",
                        pred.prop,
                        skipped
                            .iter()
                            .map(|c| format!("`{c}`"))
                            .collect::<Vec<_>>()
                            .join(", ")
                    ),
                });
            }
            classes = kept;
        }
        narrowed.push(classes);
    }

    for (index, hop) in query.hops.iter().enumerate() {
        let (from, to) = query.ends(index);
        check_hop(
            index,
            &hop.relationship,
            hop.direction,
            from,
            to,
            schema,
            &mut diagnostics,
        )?;
    }

    Ok(Checked {
        diagnostics,
        narrowed,
        unschematised,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cmd::query::lang::parse;

    /// A class from literal YAML.
    ///
    /// `load_classes` reads files; these tests want literals, so the bytes are handed to the
    /// loader's own builder. This used to assemble the struct here and transcribe the
    /// `edge_policy` match by hand — a fourth answer to what a class is, of exactly the kind
    /// [`Class::parse`] exists to prevent, and one field behind the real one for as long as it
    /// stood.
    fn class(text: &str) -> Class {
        let name = serde_yaml::from_str::<serde_yaml::Value>(text)
            .ok()
            .and_then(|v| v.get("class")?.as_str().map(str::to_string))
            .unwrap_or_default();
        Class::parse(format!("{name}.ont.yml"), text)
    }

    const REACH: &str = "class: reach\nproperties:\n  - name: regulated\n    type: string\n  \
                         - name: claim_tag\n    type: claim\nedges:\n  - relationship: \
                         measured-by\n    target: gage\n    direction: out\n";
    const GAGE: &str = "class: gage\nproperties:\n  - name: parameter\n    type: string\n\
                        edges:\n  - relationship: sources-from\n    target: concept\n    \
                        direction: out\n";
    const CONCEPT: &str = "class: concept\nproperties:\n  - name: claim_tag\n    type: claim\n";

    struct Fixture {
        classes: Vec<Class>,
        universal: Universal,
        authored: BTreeMap<String, BTreeSet<String>>,
    }

    impl Fixture {
        fn new(classes: Vec<Class>) -> Self {
            Self {
                classes,
                universal: Universal::empty(),
                authored: BTreeMap::new(),
            }
        }

        fn authoring(mut self, relationship: &str, classes: &[&str]) -> Self {
            self.authored.insert(
                relationship.to_string(),
                classes.iter().map(|c| c.to_string()).collect(),
            );
            self
        }

        fn schema(&self) -> Schema<'_> {
            Schema {
                classes: &self.classes,
                universal: &self.universal,
                authored: self.authored.clone(),
            }
        }
    }

    fn streamflow() -> Fixture {
        Fixture::new(vec![class(REACH), class(GAGE), class(CONCEPT)])
            .authoring("measured-by", &["reach"])
            .authoring("sources-from", &["gage"])
            .authoring("sourced-from", &["gage"])
    }

    // ── classes ───────────────────────────────────────────────────────────────

    #[test]
    fn an_undeclared_class_is_rejected_with_the_nearest_name() {
        let f = streamflow();
        let e = check(&parse("gauge").unwrap(), &f.schema()).unwrap_err();
        assert_eq!(e.code, "unknown-class");
        assert!(e.message.contains("did you mean `gage`"), "{}", e.message);
    }

    #[test]
    fn a_corpus_with_no_ontology_does_not_have_its_class_names_checked() {
        let f = Fixture::new(vec![]);
        assert!(check(&parse("anything").unwrap(), &f.schema()).is_ok());
    }

    // ── the edge_policy ladder ────────────────────────────────────────────────

    #[test]
    fn a_declared_relationship_toward_a_declared_target_is_licensed() {
        let f = streamflow();
        let checked = check(&parse("reach -measured-by-> gage").unwrap(), &f.schema()).unwrap();
        assert!(checked.diagnostics.is_empty());
    }

    #[test]
    fn a_declared_relationship_toward_the_wrong_class_is_rejected() {
        let f = streamflow();
        let e = check(&parse("reach -measured-by-> concept").unwrap(), &f.schema()).unwrap_err();
        assert_eq!(e.code, "unlicensed-hop");
        assert!(e.message.contains("asks for `concept`"), "{}", e.message);
    }

    #[test]
    fn an_undeclared_relationship_on_an_exhaustive_class_is_rejected() {
        let mut f = streamflow();
        f.classes[0] = class(&format!("{REACH}edge_policy: exhaustive\n"));
        let e = check(&parse("reach -bears-on-> gage").unwrap(), &f.schema()).unwrap_err();
        assert_eq!(e.code, "unlicensed-hop");
        assert!(e.message.contains("exhaustive"), "{}", e.message);
    }

    #[test]
    fn an_undeclared_relationship_on_a_characteristic_class_runs_silently() {
        let mut f = streamflow();
        f.classes[0] = class(&format!("{REACH}edge_policy: characteristic\n"));
        let checked = check(&parse("reach -bears-on-> gage").unwrap(), &f.schema()).unwrap();
        assert!(checked.diagnostics.is_empty(), "{:?}", checked.diagnostics);
    }

    #[test]
    fn an_undeclared_relationship_on_an_unstated_class_runs_with_a_warning() {
        let f = streamflow();
        let checked = check(&parse("reach -bears-on-> gage").unwrap(), &f.schema()).unwrap();
        let d = &checked.diagnostics[0];
        assert_eq!(d.level, "warn");
        assert_eq!(d.code, "undeclared-relationship");
    }

    /// The row a policy-only table gets wrong. `unlicensed_edge` short-circuits on an empty
    /// `edges:` *before* it reads the policy, so a class that wrote `exhaustive` and no
    /// edges licenses everything under the gate — and a query engine stricter than the gate
    /// is the thing RFC-0018 forbids.
    #[test]
    fn a_policy_without_edges_licenses_every_hop_here_as_it_does_at_the_gate() {
        let f = Fixture::new(vec![
            class("class: reach\nedges: []\nedge_policy: exhaustive\n"),
            class(GAGE),
        ]);
        let checked = check(&parse("reach -whatever-> gage").unwrap(), &f.schema()).unwrap();
        assert!(checked.diagnostics.is_empty());
    }

    /// The withdrawn rule, pinned as withdrawn: a relationship the class declares but no
    /// instance has authored yet must run, not be refused as a typo.
    #[test]
    fn a_declared_but_unauthored_relationship_is_not_refused() {
        let mut f = streamflow();
        f.authored.remove("measured-by");
        assert!(check(&parse("reach -measured-by-> gage").unwrap(), &f.schema()).is_ok());
    }

    /// The near miss the rejection rule could not catch, because `sourced-from` *is*
    /// authored: a diagnostic names it, and the query still runs.
    #[test]
    fn a_near_miss_is_named_even_when_the_name_is_authored() {
        // `examples/streamflow`'s real shape, and the case the withdrawn rejection rule
        // could not catch: `sourced-from` *is* authored — as a catalog citation, so no
        // traversable edge carries it — while `gage` declares `sources-from` one character
        // away. Both facts have to be said, or an empty result comes back with no hint.
        let f = Fixture::new(vec![class(GAGE), class(CONCEPT)])
            .authoring("sources-from", &["gage"])
            .authoring("sourced-from", &["gage"]);
        let checked = check(&parse("gage -sourced-from-> concept").unwrap(), &f.schema()).unwrap();
        let d = &checked.diagnostics[0];
        assert!(
            d.message
                .contains("`sources-from` differs by one character"),
            "{}",
            d.message
        );
        assert!(d.message.contains("authored by gage"), "{}", d.message);
    }

    #[test]
    fn a_relationship_nobody_authors_says_so_rather_than_returning_a_bare_empty_set() {
        let f = streamflow();
        let checked = check(&parse("reach -unrelated-verb-> gage").unwrap(), &f.schema()).unwrap();
        assert!(
            checked.diagnostics[0]
                .message
                .contains("no node in this corpus authors it"),
            "{}",
            checked.diagnostics[0].message
        );
    }

    // ── direction ─────────────────────────────────────────────────────────────

    /// The same edge from the other end. `reach` authors `measured-by`, so a backward hop
    /// out of `gage` is licensed by `reach`'s declaration, not by `gage`'s.
    #[test]
    fn a_backward_hop_is_checked_against_the_class_that_authors_it() {
        let f = streamflow();
        assert!(check(&parse("gage <-measured-by- reach").unwrap(), &f.schema()).is_ok());
        // And the forward spelling out of `gage` is not licensed: `gage` does not author it.
        let mut strict = streamflow();
        strict.classes[1] = class(&format!("{GAGE}edge_policy: exhaustive\n"));
        assert!(check(
            &parse("gage -measured-by-> reach").unwrap(),
            &strict.schema()
        )
        .is_err());
    }

    // ── predicates ────────────────────────────────────────────────────────────

    #[test]
    fn an_undeclared_property_is_rejected_with_the_declared_list() {
        let f = streamflow();
        let e = check(&parse("reach[depth=3]").unwrap(), &f.schema()).unwrap_err();
        assert_eq!(e.code, "undeclared-property");
        assert!(e.message.contains("claim_tag, regulated"), "{}", e.message);
    }

    #[test]
    fn equality_against_a_value_the_type_cannot_hold_is_rejected() {
        let f = streamflow();
        let e = check(&parse("reach[claim_tag=maybe]").unwrap(), &f.schema()).unwrap_err();
        assert_eq!(e.code, "unsatisfiable-predicate");
        assert!(e.message.contains("not an evidence tag"), "{}", e.message);
    }

    /// Operator-blind checking rejected this, and it is satisfied by every reach in
    /// `examples/streamflow`.
    #[test]
    fn inequality_against_a_value_the_type_cannot_hold_runs_with_a_warning() {
        let f = streamflow();
        let checked = check(&parse("reach[claim_tag!=maybe]").unwrap(), &f.schema()).unwrap();
        assert_eq!(checked.diagnostics[0].code, "trivial-predicate");
    }

    /// `~` is substring over the serialized text, so a fragment that is not itself a legal
    /// value is the point rather than an error.
    #[test]
    fn containment_against_a_fragment_is_legal_on_every_scalar_type() {
        let f = streamflow();
        assert!(check(&parse("reach[claim_tag~ope]").unwrap(), &f.schema()).is_ok());
    }

    // ── `*` ───────────────────────────────────────────────────────────────────

    #[test]
    fn a_star_with_a_predicate_narrows_to_the_classes_declaring_it() {
        let f = streamflow();
        let checked = check(&parse("*[claim_tag=open]").unwrap(), &f.schema()).unwrap();
        assert_eq!(checked.narrowed[0], vec!["reach", "concept"]);
        assert_eq!(checked.diagnostics[0].code, "narrowed");
        assert!(checked.diagnostics[0].message.contains("`gage`"));
    }

    /// The asymmetry that would otherwise reopen the failure this whole section closes:
    /// `*[typo]` returning empty at exit 0 while `reach[typo]` is rejected.
    #[test]
    fn a_star_with_a_property_no_class_declares_is_rejected_like_a_named_class() {
        let f = streamflow();
        let e = check(&parse("*[regualted=yes]").unwrap(), &f.schema()).unwrap_err();
        assert_eq!(e.code, "undeclared-property");
    }

    #[test]
    fn a_star_hop_is_refused_only_when_every_class_refuses_it() {
        let mut f = streamflow();
        f.classes[0] = class(&format!("{REACH}edge_policy: exhaustive\n"));
        f.classes[1] = class(&format!("{GAGE}edge_policy: exhaustive\n"));
        // Every class exhaustive, so nothing is licensed by silence or by an unstated
        // policy — `concept` alone declares `bears-on`.
        f.classes[2] = class(&format!(
            "{CONCEPT}edges:\n  - relationship: bears-on\n    target: gage\n    \
             direction: out\nedge_policy: exhaustive\n"
        ));
        // One class licensing it is enough for `*` to run.
        assert!(check(&parse("* -bears-on-> gage").unwrap(), &f.schema()).is_ok());
        // Nothing licenses this one, so every class refuses and so does the query.
        assert!(check(&parse("* -nobody-has-this-> gage").unwrap(), &f.schema()).is_err());
    }

    #[test]
    fn a_star_target_is_licensed_the_way_an_empty_declared_target_is() {
        let f = streamflow();
        assert!(check(&parse("reach -measured-by-> *").unwrap(), &f.schema()).is_ok());
    }

    #[test]
    fn edit_distance_one_is_what_it_says() {
        assert!(edit_distance_1("sources-from", "sourced-from"));
        assert!(edit_distance_1("gage", "gauge"));
        assert!(edit_distance_1("abc", "ab"));
        assert!(!edit_distance_1("abc", "abc"));
        assert!(!edit_distance_1("abc", "xyz"));
        assert!(!edit_distance_1("abc", "a"));
    }
}
