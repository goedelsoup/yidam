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
//! **And the same resource at that origin.** One location can answer many distinct requests —
//! an API base URL queried at several endpoints, a download page serving one file per state —
//! and a new endpoint or a new state's file is a second source, not a second edition of the
//! first (#1432). A record's `resource:` names which of them its bytes are, and two records
//! are versions of each other only when origin and resource both match. A record with no
//! `resource:` is read as the location itself, so two of those still match, and an entry that
//! never writes the key behaves exactly as it did before the key existed. A keyed record and an
//! unkeyed one do not match: the unkeyed one is the location's own bytes, or a resource nobody
//! named, and neither is evidence about the keyed one.
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
use crate::parse::{parse_frontmatter, CatalogArtifact, CatalogLocation};
use crate::paths::yidam_catalog_dir;
use crate::walk::walk_md_files;

/// One entry whose latest version postdates some of the nodes citing it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Superseded {
    /// The catalog entry's file stem.
    pub entry: String,
    /// What the superseding commit recorded, for display: `sha256:<12 digits>` for a new
    /// digest, or `` `path` at <8 digits> `` for a pinned read moved onto changed code.
    pub version: String,
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
    /// Entries whose working tree holds more than one version of some origin, or a pinned
    /// read — the ones whose history was read.
    pub versioned: usize,
    /// Of those, the entries whose history could not be read.
    pub unread: usize,
}

/// Whether two records name the same origin and the same resource at it, for the purpose of
/// calling one a new version of the other.
///
/// `prior_for` in [`super::fetch`] matches nothing against a record with no `from:`, and is
/// right to: it lends a licence forward, and the failing direction there is lending one
/// publisher's terms to another's bytes. The failing direction here is the opposite one —
/// silence about a claim that may no longer hold — so two unattributed records are read as one
/// source. Hand-written and bulk-imported records carry no `from:` at all, and reading them
/// as unrelated would make every edition they record invisible to this.
///
/// The resource is compared after trimming, and an empty one is an absent one, so a key
/// written as `resource: ""` does not split an entry's history in two.
fn same_source(a: &CatalogArtifact, b: &CatalogArtifact) -> bool {
    a.from == b.from && resource(a) == resource(b)
}

fn resource(a: &CatalogArtifact) -> Option<&str> {
    a.resource
        .as_deref()
        .map(str::trim)
        .filter(|r| !r.is_empty())
}

fn digest(a: &CatalogArtifact) -> Option<&str> {
    a.sha256.as_deref().map(str::trim).filter(|h| !h.is_empty())
}

/// Whether `new` is a new version of something `held` already records.
///
/// A digest already held is not new — re-recording one is the no-op
/// [`super::record::append_artifacts`] skips — and a digest from an origin, or a resource at
/// one, that nothing held names a second source rather than a second edition.
pub(crate) fn supersedes(held: &[CatalogArtifact], new: &CatalogArtifact) -> bool {
    let Some(h) = digest(new) else {
        return false;
    };
    !held.iter().any(|a| digest(a) == Some(h))
        && held
            .iter()
            .any(|a| digest(a).is_some() && same_source(a, new))
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
    pub locations: Vec<CatalogLocation>,
}

/// A pinned read whose `commit` moved between two revisions of an entry (RFC-0051 §4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Move {
    /// The index in the history of the revision that moved it.
    pub at: usize,
    pub repo: Option<String>,
    pub path: String,
    pub from: String,
    pub to: String,
    /// The `symbol` the later revision names, which is the read the move made.
    pub symbol: Option<String>,
    /// The `symbol` the earlier revision named.
    pub from_symbol: Option<String>,
}

impl Move {
    /// The symbol to compare the move at: one only when both revisions name the same one.
    /// A read that narrowed to a symbol, widened to its file or turned to another item is
    /// compared by its file, as a pin without a symbol is.
    pub(crate) fn same_symbol(&self) -> Option<&str> {
        self.symbol
            .as_deref()
            .filter(|_| self.symbol == self.from_symbol)
    }
}

