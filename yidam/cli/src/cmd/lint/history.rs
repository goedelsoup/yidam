//! When each node last had something pointing at it.
//!
//! `orphan-in` says a node is uncited. It cannot say for how long, and the difference is the
//! whole reading: a node uncited for five commits is a breadth sweep in progress and entirely
//! healthy, while one uncited for two hundred is over-collection. A derived repository's
//! `orphan-in` rate rose from 22% to 36% across its life and the rise was twelve `recording`
//! nodes landing in a single sweep — indistinguishable, from the level alone, from a corpus
//! decaying.
//!
//! The exact quantity is *when the last inbound edge disappeared*, which needs the graph
//! replayed rather than the node's age. The two differ whenever a node was cited and later
//! orphaned, and only the first is the thing anyone wants to know.
//!
//! ## Why this is affordable
//!
//! In-degree changes only when a corpus file changes, so the replay is sized by file
//! revisions and not by commits × nodes. A repository at 695 commits holds **582** corpus
//! file-revisions over its whole history, and `git log --raw` names the post-image blob of
//! each in one pass. Those blobs are read through a single `git cat-file --batch`, so the
//! whole replay is two subprocesses regardless of history length.
//!
//! It is nonetheless not free, and `orphan-in` on a healthy corpus has nothing to explain —
//! so [`super::run_checks`] calls this only when there are orphans to date.

use std::collections::{HashMap, HashSet};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Command, Stdio};

use super::checks::normalize;

/// One corpus file changing in one commit.
struct Change {
    /// `A`, `M`, `D`, or `R…`. Only the first byte is read.
    status: u8,
    /// Post-image blob. All zeroes for a deletion, which is never read.
    blob: String,
    path: String,
}

/// Whether a repo-relative path is a corpus *instance* — `.yidam/corpus/<class>/<name>.yml`.
///
/// Class definitions sit at depth 1 and end `.ont.yml`; instances sit at depth 2. Anything
/// else under the corpus (a README, an ACTIONS.md) is not a node and neither points nor is
/// pointed at.
fn is_instance(path: &str) -> bool {
    let Some(rest) = path.strip_prefix(".yidam/corpus/") else {
        return false;
    };
    rest.ends_with(".yml") && !rest.ends_with(".ont.yml") && rest.matches('/').count() == 1
}

/// The link targets a node declares, as repo-relative normalized paths.
///
/// Targets are written relative to the file that declares them, which is what makes
/// `../class/x.yml` and `class/x.yml` the same edge. Resolved here exactly as
/// [`super::checks::orphan_in`] resolves them, so the replay and the check cannot come to
/// disagree about what points at what.
fn targets_of(path: &str, content: &str) -> HashSet<String> {
    let inst: crate::parse::CorpusInstance = match serde_yaml::from_str(content) {
        Ok(i) => i,
        // A revision that does not parse contributes no edges. It was committed and the
        // corpus survived it; refusing to replay the history because one blob is malformed
        // would lose every date after it.
        Err(_) => return HashSet::new(),
    };
    let dir = Path::new(path).parent().unwrap_or(Path::new(""));
    inst.links
        .unwrap_or_default()
        .iter()
        .filter_map(|l| l.target.as_ref())
        .map(|t| normalize(&dir.join(t)).to_string_lossy().replace('\\', "/"))
        .collect()
}

/// One commit's worth of corpus change.
struct CommitChanges {
    sha: String,
    ts: i64,
    changes: Vec<Change>,
}

/// `git log --raw` over the corpus, oldest first.
fn change_stream(root: &Path) -> Vec<CommitChanges> {
    let out = Command::new("git")
        .current_dir(root)
        .args([
            "log",
            "--reverse",
            "--raw",
            "--no-abbrev",
            "--no-renames",
            "--format=C %H %at",
            "--",
            ".yidam/corpus",
        ])
        .output();
    let Ok(out) = out else { return Vec::new() };
    let Ok(text) = String::from_utf8(out.stdout) else {
        return Vec::new();
    };

    let mut commits: Vec<CommitChanges> = Vec::new();
    for line in text.lines() {
        if let Some(head) = line.strip_prefix("C ") {
            let mut f = head.split_whitespace();
            let sha = f.next().unwrap_or_default().to_string();
            let ts = f.next().and_then(|s| s.parse().ok()).unwrap_or(0);
            commits.push(CommitChanges {
                sha,
                ts,
                changes: Vec::new(),
            });
        } else if let Some(rest) = line.strip_prefix(':') {
            // :<srcmode> <dstmode> <srcsha> <dstsha> <status>\t<path>
            let Some((meta, path)) = rest.split_once('\t') else {
                continue;
            };
            let f: Vec<&str> = meta.split_whitespace().collect();
            if f.len() < 5 {
                continue;
            }
            if let Some(c) = commits.last_mut() {
                c.changes.push(Change {
                    status: f[4].as_bytes()[0],
                    blob: f[3].to_string(),
                    path: path.to_string(),
                });
            }
        }
    }
    commits
}

