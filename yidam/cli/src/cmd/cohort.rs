//! `yidam cohort` — what a set of derived repositories says about the prelude they inherited.
//!
//! [`docs/post-genesis-measurement.md`](../../../../docs/post-genesis-measurement.md) is a
//! person reading three derived repositories by hand: reconstructing historical states,
//! running each repository's own binary, and assembling series in a scratch directory. It
//! found things nothing else could — that a rising orphan rate was mostly ontology shape and
//! not decay, that source classes are exactly derivable from `edges[].direction`, that a
//! status line was wrong three ways at once.
//!
//! **It is not a loop.** The difference between an eval loop and an anecdote is whether the
//! evidence can be regenerated when the prelude changes, and that document cannot be. This is
//! the same reading as a command.
//!
//! # The output is evidence about the prelude, not about any one corpus
//!
//! That framing is in the report's shape and not only in its prose: the rows are **norms**
//! and the columns are repositories. A norm that every derivation fails is a finding about
//! the norm — not five findings about five corpora — and a report organised the other way
//! round would have to be transposed by eye before it said so.
//!
//! `yidam replay` and `yidam kuten check` answer about *a* corpus and are the right
//! instruments for one. Neither can see that a rule lost everywhere.
//!
//! # Read-only, and that is what makes it safe
//!
//! The hand-run touched no working tree, against live repositories somebody was working in.
//! Every reading here is a `git log`, a `git for-each-ref`, or a walk of committed files;
//! nothing checks anything out and nothing writes. [`tests/cohort.rs`] asserts it against a
//! dirty tree rather than by inspection, which is the only way that claim is worth making.
//!
//! # Two controls, or the clusters are artifacts
//!
//! **Vendored-prelude vintage.** A derived repository works from the prelude it vendored, not
//! from current upstream, so a repository whose `GRAPH.md` predates the closed vocabulary has
//! not *broken* a rule it never held. [`Vintage`] is the existing mechanism and is reused
//! rather than re-derived — `kuten check` already states the rule this follows: *a metric the
//! repository's vendored prelude could not have produced is reported as vintage and never as
//! divergence.*
//!
//! **Repository maturity.** Members are ordered by authored commit count and the count is
//! printed beside every row, which is `kuten.yml`'s own `measured.members` convention: raw
//! counts, ordered by history length, so the arithmetic between a count and a conclusion is
//! done on numbers the reader can see. No rate here is normalised by age, because the
//! `classes` slot retired from that profile is the record of what happens when one is — two
//! bands that measured how old a repository is and called it practice.
//!
//! # A negative result is a result
//!
//! Every norm is reported, including the ones that held, because *"a negative result about
//! coverage is the only durable record that coverage was checked."* A report that listed only
//! failures would be a shorter report about a different thing, and the commit vocabulary — the
//! control case that makes the whole argument legible — would vanish from it the moment it
//! started working.
//!
//! And an occasion that never arose is neither. A repository that has never run a phase cannot
//! have failed to delete its branch, so it is **unmeasurable** rather than compliant, and the
//! denominator every share is quoted over is the number of repositories the question could be
//! asked of.
//!
//! # Lettered, not named
//!
//! Most derived repositories are private or unpublished, so naming one in a report that will
//! be pasted into a public issue discloses it. Members are lettered, which is the convention
//! this repository already uses for downstream findings and the one `kuten.yml`'s member table
//! follows. `--paths` opts back in for the person who ran the command and already holds the
//! paths.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::cmd::replay::ReplayRow;
use crate::kuten::{measure, Measurement, Verdict, Vintage};
use crate::paths::resolve_root;

// ── the norms ─────────────────────────────────────────────────────────────────

