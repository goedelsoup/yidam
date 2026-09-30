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

use std::collections::{BTreeMap, HashMap, HashSet};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;

use crate::corpus::resolve_target;

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
/// Class definitions sit at depth 1 and end `.ont.yml`; instances sit **at or below** depth 2.
/// Anything else under the corpus (a README, an ACTIONS.md) is not a node and neither points
/// nor is pointed at.
///
/// # Why depth 2 is a floor and not an equality
///
/// [`crate::walk::walk_corpus_instances`] — the live walk, and the definition every check and
/// every present-tense query answers from — accepts `e.depth() >= 2`. This required exactly
/// two, and the two are separate answers to *what is in the corpus*: a node at
/// `.yidam/corpus/<class>/<sub>/<name>.yml` exists at HEAD, is read by the gate, resolves as
/// a `links:` target, and disappeared at every revision reconstructed through here. `--at
/// HEAD` on a clean tree stopped being the identity, and any relative edge into such a node
/// dangled historically while resolving live — the disagreement RFC-0018 requires the
/// historical path never to produce silently.
pub(crate) fn is_instance(path: &str) -> bool {
    let Some(rest) = path.strip_prefix(".yidam/corpus/") else {
        return false;
    };
    rest.ends_with(".yml") && !rest.ends_with(".ont.yml") && rest.contains('/')
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
    inst.links
        .unwrap_or_default()
        .iter()
        .filter_map(|l| l.target.as_ref())
        .map(|t| {
            resolve_target(Path::new(path), t)
                .to_string_lossy()
                .replace('\\', "/")
        })
        .collect()
}

/// One commit's worth of corpus change.
struct CommitChanges {
    sha: String,
    ts: i64,
    changes: Vec<Change>,
}

/// `git log --raw` over the corpus, oldest first.
///
/// # Why a renamed node is a new node here
///
/// `--no-renames` makes every move a deletion and an addition. A `git mv` and a delete plus
/// add are the same commit to git, so the replay treats them the same way (#1171). The node at
/// the new path starts from the commit that put it there.
///
/// That is path identity, and it is the corpus's own. A node's id is its path, a `links:`
/// target names a path, and [`crate::cmd::query::at`] rebuilds a past tree by path. Before the
/// rename, no edge could name the new path. So "uncited since before it existed" is not a
/// state the graph could have been in. Pairing the two halves would need `-M`'s similarity
/// index, and that gives up once the text changes by half. The pairing would then hold for a
/// light edit and break for a heavy one, which is the blindness #1171 set out to remove.
///
/// # Why an age can still cross a move
///
/// The frames stay path-keyed. The age folds do not have to, because a node can declare the
/// node it continues: `yidam rename` writes `moved-from:` into the moved file, naming the old
/// path the way a `target:` would (#1180). A rename is not a citation and does not answer a
/// question, so an orphan or an open question that was only moved keeps its count. The
/// declaration is text in the node, so a `git mv` and a delete plus add with every line
/// rewritten carry alike, and so does a squash or a rebase. See [`age_while`].
///
/// A move that declares nothing still restarts, as `migrate`'s class rename does (#1192):
/// `orphan-in`'s escalation and `due`'s questions clock both read the count.
/// [`super::scope::adding_commits`] carries identity by another route, because a resolution
/// record has one apart from its path: its evolution.
fn change_stream(root: &Path) -> Vec<CommitChanges> {
    // `core.quotepath=false` comes from the runner, and it matters here: `--raw` quotes a
    // non-ASCII path exactly as `--name-status` does, and the parser below tests a prefix.
    let Some(text) = crate::git::Git::new(root)
        .args([
            "log",
            "--reverse",
            "--raw",
            "--no-abbrev",
            "--no-renames",
            "--format=C %H %at",
        ])
        .paths([".yidam/corpus"])
        .try_run()
    else {
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
///
/// Shared with [`crate::cmd::query::at`], which reconstructs a whole tree rather than a
/// stream of changes but reads its blobs the same way and for the same reason.
pub(crate) fn read_blobs(root: &Path, shas: &[String]) -> HashMap<String, String> {
    let mut found = HashMap::new();
    if shas.is_empty() {
        return found;
    }
    let Ok(mut child) = crate::git::Git::new(root)
        .args(["cat-file", "--batch"])
        .spawn_piped()
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
pub(crate) fn is_class(path: &str) -> bool {
    path.strip_prefix(".yidam/corpus/")
        .is_some_and(|r| r.ends_with(".ont.yml") && !r.contains('/'))
}

/// The class a node belongs to, from its path.
pub(crate) fn class_of(path: &str) -> &str {
    path.strip_prefix(".yidam/corpus/")
        .and_then(|r| r.split('/').next())
        .unwrap_or_default()
}

/// What a class definition says about being pointed at.
///
/// Three states, and the third is not the absence of the question. A class that declares no
/// `edges:` has said **nothing** about its shape — which is why it is not a source class,
/// and equally why an uncited instance of it is not a finding against a declared
/// expectation. Collapsing "declares no expectation" into "expects to be cited" would
/// report every corpus whose ontology is not filled in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Expectation {
    /// Declares edges, none inbound: nothing is meant to point at its instances.
    Uncited,
    /// Declares an inbound edge: its instances are meant to be pointed at.
    Cited,
    /// Declares no edges at all. Says nothing, and is not read as saying anything.
    Unstated,
}

impl Expectation {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Uncited => "uncited",
            Self::Cited => "cited",
            Self::Unstated => "unstated",
        }
    }
}