/// Read many blobs in one `git cat-file --batch`.
///
/// One subprocess for the whole history. Feeding these one at a time is the difference
/// between a replay that costs milliseconds and one that costs a subprocess per revision.
fn read_blobs(root: &Path, shas: &[String]) -> HashMap<String, String> {
    let mut found = HashMap::new();
    if shas.is_empty() {
        return found;
    }
    let Ok(mut child) = Command::new("git")
        .current_dir(root)
        .args(["cat-file", "--batch"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    else {
        return found;
    };

    let mut stdin = child.stdin.take().expect("piped");
    let requested: Vec<String> = shas.to_vec();
    let writer = std::thread::spawn(move || {
        for s in &requested {
            if writeln!(stdin, "{s}").is_err() {
                return;
            }
        }
    });

    let mut reader = BufReader::new(child.stdout.take().expect("piped"));
    for sha in shas {
        // `<sha> <type> <size>\n<size bytes>\n`, or `<sha> missing\n`.
        let mut header = String::new();
        if reader.read_line(&mut header).unwrap_or(0) == 0 {
            break;
        }
        let parts: Vec<&str> = header.split_whitespace().collect();
        let Some(size) = parts.get(2).and_then(|s| s.parse::<usize>().ok()) else {
            continue;
        };
        let mut buf = vec![0u8; size + 1];
        if std::io::Read::read_exact(&mut reader, &mut buf).is_err() {
            break;
        }
        buf.pop();
        if let Ok(text) = String::from_utf8(buf) {
            found.insert(sha.clone(), text);
        }
    }
    let _ = writer.join();
    let _ = child.wait();
    found
}

/// Whether a repo-relative path is a class definition — `.yidam/corpus/<class>.ont.yml`.
fn is_class(path: &str) -> bool {
    path.strip_prefix(".yidam/corpus/")
        .is_some_and(|r| r.ends_with(".ont.yml") && !r.contains('/'))
}

/// The class a node belongs to, from its path.
pub(crate) fn class_of(path: &str) -> &str {
    path.strip_prefix(".yidam/corpus/")
        .and_then(|r| r.split('/').next())
        .unwrap_or_default()
}

/// Whether a class definition declares edges and declares none of them inbound.
///
/// The same rule [`super::checks::Class::is_source_class`] applies, read from a blob rather
/// than from disk. Silence is not a declaration: a class with no `edges:` list has said
/// nothing about its shape and is not exempt.
fn blob_is_source_class(content: &str) -> bool {
    #[derive(Default, serde::Deserialize)]
    struct Edge {
        #[serde(default)]
        direction: Option<String>,
    }
    #[derive(Default, serde::Deserialize)]
    struct Fields {
        #[serde(default)]
        edges: Vec<Edge>,
    }
    let f: Fields = serde_yaml::from_str(content).unwrap_or_default();
    !f.edges.is_empty() && !f.edges.iter().any(|e| e.direction.as_deref() == Some("in"))
}

/// The corpus as it stood at one commit.
pub(crate) struct Frame<'a> {
    pub sha: &'a str,
    pub ts: i64,
    /// Every instance node present, and the targets it points at.
    pub out: &'a HashMap<String, HashSet<String>>,
    /// Every node something points at.
    pub cited: HashSet<&'a String>,
    /// Classes that declare no inbound edge, whose instances are orphans by design.
    pub source_classes: &'a HashSet<String>,
}

impl Frame<'_> {
    /// Nodes nothing points at, excluding those the ontology says nothing should.
    pub fn orphans(&self) -> impl Iterator<Item = &String> {
        self.out
            .keys()
            .filter(move |n| !self.cited.contains(*n) && !self.source_classes.contains(class_of(n)))
    }
}