/// One rule the prelude states, and where it states it.
///
/// **This list is what the instrument can read, and it is not the set of norms the prelude
/// states.** A rule written upstream tomorrow is invisible here until somebody adds a row,
/// and no discovery closes that — a norm is prose, and which sentences are rules is a
/// judgement. What *is* closed is the other direction: `statement` is quoted from `document`,
/// and `tests/cohort.rs` holds every quote to the file it names. A norm edited out of the
/// prelude, or reworded, fails that test rather than quietly measuring a rule nobody states.
pub struct Norm {
    pub id: &'static str,
    /// The prelude document that states it, relative to `yidam/prelude/`.
    pub document: &'static str,
    /// The words that state it, verbatim. Held to `document` by the test suite.
    pub statement: &'static str,
    /// What is being read, in one line, for a reader who will not open the document.
    pub reads: &'static str,
}

pub const NORMS: &[Norm] = &[
    Norm {
        id: "commit-vocabulary",
        document: "GRAPH.md",
        statement: "Every commit's subject line begins `<verb>: `. The verb determines the \
                    commit's type. This list is closed",
        reads: "every authored commit leads with a verb in the closed list, counted whole \
                rather than register-scoped",
    },
    Norm {
        id: "phase-branch-deleted",
        document: "PHASES.md",
        statement: "A merged branch left behind is not a record of anything",
        reads: "no merged `phase/*` ref is still standing",
    },
    Norm {
        id: "cli-installed",
        document: "guidelines/directories.md",
        statement: "The `yidam` binary this repository runs, installed by `mise run \
                    yidam-build` from the commit `.yidam.toml` pins.",
        reads: "`.yidam/bin/yidam` exists, so the repository can run a gate against itself",
    },
    Norm {
        id: "vendored-prelude-unedited",
        document: "guidelines/directories.md",
        statement: "inherited yidam prelude; not modified in derived repos",
        reads: "no commit outside `vendor:`/`consume:`/`genesis:` touched `.yidam/.vendor/`",
    },
];

// ── one repository's reading ──────────────────────────────────────────────────

/// What one norm came to in one repository.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Reading {
    pub norm: &'static str,
    pub verdict: Verdict,
    /// The count behind the verdict, in the norm's own units. Present whether it held or not:
    /// `0 of 27` and `26 of 27` are the same sentence, and only one of them is a finding.
    pub evidence: String,
}

/// The endpoints of a corpus health series, and its shape.
///
/// Endpoints rather than the series: ten full series is a document, not a report, and
/// `yidam replay` is the instrument for one corpus. What a cohort adds is whether the shape
/// is shared.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Series {
    pub rows: usize,
    pub first_date: String,
    pub first_nodes: usize,
    pub first_orphans: usize,
    pub last_date: String,
    pub last_nodes: usize,
    pub last_orphans: usize,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct Member {
    /// `A`, `B`, … assigned by history length. Never a name — see the module doc.
    pub letter: String,
    /// Only with `--paths`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// The maturity control, printed beside every row rather than divided out of one.
    pub authored: usize,
    pub nodes: usize,
    pub open_questions: usize,
    pub vintage: Vintage,
    pub series: Option<Series>,
    /// `(classes behaving as declared, classes declaring)` at the last corpus commit. A
    /// measurement, never a verdict — see [`class_expectations`].
    pub declaring_classes_met: Option<(usize, usize)>,
    pub readings: Vec<Reading>,
}

/// A path that is not a derived repository, and why. Reported rather than dropped.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Skipped {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    pub reason: String,
}

/// How one norm stands across the cohort.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Standing {
    pub norm: &'static str,
    pub document: &'static str,
    pub reads: &'static str,
    pub held: usize,
    pub lost: usize,
    /// Members whose vendored prelude could not have produced the rule. Never `lost`.
    pub vintage: usize,
    /// Members the question could not be asked of — the occasion never arose.
    pub unmeasurable: usize,
}

impl Standing {
    /// The denominator every share is quoted over: repositories the question could be asked
    /// of, never repositories read.
    pub fn occasions(&self) -> usize {
        self.held + self.lost
    }
}

#[derive(Debug, serde::Serialize)]
pub struct CohortReport {
    pub read: usize,
    pub members: Vec<Member>,
    pub skipped: Vec<Skipped>,
    pub norms: Vec<Standing>,
}

