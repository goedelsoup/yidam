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
    /// From [`level`]. An error is not a diagnostic — it is the rejection.
    pub level: Level,
    pub step: usize,
    /// From [`diagnostic_code`], which is the set `tools.json` freezes.
    ///
    /// It said *from a closed set, so a client can branch without matching prose* while the
    /// set was closed nowhere a client could read: three of the five codes were named in no
    /// document, and two more were described in the contract's prose without the string a
    /// `match` would need. Frozen at contract 0.18.0.
    pub code: DiagnosticCode,
    pub message: String,
}

/// A diagnostic's severity — one of [`level`]'s two, and nothing else.
///
/// Separate from [`DiagnosticCode`] and from [`Code`] for the reason all three are newtypes:
/// the wrong vocabulary in the right field is exactly the mistake that does not announce
/// itself. RFC-0018 says an error is not a diagnostic, and until 0.18.0 nothing held a server
/// to the two values that leaves.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize)]
#[serde(transparent)]
pub struct Level(&'static str);

impl std::fmt::Display for Level {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0)
    }
}

impl PartialEq<&str> for Level {
    fn eq(&self, other: &&str) -> bool {
        self.0 == *other
    }
}

/// The two severities a diagnostic can carry.
pub mod level {
    use super::Level;

    /// Something is probably wrong and the step ran anyway.
    pub const WARN: Level = Level("warn");
    /// Nothing is wrong; something was decided, and the caller could not otherwise see it.
    pub const INFO: Level = Level("info");

    /// Both of them. An `error` would be a rejection, which is a different field.
    pub const FROZEN: &[Level] = &[WARN, INFO];
}

/// A diagnostic code — one of [`diagnostic_code`]'s, and nothing else.
///
/// A separate type from [`Code`] rather than a second roster behind one: a rejection code in
/// a `Diagnostic` would compile, serialise, and tell a client that a query it ran was refused.
/// Two closed sets are two types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize)]
#[serde(transparent)]
pub struct DiagnosticCode(&'static str);

impl std::fmt::Display for DiagnosticCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0)
    }
}

impl PartialEq<&str> for DiagnosticCode {
    fn eq(&self, other: &&str) -> bool {
        self.0 == *other
    }
}

/// Every `code` a diagnostic can carry, named once.
///
/// The vocabulary `rejected.code`'s freeze (#732) did not look at. It had the same shape and
/// less protection: no enumeration in the contract to diverge *from*, so there was nothing to
/// compare and nothing could go red. Three of the five were named in no document at all, and
/// each of those three arrived in a feature PR that changed no contract — `corpus-excluded`
/// with `--across`, `ontology-moved` with `--at`, `trivial-predicate` with the operator rules.
/// That is exactly how three lint check ids got into `query`'s rejection list and stayed for
/// nine versions.
pub mod diagnostic_code {
    use super::DiagnosticCode;

    /// `!=` against a value the type cannot hold: true of every node carrying the property.
    pub const TRIVIAL_PREDICATE: DiagnosticCode = DiagnosticCode("trivial-predicate");
    /// A relationship the class does not declare, under a non-exhaustive `edge_policy`.
    pub const UNDECLARED_RELATIONSHIP: DiagnosticCode = DiagnosticCode("undeclared-relationship");
    /// `*` narrowed to the classes its predicate can be asked of, naming those it skipped.
    pub const NARROWED: DiagnosticCode = DiagnosticCode("narrowed");
    /// A dependency whose own ontology cannot answer the query, named rather than failed.
    pub const CORPUS_EXCLUDED: DiagnosticCode = DiagnosticCode("corpus-excluded");
    /// The vocabulary moved between the revision asked about and HEAD.
    pub const ONTOLOGY_MOVED: DiagnosticCode = DiagnosticCode("ontology-moved");

    /// The codes a caller of the MCP `query` tool can receive, and the set the contract
    /// freezes — compared against `tools.json` in both directions by `serve::tools`.
    ///
    /// [`ONTOLOGY_MOVED`] is deliberately absent, for the reason `anchor-at-revision` is
    /// absent from the rejection set: it says the ontology moved *between the revision asked
    /// about and HEAD*, and no MCP call names a revision. Freezing it would put a branch in a
    /// client's `match` that nothing can take.
    pub const SURFACED: &[DiagnosticCode] = &[
        TRIVIAL_PREDICATE,
        UNDECLARED_RELATIONSHIP,
        NARROWED,
        CORPUS_EXCLUDED,
    ];
}

