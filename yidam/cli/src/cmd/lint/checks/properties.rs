use super::*;

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
pub(super) fn classes_by_name(classes: &[Class]) -> HashMap<&str, &Class> {
    classes.iter().map(|c| (c.name.as_str(), c)).collect()
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

pub const UNDECLARED_PROPERTY: CheckSpec = CheckSpec {
    id: "undeclared-property",
    title: "Instance property the class never declared",
    severity: Severity::Error,
    rationale: "A property absent from the class is invisible to everything that reads the corpus \
         through the ontology — schemas, exports, queries. It reads as data to a human and \
         as nothing to a machine, which is the failure mode worth gating on. A class that \
         declares no `properties:` has said nothing and is not checked, and a property \
         `.yidam/corpus/universal.yml` declares — by name or by pattern — may be carried by \
         any class without being declared on each.",
};

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
    Check::new(&UNDECLARED_PROPERTY, violations)
}

pub const NODE_TOO_LONG: CheckSpec = CheckSpec {
    id: "node-too-long",
    title: "Instance longer than its class allows",
    severity: Severity::Warn,
    rationale:
        "A long node is usually two nodes, or a node carrying quoted source that belongs in \
         the catalog entry it cites. The ceiling is the class's own — a class that declares \
         no `max_lines:` is not checked, because the length an instance should be is a \
         question about that class and not about corpora in general. What is counted is the \
         `description` block and not the file: the argument is about prose, and a node that \
         records where each of its edges comes from should not pay for that provenance out \
         of a budget written to stop descriptions sprawling. A corpus that raised \
         `max_lines:` to absorb structural lines can lower it again.",
};

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
/// **And the unit is the `description`, not the file.** This counted `n.text` — the bytes as
/// read, frontmatter, properties and links included — while its own rationale argues about
/// prose. The two came apart as soon as a corpus started recording where its edges come from:
/// a `claim_tag` and a `source` on each link costs about a line and a half per edge, so a
/// node that documents its provenance paid for it out of a budget written to stop
/// *descriptions* sprawling. One derived corpus reported 212 findings, 207 of them one class
/// whose fixed structure — frontmatter, two properties, three tagged links — is 18 lines
/// before a word of prose, against a ceiling of 25. Its `max_lines` was raised 42 → 60 and
/// the share over it went *up*, from 11 of 40 to 207 of 238, which is the evidence that the
/// number was never what was wrong.
///
/// A node with no description is skipped rather than falling back to the file. There is
/// nothing to be too long, and falling back would charge structure to a node that wrote no
/// prose at all — the defect, reintroduced for the nodes it hits hardest.
///
/// The harness's S7 still counts the file, and this no longer claims to agree with it. #595
/// made S7 explicitly a genesis-only number, scored once against a corpus a bootstrap has
/// just produced and never re-run; structure overhead is small there. `max_lines:` is a
/// steady-state contract a class keeps for its whole life, and structure overhead is exactly
/// what grows over one.
///
/// Warn rather than Error even when declared. Length is editorial, the ratchet in
/// [`super::super::baseline`] already distinguishes inherited from new, and a node one line over is
/// not a corpus that has stopped being true.
pub fn node_too_long(
    nodes: &[Node],
    classes: &[Class],
    prose: &crate::prose::ProseFields,
) -> Check {
    let by_name = classes_by_name(classes);
    let mut violations = Vec::new();
    for n in nodes {
        let Some(class) = by_name.get(class_of(n).as_str()) else {
            continue;
        };
        let Some(max) = class.max_lines else {
            continue;
        };
        // Every declared prose field and not `description` alone: a corpus writing
        // `summary` and `findings` was measured at 21 lines against a true 34 (#674), which
        // charged the ceiling to the one field the struct happened to name.
        //
        // **And the properties a class flagged, since #746.** The top-level reading missed
        // 14.5% of what the measured corpora write — a fifth of all block-scalar prose, most
        // of it under `properties` — so a ceiling read a node shorter than it is. Where that
        // changes a verdict it is because the node was always that long.
        let text = crate::prose::text(&n.inst, prose, &class_of(n));
        if text.is_empty() {
            continue;
        }
        let lines = text.lines().count();
        if lines <= max {
            continue;
        }
        violations.push(Violation::new(
            &n.rel,
            format!(
                "{lines} lines of prose; `{}` declares `max_lines: {max}`",
                class.rel
            ),
        ));
    }
    Check::new(&NODE_TOO_LONG, violations)
}

/// The most passages a node's composed text may split into before `node-too-many-passages`
/// reports it (RFC-0060 §2.6). About 38,000 characters, the order of the remote row ceiling.
pub(crate) const MAX_PASSAGES: usize = 32;

pub const NODE_TOO_MANY_PASSAGES: CheckSpec = CheckSpec {
    id: "node-too-many-passages",
    title: "Node too long to embed as one",
    severity: Severity::Warn,
    rationale:
        "A long node is indexed as many passages, so it stays findable, but past a point its \
         length says it is several things. Usually one is an artifact about the entity — a \
         plan, a brief, a decision log — which belongs in a node of its own linking to it, \
         or an account that grew to argue several claims, each of which is a claim node. \
         The measure is the text `yidam embed` composes, not the file, and it applies to \
         every class, whether or not the class declares `max_lines:`.",
};

/// A node whose embedding text splits into more than [`MAX_PASSAGES`] passages.
///
/// The text is composed exactly as `yidam embed` composes it, from the same prose and
/// retrievable declarations, and split by the same SDK function `index-build` uses. So a
/// count here is the number of rows the node will be in the index.
pub fn node_too_many_passages(
    nodes: &[Node],
    prose: &crate::prose::ProseFields,
    retrievable: &crate::retrievable::Retrievable,
) -> Check {
    let mut violations = Vec::new();
    for n in nodes {
        if n.malformed.is_some() {
            continue;
        }
        let class = class_of(n);
        let text = yidam_core::embed::compose_embed_text(
            &n.inst,
            &crate::retrievable::fields(prose, retrievable, &class),
        );
        let label = n.inst.label.as_deref().unwrap_or("");
        let passages =
            yidam_core::embed::split_passages(label, &text, yidam_core::embed::PASSAGE_MAX_CHARS)
                .len();
        if passages <= MAX_PASSAGES {
            continue;
        }
        violations.push(Violation::new(
            &n.rel,
            format!(
                "`{class}` node of {passages} passages ({} characters), past {MAX_PASSAGES}; \
                 a plan, brief or log about it is a node of its own, and an account arguing \
                 several claims is several claim nodes",
                text.chars().count(),
            ),
        ));
    }
    Check::new(&NODE_TOO_MANY_PASSAGES, violations)
}