pub struct Options {
    pub format: crate::report::Format,
    /// Print the repository paths beside the letters. Off by default: most derived
    /// repositories are private, and this report is written to be pasted somewhere.
    pub paths: bool,
}

// ── the readings ──────────────────────────────────────────────────────────────

fn git(root: &Path, args: &[&str]) -> Option<String> {
    crate::git::Git::new(root).args(args).try_run()
}

/// `phase/*` refs that are merged ancestors of `HEAD`, and how many exist at all.
fn phase_refs(root: &Path) -> (usize, usize) {
    let all = git(
        root,
        &[
            "for-each-ref",
            "--format=%(refname:short)",
            "refs/heads/phase/",
        ],
    )
    .map(|s| s.lines().filter(|l| !l.is_empty()).count())
    .unwrap_or(0);
    let merged = git(
        root,
        &[
            "for-each-ref",
            "--format=%(refname:short)",
            "--merged",
            "HEAD",
            "refs/heads/phase/",
        ],
    )
    .map(|s| s.lines().filter(|l| !l.is_empty()).count())
    .unwrap_or(0);
    (all, merged)
}

/// Commits that edited the vendored prelude outside the three acts licensed to.
///
/// `vendor:` re-vendors it, `consume:` is the transient bootstrap layer, and `genesis:` puts
/// it there. Anything else is a derived repository editing the copy it was told not to —
/// which is the shape of divergence that costs the most, because the next re-vendor silently
/// reverts it.
fn vendor_edits(root: &Path) -> Option<usize> {
    let log = git(root, &["log", "--format=%s", "--", ".yidam/.vendor"])?;
    let lines: Vec<&str> = log.lines().filter(|l| !l.trim().is_empty()).collect();
    if lines.is_empty() {
        return None; // no vendored prelude in this history; nothing to ask
    }
    Some(
        lines
            .iter()
            .filter(|s| {
                let verb = s.split([':', '(']).next().unwrap_or("").trim();
                !matches!(verb, "vendor" | "consume" | "genesis")
            })
            .count(),
    )
}

/// Classes that declared what being pointed at means for them, and how many behave as
/// declared, at the last commit that touched the corpus.
///
/// **Reported as a measurement and never as a norm, and the first draft of this file had it
/// the other way round.** Scored as a rule — held when every declaring class has no uncited
/// instance — it read `lost` in twelve of the thirteen repositories that could be asked, and
/// the number was real. The rule was not: GRAPH.md argues the opposite two sections later, in
/// terms, and a check written against it would have been a check against the document it
/// cited.
///
/// > The measurable quantity is residence time, not level. A node uncited for five commits is
/// > a sweep in progress and entirely healthy. A node uncited for two hundred is
/// > over-collection.
///
/// So "some instance of a declaring class is uncited today" is a snapshot of a corpus
/// mid-sweep, and calling it a lost norm would have published a finding about every corpus
/// that is working. What the number is good for is the comparison it makes possible — which
/// is why it is printed beside each member rather than dropped.
fn class_expectations(series: &[ReplayRow]) -> Option<(usize, usize)> {
    let last = series.last()?;
    let declaring: Vec<_> = last
        .by_class
        .values()
        .filter_map(|c| c.meets_expectation)
        .collect();
    (!declaring.is_empty()).then(|| (declaring.iter().filter(|m| **m).count(), declaring.len()))
}

