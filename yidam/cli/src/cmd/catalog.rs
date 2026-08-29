//! `yidam catalog-audit` — what the catalog holds, and who draws on it.
//!
//! # Two counts, kept apart
//!
//! This report and `lint` both count the corpus files citing a catalog entry, through the
//! same [`linked_paths`](crate::cmd::lint::checks::linked_paths) — a function they were
//! deliberately unified on after each had its own copy. They still disagreed, because the
//! *walk* was left duplicated: `lint` reads instances, and this read every `.md` and `.yml`
//! under the corpus, which admits class definitions and every README.
//!
//! Measured on three derived repositories, the disagreement was 2, 8 and 8 entries out of
//! 76, 21 and 41 — a number, never a verdict. It was also not noise. The files in dispute
//! are of two kinds and only one of them is decoration:
//!
//! ```text
//! education-agency.ont.yml   [verified — [the audit reports …](../catalog/…)]
//! definition/README.md       [verified], against [quietus-2009-greenway](…)
//! corpus/README.md           … [SB 11 of the 135th](../catalog/sb-11-135th-record.md) and
//!                            [HB 290 …], standalone school-choice vehicles that …
//! ```
//!
//! The first two are claims resting on a source. The third names two records to say they are
//! **out** of scope. So neither summing them nor dropping them is right, and the resolution
//! is the one `catalog_used_by_drift` already uses for its own two numbers: *both are kept so
//! the disagreement is visible rather than averaged away.*
//!
//! **Every gate reads [`Counts::nodes`]**, which is what `catalog-uncited` has always meant
//! when it says *"no corpus node draws on this source"*. The wording was precise; the
//! single unlabelled column beside it was not.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::parse::parse_frontmatter;
use crate::paths::{repo_root, yidam_catalog_dir, yidam_corpus_dir};
use crate::regen::update_file_regen;
use crate::walk::{walk_corpus_instances, walk_md_files};

/// Who draws on one catalog entry, by repo-relative path.
///
/// The paths, not only the tally. A count answers *is this source used*; an editor placing
/// a source under the node that cites it needs to know **which** node, and the walk that
/// produces the count already knows. Discarding it here only to make a consumer walk the
/// corpus again is how a client ends up re-implementing the resolver — the failure this
/// whole report contract exists to close.
///
/// Each list is distinct files: [`crate::cmd::lint::checks::linked_paths`] returns a set per
/// file, so one file linking to the same entry twice is one citation and always was. The
/// counts are `len()` of these, which is what keeps the two from ever disagreeing.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct Cited {
    /// Corpus instances that link here. **The list every gate's count reads.**
    nodes: Vec<String>,
    /// Other files under `.yidam/corpus/` that link here — class definitions and READMEs.
    ///
    /// Reported beside [`Self::nodes`] and never added to it. A claim resting on a source and
    /// a page linking to one are different things, and only the first is evidence.
    elsewhere: Vec<String>,
}

impl Cited {
    fn counts(&self) -> Counts {
        Counts {
            nodes: self.nodes.len(),
            elsewhere: self.elsewhere.len(),
        }
    }

    fn sorted(mut self) -> Self {
        self.nodes.sort();
        self.elsewhere.sort();
        self
    }
}

/// The two counts, kept exactly as the contract has always emitted them.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize)]
struct Counts {
    /// Corpus instances that link here. **The number every gate reads.**
    nodes: usize,
    /// Other files under `.yidam/corpus/` that link here — class definitions and READMEs.
    elsewhere: usize,
}

impl Counts {
    fn total(self) -> usize {
        self.nodes + self.elsewhere
    }
}

