//! Why an answer is empty, derived rather than guessed (#283, E6).
//!
//! # An empty result is where an agent invents
//!
//! A query that typechecks and matches nothing returns zero rows, and zero rows is
//! indistinguishable from a bad embedding, a class nobody has written into yet, or a corpus
//! that genuinely has no view. An agent that cannot tell those apart fills the gap from its
//! own weights — confidently, and under a claim that will be attributed to having worked in
//! the corpus.
//!
//! `retrieve` already has the right instinct one door over: when the index is missing it
//! falls back to keyword search and says `degraded: true` rather than quietly answering
//! worse. This is the same principle applied to **coverage** rather than to method.
//!
//! # Derived, and therefore refusable
//!
//! Every code below is read off something the corpus states — a class the ontology declares,
//! a relationship a class licenses, the edge set the gate resolves. None of it is inferred
//! from the query's shape, which is why the diagnosis can be trusted at the moment it matters
//! most: the moment an agent is deciding whether the silence is the corpus's or its own.
//!
//! The rejections in [`super::check`] answer *the query is wrong*. These answer *the query is
//! right and the corpus is quiet, and here is which kind of quiet*. Neither is the other, and
//! a surface that collapsed them would tell an agent its typo was a true negative.
//!
//! # Diagnosed against the local corpus, always
//!
//! Even on `--across`. `instances` counts this repository's nodes and the declaration read is
//! this repository's ontology, because a foreign corpus's silence is that corpus's claim and
//! folding it in would produce a count over classes that share a name and not a meaning — the
//! reason `--across` runs one execution per corpus in the first place. What the dependency set
//! contributes is [`Absence::elsewhere`], which is a pointer and not an answer.

use std::collections::{BTreeMap, BTreeSet};

use super::check::Checked;
use super::exec::Outcome;
use super::lang::{Dir, Query};
use super::{Context, Graph};
use crate::cmd::lint::checks::class_of;

/// The empty answer, explained.
///
/// Present exactly when the query ran and matched nothing. A query that matched something is
/// not absent, and a rejected one never ran.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Absence {
    /// Where the answer became empty. `0` is the entry step.
    pub step: usize,
    /// From a closed set, so a client branches without matching prose.
    pub code: &'static str,
    pub message: String,
    /// The denominator the message is about: instances of the step's class(es) for an entry
    /// step, nodes that reached the previous step for a hop.
    ///
    /// It is what makes `predicate-unsatisfied` a statement rather than a shrug — *none of
    /// three* and *none of nine hundred* are different facts about a corpus.
    pub instances: usize,
    /// Installed packages that hold what this corpus does not, and could answer with
    /// `--across`.
    ///
    /// Always present, empty when none do, so a client never has to distinguish "no
    /// dependency has it" from "nothing looked".
    pub elsewhere: Vec<String>,
}

/// Diagnose an empty answer.
///
/// Called only when `matched` is zero — the dependency walk below is a directory read per
/// installed package, and an answer that came back with rows has no question to ask.
pub(crate) fn diagnose(
    ctx: &Context,
    parsed: &Query,
    checked: &Checked,
    authored: &BTreeMap<String, BTreeSet<String>>,
    outcome: &Outcome,
    anchor: Option<&super::anchor::Anchor>,
) -> Absence {
    // The first zero. A later step is empty *because* an earlier one was, and reporting the
    // last step would name the place the emptiness arrived rather than the place it began.
    let step = outcome
        .reached
        .iter()
        .position(|n| *n == 0)
        .unwrap_or(outcome.reached.len().saturating_sub(1));

    match step {
        0 => entry(ctx, parsed, checked, anchor),
        _ => hop(ctx, parsed, checked, authored, outcome, step),
    }
}

