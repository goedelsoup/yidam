//! A corpus read once, and handed to whatever asks.
//!
//! Before #925 there was no value meaning *this repository's corpus*. There was a walk —
//! [`crate::walk::walk_corpus_instances`], called from twenty-five sites across eighteen
//! modules — and each caller did what it needed with the paths it came back with. Five of
//! those sites are in `cmd/corpus.rs` alone, each one opening every instance file, calling
//! `parse_instance`, and reaching for `inst.class` and `inst.label`: [`Node`] written out
//! by hand, five times, in one module. `status` reads every file twice in one invocation,
//! once to count open questions and once to count claims.
//!
//! **This is not a performance fix, and it should not be sold as one.** Measured on the
//! largest corpus on hand — 695 nodes, 2.9 MB — twenty-five interleaved runs of the binary
//! either side of this change: `lint` 786 ms before and 790 ms after, `graph` 73 ms and
//! 78 ms, `status` 444 ms and 404 ms. The only real movement is `status`, which read the
//! tree twice and now reads it once, and `graph` pays about 5 ms for a [`Node`] where it
//! used to keep a lighter parse of its own. Collapsing four derivations of the edge graph
//! into one did not move `lint` at all: a lint run is not waiting on the corpus.
//!
//! What it fixes is that a corpus had no single answer about itself. `normalize` existed
//! three times, link resolution eight (see [`super::edges`]), and `graph` and the licensing
//! checks resolve an edge by different rules — the filesystem's and the corpus's — because
//! nothing held the corpus in a form either could have consulted.
//!
//! **Every part is lazy.** A [`Corpus`] is free to construct, so a command may open one and
//! still cost nothing for the parts it does not read: `migrate` wants paths, `graph` wants
//! nodes and its own reading of the class files, `lint` wants everything. Eager loading
//! would have made this type adoptable only at the sites that already read the whole tree,
//! which is the minority, and would have `graph` opening the catalog for the first time.
//!
//! **It is a read, not a handle.** Nothing here re-walks or invalidates, so a command that
//! writes to the corpus — `migrate`, `rename`, `propose` — must not hold one across the
//! write. Those commands take paths from the walk directly, and that is why.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use crate::paths::{yidam_catalog_dir, yidam_corpus_dir};
use crate::walk::{walk_corpus_instances, walk_md_files, walk_ont_files};

use super::{load_classes, load_nodes, load_sources, Class, Edges, Node, Overlay, Source};

/// One repository's corpus: its nodes, its classes, its sources, and the graph between them.
pub struct Corpus {
    root: PathBuf,
    dir: PathBuf,
    catalog_dir: PathBuf,
    overlay: Overlay,
    instance_paths: OnceLock<Vec<PathBuf>>,
    ont_paths: OnceLock<Vec<PathBuf>>,
    catalog_paths: OnceLock<Vec<PathBuf>>,
    nodes: OnceLock<Vec<Node>>,
    classes: OnceLock<Vec<Class>>,
    sources: OnceLock<Vec<Source>>,
    edges: OnceLock<Edges>,
}

impl Corpus {
    /// The corpus of the repository at `root`, read from disk.
    pub fn open(root: &Path) -> Self {
        Self::open_with(root, Overlay::default())
    }

