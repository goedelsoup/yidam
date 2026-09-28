//! What the artifact cites of its corpus, and whether those citations still land — RFC-0028
//! A6 (#577).
//!
//! A repository that declares `[object] paths` holds two things in one tree: a corpus under
//! `.yidam/` and an artifact beside it. Each cites the other. This module reads the links
//! between them, in both directions, for `yidam kuten check` to count and for
//! `broken-object-link` to hold.
//!
//! # The form is the markdown link, and that was measured
//!
//! RFC-0028 §6 proposed reusing RFC-0019's `cites:`. On 2026-09-28 none of the fifteen derived
//! repositories on disk had written one: the key appears only in vendored prelude text. The
//! corpora already cite in the other form — a relative markdown link. The three with an
//! artifact beside the corpus hold 376, 315 and 35 of them from `.yidam/` into the rest of the
//! tree. A check over `cites:` would have counted a form nobody writes and reported every
//! repository as uncoupled. Erratum 6 in RFC-0028 records the correction.
//!
//! # One direction was already held
//!
//! A link from the corpus into the artifact sits in prose that `broken-prose-link` walks.
//! Moving one crate in a derived corpus turned four of those links red, and a `#L` range on
//! such a link is held by the line-citation checks (#563). The other direction was read by
//! nothing. Deleting a node that a crate's README linked to produced twenty findings, and none
//! named the README. That direction is what this module adds.
//!
//! # Which files are the artifact
//!
//! The tracked files that `[object] paths` claims, with the extensions in
//! [`CITING_EXTENSIONS`]. Tracked rather than walked, for the reason [`crate::cmd::tracked`]
//! gives: a list of build directories to skip grows one entry per accident.
//!
//! A file under an authorship region is not read. `excluded` means do not look. A `generated`
//! or `imported` file's link is its generator's or its upstream's citation, and counting it
//! would credit the artifact with a citation nobody here wrote.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::authorship::Authorship;
use crate::cmd::lint::checks::{self, ProseLink};
use crate::corpus::edges::normalize;
use crate::kuten::{Register, Registers};

/// The file types an artifact cites its corpus from.
///
/// Measured across the fifteen derived repositories on 2026-09-28: of the 1,598 relative
/// links into `.yidam/` written outside it, 1,518 sit in `.md`, 64 in `.rs` doc comments and
/// 16 in `.ts`. A link inside a Rust or TypeScript comment has the same syntax as one in
/// markdown, so one parser reads all three.
///
/// JSON is not read. The JSON in an artifact is generated, and where it carries corpus prose
/// — one site's graph feed holds about three thousand markdown links — each link is written
/// relative to the node it came from, not to the JSON file.
///
/// Bare paths are not read. A string such as `".yidam/corpus/site/x.yml"` in code is as often
/// a path the code writes as one it cites: of 807 bare corpus paths in those three file types,
/// the 8 that do not resolve are all test fixtures.
pub(crate) const CITING_EXTENSIONS: [&str; 3] = ["md", "rs", "ts"];

/// Links counted in one direction across the boundary.
#[derive(Debug, Clone, Copy, Default, Serialize, PartialEq, Eq)]
pub struct Links {
    /// Every link read in this direction.
    pub total: usize,
    /// The ones whose target is not on disk. Each is also a `lint` finding:
    /// `broken-object-link` for artifact → corpus, `broken-prose-link` for corpus → artifact.
    pub dead: usize,
}

/// How far one kind of corpus record reaches across the boundary.
#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq)]
pub struct Reach {
    /// Records of this kind the corpus holds.
    pub total: usize,
    /// Records the artifact links to at least once.
    pub cited: usize,
    /// Records that link into the artifact at least once.
    pub citing: usize,
    /// Records nothing in the artifact links to, repository-relative and sorted. This answers
    /// the second question #577 asks. It is where to look, not a list of defects: most
    /// records have no reason to be cited by code.
    pub uncited: Vec<String>,
}