/// A rejection code — one of [`code`]'s, and nothing else.
///
/// **The field is this type rather than `&'static str` so that a literal does not compile.**
/// A roster of constants was the first attempt and it did not hold: the ordering operators
/// landed one commit later with `unordered-property` written at its call site, which the
/// roster could not see and the contract gate therefore passed, because both lists agreed
/// about a code neither had heard of. Naming the codes in one place only helps if nothing can
/// name one somewhere else, and that is a property of the type rather than of the discipline.
///
/// The inner field is private, so `code` is the only module that can mint one. It serialises
/// as the bare string it wraps, compares against `&str`, and renders as itself, so every
/// reader — the JSON envelope, `assert_eq!`, the rendered `rejected (…)` line — is unchanged.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize)]
#[serde(transparent)]
pub struct Code(&'static str);

impl std::fmt::Display for Code {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0)
    }
}

/// So a test may say what a caller says: `assert_eq!(e.code, "unknown-class")`. Comparing
/// against the wire string is the assertion worth writing — one against another `Code` would
/// restate a constant to itself and pin nothing a client can see.
impl PartialEq<&str> for Code {
    fn eq(&self, other: &&str) -> bool {
        self.0 == *other
    }
}

/// Every `code` a rejection can carry, named once — and, since [`Code`]'s field is private,
/// the only place one can be named at all.
///
/// The set is the MCP contract's: `tools.json` freezes it and says, in those words, that *a
/// client branches on them*. Both halves of that promise used to be string literals, one at a
/// `Rejection` site and one inside a `notes` paragraph, and nothing compared them. They
/// diverged — three of the nine names the contract froze were lint check ids no rejection has
/// ever carried, and four codes this crate emits were frozen nowhere. Two of the four were
/// near-misses of a frozen name, which is why reading the lists side by side did not catch
/// it: `unknown-property` and `undeclared-property` are the same thing to a person and
/// different strings to a `match`.
pub mod code {
    use super::Code;

    /// The query text is not a path.
    pub const PARSE: Code = Code("parse");
    /// A step names a class the corpus does not declare.
    pub const UNKNOWN_CLASS: Code = Code("unknown-class");
    /// A predicate names a property no candidate class declares.
    pub const UNDECLARED_PROPERTY: Code = Code("undeclared-property");
    /// An ordering operator against a property whose declared type has no order (#725).
    pub const UNORDERED_PROPERTY: Code = Code("unordered-property");
    /// An operand the declared type cannot hold: a query that could never match.
    pub const UNSATISFIABLE_PREDICATE: Code = Code("unsatisfiable-predicate");
    /// A hop no class on the authoring side licenses, by policy or by target.
    pub const UNLICENSED_HOP: Code = Code("unlicensed-hop");
    /// `select` names a field that is not projectable.
    pub const UNKNOWN_FIELD: Code = Code("unknown-field");
    /// An anchor on a step that is arrived at rather than entered.
    pub const ANCHOR_NOT_ENTRY: Code = Code("anchor-not-entry");
    /// An anchor with no index supplied to resolve it against.
    pub const ANCHOR_UNAVAILABLE: Code = Code("anchor-unavailable");
    /// An index was supplied and the anchor did not resolve against it.
    pub const ANCHOR_UNRESOLVABLE: Code = Code("anchor-unresolvable");
    /// An anchored query asked to span dependencies the local index does not cover.
    pub const ANCHOR_ACROSS: Code = Code("anchor-across");
    /// An anchor asked for as of a past commit, which the index cannot answer.
    pub const ANCHOR_AT_REVISION: Code = Code("anchor-at-revision");
    /// The corpus at the requested revision or range could not be reconstructed.
    pub const HISTORY_UNREADABLE: Code = Code("history-unreadable");

