//! Article V's node and edge clauses, decided against the tips a resolution says it read.
//!
//! > A resolution may not introduce nodes, edges, or claims that were not held by at least
//! > one elector.
//!
//! Three objects, and only two of them can be checked. The commentary beside the article
//! says why: *a node and an edge carry an identity of their own — a node id, and a subject,
//! verb and object — so "held by at least one elector" is set membership across the
//! participating tips, and a check may settle it. A claim carries no such identity.* So this
//! module builds the node and edge halves and deliberately not the claim half; the claim
//! clause stays with the synthesizer under Article II, which is where the constitution
//! leaves it.
//!
//! # What the measurement said
//!
//! Against the one derived repository that has run a sangha — 29 resolutions, 70 recorded
//! tips, read-only:
//!
//! - **70 of 70** `ma/<elector>@<hash>` references resolve as commits, so the record's own
//!   provenance is sound.
//! - **4** instance files were added by a resolution across the whole history, and **4 of 4**
//!   were held at no tip. That is 4 findings across 3 records: one commit wrote two of the
//!   records, and both of them name the same seated node.
//! - **13** edges were introduced, and **13 of 13** are incident to one of those 4 nodes.
//!   Not one edge was introduced between two nodes that already existed.
//! - **0 renames**, and **0** of the introduced nodes are named under `What remains open`.
//!
//! Two design decisions follow from that shape rather than from taste. Edges incident to an
//! introduced node are **not** reported: they are the same finding said 13 ways, and the node
//! is where a reader would go. And the arm that survives — an edge between two nodes that
//! were already there, which is the purest form of *a resolution asserting a relation nobody
//! argued for* — has never fired in a real corpus, which is exactly why it needs a test that
//! makes it.
//!
//! # What the clone can answer, which is not always the question
//!
//! A `ma/*` tip is not necessarily an ancestor of the baseline: transport copies a file, and
//! `adopt` merges the rigpa into the elector's branch rather than the other way round. **15
//! of those 70 tips are not ancestors of HEAD** and resolve only because the `ma/*` branches
//! are in the clone. So the same repository answers differently depending on how it was
//! fetched, and the four shapes were measured rather than reasoned about:
//!
//! | clone | scope-unheld | unverifiable |
//! |---|---:|---:|
//! | full, all refs | **4** | 0 |
//! | `--single-branch`, full history | **4** | 6 |
//! | `--depth 1` | 0 | 29 |
//! | `--depth 5 --no-single-branch` | 0 | 29 |
//!
//! The second row is the one that decides whether this is worth having: an ordinary
//! single-branch CI checkout still finds every one of the four violations, and says out loud
//! which six resolutions it could not judge. The gate is not decorative where it runs.
//!
//! Two rules follow. **A resolution is judged on the whole set of its tips or not at all** —
//! reporting the node clause against the tips that happened to resolve would manufacture an
//! error at the one severity that gates, on evidence known to be partial. And **a resolution
//! that was not judged is reported**, by [`unverifiable`] at [`Severity::Warn`], because a
//! gate that quietly checks nothing is indistinguishable from one that found nothing wrong.
//!
//! The last two rows are also a trap that had to be disarmed. A shallow clone's boundary
//! commit has no parents, so git reports every path in it as *added* — a boundary carrying a
//! resolvable tip would have read as one resolution seating the entire corpus, and the
//! `--depth 5` row is exactly that case: all 104 refs present, every tip resolvable. Records
//! attributed to a boundary commit are dropped and reported as unverifiable instead.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::Path;
use std::process::Command;

use super::checks::normalize;
use super::history::{is_instance, read_blobs};
use super::model::{Check, Severity, Violation};
use crate::cmd::sangha::Resolution;

const CORPUS: &str = ".yidam/corpus";
const RESOLUTIONS: &str = ".yidam/sangha/resolutions";
const POSITIONS: &str = ".yidam/sangha/positions";

/// One node or edge a resolution's commit put into the corpus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Introduced {
    /// `<class>/<name>` — the node itself, or the subject of the edge.
    pub node: String,
    /// `(relationship, target node id)` for an edge; `None` when this is the node.
    pub edge: Option<(String, String)>,
    /// Whether at least one participating tip held it.
    pub held: bool,
}

/// What one resolution introduced, and what could be read in order to judge it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct ScopeAudit {
    /// Repo-relative path of the resolution record.
    pub file: String,
    /// How many tips the record names.
    pub tips: usize,
    /// The ones naming no commit in this clone, verbatim.
    pub unreadable_tips: Vec<String>,
    /// The record is committed and no commit in this clone can be said to have added it.
    ///
    /// A shallow checkout is the case, and it is not exotic: `actions/checkout` fetches one
    /// commit unless a workflow says otherwise. Git *will* name the boundary commit as the
    /// one that added the record — a commit with no parents reads as adding everything in it
    /// — and that attribution is worse than none, so [`audit`] discards it and this is what
    /// is left. Reporting a clean gate instead is the decorative outcome the whole check
    /// exists to avoid.
    pub history_truncated: bool,
    pub introduced: Vec<Introduced>,
    /// The record's `## What remains open` section, verbatim.
    pub remains_open: String,
}

impl ScopeAudit {
    /// Why the scope clauses were not decided for this resolution, if they were not.
    ///
    /// Three ways to be undecidable and one to be decided, and the difference matters at the
    /// severity that gates: partial evidence is not evidence. A record naming no tips is
    /// included deliberately — everything such a resolution introduces is trivially held by
    /// nobody, and reporting that as a scope violation would blame the synthesis for a
    /// malformed record.
    fn unverifiable(&self) -> Option<String> {
        if self.history_truncated {
            return Some(
                "the commit that added this record is not in this clone — a shallow checkout \
                 cannot see what a resolution changed"
                    .to_string(),
            );
        }
        if self.tips == 0 {
            return Some(
                "the record names no `tips:`, so there is nothing to check it against".to_string(),
            );
        }
        if !self.unreadable_tips.is_empty() {
            return Some(format!(
                "{} of {} tip(s) name no commit here — {}",
                self.unreadable_tips.len(),
                self.tips,
                self.unreadable_tips.join(", ")
            ));
        }
        None
    }
}

