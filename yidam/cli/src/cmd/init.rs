//! `yidam init` — the smallest real corpus, written where the shell is standing.
//!
//! Three routes to a corpus existed before this one and none of them was a command that
//! works where the person is (#1035). `clone` and `overlay` copy the template out of the
//! checkout the shell is standing in, so in somebody's own project neither does anything —
//! `clone` said so from #913 and `overlay` from #1033. The other two routes are
//! `docs/first-corpus-by-hand.md` and the `starting-a-corpus` skill, and both are prose
//! telling a reader to type out a skeleton that is the same every time.
//!
//! **What differs between two corpora is the class names, the relationship, and the nodes.**
//! What does not differ is the directory layout, the key set of a class file, the front
//! matter of an instance, and the fact that a leaf node with no outgoing edge is an
//! `orphan-out` error. This command writes the second set and leaves placeholders in the
//! first, so that the part only the person knows is the part they are left holding.
//!
//! # What it is not
//!
//! **It does not derive a repository.** Nothing here reads the template, and there is no
//! `.yidam.toml` pin: a corpus written by this command came from nowhere and says so. The
//! bootstrap dialogue remains the better corpus and `clone`/`overlay` remain how you reach
//! it — `docs/first-corpus-by-hand.md` §"Then grow it with the dialogue" is the order to do
//! the two in.
//!
//! **It does not commit.** The first commit's subject is a `genesis:` naming the domain, and
//! only the person knows the domain. `export` and `bundle` read it later.

use anyhow::{bail, Result};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use crate::paths::repo_root;

/// The classes written when `--class` names none.
///
/// A pair rather than one, because one class and one node is a corpus a reader cannot see
/// the point of: the thing a class file decides is what may link to what, and that question
/// does not arise until there are two kinds. A pair rather than three, because the
/// `starting-a-corpus` skill argues the number and lands on two — "eight classes agreed to
/// in one message is a guess, and every node written under it inherits the guess".
///
/// The names are deliberately thin. `concept` is what the by-hand page opens with, and
/// `observation` is its counterpart in the shape every corpus here has: something you are
/// trying to understand, and something seen that bears on it. Neither is a suggestion about
/// the person's domain — that is what the placeholder prose in each file says in as many
/// words.
pub const DEFAULT_CLASSES: &[&str] = &["concept", "observation"];

/// The relationship declared between the classes.
///
/// One name for every pair, because the command cannot know the domain and a *plausible*
/// relationship is worse than an obviously blank one: a reader who is handed
/// `measured-by` between two classes they just named has been given an assertion about
/// their domain by a program that has never seen it. `relates-to` is the by-hand page's own
/// bare association, and every file that carries it says to rename it.
const RELATIONSHIP: &str = "relates-to";

/// The claim-typed property every class declares and every instance carries.
const CLAIM_PROPERTY: &str = "claim_tag";

/// One file the command writes: where it goes, and what goes in it.
pub struct Written {
    /// Repo-relative, which is how it is reported and how a test names it.
    pub rel: String,
    pub body: String,
}

/// Which instances exist, and what each one links to.
///
/// The ring is over *instances* rather than over classes, and that is what removes the one
/// special case this generator would otherwise have. Every instance links to the next and
/// the last links to the first, so every node has an outgoing edge — which `orphan-out`
/// requires at `Error` — and every node has an inbound one. A single class gets two
/// instances instead of one node linking to itself: a self-edge satisfies both checks and
/// teaches the wrong thing, and `pointed_classes` reads it as naming neither end.
fn instance_ring(classes: &[String]) -> Vec<(usize, &'static str)> {
    match classes.len() {
        1 => vec![(0, "example"), (0, "example-2")],
        n => (0..n).map(|i| (i, "example")).collect(),
    }
}