    /// The codes a caller of the MCP `query` tool can receive, and the set the contract
    /// freezes. `serve::tools` compares this against `tools.json` in both directions — a code
    /// missing from the contract is one a client was told to branch on and never sees, and a
    /// frozen name missing from here is a branch that can never be taken — and checks the
    /// value on the way out of the tool besides.
    ///
    /// [`ANCHOR_AT_REVISION`] and [`HISTORY_UNREADABLE`] are deliberately absent. Both are
    /// about a revision and the MCP surface has none: the contract's `at` is null for a
    /// server answering about its loaded corpus, so no call can supply the `--at` or
    /// `--between` they answer. Freezing them would put two more names in a client's `match`
    /// that no conforming server can reach, which is the failure the list already had three
    /// of.
    ///
    /// A new code has to be added here to be frozen, and that is now the *only* omission
    /// possible: it cannot be written at a call site instead, so the failure mode is a
    /// contract gate that goes red rather than a code nobody knew to look for.
    pub const SURFACED: &[Code] = &[
        PARSE,
        UNKNOWN_CLASS,
        UNDECLARED_PROPERTY,
        UNORDERED_PROPERTY,
        UNSATISFIABLE_PREDICATE,
        UNLICENSED_HOP,
        UNKNOWN_FIELD,
        ANCHOR_NOT_ENTRY,
        ANCHOR_UNAVAILABLE,
        ANCHOR_UNRESOLVABLE,
        ANCHOR_ACROSS,
    ];
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Rejection {
    pub step: Option<usize>,
    /// From [`code`], which is the set `tools.json` freezes — and, because [`Code`] cannot be
    /// built outside it, the only set this field can hold.
    pub code: Code,
    pub message: String,
}

fn reject(code: Code, step: Option<usize>, message: impl Into<String>) -> Rejection {
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
            code::UNDECLARED_PROPERTY,
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
    // `prop?` reads whether the node carries the property at all. That is a question about
    // every declared type equally, it has no operand to typecheck, and the arms below all
    // concern an operand — so it is answered here rather than falling through them.
    if pred.op == Op::Absent {
        return Ok(());
    }
    // **The ordering operators are `date` only, and this is the rejection that keeps them
    // honest.** RFC-0018 deferred them on exactly this ground: the declared types are
    // `string`, `text`, `date`, `ref` and `claim` with no numeric among them, so an ordering
    // that fell back to comparing text would be correct on `date` and a trap on the rest —
    // `length_km<9` would rank `10` below `9`, and say nothing about having done so. A
    // corpus that coins its own type gets the same refusal rather than a lexical guess,
    // because nothing here knows what its order is.
    if pred.op.is_ordering() && declared != "date" {
        return Err(reject(
            code::UNORDERED_PROPERTY,
            Some(step_index),
            format!(
                "`{}` is `type: {declared}` on `{}`, and `{}` is defined on `date` only — \
                 comparing anything else would be a comparison of text, which reads `10` as \
                 before `9`. Use `=`, `!=` or `~`.",
                pred.prop,
                class.name,
                pred.op.as_str()
            ),
        ));
    }
    let as_yaml = serde_yaml::Value::String(pred.value.clone());
    let bad = crate::cmd::lint::checks::property_type_violation(declared, &as_yaml);
    match (pred.op, bad) {
        // An ordering needs a date on both sides. The operand's is the half a query can be
        // wrong about, and `property_type_violation`'s own sentence says what a date is —
        // which is the sentence the corpus already gets when it writes one wrong.
        (op, Some(why)) if op.is_ordering() => Err(reject(
            code::UNSATISFIABLE_PREDICATE,
            Some(step_index),
            format!(
                "`{}` cannot be compared against that value: {why}",
                pred.prop
            ),
        )),
        // `=` asks for a value the property can hold. Asking for one it cannot is a query
        // that could never match, which is a rejection and not an empty result.
        (Op::Eq, Some(why)) => Err(reject(
            code::UNSATISFIABLE_PREDICATE,
            Some(step_index),
            format!("`{}` cannot hold that value: {why}", pred.prop),
        )),
        // `!=` excluding a value the type cannot hold is trivially true of every node that
        // carries the property. Legal, and worth saying.
        (Op::Ne, Some(why)) => {
            out.push(Diagnostic {
                level: level::WARN,
                step: step_index,
                code: diagnostic_code::TRIVIAL_PREDICATE,
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
            code::UNLICENSED_HOP,
            Some(step_index),
            rejections.join("; "),
        ));
    }

    for class in undeclared_by {
        out.push(Diagnostic {
            level: level::WARN,
            step: step_index,
            code: diagnostic_code::UNDECLARED_RELATIONSHIP,
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
                code::UNKNOWN_CLASS,
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
                        code::UNDECLARED_PROPERTY,
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
                    level: level::INFO,
                    step: index,
                    code: diagnostic_code::NARROWED,
                    // **The predicate, not the property name.** Declaring it was the only way
                    // to fail this loop until the ordering operators arrived (#725); now a
                    // class can be skipped for declaring `began` as `type: string`, and a
                    // message saying it does not declare `began` would be false about the one
                    // class it names.
                    message: format!(
                        "`*` narrowed to the classes `{}` can be asked of; skipped {}",
                        pred.spelled(),
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

    // ── ordering and absence (#725) ───────────────────────────────────────────

    const TENURE: &str = "class: tenure\nproperties:\n  - name: began\n    type: date\n  \
                          - name: ended\n    type: date\n  - name: note\n    type: string\n";

    #[test]
    fn ordering_a_date_property_against_a_date_is_licensed() {
        let f = Fixture::new(vec![class(TENURE)]);
        for query in [
            "tenure[began<=1893]",
            "tenure[began<1893-04,ended>?1893-04-01]",
            "tenure[ended?]",
        ] {
            let checked = check(&parse(query).unwrap(), &f.schema()).unwrap();
            assert!(
                checked.diagnostics.is_empty(),
                "{query}: {:?}",
                checked.diagnostics
            );
        }
    }

    /// **The rejection RFC-0018 deferred the operators for.** There is no numeric declared
    /// type, so an ordering on anything but `date` would silently be a comparison of text.
    #[test]
    fn ordering_a_property_that_is_not_a_date_is_rejected_rather_than_answered_lexically() {
        let f = Fixture::new(vec![class(TENURE)]);
        let e = check(&parse("tenure[note<9]").unwrap(), &f.schema()).unwrap_err();
        assert_eq!(e.code, "unordered-property");
        assert!(e.message.contains("`date` only"), "{}", e.message);
        assert!(
            e.message.contains("reads `10` as before `9`"),
            "{}",
            e.message
        );
    }

    /// A type the corpus coined gets the same refusal. Nothing here knows its order, and
    /// guessing one is the trap in a different costume.
    #[test]
    fn ordering_a_coined_type_is_refused_rather_than_guessed() {
        let f = Fixture::new(vec![class(
            "class: measurement\nproperties:\n  - name: flow\n    type: discharge\n",
        )]);
        let e = check(&parse("measurement[flow>9]").unwrap(), &f.schema()).unwrap_err();
        assert_eq!(e.code, "unordered-property");
        assert!(e.message.contains("type: discharge"), "{}", e.message);
    }

    /// The other half of an ordering is the operand, and it is the half the query can be
    /// wrong about. The sentence is `property_type_violation`'s own — the one the corpus
    /// already gets when it writes a date wrong.
    #[test]
    fn ordering_against_an_operand_that_is_not_a_date_is_rejected() {
        let f = Fixture::new(vec![class(TENURE)]);
        let e = check(&parse("tenure[began<last-tuesday]").unwrap(), &f.schema()).unwrap_err();
        assert_eq!(e.code, "unsatisfiable-predicate");
        assert!(e.message.contains("is not a date"), "{}", e.message);
    }

    /// `prop?` reads whether the node carries the property, which is a question about every
    /// declared type equally — and it has no operand to typecheck.
    #[test]
    fn the_absence_test_is_legal_on_any_declared_type() {
        let f = streamflow();
        for query in ["reach[claim_tag?]", "reach[regulated?]", "gage[parameter?]"] {
            assert!(
                check(&parse(query).unwrap(), &f.schema()).is_ok(),
                "{query}"
            );
        }
        // Still a declared property, though: the affix does not exempt the name.
        let e = check(&parse("reach[depth?]").unwrap(), &f.schema()).unwrap_err();
        assert_eq!(e.code, "undeclared-property");
    }

    /// `*` narrows past a class the predicate cannot be asked of, and is rejected only when
    /// every class refuses — the rule already in force, now reachable for a second reason.
    ///
    /// The message is the half worth pinning. It read *"narrowed to the classes declaring
    /// `began`"*, which is false about `memo`: `memo` declares `began`, as a string with no
    /// order. A skipped class named under a wrong reason is worse than one not named.
    #[test]
    fn a_star_ordering_narrows_past_a_class_that_declares_the_property_undated() {
        let f = Fixture::new(vec![
            class(TENURE),
            class("class: memo\nproperties:\n  - name: began\n    type: string\n"),
        ]);
        let checked = check(&parse("*[began<1893]").unwrap(), &f.schema()).unwrap();
        assert_eq!(checked.narrowed[0], vec!["tenure"]);
        let d = &checked.diagnostics[0];
        assert_eq!(d.code, "narrowed");
        assert!(d.message.contains("skipped `memo`"), "{}", d.message);
        assert!(
            !d.message.contains("declaring"),
            "`memo` does declare `began` — {}",
            d.message
        );
        assert!(d.message.contains("`began<1893`"), "{}", d.message);
    }

    /// When no class survives, the rejection is the last one raised rather than the generic
    /// undeclared-property sentence — so `*` says the ordering was the problem.
    #[test]
    fn a_star_ordering_no_class_can_answer_is_rejected_with_the_ordering_reason() {
        let f = Fixture::new(vec![class(
            "class: memo\nproperties:\n  - name: began\n    type: string\n",
        )]);
        let e = check(&parse("*[began<1893]").unwrap(), &f.schema()).unwrap_err();
        assert_eq!(e.code, "unordered-property");
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