// ── the checks ────────────────────────────────────────────────────────────────

/// Whether the record's own `What remains open` names this node.
///
/// The one exception the constitution licenses is an open-question node standing for a
/// tension that could not be resolved, and it is held by no elector by construction. Nothing
/// in the corpus model *marks* such a node — there is no class for it and no field — so the
/// only thing that can license it is the document that declares the tension unresolved.
/// Tying the exception to the record is not a workaround for a missing marker: it is the
/// correct place, because what makes the node legal is the resolution saying the question is
/// still open.
///
/// Matched on the node id and on the bare stem, generously and on purpose. This can only ever
/// withdraw an error, and at the severity that gates, a finding suppressed by a record that
/// mentions the node is cheaper than one raised against a record that meant to except it.
fn excepted(remains_open: &str, node: &str) -> bool {
    let stem = node.rsplit('/').next().unwrap_or(node);
    remains_open.contains(node) || remains_open.contains(stem)
}

pub(crate) fn unheld(audits: &[ScopeAudit]) -> Check {
    let mut violations = Vec::new();
    for a in audits {
        // Judged on the whole set of tips or not at all — see the module docs.
        if a.unverifiable().is_some() {
            continue;
        }
        let seated: HashSet<&str> = a
            .introduced
            .iter()
            .filter(|i| i.edge.is_none())
            .map(|i| i.node.as_str())
            .collect();
        for i in a.introduced.iter().filter(|i| !i.held) {
            match &i.edge {
                None => {
                    if excepted(&a.remains_open, &i.node) {
                        continue;
                    }
                    let incident = a
                        .introduced
                        .iter()
                        .filter(|e| !e.held)
                        .filter_map(|e| e.edge.as_ref().map(|(_, t)| (&e.node, t)))
                        .filter(|(s, t)| *s == &i.node || t.as_str() == i.node)
                        .count();
                    let edges = match incident {
                        0 => String::new(),
                        n => format!(", and {n} edge(s) incident to it"),
                    };
                    violations.push(Violation::new(
                        a.file.clone(),
                        format!(
                            "seats `{}`{edges} — no tip this record names holds that node, \
                             and no position at one names it",
                            i.node
                        ),
                    ));
                }
                Some((relationship, target)) => {
                    // Every edge touching a node this same resolution seated is a
                    // consequence of seating it, and the node is the finding.
                    if seated.contains(i.node.as_str()) || seated.contains(target.as_str()) {
                        continue;
                    }
                    violations.push(Violation::new(
                        a.file.clone(),
                        format!(
                            "asserts `{} -[{relationship}]-> {target}` between two nodes that \
                             already existed — no tip this record names holds that edge",
                            i.node
                        ),
                    ));
                }
            }
        }
    }
    Check::new(
        "resolution-scope-unheld",
        "Resolution seats a node or edge no elector held",
        Severity::Error,
        "Article V confines a resolution to what the electors brought to it: it may not \
         introduce nodes, edges or claims that were not held by at least one of them. A node \
         and an edge carry an identity — a node id, and a subject, verb and object — so this \
         is set membership across the tips the record itself names, and there is no state of \
         the world in which the answer is arguable. That is why it gates where the claim \
         clause cannot: a claim has no identity to compare, and the constitution leaves that \
         judgement with the synthesizer rather than delegating it to a checker. The rule this \
         most often catches is the one the commentary states: a class is not its instances, \
         so a resolution that seats the first instances of a class alongside the class it \
         adopts is introducing nodes no elector held.",
        violations,
    )
}

pub(crate) fn unverifiable(audits: &[ScopeAudit]) -> Check {
    let violations = audits
        .iter()
        .filter_map(|a| {
            let why = a.unverifiable()?;
            Some(Violation::new(
                a.file.clone(),
                format!("{why} — this resolution's scope was not checked"),
            ))
        })
        .collect();
    Check::new(
        "resolution-scope-unverifiable",
        "Resolution's scope fidelity could not be decided here",
        Severity::Warn,
        "Whether Article V's node and edge clauses can be decided depends on what the clone \
         holds. A `ma/*` tip need not be an ancestor of the baseline — transport copies a \
         file and `adopt` merges the rigpa into the elector's branch, not the reverse — so 15 \
         of the 70 tips measured are reachable only through the `ma/*` branches themselves, \
         which a single-branch checkout does not fetch; and a depth-1 checkout cannot see \
         what a resolution's own commit changed. That makes this a Warn: the record may be \
         sound and the clone merely thin. What it must not do is pass silently. A gate that \
         judges nothing while reporting nothing is indistinguishable from a gate that found \
         nothing wrong, and this check is the difference.",
        violations,
    )
}

// ── reading the repository ────────────────────────────────────────────────────

/// `.yidam/corpus/person/mick.yml` → `person/mick`.
fn node_id(path: &str) -> String {
    let rest = path.strip_prefix(CORPUS).unwrap_or(path);
    let rest = rest.strip_prefix('/').unwrap_or(rest);
    rest.strip_suffix(".yml").unwrap_or(rest).to_string()
}