    /// The same, read through `overlay` — the editor's buffers where it has them.
    ///
    /// The overlay is held rather than passed per call, so *every* consumer of this corpus
    /// sees the same tree. Before this, `lint` was the only caller that added
    /// [`Overlay::unsaved_instances`] to its walk; a second surface reading the corpus in an
    /// editor would have had to remember to, and a surface that forgot would answer about
    /// files while the editor asked about buffers.
    pub fn open_with(root: &Path, overlay: Overlay) -> Self {
        Self {
            dir: yidam_corpus_dir(root),
            catalog_dir: yidam_catalog_dir(root),
            root: root.to_path_buf(),
            overlay,
            instance_paths: OnceLock::new(),
            ont_paths: OnceLock::new(),
            catalog_paths: OnceLock::new(),
            nodes: OnceLock::new(),
            classes: OnceLock::new(),
            sources: OnceLock::new(),
            edges: OnceLock::new(),
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// `.yidam/corpus`, the directory instance paths are relative to.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn catalog_dir(&self) -> &Path {
        &self.catalog_dir
    }

    /// Every instance file, sorted, plus the unsaved buffers that are not files yet.
    pub fn instance_paths(&self) -> &[PathBuf] {
        self.instance_paths.get_or_init(|| {
            let mut paths = walk_corpus_instances(&self.dir);
            // Empty for every caller but the language server — see [`Self::open_with`].
            paths.extend(self.overlay.unsaved_instances(&self.dir));
            paths
        })
    }

    /// Every `<class>.ont.yml` directly under the corpus directory.
    pub fn ont_paths(&self) -> &[PathBuf] {
        self.ont_paths.get_or_init(|| walk_ont_files(&self.dir))
    }

    /// Every catalog entry, which is where a [`Source`] is written.
    pub fn catalog_paths(&self) -> &[PathBuf] {
        self.catalog_paths
            .get_or_init(|| walk_md_files(&self.catalog_dir))
    }

    pub fn nodes(&self) -> &[Node] {
        self.nodes
            .get_or_init(|| load_nodes(&self.root, self.instance_paths(), &self.overlay))
    }

    pub fn classes(&self) -> &[Class] {
        self.classes
            .get_or_init(|| load_classes(&self.root, self.ont_paths(), &self.overlay))
    }

    pub fn sources(&self) -> &[Source] {
        self.sources
            .get_or_init(|| load_sources(&self.root, self.catalog_paths(), &self.overlay))
    }

    /// The graph over [`Self::nodes`], resolved once.
    pub fn edges(&self) -> &Edges {
        self.edges.get_or_init(|| Edges::build(self.nodes()))
    }

    /// This corpus's records, moved out: its nodes, its classes, and the graph between them.
    ///
    /// For the consumer that must *hold* a corpus rather than read one.
    /// [`crate::cmd::query::Graph`] is that consumer — `serve --mcp` answers many queries
    /// against a single load, and it holds a dependency's corpus in the same shape, read
    /// from a directory that is not a repository root and so is not something this type can
    /// open. None of the records is `Clone`, so lending them would make `Graph` borrow from
    /// a field of itself.
    pub fn into_parts(self) -> (Vec<Node>, Vec<Class>, Edges) {
        // Forces each part before the cells are consumed; `edges` pulls in `nodes`.
        let _ = self.classes();
        let _ = self.edges();
        (
            self.nodes.into_inner().unwrap_or_default(),
            self.classes.into_inner().unwrap_or_default(),
            self.edges.into_inner().unwrap_or_default(),
        )
    }

    /// The class names this ontology defines, taken from the `.ont.yml` filenames.
    ///
    /// The *filename* and not the `class:` field inside, because the filename is what an
    /// instance's directory is matched against — the same rule `graph-check` applies, and
    /// the one `graph.rs` documents when it falls back to the stem.
    pub fn defined_classes(&self) -> impl Iterator<Item = &str> {
        self.ont_paths().iter().filter_map(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .and_then(|n| n.strip_suffix(".ont.yml"))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(root: &Path, rel: &str, text: &str) {
        let p = root.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, text).unwrap();
    }

    fn fixture() -> tempfile::TempDir {
        let t = tempfile::tempdir().unwrap();
        let root = t.path();
        write(root, ".yidam/corpus/concept.ont.yml", "class: concept\n");
        write(root, ".yidam/corpus/gauge.ont.yml", "class: gauge\n");
        write(
            root,
            ".yidam/corpus/concept/a.yml",
            "class: concept\nlabel: A\nlinks:\n  - target: ../gauge/g.yml\n    relationship: measured-by\n",
        );
        write(
            root,
            ".yidam/corpus/gauge/g.yml",
            "class: gauge\nlabel: G\n",
        );
        write(root, ".yidam/catalog/s.md", "---\nobtained: true\n---\n");
        t
    }

    #[test]
    fn one_corpus_answers_for_nodes_classes_sources_and_edges() {
        let t = fixture();
        let c = Corpus::open(t.path());
        assert_eq!(c.nodes().len(), 2);
        assert_eq!(c.classes().len(), 2);
        assert_eq!(c.sources().len(), 1);
        // `concept/a.yml` sorts before `gauge/g.yml`, and the walk sorts.
        assert_eq!(c.edges().out(0).len(), 1);
        assert_eq!(c.edges().out(0)[0].to, Some(1));
        assert_eq!(c.edges().incoming(1), &[0]);
    }

    #[test]
    fn a_node_is_found_by_the_path_a_link_resolves_to() {
        let t = fixture();
        let c = Corpus::open(t.path());
        let target = c.dir().join("concept").join("..").join("gauge/g.yml");
        assert!(c.edges().holds(&target));
        assert!(!c.edges().holds(&c.dir().join("gauge/absent.yml")));
    }

    #[test]
    fn defined_classes_come_from_the_filenames() {
        let t = fixture();
        let c = Corpus::open(t.path());
        let mut names: Vec<&str> = c.defined_classes().collect();
        names.sort_unstable();
        assert_eq!(names, vec!["concept", "gauge"]);
    }

    /// The property that makes this type safe to open anywhere: opening reads nothing.
    #[test]
    fn a_corpus_that_is_never_asked_reads_nothing() {
        let t = tempfile::tempdir().unwrap();
        // No `.yidam/` at all. Every walk would come back empty, but none of them runs.
        let c = Corpus::open(t.path());
        assert_eq!(c.dir(), t.path().join(".yidam").join("corpus"));
        assert!(c.instance_paths.get().is_none());
        assert!(c.ont_paths.get().is_none());
        assert!(c.catalog_paths.get().is_none());
        assert!(c.nodes.get().is_none());
    }

    /// Asking for edges loads nodes and nothing else — the laziness the module note claims,
    /// asserted rather than described.
    #[test]
    fn edges_pull_in_nodes_and_leave_the_catalog_alone() {
        let t = fixture();
        let c = Corpus::open(t.path());
        let _ = c.edges();
        assert!(c.nodes.get().is_some());
        assert!(c.classes.get().is_none());
        assert!(c.sources.get().is_none());
        assert!(c.catalog_paths.get().is_none());
    }

    #[test]
    fn an_unsaved_buffer_is_a_node_of_the_corpus() {
        let t = fixture();
        let mut overlay = Overlay::default();
        let unsaved = yidam_corpus_dir(t.path()).join("concept").join("new.yml");
        overlay.set(unsaved.clone(), "class: concept\nlabel: New\n".into());
        let c = Corpus::open_with(t.path(), overlay);
        assert_eq!(c.nodes().len(), 3);
        assert!(c.edges().holds(&unsaved));
        assert!(c
            .nodes()
            .iter()
            .any(|n| n.inst.label.as_deref() == Some("New")));
    }

    #[test]
    fn a_buffer_answers_for_a_file_that_exists() {
        let t = fixture();
        let mut overlay = Overlay::default();
        let a = yidam_corpus_dir(t.path()).join("concept").join("a.yml");
        overlay.set(a.clone(), "class: concept\nlabel: Edited\n".into());
        let c = Corpus::open_with(t.path(), overlay);
        assert_eq!(c.nodes().len(), 2);
        // The buffer stands in for the file: `A` is what is on disk, and nothing reads it.
        assert!(c
            .nodes()
            .iter()
            .any(|n| n.inst.label.as_deref() == Some("Edited")));
        assert!(c
            .nodes()
            .iter()
            .all(|n| n.inst.label.as_deref() != Some("A")));
        assert!(c.edges().holds(&a));
        // And the edge it had on disk is gone, because the buffer does not write one.
        assert!(c.edges().out(0).is_empty());
    }

    #[test]
    fn an_empty_repository_answers_empty_rather_than_failing() {
        let t = tempfile::tempdir().unwrap();
        let c = Corpus::open(t.path());
        assert!(c.nodes().is_empty());
        assert!(c.classes().is_empty());
        assert!(c.sources().is_empty());
        assert!(c.edges().out(0).is_empty());
    }
}