/// The entry step matched nothing.
fn entry(
    ctx: &Context,
    parsed: &Query,
    checked: &Checked,
    anchor: Option<&super::anchor::Anchor>,
) -> Absence {
    let graph = ctx.graph;
    let empty = Vec::new();
    let classes = checked.narrowed.first().unwrap_or(&empty);
    let instances = graph
        .nodes
        .iter()
        .filter(|n| classes.contains(&class_of(n)))
        .count();
    let elsewhere = elsewhere(ctx, classes);

    // An anchor that landed nowhere is its own answer, and it is not a statement about the
    // corpus. Reporting `class-unpopulated` here would blame a populated class for a
    // retrieval that returned no entry node — and on the keyword path, which is where most
    // readers of this repository are, that happens whenever the query's words are not the
    // corpus's words.
    if let Some(a) = anchor {
        if a.entries.is_empty() {
            return Absence {
                step: 0,
                code: "anchor-empty",
                message: format!(
                    "the similarity anchor resolved to no entry node, so nothing was walked \
                     from. {} — this is a statement about the retrieval, not about the {}{} \
                     instance(s) the corpus holds.",
                    match a.degraded_reason {
                        Some(reason) => format!(
                            "It ran as keyword search rather than similarity ({reason}){}",
                            a.repair.map(|r| format!("; {r}")).unwrap_or_default()
                        ),
                        None => "It ran as semantic search".to_string(),
                    },
                    instances,
                    match classes.len() {
                        1 => format!(" `{}`", classes[0]),
                        _ => String::new(),
                    }
                ),
                instances,
                elsewhere,
            };
        }
    }

    if instances == 0 {
        return Absence {
            step: 0,
            code: "class-unpopulated",
            message: format!(
                "{} {} declared and holds no instances in this corpus. The ontology names it; \
                 nothing has been written into it yet.{}",
                names(classes),
                is_are(classes),
                offer(&elsewhere)
            ),
            instances,
            elsewhere,
        };
    }

    let predicates: Vec<String> = parsed.steps[0]
        .filter
        .iter()
        .map(|p| format!("{}{}{}", p.prop, p.op.as_str(), p.value))
        .collect();
    if !predicates.is_empty() {
        return Absence {
            step: 0,
            code: "predicate-unsatisfied",
            message: format!(
                "{} holds {instances} instance(s) and none satisfies `{}`.{} This is a real \
                 empty result: the corpus has the nodes and does not have the value.",
                names(classes),
                predicates.join("`, `"),
                values_seen(graph, classes, &parsed.steps[0]),
            ),
            instances,
            elsewhere,
        };
    }

    // Every remaining shape is one the arms above should have caught — a class with instances
    // and no predicate matches all of them. Reported rather than asserted away, because an
    // `unreachable!` here would turn a wrong diagnosis into a crashed query.
    Absence {
        step: 0,
        code: "no-match",
        message: format!(
            "{} holds {instances} instance(s) and none satisfied the entry step.",
            names(classes)
        ),
        instances,
        elsewhere,
    }
}

