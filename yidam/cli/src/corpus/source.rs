//! A catalog entry: one `catalog/*.md` file's frontmatter, parsed once.

use std::path::PathBuf;

/// A catalog entry parsed once.
pub struct Source {
    pub rel: String,
    /// Absolute path on disk. A citation is a link that resolves to *this file*, which is
    /// what replaced the slug this struct used to carry — the slug existed only to be
    /// searched for in a node's bytes, and nothing else ever needed it.
    pub path: PathBuf,
    pub obtained: bool,
    /// The declared list, or `None` where the key is absent. Not flattened to a `Vec`: an
    /// absent key and `used-by: []` are different claims and [`crate::cmd::lint::checks::used_by_drift`] turns on the
    /// difference.
    pub used_by: Option<Vec<String>>,
    pub locations: Vec<crate::parse::CatalogLocation>,
    /// When the entry says it was last fetched, verbatim. See [`crate::cmd::lint::ttl`].
    pub retrieved: Option<String>,
    /// The entry's own TTL, which beats the corpus default.
    pub ttl_days: Option<u32>,
    /// What this entry says it has obtained, by content address.
    ///
    /// Empty on every entry written before RFC-0023, and the checks that read it are written
    /// so that an empty list is silent. A corpus adopting the field opts into the checks; a
    /// corpus that has not adopted it sees no new findings at all.
    pub artifacts: Vec<crate::parse::CatalogArtifact>,
}