/// Rebuild the corpus forward through history, calling `frame` once per commit that touched
/// it.
///
/// The single walk. Both consumers here — the dates `orphan-in` reports and the series
/// `yidam replay` prints — are folds over it, so they cannot come to disagree about what the
/// graph looked like on a given day.
pub(crate) fn replay(root: &Path, mut frame: impl FnMut(Frame<'_>)) {
    let commits = change_stream(root);
    if commits.is_empty() {
        return;
    }

    let wanted: Vec<String> = {
        let mut seen = HashSet::new();
        commits
            .iter()
            .flat_map(|c| c.changes.iter())
            .filter(|c| c.status != b'D' && (is_instance(&c.path) || is_class(&c.path)))
            .map(|c| c.blob.clone())
            .filter(|b| seen.insert(b.clone()))
            .collect()
    };
    let blobs = read_blobs(root, &wanted);

    // Live corpus state, rebuilt forward: each node's outbound targets.
    let mut out: HashMap<String, HashSet<String>> = HashMap::new();
    let mut source_classes: HashSet<String> = HashSet::new();

    for c in &commits {
        let mut touched = false;
        for ch in &c.changes {
            let content = || blobs.get(&ch.blob).map(String::as_str).unwrap_or("");
            if is_instance(&ch.path) {
                touched = true;
                if ch.status == b'D' {
                    out.remove(&ch.path);
                } else {
                    out.insert(ch.path.clone(), targets_of(&ch.path, content()));
                }
            } else if is_class(&ch.path) {
                touched = true;
                let name = class_of(&ch.path).trim_end_matches(".ont.yml").to_string();
                if ch.status == b'D' || !blob_is_source_class(content()) {
                    source_classes.remove(&name);
                } else {
                    source_classes.insert(name);
                }
            }
        }
        if !touched {
            continue;
        }
        let cited: HashSet<&String> = out.values().flatten().collect();
        frame(Frame {
            sha: &c.sha,
            ts: c.ts,
            out: &out,
            cited,
            source_classes: &source_classes,
        });
    }
}

/// Every commit that touched the corpus, oldest first.
///
/// The clock a baseline entry ages against, and deliberately the same clock
/// [`uncited_age`] counts in: a commit that changed nothing under `.yidam/corpus` did not
/// decline to pay down corpus debt, because it was not looking at the corpus. Two different
/// counting rules for two ages a reader sees side by side would be a small cruelty.
///
/// One subprocess, and the same `git log` the replay already runs — this is the shas of
/// that walk without the blobs.
pub fn corpus_commits(root: &Path) -> Vec<String> {
    change_stream(root).into_iter().map(|c| c.sha).collect()
}

/// How many corpus-touching commits have landed since `sha`, counting the one at HEAD.
///
/// `None` when the sha is not in the corpus history at all — a baseline written before a
/// rebase, or hand-edited. An entry whose clock cannot be read is not expired; it is
/// unreadable, and treating unreadable as expired would fail a build over a rewritten
/// history rather than over anything the corpus did.
pub fn commits_since(commits: &[String], sha: &str) -> Option<usize> {
    let at = commits.iter().position(|c| c == sha)?;
    Some(commits.len() - at)
}

/// How long a corpus-state condition has held, for one node.
///
/// **Commits, not days.** [`super::orphan_in_dated`] reports a *date* and deliberately not
/// an age, because an age in days is a function of when you ask: the same corpus renders
/// differently tomorrow and no golden can pin it. A count of commits has neither problem —
/// it is a function of HEAD, so it is reproducible from the repository alone, and it is the
/// unit the measurement document argues in ("a node uncited for five commits is a sweep in
/// progress; one uncited for two hundred is over-collection").
///
/// **Corpus-touching commits.** The replay visits only commits that changed
/// `.yidam/corpus`, and those are the only commits that could have changed whether a node
/// is cited. A commit that touched nothing here is not evidence that the condition
/// survived scrutiny, so counting it would inflate every age in a repository that also
/// holds code — which is every repository bootstrapped in existing-repo mode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Age {
    /// The commit at which the condition first held.
    pub sha: String,
    /// That commit's timestamp. The date half of the finding, unchanged.
    pub ts: i64,
    /// Corpus-touching commits it has held for, counting the one at HEAD.
    pub commits: usize,
}