/// The edges a node declares, as `(relationship, target node id)`.
///
/// Targets resolve against the declaring file's directory, exactly as the replay and the
/// orphan checks resolve them, so an edge written `../class/x.yml` and one written
/// `class/x.yml` are one edge here too.
fn links_of(path: &str, text: &str) -> BTreeSet<(String, String)> {
    let inst: crate::parse::CorpusInstance = match serde_yaml::from_str(text) {
        Ok(i) => i,
        // A revision that does not parse declares no edges. It is not this check's job to
        // report malformed YAML, and refusing to audit the resolution because one blob is
        // unreadable would lose the node clause along with the edge clause.
        Err(_) => return BTreeSet::new(),
    };
    let dir = Path::new(path).parent().unwrap_or(Path::new(""));
    inst.links
        .unwrap_or_default()
        .iter()
        .filter_map(|l| {
            let target = l.target.as_ref()?;
            let resolved = normalize(&dir.join(target))
                .to_string_lossy()
                .replace('\\', "/");
            Some((
                l.relationship.clone().unwrap_or_else(|| "link".to_string()),
                node_id(&resolved),
            ))
        })
        .collect()
}

/// The `## What remains open` section of a resolution record, verbatim.
fn remains_open(text: &str) -> String {
    let Some(at) = text.find("\n## What remains open") else {
        return String::new();
    };
    let rest = &text[at + 1..];
    let body = rest.split_once('\n').map(|(_, b)| b).unwrap_or("");
    match body.find("\n## ") {
        Some(end) => body[..end].to_string(),
        None => body.to_string(),
    }
}

fn git(root: &Path, args: &[&str]) -> String {
    Command::new("git")
        .current_dir(root)
        .args(args)
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .unwrap_or_default()
}

/// Feed `input` to `git <args>` and read stdout.
fn git_stdin(root: &Path, args: &[&str], input: &str) -> String {
    use std::io::Write;
    use std::process::Stdio;
    let Ok(mut child) = Command::new("git")
        .current_dir(root)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    else {
        return String::new();
    };
    // Written from a thread: a batch large enough to fill the pipe buffer would otherwise
    // deadlock against a child that is blocked writing its own answer.
    let stdin = child.stdin.take();
    let owned = input.to_string();
    let writer = std::thread::spawn(move || {
        if let Some(mut stdin) = stdin {
            let _ = stdin.write_all(owned.as_bytes());
        }
    });
    let out = child.wait_with_output().ok();
    let _ = writer.join();
    out.and_then(|o| String::from_utf8(o.stdout).ok())
        .unwrap_or_default()
}

/// The commit that first added each path under `.yidam/sangha/resolutions`.
///
/// The record, not the branch. `rigpa/<evolution>` is the obvious handle and it is the wrong
/// one: of the 29 resolutions measured, the branch tip is the record-adding commit for only
/// 23, has moved past it for 4, and 2 resolutions have no `rigpa/*` branch left at all. The
/// verb is no better a handle — the adding commit says `synthesize:` 25 times and `resolve:`
/// four.
///
/// The *first* add wins. A record deleted and restored is one resolution that happened once.
fn adding_commits(root: &Path) -> HashMap<String, String> {
    let out = git(
        root,
        &[
            "log",
            "--reverse",
            "--raw",
            "--no-abbrev",
            "--no-renames",
            "--format=C %H",
            "--",
            RESOLUTIONS,
        ],
    );
    let mut sha = String::new();
    let mut adds: HashMap<String, String> = HashMap::new();
    for line in out.lines() {
        if let Some(h) = line.strip_prefix("C ") {
            sha = h.trim().to_string();
        } else if let Some(rest) = line.strip_prefix(':') {
            let Some((meta, path)) = rest.split_once('\t') else {
                continue;
            };
            let f: Vec<&str> = meta.split_whitespace().collect();
            if f.len() >= 5 && f[4].starts_with('A') {
                adds.entry(path.to_string()).or_insert_with(|| sha.clone());
            }
        }
    }
    adds
}

/// Which of `shas` have no parent — a real genesis, or a shallow clone's boundary.
fn roots_among(root: &Path, shas: &BTreeSet<String>) -> HashSet<String> {
    if shas.is_empty() {
        return HashSet::new();
    }
    let mut args: Vec<&str> = vec!["rev-list", "--no-walk", "--parents"];
    args.extend(shas.iter().map(String::as_str));
    git(root, &args)
        .lines()
        .filter(|l| l.split_whitespace().count() == 1)
        .map(|l| l.trim().to_string())
        .collect()
}

/// One corpus file changed by a resolution's commit.
struct Change {
    /// `A` for an added node; `M` or `R` for one whose edges may have moved.
    status: u8,
    /// Pre-image blob, all zeroes for an addition.
    src: String,
    /// Post-image blob.
    dst: String,
    /// Post-image path, and the one a node id is read from.
    path: String,
    /// Pre-image path, which differs from `path` only under a rename.
    was: String,
}

/// What each of `shas` changed under the corpus, in one subprocess.
///
/// `--stdin` requires full object names, which is why [`adding_commits`] reads
/// `--no-abbrev`. `-M` is passed explicitly rather than relied on: `diff.renames` is a user
/// setting, and a check whose findings depend on the reader's git config is not a check.
fn corpus_changes(root: &Path, shas: &BTreeSet<String>) -> HashMap<String, Vec<Change>> {
    let mut out: HashMap<String, Vec<Change>> = HashMap::new();
    if shas.is_empty() {
        return out;
    }
    let input: String = shas.iter().map(|s| format!("{s}\n")).collect();
    let text = git_stdin(
        root,
        &["diff-tree", "--stdin", "-r", "-M", "--raw", "--root"],
        &input,
    );
    let mut sha = String::new();
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix(':') {
            let mut fields = rest.split('\t');
            let Some(meta) = fields.next() else { continue };
            let Some(first) = fields.next() else { continue };
            let f: Vec<&str> = meta.split_whitespace().collect();
            if f.len() < 5 {
                continue;
            }
            // `R100\told\tnew` carries two paths; every other status carries one.
            let second = fields.next();
            let (was, path) = match second {
                Some(new) => (first.to_string(), new.to_string()),
                None => (first.to_string(), first.to_string()),
            };
            out.entry(sha.clone()).or_default().push(Change {
                status: f[4].as_bytes()[0],
                src: f[2].to_string(),
                dst: f[3].to_string(),
                path,
                was,
            });
        } else if line.len() == 40 && line.bytes().all(|b| b.is_ascii_hexdigit()) {
            sha = line.to_string();
        }
    }
    out
}

