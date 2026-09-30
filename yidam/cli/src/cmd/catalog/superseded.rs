//! Which nodes cite a source they read before its latest version (#1200).
//!
//! # What was silent
//!
//! A re-fetch of a source whose bytes changed appends a second record ([`super::record`]
//! says why both are kept), and one kind of citation reacts: an unpinned quotation of an entry
//! holding two artifacts becomes `quotation-unresolved`. Every other kind stays silent — a
//! markdown link, an edge `source:`, a quotation pinned to the old digest. The node goes on
//! citing the entry, and the entry now holds bytes the author never read.
//!
//! `due`'s catalog clock does not see it either. That clock measures **age** against a TTL,
//! so a source re-fetched yesterday reads as fresh whatever its content did. A new version is
//! an event rather than an age, and it is the case that matters most.
//!
//! # What counts as a new version
//!
//! **A digest arriving from an origin the entry already held a digest from.** Not merely a
//! second digest: an entry commonly lists a human-facing page beside a machine endpoint, and
//! the first fetch of the endpoint after the page is a second source, not a second edition.
//! The origin is the record's `from:`, compared the way [`super::fetch`]'s `prior_for`
//! compares it — with one difference, argued where it lives: two records that both say
//! nothing about their origin are read as one source here.
//!
//! A text reading (#1172) is not a version. It is a reading of bytes that did not change, and
//! it lives under the record rather than beside it, so it never reaches the digest set read
//! here.
//!
//! # What discharges it
//!
//! **A commit touching the node after the commit that recorded the version.** Of the three
//! options #1200 lists this is the cheap one, and the one it calls easy to game — and a touch
//! is still a person's act, in a commit, in review, which is where every other judgement in
//! this corpus is checked. The precise option, pinning each citation to a digest, asks for a
//! pin on every link, and a link has no pin to write. A quotation pinned to the new digest is
//! covered anyway: the pin cannot be written until the digest exists, so writing it is a touch.
//!
//! # Computed from history, on each run
//!
//! Nothing is stored. The digests an entry held at each of its commits are read with
//! `git show`, and only for entries whose working tree already holds two digests from one
//! origin — every other entry is answered by one parse and no subprocess. Then one `git log`
//! per such entry says which of its citing nodes have been touched since.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use crate::corpus::{normalize, Corpus};
use crate::parse::{parse_frontmatter, CatalogArtifact};
use crate::paths::yidam_catalog_dir;
use crate::walk::walk_md_files;

/// One entry whose latest version postdates some of the nodes citing it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Superseded {
    /// The catalog entry's file stem.
    pub entry: String,
    /// The digest the superseding commit recorded.
    pub digest: String,
    /// The commit that recorded it.
    pub commit: String,
    /// Its commit date, as days since the epoch.
    pub day: i64,
    /// Nodes citing the entry and untouched since that commit, repo-relative and sorted.
    pub nodes: Vec<String>,
}

/// Everything the `due` clock needs from one read.
#[derive(Debug, Default)]
pub(crate) struct Reading {
    /// Entries owed a re-read, with at least one node each.
    pub superseded: Vec<Superseded>,
    /// Entries whose working tree holds more than one version of some origin — the ones whose
    /// history was read.
    pub versioned: usize,
    /// Of those, the entries whose history could not be read.
    pub unread: usize,
}

/// Whether two records name the same origin, for the purpose of calling one a new version
/// of the other.
///
/// `prior_for` in [`super::fetch`] matches nothing against a record with no `from:`, and is
/// right to: it lends a licence forward, and the failing direction there is lending one
/// publisher's terms to another's bytes. The failing direction here is the opposite one —
/// silence about a claim that may no longer hold — so two unattributed records are read as one
/// source. Hand-written and bulk-imported records carry no `from:` at all, and reading them
/// as unrelated would make every edition they record invisible to this.
fn same_origin(a: &CatalogArtifact, b: &CatalogArtifact) -> bool {
    a.from == b.from
}

fn digest(a: &CatalogArtifact) -> Option<&str> {
    a.sha256.as_deref().map(str::trim).filter(|h| !h.is_empty())
}

/// Whether `new` is a new version of something `held` already records.
///
/// A digest already held is not new — re-recording one is the no-op
/// [`super::record::append_artifacts`] skips — and a digest from an origin nothing held names
/// a second source rather than a second edition.
pub(crate) fn supersedes(held: &[CatalogArtifact], new: &CatalogArtifact) -> bool {
    let Some(h) = digest(new) else {
        return false;
    };
    !held.iter().any(|a| digest(a) == Some(h))
        && held
            .iter()
            .any(|a| digest(a).is_some() && same_origin(a, new))
}

