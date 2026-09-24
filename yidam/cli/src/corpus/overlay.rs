//! Reading a corpus file through whatever is open in an editor.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Unsaved editor buffers, keyed by absolute path.
///
/// Every check in `lint` reads the working tree, which is exactly right for a gate and
/// exactly wrong for an editor: the file you are typing into is the one whose findings you
/// want, and it is the one on disk that is stale. An overlay lets `serve --lsp` answer about
/// the buffer without any check knowing that is what it is doing.
///
/// Empty for every other caller, and `Overlay::read` is then a plain `read_to_string`.
#[derive(Debug, Default, Clone)]
pub struct Overlay(HashMap<PathBuf, String>);

impl Overlay {
    pub fn set(&mut self, path: PathBuf, text: String) {
        self.0.insert(path, text);
    }

    pub fn clear(&mut self, path: &Path) {
        self.0.remove(path);
    }

    /// The buffer if one is open, otherwise the file.
    pub fn read(&self, path: &Path) -> String {
        match self.0.get(path) {
            Some(text) => text.clone(),
            None => std::fs::read_to_string(path).unwrap_or_default(),
        }
    }

    /// Instance buffers the walker cannot see: open under `corpus`, and not yet on disk.
    ///
    /// Every path the checks read comes from a directory walk, so a buffer for a file that
    /// has not been saved once was read by nobody — an editor's `:e concept/new.yml` got no
    /// verdict until the first `:w`, and the web editor's node form (#607) is a buffer that
    /// by design is *never* written, so it got none at all. Same shape as `read`: the walk
    /// answers for what is on disk, and the overlay answers for what is not, with the same
    /// predicate `walk_corpus_instances` applies — under the corpus, at least a class
    /// directory deep, `.yml`, and not a class file.
    ///
    /// A buffer whose file *does* exist is the walk's already and is not repeated here.
    pub fn unsaved_instances(&self, corpus: &Path) -> Vec<PathBuf> {
        let mut found: Vec<PathBuf> = self
            .0
            .keys()
            .filter(|p| !p.exists())
            .filter(|p| {
                let Ok(rel) = p.strip_prefix(corpus) else {
                    return false;
                };
                let name = rel.file_name().map(|n| n.to_string_lossy());
                rel.components().count() >= 2
                    && p.extension().is_some_and(|x| x == "yml")
                    && name.is_some_and(|n| !n.ends_with(".ont.yml"))
            })
            .cloned()
            .collect();
        found.sort();
        found
    }
}