pub const MISSING_PROPERTY: CheckSpec = CheckSpec {
    id: "missing-property",
    title: "Declared property the instance omits",
    severity: Severity::Warn,
    rationale: "The commonest cause is a class that grew a field its instances never did — \
         invisible to every reader and obvious in a list. A property declared \
         `required: true` GATES: the class wrote that contract, and an instance omitting it \
         contradicts the ontology, which is what this check's four siblings gate on. \
         `required: false` is not reported: the class said an instance may omit it, and \
         an omission the class licensed is not a finding. A property that says neither is \
         reported and does not gate, because a class that said nothing about a property \
         cannot be read as demanding it — a node that makes no tagged claim is a real \
         state, not a defect, and gating on omission would assert a contract nobody wrote.",
};

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
            // `required: false` is the class saying an instance may omit this, and an
            // omission it licensed is not a finding (#1055) — `edge_policy: characteristic`'s
            // shape. Before this, the one corpus that had answered `required:` on every
            // property drew all 36 of its warnings from the 8 it had marked `false`, and the
            // deliberate omissions were indistinguishable from the ones nobody decided.
            // Silence stays reported: absent means the ontology was never asked.
            //
            // The severity is a function of the DECLARATION, not a blanket judgement about
            // omissions — the shape `orphan-in` already has, where residence time rather
            // than the check's level decides. A class that said `required: true` has
            // written the contract this instance contradicts, and contradiction is what the
            // other four checks gate on. A class that said nothing has not, and gating
            // there would assert a contract the ontology never wrote.
            //
            // Read through `omission()` because `graph` reports the same verdict to editors,
            // and a second derivation is how the two came to disagree (#1155).
            let (qualifier, escalated) = match declared.omission() {
                Omission::Licensed => continue,
                Omission::Gates => (" as `required: true`", Some(Severity::Error)),
                Omission::Reported => ("", None),
            };
            let violation = Violation::new(
                &n.rel,
                format!(
                    "`{}` is declared by `{}`{qualifier} and this instance does not carry it",
                    declared.name, class.rel,
                ),
            );
            violations.push(match escalated {
                Some(severity) => violation.at(severity),
                None => violation,
            });
        }
    }
    Check::new(&MISSING_PROPERTY, violations)
}

/// One `struct` or `enum` the tree defines, and where.
///
/// Built by [`super::super::run_checks_with`] from a walk of `crates/`, and taken as an argument
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

pub const UNIMPLEMENTED_CLASS: CheckSpec = CheckSpec {
    id: "unimplemented-class",
    title: "Class naming an implementation the code does not define",
    severity: Severity::Error,
    rationale: "A class declaring `implemented_by:` has stated a fact about `crates/`, and a tree \
         with no type of that name contradicts it — which is what this check's siblings \
         gate on, and an omission is not. A class that declares nothing is not checked, and \
         that silence is measured rather than timid: across twelve derived corpora 129 of \
         157 declared classes have no type bearing their name, and matching every language \
         in the tree raises it to 88%. Their ontologies model a domain and their code models \
         the pipeline that gathers evidence about it, so a class without a type is the \
         normal case and reporting it would call four fifths of every ontology debt. A \
         rename is invisible: the tree is scanned rather than a diff, so a class implemented \
         under a new name is simply found.",
};

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
    Check::new(&UNIMPLEMENTED_CLASS, violations)
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
        // **A number is a YAML number, unquoted** (RFC-0040). The mirror of the `string`
        // arm's "quote it": a corpus that writes `length_km: "24"` has stored text that
        // happens to spell a number, and the compiled schema — `{"type": "number"}` — would
        // refuse the file while this gate waved it through. The one reading a number gets is
        // [`numeric_value`], which `query`'s ordering shares for the reason `iso_date_parts`
        // gives: a second parser is a corpus where `lint` accepts what `query` will not order.
        "number" => match value {
            serde_yaml::Value::Number(n) => match numeric_value(&n.to_string()) {
                Some(_) => None,
                None => Some(format!("`{n}` is not a finite number")),
            },
            serde_yaml::Value::String(s) => match numeric_value(s) {
                Some(_) => Some(format!("`{}` is text, not a number — unquote it", s.trim())),
                None => Some(format!("`{}` is not a number", s.trim())),
            },
            serde_yaml::Value::Bool(_) => Some("is a boolean, not a number".to_string()),
            other => scalar(other).err(),
        },
        // **A value that names its document** (RFC-0046). The shape is all this can test —
        // whether the words are in the bytes is `quotation-span-drift`'s question, and needs
        // the catalog and the vault cache — and it is read by the one parser those checks
        // read with, so a value this admits is one they can resolve.
        super::super::quotations::QUOTATION_PROPERTY_TYPE => {
            super::super::quotations::read(value).err()
        }
        // **A list, as a bare `list` declares it** (RFC-0050): a non-empty sequence of
        // anything. What `items:` or `fields:` add is [`declared_violation`]'s, because they
        // live on the declaration and this predicate is handed only its type.
        LIST_PROPERTY_TYPE => match value {
            serde_yaml::Value::Sequence(items) if items.is_empty() => {
                Some("is an empty list — leave the property out instead".to_string())
            }
            serde_yaml::Value::Sequence(_) => None,
            serde_yaml::Value::Null => Some("is empty".to_string()),
            serde_yaml::Value::Mapping(_) => {
                Some("is a mapping, not a list — write it as a one-item list".to_string())
            }
            _ => Some("is not a list — write it as a one-item list".to_string()),
        },
        _ => None,
    }
}