/// The edge declarations each class carries, derived from the instance ring.
///
/// Returned as `(relationship, target, direction)` per class so the class file and the
/// instance links cannot disagree: both are read off the same ring. Each hop `a → b`
/// declares `out` on `a`'s class and `in` on `b`'s class, which is the convention the
/// by-hand page states — `direction` says who authors the link, and both ends declare it so
/// neither file can be read as the whole story.
///
/// Only the authoring end licenses anything: `unlicensed_edge` and `edge_target_class` read
/// the class of the node the link is written in and nothing else. The `in` declaration is
/// there for a reader and for `pointed_classes`, not for the gate.
fn edge_declarations(classes: &[String], ring: &[(usize, &str)]) -> Vec<Vec<(String, String)>> {
    let mut out: Vec<Vec<(String, String)>> = vec![Vec::new(); classes.len()];
    for (i, (from, _)) in ring.iter().enumerate() {
        let (to, _) = ring[(i + 1) % ring.len()];
        push_unique(&mut out[*from], (classes[to].clone(), "out".to_string()));
        push_unique(&mut out[to], (classes[*from].clone(), "in".to_string()));
    }
    out
}

fn push_unique(list: &mut Vec<(String, String)>, entry: (String, String)) {
    if !list.contains(&entry) {
        list.push(entry);
    }
}

/// `concept` → `Concept`, `mixing-zone` → `Mixing zone`. The label a reader would have
/// typed, so that the placeholder does not also need editing to be well-formed.
fn titlecase(slug: &str) -> String {
    let spaced = slug.replace('-', " ");
    let mut chars = spaced.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => spaced,
    }
}

/// One `<class>.ont.yml`.
///
/// The comments are the point of the file. A reader who copies a class out of a page learns
/// the key set and nothing about which parts of it are decisions; the three that are — what
/// `direction` settles, what `type: claim` buys, and what an undeclared edge means — are the
/// three the `starting-a-corpus` skill says go wrong on the first attempt, every time.
fn class_file(name: &str, edges: &[(String, String)]) -> Written {
    let mut body = format!(
        "\
# A class: the kind of thing this corpus holds, and what may be said about one.
#
# `yidam init` wrote this. The shape is the same in every corpus; the names and
# the prose are the part only you know. Rewrite them before you write nodes —
# an edge into a node is written by its name, and renaming severs edges.
class: {name}
label: {label}
description: >-
  Rewrite this sentence. A class description says what an instance IS — the
  shape you would recognise one by, and what admits something to the class.
  Keep it a description of a kind: a claim about why the class is here is
  asserted identically by every instance and no tag can attribute it.
properties:
  # Declaring a property `claim` makes its *value* a tag: written bare, the way
  # `claim_tag: open` is in the nodes, the reports read it as a standing.
  # Undeclared it is an ordinary string and no report counts it. A bracketed
  # `[open]` in prose is counted either way — what this buys is the standing as
  # a field rather than as text a scan happens to find.
  - name: {CLAIM_PROPERTY}
    type: claim
    description: How well established this node's claim is.
edges:
",
        label = titlecase(name),
    );
    for (target, direction) in edges {
        let note = match direction.as_str() {
            "out" => "Rename this. The relationship is the second thing only you know.",
            _ => "The same edge from the other end, declared so both files agree.",
        };
        let _ = write!(
            body,
            "\
  # `direction` decides which file authors the link: `{direction}` means it is written
  # {where_written}. Declaring one edge `out` at both ends gets two edges, not one.
  - relationship: {RELATIONSHIP}
    target: {target}
    direction: {direction}
    description: {note}
",
            where_written = match direction.as_str() {
                "out" => "in an instance of this class".to_string(),
                _ => format!("in the `{target}` at the other end"),
            },
        );
    }
    body.push_str(
        "\
# An edge no class declares is a broken link rather than a new kind, and
# `yidam graph-check` reports it against the file at the other end. Say which
# this list is once you know: `exhaustive` closes the vocabulary and makes
# anything outside it an error; `characteristic` says an undeclared
# relationship is a deliberate coinage. Saying nothing is warned about.
# edge_policy: characteristic
",
    );
    Written {
        rel: format!(".yidam/corpus/{name}.ont.yml"),
        body,
    }
}