/// What the ontology at this commit says about each class being pointed at.
///
/// Three states, and the third has to survive: a class nothing points at *and* which
/// declares no edges said nothing at all, which is not the same as saying it is a source
/// class. A class that declares no edges but which the ontology points *at*, from the other
/// end, has been spoken for and is `Cited`.
///
/// **Pointed at, not merely named.** That distinction is the whole of #659. This asked
/// "does any edge anywhere carry this class as its `target`", which is direction-blind:
/// `D: {relationship: r, target: C, direction: in}` says instances of `C` point at instances
/// of `D`, and names `C` while saying nothing about anything pointing at `C`. A `C` that
/// declared no edges of its own was read as `Cited` on the strength of it and scored against
/// `uncited == 0` — which is precisely the state the missing entry exists to represent, and
/// `direction: in` is not hypothetical: the reports fixture's own `concept.ont.yml` uses it.
/// [`crate::corpus::pointed_classes`] is the one reading of `direction:`, and both questions
/// now go through it rather than being answered twice.
fn expectations_of(view: &[crate::corpus::EdgeView<'_>]) -> HashMap<String, Expectation> {
    let sources = crate::corpus::source_classes(view);
    let pointed = crate::corpus::pointed_classes(view);
    view.iter()
        .filter_map(|v| {
            if sources.contains(v.name) {
                Some((v.name.to_string(), Expectation::Uncited))
            } else if v.edges.is_empty() && !pointed.contains(v.name) {
                None
            } else {
                Some((v.name.to_string(), Expectation::Cited))
            }
        })
        .collect()
}

/// The corpus as it stood at one commit.
pub(crate) struct Frame<'a> {
    pub sha: &'a str,
    pub ts: i64,
    /// Every instance node present, and the targets it points at.
    pub out: &'a HashMap<String, HashSet<String>>,
    /// Every instance node present, and its content at this commit.
    ///
    /// Borrowed from the one `cat-file --batch` the walk already ran, so carrying it costs a
    /// pointer per live node rather than a second read of history. It exists because
    /// [`open_question_age`] asks a question of the *text* — a `[open]` tag in prose — that
    /// no reduction of the edges can answer.
    pub text: &'a HashMap<String, &'a str>,
    /// Which properties carried an evidence tag **at this commit**, not today.
    ///
    /// A class that grew a `type: claim` property last month did not make last year's
    /// instances open questions retroactively, and reading today's ontology over the whole
    /// walk would date every such question to the commit that added the property. Built per
    /// frame from the class blobs, through the same parser
    /// [`crate::claims::ClaimFields::load`] uses.
    pub claim_fields: &'a crate::claims::ClaimFields,
    /// Every node something points at.
    pub cited: HashSet<&'a String>,
    /// Classes that declare no inbound edge, whose instances are orphans by design.
    pub source_classes: &'a HashSet<String>,
    /// What each class declares about being pointed at, for the classes that say anything.
    ///
    /// A class absent from this map declared nothing — which is the third state, not a
    /// missing entry. See [`Expectation`].
    pub expectations: &'a HashMap<String, Expectation>,
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
    // The same nodes' content, borrowed from the blobs already read. Kept in step with
    // `out` — every insert and every removal touches both — so a frame cannot show a node
    // whose text is a previous revision's.
    let mut text: HashMap<String, &str> = HashMap::new();
    // The ontology as it stands, class by class, keyed by the `.ont.yml` stem — which is
    // what an instance's class resolves to, live and here.
    //
    // **Kept rather than reduced on the way in**, because the reduction reads every class at
    // once. An inbound relationship may be declared from either end, so which classes are
    // exempt is a property of the whole ontology at a commit rather than of one file in it.
    // The replay therefore keeps the declarations and derives the answer per frame, through
    // [`crate::corpus::source_classes`] — the same function the check calls, so the two
    // cannot disagree about which classes are exempt. That guarantee held for *exempt* and
    // not for *pointed at*, which the replay went on to answer for itself, direction-blind,
    // and got wrong (#659); both now read [`crate::corpus::pointed_classes`], which is where
    // `direction:` is interpreted and the only place it is.
    //
    // **One [`crate::corpus::Class`] per blob, where this was two parses** (#1116). The
    // replay read each class blob twice — once through a local `edges:` struct and once
    // through `claims::declared_claim_fields` — so a past revision of the ontology was
    // described by two readers that the live corpus had replaced with one. `Class::parse` is
    // the live parse, given a blob instead of a file.
    let mut classes: BTreeMap<String, crate::corpus::Class> = BTreeMap::new();

    for c in &commits {
        let mut touched = false;
        for ch in &c.changes {
            let content = || blobs.get(&ch.blob).map(String::as_str).unwrap_or("");
            if is_instance(&ch.path) {
                touched = true;
                if ch.status == b'D' {
                    out.remove(&ch.path);
                    text.remove(&ch.path);
                } else {
                    out.insert(ch.path.clone(), targets_of(&ch.path, content()));
                    text.insert(ch.path.clone(), content());
                }
            } else if is_class(&ch.path) {
                touched = true;
                // The stem, from the one place that derives it — the same answer
                // `Class::parse` gives the record inserted just below (#1116).
                let name = crate::corpus::Class::name_of(&ch.path);
                match ch.status {
                    b'D' => {
                        classes.remove(&name);
                    }
                    _ => {
                        classes.insert(
                            name,
                            crate::corpus::Class::parse(ch.path.clone(), content()),
                        );
                    }
                }
            }
        }
        if !touched {
            continue;
        }
        let cited: HashSet<&String> = out.values().flatten().collect();
        // Derived per frame rather than maintained incrementally: one class's edit can
        // change another class's exemption, so there is nothing to update in place.
        let view: Vec<crate::corpus::EdgeView<'_>> = classes
            .iter()
            .map(|(name, c)| crate::corpus::EdgeView {
                name,
                edges: &c.edges,
            })
            .collect();
        let source_classes = crate::corpus::source_classes(&view);
        let expectations = expectations_of(&view);
        let claim_fields = crate::claims::ClaimFields::from_classes(classes.values());
        frame(Frame {
            sha: &c.sha,
            ts: c.ts,
            out: &out,
            text: &text,
            claim_fields: &claim_fields,
            cited,
            source_classes: &source_classes,
            expectations: &expectations,
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
    age_while(root, |f, node, _| !f.cited.contains(node))
}

/// Whether a node's content at one commit reads as an open question.
///
/// The live predicate, applied to a past revision. `status` and `open-questions` ask
/// [`crate::claims::has_open_claim`] the same way over the working tree; asking it
/// differently here would produce an age for a condition the present-tense reports do not
/// agree is holding.
///
/// **Not a duplicate of [`crate::corpus`]'s read** (#1116). The bytes are a blob out of the
/// replay, not a file on disk, and the replay hands them over one frame at a time — there is
/// no corpus to open at a past commit without `query::at`'s tree listing and blob fill per
/// frame, which is a whole corpus reconstructed to answer one predicate. The *record* is the
/// model's: [`crate::corpus::parse_or_default`] is the one place bytes become a
/// [`crate::parse::CorpusInstance`]. Its outcome is dropped, because a node that did not
/// parse at some past commit is not a question at that commit and an age has nowhere to say
/// otherwise.
fn is_open(path: &str, content: &str, fields: &crate::claims::ClaimFields) -> bool {
    let (inst, _malformed): (crate::parse::CorpusInstance, _) =
        crate::corpus::parse_or_default(content);
    let label = inst.label.unwrap_or_default();
    // The node's declared class, then its directory. `ClaimFields` is keyed by the
    // `<class>.ont.yml` stem (#1116) and `class_of` reads the directory, which is the same
    // name; the declared field is tried first because that is the lookup `status` and
    // `open-questions` make, and asking differently here would date a condition those
    // reports do not agree is holding. The fallback is this reader's own: a historical node
    // that named no class still lived in a class directory.
    let class = inst.class.unwrap_or_else(|| class_of(path).to_string());
    crate::claims::has_open_claim(&label, content, fields.for_class(&class))
}

/// For every node that is an open question at HEAD, how long it has been one.
///
/// The third clock `yidam due` reads, and the one that had no machinery. `status` counts
/// open questions and cannot say whether it is counting a question asked this morning or one
/// that has stood unanswered through two hundred commits — which is the whole distinction
/// `docs/post-genesis-measurement.md` draws about residence time.
///
/// **Dated from when it became a question, not from when the node was written.** A node
/// authored a year ago and tagged `[open]` yesterday has been a question for one commit.
/// That is the same distinction [`uncited_age`] draws, for the same reason, and node age
/// cannot draw either.
///
/// **A question that was closed and reopened dates from the reopening.** The condition is
/// read per frame and any commit in which it did not hold ends the residence — an answered
/// question that a later commit reopens is a new question, and carrying the old date would
/// report a corpus as having ignored something it in fact resolved.
pub fn open_question_age(root: &Path) -> HashMap<String, Age> {
    age_while(root, |f, node, content| {
        is_open(node, content, f.claim_fields)
    })
}

/// The node a path's content says it continues, resolved as a `target:` is.
///
/// `yidam rename` writes it. Read from `extra` rather than a field of its own, because
/// nothing but this fold reads it.
fn moved_from(path: &str, content: &str) -> Option<String> {
    let inst: crate::parse::CorpusInstance = serde_yaml::from_str(content).ok()?;
    let from = inst.extra.get("moved-from")?.as_str()?;
    Some(
        resolve_target(Path::new(path), from)
            .to_string_lossy()
            .replace('\\', "/"),
    )
}

/// For every node present at HEAD, how long `holds` has been true of it, carried across a
/// declared move.
///
/// The one fold both ages are, so they cannot come to disagree about what a move carries.
///
/// **A move carries only what was holding.** A node that leaves the tree while the condition
/// holds is kept aside. A node that arrives with `moved-from:` naming it takes that entry,
/// and is then tested like any other: a rename commit that also adds the citation or answers
/// the question ends the age there. The delete and the add may be separate commits. The
/// frames in between still count, because nothing in them cited the node or answered it.
///
/// **A copy does not carry.** If the declared node is still in the tree when the new path
/// arrives, the new path is a second node and dates from its own commit. That is the rule
/// [`super::scope::adding_commits`] applies to a resolution record, for the same reason.
///
/// **Only on arrival.** The declaration is read in the frame where a path first appears.
/// A `moved-from:` left in the file after that says where the node came from and nothing
/// more, so a later node that reuses the old path is not mistaken for it.
fn age_while(
    root: &Path,
    holds: impl Fn(&Frame<'_>, &String, &str) -> bool,
) -> HashMap<String, Age> {
    // Value is (sha, ts, index of the frame at which the condition began). The index becomes
    // a count once the walk is over and the total is known; it cannot be a count while the
    // walk is running, because the walk does not know how many frames are left.
    let mut since: HashMap<String, (String, i64, usize)> = HashMap::new();
    // Entries whose node has left the tree, kept for an arrival that declares it.
    let mut departed: HashMap<String, (String, i64, usize)> = HashMap::new();
    let mut present: HashSet<String> = HashSet::new();
    let mut frames = 0usize;
    replay(root, |f| {
        // Departures first, so an arrival in the same commit finds the entry it continues.
        for gone in present.iter().filter(|n| !f.text.contains_key(*n)) {
            if let Some(entry) = since.remove(gone) {
                departed.insert(gone.clone(), entry);
            }
        }
        for (node, content) in f.text {
            if present.contains(node) {
                continue;
            }
            let Some(from) = moved_from(node, content).filter(|p| !f.text.contains_key(p)) else {
                continue;
            };
            if let Some(entry) = departed.remove(&from) {
                since.insert(node.clone(), entry);
            }
        }
        for (node, content) in f.text {
            if holds(&f, node, content) {
                // Keep the earliest commit at which the condition held.
                since
                    .entry(node.clone())
                    .or_insert((f.sha.to_string(), f.ts, frames));
            } else {
                since.remove(node);
            }
        }
        present = f.text.keys().cloned().collect();
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
        // Deeper than a class directory and still a node: this is what the live walk reads,
        // and the two definitions must not disagree about what the corpus holds.
        assert!(is_instance(".yidam/corpus/person/sub/x.yml"));
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

    /// Commit at a fixed date so the assertions are about the replay and not the clock.
    fn commit(dir: &Path, day: &str, msg: &str) {
        crate::git::fixture::commit_at(dir, msg, &format!("{day}T00:00:00Z"));
    }

    fn node(dir: &Path, rel: &str, body: &str) {
        let p = dir.join(".yidam/corpus").join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, body).unwrap();
    }

    use crate::git::fixture::init;

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

    /// **A move that declares nothing is a new node, however it is made** (#1171). `a` is
    /// moved with `git mv` and `b` by a delete plus add. Both give the same answer: each is
    /// dated from the commit that moved it, and nothing is left at the old paths. See
    /// [`change_stream`] for why path is the identity here. A shape that answered differently
    /// would be the drift-gate defect #1171 was split from. A move that declares its origin
    /// is the next test.
    #[test]
    fn a_node_moved_by_git_mv_and_one_moved_by_delete_plus_add_answer_alike() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path();
        init(root);

        node(root, "concept/a.yml", "class: concept\nlinks: []\n");
        node(root, "concept/b.yml", "class: concept\nlinks: []\n");
        commit(root, "2026-01-01", "establish: a and b");

        crate::git::fixture::git(
            root,
            &[
                "mv",
                ".yidam/corpus/concept/a.yml",
                ".yidam/corpus/concept/a-moved.yml",
            ],
        );
        std::fs::remove_file(root.join(".yidam/corpus/concept/b.yml")).unwrap();
        node(root, "concept/b-moved.yml", "class: concept\nlinks: []\n");
        commit(root, "2026-01-09", "revise: move a and b");

        let since = uncited_age(root);
        for moved in ["a-moved", "b-moved"] {
            let age = since.get(&format!(".yidam/corpus/concept/{moved}.yml"));
            assert_eq!(
                age.map(|a| day(a.ts)),
                Some("2026-01-09".into()),
                "{since:?}"
            );
            assert_eq!(age.map(|a| a.commits), Some(1), "{since:?}");
        }
        assert!(
            !since.contains_key(".yidam/corpus/concept/a.yml"),
            "{since:?}"
        );
        assert!(
            !since.contains_key(".yidam/corpus/concept/b.yml"),
            "{since:?}"
        );
    }

    /// `a` and `b` are uncited open questions, written on the 1st and left alone through a
    /// second corpus commit.
    fn two_standing_questions(root: &Path) {
        init(root);
        node(root, "concept/a.yml", ASKING_A);
        node(root, "concept/b.yml", ASKING_B);
        commit(root, "2026-01-01", "open: whether a and b hold");
        node(root, "concept/c.yml", "class: concept\nlabel: C\n");
        commit(root, "2026-01-05", "establish: c");
    }

    const ASKING_A: &str = "class: concept\nlabel: A\ndescription: it is `[open]`\n";
    const ASKING_B: &str = "class: concept\nlabel: B\ndescription: it is `[open]`\n";

    /// Both ages, for one node, as (first day, commits).
    fn both_ages(root: &Path, rel: &str) -> [Option<(String, usize)>; 2] {
        let key = format!(".yidam/corpus/{rel}");
        [uncited_age(root), open_question_age(root)]
            .map(|ages| ages.get(&key).map(|a| (day(a.ts), a.commits)))
    }

    /// **A declared move carries both ages, however it is made** (#1180). `a` is moved with
    /// `git mv`. `b` is deleted and re-added with every line rewritten, so no similarity index
    /// could pair it. Each declares `moved-from:`, and each keeps the count it had: the
    /// rename is not a citation and does not answer the question.
    #[test]
    fn a_declared_move_carries_both_ages_by_git_mv_and_by_a_rewritten_delete_plus_add() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path();
        two_standing_questions(root);

        crate::git::fixture::git(
            root,
            &[
                "mv",
                ".yidam/corpus/concept/a.yml",
                ".yidam/corpus/concept/a-moved.yml",
            ],
        );
        node(
            root,
            "concept/a-moved.yml",
            &format!("{ASKING_A}moved-from: ../concept/a.yml\n"),
        );
        std::fs::remove_file(root.join(".yidam/corpus/concept/b.yml")).unwrap();
        node(
            root,
            "concept/b-moved.yml",
            "moved-from: ../concept/b.yml\nclass: concept\nlabel: Bee, restated\n\
             description: whether the restatement holds is still `[open]`\n",
        );
        commit(root, "2026-01-09", "migrate: a and b");

        for moved in ["concept/a-moved.yml", "concept/b-moved.yml"] {
            assert_eq!(
                both_ages(root, moved),
                [
                    Some(("2026-01-01".into(), 3)),
                    Some(("2026-01-01".into(), 3))
                ],
                "{moved} keeps the date and count it had before the move"
            );
        }
        for old in ["concept/a.yml", "concept/b.yml"] {
            assert_eq!(both_ages(root, old), [None, None], "{old} is gone");
        }
    }

    /// The delete and the add may be separate commits. The commit between them is counted,
    /// because nothing in it cited the node or answered it.
    #[test]
    fn a_declared_move_carries_across_separate_delete_and_add_commits() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path();
        two_standing_questions(root);

        std::fs::remove_file(root.join(".yidam/corpus/concept/a.yml")).unwrap();
        commit(root, "2026-01-09", "revise: a withdrawn");
        node(
            root,
            "concept/a-back.yml",
            &format!("{ASKING_A}moved-from: ../concept/a.yml\n"),
        );
        commit(root, "2026-01-12", "revise: a restored under a new name");

        assert_eq!(
            both_ages(root, "concept/a-back.yml"),
            [
                Some(("2026-01-01".into(), 4)),
                Some(("2026-01-01".into(), 4))
            ]
        );
    }

    /// A copy made while the original is still in the tree is a second node, and dates from
    /// its own commit. The original keeps its age.
    #[test]
    fn a_copy_declaring_a_node_still_present_starts_its_own_age() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path();
        two_standing_questions(root);

        node(
            root,
            "concept/a-copy.yml",
            &format!("{ASKING_A}moved-from: ../concept/a.yml\n"),
        );
        commit(root, "2026-01-09", "establish: a copy of a");

        assert_eq!(
            both_ages(root, "concept/a-copy.yml"),
            [
                Some(("2026-01-09".into(), 1)),
                Some(("2026-01-09".into(), 1))
            ]
        );
        assert_eq!(
            both_ages(root, "concept/a.yml"),
            [
                Some(("2026-01-01".into(), 3)),
                Some(("2026-01-01".into(), 3))
            ]
        );
    }

    /// Each move names only the path it left. One name is enough, because the fold carries
    /// the age forward at every step.
    #[test]
    fn a_chain_of_declared_moves_carries_to_the_end() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path();
        two_standing_questions(root);

        std::fs::remove_file(root.join(".yidam/corpus/concept/a.yml")).unwrap();
        node(
            root,
            "concept/a2.yml",
            &format!("{ASKING_A}moved-from: ../concept/a.yml\n"),
        );
        commit(root, "2026-01-09", "migrate: a → a2");
        std::fs::remove_file(root.join(".yidam/corpus/concept/a2.yml")).unwrap();
        node(
            root,
            "gauge/a3.yml",
            &format!("{ASKING_A}moved-from: ../concept/a2.yml\n"),
        );
        commit(root, "2026-01-12", "migrate: a2 → a3");

        assert_eq!(
            both_ages(root, "gauge/a3.yml"),
            [
                Some(("2026-01-01".into(), 4)),
                Some(("2026-01-01".into(), 4))
            ]
        );
    }

    /// A move carries only what was holding. `a` is moved in the commit that answers it, and
    /// `b` in the commit where something first cites it. Each age ends there: an orphan
    /// renamed so that it could be cited is cited, and no longer an orphan.
    #[test]
    fn a_move_that_answers_or_cites_ends_the_age_it_carried() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path();
        two_standing_questions(root);

        std::fs::remove_file(root.join(".yidam/corpus/concept/a.yml")).unwrap();
        node(
            root,
            "concept/a2.yml",
            "class: concept\nlabel: A\ndescription: it is `[verified]`\n\
             moved-from: ../concept/a.yml\n",
        );
        std::fs::remove_file(root.join(".yidam/corpus/concept/b.yml")).unwrap();
        node(
            root,
            "concept/b2.yml",
            &format!("{ASKING_B}moved-from: ../concept/b.yml\n"),
        );
        node(
            root,
            "concept/c.yml",
            "class: concept\nlabel: C\nlinks:\n  - target: ../concept/b2.yml\n",
        );
        commit(root, "2026-01-09", "migrate: a answered, b cited");

        let [uncited, open] = both_ages(root, "concept/a2.yml");
        assert_eq!(open, None, "a was answered in the commit that moved it");
        assert_eq!(
            uncited,
            Some(("2026-01-01".into(), 3)),
            "a is still uncited"
        );
        let [uncited, open] = both_ages(root, "concept/b2.yml");
        assert_eq!(uncited, None, "b is cited from the commit that moved it");
        assert_eq!(
            open,
            Some(("2026-01-01".into(), 3)),
            "b is still a question"
        );
    }

    // ── declared expectations ─────────────────────────────────────────────────
    //
    // What `replay`'s `by_class` rows are scored against. The contract's promise is that a
    // class which declared nothing carries `"meets_expectation": null` — scoring a class
    // against an expectation it never stated is how a corpus with an unfilled ontology gets
    // reported as failing — and `replay.rs` keeps that promise by one `declared.map(…)`, so
    // a missing entry here is exactly a `null` there.

    /// The fixture's own two classes, with `concept` stripped of its `edges:`.
    ///
    /// One class declaring nothing, one declaring a single `measured-by` edge naming it. The
    /// three cases below differ by `direction` alone, which is the whole of what is at issue.
    fn named_from_the_gauge(
        direction: Option<&str>,
    ) -> BTreeMap<String, Vec<crate::corpus::ClassEdge>> {
        use crate::corpus::ClassEdge;
        BTreeMap::from([
            ("concept".to_string(), Vec::new()),
            (
                "gauge".to_string(),
                vec![ClassEdge {
                    relationship: "measured-by".to_string(),
                    target: "concept".to_string(),
                    direction: direction.map(str::to_string),
                    description: String::new(),
                }],
            ),
        ])
    }

    /// The shape `replay` hands [`expectations_of`], built from a map the tests can write.
    fn views(
        decls: &BTreeMap<String, Vec<crate::corpus::ClassEdge>>,
    ) -> Vec<crate::corpus::EdgeView<'_>> {
        decls
            .iter()
            .map(|(name, edges)| crate::corpus::EdgeView { name, edges })
            .collect()
    }

    /// #659. `direction: in` on `gauge` says instances of `concept` point at a gauge — it
    /// NAMES `concept` and says nothing whatever about anything pointing at one.
    ///
    /// The reading used to be `edges.iter().any(|e| e.target == name)`, which cannot see the
    /// difference, so a `concept` declaring no edges of its own was read as expecting to be
    /// cited and scored against `uncited == 0` — a class that had declared nothing, judged
    /// against the one state the missing entry exists to represent.
    #[test]
    fn a_class_named_by_an_inbound_declaration_is_not_thereby_pointed_at() {
        let e = expectations_of(&views(&named_from_the_gauge(Some("in"))));
        assert_eq!(e.get("concept"), None, "{e:?}");
        // And the class that did declare is still scored: something points at a gauge.
        assert_eq!(e.get("gauge"), Some(&Expectation::Cited), "{e:?}");
    }

    /// The arm that must keep working, and the reason this is not fixed by deleting the
    /// branch: stated from the other end, the same relationship does point at `concept`.
    #[test]
    fn a_class_another_class_points_at_is_cited_though_it_declares_nothing() {
        let e = expectations_of(&views(&named_from_the_gauge(Some("out"))));
        assert_eq!(e.get("concept"), Some(&Expectation::Cited), "{e:?}");
    }

    /// A declaration with no `direction` names both ends, here as in `source_classes`: it
    /// says a relationship exists without saying which way it runs, and the safe reading of
    /// an ambiguous declaration is the one that does not silence a report. One rule, applied
    /// the same way to both questions — which is the whole of what #659 was about.
    #[test]
    fn a_declaration_with_no_direction_names_both_ends() {
        let e = expectations_of(&views(&named_from_the_gauge(None)));
        assert_eq!(e.get("concept"), Some(&Expectation::Cited), "{e:?}");
    }

    /// The same case through the walk that computes it, on class blobs read out of git —
    /// `replay`'s own path, and the one `by_class` is built from.
    #[test]
    fn a_class_declaring_nothing_is_scored_against_nothing() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path();
        init(root);

        // The fixture's own shape, minus concept's edges: `direction: in` is not
        // hypothetical, and `.yidam/corpus/concept.ont.yml` uses it.
        node(
            root,
            "gauge.ont.yml",
            "class: gauge\nedges:\n  - relationship: measured-by\n    target: concept\n    direction: in\n",
        );
        node(root, "concept.ont.yml", "class: concept\n");
        node(root, "concept/a.yml", "class: concept\nlinks: []\n");
        node(root, "gauge/g.yml", "class: gauge\nlinks: []\n");
        commit(
            root,
            "2026-01-01",
            "establish: concept named from the gauge's end",
        );

        let mut seen: Option<HashMap<String, Expectation>> = None;
        replay(root, |f| seen = Some(f.expectations.clone()));
        let e = seen.expect("the replay visited the commit");
        assert_eq!(e.get("concept"), None, "{e:?}");
        assert_eq!(e.get("gauge"), Some(&Expectation::Cited), "{e:?}");
    }

    // ── open questions ────────────────────────────────────────────────────────

    /// The reason this replays instead of reading the node's age.
    ///
    /// `a` is written on the 1st and does not become a question until the 9th. Node age
    /// would report eight days of unanswered question that nobody had yet asked.
    #[test]
    fn a_question_dates_from_when_it_became_one_and_not_from_authorship() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path();
        init(root);

        node(root, "concept/a.yml", "class: concept\nlabel: A\n");
        commit(root, "2026-01-01", "establish: a, answering nothing");

        node(
            root,
            "concept/a.yml",
            "class: concept\nlabel: A\ndescription: whether it holds is `[open]`\n",
        );
        commit(root, "2026-01-09", "open: whether a holds");

        let since = open_question_age(root);
        let age = since
            .get(".yidam/corpus/concept/a.yml")
            .expect("a is an open question at HEAD");
        assert_eq!(day(age.ts), "2026-01-09", "{since:?}");
        assert_eq!(age.commits, 1, "one corpus commit, HEAD inclusive");
    }

    /// A question answered and later reopened is a new question.
    ///
    /// Carrying the original date would report a corpus as having ignored for months
    /// something it in fact resolved and then reconsidered — which is the opposite of what
    /// happened, and in the unflattering direction.
    #[test]
    fn a_reopened_question_dates_from_the_reopening() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path();
        init(root);

        let asking = "class: concept\nlabel: A\ndescription: it is `[open]`\n";
        let settled = "class: concept\nlabel: A\ndescription: it is `[verified]`\n";

        node(root, "concept/a.yml", asking);
        commit(root, "2026-01-01", "open: whether a holds");
        node(root, "concept/a.yml", settled);
        commit(root, "2026-02-01", "close: a holds");
        node(root, "concept/a.yml", asking);
        commit(root, "2026-03-01", "open: a, reconsidered");

        let since = open_question_age(root);
        assert_eq!(
            since.get(".yidam/corpus/concept/a.yml").map(|a| day(a.ts)),
            Some("2026-03-01".to_string()),
            "{since:?}"
        );
    }

    /// An answered question carries no age at all.
    #[test]
    fn an_answered_question_is_not_in_the_map() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path();
        init(root);

        node(
            root,
            "concept/a.yml",
            "class: concept\nlabel: A\ndescription: it is `[open]`\n",
        );
        commit(root, "2026-01-01", "open: whether a holds");
        node(
            root,
            "concept/a.yml",
            "class: concept\nlabel: A\ndescription: it is `[verified]`\n",
        );
        commit(root, "2026-02-01", "close: a holds");

        assert!(
            !open_question_age(root).contains_key(".yidam/corpus/concept/a.yml"),
            "a is answered and must carry no age"
        );
    }

    /// The ontology is read at the commit, not at HEAD.
    ///
    /// `lead` gains a `type: claim` property in February. The node's `claim_tag: open` was
    /// sitting in a property that carried no tag until then, so the question begins in
    /// February — not in January, when a reading through today's ontology would place it.
    /// Getting this wrong ages every structurally-tagged question to the day its class was
    /// written, which is the direction that manufactures overdue findings.
    #[test]
    fn a_structural_tag_dates_from_when_its_class_declared_the_property() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path();
        init(root);

        let untyped = "class: lead\nproperties:\n  - name: claim_tag\n    type: string\n";
        let typed = "class: lead\nproperties:\n  - name: claim_tag\n    type: claim\n";

        std::fs::create_dir_all(root.join(".yidam/corpus")).unwrap();
        std::fs::write(root.join(".yidam/corpus/lead.ont.yml"), untyped).unwrap();
        node(
            root,
            "lead/x.yml",
            "class: lead\nlabel: X\nproperties:\n  claim_tag: open\n",
        );
        commit(root, "2026-01-01", "establish: x under an untyped lead");

        std::fs::write(root.join(".yidam/corpus/lead.ont.yml"), typed).unwrap();
        commit(root, "2026-02-01", "revise: lead's claim_tag carries a tag");

        let since = open_question_age(root);
        assert_eq!(
            since.get(".yidam/corpus/lead/x.yml").map(|a| day(a.ts)),
            Some("2026-02-01".to_string()),
            "read through today's ontology this would say 2026-01-01: {since:?}"
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
        crate::git::fixture::git_out(dir, &["rev-parse", rev])
    }
}