/// The declared type a value may hold several of (RFC-0050).
pub(crate) const LIST_PROPERTY_TYPE: &str = "list";

/// Every `type:` a reader grades: the scalar item types, and the two that are not one.
/// [`property_type_unknown`] reports any other; the test
/// `known_types_are_exactly_the_graded_ones` holds this list to [`property_type_violation`].
pub(crate) const KNOWN_PROPERTY_TYPES: [&str; 8] = [
    "string",
    "text",
    "ref",
    "date",
    "number",
    crate::claims::CLAIM_PROPERTY_TYPE,
    super::super::quotations::QUOTATION_PROPERTY_TYPE,
    LIST_PROPERTY_TYPE,
];

/// Why a `list` declaration constrains nothing, or `None` when it is well formed.
///
/// `items:` and `fields:` together say two things about one item; an `items:` outside
/// [`yidam_core::ontology::LIST_ITEM_TYPES`] names a list, a quotation or a coined type; a
/// field typed `list` or `quotation` is one the readers that walk records do not reach. Each
/// is reported once, on the class, by [`list_declaration_invalid`] — and an instance under it
/// is held only to the bare `list` shape, because a declaration that cannot be read cannot
/// be held to either half of what it says. The compiled schema says the same: `true`.
pub(crate) fn list_declaration_problem(p: &crate::corpus::ClassProperty) -> Option<String> {
    if p.r#type != LIST_PROPERTY_TYPE {
        return None;
    }
    let item_type = |t: &str| yidam_core::ontology::LIST_ITEM_TYPES.contains(&t);
    let kinds = yidam_core::ontology::LIST_ITEM_TYPES.join("`, `");
    if !p.items.is_empty() && !p.fields.is_empty() {
        return Some(
            "declares both `items:` and `fields:` — an item is a scalar or a record, not both"
                .into(),
        );
    }
    if !p.items.is_empty() && !item_type(&p.items) {
        return Some(format!(
            "declares `items: {}` — an item is one of `{kinds}`, or a record declared with `fields:`",
            p.items
        ));
    }
    p.fields
        .iter()
        .find(|f| matches!(f.r#type.as_str(), LIST_PROPERTY_TYPE | super::super::quotations::QUOTATION_PROPERTY_TYPE))
        .map(|f| {
            format!(
                "declares the field `{}` as `{}` — a record field is one of `{kinds}`, or a type the corpus coined",
                f.name, f.r#type
            )
        })
}

/// What an instance value violates in its full declaration, or `None`.
///
/// [`property_type_violation`] reads a type; this reads the declaration, which is what a
/// `string`'s `values:` and a `list`'s `items:` and `fields:` need. The shape first, then the
/// set: a value that is not text is reported for what it is, not also for being outside a
/// list of text.
pub(crate) fn declared_violation(
    p: &crate::corpus::ClassProperty,
    value: &serde_yaml::Value,
) -> Option<String> {
    let why = property_type_violation(&p.r#type, value);
    if why.is_some() {
        return why;
    }
    match p.r#type.as_str() {
        "string" => declared_value_violation(&p.values, value),
        LIST_PROPERTY_TYPE if list_declaration_problem(p).is_none() => {
            let serde_yaml::Value::Sequence(entries) = value else {
                return None;
            };
            entries.iter().enumerate().find_map(|(i, entry)| {
                let at = i + 1;
                if !p.items.is_empty() {
                    item_violation(p, entry).map(|why| format!("item {at} {why}"))
                } else if !p.fields.is_empty() {
                    record_violation(&p.fields, entry).map(|why| format!("item {at} {why}"))
                } else {
                    None
                }
            })
        }
        _ => None,
    }
}

/// One item of an `items:` list, held to the scalar arm its type names and to the list's set.
fn item_violation(p: &crate::corpus::ClassProperty, item: &serde_yaml::Value) -> Option<String> {
    // The one-or-a-list tolerance is the property's: an item is one tag, and a list inside
    // a list is the shape being wrong, whatever the claim arm would accept on its own.
    if let serde_yaml::Value::Sequence(_) = item {
        return Some("is a list — an item is one value".into());
    }
    property_type_violation(&p.items, item).or_else(|| {
        (p.items == "string")
            .then(|| declared_value_violation(&p.values, item))
            .flatten()
    })
}

/// One item of a `fields:` list: a mapping, closed against keys no field names, carrying
/// every field declared `required: true`, and each field held to its own declaration.
fn record_violation(
    fields: &[crate::corpus::ClassProperty],
    entry: &serde_yaml::Value,
) -> Option<String> {
    let serde_yaml::Value::Mapping(map) = entry else {
        return Some("is not a record — each item is a mapping of the declared fields".into());
    };
    for key in map.keys() {
        let name = key.as_str().unwrap_or_default();
        if !fields.iter().any(|f| f.name == name) {
            let named = fields
                .iter()
                .map(|f| format!("`{}`", f.name))
                .collect::<Vec<_>>()
                .join(", ");
            return Some(format!(
                "has `{}`, which no field declares — the fields are {named}",
                serde_yaml::to_string(key).unwrap_or_default().trim()
            ));
        }
    }
    fields.iter().find_map(|f| match map.get(f.name.as_str()) {
        None if f.required() => Some(format!("has no `{}`, which every item carries", f.name)),
        None => None,
        Some(v) => declared_violation(f, v)
            .map(|why| format!("has `{}` declared `{}`, which {why}", f.name, f.r#type)),
    })
}

/// A `string` outside the closed set its class declares (RFC-0044), or `None`.
///
/// `pub(crate)` for `migrate`, for the reason [`property_type_violation`] is: a retype into
/// `string` on a property that declares a set is a retype into this predicate, and a
/// migration that did not consult it would be a migration into a failing build.
///
/// Only text is tested. A list, a mapping or a bare number is the *shape* being wrong, and
/// [`property_type_violation`] has already said so — reporting it a second time for being
/// outside a list of text would be two findings for one mistake. An empty set is no set at
/// all: a closed set of nothing would refuse every value, which no class means by
/// `values: []`. The match is exact, as JSON Schema's `enum` is: the compiled schema refuses
/// `In operation` against `in operation`, and a gate that admitted it would be looser than
/// the schema it exists to be no stricter than.
pub(crate) fn declared_value_violation(
    values: &[String],
    value: &serde_yaml::Value,
) -> Option<String> {
    if values.is_empty() {
        return None;
    }
    let serde_yaml::Value::String(s) = value else {
        return None;
    };
    if values.iter().any(|v| v == s) {
        return None;
    }
    let set = values
        .iter()
        .map(|v| format!("`{v}`"))
        .collect::<Vec<_>>()
        .join(", ");
    Some(format!(
        "`{s}` is not one of the values the class declares — {set}"
    ))
}

/// The value of a `number`, or `None` when the text is not one.
///
/// The whole of the `number` arm's rule, returning what it read, so that an ordering over
/// numbers and the check that admits them cannot disagree about which values are numbers —
/// [`iso_date_parts`]'s reason, one type over. Finite only: YAML spells `.inf` and `.nan`,
/// and neither is a quantity a corpus measured. There is no precision rule here and none is
/// wanted — `1893` denotes an interval, which is why dates have one; `7` and `7.0` denote
/// the same point, which is why `=` on a number compares numerically (RFC-0040).
pub fn numeric_value(s: &str) -> Option<f64> {
    s.trim().parse::<f64>().ok().filter(|n| n.is_finite())
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
    iso_date_parts(s).is_some()
}

/// The components of an ISO date, in order, or `None` when it is not one.
///
/// The whole of [`is_iso_date`]'s rule, returning what it read instead of discarding it, so
/// that a comparison over dates and the check that admits them cannot come to disagree about
/// which strings are dates. `query`'s ordering operators are the second reader (#725), and a
/// second parser there would be a corpus where `lint` accepts a value that `query` will not
/// order — the quietest possible split.
///
/// Every component is fixed-width and zero-padded, so a lexical comparison of the parts is a
/// numeric one, and the caller needs no integers.
pub fn iso_date_parts(s: &str) -> Option<Vec<&str>> {
    let mut parts = s.split('-');
    // Year, then optionally month, then optionally day. A trailing separator leaves an
    // empty part, which fails the width test rather than passing as absent — `1985-` is
    // not a year.
    let widths = [4, 2, 2];
    let mut seen = Vec::with_capacity(3);
    for w in widths {
        match parts.next() {
            None => break,
            Some(p) if p.len() == w && p.chars().all(|c| c.is_ascii_digit()) => seen.push(p),
            Some(_) => return None,
        }
    }
    match !seen.is_empty() && parts.next().is_none() {
        true => Some(seen),
        false => None,
    }
}

/// Order two ISO dates **at the precision they share**, or `None` if either is not a date.
///
/// The corpus writes a date to whatever precision it knows it — `is_iso_date`'s own rationale
/// records `formed: "1985"` occurring 71 times in one derived corpus — so a comparison has to
/// say what it does when the two sides disagree about precision. This one drops both to the
/// coarser: `1893-04-01 < 1900` is decided on the years alone.
///
/// **This is deliberately not `=`'s rule, and the divergence is the point.** `=` compares at
/// the precision the *query* wrote, so `born=1893-04-01` does not match `born: 1893` — a
/// day-precision question the corpus cannot answer. Ordering at the query's precision would
/// make `born<=1900-01-01` skip every person whose birth year alone is known, silently, in
/// the direction of a smaller answer. An equality asked more precisely than the corpus knows
/// is genuinely unanswerable; an ordering usually is not — `1893` is before `1900-01-01`
/// whatever day it fell on — and answering it is the reason the operator exists.
///
/// The consequence to know: where the corpus is coarser than the query, `<=` and `>=` can
/// both hold for a value `=` rejects. When the two sides carry the same precision the three
/// are trichotomous, which is the case a caller reasons about.
///
/// **Here rather than in `query`, because it has a second reader.** [`interval_overlap`]
/// orders a class's declared start and end by it (#1201), and a second rule there would be a
/// corpus where `query` says a holder was in office on a day and `lint` says nobody was.
/// Equality at the shared precision is what an overlap check must not guess past: `1893`
/// and `1893-06-01` compare equal, and so a term ending `1893` and one beginning
/// `1893-06-01` are adjacent rather than overlapping.
pub fn compare_dates(left: &str, right: &str) -> Option<std::cmp::Ordering> {
    let (left, right) = (iso_date_parts(left.trim())?, iso_date_parts(right.trim())?);
    let shared = left.len().min(right.len());
    Some(left[..shared].cmp(&right[..shared]))
}

pub const PROPERTY_TYPE: CheckSpec = CheckSpec {
    id: "property-type",
    title: "Property value that contradicts its declared type",
    severity: Severity::Error,
    rationale: "A typed field the ontology declares is read by type, not by eye. A `claim` field \
         holding prose is counted as no claim at all; a `string` holding an unquoted number \
         has lost its leading zeros before anything reads it; a `number` holding a quoted \
         one is text the compiled schema refuses and an ordering cannot compare. A `string` \
         whose class declares `values:` is held to that set: a value outside it is either a \
         widening the class never recorded or prose in a token field, and both are reported \
         so the set the class publishes stays true. A `list` is a non-empty list, and \
         each item is held to the `items:` type or the `fields:` record its class declares: \
         a record is closed as the class is, so a misspelled `claim_tg` is reported rather \
         than read as an entry with no standing. Types the corpus coins for itself are left \
         alone — this reports only the ones it can test, and `property-type-unknown` names \
         the rest.",
};

/// Property values that do not satisfy the type the class declares.
///
/// `claim` is the type that pays for this check on its own. `claims.rs` matches the three
/// evidence tokens exactly, so a field declared `type: claim` and filled with prose is
/// counted as **no claim at all** — a node that reads as evidenced to a human and as bare
/// assertion to every counter. That is the same silent-undercount failure
/// `claim-tag-malformed` reports in prose, one field over, and the matcher is reused rather
/// than re-derived so the two cannot disagree about what a tag is.
///
/// **A `string` declaring `values:` is held to that set** (RFC-0044), and it rides here
/// rather than in a check of its own because it is the same finding: a value the declaration
/// does not admit. Before the field existed a class spelled its set in the description —
/// `plant | refinery | canal` — where nothing reads it, and across seven corpora 40
/// properties did so while 33 instances held a value outside the set their own class
/// documented. A class that declares the set has asked for the gate, as one declaring
/// `edge_policy: exhaustive` has; a class that declares none is unbounded, as before.
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
            // The class first, then `universal.yml` — the one precedence rule
            // [`crate::universal::Universal::declared_type_for`] states, so this and the
            // other three readers of it cannot disagree about which values are typed.
            let Some(declared) = universal.declared_type_for(Some(*class), &key) else {
                continue;
            };
            // The class's declaration whole, when the class made it: a `string`'s set and a
            // `list`'s items and fields are the class's own, and `universal.yml` declares a
            // type and nothing more.
            let why = match class.properties.iter().find(|p| p.name == key) {
                Some(p) => declared_violation(p, value),
                None => property_type_violation(declared, value),
            };
            if let Some(why) = why {
                violations.push(Violation::new(
                    &n.rel,
                    format!("`{key}` is declared `{declared}` and {why}"),
                ));
            }
        }
    }
    Check::new(&PROPERTY_TYPE, violations)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cmd::lint::testkit::*;

    // ── node-too-long ─────────────────────────────────────────────────────────

    /// A class of `name` whose instances may be `max` lines, or any length when `None`.
    fn capped(name: &str, max: Option<usize>) -> Class {
        Class {
            rel: format!(".yidam/corpus/{name}.ont.yml"),
            text: String::new(),
            description: String::new(),
            name: name.into(),
            properties: vec![],
            edges: vec![],
            label: String::new(),
            edge_policy: EdgePolicy::default(),
            outgoing_optional: false,
            max_lines: max,
            interval: None,
            prose: Vec::new(),
            implemented_by: None,
            foundational_type: None,
            dead_alignment_fields: vec![],
            malformed: None,
        }
    }

    // ── node-too-many-passages ────────────────────────────────────────────────

    /// A `measure` node whose description is `lines` lines of about a hundred characters.
    fn long_measure(lines: usize) -> Node {
        let body: String = (0..lines)
            .map(|i| format!("  line {i:04} of an account that has grown well past what one passage holds, and keeps going\n"))
            .collect();
        node(
            "measure/m.yml",
            &format!("class: measure\nlabel: Long\ndescription: |\n{body}"),
        )
    }

    fn passages_of(n: &Node) -> usize {
        let text = yidam_core::embed::compose_embed_text(
            &n.inst,
            &crate::retrievable::fields(
                &declaring("measure", &[]),
                &crate::retrievable::Retrievable::default(),
                "measure",
            ),
        );
        yidam_core::embed::split_passages("Long", &text, yidam_core::embed::PASSAGE_MAX_CHARS).len()
    }

    /// RFC-0060 §2.8: it fires on a node of 33 passages and stays silent on one of 32.
    #[test]
    fn a_node_past_thirty_two_passages_is_reported_and_one_at_it_is_not() {
        let at = |want: usize| {
            (1..2_000)
                .map(long_measure)
                .find(|n| passages_of(n) == want)
                .expect("some line count lands on each passage count")
        };
        let check = |n: Node| {
            node_too_many_passages(
                std::slice::from_ref(&n),
                &declaring("measure", &[]),
                &crate::retrievable::Retrievable::default(),
            )
        };
        assert!(check(at(MAX_PASSAGES)).passed());
        let c = check(at(MAX_PASSAGES + 1));
        assert_eq!(c.violations.len(), 1);
        let detail = &c.violations[0].detail;
        assert!(detail.contains("`measure` node of 33 passages"), "{detail}");
        assert!(
            detail.contains("characters") && detail.contains("a node of its own"),
            "{detail}"
        );
        assert!(c.violations[0].node.ends_with("measure/m.yml"));
    }

    /// #746, in one assertion: a fifth of what the corpora write is under `properties`, and
    /// the ceiling was reading past it. `properties.method` is a block scalar on 341 nodes
    /// across 214 KB in the measured population, and none of it counted.
    #[test]
    fn a_flagged_property_counts_toward_the_ceiling() {
        let n = node(
            "measure/m.yml",
            "class: measure\nlabel: L\ndescription: |\n  one\n  two\nproperties:\n  method: |\n    three\n    four\n    five\n",
        );
        let classes = [capped("measure", Some(4))];
        assert!(
            node_too_long(
                std::slice::from_ref(&n),
                &classes,
                &declaring("measure", &[])
            )
            .passed(),
            "unflagged, the node reads as two lines and is under the ceiling"
        );
        let c = node_too_long(
            std::slice::from_ref(&n),
            &classes,
            &declaring_with("measure", &[], &["method"]),
        );
        assert_eq!(
            c.violations.len(),
            1,
            "flagged, it is five lines against four"
        );
        assert!(
            c.violations[0].detail.contains("5 lines of prose"),
            "reports what it measured: {}",
            c.violations[0].detail
        );
    }

    /// #674, in one assertion: the same node measures 3 lines or 7 depending on whether the
    /// check reads `description` or the prose the corpus actually declared. The band was
    /// fitted on one number and the ceiling gated on the other.
    #[test]
    fn a_ceiling_counts_every_declared_prose_field() {
        const YAML: &str = "class: finding\nlabel: L\nsummary: |\n  s1\n  s2\ndescription: |\n  d1\n  d2\n  d3\nfindings: |\n  f1\n  f2\n";
        let classes = [capped("finding", Some(5))];
        assert!(
            node_too_long(
                &[node("finding/a.yml", YAML)],
                &classes,
                &Default::default()
            )
            .passed(),
            "reading `description` alone the node is 3 lines and under the ceiling"
        );
        let c = node_too_long(
            &[node("finding/a.yml", YAML)],
            &classes,
            &declaring("finding", &["summary", "findings"]),
        );
        assert_eq!(
            c.violations.len(),
            1,
            "7 lines of declared prose, ceiling 5"
        );
        assert!(
            c.violations[0].detail.contains("7 lines of prose"),
            "and it names the unit it counted: {}",
            c.violations[0].detail
        );
    }

    /// An instance of `class` that is `lines` lines long.
    fn sized(rel: &str, class: &str, lines: usize) -> Node {
        let mut yaml = format!("class: {class}\nlabel: L\ndescription: |\n");
        for _ in 0..lines {
            yaml.push_str("  filler\n");
        }
        let n = node(rel, &yaml);
        assert_eq!(
            n.inst
                .description
                .as_deref()
                .unwrap_or_default()
                .lines()
                .count(),
            lines,
            "the helper sizes the description, which is what the check counts"
        );
        n
    }

    /// A class that declares no ceiling is not checked, however long its instances are.
    ///
    /// The whole design in one assertion. Porting the bootstrap rubric's 40 here would
    /// report against 335 of the 410 nodes in the five corpora this was measured on.
    #[test]
    fn a_class_that_declares_no_ceiling_is_not_checked() {
        let nodes = vec![sized("person/a.yml", "person", 200)];
        assert!(
            node_too_long(&nodes, &[capped("person", None)], &Default::default()).passed(),
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
        let c = node_too_long(&nodes, &[capped("person", Some(40))], &Default::default());
        assert_eq!(c.violations.len(), 1, "only the long one is over");
        assert_eq!(c.violations[0].node, "person/long.yml");
        assert!(
            c.violations[0].detail.contains("52 lines of prose")
                && c.violations[0].detail.contains("max_lines: 40"),
            "the finding must carry both numbers and name the unit, not just a verdict: {}",
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
        assert!(node_too_long(&nodes, &[capped("person", Some(40))], &Default::default()).passed());
    }

    /// One class's ceiling says nothing about another's.
    #[test]
    fn a_ceiling_binds_only_the_class_that_declared_it() {
        let nodes = vec![sized("statute/a.yml", "statute", 60)];
        let classes = vec![capped("person", Some(10)), capped("statute", None)];
        assert!(
            node_too_long(&nodes, &classes, &Default::default()).passed(),
            "statute declared nothing; person's number is not corpus-wide"
        );
    }

    /// The reported defect: a node paid for its own provenance out of its prose allowance.
    ///
    /// The description here is four lines and the ceiling is six. What made this node long
    /// is frontmatter, two properties and three links carrying a `claim_tag` and a `source`
    /// apiece — the structure a corpus grows when it records where its edges come from, and
    /// exactly what a budget written against sprawling *prose* has no business charging for.
    #[test]
    fn structure_is_not_charged_against_the_prose_budget() {
        let yaml = "class: tenure\n\
                    label: A holder\n\
                    description: |\n  \
                      Who held the office, from when, and until when.\n  \
                      The dates are those recorded in the minute book.\n  \
                      Two predecessors are named in the same entry.\n  \
                      The ending is recorded as a resignation.\n\
                    properties:\n  \
                      began: 1913-04-02\n  \
                      ended: 1919-11-30\n\
                    links:\n\
                    - rel: held\n  \
                      target: ../office/registrar.yml\n  \
                      claim_tag: verified\n  \
                      source: ../../catalog/minute-book-1913.md\n\
                    - rel: succeeded\n  \
                      target: ../tenure/prior.yml\n  \
                      claim_tag: verified\n  \
                      source: ../../catalog/minute-book-1913.md\n\
                    - rel: recorded_in\n  \
                      target: ../document/minutes.yml\n  \
                      claim_tag: verified\n  \
                      source: ../../catalog/minute-book-1913.md\n";
        let n = node("tenure/a.yml", yaml);
        assert!(
            n.text.lines().count() > 20,
            "the file is long, which is the point of the fixture"
        );
        assert!(
            node_too_long(&[n], &[capped("tenure", Some(6))], &Default::default()).passed(),
            "four lines of prose is under a six-line ceiling, whatever the file weighs"
        );
    }

    /// A node with no description is skipped, not measured against its file.
    ///
    /// Falling back to `text` would reintroduce the defect precisely where it bites hardest:
    /// a node that is all structure and no prose.
    #[test]
    fn a_node_with_no_description_is_not_reported() {
        let yaml = "class: person\nlabel: A\nlinks:\n- rel: knows\n  target: ../person/b.yml\n";
        let n = node("person/a.yml", yaml);
        assert!(n.inst.description.is_none());
        assert!(node_too_long(&[n], &[capped("person", Some(1))], &Default::default()).passed());
    }

    // ── unimplemented-class ───────────────────────────────────────────────────

    /// A class of `name` that says `impl_by` implements it, or says nothing.
    fn implemented(name: &str, impl_by: Option<&str>) -> Class {
        Class {
            rel: format!(".yidam/corpus/{name}.ont.yml"),
            text: String::new(),
            description: String::new(),
            name: name.into(),
            properties: vec![],
            edges: vec![],
            label: String::new(),
            edge_policy: EdgePolicy::default(),
            outgoing_optional: false,
            max_lines: None,
            interval: None,
            prose: Vec::new(),
            implemented_by: impl_by.map(str::to_string),
            foundational_type: None,
            dead_alignment_fields: vec![],
            malformed: None,
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
        let class = Class::parse(".yidam/corpus/a.ont.yml", "implemented_by: '   '");
        assert_eq!(class.implemented_by, None);
        assert!(unimplemented_class(&[class], &tree(&[])).passed());
    }

    #[test]
    fn a_declaration_is_read_off_the_class_file_and_trimmed() {
        assert_eq!(
            Class::parse(
                ".yidam/corpus/a.ont.yml",
                "implemented_by: '  Intervention  '"
            )
            .implemented_by,
            Some("Intervention".to_string())
        );
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

    /// `required:` has three answers and they are three severities (#1055): `true` gates,
    /// silence is reported, and `false` is not a finding at all.
    ///
    /// `false` and silence both mean an instance may omit the property, so neither gates.
    /// What differs is whether anyone decided. A corpus that answered `required:` on every
    /// property drew all 36 of its warnings from the 8 it had marked `false`, so warning
    /// there kept the omissions it had licensed in the same list as the ones nobody chose.
    #[test]
    fn the_three_required_answers_are_three_different_severities() {
        let declaring = "properties:\n  \
                         - name: parameter\n    type: string\n    required: true\n  \
                         - name: datum\n    type: string\n  \
                         - name: vendor\n    type: string\n    required: false\n\
                         edges: []\n";
        let nodes = vec![node(
            ".yidam/corpus/gage/outlet.yml",
            "class: gage\nlinks: []\n",
        )];
        let classes = vec![class_from("gage", declaring)];

        let c = missing_property(&nodes, &classes);
        let found: Vec<_> = c
            .violations
            .iter()
            .map(|v| {
                let name = ["parameter", "datum", "vendor"]
                    .into_iter()
                    .find(|p| v.detail.contains(&format!("`{p}`")))
                    .unwrap_or_else(|| panic!("names no declared property: {}", v.detail));
                (name, c.severity_of(v))
            })
            .collect();
        assert_eq!(
            found,
            [("parameter", Severity::Error), ("datum", Severity::Warn)],
            "`vendor` is `required: false` and must not be reported"
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
            edge_target_class(&nodes, &edges_of(&nodes), &classes),
        ] {
            assert_eq!(c.severity, Severity::Error, "{} must gate", c.id);
        }
        assert_eq!(
            unlicensed_edge(&nodes, &edges_of(&nodes), &classes).severity,
            Severity::Warn,
            "an undeclared relationship gates only on a class that closed its vocabulary"
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
        assert!(unlicensed_edge(&nodes, &edges_of(&nodes), &silent).passed());
        assert!(edge_target_class(&nodes, &edges_of(&nodes), &silent).passed());
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

    /// **A number is a YAML number, unquoted** (RFC-0040). The compiled schema says
    /// `{"type": "number"}`, and a gate that accepted `"24"` here would be looser than the
    /// schema it exists to be no stricter than — the split from the other side.
    #[test]
    fn a_number_field_holding_a_quoted_number_is_reported() {
        let classes = vec![class_from(
            "reach",
            "properties:\n  - name: length_km\n    type: number\n",
        )];
        let length = |v: &str| {
            vec![node(
                ".yidam/corpus/reach/alpha.yml",
                &format!("class: reach\nproperties:\n  length_km: {v}\nlinks: []\n"),
            )]
        };
        for v in ["24", "6.5", "-3", "0", "1e3"] {
            assert!(
                property_type(&length(v), &classes, &NONE).passed(),
                "{v} is a number"
            );
        }
        let quoted = property_type(&length("\"24\""), &classes, &NONE);
        assert_eq!(quoted.violations.len(), 1);
        assert!(
            quoted.violations[0].detail.contains("unquote it"),
            "{quoted:#?}"
        );
        // Prose, a boolean, and the two YAML spellings of a non-number are each a field
        // filled in wrong, and none of them is "unquote it".
        for v in ["about 24", "yes", ".inf", ".nan", "\"~24\""] {
            let c = property_type(&length(v), &classes, &NONE);
            assert_eq!(c.violations.len(), 1, "{v}: {c:#?}");
            assert!(!c.violations[0].detail.contains("unquote"), "{v}: {c:#?}");
        }
    }

    /// **A quotation names its document** (RFC-0046). Text in a `quotation` field is the
    /// `type: text` anchors of hb-96 again: words that read like a quotation with nothing to
    /// hold them against. The shape is this check's; the bytes are `quotation-span-drift`'s.
    #[test]
    fn a_quotation_field_holding_bare_text_is_reported() {
        let classes = vec![class_from(
            "provision",
            "properties:\n  - name: anchors\n    type: quotation\n",
        )];
        let anchors = |v: &str| {
            vec![node(
                ".yidam/corpus/provision/item-12.yml",
                &format!("class: provision\nproperties:\n  anchors:\n{v}links: []\n"),
            )]
        };
        let held = anchors("    - of: veto-message\n      span: in the public interest\n");
        assert!(property_type(&held, &classes, &NONE).passed());

        let bare = property_type(&anchors("    in the public interest\n"), &classes, &NONE);
        assert_eq!(bare.violations.len(), 1);
        assert!(
            bare.violations[0]
                .detail
                .contains("is declared `quotation` and is not a quotation"),
            "{bare:#?}"
        );
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

    /// RFC-0044: the set is declared where the gate reads it, and a value outside it is the
    /// finding that 33 instances across three corpora had been waiting for.
    #[test]
    fn a_string_outside_its_declared_values_is_reported() {
        let classes = vec![class_from(
            "site",
            "properties:\n  - name: status\n    type: string\n    values: [extant, demolished, in operation]\n",
        )];
        let status = |v: &str| {
            vec![node(
                ".yidam/corpus/site/alpha.yml",
                &format!("class: site\nproperties:\n  status: {v}\nlinks: []\n"),
            )]
        };
        for v in ["extant", "demolished", "in operation", "\"in operation\""] {
            assert!(
                property_type(&status(v), &classes, &NONE).passed(),
                "{v} is in the set"
            );
        }
        // A widening nobody recorded, prose in a token field, and a spelling the set does
        // not carry: each is one finding, and the finding names the set.
        for v in ["abandoned", "\"demolished in 1971\"", "Extant"] {
            let c = property_type(&status(v), &classes, &NONE);
            assert_eq!(c.violations.len(), 1, "{v}: {c:#?}");
            assert!(
                c.violations[0]
                    .detail
                    .contains("`extant`, `demolished`, `in operation`"),
                "{v}: {c:#?}"
            );
        }
        // The shape first: a bare number under a `string` is "quote it", once, and not also
        // "not one of the values".
        let c = property_type(&status("7"), &classes, &NONE);
        assert_eq!(c.violations.len(), 1, "{c:#?}");
        assert!(c.violations[0].detail.contains("quote it"), "{c:#?}");
        assert!(!c.violations[0].detail.contains("values"), "{c:#?}");
    }

    /// Empty is absent, and the field binds `string` alone: `text` is prose, and a coined
    /// type has said the gate does not know its shape.
    #[test]
    fn an_empty_or_misplaced_value_set_binds_nothing() {
        let status = |v: &str| {
            vec![node(
                ".yidam/corpus/site/alpha.yml",
                &format!("class: site\nproperties:\n  status: {v}\nlinks: []\n"),
            )]
        };
        for declaration in [
            "properties:\n  - name: status\n    type: string\n    values: []\n",
            "properties:\n  - name: status\n    type: text\n    values: [extant]\n",
            "properties:\n  - name: status\n    type: site-status\n    values: [extant]\n",
        ] {
            let classes = vec![class_from("site", declaration)];
            assert!(
                property_type(&status("abandoned"), &classes, &NONE).passed(),
                "{declaration}"
            );
        }
    }

    /// RFC-0050, the bare form: a non-empty list of anything, which is what `demi-moore`'s
    /// coined `type: list` already holds. One value not wrapped in a list is the shape being
    /// wrong — the claim arm's one-or-a-list tolerance is not a list's.
    #[test]
    fn a_bare_list_holds_any_non_empty_list() {
        let classes = vec![class_from(
            "observation",
            "properties:\n  - name: extracts\n    type: list\n",
        )];
        let extracts = |v: &str| {
            vec![node(
                ".yidam/corpus/observation/alpha.yml",
                &format!("class: observation\nproperties:\n  extracts:{v}\nlinks: []\n"),
            )]
        };
        for v in ["\n    - file: a.csv\n      grain: day\n", " [a, 2, {b: c}]"] {
            assert!(property_type(&extracts(v), &classes, &NONE).passed(), "{v}");
        }
        for (v, says) in [
            (" []", "empty list"),
            (" a.csv", "is not a list"),
            ("\n    file: a.csv\n", "is a mapping"),
            ("", "is empty"),
        ] {
            let c = property_type(&extracts(v), &classes, &NONE);
            assert_eq!(c.violations.len(), 1, "{v}: {c:#?}");
            assert!(c.violations[0].detail.contains(says), "{v}: {c:#?}");
        }
    }

    /// `items:` holds every item to the scalar arm it names, and the list's `values:` to
    /// each item. The finding names the item, because "a list is wrong" finds nothing.
    #[test]
    fn a_list_holds_each_item_to_its_declared_type() {
        let classes = vec![class_from(
            "pathway",
            "properties:\n  - name: modes\n    type: list\n    items: string\n    values: [rail, bus]\n  - name: lengths\n    type: list\n    items: number\n  - name: standings\n    type: list\n    items: claim\n",
        )];
        let pathway = |props: &str| {
            vec![node(
                ".yidam/corpus/pathway/alpha.yml",
                &format!("class: pathway\nproperties:\n{props}links: []\n"),
            )]
        };
        let ok =
            pathway("  modes: [rail, bus]\n  lengths: [1.5, 3]\n  standings: [open, verified]\n");
        assert!(property_type(&ok, &classes, &NONE).passed());
        for (props, says) in [
            ("  modes: [rail, ferry]\n", "item 2"),
            ("  modes: [rail, 7]\n", "quote it"),
            ("  lengths: [1.5, about 3]\n", "item 2"),
            ("  standings: [open, definitely-true]\n", "item 2"),
            ("  modes: [[rail]]\n", "an item is one value"),
        ] {
            let c = property_type(&pathway(props), &classes, &NONE);
            assert_eq!(c.violations.len(), 1, "{props}: {c:#?}");
            assert!(c.violations[0].detail.contains(says), "{props}: {c:#?}");
        }
    }

    /// `fields:` declares a record, and declaring it closes it: the misspelled `claim_tg` is
    /// the silent loss #1391 reported, an entry reading as having no standing at all.
    #[test]
    fn a_list_of_records_is_closed_and_holds_each_field() {
        let classes = vec![class_from(
            "pathway",
            "properties:\n  - name: effects\n    type: list\n    fields:\n      - { name: label, type: string, required: true }\n      - { name: value, type: number }\n      - { name: claim_tag, type: claim }\n      - { name: horizon, type: string, values: [short, long] }\n",
        )];
        let effects = |v: &str| {
            vec![node(
                ".yidam/corpus/pathway/alpha.yml",
                &format!("class: pathway\nproperties:\n  effects:\n{v}links: []\n"),
            )]
        };
        let ok = effects(
            "    - { label: fare, value: -0.3, claim_tag: verified, horizon: short }\n    - { label: fare, claim_tag: open }\n",
        );
        assert!(property_type(&ok, &classes, &NONE).passed());
        for (v, says) in [
            (
                "    - { label: fare, claim_tg: open }\n",
                "`claim_tg`, which no field declares",
            ),
            ("    - { value: -0.3 }\n", "has no `label`"),
            (
                "    - { label: fare, value: high }\n",
                "has `value` declared `number`",
            ),
            (
                "    - { label: fare, claim_tag: definitely-true }\n",
                "`claim_tag` declared `claim`",
            ),
            (
                "    - { label: fare, horizon: medium }\n",
                "`short`, `long`",
            ),
            (
                "    - { label: fare }\n    - fare\n",
                "item 2 is not a record",
            ),
        ] {
            let c = property_type(&effects(v), &classes, &NONE);
            assert_eq!(c.violations.len(), 1, "{v}: {c:#?}");
            assert!(c.violations[0].detail.contains(says), "{v}: {c:#?}");
        }
    }

    /// **The list is the grader's, not a second opinion of it.** A known type is one
    /// [`property_type_violation`] reports something for — an empty value, which every graded
    /// type refuses — and a coined one is a type it reports nothing for, whatever it holds.
    /// A type added to the grader and not to [`KNOWN_PROPERTY_TYPES`] is then reported as
    /// unread by a check that is wrong about it; this keeps the two from drifting the other way.
    #[test]
    fn known_types_are_exactly_the_graded_ones() {
        for t in KNOWN_PROPERTY_TYPES {
            assert!(
                property_type_violation(t, &serde_yaml::Value::Null).is_some(),
                "`{t}` is listed as known and grades nothing"
            );
        }
        for v in ["~", "definitely-true", "[1, 2]", "{ a: 1 }", "7", "\"\""] {
            let value: serde_yaml::Value = serde_yaml::from_str(v).unwrap();
            assert_eq!(property_type_violation("effect", &value), None, "{v}");
        }
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
}
