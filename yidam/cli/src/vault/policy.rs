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
use super::derived::Derived;

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
    /// What the record says these bytes are. Only `vault materialize` reads it — a content
    /// address is right for storage and useless for opening, and this is what supplies the
    /// extension a person's PDF reader needs.
    pub media_type: Option<String>,
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

/// The directories a derived artifact is computed from.
///
/// **This is the list that decides whether pushing it leaks.** An index is not a file that
/// happens to sit in `.yidam/index/`; it is a re-encoding of everything walked to build it,
/// and `model::VectorRow` carries the node's `text` verbatim. So the question "may this
/// artifact leave" is really "may everything it was derived from leave".
///
/// `sadhana/github/workflows/release.yml` asks the same question of a bundle and names
/// `.yidam/corpus`, `.yidam/skills`, `.yidam/decisions`. This adds **`.yidam/catalog`** for the
/// two embedding-derived kinds, because `cmd/embed.rs` composes text from catalog entries as
/// well as corpus nodes — an entry's own prose ends up in the index, and the workflow's list
/// does not cover it.
pub fn derived_sources(d: Derived) -> &'static [&'static str] {
    match d {
        // `cmd/embed.rs` walks the corpus and the catalog; `index-build` consumes the result.
        Derived::Index | Derived::Embeddings => &[".yidam/corpus", ".yidam/catalog"],
        // `cmd/bundle.rs` packs corpus classes, skills, decisions — and `index/corpus.arrow`,
        // which is why the first two entries are here twice over.
        Derived::Bundle => &[
            ".yidam/corpus",
            ".yidam/catalog",
            ".yidam/skills",
            ".yidam/decisions",
        ],
    }
}

/// May this computed artifact be uploaded?
///
/// A catalog artifact is refused for what its own record says. A derived artifact has no
/// record — nobody wrote one, because nobody fetched it — so the question is answered from
/// what it was built out of. A declared-private path that **intersects** the material this
/// artifact encodes means the artifact carries private text, and no vault should receive it.
///
/// Intersection is tested both ways, as the release workflow tests it: a private path inside
/// a source directory, and a private path that *contains* one. `dossier/` beside the corpus
/// is the first; a repository declaring `.yidam/` private is the second.
///
/// **Only a path that actually holds something counts.** A declared directory holding nothing
/// but a `README.md` or a `.gitkeep` is a placeholder, and refusing on it would make the
/// feature unusable for a repository that declared its intent before having anything to
/// protect — which is the order this file asks people to work in.
pub fn derived_may_push(root: &Path, d: Derived, private: &[String]) -> Disposition {
    for p in private {
        if !holds_content(&root.join(p)) {
            continue;
        }
        for src in derived_sources(d) {
            let intersects =
                p == src || p.starts_with(&format!("{src}/")) || src.starts_with(&format!("{p}/"));
            if intersects {
                return Disposition::Refused(format!(
                    "`{p}` is declared private and is part of what the {} is built from \
                     ({src}). The {} carries the text of every node it encodes, so pushing \
                     it would publish that material — and the artifact outlives the access",
                    d.kind(),
                    d.kind()
                ));
            }
        }
    }
    Disposition::Push
}