fn read_one(root: &Path, m: &Measurement, vintage: &Vintage) -> Vec<Reading> {
    let mut out = Vec::new();

    // Vintage-gated: a repository whose vendored GRAPH.md does not declare the list closed
    // was never told there was one to break.
    out.push(if !vintage.vocabulary_is_closed {
        Reading {
            norm: "commit-vocabulary",
            verdict: Verdict::Vintage,
            evidence: if vintage.graph_present {
                "the vendored GRAPH.md does not declare the vocabulary closed".into()
            } else {
                "no vendored GRAPH.md to read".into()
            },
        }
    } else if m.commits == 0 {
        Reading {
            norm: "commit-vocabulary",
            verdict: Verdict::Unmeasurable,
            evidence: "no authored commits".into(),
        }
    } else {
        Reading {
            norm: "commit-vocabulary",
            verdict: verdict_of(m.off_vocabulary_commits == 0),
            evidence: format!(
                "{} of {} authored commits off-vocabulary, {} of them a verb wearing a scope",
                m.off_vocabulary_commits, m.commits, m.suffixed_commits
            ),
        }
    });

    // The occasion is a phase having been run, not the repository existing. Counted from the
    // commits that settle one, so a repository that ran phases and deleted every branch is
    // distinguishable from one that never ran a phase — which a ref count alone cannot do.
    let (all_refs, merged_refs) = phase_refs(root);
    out.push(if m.phase_commits == 0 && all_refs == 0 {
        Reading {
            norm: "phase-branch-deleted",
            verdict: Verdict::Unmeasurable,
            evidence: "no phase has been settled here".into(),
        }
    } else {
        Reading {
            norm: "phase-branch-deleted",
            verdict: verdict_of(merged_refs == 0),
            evidence: format!(
                "{merged_refs} of {all_refs} phase/* ref(s) merged and still standing, \
                 across {} phase commit(s)",
                m.phase_commits
            ),
        }
    });

    let installed = root.join(".yidam/bin/yidam").exists();
    out.push(Reading {
        norm: "cli-installed",
        verdict: verdict_of(installed),
        evidence: if installed {
            ".yidam/bin/yidam is present".into()
        } else {
            "no .yidam/bin/yidam — this repository has never run a gate against itself".into()
        },
    });

    out.push(match vendor_edits(root) {
        None => Reading {
            norm: "vendored-prelude-unedited",
            verdict: Verdict::Unmeasurable,
            evidence: "no commit has ever touched .yidam/.vendor/".into(),
        },
        Some(n) => Reading {
            norm: "vendored-prelude-unedited",
            verdict: verdict_of(n == 0),
            evidence: format!("{n} commit(s) edited the vendored prelude outside a re-vendor"),
        },
    });

    out
}

fn verdict_of(held: bool) -> Verdict {
    if held {
        Verdict::Conforming
    } else {
        Verdict::Divergent
    }
}

/// `A`, `B`, … `Z`, `AA`, `AB`. Bijective base-26, so no member is ever unlabelled.
fn letter(mut i: usize) -> String {
    let mut out = Vec::new();
    loop {
        out.push(b'A' + (i % 26) as u8);
        if i < 26 {
            break;
        }
        i = i / 26 - 1;
    }
    out.reverse();
    String::from_utf8(out).unwrap_or_else(|_| "?".into())
}

// ── the command ───────────────────────────────────────────────────────────────

