//! `prose-commit-unpinned` (RFC-0051 §3): prose under `.yidam/` that says "at <hex>" for a
//! commit this repository does not have.
//!
//! **The migration's lever, not a rule about prose.** A commit in this repository is
//! unambiguous and left alone. The check fires on the case the reference data showed to be
//! unreadable: a commit that belongs to someone else, named in a sentence that does not say
//! whose. The finding names the `path` dependency the commit does resolve in, so the author
//! can move the read into a pinned catalog location with `repo:`.
//!
//! **`.yidam/` only** (Decision 3). A derived repository's `docs/` is the project's prose, and
//! `lint`'s only opinion on it is the line citations. A dependency's materialized corpus under
//! `.yidam/tonpa/` is that dependency's prose, naming its own commits, and is not read either.
//!
//! **One process per repository.** Every commit named anywhere is looked up here in one
//! `git cat-file --batch-command`, and the ones missing here in one more per `path`
//! dependency, through [`super::pins::resolve_commits`].

use std::collections::BTreeMap;
use std::path::Path;

use super::model::{Check, Severity, Violation};
use super::pins::{resolve_commits, Lookup};
use crate::corpus::{Node, Source};

pub(crate) const ID: &str = "prose-commit-unpinned";

/// The check over every node and catalog entry.
pub(crate) fn check(root: &Path, nodes: &[Node], sources: &[Source]) -> Check {
    let texts = nodes
        .iter()
        .map(|n| (n.rel.as_str(), n.text.as_str()))
        .chain(sources.iter().map(|s| (s.rel.as_str(), s.text.as_str())))
        .filter(|(rel, _)| read_here(rel));
    let mut sightings: Vec<(&str, String)> = Vec::new();
    for (rel, text) in texts {
        for hex in commits_named(text) {
            if !sightings.iter().any(|(r, h)| *r == rel && *h == hex) {
                sightings.push((rel, hex));
            }
        }
    }
    let violations = foreign(root, &sightings)
        .map(|owners| {
            sightings
                .iter()
                .filter_map(|(rel, hex)| {
                    let owned = owners.get(hex)?;
                    Some(Violation::new(*rel, detail(hex, owned)))
                })
                .collect()
        })
        .unwrap_or_default();
    Check::new(
        ID,
        "Prose names a commit that is not in this repository",
        Severity::Warn,
        "\"At <commit>\" in a node or catalog entry dates a read, and a reader can follow it \
         only in the repository that has the commit. A commit in this repository is \
         unambiguous and left alone. One that is not, usually a sibling's, reads as a dead \
         reference here; the finding names the `path` dependency it resolves in, so the read \
         can move into a catalog location pinned with `commit` and `repo`. Only `.yidam/` is \
         read: `docs/` is the project's own prose.",
        violations,
    )
}

/// Whether prose at `rel` is this corpus's: under `.yidam/`, and not a dependency's
/// materialized corpus.
fn read_here(rel: &str) -> bool {
    rel.starts_with(".yidam/") && !rel.starts_with(".yidam/tonpa/")
}

/// The finding's detail for `hex`, given the dependencies it resolves in.
fn detail(hex: &str, owners: &[String]) -> String {
    if owners.is_empty() {
        return format!(
            "`at {hex}` names a commit that is in neither this repository nor any `path` \
             dependency in `.yidam/tonpa.toml`"
        );
    }
    let names: Vec<String> = owners.iter().map(|o| format!("`{o}`")).collect();
    format!(
        "`at {hex}` names a commit in {}, not in this repository; pin the read as a catalog \
         location with `commit: {hex}` and `repo: {}`",
        names.join(", "),
        owners[0]
    )
}

/// Every commit named in `sightings` that this repository does not have, with the `path`
/// dependencies it resolves in.
///
/// `None` when git cannot run here, which reports nothing rather than every commit foreign.
/// In a shallow clone a commit found nowhere has not shown it is missing, so only the ones a
/// dependency has are kept.
fn foreign(root: &Path, sightings: &[(&str, String)]) -> Option<BTreeMap<String, Vec<String>>> {
    let mut hexes: Vec<String> = sightings.iter().map(|(_, h)| h.clone()).collect();
    hexes.sort();
    hexes.dedup();
    let here = resolve_commits(root, &hexes)?;
    let missing: Vec<String> = hexes
        .into_iter()
        .zip(here)
        .filter(|(_, l)| *l == Lookup::Missing)
        .map(|(h, _)| h)
        .collect();
    let mut owners: BTreeMap<String, Vec<String>> =
        missing.iter().map(|h| (h.clone(), Vec::new())).collect();
    if missing.is_empty() {
        return Some(owners);
    }
    let config = crate::deps::load_config(&crate::paths::tonpa_config_path(root));
    for (name, dep) in &config.dependencies {
        let Some(path) = &dep.path else { continue };
        let Some(there) = resolve_commits(&root.join(path), &missing) else {
            continue;
        };
        for (hex, lookup) in missing.iter().zip(there) {
            if lookup != Lookup::Missing {
                owners.entry(hex.clone()).or_default().push(name.clone());
            }
        }
    }
    if crate::git::is_shallow(root) {
        owners.retain(|_, o| !o.is_empty());
    }
    Some(owners)
}