/// Whether a declared path holds anything but placeholders.
///
/// `.gitkeep` and `README.md` are exactly what the release workflow excludes, and for the
/// same reason: they are how an empty declared directory is kept in git at all.
fn holds_content(path: &Path) -> bool {
    if path.is_file() {
        return true;
    }
    if !path.is_dir() {
        return false;
    }
    let mut stack = vec![path.to_path_buf()];
    while let Some(at) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&at) else {
            continue;
        };
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
                continue;
            }
            let name = p
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            if name != ".gitkeep" && name != "README.md" {
                return true;
            }
        }
    }
    false
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
                media_type: a.media_type.clone(),
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
            media_type: None,
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

    fn corpus_with(root: &Path, rel: &str, body: &[u8]) {
        let p = root.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, body).unwrap();
    }

    /// **The leak this phase would otherwise open.** An index re-encodes every node it walked,
    /// text and all, so a private corpus directory means a private index — and the catalog
    /// record guard cannot see it, because the index has no record.
    #[test]
    fn an_index_built_over_a_private_corpus_directory_is_not_pushed() {
        let tmp = TempDir::new().unwrap();
        corpus_with(tmp.path(), ".yidam/corpus/dossier/target.md", b"# who");
        let private = vec![".yidam/corpus/dossier".to_string()];
        match derived_may_push(tmp.path(), Derived::Index, &private) {
            Disposition::Refused(why) => {
                assert!(why.contains("dossier"), "{why}");
                assert!(why.contains("outlives the access"), "{why}");
            }
            _ => panic!("an index over private nodes must not be pushed"),
        }
    }

    /// `embed.rs` composes text from catalog entries too, and the release workflow's list does
    /// not name `.yidam/catalog`. This is where that gap is closed.
    #[test]
    fn a_private_catalog_directory_also_stops_an_index() {
        let tmp = TempDir::new().unwrap();
        corpus_with(
            tmp.path(),
            ".yidam/catalog/embargoed/paper.md",
            b"---\nname: x\n---\n",
        );
        let private = vec![".yidam/catalog/embargoed".to_string()];
        assert!(!derived_may_push(tmp.path(), Derived::Index, &private).is_push());
    }

    /// The other direction: a declaration that *contains* a source directory publishes
    /// everything under it. The release workflow tests both ways and so does this.
    #[test]
    fn a_private_path_containing_the_corpus_stops_a_push_too() {
        let tmp = TempDir::new().unwrap();
        corpus_with(tmp.path(), ".yidam/corpus/thing.md", b"x");
        let private = vec![".yidam".to_string()];
        assert!(!derived_may_push(tmp.path(), Derived::Index, &private).is_push());
    }

    /// A declared directory holding only placeholders is intent, not material. Refusing on it
    /// would punish the order this repository asks people to work in — declare first.
    #[test]
    fn a_declared_path_holding_only_placeholders_does_not_stop_a_push() {
        let tmp = TempDir::new().unwrap();
        corpus_with(
            tmp.path(),
            ".yidam/corpus/dossier/README.md",
            b"what goes here",
        );
        corpus_with(tmp.path(), ".yidam/corpus/dossier/.gitkeep", b"");
        let private = vec![".yidam/corpus/dossier".to_string()];
        assert!(derived_may_push(tmp.path(), Derived::Index, &private).is_push());
    }

    /// A private path nowhere near the material is not this artifact's problem.
    #[test]
    fn a_private_path_outside_what_the_artifact_encodes_is_irrelevant() {
        let tmp = TempDir::new().unwrap();
        corpus_with(tmp.path(), "dossier/notes.md", b"private working notes");
        let private = vec!["dossier".to_string()];
        assert!(derived_may_push(tmp.path(), Derived::Index, &private).is_push());
        // …but it is the bundle's problem only if the bundle carries it, which it does not.
        assert!(derived_may_push(tmp.path(), Derived::Bundle, &private).is_push());
    }

    /// A repository declaring nothing private pushes its own output freely. This is the
    /// common case, and the default for derived material is the opposite of the catalog's.
    #[test]
    fn a_repository_with_nothing_declared_private_pushes_its_own_output() {
        let tmp = TempDir::new().unwrap();
        for d in Derived::ALL {
            assert!(derived_may_push(tmp.path(), d, &[]).is_push(), "{d:?}");
        }
    }

    /// A bundle carries skills and decisions as well; the index does not.
    #[test]
    fn the_sources_of_each_kind_are_what_builds_it_reads() {
        assert!(derived_sources(Derived::Bundle).contains(&".yidam/skills"));
        assert!(!derived_sources(Derived::Index).contains(&".yidam/skills"));
        for d in Derived::ALL {
            assert!(
                derived_sources(d).contains(&".yidam/corpus"),
                "{d:?} encodes the corpus"
            );
        }
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