/// Every move of a pinned read's `commit`, oldest first.
///
/// A pin is one read of one file in one repository, so it is matched across revisions by
/// `repo` and path, and by `symbol` among several pins of one file. A later pin that is an
/// earlier pin unchanged moved nothing. Otherwise it takes the earlier pin naming the same
/// symbol, or the one earlier pin of its file when the file had and has exactly one. A pin
/// seen for the first time is a first read and moves nothing, and a pin whose shape lint
/// rejects is not a read at all.
pub(crate) fn pin_moves(history: &[Revision]) -> Vec<Move> {
    type Key = (Option<String>, String);
    type Pins = Vec<(Option<String>, String)>;
    let mut last: HashMap<Key, Pins> = HashMap::new();
    let mut moves = Vec::new();
    for (at, rev) in history.iter().enumerate() {
        let mut now: Vec<(Key, Pins)> = Vec::new();
        for loc in &rev.locations {
            let Ok((commit, path)) = crate::cmd::lint::pins::shape(loc) else {
                continue;
            };
            let key = (loc.repo.clone(), path);
            let pin = (loc.symbol.clone(), commit);
            match now.iter_mut().find(|(k, _)| *k == key) {
                Some((_, pins)) => pins.push(pin),
                None => now.push((key, vec![pin])),
            }
        }
        for (key, pins) in now {
            if let Some(before) = last.get(&key) {
                for pin @ (symbol, commit) in &pins {
                    if before.contains(pin) {
                        continue;
                    }
                    let from = before
                        .iter()
                        .find(|(s, _)| s == symbol)
                        .or_else(|| (before.len() == 1 && pins.len() == 1).then(|| &before[0]));
                    if let Some((from_symbol, from)) = from.filter(|(_, c)| c != commit) {
                        moves.push(Move {
                            at,
                            repo: key.0.clone(),
                            path: key.1.clone(),
                            from: from.clone(),
                            to: commit.clone(),
                            symbol: symbol.clone(),
                            from_symbol: from_symbol.clone(),
                        });
                    }
                }
            }
            last.insert(key, pins);
        }
    }
    moves
}