/// The links between a corpus and the artifact beside it.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Coupling {
    /// The globs `[object] paths` declares, as read.
    pub object: Vec<String>,
    /// The artifact files read: tracked, claimed by a glob, one of [`CITING_EXTENSIONS`], and
    /// outside every authorship region.
    pub files: usize,
    /// Links from the artifact into `.yidam/`.
    pub inbound: Links,
    /// Links from corpus records into the artifact.
    pub outbound: Links,
    pub nodes: Reach,
    pub decisions: Reach,
    pub catalog: Reach,
}

/// Whether the coupling is read at all.
///
/// Not where no object is declared: every path is then corpus, and there is no boundary to
/// cross. Not where the held kuten declares the corpus `projected`: RFC-0028 §6 says the
/// projection *is* the coupling, so the corpus is regenerated from the artifact and a link
/// between them says nothing a person chose.
pub(crate) fn applies(root: &Path, registers: &Registers) -> bool {
    registers.declares_object()
        && crate::kuten::declared_direction(root) != crate::kuten::Direction::Projected
}

/// The artifact files, repository-relative and sorted. See the module note for which.
pub(crate) fn object_files(
    root: &Path,
    registers: &Registers,
    authorship: &Authorship,
) -> Vec<String> {
    if !registers.declares_object() {
        return Vec::new();
    }
    let mut files: Vec<String> = crate::cmd::tracked::list(root)
        .unwrap_or_default()
        .into_iter()
        .filter(|rel| !rel.starts_with(".yidam/"))
        .filter(|rel| {
            Path::new(rel)
                .extension()
                .and_then(|x| x.to_str())
                .is_some_and(|x| CITING_EXTENSIONS.contains(&x))
        })
        .filter(|rel| registers.register_of(rel) == Register::Object)
        .filter(|rel| authorship.covering(rel).is_none())
        // Tracked and deleted in the working tree: there is nothing to read.
        .filter(|rel| root.join(rel).is_file())
        .collect();
    files.sort();
    files
}

/// The links in one artifact file whose target lies under `.yidam/`.
///
/// `rel` is the file's path from `root`, which is what a finding names and what the
/// line-citation checks re-read the citing file by.
pub(crate) fn inbound(root: &Path, rel: &str, text: &str) -> Vec<ProseLink> {
    let corpus = normalize(&root.join(".yidam"));
    let path = root.join(rel);
    let dir = path.parent().unwrap_or(root);
    checks::prose_links(rel, dir, text)
        .into_iter()
        .filter(|l| normalize(&l.resolved).starts_with(&corpus))
        .collect()
}

/// The links in one corpus record whose target lies in the artifact.
pub(crate) fn outbound(
    root: &Path,
    registers: &Registers,
    record: &Path,
    text: &str,
) -> Vec<ProseLink> {
    let base = normalize(root);
    let dir = record.parent().unwrap_or(root);
    checks::prose_links("", dir, text)
        .into_iter()
        .filter(|l| {
            let target = normalize(&l.resolved);
            target.strip_prefix(&base).is_ok_and(|rel| {
                let rel = rel.to_string_lossy();
                !rel.starts_with(".yidam/") && registers.register_of(&rel) == Register::Object
            })
        })
        .collect()
}