pub fn collect(roots: &[PathBuf], show_paths: bool) -> CohortReport {
    let mut skipped = Vec::new();
    let mut read = Vec::new();

    for given in roots {
        let named = |p: &Path| show_paths.then(|| p.display().to_string());
        let Ok(root) = resolve_root(Some(given)) else {
            skipped.push(Skipped {
                path: named(given),
                reason: "the path could not be resolved".into(),
            });
            continue;
        };
        if !root.join(".yidam").is_dir() {
            skipped.push(Skipped {
                path: named(given),
                reason: "no .yidam/ at or above this path — not a derived repository".into(),
            });
            continue;
        }
        if git(&root, &["rev-parse", "HEAD"]).is_none() {
            skipped.push(Skipped {
                path: named(&root),
                reason: "no commits, so there is no history to read".into(),
            });
            continue;
        }
        read.push(root);
    }

    // Ordered by history length, and by path within a tie so two runs over one cohort letter
    // it the same way. `kuten.yml`'s member table is ordered the same way, for the same
    // reason: maturity is the first thing a reader has to hold constant.
    let mut measured: Vec<(PathBuf, Measurement)> = read
        .into_iter()
        .map(|r| (measure(&r), r))
        .map(|(m, r)| (r, m))
        .collect();
    measured.sort_by(|a, b| (a.1.commits, &a.0).cmp(&(b.1.commits, &b.0)));

    let mut members = Vec::new();
    for (i, (root, m)) in measured.into_iter().enumerate() {
        let vintage = Vintage::of_repo(&root);
        let rows = crate::cmd::replay::collect(&root);
        let readings = read_one(&root, &m, &vintage);
        let series = match (rows.first(), rows.last()) {
            (Some(f), Some(l)) => Some(Series {
                rows: rows.len(),
                first_date: f.date.clone(),
                first_nodes: f.nodes,
                first_orphans: f.orphans,
                last_date: l.date.clone(),
                last_nodes: l.nodes,
                last_orphans: l.orphans,
            }),
            _ => None,
        };
        members.push(Member {
            declaring_classes_met: class_expectations(&rows),
            letter: letter(i),
            path: show_paths.then(|| root.display().to_string()),
            authored: m.commits,
            nodes: m.nodes,
            open_questions: m.open_questions,
            vintage,
            series,
            readings,
        });
    }

    let norms = NORMS
        .iter()
        .map(|n| {
            let mut s = Standing {
                norm: n.id,
                document: n.document,
                reads: n.reads,
                held: 0,
                lost: 0,
                vintage: 0,
                unmeasurable: 0,
            };
            for r in members
                .iter()
                .flat_map(|m| &m.readings)
                .filter(|r| r.norm == n.id)
            {
                match r.verdict {
                    Verdict::Conforming => s.held += 1,
                    Verdict::Divergent => s.lost += 1,
                    Verdict::Vintage => s.vintage += 1,
                    Verdict::Unmeasurable => s.unmeasurable += 1,
                }
            }
            s
        })
        .collect();

    CohortReport {
        read: members.len(),
        members,
        skipped,
        norms,
    }
}

/// **Exits zero, always.** A norm that lost is a question about the norm, and a command that
/// gated on it would make one repository's practice a defect in another's build — which is
/// `kuten check`'s argument and `due`'s before it.
pub fn cohort(root: Option<&std::path::Path>, roots: &[PathBuf], opts: Options) -> Result<()> {
    let report = collect(roots, opts.paths);
    if opts.format.is_json() {
        // The envelope's `root` is this repository — the prelude the cohort is evidence
        // about — and not any member of it. That is the subject of the report.
        return crate::report::emit(&crate::paths::resolve_root(root)?, report);
    }
    println!("{}", render(&report));
    Ok(())
}