/// For every node present at HEAD, how long nothing has pointed at it — absent when
/// something points at it now.
///
/// A node that has never been cited dates from the commit that added it. A node cited and
/// later orphaned dates from the commit that removed the last edge into it, which is the
/// distinction node age cannot draw.
pub fn uncited_age(root: &Path) -> HashMap<String, Age> {
    // Value is (sha, ts, index of the frame at which it went uncited). The index becomes a
    // count once the walk is over and the total is known; it cannot be a count while the
    // walk is running, because the walk does not know how many frames are left.
    let mut since: HashMap<String, (String, i64, usize)> = HashMap::new();
    let mut frames = 0usize;
    replay(root, |f| {
        for node in f.out.keys() {
            if f.cited.contains(node) {
                // Something points at it as of this commit; any earlier orphaning ended.
                since.remove(node);
            } else {
                // Uncited now. Keep the earliest commit at which that became true.
                since
                    .entry(node.clone())
                    .or_insert((f.sha.to_string(), f.ts, frames));
            }
        }
        since.retain(|n, _| f.out.contains_key(n));
        frames += 1;
    });
    since
        .into_iter()
        .map(|(node, (sha, ts, first))| {
            (
                node,
                Age {
                    sha,
                    ts,
                    // HEAD inclusive: a condition that first held at the last frame has
                    // held for one commit, not zero.
                    commits: frames - first,
                },
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_depth_two_yml_under_corpus_is_an_instance() {
        assert!(is_instance(".yidam/corpus/person/mick.yml"));
        assert!(!is_instance(".yidam/corpus/person.ont.yml"));
        assert!(!is_instance(".yidam/corpus/README.md"));
        assert!(!is_instance(".yidam/corpus/person/ACTIONS.md"));
        // Deeper than the model allows, so not a node.
        assert!(!is_instance(".yidam/corpus/person/sub/x.yml"));
        assert!(!is_instance("docs/person/x.yml"));
    }

    /// Targets resolve against the declaring file's directory, so `../band/x.yml` from
    /// `person/` and `band/x.yml` name one edge. Divergence here would make the replay
    /// disagree with the check it explains.
    #[test]
    fn targets_resolve_relative_to_the_declaring_node() {
        let t = targets_of(
            ".yidam/corpus/person/mick.yml",
            "class: person\nlinks:\n  - target: ../band/napalm.yml\n  - target: ../person.ont.yml\n",
        );
        assert!(t.contains(".yidam/corpus/band/napalm.yml"), "{t:?}");
        assert!(t.contains(".yidam/corpus/person.ont.yml"), "{t:?}");
    }

    #[test]
    fn an_unparseable_revision_contributes_no_edges() {
        assert!(targets_of(".yidam/corpus/a/b.yml", "\tnot: [valid").is_empty());
    }

    // ── the replay ────────────────────────────────────────────────────────────

    fn git(dir: &Path, args: &[&str]) {
        let ok = Command::new("git")
            .current_dir(dir)
            .args(args)
            .status()
            .unwrap()
            .success();
        assert!(ok, "git {args:?} failed");
    }

    /// Commit at a fixed date so the assertions are about the replay and not the clock.
    fn commit(dir: &Path, day: &str, msg: &str) {
        git(dir, &["add", "-A"]);
        let stamp = format!("{day}T00:00:00Z");
        let ok = Command::new("git")
            .current_dir(dir)
            .args(["commit", "-q", "-m", msg])
            .env("GIT_AUTHOR_DATE", &stamp)
            .env("GIT_COMMITTER_DATE", &stamp)
            .status()
            .unwrap()
            .success();
        assert!(ok, "commit failed");
    }

    fn node(dir: &Path, rel: &str, body: &str) {
        let p = dir.join(".yidam/corpus").join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, body).unwrap();
    }

    fn init(dir: &Path) {
        git(dir, &["init", "-q", "-b", "main"]);
        git(dir, &["config", "user.email", "t@t.com"]);
        git(dir, &["config", "user.name", "T"]);
    }

    fn day(ts: i64) -> String {
        crate::cmd::export::unix_to_iso(ts as u64)
            .split('T')
            .next()
            .unwrap()
            .to_string()
    }

    /// The whole reason this replays the graph instead of reading each node's age.
    ///
    /// `a` is cited when it is authored and loses its only citation four days later. Node
    /// age would date it from the day it was written and call it four days more neglected
    /// than it is; what a reader wants is the day it stopped being pointed at.
    #[test]
    fn a_node_cited_and_later_orphaned_dates_from_the_orphaning() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path();
        init(root);

        node(root, "concept/a.yml", "class: concept\nlinks: []\n");
        node(
            root,
            "concept/b.yml",
            "class: concept\nlinks:\n  - target: ../concept/a.yml\n",
        );
        commit(root, "2026-01-01", "establish: a, cited by b");

        // b stops pointing at a. Nothing else changes, and a is untouched.
        node(root, "concept/b.yml", "class: concept\nlinks: []\n");
        commit(root, "2026-01-05", "revise: b no longer cites a");

        let since = uncited_age(root);
        assert_eq!(
            since.get(".yidam/corpus/concept/a.yml").map(|a| day(a.ts)),
            Some("2026-01-05".to_string()),
            "dates from the orphaning, not from authorship: {since:?}"
        );
    }

    /// A node nothing ever pointed at dates from the commit that added it — the case where
    /// the replay and node age agree, which on the corpora available today is every case.
    #[test]
    fn a_node_never_cited_dates_from_its_authorship() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path();
        init(root);

        node(root, "concept/a.yml", "class: concept\nlinks: []\n");
        commit(root, "2026-01-01", "establish: a");
        node(root, "concept/b.yml", "class: concept\nlinks: []\n");
        commit(root, "2026-01-09", "establish: b");

        let since = uncited_age(root);
        assert_eq!(
            since.get(".yidam/corpus/concept/a.yml").map(|a| day(a.ts)),
            Some("2026-01-01".to_string())
        );
        assert_eq!(
            since.get(".yidam/corpus/concept/b.yml").map(|a| day(a.ts)),
            Some("2026-01-09".to_string())
        );
    }

    /// A citation that comes back clears the clock. Otherwise a node orphaned briefly and
    /// then wired up would keep reporting the old date forever.
    #[test]
    fn a_restored_citation_clears_the_date() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path();
        init(root);

        node(root, "concept/a.yml", "class: concept\nlinks: []\n");
        node(root, "concept/b.yml", "class: concept\nlinks: []\n");
        commit(root, "2026-01-01", "establish: a and b, neither citing");

        node(
            root,
            "concept/b.yml",
            "class: concept\nlinks:\n  - target: ../concept/a.yml\n",
        );
        commit(root, "2026-01-06", "revise: b cites a");

        let since = uncited_age(root);
        assert!(
            !since.contains_key(".yidam/corpus/concept/a.yml"),
            "a is cited now and must carry no date: {since:?}"
        );
        assert!(
            since.contains_key(".yidam/corpus/concept/b.yml"),
            "b still has nothing pointing at it"
        );
    }

    /// A deleted node leaves no date behind. It is not an orphan; it is gone.
    #[test]
    fn a_deleted_node_is_forgotten() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path();
        init(root);

        node(root, "concept/a.yml", "class: concept\nlinks: []\n");
        commit(root, "2026-01-01", "establish: a");
        std::fs::remove_file(root.join(".yidam/corpus/concept/a.yml")).unwrap();
        node(root, "concept/b.yml", "class: concept\nlinks: []\n");
        commit(root, "2026-01-03", "withdraw: a; establish: b");

        let since = uncited_age(root);
        assert!(
            !since.contains_key(".yidam/corpus/concept/a.yml"),
            "{since:?}"
        );
        assert!(since.contains_key(".yidam/corpus/concept/b.yml"));
    }

    // ── residence time ────────────────────────────────────────────────────────

    /// The count is what the date cannot say. Three nodes orphaned on the same day, at
    /// different points in the history, are indistinguishable by date and ordered by this.
    #[test]
    fn the_commit_count_separates_findings_the_date_cannot() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path();
        init(root);

        node(root, "concept/hub.yml", "class: concept\nlinks: []\n");
        node(root, "concept/old.yml", "class: concept\nlinks: []\n");
        commit(root, "2026-01-01", "establish: hub and old");
        for i in 1..=3 {
            node(
                root,
                &format!("concept/f{i}.yml"),
                "class: concept\nlinks: []\n",
            );
            commit(root, "2026-01-01", &format!("scope: filler {i}"));
        }

        let ages = uncited_age(root);
        let commits = |n: &str| {
            ages.get(&format!(".yidam/corpus/concept/{n}.yml"))
                .unwrap()
                .commits
        };

        // Every one of them is uncited "since 2026-01-01" — the date is the same for all
        // five and orders none of them.
        assert_eq!(
            ages.values()
                .map(|a| day(a.ts))
                .collect::<std::collections::HashSet<_>>(),
            std::collections::HashSet::from(["2026-01-01".to_string()])
        );
        // Four frames: the genesis and three fillers.
        assert_eq!(commits("old"), 4, "present since the first frame");
        assert_eq!(commits("f1"), 3);
        assert_eq!(commits("f3"), 1, "arrived at HEAD — one commit, not zero");
    }

    /// A commit that did not touch the corpus is not evidence that a finding survived
    /// anything, and counting it would inflate every age in a repository that also holds
    /// code — which is every repository bootstrapped in existing-repo mode.
    #[test]
    fn commits_that_miss_the_corpus_do_not_age_a_finding() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path();
        init(root);

        node(root, "concept/a.yml", "class: concept\nlinks: []\n");
        commit(root, "2026-01-01", "establish: a");
        for i in 1..=5 {
            std::fs::write(root.join(format!("src{i}.rs")), "fn main() {}\n").unwrap();
            commit(root, "2026-01-02", &format!("build: unrelated {i}"));
        }

        let ages = uncited_age(root);
        assert_eq!(
            ages[".yidam/corpus/concept/a.yml"].commits, 1,
            "five commits landed and none of them could have cited anything"
        );
    }

    /// The clock restarts, rather than pausing, when a citation comes back and goes away
    /// again. Otherwise a node briefly wired up would keep reporting its original age.
    #[test]
    fn a_restored_and_relost_citation_restarts_the_count() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path();
        init(root);

        node(root, "concept/a.yml", "class: concept\nlinks: []\n");
        node(root, "concept/b.yml", "class: concept\nlinks: []\n");
        commit(root, "2026-01-01", "establish: a and b");
        commit_touching(root, "2026-01-02", "scope: a widening pass");

        // b cites a, then stops.
        node(
            root,
            "concept/b.yml",
            "class: concept\nlinks:\n  - target: ../concept/a.yml\n",
        );
        commit(root, "2026-01-03", "revise: b cites a");
        node(root, "concept/b.yml", "class: concept\nlinks: []\n");
        commit(root, "2026-01-04", "revise: b no longer cites a");

        let ages = uncited_age(root);
        let a = &ages[".yidam/corpus/concept/a.yml"];
        assert_eq!(day(a.ts), "2026-01-04", "dates from the second orphaning");
        assert_eq!(a.commits, 1, "and counts from it too, not from the first");
    }

    /// A touch that changes nothing about citation still advances the count: the corpus was
    /// worked on and this node was not linked.
    fn commit_touching(dir: &Path, day: &str, msg: &str) {
        node(
            dir,
            "concept/filler.yml",
            &format!("class: concept\nlabel: {msg}\nlinks: []\n"),
        );
        commit(dir, day, msg);
    }

    /// The sha is not decoration: it is the commit a reader runs `git show` on to see what
    /// removed the last edge. Asserted against `git rev-parse` rather than pinned in a
    /// golden, where forty opaque characters would match any other forty.
    #[test]
    fn the_dated_commit_is_the_one_that_orphaned_the_node() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path();
        init(root);

        node(root, "concept/a.yml", "class: concept\nlinks: []\n");
        node(
            root,
            "concept/b.yml",
            "class: concept\nlinks:\n  - target: ../concept/a.yml\n",
        );
        commit(root, "2026-01-01", "establish: a, cited by b");

        node(root, "concept/b.yml", "class: concept\nlinks: []\n");
        commit(root, "2026-01-05", "revise: b no longer cites a");
        let orphaning = rev_parse(root, "HEAD");

        // A later commit that touches the corpus without changing what points at `a`.
        node(root, "concept/c.yml", "class: concept\nlinks: []\n");
        commit(root, "2026-01-07", "establish: c");

        let ages = uncited_age(root);
        let a = &ages[".yidam/corpus/concept/a.yml"];
        assert_eq!(a.sha, orphaning, "the commit that severed the last edge");
        assert_ne!(
            a.sha,
            rev_parse(root, "HEAD"),
            "not simply the newest commit"
        );
        assert_eq!(a.commits, 2, "the orphaning and the commit after it");
    }

    fn rev_parse(dir: &Path, rev: &str) -> String {
        let out = Command::new("git")
            .current_dir(dir)
            .args(["rev-parse", rev])
            .output()
            .unwrap();
        String::from_utf8(out.stdout).unwrap().trim().to_string()
    }
}