/// A hop landed nowhere.
fn hop(
    ctx: &Context,
    parsed: &Query,
    checked: &Checked,
    authored: &BTreeMap<String, BTreeSet<String>>,
    outcome: &Outcome,
    step: usize,
) -> Absence {
    let graph = ctx.graph;
    let hop = &parsed.hops[step - 1];
    let rel = &hop.relationship;
    let from = outcome.reached[step - 1];
    let empty = Vec::new();
    let source = checked.narrowed.get(step - 1).unwrap_or(&empty);
    let landing = checked.narrowed.get(step).unwrap_or(&empty);
    let elsewhere = elsewhere(ctx, landing);

    // **The row this whole module is most worth having.** A relationship a class licenses and
    // no instance has ever authored is either an ontology that overreached or a corpus with a
    // gap, and it is invisible from every other angle: the class file says the relationship
    // exists, the gate has nothing to complain about, and a traversal by it comes back empty
    // exactly like a traversal by a name that was mistyped. Surfacing it produces the kind of
    // open question the corpus wants.
    if !authored.contains_key(rel) {
        let declared: Vec<&str> = graph
            .classes
            .iter()
            .filter(|c| source.contains(&c.name) && c.edges.iter().any(|e| &e.relationship == rel))
            .map(|c| c.name.as_str())
            .collect();
        // **Declared-and-unused and never-heard-of are two findings, not one.** They come
        // back from a traversal identically and their repairs are opposite: the first is a
        // corpus that has not written a relationship its ontology promises, and the second is
        // almost always a name that was mistyped — which the check has already said, in the
        // near-miss diagnostic beside this. One code covering both would hand a caller a
        // sentence about the ontology when the answer is a typo.
        if declared.is_empty() {
            return Absence {
                step,
                code: "relationship-unknown",
                message: format!(
                    "no class declares `{rel}` and no instance authors it, so this hop could \
                     not have gone anywhere. Check the name against the step's diagnostics \
                     before reading this as a gap in the corpus."
                ),
                instances: from,
                elsewhere,
            };
        }
        return Absence {
            step,
            code: "relationship-unauthored",
            message: format!(
                "`{}` declares `{rel}` and no instance in this corpus authors it. The \
                 relationship exists in the ontology and not in the graph — either the \
                 ontology reached past the corpus, or this is a gap worth an `open:` commit.",
                declared.join("`, `")
            ),
            instances: from,
            elsewhere,
        };
    }

    if outcome.followed[step] == 0 {
        return Absence {
            step,
            code: "no-edge-from-here",
            message: format!(
                "`{rel}` is authored in this corpus, by {}, and none of the {from} node(s) \
                 that reached the previous step has one {}. The relationship is in use; it is \
                 not in use here.",
                names(&authored[rel].iter().cloned().collect::<Vec<_>>()),
                match hop.direction {
                    Dir::Out => "leaving it",
                    Dir::In => "pointing at it",
                }
            ),
            instances: from,
            elsewhere,
        };
    }

    Absence {
        step,
        code: "edge-lands-elsewhere",
        message: format!(
            "{} `{rel}` edge(s) were followed from the {from} node(s) that reached the \
             previous step, and nothing they landed on satisfied {}. The edges are there and \
             they go somewhere else.",
            outcome.followed[step],
            names(landing)
        ),
        instances: from,
        elsewhere,
    }
}

/// One or many class names, backticked — a noun phrase, so the sentence supplies its own verb.
fn names(classes: &[String]) -> String {
    match classes.is_empty() {
        true => "no class".to_string(),
        false => format!("`{}`", classes.join("`, `")),
    }
}

/// `is` or `are`, agreeing with how many classes [`names`] just listed.
fn is_are(classes: &[String]) -> &'static str {
    match classes.len() {
        1 => "is",
        _ => "are",
    }
}

/// The values the corpus actually holds for a predicate's property.
///
/// The half of "say which" that turns a refusal into a next action: an agent told only that
/// nothing satisfies `regulated=yes` cannot tell a misspelled value from a corpus that has
/// never recorded one. Capped, because the point is to show the shape of what is there and a
/// corpus with nine hundred distinct values has answered the question by the third.
fn values_seen(graph: &Graph, classes: &[String], step: &super::lang::Step) -> String {
    const SHOW: usize = 3;
    let Some(pred) = step.filter.first() else {
        return String::new();
    };
    let mut seen: BTreeSet<String> = BTreeSet::new();
    for node in graph
        .nodes
        .iter()
        .filter(|n| classes.contains(&class_of(n)))
    {
        let Some(props) = node.inst.properties.as_ref() else {
            continue;
        };
        let Some(value) = props.get(serde_yaml::Value::String(pred.prop.clone())) else {
            continue;
        };
        if let Some(text) = value.as_str() {
            seen.insert(text.to_string());
        }
    }
    if seen.is_empty() {
        return format!(" No instance writes `{}` at all.", pred.prop);
    }
    let shown: Vec<String> = seen.iter().take(SHOW).map(|v| format!("`{v}`")).collect();
    format!(
        " The value(s) present are {}{}.",
        shown.join(", "),
        match seen.len() > SHOW {
            true => format!(" and {} more", seen.len() - SHOW),
            false => String::new(),
        }
    )
}