pub(crate) fn render(r: &CohortReport) -> String {
    let mut out = String::new();
    if r.read == 0 {
        out.push_str("No derived repository was read.\n");
        for (i, s) in r.skipped.iter().enumerate() {
            let _ = writeln!(
                out,
                "  {} — {}",
                s.path
                    .as_deref()
                    .map_or_else(|| format!("path {}", i + 1), str::to_string),
                s.reason
            );
        }
        return out;
    }

    let _ = writeln!(
        out,
        "{} repositor{}, lettered by authored commit count.\n",
        r.read,
        if r.read == 1 { "y" } else { "ies" }
    );
    for m in &r.members {
        let _ = write!(
            out,
            "  {:<3} {:>5} commits  {:>4} nodes  {:>4} open",
            m.letter, m.authored, m.nodes, m.open_questions
        );
        if !m.vintage.vocabulary_is_closed {
            out.push_str("   [vintage: the vocabulary it holds is not closed]");
        }
        if let Some(p) = &m.path {
            let _ = write!(out, "   {p}");
        }
        out.push('\n');
        match &m.series {
            Some(s) => {
                let _ = writeln!(
                    out,
                    "      {} → {}   orphans {} of {} → {} of {}   ({} rows)",
                    s.first_date,
                    s.last_date,
                    s.first_orphans,
                    s.first_nodes,
                    s.last_orphans,
                    s.last_nodes,
                    s.rows
                );
            }
            // Not omitted, because the omission is the finding. A corpus git does not track
            // has nodes on disk and no history to replay, so the node count above and the
            // series disagree — and one of the eighteen is exactly that: a declared
            // non-vendoring consumer whose `.yidam/corpus/` is a git-ignored projection from
            // its own exporter. A blank line here would have read as a short history.
            None => {
                let _ = writeln!(
                    out,
                    "      no series: no commit in this history touched a tracked corpus file, \
                     so the {} node(s) above are on disk and not in git",
                    m.nodes
                );
            }
        }
        // A measurement and not a verdict — see `class_expectations` for why this is not a
        // norm, and for what scoring it as one reported.
        if let Some((met, declaring)) = m.declaring_classes_met {
            let _ = writeln!(
                out,
                "      {met} of {declaring} class(es) that declare an inbound edge have every \
                 instance cited today",
            );
        }
    }

    out.push_str(
        "\n  The last line of each member is a measurement and not a score. GRAPH.md argues \
         that the\n  quantity worth reading is how long a node has been uncited and not how \
         many are today —\n  a corpus mid-sweep is doing exactly what a sweep does — so no \
         norm is scored on it here.\n",
    );

    if !r.skipped.is_empty() {
        let _ = write!(out, "\n{} path(s) read nothing:\n", r.skipped.len());
        for s in &r.skipped {
            let _ = writeln!(
                out,
                "  {} — {}",
                s.path.as_deref().unwrap_or("a path"),
                s.reason
            );
        }
    }

    out.push_str("\nNorms — what the prelude asked for, and what its derivations did.\n\n");
    for s in &r.norms {
        let occasions = s.occasions();
        let tag = if occasions == 0 {
            "unasked".to_string()
        } else if s.lost == 0 {
            format!("held {}/{}", s.held, occasions)
        } else {
            format!("lost {}/{}", s.lost, occasions)
        };
        let _ = writeln!(out, "  [{tag:^11}] {:<26} {}", s.norm, s.document);
        let _ = writeln!(out, "                {}", s.reads);
        let mut aside = Vec::new();
        if s.vintage > 0 {
            aside.push(format!(
                "{} could not have held it — the prelude they vendored does not state it",
                s.vintage
            ));
        }
        if s.unmeasurable > 0 {
            aside.push(format!("{} never had the occasion", s.unmeasurable));
        }
        if !aside.is_empty() {
            let _ = writeln!(out, "                ({})", aside.join("; "));
        }
        // The count behind every loss, per member. A verdict with no number under it is a
        // claim the reader cannot check, and a share quoted over a table nobody publishes is
        // how two of `kuten.yml`'s four bands shipped excluding their own evidence.
        for m in &r.members {
            for reading in m.readings.iter().filter(|x| x.norm == s.norm) {
                if reading.verdict == Verdict::Divergent {
                    let _ = writeln!(out, "                  {} — {}", m.letter, reading.evidence);
                }
            }
        }
        out.push('\n');
    }

    // Two thresholds, and neither is a tuned number. *Every* occasion is the strongest form
    // of the claim. *More than half* is the plain statement that most of the repositories
    // which could have kept a rule did not — the reading #288 asks for, a norm three
    // derivations all fail being a finding about the norm — without a cutoff chosen to make
    // it come out a particular way. Under three occasions there is no cohort to generalise
    // from and neither sentence is said.
    let everywhere: Vec<&Standing> = r
        .norms
        .iter()
        .filter(|s| s.occasions() >= 3 && s.held == 0)
        .collect();
    let mostly: Vec<&Standing> = r
        .norms
        .iter()
        .filter(|s| s.occasions() >= 3 && s.held > 0 && s.lost * 2 > s.occasions())
        .collect();

    if everywhere.is_empty() && mostly.is_empty() {
        out.push_str(
            "No norm lost in most of the repositories the question could be asked of. That is \
             the\ndurable record that coverage was checked, and it is the only form that \
             answer can take.",
        );
        return out;
    }
    out.push_str("What this says about the prelude rather than about any corpus:\n\n");
    for s in everywhere {
        let _ = writeln!(
            out,
            "  · `{}` lost in every one of the {} repositories that could have kept it.",
            s.norm,
            s.occasions()
        );
    }
    for s in mostly {
        let _ = writeln!(
            out,
            "  · `{}` lost in {} of the {} repositories that could have kept it.",
            s.norm,
            s.lost,
            s.occasions()
        );
    }
    out.push_str(
        "\nA rule most of its derivations do not keep is a finding about the rule. It needs a \
         mechanism\nthat answers during the act, a different rule, or withdrawal — and which \
         of those is a question\nfor whoever wrote it, which is why this exits zero.",
    );
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn letters_are_assigned_without_running_out() {
        assert_eq!(letter(0), "A");
        assert_eq!(letter(25), "Z");
        assert_eq!(letter(26), "AA");
        assert_eq!(letter(27), "AB");
        // Every index gets a distinct label, which is the whole requirement.
        let all: std::collections::BTreeSet<String> = (0..1000).map(letter).collect();
        assert_eq!(all.len(), 1000);
    }

    fn standing(held: usize, lost: usize, vintage: usize, unmeasurable: usize) -> Standing {
        Standing {
            norm: "n",
            document: "d",
            reads: "r",
            held,
            lost,
            vintage,
            unmeasurable,
        }
    }

    /// The denominator is occasions, not repositories — the distinction the whole report
    /// turns on, because a rule nobody had the chance to break is not a rule that held.
    #[test]
    fn the_denominator_excludes_members_the_question_was_not_asked_of() {
        assert_eq!(standing(3, 2, 4, 5).occasions(), 5);
        assert_eq!(standing(0, 0, 9, 9).occasions(), 0);
    }

    fn report(norms: Vec<Standing>) -> CohortReport {
        CohortReport {
            read: 2,
            members: vec![],
            skipped: vec![],
            norms,
        }
    }

    #[test]
    fn a_norm_that_lost_everywhere_is_named_as_a_finding_about_the_rule() {
        let text = render(&report(vec![Standing {
            norm: "phase-branch-deleted",
            ..standing(0, 5, 0, 3)
        }]));
        assert!(text.contains("lost 5/5"), "{text}");
        assert!(text.contains("lost in every one of the 5"), "{text}");
        assert!(text.contains("a finding about the rule"), "{text}");
        assert!(text.contains("3 never had the occasion"), "{text}");
    }

    /// The other threshold: most of the repositories that could have kept it did not. The
    /// first cohort run turned on exactly this case, so a report that only said something
    /// when a rule lost *everywhere* would have said nothing about a rule lost 12 of 13.
    #[test]
    fn a_norm_most_of_a_cohort_lost_is_named_too() {
        let text = render(&report(vec![Standing {
            norm: "cli-installed",
            ..standing(4, 6, 0, 0)
        }]));
        assert!(text.contains("lost 6/10"), "{text}");
        assert!(text.contains("lost in 6 of the 10"), "{text}");
    }

    /// And a rule most of a cohort *kept* is not dressed up as one they did not.
    #[test]
    fn a_norm_a_minority_lost_is_reported_without_a_finding() {
        let text = render(&report(vec![Standing {
            norm: "vendored-prelude-unedited",
            ..standing(12, 1, 0, 0)
        }]));
        assert!(text.contains("lost 1/13"), "{text}");
        assert!(text.contains("No norm lost in most"), "{text}");
    }

    /// A rule asked of exactly one repository is not evidence about the prelude.
    #[test]
    fn one_repository_losing_is_not_a_finding_about_the_norm() {
        let text = render(&report(vec![Standing {
            norm: "cli-installed",
            ..standing(0, 1, 0, 0)
        }]));
        assert!(text.contains("lost 1/1"), "{text}");
        assert!(text.contains("No norm lost in most"), "{text}");
    }
}