#[derive(serde::Serialize)]
struct SourceRow {
    entry: String,
    #[serde(rename = "type")]
    kind: String,
    description: String,
    obtained: bool,
    /// Every corpus file linking here, of whatever kind.
    ///
    /// Retained, and retained with exactly its previous meaning and value: this field is the
    /// report contract's, and narrowing what a field means is the same break as removing it.
    /// `nodes` and `elsewhere` decompose it rather than replacing it.
    citations: usize,
    #[serde(flatten)]
    counts: Counts,
    /// The corpus instances linking here, repo-relative and sorted.
    ///
    /// Instances only, and deliberately asymmetric with [`Counts::elsewhere`], which stays a
    /// count. A class definition or a README is not a node, so it has no row in a corpus
    /// view to sit under — listing those paths would be a field with no reader, and the
    /// count already tells you they exist.
    cited_by: Vec<String>,
    /// The entry's declared `used-by`, verbatim. Empty when it declares none.
    used_by: Vec<String>,
    /// How that list disagrees with the citations, or `null` when no list is declared.
    ///
    /// Null and an empty drift are different answers — the first is an entry that never
    /// claimed anything, the second is one whose claim holds — and `catalog-used-by-drift`
    /// draws the same distinction from the same function.
    drift: Option<crate::cmd::lint::checks::UsedByDrift>,
}

#[derive(serde::Serialize)]
struct CatalogReport {
    sources: Vec<SourceRow>,
}

/// Every corpus file's links, resolved once and tallied by whether the file is a node.
///
/// One pass over the corpus, links resolved once per file — the shape
/// [`crate::cmd::lint::checks::citations`] already uses. This used to re-read the whole
/// corpus for *every* catalog entry, which on the largest instrumented repository is 76
/// entries against 308 files: twenty-three thousand reads to print one table.
///
/// Keyed by every link target, not only catalog entries. Filtering to the catalog would mean
/// deciding here what a catalog path looks like, and the caller already holds the entries.
fn draws_on(root: &Path, corpus: &Path) -> HashMap<PathBuf, Cited> {
    // What counts as a node comes from the one function that answers that, rather than from
    // a filter written again here. The duplicated *walk* is the whole of this defect.
    let instances: HashSet<PathBuf> = walk_corpus_instances(corpus)
        .iter()
        .map(|p| crate::cmd::lint::checks::normalize(p))
        .collect();

    let mut out: HashMap<PathBuf, Cited> = HashMap::new();
    for entry in walkdir::WalkDir::new(corpus)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
    {
        let path = entry.path();
        if !path.extension().is_some_and(|x| x == "md" || x == "yml") {
            continue;
        }
        let text = std::fs::read_to_string(path).unwrap_or_default();
        let rel = path.strip_prefix(root).unwrap_or(path).to_string_lossy();
        let is_node = instances.contains(&crate::cmd::lint::checks::normalize(path));
        for target in crate::cmd::lint::checks::linked_paths(path, &rel, &text) {
            let c = out.entry(target).or_default();
            if is_node {
                c.nodes.push(rel.to_string());
            } else {
                c.elsewhere.push(rel.to_string());
            }
        }
    }
    // Sorted here rather than at each use: the walk order is `walkdir`'s, which is neither
    // stable across platforms nor meaningful, and a report field whose order moves between
    // machines is one no golden can pin.
    out.into_iter().map(|(k, v)| (k, v.sorted())).collect()
}

/// The legend under the table.
///
/// Generated with the block rather than written once into the README, because a REGEN block
/// that explains itself somewhere else is one a re-vendored repository never receives.
const LEGEND: &str = "\n\n**Nodes** counts corpus instances that link here — the number \
     every gate reads, and what `catalog-uncited` means by *no corpus node draws on this \
     source*. **Elsewhere** counts other files under `.yidam/corpus/` that link here: class \
     definitions and README prose. They are kept apart rather than summed, because a claim \
     resting on a source and a page linking to one are different things.";

