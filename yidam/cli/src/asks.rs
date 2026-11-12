//! Questions this corpus addresses to a corpus it depends on (#1467).
//!
//! # Why they need a home
//!
//! A repository that consumes other corpora finds most of its questions at the boundary — a
//! fixture that will not reproduce, a unit that does not match, a value the other corpus holds
//! `[open]`. Those are questions *for the other corpus*, and this repository may not write
//! there: a derived corpus never writes to a corpus it cites, and there is no forum between
//! two peer corpora (`agent-conduct.md`, "When your corpus disagrees with the one it cites").
//!
//! So the question has to live here, and before this module it lived nowhere the tooling
//! could see. The consumer that reported it kept 41 drafts in a hand-made outbox outside
//! `.yidam/`, each carrying the pin it was read against and a status maintained in prose.
//! Nothing told it when the other corpus moved past a draft's pin, so drafts went stale
//! silently. Of the 40 it tabulated, 26 had been answered upstream.
//!
//! # The shape
//!
//! `.yidam/asks/<package>/<slug>.md`: the directory is the dependency, as `.yidam/tonpa.toml`
//! names it, and the file is markdown, because an ask is prose addressed to a person in
//! another repository. The header is the part this module reads:
//!
//! ```yaml
//! ---
//! pin: 0bd97e7b                          # the far side's commit this was read against
//! node: model/intelligent-driver-model   # optional; absent means the corpus as a whole
//! status: answered                       # open | answered | superseded | dropped
//! answered_at: b3f596a7                  # the far side's commit, when answered
//! ---
//! ```
//!
//! `superseded` names its successor by slug under `superseded_by:`, in the same package's
//! directory. Every ask is a draft: nothing here sends one anywhere.
//!
//! # What the clock can and cannot know
//!
//! The question this answers is *has the corpus this was asked of moved since it was asked*,
//! and it is answered offline, against the pin this repository installed — the lock's
//! `commit`, or the declared `rev` before anything was locked. It is not the far side's
//! `HEAD`: a sibling checkout may not be on this machine, and a fetched dependency has no
//! history to ask. The re-pin is also the moment that matters, because it is the moment this
//! repository starts reading the far side's new answers.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Every status an ask can be in. The roster `ask-malformed` checks a header against.
pub const STATUSES: &[&str] = &["open", "answered", "superseded", "dropped"];

/// An ask's header.
#[derive(Debug, Default, Clone, serde::Deserialize)]
pub struct Header {
    pub pin: Option<String>,
    pub node: Option<String>,
    pub status: Option<String>,
    pub answered_at: Option<String>,
    pub superseded_by: Option<String>,
}

/// One ask, as read from disk.
#[derive(Debug, Clone)]
pub struct Ask {
    /// Repository-relative path, `/`-separated.
    pub rel: String,
    /// The directory it sits in, which is the dependency it is addressed to. `None` for a file
    /// directly under `.yidam/asks/`, which names no dependency at all.
    pub package: Option<String>,
    /// The file stem, which is what `superseded_by:` names.
    pub slug: String,
    pub header: Header,
    /// Why the header did not parse, when it did not.
    pub unreadable: Option<String>,
    /// The first markdown heading of the body, or the slug when it has none.
    pub title: String,
}

impl Ask {
    /// Whether this ask is still waiting on the far side. An ask with no `status:` is open,
    /// which is what a draft is until someone says otherwise; `ask-malformed` reports the
    /// missing key rather than this reading guessing at a different state.
    pub fn is_open(&self) -> bool {
        self.unreadable.is_none() && self.header.status.as_deref().unwrap_or("open") == "open"
    }
}

/// `.yidam/asks/`.
pub fn dir(root: &Path) -> PathBuf {
    crate::paths::yidam_asks_dir(root)
}