/// The whole coupling of the repository at `root`, or `None` where it does not apply.
pub(crate) fn measure(root: &Path) -> Option<Coupling> {
    let registers = Registers::of_repo(root);
    if !applies(root, &registers) {
        return None;
    }
    let authorship = Authorship::load_or_default(root);

    let files = object_files(root, &registers, &authorship);
    let mut inbound_links = Vec::new();
    for rel in &files {
        let text = std::fs::read_to_string(root.join(rel)).unwrap_or_default();
        inbound_links.extend(inbound(root, rel, &text));
    }
    let cited: BTreeSet<PathBuf> = inbound_links
        .iter()
        .map(|l| normalize(&l.resolved))
        .collect();

    let corpus = crate::corpus::Corpus::open(root);
    let mut outbound_links = Links::default();
    let mut reach = |records: Vec<(&Path, String)>| -> Reach {
        let mut r = Reach {
            total: records.len(),
            ..Reach::default()
        };
        for (path, text) in records {
            let out = outbound(root, &registers, path, &text);
            outbound_links.total += out.len();
            outbound_links.dead += out.iter().filter(|l| !l.resolved.exists()).count();
            if !out.is_empty() {
                r.citing += 1;
            }
            if cited.contains(&normalize(path)) {
                r.cited += 1;
            } else {
                r.uncited.push(
                    path.strip_prefix(root)
                        .unwrap_or(path)
                        .to_string_lossy()
                        .to_string(),
                );
            }
        }
        r.uncited.sort();
        r
    };
    let nodes = reach(
        corpus
            .nodes()
            .iter()
            .map(|n| (n.path.as_path(), n.text.clone()))
            .collect(),
    );
    let decisions = reach(
        corpus
            .decisions()
            .iter()
            .map(|d| {
                let text = std::fs::read_to_string(&d.path).unwrap_or_default();
                (d.path.as_path(), text)
            })
            .collect(),
    );
    let catalog = reach(
        corpus
            .sources()
            .iter()
            .map(|s| (s.path.as_path(), s.text.clone()))
            .collect(),
    );

    Some(Coupling {
        object: registers.globs().to_vec(),
        files: files.len(),
        inbound: Links {
            total: inbound_links.len(),
            dead: inbound_links
                .iter()
                .filter(|l| !l.resolved.exists())
                .count(),
        },
        outbound: outbound_links,
        nodes,
        decisions,
        catalog,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::fixture::{self, write};

    /// The measured shape, cut down: a corpus with two nodes, a decision and a catalog entry,
    /// and an artifact of two crates and a web app. Staged and not committed: the tracked set
    /// is the index, which is what [`object_files`] reads.
    fn coupled() -> tempfile::TempDir {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        fixture::init(root);
        write(
            root,
            ".yidam/config.toml",
            "[object]\npaths = [\"crates/**\", \"web/**\"]\n",
        );
        write(root, ".yidam/corpus/site.ont.yml", "class: site\n");
        write(
            root,
            ".yidam/corpus/site/courthouse.yml",
            "class: site\nlabel: Courthouse\ndescription: |\n  Scored by [the scorer](../../../crates/nrhp/src/lib.rs).\n",
        );
        write(
            root,
            ".yidam/corpus/site/jail.yml",
            "class: site\nlabel: Jail\ndescription: Not cited.\n",
        );
        write(
            root,
            ".yidam/decisions/scoring.yml",
            "title: Score sites\nstatus: accepted\n",
        );
        write(
            root,
            ".yidam/catalog/nrhp.md",
            "---\nobtained: true\n---\n# NRHP\n",
        );
        write(
            root,
            "crates/nrhp/README.md",
            "Reads [the courthouse](../../.yidam/corpus/site/courthouse.yml).\n\n\
             Decided in [scoring](../../.yidam/decisions/scoring.yml).\n",
        );
        write(
            root,
            "crates/nrhp/src/lib.rs",
            "//! Source: [NRHP](../../../.yidam/catalog/nrhp.md).\npub fn score() {}\n",
        );
        write(root, "web/app.ts", "// No citation here.\nexport {};\n");
        // Claimed by no glob: corpus register, not read.
        write(
            root,
            "notes/README.md",
            "[courthouse](../.yidam/corpus/site/jail.yml)\n",
        );
        fixture::git(root, &["add", "-A"]);
        tmp
    }

    #[test]
    fn the_artifact_is_the_tracked_files_the_globs_claim() {
        let tmp = coupled();
        let root = tmp.path();
        write(
            root,
            "crates/nrhp/untracked.md",
            "[x](../../.yidam/corpus/site/jail.yml)\n",
        );
        write(root, "crates/nrhp/data.json", "{}\n");
        fixture::git(root, &["add", "crates/nrhp/data.json"]);
        let files = object_files(
            root,
            &Registers::of_repo(root),
            &Authorship::load_or_default(root),
        );
        assert_eq!(
            files,
            [
                "crates/nrhp/README.md",
                "crates/nrhp/src/lib.rs",
                "web/app.ts"
            ]
        );
    }

    #[test]
    fn a_file_under_an_authorship_region_is_not_the_artifacts_citation() {
        let tmp = coupled();
        let root = tmp.path();
        write(
            root,
            ".yidam/authorship.yml",
            "generated:\n  - path: web/\n    by: the site build\n",
        );
        let files = object_files(
            root,
            &Registers::of_repo(root),
            &Authorship::load_or_default(root),
        );
        assert!(!files.iter().any(|f| f.starts_with("web/")), "{files:?}");
        assert!(files.contains(&"crates/nrhp/README.md".to_string()));
    }

    #[test]
    fn only_links_into_the_corpus_are_inbound() {
        let tmp = tempfile::tempdir().unwrap();
        let text = "[node](../../.yidam/corpus/a/b.yml) [sibling](../other/README.md) \
                    [web](https://example.com) `[code](../../.yidam/x.yml)`\n";
        let links = inbound(tmp.path(), "crates/x/README.md", text);
        let targets: Vec<&str> = links.iter().map(|l| l.target.as_str()).collect();
        assert_eq!(targets, ["../../.yidam/corpus/a/b.yml"]);
        assert_eq!(links[0].file, "crates/x/README.md");
        assert_eq!(links[0].line, 1);
    }

    #[test]
    fn only_links_into_the_object_register_are_outbound() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let registers = Registers::of_globs(vec!["crates/**".into()]);
        let record = root.join(".yidam/corpus/site/a.yml");
        let text = "[crate](../../../crates/x/lib.rs) [node](b.yml) \
                    [notes](../../../notes/n.md)\n";
        let links = outbound(root, &registers, &record, text);
        let targets: Vec<&str> = links.iter().map(|l| l.target.as_str()).collect();
        assert_eq!(targets, ["../../../crates/x/lib.rs"]);
    }

    /// Both directions and all three kinds, from one fixture whose every count is different,
    /// so a count read from the wrong field cannot pass.
    #[test]
    fn the_coupling_counts_both_directions_per_kind() {
        let tmp = coupled();
        let c = measure(tmp.path()).expect("an object is declared");
        assert_eq!(c.object, ["crates/**", "web/**"]);
        assert_eq!(c.files, 3);
        assert_eq!(c.inbound, Links { total: 3, dead: 0 });
        assert_eq!(c.outbound, Links { total: 1, dead: 0 });
        assert_eq!(
            c.nodes,
            Reach {
                total: 2,
                cited: 1,
                citing: 1,
                uncited: vec![".yidam/corpus/site/jail.yml".into()],
            }
        );
        assert_eq!((c.decisions.total, c.decisions.cited), (1, 1));
        assert!(c.decisions.uncited.is_empty());
        assert_eq!(
            (c.catalog.total, c.catalog.cited, c.catalog.citing),
            (1, 1, 0)
        );
    }

    /// The mutation #577 was measured with: delete a node the artifact links to.
    #[test]
    fn deleting_a_cited_node_leaves_a_dead_inbound_link() {
        let tmp = coupled();
        std::fs::remove_file(tmp.path().join(".yidam/corpus/site/courthouse.yml")).unwrap();
        let c = measure(tmp.path()).unwrap();
        assert_eq!(c.inbound, Links { total: 3, dead: 1 });
        assert_eq!(c.nodes.total, 1);
    }

    #[test]
    fn no_object_declared_means_no_coupling() {
        let tmp = coupled();
        write(tmp.path(), ".yidam/config.toml", "");
        assert_eq!(measure(tmp.path()), None);
    }

    /// RFC-0028 §6: a projected corpus is regenerated from its object, so its coupling is
    /// not read. The same tree with the direction flipped is the only difference.
    #[test]
    fn a_projected_corpus_is_not_read() {
        let tmp = coupled();
        let root = tmp.path();
        assert!(measure(root).is_some());
        write(
            root,
            ".yidam/decisions/kuten.yml",
            "kuten: mirror\nrevision: 1\n",
        );
        write(
            root,
            ".yidam/.vendor/prelude/kuten/mirror/kuten.yml",
            "kuten: mirror\nrevision: 1\nobject:\n  direction: authored\n",
        );
        assert!(measure(root).is_some(), "an authored kuten still reads it");
        write(
            root,
            ".yidam/.vendor/prelude/kuten/mirror/kuten.yml",
            "kuten: mirror\nrevision: 1\nobject:\n  direction: projected\n",
        );
        assert_eq!(measure(root), None);
    }
}
