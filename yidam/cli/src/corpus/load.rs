//! Paths in, records out — the one way a corpus's bytes become [`Node`], [`Class`] and
//! [`Source`].
//!
//! Every loader reads through an [`Overlay`] rather than straight from disk, so `serve --lsp`
//! can answer about the buffer somebody is typing into without any caller knowing that is
//! what it is doing. For every other caller the overlay is empty and `read` is a plain
//! `read_to_string`.

use std::path::{Path, PathBuf};

use crate::parse::parse_frontmatter;

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
            let fm = parse_frontmatter(&text);
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