/// Every `.md` file under `.yidam/asks/`, at most one directory deep, in path order.
///
/// A `README.md` directly under `.yidam/asks/` is the directory's own prose and is not an ask.
pub fn read(root: &Path) -> Vec<Ask> {
    let base = dir(root);
    let mut files: Vec<(Option<String>, PathBuf)> = Vec::new();
    let Ok(entries) = std::fs::read_dir(&base) else {
        return Vec::new();
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let package = path.file_name().map(|n| n.to_string_lossy().into_owned());
            if let Ok(inner) = std::fs::read_dir(&path) {
                files.extend(
                    inner
                        .flatten()
                        .map(|e| e.path())
                        .filter(|p| is_markdown(p))
                        .map(|p| (package.clone(), p)),
                );
            }
        } else if is_markdown(&path) && path.file_name().is_some_and(|n| n != "README.md") {
            files.push((None, path));
        }
    }
    files.sort_by(|a, b| a.1.cmp(&b.1));
    files
        .into_iter()
        .map(|(package, path)| {
            let text = std::fs::read_to_string(&path).unwrap_or_default();
            let (header, unreadable) = crate::parse::parse_header_as::<Header>(&text);
            let slug = path
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
            let title = crate::parse::frontmatter_body(&text)
                .lines()
                .find_map(|l| {
                    l.strip_prefix('#')
                        .map(|h| h.trim_start_matches('#').trim())
                })
                .filter(|h| !h.is_empty())
                .map(str::to_string)
                .unwrap_or_else(|| slug.clone());
            Ask {
                rel: rel(root, &path),
                package,
                slug,
                header,
                unreadable,
                title,
            }
        })
        .collect()
}

fn is_markdown(p: &Path) -> bool {
    p.is_file() && p.extension().is_some_and(|e| e == "md")
}