/// Whether an entry holds two digests from one origin, which is the precondition for any of
/// its history being a supersession.
fn has_versions(artifacts: &[CatalogArtifact]) -> bool {
    artifacts
        .iter()
        .enumerate()
        .any(|(i, a)| supersedes(&artifacts[..i], a))
}

/// One commit's view of an entry, oldest first.
#[derive(Debug, Clone)]
pub(crate) struct Revision {
    pub commit: String,
    pub day: i64,
    pub artifacts: Vec<CatalogArtifact>,
}

/// The latest commit that recorded a new version, and the digest it recorded.
///
/// Read against everything held **up to** that commit rather than against its parent alone,
/// so a digest that was removed and later written back is not a new version the second time.
/// When one commit records several new versions the last in list order is named, which is the
/// last one appended.
pub(crate) fn latest_supersession(history: &[Revision]) -> Option<(&Revision, String)> {
    let mut seen: Vec<CatalogArtifact> = Vec::new();
    let mut latest = None;
    for rev in history {
        let mut recorded = None;
        for a in &rev.artifacts {
            if supersedes(&seen, a) {
                recorded = digest(a).map(str::to_string);
            }
        }
        // Appended only after the commit is read in full, so two digests arriving together
        // from one origin are two records of one event and not a version of each other.
        for a in &rev.artifacts {
            if digest(a).is_some() && !seen.iter().any(|s| digest(s) == digest(a)) {
                seen.push(a.clone());
            }
        }
        if let Some(d) = recorded {
            latest = Some((rev, d));
        }
    }
    latest
}

/// Every commit that touched the entry at `rel`, oldest first, with what it held then.
///
/// `--follow`, so a `rename` of the entry does not start its history over, and the path each
/// commit printed is the one read back from it. A commit whose tree does not hold the file —
/// the one that deleted it — contributes nothing rather than an empty set, which would read
/// a later restore as every digest arriving at once.
///
/// `None` when git cannot answer at all.
fn history(root: &Path, rel: &str) -> Option<Vec<Revision>> {
    let text = crate::git::Git::new(root)
        .args(["log", "--follow", "--format=%x00%H %ct", "--name-only"])
        .paths([rel])
        .try_run()?;
    let mut touched: Vec<(String, i64, String)> = Vec::new();
    let mut current: Option<(String, i64)> = None;
    for line in text.lines() {
        if let Some(head) = line.strip_prefix('\0') {
            let mut parts = head.split(' ');
            let sha = parts.next().unwrap_or_default().to_string();
            let ct: i64 = parts
                .next()
                .and_then(|t| t.parse().ok())
                .unwrap_or_default();
            current = Some((sha, ct.div_euclid(86_400)));
            continue;
        }
        if line.trim().is_empty() {
            continue;
        }
        if let Some((sha, day)) = current.take() {
            touched.push((sha, day, line.to_string()));
        }
    }
    touched.reverse();
    Some(
        touched
            .into_iter()
            .filter_map(|(commit, day, path)| {
                let text = crate::git::Git::new(root)
                    .arg("show")
                    .rev(format!("{commit}:{path}"))
                    .try_run()?;
                Some(Revision {
                    commit,
                    day,
                    artifacts: parse_frontmatter(&text).artifacts.unwrap_or_default(),
                })
            })
            .collect(),
    )
}

/// Of `nodes`, the ones some commit after `since` touched.
///
/// `None` when git refuses, which the caller reads as *everything owed is unknown* rather
/// than as *nothing was touched* — the second would report every citing node as owed on a
/// clone that simply cannot see the range.
pub(crate) fn touched_since(root: &Path, since: &str, nodes: &[String]) -> Option<HashSet<String>> {
    if nodes.is_empty() {
        return Some(HashSet::new());
    }
    let text = crate::git::Git::new(root)
        .args(["log", "--format=", "--name-only"])
        .rev(format!("{since}..HEAD"))
        .paths(nodes)
        .try_run()?;
    Some(
        text.lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .map(str::to_string)
            .collect(),
    )
}

/// The nodes citing each catalog entry, keyed by the entry's normalized path.
pub(crate) fn citing_nodes(root: &Path) -> HashMap<std::path::PathBuf, Vec<String>> {
    if !crate::paths::yidam_corpus_dir(root).exists() {
        return HashMap::new();
    }
    super::audit::draws_on(&Corpus::open(root))
        .into_iter()
        .map(|(k, v)| (k, v.nodes))
        .collect()
}