/// Resolve every `ma/<elector>@<hash>` tip to a commit, in one subprocess.
///
/// Keyed on the `@<hash>` half rather than on the branch: the branch has moved, and the whole
/// point of recording a hash is that the tip read is the tip named. A record naming no hash
/// at all resolves to nothing, which is the honest answer — there is no commit in it.
fn resolve_tips(root: &Path, tips: &BTreeSet<String>) -> HashMap<String, String> {
    let mut out = HashMap::new();
    if tips.is_empty() {
        return out;
    }
    let asked: Vec<&String> = tips.iter().collect();
    let input: String = asked
        .iter()
        .map(|t| format!("{}^{{commit}}\n", t.rsplit('@').next().unwrap_or(t)))
        .collect();
    let text = git_stdin(root, &["cat-file", "--batch-check"], &input);
    for (tip, line) in asked.iter().zip(text.lines()) {
        let mut f = line.split_whitespace();
        let (Some(oid), Some(kind)) = (f.next(), f.next()) else {
            continue;
        };
        if kind == "commit" {
            out.insert((*tip).clone(), oid.to_string());
        }
    }
    out
}

/// Which of `paths` exist at each of `revs`, in one subprocess.
fn present(root: &Path, revs: &[String], paths: &BTreeSet<String>) -> HashSet<(String, String)> {
    let mut out = HashSet::new();
    if revs.is_empty() || paths.is_empty() {
        return out;
    }
    let asked: Vec<(String, String)> = revs
        .iter()
        .flat_map(|r| paths.iter().map(move |p| (r.clone(), p.clone())))
        .collect();
    let input: String = asked.iter().map(|(r, p)| format!("{r}:{p}\n")).collect();
    let text = git_stdin(root, &["cat-file", "--batch-check"], &input);
    for (pair, line) in asked.iter().zip(text.lines()) {
        if line.split_whitespace().nth(1) == Some("blob") {
            out.insert(pair.clone());
        }
    }
    out
}

/// Whether any position filed at `rev` so much as names this node.
///
/// The second half of *held*, and the one that keeps the check honest about what Article V
/// says. An elector who argued in prose for a node without committing the file held it in
/// every sense the article means; requiring the file to exist on their branch would read
/// "held by an elector" as "already built by an elector", which is a different and much
/// stricter rule than the one written down.
///
/// Measured, this is a real widening and not a blanket pardon: across the tips of the
/// resolutions that seated anything, the positions name 13 to 20 of the corpus's 106 node
/// stems. It withdrew none of the four findings.
fn named_in_a_position(root: &Path, rev: &str, node: &str) -> bool {
    let stem = node.rsplit('/').next().unwrap_or(node);
    !git(
        root,
        &[
            "grep",
            "--name-only",
            "-F",
            "-e",
            node,
            "-e",
            stem,
            rev,
            "--",
            POSITIONS,
        ],
    )
    .trim()
    .is_empty()
}

