//! Paths in, records out — the one way a corpus's bytes become [`Node`], [`Class`] and
//! [`Source`].
//!
//! Every loader reads through an [`Overlay`] rather than straight from disk, so `serve --lsp`
//! can answer about the buffer somebody is typing into without any caller knowing that is
//! what it is doing. For every other caller the overlay is empty and `read` is a plain
//! `read_to_string`.

use std::path::{Path, PathBuf};

use crate::parse::parse_frontmatter_reporting;
use crate::walk::walk_ont_files;

use super::{Class, Node, Overlay, Source};

fn rel_of(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .to_string()
}

pub fn load_nodes(root: &Path, paths: &[PathBuf], overlay: &Overlay) -> Vec<Node> {
    paths
        .iter()
        .map(|p| Node::parse(p.clone(), rel_of(root, p), overlay.read(p)))
        .collect()
}

pub fn load_sources(root: &Path, paths: &[PathBuf], overlay: &Overlay) -> Vec<Source> {
    paths
        .iter()
        .map(|p| {
            let text = overlay.read(p);
            let (fm, malformed) = parse_frontmatter_reporting(&text);
            Source {
                rel: rel_of(root, p),
                path: p.clone(),
                // Absent means obtained. Only an explicit `false` claims otherwise.
                obtained: fm.obtained.unwrap_or(true),
                // Carried as an `Option`. `unwrap_or_default()` here made `used-by: []`
                // indistinguishable from an absent key by the time the check saw it.
                used_by: fm.used_by,
                locations: fm.location.unwrap_or_default(),
                retrieved: fm.retrieved,
                ttl_days: fm.ttl_days,
                artifacts: fm.artifacts.unwrap_or_default(),
                text,
                malformed,
            }
        })
        .collect()
}
pub fn load_classes(root: &Path, paths: &[PathBuf], overlay: &Overlay) -> Vec<Class> {
    paths
        .iter()
        .map(|p| Class::parse(rel_of(root, p), overlay.read(p)))
        .collect()
}

/// Every `<class>.ont.yml` directly under `corpus_dir`, parsed once.
///
/// [`super::Corpus`] is the reader for a *repository*, and four class readers could not use
/// it: `claims`, `prose` and `retrievable` are handed a corpus directory rather than a root,
/// and one of `claims`'s callers is pointed at an unpacked **dependency's** corpus, which is
/// not inside a repository at all. Each of them therefore walked `.ont.yml` and wrote its own
/// `serde_yaml::from_str` — four more answers to what a class file says (#1116).
///
/// This is the same [`Class::parse`] the corpus model uses, reached without a root. What it
/// does not have is the [`Overlay`]: there is no editor buffer behind a path-only read, and a
/// caller that wants one has a [`super::Corpus`] and should ask it. [`Class::rel`] is
/// therefore relative to the corpus directory rather than the repository, which no reader
/// here prints — they read [`Class::name`] and the declarations under it.
pub fn read_classes(corpus_dir: &Path) -> Vec<Class> {
    load_classes(corpus_dir, &walk_ont_files(corpus_dir), &Overlay::default())
}
