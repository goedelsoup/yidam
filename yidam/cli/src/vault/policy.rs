//! What may leave this machine, and what may not.
//!
//! # This is the first egress channel `yidam` itself opens
//!
//! `prelude/guidelines/directories.md` says so before anything could:
//!
//! > **This is access control over material at rest. It says nothing about data leaving at
//! > runtime.** […] an egress check would have to know every network call the domain computer
//! > makes, and CI is hermetic precisely so that it makes none.
//!
//! It then lists the channels a yidam repository opens — connectors, a deployed web shell, a
//! hosted encoder, anything with telemetry — and says each is the reader's responsibility.
//! `vault push` is that channel, and unlike the others it is one this binary owns, so it is
//! one this binary can gate.
//!
//! # Two independent refusals, and the order matters
//!
//! `.yidam/private-paths` is about **this repository**: material that must not sit somewhere
//! public. `redistributable` is about **the source**: bytes somebody licensed you to read and
//! not to host. Neither implies the other, and an artifact must clear both.
//!
//! The release workflow already applies the first rule to a bundle, and gives the reason this
//! inherits: *the artifact outlives the access.* A file removed from a repository stops being
//! readable; an object uploaded to a bucket does not.
//!
//! # The default is refusal, for a third party's bytes
//!
//! A catalog artifact is not pushed unless its record says `redistributable: true`. A default
//! of *upload unless told otherwise* would make the first `vault push` anybody runs a
//! redistribution nobody chose — and a catalog is full of papers, which is exactly the
//! material where that matters.
//!
//! The repository's own derived output — an index, an embedding set, a bundle — defaults the
//! other way, and arrives with the phase that stores it.

use std::path::Path;

use anyhow::{Context, Result};

use super::cas::ContentHash;

/// One artifact the working tree names, with what deciding about it requires.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Named {
    pub hash: ContentHash,
    /// What kind of artifact this is, in the vocabulary `holds` routes on.
    ///
    /// Carried rather than assumed, even though every artifact today is a `catalog` one: the
    /// routing function takes a kind, and writing it against a constant would mean #417's
    /// index and embeddings arrive by editing the router rather than by declaring a kind.
    pub kind: String,
    /// The catalog entry that names it, repo-relative.
    pub rel: String,
    /// Which vault the record routes it to, if it says. `None` means route by kind.
    pub vault: Option<String>,
    /// Whether the record licenses redistribution. `None` is not `false` — it is *nobody has
    /// said* — and both refuse a push, but only one of them is a decision.
    pub redistributable: Option<bool>,
    pub bytes: Option<u64>,
}

/// Whether an artifact may be uploaded, and if not, why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Disposition {
    /// It may go to the named vault.
    Push,
    /// It may not. The reason is written for someone reading a terminal.
    Refused(String),
}

impl Disposition {
    pub fn is_push(&self) -> bool {
        matches!(self, Disposition::Push)
    }
}

/// The declared-private paths, one per line.
///
/// `#` comments and blank lines ignored, matching the format the CI and release workflows
/// already read. Absent means nothing is declared private, which is most repositories.
///
/// This is the first reader of that file in Rust — until now it existed only in two shell
/// steps — so the format is followed rather than invented.
pub fn read_private_paths(root: &Path) -> Result<Vec<String>> {
    let path = root.join(".yidam").join("private-paths");
    if !path.exists() {
        return Ok(Vec::new());
    }
    let text =
        std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    Ok(text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(|l| l.trim_end_matches('/').to_string())
        .collect())
}

/// Whether a repo-relative path sits under a declared-private path.
///
/// A declaration names a file or a directory; `dossier` covers `dossier/a/b.md` and does not
/// cover `dossiers/x.md`. The trailing-slash normalisation happens on the way in, so both
/// `dossier` and `dossier/` behave the same — a person writing that file should not have to
/// know which one this wanted.
pub fn is_private(rel: &str, private: &[String]) -> bool {
    let rel = rel.trim_start_matches("./");
    private
        .iter()
        .any(|p| rel == p || rel.starts_with(&format!("{p}/")))
}

/// May these bytes be uploaded?
///
/// **Whether, not where.** Routing is [`super::config::Vaults::route`]'s question, and the two
/// are kept apart because they fail differently: a route is edited casually by somebody
/// reorganising storage, and a licence is not something that edit is allowed to undo. A caller
/// asks both, and an artifact needs a route *and* a permission.
///
/// Both refusals here are checked and the **private-paths one is reported first**, because it
/// is a statement about this repository that the person running the command can act on, while
/// `redistributable` is a fact about a third party's licence that they may not be able to
/// change at all.
pub fn may_push(a: &Named, private: &[String]) -> Disposition {
    if is_private(&a.rel, private) {
        return Disposition::Refused(format!(
            "{} is under a path `.yidam/private-paths` declares private. \
             The artifact outlives the access",
            a.rel
        ));
    }
    match a.redistributable {
        Some(true) => Disposition::Push,
        Some(false) => Disposition::Refused(format!(
            "{} records `redistributable: false` — licensed to read, not to host",
            a.rel
        )),
        None => Disposition::Refused(format!(
            "{} does not say whether these bytes may be redistributed. \
             Add `redistributable: true` to the record if they may",
            a.rel
        )),
    }
}