/// One `<class>/<slug>.yml`, linking at the next instance in the ring.
///
/// `[open]` rather than `[verified]`, and that is not a placeholder detail. A `[verified]`
/// claim resting on no catalog entry is a `verified-unsourced` finding on the first run, and
/// a corpus whose every claim is settled on the day it is created is a corpus whose tags are
/// decoration. `open-questions` answering with something is the other half: the report is
/// how a reader sees that the tag did anything.
fn instance_file(class: &str, slug: &str, target_class: &str, target_slug: &str) -> Written {
    let body = format!(
        "\
class: {class}
label: {label}
description: >-
  Replace this with something you actually know. A node is one claim's worth of
  understanding, and the tag says how well established it is. Nobody has
  settled this one. [open]
properties:
  {CLAIM_PROPERTY}: open
links:
  # Relative to this file, which is why it starts `../`. This class declared
  # `{RELATIONSHIP}` as `out`, so the link is authored here and the node at the
  # other end gets it without being edited.
  - target: ../{target_class}/{target_slug}.yml
    relationship: {RELATIONSHIP}
",
        label = titlecase(slug).replace("Example", &format!("Example {class}")),
    );
    Written {
        rel: format!(".yidam/corpus/{class}/{slug}.yml"),
        body,
    }
}

/// The one catalog entry.
///
/// `obtained: false`, which is the field carrying the difference between a source registered
/// ahead of the work that will use it and a claim resting on evidence that never became a
/// node. An entry declaring the first is not reported by `catalog-uncited`, so the tree this
/// command writes has a catalog directory with something in it — git tracks no empty
/// directory — and a clean first `lint`.
fn catalog_file() -> Written {
    Written {
        rel: ".yidam/catalog/example-source.md".to_string(),
        body: "\
---
name: Example source
description: Replace this with a real source — a paper, a dataset, an interview.
type: paper
obtained: false
---

A catalog entry registers where knowledge came from. A node draws on one with an
ordinary markdown link into this directory, written in the node's prose, and that
is the whole citation mechanism — nothing else has to be configured.

`obtained: false` says this source has been named and not yet read. Change it to
`true` once it has been, and `lint` starts asking which nodes rest on it: a
`[verified]` claim resting on nothing is a finding, and this is what it rests on.
"
        .to_string(),
    }
}

/// Every file `init` writes, for a given class list. Pure, so the tree can be asserted on
/// without one being created — and so `tests/init_corpus.rs` can build it and run the gates.
pub fn corpus_files(classes: &[String]) -> Vec<Written> {
    let ring = instance_ring(classes);
    let edges = edge_declarations(classes, &ring);

    let mut files: Vec<Written> = classes
        .iter()
        .enumerate()
        .map(|(i, name)| class_file(name, &edges[i]))
        .collect();

    for (i, (class, slug)) in ring.iter().enumerate() {
        let (to_class, to_slug) = ring[(i + 1) % ring.len()];
        files.push(instance_file(
            &classes[*class],
            slug,
            &classes[to_class],
            to_slug,
        ));
    }

    files.push(catalog_file());
    files
}

/// Why a class name was refused, or `None`.
///
/// A class name is half of every node id under it, and those ids are rendered into four
/// incompatible encodings — `name_not_a_slug` is the check and its docstring has the table.
/// Refusing here rather than writing the file and letting `lint` report it is the difference
/// between a typo caught in the second before the corpus exists and one caught after nodes
/// carry it: renaming is what costs, because an edge into a node is written by its name.
fn why_not_a_class(name: &str) -> Option<String> {
    if name.is_empty() {
        return Some("a class name may not be empty".to_string());
    }
    if !yidam_core::uri::is_slug(name) {
        return Some(format!(
            "`{name}` is not a slug — lowercase letters, digits and single interior hyphens \
             only. A class name is half of every node id under it, and those ids are \
             rendered into URIs by surfaces that encode differently"
        ));
    }
    None
}

/// The class list to write: what `--class` named, or [`DEFAULT_CLASSES`].
fn resolve_classes(named: &[String]) -> Result<Vec<String>> {
    if named.is_empty() {
        return Ok(DEFAULT_CLASSES.iter().map(|c| c.to_string()).collect());
    }
    let mut seen: Vec<String> = Vec::new();
    for name in named {
        if let Some(why) = why_not_a_class(name) {
            bail!("{why}");
        }
        if seen.contains(name) {
            bail!(
                "`--class {name}` was named twice. One class is one `.ont.yml`, and the second \
                 would overwrite the first."
            );
        }
        seen.push(name.clone());
    }
    Ok(seen)
}