pub fn catalog_audit(format: crate::report::Format) -> Result<()> {
    let root = repo_root()?;
    let catalog = yidam_catalog_dir(&root);
    let entries = walk_md_files(&catalog);

    // Gathered in the same pass as the markdown rows so the two cannot disagree about
    // what the catalog holds.
    let mut source_rows: Vec<SourceRow> = Vec::new();

    let content = if entries.is_empty() {
        "_No catalog entries yet._".to_string()
    } else {
        let corpus = yidam_corpus_dir(&root);
        let draws = if corpus.exists() {
            draws_on(&root, &corpus)
        } else {
            HashMap::new()
        };

        // The REGEN marker in catalog/README.md promises source type and retrieval status
        // alongside the citation count; both were specified and neither was emitted.
        let mut rows = vec![
            "| Entry | Type | Description | Obtained | Nodes | Elsewhere |".to_string(),
            "|---|---|---|---|---|---|".to_string(),
        ];
        for path in &entries {
            let text = std::fs::read_to_string(path).unwrap_or_default();
            let fm = parse_frontmatter(&text);
            let desc = fm.description.unwrap_or_else(|| "—".to_string());
            let kind = fm.r#type.unwrap_or_else(|| "—".to_string());
            // Absent means obtained; only an explicit `false` claims otherwise.
            let obtained = if fm.obtained.unwrap_or(true) {
                "yes"
            } else {
                "not yet"
            };
            let is_obtained = fm.obtained.unwrap_or(true);
            let filename = path.file_name().unwrap_or_default().to_string_lossy();
            // A citation is a link that resolves to this entry, not a sentence containing
            // its slug. `lint` had the same defect in its own copy of this count; both now
            // ask one function. See [`crate::cmd::lint::checks::linked_paths`].
            let cited = draws
                .get(&crate::cmd::lint::checks::normalize(path))
                .cloned()
                .unwrap_or_default();
            let counts = cited.counts();
            let used_by = fm.used_by.unwrap_or_default();
            // Against `cited.nodes` and not `cited.total()`: `catalog-used-by-drift` compares
            // the list to the citations *the gate reads*, and a report disagreeing with the
            // gate about which entries have drifted is the one defect the shared function
            // above exists to make impossible.
            let drift = crate::cmd::lint::checks::used_by_drift(&used_by, &cited.nodes);
            source_rows.push(SourceRow {
                entry: filename.to_string(),
                kind: kind.clone(),
                description: desc.clone(),
                obtained: is_obtained,
                citations: counts.total(),
                counts,
                cited_by: cited.nodes,
                used_by,
                drift,
            });
            rows.push(format!(
                "| [{filename}]({filename}) | {kind} | {desc} | {obtained} | {} | {} |",
                counts.nodes, counts.elsewhere
            ));
        }
        format!("{}{LEGEND}", rows.join("\n"))
    };

    if format.is_json() {
        return crate::report::emit(
            &root,
            CatalogReport {
                sources: source_rows,
            },
        );
    }

    crate::regen::emit(&content);
    update_file_regen(&catalog.join("README.md"), "yidam catalog-audit", &content)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn corpus(root: &Path) -> PathBuf {
        let c = root.join(".yidam/corpus");
        std::fs::create_dir_all(c.join("concept")).unwrap();
        std::fs::create_dir_all(root.join(".yidam/catalog")).unwrap();
        std::fs::write(root.join(".yidam/catalog/source.md"), "# Source\n").unwrap();
        c
    }

    fn write(root: &Path, rel: &str, body: &str) {
        let p = root.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, body).unwrap();
    }

    fn cited(root: &Path) -> Cited {
        let c = root.join(".yidam/corpus");
        draws_on(root, &c)
            .get(&crate::cmd::lint::checks::normalize(
                &root.join(".yidam/catalog/source.md"),
            ))
            .cloned()
            .unwrap_or_default()
    }

    fn counts(root: &Path) -> Counts {
        cited(root).counts()
    }

    #[test]
    fn an_instance_linking_to_an_entry_is_a_node_citation() {
        let tmp = tempfile::tempdir().unwrap();
        corpus(tmp.path());
        write(
            tmp.path(),
            ".yidam/corpus/concept/low-flow.yml",
            "class: concept\ndescription: |\n  Drawn from [it](../../catalog/source.md).\n",
        );
        assert_eq!(
            counts(tmp.path()),
            Counts {
                nodes: 1,
                elsewhere: 0
            }
        );
    }

    /// The defect #346 reports: a README is not a node, and counting it as one made this
    /// report and `lint` disagree about a number they compute with the same function.
    #[test]
    fn a_readme_linking_to_an_entry_is_counted_elsewhere() {
        let tmp = tempfile::tempdir().unwrap();
        corpus(tmp.path());
        write(
            tmp.path(),
            ".yidam/corpus/concept/README.md",
            "Obtained from [it](../../catalog/source.md).\n",
        );
        let c = counts(tmp.path());
        assert_eq!(
            c,
            Counts {
                nodes: 0,
                elsewhere: 1
            }
        );
        assert_eq!(c.total(), 1, "the old field keeps its old value");
    }

    /// A class description carrying `[verified — [source](…)]` is real provenance, and the
    /// narrow reading would have stopped counting it anywhere at all.
    #[test]
    fn a_class_definition_linking_to_an_entry_is_counted_elsewhere() {
        let tmp = tempfile::tempdir().unwrap();
        corpus(tmp.path());
        write(
            tmp.path(),
            ".yidam/corpus/concept.ont.yml",
            "class: concept\ndescription: |\n  Rests on [it](../catalog/source.md).\n",
        );
        assert_eq!(
            counts(tmp.path()),
            Counts {
                nodes: 0,
                elsewhere: 1
            }
        );
    }

    /// The two are reported apart and summed nowhere but in the retained wire field.
    #[test]
    fn the_two_kinds_are_counted_separately_and_sum_to_the_old_number() {
        let tmp = tempfile::tempdir().unwrap();
        corpus(tmp.path());
        write(
            tmp.path(),
            ".yidam/corpus/concept/low-flow.yml",
            "class: concept\ndescription: |\n  Drawn from [it](../../catalog/source.md).\n",
        );
        write(
            tmp.path(),
            ".yidam/corpus/concept/README.md",
            "Indexed from [it](../../catalog/source.md).\n",
        );
        let c = counts(tmp.path());
        assert_eq!(
            c,
            Counts {
                nodes: 1,
                elsewhere: 1
            }
        );
        assert_eq!(c.total(), 2);
    }

    #[test]
    fn an_entry_nothing_links_to_counts_zero_of_each() {
        let tmp = tempfile::tempdir().unwrap();
        corpus(tmp.path());
        write(
            tmp.path(),
            ".yidam/corpus/concept/low-flow.yml",
            "class: concept\ndescription: |\n  Nothing here.\n",
        );
        assert_eq!(counts(tmp.path()), Counts::default());
    }

    /// The count is the list's length, which is the only arrangement in which they cannot
    /// come to disagree — the defect this whole module's header describes, in miniature.
    #[test]
    fn the_citing_instances_are_named_sorted_and_the_count_is_their_number() {
        let tmp = tempfile::tempdir().unwrap();
        corpus(tmp.path());
        for name in ["tailwater", "low-flow"] {
            write(
                tmp.path(),
                &format!(".yidam/corpus/concept/{name}.yml"),
                "class: concept\ndescription: |\n  Drawn from [it](../../catalog/source.md).\n",
            );
        }
        let c = cited(tmp.path());
        assert_eq!(
            c.nodes,
            vec![
                ".yidam/corpus/concept/low-flow.yml",
                ".yidam/corpus/concept/tailwater.yml",
            ],
            "sorted, because walkdir's order is neither stable nor meaningful"
        );
        assert_eq!(c.counts().nodes, c.nodes.len());
    }

    /// A README is not a node, so it has no row to sit under. Counted, never named.
    #[test]
    fn a_non_instance_is_counted_and_not_named() {
        let tmp = tempfile::tempdir().unwrap();
        corpus(tmp.path());
        write(
            tmp.path(),
            ".yidam/corpus/concept/README.md",
            "Indexed from [it](../../catalog/source.md).\n",
        );
        let c = cited(tmp.path());
        assert!(c.nodes.is_empty());
        assert_eq!(c.counts().elsewhere, 1);
        assert_eq!(
            c.elsewhere,
            vec![".yidam/corpus/concept/README.md"],
            "the walk knows which file it was; only the report withholds it"
        );
    }

    /// One file linking twice is one citation, which is what the count meant before.
    #[test]
    fn a_file_linking_twice_is_counted_once() {
        let tmp = tempfile::tempdir().unwrap();
        corpus(tmp.path());
        write(
            tmp.path(),
            ".yidam/corpus/concept/low-flow.yml",
            "class: concept\ndescription: |\n  [a](../../catalog/source.md) and \
             [b](../../catalog/source.md).\n",
        );
        assert_eq!(
            counts(tmp.path()),
            Counts {
                nodes: 1,
                elsewhere: 0
            }
        );
    }
}