/// Every artifact the catalog names, in a stable order.
pub fn named_artifacts(root: &Path) -> Vec<Named> {
    let catalog = crate::paths::yidam_catalog_dir(root);
    let mut out = Vec::new();
    for path in crate::walk::walk_md_files(&catalog) {
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        let fm = crate::parse::parse_frontmatter(&text);
        let rel = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .to_string();
        for a in fm.artifacts.unwrap_or_default() {
            // A record whose digest does not parse names nothing, and
            // `catalog-artifact-malformed` reports it. Skipping it here rather than failing
            // keeps a push working on the rest of a corpus while one entry is being fixed.
            let Some(hash) = a.sha256.as_deref().and_then(|h| ContentHash::parse(h).ok()) else {
                continue;
            };
            out.push(Named {
                hash,
                kind: super::config::CATALOG_KIND.to_string(),
                rel: rel.clone(),
                vault: a.vault.clone(),
                redistributable: a.redistributable,
                bytes: a.bytes,
            });
        }
    }
    out.sort_by(|a, b| (&a.rel, &a.hash).cmp(&(&b.rel, &b.hash)));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn named(rel: &str, vault: Option<&str>, redistributable: Option<bool>) -> Named {
        Named {
            hash: ContentHash::of_bytes(rel.as_bytes()),
            kind: crate::vault::CATALOG_KIND.to_string(),
            rel: rel.to_string(),
            vault: vault.map(str::to_string),
            redistributable,
            bytes: None,
        }
    }

    /// **The default is refusal.** A record that says nothing about redistribution is not a
    /// licence, and the first `vault push` anybody runs must not be one.
    #[test]
    fn a_record_that_says_nothing_is_not_pushed() {
        let d = may_push(&named(".yidam/catalog/pearl-2009.md", None, None), &[]);
        assert!(!d.is_push());
        match d {
            Disposition::Refused(why) => assert!(why.contains("does not say"), "{why}"),
            _ => unreachable!(),
        }
    }

    #[test]
    fn an_explicit_licence_is_what_permits_a_push() {
        assert!(may_push(&named(".yidam/catalog/x.md", None, Some(true)), &[]).is_push());
    }

    #[test]
    fn an_explicit_refusal_is_reported_as_a_licence_and_not_as_an_omission() {
        match may_push(&named(".yidam/catalog/x.md", None, Some(false)), &[]) {
            Disposition::Refused(why) => {
                assert!(why.contains("licensed to read, not to host"), "{why}")
            }
            _ => panic!("must refuse"),
        }
    }

    /// This answers *whether*, never *where*. A record naming a vault — including the local
    /// one — is a licensed record either way, and folding routing in here would let somebody
    /// reorganising storage edit a licence by accident.
    #[test]
    fn routing_is_not_this_functions_question() {
        assert!(may_push(&named(".yidam/catalog/x.md", Some("none"), Some(true)), &[]).is_push());
        assert!(may_push(
            &named(".yidam/catalog/x.md", Some("sources"), Some(true)),
            &[]
        )
        .is_push());
        // And a route does not confer one.
        assert!(!may_push(&named(".yidam/catalog/x.md", Some("sources"), None), &[]).is_push());
    }

    /// **The load-bearing guard.** A declared-private path refuses a push even when the record
    /// licenses redistribution — the two questions are independent and an artifact clears both
    /// or neither.
    #[test]
    fn a_private_path_refuses_a_push_that_the_licence_would_have_allowed() {
        let private = vec!["dossier".to_string()];
        let a = named("dossier/evidence.md", None, Some(true));
        match may_push(&a, &private) {
            Disposition::Refused(why) => {
                assert!(why.contains("private-paths"), "{why}");
                assert!(why.contains("outlives the access"), "{why}");
            }
            _ => panic!("a private path must refuse"),
        }
    }

    #[test]
    fn privacy_matches_a_directory_prefix_and_not_a_name_prefix() {
        let private = vec!["dossier".to_string()];
        assert!(is_private("dossier", &private));
        assert!(is_private("dossier/a/b.md", &private));
        // The trap: `dossiers` is a different directory and must not be swept in.
        assert!(!is_private("dossiers/a.md", &private));
        assert!(!is_private("other/dossier.md", &private));
    }

    /// Both spellings of a directory declaration behave the same. Somebody writing that file
    /// should not have to know which one the parser wanted.
    #[test]
    fn a_trailing_slash_in_the_declaration_makes_no_difference() {
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join(".yidam")).unwrap();
        std::fs::write(
            tmp.path().join(".yidam/private-paths"),
            "# a comment\n\ndossier/\n  working-notes  \n",
        )
        .unwrap();
        let p = read_private_paths(tmp.path()).unwrap();
        assert_eq!(p, vec!["dossier".to_string(), "working-notes".to_string()]);
        assert!(is_private("dossier/x.md", &p));
        assert!(is_private("working-notes/y.md", &p));
    }

    #[test]
    fn a_repository_declaring_nothing_private_reads_as_an_empty_list() {
        let tmp = TempDir::new().unwrap();
        assert_eq!(
            read_private_paths(tmp.path()).unwrap(),
            Vec::<String>::new()
        );
    }

    #[test]
    fn named_artifacts_reads_records_and_skips_ones_that_name_nothing() {
        let tmp = TempDir::new().unwrap();
        let catalog = tmp.path().join(".yidam/catalog");
        std::fs::create_dir_all(&catalog).unwrap();
        let good = ContentHash::of_bytes(b"a");
        std::fs::write(
            catalog.join("one.md"),
            format!(
                "---\nname: One\nartifacts:\n  - sha256: {good}\n    redistributable: true\n  \
                 - sha256: not-a-digest\n---\n"
            ),
        )
        .unwrap();
        std::fs::write(catalog.join("two.md"), "---\nname: Two\n---\n").unwrap();

        let found = named_artifacts(tmp.path());
        assert_eq!(
            found.len(),
            1,
            "the malformed record names nothing: {found:?}"
        );
        assert_eq!(found[0].hash, good);
        assert_eq!(found[0].redistributable, Some(true));
        assert!(found[0].rel.ends_with("one.md"));
    }
}