/// Every commit `text` names as "at <hex>": 7 to 40 hex digits, at least one of them a digit
/// so an English word is not read as one, optionally in backticks, lowercased.
fn commits_named(text: &str) -> Vec<String> {
    let bytes = text.as_bytes();
    let word = |b: u8| b.is_ascii_alphanumeric() || b == b'_';
    let mut found = Vec::new();
    let mut i = 0;
    while i + 2 <= bytes.len() {
        let at = matches!(&bytes[i..i + 2], b"at" | b"At");
        if !at || (i > 0 && word(bytes[i - 1])) {
            i += 1;
            continue;
        }
        let mut j = i + 2;
        let gap = j;
        while j < bytes.len() && bytes[j].is_ascii_whitespace() {
            j += 1;
        }
        if j == gap {
            i += 1;
            continue;
        }
        if bytes.get(j) == Some(&b'`') {
            j += 1;
        }
        let start = j;
        while j < bytes.len() && bytes[j].is_ascii_hexdigit() {
            j += 1;
        }
        let hex = &text[start..j];
        let bounded = bytes.get(j).is_none_or(|b| !word(*b));
        if bounded && (7..=40).contains(&hex.len()) && hex.bytes().any(|b| b.is_ascii_digit()) {
            found.push(hex.to_ascii_lowercase());
        }
        i = j.max(i + 1);
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::fixture::{commit, git_out, init, write};
    use std::path::PathBuf;

    /// A repository with one commit whose content is `seed`, and that commit's full id.
    fn repo(dir: &Path, seed: &str) -> String {
        std::fs::create_dir_all(dir).unwrap();
        init(dir);
        write(dir, "seed.txt", seed);
        commit(dir, seed);
        git_out(dir, &["rev-parse", "HEAD"]).trim().to_string()
    }

    fn node(rel: &str, text: &str) -> Node {
        Node::parse(PathBuf::from(rel), rel, text.to_string())
    }

    /// `here` with a `path` dependency `sib` beside it, and each one's commit.
    fn pair(tmp: &Path) -> (PathBuf, String, String) {
        let here = tmp.join("here");
        let ours = repo(&here, "ours");
        let theirs = repo(&tmp.join("sib"), "theirs");
        write(
            &here,
            ".yidam/tonpa.toml",
            "[dependencies.sib]\npath = \"../sib\"\n",
        );
        (here, ours, theirs)
    }

    #[test]
    fn reads_at_hex_and_nothing_else() {
        assert_eq!(
            commits_named("read at 1a2b3c4, and At `ABCDEF0123` then."),
            ["1a2b3c4", "abcdef0123"]
        );
        let none = [
            "at defaced",                                   // no digit: a word
            "at 1a2b3c",                                    // six digits
            "at 1a2b3c4z",                                  // not bounded
            "that 1a2b3c4",                                 // `at` inside a word
            "at1a2b3c4",                                    // no gap
            "at 1a2b3c4d5e6f1a2b3c4d5e6f1a2b3c4d5e6f1a2b3", // 41 digits
        ];
        for text in none {
            assert!(commits_named(text).is_empty(), "{text}");
        }
    }

    #[test]
    fn a_sibling_commit_is_reported_and_names_the_sibling() {
        let tmp = tempfile::tempdir().unwrap();
        let (here, ours, theirs) = pair(tmp.path());
        let short = &theirs[..10];
        let nodes = [node(
            ".yidam/nodes/n.md",
            &format!("Read at {short}, and at {}.\n", &ours[..7]),
        )];
        let c = check(&here, &nodes, &[]);
        assert_eq!(c.violations.len(), 1, "{:?}", c.violations);
        let v = &c.violations[0];
        assert_eq!(v.node, ".yidam/nodes/n.md");
        assert!(
            v.detail.contains("`sib`") && v.detail.contains(short),
            "{}",
            v.detail
        );
    }

    #[test]
    fn a_commit_found_nowhere_is_reported_without_an_owner() {
        let tmp = tempfile::tempdir().unwrap();
        let (here, _, _) = pair(tmp.path());
        let nodes = [node(".yidam/nodes/n.md", "Read at 0000000deadbeef.\n")];
        let c = check(&here, &nodes, &[]);
        assert_eq!(c.violations.len(), 1, "{:?}", c.violations);
        assert!(
            c.violations[0].detail.contains("neither"),
            "{}",
            c.violations[0].detail
        );
    }

    #[test]
    fn docs_and_a_dependency_corpus_are_not_read() {
        let tmp = tempfile::tempdir().unwrap();
        let (here, _, theirs) = pair(tmp.path());
        let text = format!("Read at {theirs}.\n");
        let nodes = [
            node("docs/notes.md", &text),
            node(".yidam/tonpa/sib/nodes/n.md", &text),
        ];
        assert!(check(&here, &nodes, &[]).passed());
    }

    #[test]
    fn outside_a_repository_nothing_is_reported() {
        let tmp = tempfile::tempdir().unwrap();
        let nodes = [node(".yidam/nodes/n.md", "Read at 1a2b3c4d.\n")];
        assert!(check(tmp.path(), &nodes, &[]).passed());
    }
}