/// Audit every resolution record against the tips it names.
///
/// Returns an empty audit list without touching git when there are no records, which is every
/// repository not running a sangha — collective mode is opt-in and the common case is a
/// corpus with no resolutions at all.
///
/// A record with no adding commit is not audited: it has been written and not yet committed,
/// which is the ordinary state of a resolution somebody is drafting, and there is no tree to
/// diff it against.
pub(crate) fn audit(root: &Path, records: &[Resolution]) -> Vec<ScopeAudit> {
    if records.is_empty() {
        return Vec::new();
    }
    let mut adds = adding_commits(root);
    // A shallow clone's boundary commit has no parents, so git reports it as a root and every
    // path in it as *added*. That is the one shape that could turn a thin checkout into a
    // flood of errors rather than into silence: the boundary would read as a resolution that
    // seated the entire corpus. Records attributed to it are dropped here and reported as
    // unverifiable below, which is what they are.
    if git(root, &["rev-parse", "--is-shallow-repository"]).trim() == "true" {
        let roots = roots_among(root, &adds.values().cloned().collect());
        adds.retain(|_, sha| !roots.contains(sha));
    }
    let shas: BTreeSet<String> = records
        .iter()
        .filter_map(|r| adds.get(&r.file).cloned())
        .collect();
    let changes = corpus_changes(root, &shas);

    let all_tips: BTreeSet<String> = records
        .iter()
        .flat_map(|r| r.tips.iter().cloned())
        .collect();
    let resolved = resolve_tips(root, &all_tips);

    // Which records the baseline actually carries. A record with no adding commit is one of
    // two very different things — a draft nobody has committed, or a committed record whose
    // history this clone does not have — and only the second is a finding.
    let committed = present(
        root,
        &["HEAD".to_string()],
        &records.iter().map(|r| r.file.clone()).collect(),
    );

    // One pass to collect what has to be read, then two batched reads. Doing this per record
    // would spawn a subprocess per file per tip.
    let mut want_paths: BTreeSet<String> = BTreeSet::new();
    let mut want_blobs: BTreeSet<String> = BTreeSet::new();
    for r in records {
        let Some(sha) = adds.get(&r.file) else {
            continue;
        };
        for c in changes.get(sha).into_iter().flatten() {
            if !is_instance(&c.path) || c.status == b'D' {
                continue;
            }
            want_paths.insert(c.was.clone());
            want_blobs.insert(c.dst.clone());
            if c.status != b'A' {
                want_blobs.insert(c.src.clone());
            }
        }
    }
    let live_tips: Vec<String> = resolved.values().cloned().collect();
    let at_tip = present(root, &live_tips, &want_paths);
    // The node's text at each tip, for the edge half: an edge is held when the same
    // `(relationship, target)` stands on the same node somewhere in the tips.
    let tip_blobs: Vec<String> = at_tip.iter().map(|(r, p)| format!("{r}:{p}")).collect();
    let blobs = read_blobs(
        root,
        &want_blobs
            .iter()
            .cloned()
            .chain(tip_blobs)
            .collect::<Vec<_>>(),
    );

    let mut out = Vec::new();
    for r in records {
        let sha = adds.get(&r.file);
        if sha.is_none() && !committed.contains(&("HEAD".to_string(), r.file.clone())) {
            continue;
        }
        let mut audit = ScopeAudit {
            file: r.file.clone(),
            tips: r.tips.len(),
            unreadable_tips: r
                .tips
                .iter()
                .filter(|t| !resolved.contains_key(*t))
                .cloned()
                .collect(),
            history_truncated: sha.is_none(),
            remains_open: remains_open(
                &std::fs::read_to_string(root.join(&r.file)).unwrap_or_default(),
            ),
            introduced: Vec::new(),
        };
        let Some(sha) = sha else {
            out.push(audit);
            continue;
        };
        let tips: Vec<String> = r
            .tips
            .iter()
            .filter_map(|t| resolved.get(t).cloned())
            .collect();
        for c in changes.get(sha).into_iter().flatten() {
            if !is_instance(&c.path) || c.status == b'D' {
                continue;
            }
            let id = node_id(&c.path);
            if c.status == b'A' {
                let held = tips.iter().any(|t| {
                    at_tip.contains(&(t.clone(), c.was.clone()))
                        || named_in_a_position(root, t, &id)
                });
                audit.introduced.push(Introduced {
                    node: id.clone(),
                    edge: None,
                    held,
                });
            }
            let after = blobs
                .get(&c.dst)
                .map(|t| links_of(&c.path, t))
                .unwrap_or_default();
            let before = match c.status {
                b'A' => BTreeSet::new(),
                _ => blobs
                    .get(&c.src)
                    .map(|t| links_of(&c.was, t))
                    .unwrap_or_default(),
            };
            for edge in after.difference(&before) {
                let held = tips.iter().any(|t| {
                    blobs
                        .get(&format!("{t}:{}", c.was))
                        .is_some_and(|text| links_of(&c.was, text).contains(edge))
                });
                audit.introduced.push(Introduced {
                    node: id.clone(),
                    edge: Some(edge.clone()),
                    held,
                });
            }
        }
        out.push(audit);
    }
    out
}