fn rel(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .components()
        .map(|c| c.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

/// Two spellings of one commit are the same commit when either is a prefix of the other.
///
/// A short sha's length is chosen by git from the producing repository's object count, so an
/// ask written with eight characters and a lock holding forty are the same pin.
/// `external-citation-pin-moved` compares the same way and for the same reason.
pub fn same_commit(a: &str, b: &str) -> bool {
    let (a, b) = (a.trim(), b.trim());
    !a.is_empty() && !b.is_empty() && (a.starts_with(b) || b.starts_with(a))
}

/// What this repository has pinned a dependency at, and since when.
#[derive(Debug, Clone, PartialEq)]
pub struct Pinned {
    pub commit: String,
    /// Days since the epoch of the commit that put this pin into the lock. `None` where git
    /// cannot say — the lock is uncommitted, or there is no lock and the pin is only declared.
    pub since: Option<i64>,
}

/// Every declared dependency's current pin, keyed by name. A dependency read live from a
/// sibling's working tree has no pin and maps to `None`.
///
/// The lock's `commit` is preferred to the declaration's `rev`, because the lock is what was
/// actually installed. The declaration is read where nothing is locked yet.
pub fn pins(root: &Path) -> BTreeMap<String, Option<Pinned>> {
    let config = crate::deps::load_config(&crate::paths::tonpa_config_path(root));
    let lock_path = crate::paths::tonpa_lock_path(root);
    let lock = crate::deps::load_lock(&lock_path).unwrap_or_default();
    config
        .dependencies
        .iter()
        .map(|(name, dep)| {
            let locked = lock
                .packages
                .iter()
                .find(|p| &p.name == name)
                .and_then(|p| p.commit.clone().or_else(|| p.rev.clone()));
            let pinned = match (locked, dep.rev.clone()) {
                (Some(commit), _) => Some(Pinned {
                    since: pinned_since(root, &lock_path, &commit),
                    commit,
                }),
                (None, Some(rev)) => Some(Pinned {
                    commit: rev,
                    since: None,
                }),
                (None, None) => None,
            };
            (name.clone(), pinned)
        })
        .collect()
}

/// The day the lock last gained `commit`, by git's pickaxe.
///
/// The newest commit that changed how often the sha appears is the one that added it: the pin
/// is in the lock now, so any later commit that took it out would have to be followed by one
/// that put it back.
fn pinned_since(root: &Path, lock: &Path, commit: &str) -> Option<i64> {
    let rel = lock.strip_prefix(root).ok()?;
    let out = crate::git::Git::new(root)
        .args(["log", "-1", "--format=%ct"])
        .arg(format!("-S{commit}"))
        .paths([rel])
        .try_run()?;
    let ct: i64 = out.trim().parse().ok()?;
    Some(ct.div_euclid(86_400))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(root: &Path, rel: &str, text: &str) {
        let p = root.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, text).unwrap();
    }

    #[test]
    fn an_ask_is_read_from_its_package_directory() {
        let tmp = tempfile::tempdir().unwrap();
        write(
            tmp.path(),
            ".yidam/asks/upstream/three-units.md",
            "---\npin: 0bd97e7b\nnode: concept/base-flow\n---\n\n# Three units\n\nBody.\n",
        );
        let asks = read(tmp.path());
        assert_eq!(asks.len(), 1);
        let a = &asks[0];
        assert_eq!(a.rel, ".yidam/asks/upstream/three-units.md");
        assert_eq!(a.package.as_deref(), Some("upstream"));
        assert_eq!(a.slug, "three-units");
        assert_eq!(a.title, "Three units");
        assert_eq!(a.header.pin.as_deref(), Some("0bd97e7b"));
        assert_eq!(a.header.node.as_deref(), Some("concept/base-flow"));
        assert!(a.is_open(), "no status reads as open");
    }

    #[test]
    fn a_readme_at_the_top_is_not_an_ask_and_a_loose_file_is() {
        let tmp = tempfile::tempdir().unwrap();
        write(tmp.path(), ".yidam/asks/README.md", "# Asks\n");
        write(tmp.path(), ".yidam/asks/loose.md", "---\npin: a\n---\n");
        let asks = read(tmp.path());
        assert_eq!(asks.len(), 1);
        assert_eq!(asks[0].package, None);
    }

    #[test]
    fn only_an_open_ask_is_open() {
        let tmp = tempfile::tempdir().unwrap();
        for s in STATUSES {
            write(
                tmp.path(),
                &format!(".yidam/asks/up/{s}.md"),
                &format!("---\npin: a\nstatus: {s}\n---\n"),
            );
        }
        write(tmp.path(), ".yidam/asks/up/broken.md", "---\npin: [\n---\n");
        let open: Vec<String> = read(tmp.path())
            .into_iter()
            .filter(Ask::is_open)
            .map(|a| a.slug)
            .collect();
        assert_eq!(
            open,
            vec!["open"],
            "an unreadable header is not open either"
        );
    }

    #[test]
    fn a_short_and_a_long_sha_are_one_commit() {
        assert!(same_commit("0bd97e7b", "0bd97e7bbd960c2f"));
        assert!(same_commit("0bd97e7bbd960c2f", "0bd97e7b"));
        assert!(!same_commit("0bd97e7b", "1ed4ccb4"));
        assert!(!same_commit("", "0bd97e7b"), "an empty pin matches nothing");
    }

    #[test]
    fn the_lock_wins_over_the_declaration_and_a_live_path_has_no_pin() {
        let tmp = tempfile::tempdir().unwrap();
        write(
            tmp.path(),
            ".yidam/tonpa.toml",
            "[dependencies.locked]\npath = \"../l\"\nrev = \"aaaa\"\n\
             [dependencies.declared]\npath = \"../d\"\nrev = \"bbbb\"\n\
             [dependencies.live]\npath = \"../v\"\n",
        );
        write(
            tmp.path(),
            ".yidam/tonpa/tonpa.lock",
            "[[package]]\nname = \"locked\"\nurl = \"../l\"\nsha256 = \"x\"\ncommit = \"cccc\"\nrev = \"cccc\"\n",
        );
        let p = pins(tmp.path());
        assert_eq!(p["locked"].as_ref().unwrap().commit, "cccc");
        assert_eq!(p["declared"].as_ref().unwrap().commit, "bbbb");
        assert_eq!(p["declared"].as_ref().unwrap().since, None);
        assert_eq!(p["live"], None);
    }
}