/// Read every entry in `root`'s catalog.
pub(crate) fn read(root: &Path) -> Reading {
    let dir = yidam_catalog_dir(root);
    let entries: Vec<(std::path::PathBuf, Vec<CatalogArtifact>)> = walk_md_files(&dir)
        .into_iter()
        .filter(|p| p.file_name().is_some_and(|n| n != "README.md"))
        .filter_map(|p| {
            let text = std::fs::read_to_string(&p).ok()?;
            let held = parse_frontmatter(&text).artifacts.unwrap_or_default();
            has_versions(&held).then_some((p, held))
        })
        .collect();

    let mut out = Reading {
        versioned: entries.len(),
        ..Reading::default()
    };
    if entries.is_empty() {
        return out;
    }
    let citing = citing_nodes(root);

    for (path, _) in entries {
        let rel = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .to_string();
        let Some(history) = history(root, &rel) else {
            out.unread += 1;
            continue;
        };
        // Held in the working tree and in no commit: not yet a version anything was read
        // against, and a `refresh:` commit is what makes it one.
        let Some((rev, digest)) = latest_supersession(&history) else {
            continue;
        };
        let nodes = citing.get(&normalize(&path)).cloned().unwrap_or_default();
        let Some(touched) = touched_since(root, &rev.commit, &nodes) else {
            out.unread += 1;
            continue;
        };
        let mut owed: Vec<String> = nodes.into_iter().filter(|n| !touched.contains(n)).collect();
        if owed.is_empty() {
            continue;
        }
        owed.sort();
        out.superseded.push(Superseded {
            entry: path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string(),
            digest,
            commit: rev.commit.clone(),
            day: rev.day,
            nodes: owed,
        });
    }
    out.superseded.sort_by(|a, b| a.entry.cmp(&b.entry));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::ArtifactOrigin;

    fn rec(sha: &str, from: Option<usize>) -> CatalogArtifact {
        CatalogArtifact {
            sha256: Some(sha.to_string()),
            from: from.map(ArtifactOrigin::Location),
            ..Default::default()
        }
    }

    fn rev(commit: &str, artifacts: Vec<CatalogArtifact>) -> Revision {
        Revision {
            commit: commit.to_string(),
            day: 0,
            artifacts,
        }
    }

    #[test]
    fn a_new_digest_from_a_held_origin_is_a_new_version() {
        assert!(supersedes(&[rec("aa", Some(0))], &rec("bb", Some(0))));
    }

    /// A page beside an endpoint: the first fetch of the second is a second source.
    #[test]
    fn a_first_digest_from_another_origin_is_not() {
        assert!(!supersedes(&[rec("aa", Some(0))], &rec("bb", Some(1))));
    }

    #[test]
    fn a_digest_already_held_is_not() {
        assert!(!supersedes(&[rec("aa", Some(0))], &rec("aa", Some(0))));
    }

    #[test]
    fn nothing_held_supersedes_nothing() {
        assert!(!supersedes(&[], &rec("aa", Some(0))));
    }

    /// Hand-written records carry no `from:`, and reading them as unrelated would hide every
    /// edition they record.
    #[test]
    fn two_records_naming_no_origin_are_one_source() {
        assert!(supersedes(&[rec("aa", None)], &rec("bb", None)));
        assert!(!supersedes(&[rec("aa", None)], &rec("bb", Some(0))));
    }

    /// A reading sits under its record, so a record gaining one gains no digest.
    #[test]
    fn a_text_reading_is_not_a_version() {
        let mut read = rec("aa", Some(0));
        read.text = Some(crate::parse::TextReading {
            sha256: Some("tt".into()),
            extractor: Some("x".into()),
        });
        let history = [rev("c1", vec![rec("aa", Some(0))]), rev("c2", vec![read])];
        assert!(latest_supersession(&history).is_none());
    }

    /// A first fetch of an entry with two locations records two digests in one commit, and
    /// neither is a version of the other.
    #[test]
    fn two_digests_arriving_together_are_one_event() {
        let history = [rev("c1", vec![rec("aa", None), rec("bb", None)])];
        assert!(latest_supersession(&history).is_none());
    }

    #[test]
    fn the_latest_superseding_commit_is_named() {
        let history = [
            rev("c1", vec![rec("aa", Some(0))]),
            rev("c2", vec![rec("aa", Some(0)), rec("bb", Some(0))]),
            rev("c3", vec![rec("aa", Some(0)), rec("bb", Some(0))]),
            rev(
                "c4",
                vec![rec("aa", Some(0)), rec("bb", Some(0)), rec("cc", Some(0))],
            ),
        ];
        let (r, d) = latest_supersession(&history).unwrap();
        assert_eq!((r.commit.as_str(), d.as_str()), ("c4", "cc"));
    }

    /// Removing a digest and writing it back is not a new version the second time.
    #[test]
    fn a_digest_written_back_is_not_new_again() {
        let history = [
            rev("c1", vec![rec("aa", Some(0))]),
            rev("c2", vec![rec("aa", Some(0)), rec("bb", Some(0))]),
            rev("c3", vec![rec("aa", Some(0))]),
            rev("c4", vec![rec("aa", Some(0)), rec("bb", Some(0))]),
        ];
        assert_eq!(latest_supersession(&history).unwrap().0.commit, "c2");
    }
}