/// The latest move whose two commits hold different objects, which is the only move that is
/// a new edition: re-dating a read of unchanged code supersedes nothing. A move of a pin
/// naming the same `symbol` on both sides is compared as the catalog clock compares it, so a
/// move across edits elsewhere in the file supersedes nothing either.
///
/// `Err` when some later move could not be compared, because a clone that cannot see a
/// commit has not shown the code is unchanged.
fn latest_changed_move<'a>(
    root: &Path,
    moves: &'a [Move],
    config: &crate::deps::TonpaConfig,
) -> Result<Option<&'a Move>, ()> {
    use crate::cmd::lint::pins::{place, resolve, Lookup, Place, Query};
    type Object = Option<(String, Option<String>)>;
    let mut objects: HashMap<(Option<String>, String, String), Object> = HashMap::new();
    let mut by_repo: HashMap<Option<String>, Vec<&Move>> = HashMap::new();
    for m in moves {
        by_repo.entry(m.repo.clone()).or_default().push(m);
    }
    for (repo, ms) in by_repo {
        let Place::Dir(dir) = place(root, repo.as_deref(), config) else {
            continue;
        };
        let queries: Vec<Query> = ms
            .iter()
            .flat_map(|m| {
                [&m.from, &m.to].map(|c| Query {
                    commit: c.clone(),
                    path: m.path.clone(),
                    contents: m.same_symbol().is_some(),
                })
            })
            .collect();
        let Some(answers) = resolve(&dir, &queries) else {
            continue;
        };
        for (q, a) in queries.iter().zip(answers) {
            let oid = match a.object {
                Lookup::Found { oid, .. } => Some((oid, a.text)),
                _ => None,
            };
            objects.insert((repo.clone(), q.commit.clone(), q.path.clone()), oid);
        }
    }
    let object = |m: &Move, c: &String| {
        objects
            .get(&(m.repo.clone(), c.clone(), m.path.clone()))
            .cloned()
            .flatten()
    };
    for m in moves.iter().rev() {
        match (object(m, &m.from), object(m, &m.to)) {
            (Some((a, _)), Some((b, _))) if a == b => continue,
            (Some((_, was)), Some((_, now))) => match m.same_symbol() {
                Some(symbol)
                    if !crate::cmd::lint::symbols::compare(
                        &m.path,
                        symbol,
                        was.as_deref(),
                        now.as_deref(),
                    )
                    .due() =>
                {
                    continue
                }
                _ => return Ok(Some(m)),
            },
            _ => return Err(()),
        }
    }
    Ok(None)
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
/// Each revision's text is read in one `cat-file` process for the whole history, not one
/// `git show` per commit.
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
    let queries: Vec<crate::cmd::lint::pins::Query> = touched
        .iter()
        .map(|(commit, _, path)| crate::cmd::lint::pins::Query {
            commit: commit.clone(),
            path: path.clone(),
            contents: true,
        })
        .collect();
    let answers = crate::cmd::lint::pins::resolve(root, &queries)?;
    Some(
        touched
            .into_iter()
            .zip(answers)
            .filter_map(|((commit, day, _), a)| {
                let fm = parse_frontmatter(&a.text?);
                Some(Revision {
                    commit,
                    day,
                    artifacts: fm.artifacts.unwrap_or_default(),
                    locations: fm.location.unwrap_or_default(),
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
            let fm = parse_frontmatter(&text);
            let held = fm.artifacts.unwrap_or_default();
            let pinned = fm
                .location
                .unwrap_or_default()
                .iter()
                .any(|l| l.is_pinned());
            (has_versions(&held) || pinned).then_some((p, held))
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
    let config = crate::deps::load_config(&crate::paths::tonpa_config_path(root));

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
        let digested = latest_supersession(&history).map(|(rev, digest)| {
            let at = history
                .iter()
                .position(|r| std::ptr::eq(r, rev))
                .unwrap_or_default();
            (
                at,
                format!("sha256:{}", digest.get(..12).unwrap_or(&digest)),
            )
        });
        let moves = pin_moves(&history);
        let Ok(moved) = latest_changed_move(root, &moves, &config) else {
            out.unread += 1;
            continue;
        };
        let moved = moved.map(|m| {
            let path = match &m.repo {
                Some(r) => format!("{r}:{}", m.path),
                None => m.path.clone(),
            };
            (
                m.at,
                format!("`{path}` at {}", m.to.get(..8).unwrap_or(&m.to)),
            )
        });
        // The later of the two events, so a node touched after the last one owes nothing.
        let Some((at, version)) = digested.into_iter().chain(moved).max_by_key(|(at, _)| *at)
        else {
            continue;
        };
        let Some(rev) = history.get(at) else {
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
            version,
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

    fn at(sha: &str, from: Option<usize>, resource: &str) -> CatalogArtifact {
        CatalogArtifact {
            resource: Some(resource.to_string()),
            ..rec(sha, from)
        }
    }

    fn rev(commit: &str, artifacts: Vec<CatalogArtifact>) -> Revision {
        Revision {
            commit: commit.to_string(),
            day: 0,
            artifacts,
            locations: Vec::new(),
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

    /// #1432: an API base URL asked a second endpoint, and a download page's next state file.
    /// Each is a second source behind one location, and nothing anyone read has changed.
    #[test]
    fn a_new_resource_at_a_held_origin_is_not() {
        let held = [at("aa", Some(0), "schedule_b?committee_id=C1")];
        assert!(!supersedes(
            &held,
            &at("bb", Some(0), "committee/C1/totals")
        ));
        let held = [
            at("oh", Some(0), "eo_oh.csv"),
            at("va", Some(0), "eo_va.csv"),
        ];
        assert!(!supersedes(&held, &at("de", Some(0), "eo_de.csv")));
    }

    #[test]
    fn a_new_digest_of_a_held_resource_is_a_new_version() {
        let held = [
            at("oh", Some(0), "eo_oh.csv"),
            at("va", Some(0), "eo_va.csv"),
        ];
        assert!(supersedes(&held, &at("oh2", Some(0), "eo_oh.csv")));
        // Trimmed, so a stray space is not a second source.
        assert!(supersedes(&held, &at("oh2", Some(0), " eo_oh.csv ")));
    }

    /// The resource narrows the origin and does not replace it: one file name at two
    /// locations is two sources.
    #[test]
    fn one_resource_at_two_origins_is_two_sources() {
        assert!(!supersedes(
            &[at("aa", Some(0), "x.csv")],
            &at("bb", Some(1), "x.csv")
        ));
    }

    /// An unkeyed record is the location's own bytes, or a resource nobody named; neither says
    /// anything about a keyed one, in either direction. An empty key is no key.
    #[test]
    fn a_keyed_and_an_unkeyed_record_are_two_sources() {
        assert!(!supersedes(
            &[rec("aa", Some(0))],
            &at("bb", Some(0), "x.csv")
        ));
        assert!(!supersedes(
            &[at("aa", Some(0), "x.csv")],
            &rec("bb", Some(0))
        ));
        assert!(supersedes(&[rec("aa", Some(0))], &at("bb", Some(0), "  ")));
    }

    /// The history #1432 reported: Ohio, Virginia and DC, then Delaware, Florida and
    /// Massachusetts in a later commit. Nothing is a new version until Ohio is re-fetched.
    #[test]
    fn files_added_beside_earlier_ones_supersede_nothing() {
        let first = vec![
            at("oh", Some(0), "eo_oh.csv"),
            at("va", Some(0), "eo_va.csv"),
            at("dc", Some(0), "eo_dc.csv"),
        ];
        let mut second = first.clone();
        second.extend([
            at("de", Some(0), "eo_de.csv"),
            at("fl", Some(0), "eo_fl.csv"),
            at("ma", Some(0), "eo_ma.csv"),
        ]);
        let mut third = second.clone();
        third.push(at("oh2", Some(0), "eo_oh.csv"));
        let history = [rev("c1", first), rev("c2", second.clone())];
        assert!(latest_supersession(&history).is_none());
        assert!(!has_versions(&second));
        let history = [rev("c1", second), rev("c2", third.clone())];
        let (r, d) = latest_supersession(&history).unwrap();
        assert_eq!((r.commit.as_str(), d.as_str()), ("c2", "oh2"));
        assert!(has_versions(&third));
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

    fn pin(commit: &str, symbol: Option<&str>) -> CatalogLocation {
        CatalogLocation {
            kind: Some("file".into()),
            value: Some("src/lib.rs".into()),
            commit: Some(commit.into()),
            symbol: symbol.map(str::to_string),
            ..Default::default()
        }
    }

    fn pinned(locations: Vec<CatalogLocation>) -> Revision {
        Revision {
            locations,
            ..rev("c", Vec::new())
        }
    }

    const A: &str = "aaaaaaa";
    const B: &str = "bbbbbbb";
    const C: &str = "ccccccc";

    #[test]
    fn two_symbol_pins_of_one_file_are_two_reads() {
        let both = || pinned(vec![pin(A, Some("load")), pin(B, Some("save"))]);
        assert_eq!(pin_moves(&[both(), both()]), Vec::new());

        let moved = pinned(vec![pin(A, Some("load")), pin(C, Some("save"))]);
        let moves = pin_moves(&[both(), moved]);
        assert_eq!(moves.len(), 1, "{moves:?}");
        assert_eq!((moves[0].from.as_str(), moves[0].to.as_str()), (B, C));
        assert_eq!(moves[0].same_symbol(), Some("save"));
    }

    #[test]
    fn a_move_that_changes_the_symbol_is_compared_by_its_file() {
        for (was, now) in [
            (None, Some("load")),
            (Some("load"), None),
            (Some("load"), Some("save")),
        ] {
            let moves = pin_moves(&[pinned(vec![pin(A, was)]), pinned(vec![pin(B, now)])]);
            assert_eq!(moves.len(), 1, "{was:?} -> {now:?}: {moves:?}");
            assert_eq!(moves[0].same_symbol(), None, "{was:?} -> {now:?}");
        }
        let moves = pin_moves(&[
            pinned(vec![pin(A, Some("load"))]),
            pinned(vec![pin(B, Some("load"))]),
        ]);
        assert_eq!(moves[0].same_symbol(), Some("load"));
    }
}