/// Refuse a tree that is not somewhere a corpus can live.
///
/// Two refusals, and the second is the one worth arguing for. **A corpus is a git history
/// with a gate over it**: `lint` dates a finding against that history, `log`, `replay`,
/// `propose` and `status` read it, and the first commit's `genesis:` subject is where
/// `export` and `bundle` get the domain name. So `init` asks for a repository rather than
/// creating one — `git init` in a directory somebody is standing in is a larger thing to do
/// unasked than to name in one line.
///
/// The `.yidam/` refusal is `clone`'s and `overlay`'s, said the same way. A corpus that
/// exists is not one to scaffold over, and the files this command writes are placeholders:
/// overwriting a class somebody wrote with one telling them to rewrite it is the worst
/// outcome available here.
fn require_somewhere_to_write(root: &Path) -> Result<()> {
    if root.join(".yidam").exists() {
        bail!(
            ".yidam/ already exists in {} — this is already a corpus\n  \
             `init` writes a skeleton and would overwrite what is there. To add a class to a \
             corpus that exists, write its `.ont.yml` beside the others; \
             `yidam schema` will pick it up.",
            root.display()
        );
    }
    if !crate::git::Git::new(root)
        .args(["rev-parse", "--show-toplevel"])
        .succeeded()
    {
        bail!(
            "not inside a git repository: {}\n  \
             A corpus is a git history with a gate over it — `lint` dates a finding against \
             that history, and `export` reads the domain name off the first commit's \
             `genesis:` subject. Start one first:\n      git init\n  \
             `init` does not do it for you: creating a repository in a directory you are \
             standing in is a larger thing to do unasked than to say in one line.",
            root.display()
        );
    }
    Ok(())
}

/// Write the corpus into `root`. Split from [`init`] so the refusals and the tree can be
/// asked of a fixture, which is the discovery [`init`] adds and the only part that needs a
/// real shell to be standing anywhere.
fn init_into(root: &Path, named: &[String]) -> Result<Vec<PathBuf>> {
    let classes = resolve_classes(named)?;
    require_somewhere_to_write(root)?;

    let mut written = Vec::new();
    for file in corpus_files(&classes) {
        let path = root.join(&file.rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, &file.body)?;
        written.push(PathBuf::from(&file.rel));
    }
    Ok(written)
}