/// Installed packages holding instances of any of these classes.
///
/// A directory read per package and no parsing: the question is whether a corpus this one
/// merely installed has nodes where this one has none, and the answer is the presence of
/// `<pkg>/corpus/<class>/*.yml`. Run only on an empty answer.
///
/// **Silent in two situations, and both are the same argument.** A query about a past commit
/// gets no offer: a dependency set has no history — what this repository has is whatever
/// bundle is unpacked *now* — so pointing at today's packages to explain a past corpus's
/// silence would be an anachronism dressed as a lead. That is the reason `--across` and
/// `--at` are refused together, one field further in. And a run that already spanned the
/// dependency set gets none either: it looked, and "re-run with `--across`" to a caller who
/// did is advice to repeat the query that just came back empty.
fn elsewhere(ctx: &Context, classes: &[String]) -> Vec<String> {
    let graph = ctx.graph;
    if classes.is_empty() || ctx.at.is_some() || !graph.across.is_empty() {
        return Vec::new();
    }
    crate::deps::resolved(&graph.root)
        .into_iter()
        .filter(|dep| {
            classes.iter().any(|class| {
                std::fs::read_dir(dep.corpus_dir.join(class))
                    .map(|mut entries| {
                        entries.any(|e| {
                            e.map(|e| e.path().extension().is_some_and(|x| x == "yml"))
                                .unwrap_or(false)
                        })
                    })
                    .unwrap_or(false)
            })
        })
        .map(|dep| dep.name)
        .collect()
}