/// Both checks, so the two readings of one audit stay in one place.
pub(crate) fn checks(audits: &[ScopeAudit]) -> [Check; 2] {
    [unheld(audits), unverifiable(audits)]
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── the pure readings ─────────────────────────────────────────────────────

    fn seated(node: &str, held: bool) -> Introduced {
        Introduced {
            node: node.to_string(),
            edge: None,
            held,
        }
    }

    fn asserted(node: &str, rel: &str, target: &str, held: bool) -> Introduced {
        Introduced {
            node: node.to_string(),
            edge: Some((rel.to_string(), target.to_string())),
            held,
        }
    }

    fn audit(introduced: Vec<Introduced>) -> ScopeAudit {
        ScopeAudit {
            file: "r.md".into(),
            tips: 2,
            unreadable_tips: Vec::new(),
            history_truncated: false,
            introduced,
            remains_open: String::new(),
        }
    }

    #[test]
    fn a_node_no_tip_held_is_reported() {
        let c = unheld(&[audit(vec![seated("concept/c", false)])]);
        assert_eq!(c.violations.len(), 1);
        assert!(c.violations[0].detail.contains("concept/c"), "{c:?}");
        assert_eq!(c.severity, Severity::Error);
    }

    #[test]
    fn a_node_a_tip_held_is_not() {
        assert!(unheld(&[audit(vec![seated("concept/c", true)])]).passed());
    }

    /// The rule the constitution's commentary states, and the one the measured corpus broke
    /// three times: a position arguing that a class should exist does not hold the instances
    /// of it, so the instances are the finding and the class is not.
    #[test]
    fn edges_incident_to_a_seated_node_are_counted_and_not_repeated() {
        let c = unheld(&[audit(vec![
            seated("districting-plan/plan-2025", false),
            asserted("districting-plan/plan-2025", "advances", "policy/x", false),
            asserted(
                "org/commission",
                "adopts",
                "districting-plan/plan-2025",
                false,
            ),
        ])]);
        assert_eq!(c.violations.len(), 1, "one node, not three findings: {c:?}");
        assert!(
            c.violations[0].detail.contains("2 edge(s) incident to it"),
            "{}",
            c.violations[0].detail
        );
    }

    /// The arm that has never fired in a real corpus. Every one of the 13 edges a resolution
    /// has ever introduced touched a node that same resolution seated; an edge asserted
    /// *between two nodes that were already there* is the purest form of the thing Article V
    /// forbids, and nothing but a test makes it happen.
    #[test]
    fn an_edge_between_two_nodes_that_already_existed_is_its_own_finding() {
        let c = unheld(&[audit(vec![asserted(
            "person/mick",
            "chairs",
            "org/commission",
            false,
        )])]);
        assert_eq!(c.violations.len(), 1, "{c:?}");
        assert!(
            c.violations[0]
                .detail
                .contains("`person/mick -[chairs]-> org/commission`"),
            "{}",
            c.violations[0].detail
        );
    }

    #[test]
    fn an_edge_a_tip_held_is_not_reported() {
        assert!(unheld(&[audit(vec![asserted(
            "person/mick",
            "chairs",
            "org/commission",
            true
        )])])
        .passed());
    }

    /// The one exception the article licenses, and the only thing that can license it: the
    /// record saying the question is still open.
    #[test]
    fn a_node_the_record_leaves_open_is_excepted() {
        let mut a = audit(vec![seated("question/who-appoints", false)]);
        a.remains_open =
            "\nWhether the presiding officer appoints the body — `question/who-appoints`.\n".into();
        assert!(unheld(&[a]).passed());
    }

    #[test]
    fn the_exception_reads_only_what_remains_open() {
        let mut a = audit(vec![seated("question/who-appoints", false)]);
        // Named under the wrong heading: `What was resolved` describes a settled past, and a
        // node named there is not a tension left open.
        a.remains_open = String::new();
        assert_eq!(unheld(&[a]).violations.len(), 1);
    }

    /// Partial evidence at the severity that gates is worse than none. A resolution whose
    /// tips cannot all be read is not judged here at all — its sibling says so.
    #[test]
    fn a_resolution_with_an_unreadable_tip_is_not_judged() {
        let mut a = audit(vec![seated("concept/c", false)]);
        a.unreadable_tips = vec!["ma/one@abc1234".into()];
        assert!(unheld(&[a.clone()]).passed());
        let c = unverifiable(&[a]);
        assert_eq!(c.violations.len(), 1);
        assert_eq!(c.severity, Severity::Warn);
        assert!(c.violations[0].detail.contains("ma/one@abc1234"), "{c:?}");
    }

    /// A record naming no tips at all holds nothing against anything. Everything such a
    /// resolution introduced is trivially unheld, and reporting that as a scope violation
    /// would blame the synthesis for a malformed record.
    #[test]
    fn a_record_naming_no_tips_is_unverifiable_rather_than_in_breach() {
        let mut a = audit(vec![seated("concept/c", false)]);
        a.tips = 0;
        assert!(unheld(&[a.clone()]).passed());
        let c = unverifiable(&[a]);
        assert_eq!(c.violations.len(), 1);
        assert!(c.violations[0].detail.contains("names no `tips:`"), "{c:?}");
    }

    /// The reason a thin clone must say which reason. A shallow checkout cannot see what a
    /// resolution changed, and a check that reported nothing there would be indistinguishable
    /// from one that found nothing wrong.
    #[test]
    fn a_record_whose_commit_is_outside_the_clone_is_unverifiable() {
        let mut a = audit(vec![seated("concept/c", false)]);
        a.history_truncated = true;
        assert!(unheld(&[a.clone()]).passed());
        let c = unverifiable(&[a]);
        assert_eq!(c.violations.len(), 1);
        assert!(
            c.violations[0].detail.contains("not in this clone"),
            "{c:?}"
        );
    }

    #[test]
    fn a_resolution_whose_tips_all_read_reports_nothing_unreadable() {
        assert!(unverifiable(&[audit(vec![seated("concept/c", false)])]).passed());
    }

    #[test]
    fn a_repository_with_no_resolutions_reports_nothing() {
        assert!(unheld(&[]).passed());
        assert!(unverifiable(&[]).passed());
    }

    // ── parsing ───────────────────────────────────────────────────────────────

    #[test]
    fn a_node_id_is_its_path_under_the_corpus() {
        assert_eq!(node_id(".yidam/corpus/person/mick.yml"), "person/mick");
        assert_eq!(node_id(".yidam/corpus/person/sub/x.yml"), "person/sub/x");
    }

    /// Resolved against the declaring file's directory, exactly as the replay and the orphan
    /// checks resolve them — so `../class/x.yml` and `class/x.yml` are one edge here too, and
    /// an edge cannot read as introduced merely because somebody rewrote its path.
    ///
    /// The sibling target is the half that pins the resolution: `../band/napalm.yml` reduces
    /// to the same string with or without the declaring directory, so an edge written that
    /// way proves nothing about whether the directory was consulted at all.
    #[test]
    fn edge_targets_resolve_relative_to_the_declaring_node() {
        let l = links_of(
            ".yidam/corpus/person/mick.yml",
            "class: person\nlinks:\n  - target: ../band/napalm.yml\n    relationship: plays-in\n\
             \x20 - target: keith.yml\n    relationship: plays-with\n",
        );
        assert!(
            l.contains(&("plays-in".into(), "band/napalm".into())),
            "{l:?}"
        );
        assert!(
            l.contains(&("plays-with".into(), "person/keith".into())),
            "{l:?}"
        );
    }

    /// One edge, written two ways. A tip that declared `../concept/b.yml` and a resolution
    /// that writes `b.yml` are saying the same thing, and a check that read the strings would
    /// report the second as introduced.
    #[test]
    fn an_edge_rewritten_relatively_is_the_same_edge() {
        let a = links_of(
            ".yidam/corpus/concept/a.yml",
            "class: concept\nlinks:\n  - target: ../concept/b.yml\n    relationship: rests-on\n",
        );
        let b = links_of(
            ".yidam/corpus/concept/a.yml",
            "class: concept\nlinks:\n  - target: b.yml\n    relationship: rests-on\n",
        );
        assert_eq!(a, b, "the same edge, spelled two ways");
    }

    #[test]
    fn an_unparseable_node_declares_no_edges() {
        assert!(links_of(".yidam/corpus/a/b.yml", "\tnot: [valid").is_empty());
    }

    #[test]
    fn what_remains_open_is_read_to_the_next_heading() {
        let text = "---\nevolution: e\n---\n\n## What was resolved\n\nA thing.\n\n\
                    ## What remains open\n\nWhether `concept/x` belongs.\n\n## Notes\n\nOther.\n";
        let open = remains_open(text);
        assert!(open.contains("concept/x"), "{open:?}");
        assert!(!open.contains("Other"), "{open:?}");
        assert!(!open.contains("A thing"), "{open:?}");
    }

    #[test]
    fn a_record_with_no_open_section_excepts_nothing() {
        assert_eq!(remains_open("## What was resolved\n\nAll of it.\n"), "");
    }

    // ── against a repository ──────────────────────────────────────────────────

    fn git(dir: &Path, args: &[&str]) {
        let ok = Command::new("git")
            .current_dir(dir)
            .args(args)
            .status()
            .unwrap()
            .success();
        assert!(ok, "git {args:?} failed");
    }

    fn write(dir: &Path, rel: &str, body: &str) {
        let p = dir.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, body).unwrap();
    }

    fn commit(dir: &Path, msg: &str) {
        git(dir, &["add", "-A"]);
        let ok = Command::new("git")
            .current_dir(dir)
            .args(["commit", "-q", "--no-gpg-sign", "-m", msg])
            .env("GIT_AUTHOR_DATE", "2026-01-01T00:00:00Z")
            .env("GIT_COMMITTER_DATE", "2026-01-01T00:00:00Z")
            .status()
            .unwrap()
            .success();
        assert!(ok, "commit failed");
    }

    fn head(dir: &Path) -> String {
        super::git(dir, &["rev-parse", "--short", "HEAD"])
            .trim()
            .to_string()
    }

    /// A repository holding two nodes, with `ma/one` parked at a tip that holds them.
    fn repo() -> (tempfile::TempDir, String) {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path();
        git(root, &["init", "-q", "-b", "main"]);
        git(root, &["config", "user.email", "t@t.com"]);
        git(root, &["config", "user.name", "T"]);
        git(root, &["config", "commit.gpgsign", "false"]);
        write(root, ".yidam/corpus/concept/a.yml", "class: concept\n");
        write(root, ".yidam/corpus/concept/b.yml", "class: concept\n");
        commit(root, "establish: a and b");
        git(root, &["branch", "ma/one"]);
        let tip = head(root);
        (tmp, tip)
    }

    fn record(tips: &[String], open: &str) -> String {
        let list: String = tips.iter().map(|t| format!("  - {t}\n")).collect();
        format!(
            "---\nevolution: e\ndate: 2026-01-02\ntips:\n{list}---\n\n\
             ## What was resolved\n\nSomething.\n\n## What remains open\n\n{open}\n"
        )
    }

    fn run(root: &Path) -> [Check; 2] {
        let data = crate::cmd::sangha::sangha_data(root);
        checks(&audit_repo(root, &data.resolutions))
    }

    // Named apart from the test helper `audit` above, which builds one by hand.
    fn audit_repo(root: &Path, records: &[Resolution]) -> Vec<ScopeAudit> {
        super::audit(root, records)
    }

    /// The whole check, end to end, on the case the measured corpus produced three times.
    #[test]
    fn a_resolution_seating_a_node_no_tip_holds_is_caught() {
        let (tmp, tip) = repo();
        let root = tmp.path();
        write(root, ".yidam/corpus/concept/c.yml", "class: concept\n");
        write(
            root,
            ".yidam/sangha/resolutions/e.md",
            &record(&[format!("ma/one@{tip}")], "Nothing."),
        );
        commit(root, "synthesize: e");

        let [unheld, unreadable] = run(root);
        assert!(unreadable.passed(), "{unreadable:?}");
        assert_eq!(unheld.violations.len(), 1, "{unheld:?}");
        assert!(
            unheld.violations[0].detail.contains("concept/c"),
            "{}",
            unheld.violations[0].detail
        );
    }

    /// The same resolution, against a tip that already holds the node. This is the arm that
    /// makes the check a check rather than "did a resolution add a file".
    #[test]
    fn a_node_standing_at_the_tip_is_held() {
        let (tmp, _) = repo();
        let root = tmp.path();
        // The elector authors it on their own branch; the tip recorded is that commit.
        git(root, &["switch", "-q", "ma/one"]);
        write(root, ".yidam/corpus/concept/c.yml", "class: concept\n");
        commit(root, "open: c, on my own branch");
        let tip = head(root);
        git(root, &["switch", "-q", "main"]);

        write(root, ".yidam/corpus/concept/c.yml", "class: concept\n");
        write(
            root,
            ".yidam/sangha/resolutions/e.md",
            &record(&[format!("ma/one@{tip}")], "Nothing."),
        );
        commit(root, "synthesize: e");

        let [unheld, _] = run(root);
        assert!(unheld.passed(), "{unheld:?}");
    }

    /// Held in prose. An elector who argued for a node without committing the file held it in
    /// every sense Article V means, and requiring the file would read "held by an elector" as
    /// "already built by an elector".
    #[test]
    fn a_node_only_named_in_a_position_is_held() {
        let (tmp, _) = repo();
        let root = tmp.path();
        git(root, &["switch", "-q", "ma/one"]);
        write(
            root,
            ".yidam/sangha/positions/one-e.md",
            "The corpus is short a node for `concept/c`, and it should have one.\n",
        );
        commit(root, "open: my position on e");
        let tip = head(root);
        git(root, &["switch", "-q", "main"]);

        write(root, ".yidam/corpus/concept/c.yml", "class: concept\n");
        write(
            root,
            ".yidam/sangha/resolutions/e.md",
            &record(&[format!("ma/one@{tip}")], "Nothing."),
        );
        commit(root, "synthesize: e");

        let [unheld, _] = run(root);
        assert!(unheld.passed(), "{unheld:?}");
    }

    /// The never-fired arm, end to end: an edge asserted between two nodes that were already
    /// in the corpus, which no tip declared.
    #[test]
    fn an_edge_asserted_between_standing_nodes_is_caught() {
        let (tmp, tip) = repo();
        let root = tmp.path();
        write(
            root,
            ".yidam/corpus/concept/a.yml",
            "class: concept\nlinks:\n  - target: ../concept/b.yml\n    relationship: rests-on\n",
        );
        write(
            root,
            ".yidam/sangha/resolutions/e.md",
            &record(&[format!("ma/one@{tip}")], "Nothing."),
        );
        commit(root, "synthesize: e");

        let [unheld, _] = run(root);
        assert_eq!(unheld.violations.len(), 1, "{unheld:?}");
        assert!(
            unheld.violations[0]
                .detail
                .contains("`concept/a -[rests-on]-> concept/b`"),
            "{}",
            unheld.violations[0].detail
        );
    }

    #[test]
    fn a_tip_that_names_no_commit_is_reported_and_stops_the_audit() {
        let (tmp, _) = repo();
        let root = tmp.path();
        write(root, ".yidam/corpus/concept/c.yml", "class: concept\n");
        write(
            root,
            ".yidam/sangha/resolutions/e.md",
            &record(&[String::from("ma/one@deadbee")], "Nothing."),
        );
        commit(root, "synthesize: e");

        let [unheld, unreadable] = run(root);
        assert!(
            unheld.passed(),
            "partial evidence must not gate: {unheld:?}"
        );
        assert_eq!(unreadable.violations.len(), 1, "{unreadable:?}");
    }

    /// A record written and not yet committed has no tree to diff against, and drafting one
    /// is the ordinary state of a resolution in progress.
    #[test]
    fn an_uncommitted_record_is_not_audited() {
        let (tmp, tip) = repo();
        let root = tmp.path();
        write(root, ".yidam/corpus/concept/c.yml", "class: concept\n");
        write(
            root,
            ".yidam/sangha/resolutions/e.md",
            &record(&[format!("ma/one@{tip}")], "Nothing."),
        );
        let [unheld, unreadable] = run(root);
        assert!(unheld.passed(), "{unheld:?}");
        assert!(unreadable.passed(), "{unreadable:?}");
    }

    /// Renaming a node is not seating one, and whether git says so depends on `diff.renames`
    /// — a user setting. A check whose findings turn on the reader's git config is not a
    /// check, which is why `-M` is passed rather than relied on.
    #[test]
    fn a_renamed_node_is_not_a_seated_one() {
        let (tmp, tip) = repo();
        let root = tmp.path();
        // The condition that would otherwise hide this: rename detection off by config.
        git(root, &["config", "diff.renames", "false"]);
        git(
            root,
            &[
                "mv",
                ".yidam/corpus/concept/a.yml",
                ".yidam/corpus/concept/a-renamed.yml",
            ],
        );
        write(
            root,
            ".yidam/sangha/resolutions/e.md",
            &record(&[format!("ma/one@{tip}")], "Nothing."),
        );
        commit(root, "rename: a becomes a-renamed");

        let [unheld, _] = run(root);
        assert!(unheld.passed(), "{unheld:?}");
    }

    /// A record deleted and restored is one resolution that happened once, and the commit
    /// that matters is the one that performed it. Reading the newest add would diff a commit
    /// that touched no corpus file and find nothing at all.
    #[test]
    fn a_record_deleted_and_restored_is_audited_at_the_commit_that_settled_it() {
        let (tmp, tip) = repo();
        let root = tmp.path();
        let text = record(&[format!("ma/one@{tip}")], "Nothing.");
        write(root, ".yidam/corpus/concept/c.yml", "class: concept\n");
        write(root, ".yidam/sangha/resolutions/e.md", &text);
        commit(root, "synthesize: e");

        std::fs::remove_file(root.join(".yidam/sangha/resolutions/e.md")).unwrap();
        commit(root, "retire: the record, briefly");
        write(root, ".yidam/sangha/resolutions/e.md", &text);
        commit(root, "record: restored");

        let [unheld, _] = run(root);
        assert_eq!(unheld.violations.len(), 1, "{unheld:?}");
        assert!(unheld.violations[0].detail.contains("concept/c"));
    }

    /// The hazard this closes, end to end. A shallow clone's boundary commit has no parents,
    /// so git reports every path in it as *added* — and a boundary that also happens to carry
    /// a resolvable tip would read as one resolution seating the whole corpus. The right
    /// answer is not silence and it is certainly not a flood: it is that the clone cannot
    /// answer the question.
    #[test]
    fn a_shallow_clone_says_so_rather_than_seating_the_whole_corpus() {
        let (tmp, tip) = repo();
        let root = tmp.path();
        write(root, ".yidam/corpus/concept/c.yml", "class: concept\n");
        write(
            root,
            ".yidam/sangha/resolutions/e.md",
            &record(&[format!("ma/one@{tip}")], "Nothing."),
        );
        commit(root, "synthesize: e");

        let thin = tempfile::TempDir::new().unwrap();
        let url = format!("file://{}", root.display());
        git(
            thin.path(),
            &[
                "clone",
                "-q",
                "--depth",
                "1",
                "--no-single-branch",
                "--branch",
                "main",
                &url,
                ".",
            ],
        );
        let [unheld, unverifiable] = run(thin.path());
        assert!(unheld.passed(), "{unheld:?}");
        assert_eq!(unverifiable.violations.len(), 1, "{unverifiable:?}");
        assert!(
            unverifiable.violations[0]
                .detail
                .contains("not in this clone"),
            "{}",
            unverifiable.violations[0].detail
        );
    }

    /// A node the record itself leaves open, through the whole pipeline.
    #[test]
    fn an_open_question_node_the_record_names_is_excepted_end_to_end() {
        let (tmp, tip) = repo();
        let root = tmp.path();
        write(root, ".yidam/corpus/concept/c.yml", "class: concept\n");
        write(
            root,
            ".yidam/sangha/resolutions/e.md",
            &record(
                &[format!("ma/one@{tip}")],
                "Whether `concept/c` belongs at all — neither elector would say.",
            ),
        );
        commit(root, "synthesize: e");

        let [unheld, _] = run(root);
        assert!(unheld.passed(), "{unheld:?}");
    }
}