pub fn init(classes: &[String]) -> Result<()> {
    let root = repo_root()?;
    let written = init_into(&root, classes)?;

    println!("Wrote {} file(s) into {}:", written.len(), root.display());
    for rel in &written {
        println!("  {}", rel.display());
    }
    println!();
    println!("Next steps:");
    println!("  1. Open the .ont.yml files and say what your kinds actually are.");
    println!("     That is the whole of the work and the one part no tool performs.");
    println!("  2. Replace the example nodes. Keep the [open] tags honest — a tag");
    println!("     nobody has settled is the corpus working, not a gap to fill in.");
    println!("  3. yidam graph-check && yidam lint");
    println!("  4. git add -A && git commit -m 'genesis: <your domain>'");
    println!();
    println!("`yidam schema` writes JSON Schema for what you declared, so an editor");
    println!("completes property names and rejects a class you have not.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    fn rels(files: &[Written]) -> Vec<String> {
        files.iter().map(|f| f.rel.clone()).collect()
    }

    fn classes(names: &[&str]) -> Vec<String> {
        names.iter().map(|n| n.to_string()).collect()
    }

    /// The default tree, listed rather than counted.
    ///
    /// **Five, where #1035's title says six.** The six is
    /// `docs/first-corpus-by-hand.md`'s worked example, which has two classes, *three*
    /// nodes and a catalog entry — the third node is a second `concept`, there to show
    /// that a class holds more than one thing. The ring gives a reader that for free: two
    /// nodes already link in both directions, and a third would be a node with nothing new
    /// in it to edit. Changing this list is a decision about what a first reader is handed,
    /// so it is asserted by name and not by length.
    #[test]
    fn the_default_corpus_is_the_five_files_listed_here() {
        let files = corpus_files(&resolve_classes(&[]).unwrap());
        assert_eq!(
            rels(&files),
            vec![
                ".yidam/corpus/concept.ont.yml",
                ".yidam/corpus/observation.ont.yml",
                ".yidam/corpus/concept/example.yml",
                ".yidam/corpus/observation/example.yml",
                ".yidam/catalog/example-source.md",
            ],
            "the default tree changed shape"
        );
    }

    /// `orphan-out` is an `Error`, so a node with no outgoing link fails the first gate a
    /// reader runs. The ring is what makes that impossible for any class count, and this
    /// asserts it as the property rather than checking the two-class case by hand.
    #[test]
    fn every_instance_the_ring_writes_has_an_outgoing_link() {
        for n in 1..=5 {
            let names: Vec<String> = (0..n).map(|i| format!("kind-{i}")).collect();
            let ring = instance_ring(&names);
            assert!(
                ring.len() >= 2,
                "{n} class(es) produced {} instance(s) — a ring of one is a self-link",
                ring.len()
            );
            let files = corpus_files(&names);
            let instances: Vec<&Written> = files
                .iter()
                .filter(|f| f.rel.ends_with(".yml") && !f.rel.ends_with(".ont.yml"))
                .collect();
            assert_eq!(instances.len(), ring.len());
            for inst in instances {
                assert!(
                    inst.body.contains("links:") && inst.body.contains("  - target: ../"),
                    "{} carries no outgoing link:\n{}",
                    inst.rel,
                    inst.body
                );
            }
        }
    }

    /// The other half: every instance is also pointed at, so a reader's first `lint` has no
    /// `orphan-in` to explain either. A ring gives both at once and a chain gives neither at
    /// the ends, which is the mutation this catches.
    #[test]
    fn every_instance_the_ring_writes_is_pointed_at() {
        for n in 1..=5 {
            let names: Vec<String> = (0..n).map(|i| format!("kind-{i}")).collect();
            let files = corpus_files(&names);
            let targets: BTreeSet<String> = files
                .iter()
                .flat_map(|f| f.body.lines())
                .filter_map(|l| l.trim().strip_prefix("- target: ../"))
                .map(|t| format!(".yidam/corpus/{t}"))
                .collect();
            let instances: BTreeSet<String> = files
                .iter()
                .map(|f| f.rel.clone())
                .filter(|r| r.ends_with(".yml") && !r.ends_with(".ont.yml"))
                .collect();
            assert_eq!(
                targets, instances,
                "{n} class(es): not every node is a target"
            );
        }
    }

    /// Each hop's authoring end declares it. `unlicensed_edge` reads the class of the node
    /// the link is written in and nothing else, so this is the declaration the gate needs —
    /// and a generator that wrote only the `in` half would produce a warning per node.
    #[test]
    fn the_authoring_class_declares_the_edge_it_authors() {
        let names = classes(&["alpha", "beta", "gamma"]);
        let ring = instance_ring(&names);
        let edges = edge_declarations(&names, &ring);
        for (i, name) in names.iter().enumerate() {
            let next = &names[(i + 1) % names.len()];
            assert!(
                edges[i].contains(&(next.clone(), "out".to_string())),
                "`{name}` authors a link to a `{next}` and does not declare it: {:?}",
                edges[i]
            );
            let prev = &names[(i + names.len() - 1) % names.len()];
            assert!(
                edges[i].contains(&(prev.clone(), "in".to_string())),
                "`{name}` is pointed at by a `{prev}` and does not declare it: {:?}",
                edges[i]
            );
        }
    }

    /// A single class is the case a chain cannot serve and a self-link serves wrongly.
    #[test]
    fn one_class_gets_two_instances_rather_than_a_self_link() {
        let files = corpus_files(&classes(&["thing"]));
        assert_eq!(
            rels(&files),
            vec![
                ".yidam/corpus/thing.ont.yml",
                ".yidam/corpus/thing/example.yml",
                ".yidam/corpus/thing/example-2.yml",
                ".yidam/catalog/example-source.md",
            ]
        );
        for f in &files {
            assert!(
                !f.body
                    .contains("- target: ../thing/example.yml\n    relationship")
                    || f.rel.ends_with("example-2.yml"),
                "{} links at itself",
                f.rel
            );
        }
    }

    /// The three refusals, each with the thing it refuses named in the message.
    #[test]
    fn a_class_name_that_is_not_a_slug_is_refused() {
        for bad in ["Concept", "mixing zone", "", "trailing-", "a--b"] {
            let err = resolve_classes(&[bad.to_string()])
                .expect_err(&format!("`{bad}` was accepted as a class name"))
                .to_string();
            assert!(
                err.contains("class name") || err.contains(bad),
                "the refusal for `{bad}` names neither it nor what it is: {err}"
            );
        }
        // The paired assertion: a refusal that refused everything would satisfy the loop.
        assert!(resolve_classes(&["mixing-zone".to_string()]).is_ok());
    }

    #[test]
    fn the_same_class_named_twice_is_refused() {
        let err = resolve_classes(&classes(&["a", "b", "a"]))
            .unwrap_err()
            .to_string();
        assert!(err.contains("named twice"), "{err}");
    }

    #[test]
    fn a_corpus_that_exists_is_not_scaffolded_over() {
        let tmp = tempfile::TempDir::new().unwrap();
        crate::git::fixture::init(tmp.path());
        std::fs::create_dir_all(tmp.path().join(".yidam/corpus")).unwrap();
        let err = init_into(tmp.path(), &[]).unwrap_err().to_string();
        assert!(err.contains("already exists"), "{err}");
    }

    #[test]
    fn a_directory_outside_git_is_refused_with_the_line_that_fixes_it() {
        let tmp = tempfile::TempDir::new().unwrap();
        let err = init_into(tmp.path(), &[]).unwrap_err().to_string();
        assert!(err.contains("not inside a git repository"), "{err}");
        assert!(
            err.contains("git init"),
            "the refusal names no remedy: {err}"
        );
    }

    /// The names are refused before the directory is looked at, because a typo in a class
    /// name is the cheapest thing to fix and the most expensive to fix later.
    #[test]
    fn a_bad_class_name_is_refused_before_anything_is_written() {
        let tmp = tempfile::TempDir::new().unwrap();
        crate::git::fixture::init(tmp.path());
        assert!(init_into(tmp.path(), &["Concept".to_string()]).is_err());
        assert!(
            !tmp.path().join(".yidam").exists(),
            "a refused run left a .yidam/ behind"
        );
    }

    #[test]
    fn the_files_land_where_the_listing_says_they_do() {
        let tmp = tempfile::TempDir::new().unwrap();
        crate::git::fixture::init(tmp.path());
        let written = init_into(tmp.path(), &classes(&["reach", "gage"])).unwrap();
        assert!(!written.is_empty());
        for rel in &written {
            assert!(
                tmp.path().join(rel).is_file(),
                "`{}` was reported and is not there",
                rel.display()
            );
        }
    }

    /// A class description asserting a purpose is `class-asserts-purpose`, and the
    /// placeholder prose is the one text every corpus created this way inherits. It would be
    /// a finding shipped into every one of them.
    #[test]
    fn the_placeholder_prose_trips_no_check_it_is_telling_the_reader_about() {
        for f in corpus_files(&resolve_classes(&[]).unwrap()) {
            if !f.rel.ends_with(".ont.yml") {
                continue;
            }
            let class = crate::corpus::Class::parse(&f.rel, &f.body);
            assert!(class.malformed.is_none(), "{} did not parse", f.rel);
            let check = crate::cmd::lint::checks::class_asserts_purpose(&[class]);
            assert!(
                check.violations.is_empty(),
                "{} asserts a purpose: {:?}",
                f.rel,
                check.violations
            );
        }
    }

    #[test]
    fn titlecase_reads_as_a_label() {
        assert_eq!(titlecase("concept"), "Concept");
        assert_eq!(titlecase("mixing-zone"), "Mixing zone");
    }
}