/// The offer to look next door, when there is one.
fn offer(elsewhere: &[String]) -> String {
    match elsewhere.is_empty() {
        true => String::new(),
        false => format!(
            " `{}` holds instances of it — re-run with `--across` to query the dependency \
             set, and read the result as that corpus's claim rather than this one's.",
            elsewhere.join("`, `")
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cmd::query::{run, run_across, Options};

    /// A corpus shaped so each absence code has exactly one way to be reached.
    ///
    /// `note` is declared and empty; `never-used` is declared and authored by nobody;
    /// `sources-from` is authored by `gage` and by no `reach`; and `gage` writes a `units`
    /// value no reasonable query would guess. Every one of those is invisible from the
    /// outside — which is the point, since each produces the same zero rows.
    fn write(dir: &std::path::Path, rel: &str, body: &str) {
        let path = dir.join(".yidam/corpus").join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, body).unwrap();
    }

    fn fixture() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        write(
            dir.path(),
            "reach.ont.yml",
            "class: reach\nedges:\n  - relationship: measured-by\n    target: gage\n    \
             direction: out\n  - relationship: never-used\n    target: gage\n    \
             direction: out\n",
        );
        write(
            dir.path(),
            "gage.ont.yml",
            "class: gage\nproperties:\n  - name: units\n    type: string\nedges:\n  \
             - relationship: sources-from\n    target: concept\n    direction: out\n",
        );
        write(dir.path(), "concept.ont.yml", "class: concept\n");
        // Declared, and holding nothing. The whole point of the class. It declares a
        // property so the `--across` test below has a predicate to fail on — without one,
        // every spanning query it could ask would either match or be rejected.
        write(
            dir.path(),
            "note.ont.yml",
            "class: note\nproperties:\n  - name: kind\n    type: string\n",
        );
        write(
            dir.path(),
            "reach/tailwater.yml",
            "class: reach\nlabel: Tailwater\nlinks:\n  - target: ../gage/canyon.yml\n    \
             relationship: measured-by\n",
        );
        write(
            dir.path(),
            "gage/canyon.yml",
            "class: gage\nlabel: Canyon\nproperties:\n  units: cubic feet per second\n\
             links:\n  - target: ../concept/flow.yml\n    relationship: sources-from\n",
        );
        write(
            dir.path(),
            "concept/flow.yml",
            "class: concept\nlabel: Flow\n",
        );
        dir
    }

    fn absent(dir: &tempfile::TempDir, query: &str) -> Absence {
        let report = run(dir.path(), query, &Options::default());
        assert!(report.rejected.is_none(), "{:?}", report.rejected);
        assert_eq!(
            report.matched, 0,
            "this query was supposed to match nothing"
        );
        report.absence.expect("an empty answer carries an absence")
    }

    /// Row one: the ontology declares it; the walk finds none.
    #[test]
    fn a_declared_class_with_no_instances_says_so() {
        let a = absent(&fixture(), "note");
        assert_eq!((a.code, a.step, a.instances), ("class-unpopulated", 0, 0));
        assert!(a.message.contains("`note`"), "{}", a.message);
        assert!(a.message.contains("nothing has been written into it yet"));
    }

    /// Row two: it has instances and none satisfies the predicate — **and say which**. The
    /// values present are the half that turns a dead end into a next action: without them an
    /// agent cannot tell a misspelled value from a corpus that never recorded one.
    #[test]
    fn a_predicate_nothing_satisfies_names_the_values_that_are_there() {
        let a = absent(&fixture(), "gage[units=cfs]");
        assert_eq!((a.code, a.instances), ("predicate-unsatisfied", 1));
        assert!(a.message.contains("units=cfs"), "{}", a.message);
        assert!(
            a.message.contains("`cubic feet per second`"),
            "the values present are the point of this arm:\n{}",
            a.message
        );
    }

    /// Row three, and the one worth more than it looks. A relationship a class licenses and
    /// no instance uses is either an ontology that overreached or a corpus with a gap, and it
    /// is invisible from every other angle: the class file says it exists, the gate has
    /// nothing to complain about, and the traversal comes back exactly like a typo would.
    #[test]
    fn a_relationship_the_ontology_declares_and_nobody_authors_is_named_as_such() {
        let a = absent(&fixture(), "reach -never-used-> gage");
        assert_eq!((a.code, a.step), ("relationship-unauthored", 1));
        assert!(
            a.message.contains("`reach` declares `never-used`"),
            "{}",
            a.message
        );
        assert!(
            a.message.contains("reached past the corpus") || a.message.contains("open:"),
            "{}",
            a.message
        );
    }

    /// The other half of that split. A name nothing declares and nothing authors is almost
    /// always a typo — the check has already said so in the near-miss diagnostic beside this
    /// — and answering it with a sentence about the ontology having overreached would send a
    /// reader to audit a class file over a misspelling.
    #[test]
    fn a_relationship_nothing_declares_is_not_reported_as_an_ontology_gap() {
        let a = absent(&fixture(), "reach -measurd-by-> gage");
        assert_eq!((a.code, a.step), ("relationship-unknown", 1));
        assert!(
            a.message.contains("no class declares `measurd-by`"),
            "{}",
            a.message
        );
    }

    /// The relationship is in use, and not here. Distinct from the row above because the
    /// repairs are opposite: one is a corpus that has not written a relationship, the other
    /// is a query pointed at the wrong end of one that is written.
    #[test]
    fn a_relationship_used_elsewhere_but_not_here_is_a_different_answer() {
        let a = absent(&fixture(), "reach -sources-from-> concept");
        assert_eq!((a.code, a.step), ("no-edge-from-here", 1));
        assert!(
            a.message.contains("authored in this corpus, by `gage`"),
            "{}",
            a.message
        );
    }

    /// Edges were followed and nothing they landed on qualified. The count is what separates
    /// this from `no-edge-from-here`, and reporting either as the other would send a reader
    /// to look for a relationship that is right there.
    #[test]
    fn edges_that_go_somewhere_the_step_rejects_say_how_many_were_followed() {
        let a = absent(&fixture(), "reach -measured-by-> gage[units=cfs]");
        assert_eq!(
            (a.code, a.step, a.instances),
            ("edge-lands-elsewhere", 1, 1)
        );
        assert!(
            a.message.contains("1 `measured-by` edge(s) were followed"),
            "{}",
            a.message
        );
    }

    /// An anchor that lands nowhere is a statement about the retrieval, not about the corpus.
    /// Reporting `class-unpopulated` here would blame a populated class for a search whose
    /// words were not the corpus's words — which on the keyword path is most of the time.
    #[test]
    fn an_anchor_that_resolves_to_nothing_does_not_blame_the_class() {
        let a = absent(&fixture(), r#"reach~"zzzz qqqq wwww" -measured-by-> gage"#);
        assert_eq!((a.code, a.step), ("anchor-empty", 0));
        assert!(
            a.instances > 0,
            "the class is populated and the message says so"
        );
        assert!(a.message.contains("not about the"), "{}", a.message);
    }

    /// An answer is not absent. The field is null rather than a code meaning "fine", because
    /// the one thing it must never be is ambiguous.
    #[test]
    fn an_answer_carries_no_absence_and_neither_does_a_rejection() {
        let dir = fixture();
        assert!(run(dir.path(), "reach", &Options::default())
            .absence
            .is_none());
        let rejected = run(dir.path(), "raech", &Options::default());
        assert!(rejected.rejected.is_some());
        assert!(
            rejected.absence.is_none(),
            "a rejected query never ran, so there is nothing to diagnose"
        );
    }

    // ── row four: reachable only through a dependency ─────────────────────────

    /// The same corpus, plus an installed package that has written into the class this one
    /// declares and left empty.
    fn with_dependency() -> tempfile::TempDir {
        let dir = fixture();
        let write_dep = |rel: &str, body: &str| {
            let path = dir.path().join(".yidam/tonpa/upstream").join(rel);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, body).unwrap();
        };
        write_dep("manifest.yml", "commit: \"abc1234\"\n");
        write_dep(
            "corpus/note.ont.yml",
            "class: note\nproperties:\n  - name: kind\n    type: string\n",
        );
        write_dep(
            "corpus/note/upstream-note.yml",
            "class: note\nlabel: Upstream note\nproperties:\n  kind: upstream\n",
        );
        dir
    }

    #[test]
    fn a_class_this_corpus_left_empty_and_a_dependency_filled_says_where_to_look() {
        let a = absent(&with_dependency(), "note");
        assert_eq!(a.code, "class-unpopulated");
        assert_eq!(a.elsewhere, vec!["upstream".to_string()]);
        assert!(a.message.contains("--across"), "{}", a.message);
        assert!(
            a.message
                .contains("that corpus's claim rather than this one's"),
            "the offer must not read as though the answer would be this repository's:\n{}",
            a.message
        );
    }

    /// A caller who already spanned the dependency set is not told to span it. The offer is
    /// advice, and advice to repeat the query that just came back empty is noise.
    #[test]
    fn a_run_that_already_spanned_the_dependency_set_is_offered_nothing() {
        let dir = with_dependency();
        // The dependency *does* hold `note` instances — the previous test is the proof —
        // and this predicate is one none of them satisfies. So the offer would fire if the
        // spanning run did not suppress it.
        let report = run_across(dir.path(), "note[kind=absent]", &Options::default());
        assert!(report.rejected.is_none(), "{:?}", report.rejected);
        assert_eq!(report.matched, 0);
        let a = report.absence.expect("still empty, still diagnosed");
        assert!(
            a.elsewhere.is_empty(),
            "it looked next door already: {:?}",
            a.elsewhere
        );
        assert!(!a.message.contains("--across"), "{}", a.message);
    }
}
