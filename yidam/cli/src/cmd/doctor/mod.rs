//! `yidam doctor` — one command that answers whether this setup is sound.
//!
//! The questions a person asks when something is wrong had no single home. They were
//! spread across a stderr warning nobody reads twice ([`crate::paths::warn_if_shadowed`]),
//! a `continue-on-error` CI step (`mise run yidam-vendor-status`), three reports, and a
//! comment in `mise.toml` that nothing enforced. Every one of them has bitten a real
//! repository, and a new collaborator hits several before reaching anything the template
//! is actually for.
//!
//! Two properties are load-bearing.
//!
//! **It writes nothing.** That is worth stating because `status` looks like a read and is
//! not — it is in `cmd::regen::GENERATORS` and rewrites a README block in whatever
//! repository it is run against. `doctor` must be safe to point at a repository you only
//! mean to inspect, so its one dangerous check ([`Check::REGEN`]) borrows `regen --check`'s
//! non-writing mode rather than its own copy of the ten generators.
//!
//! **It does no network.** Staleness of the vendored prelude is reported from
//! `.yidam.toml`'s recorded commit date, which answers "how old is what I am running"
//! without asking an origin anything. Whether the origin has *moved* is a different
//! question, it costs a fetch, and `yidam-vendor-status` remains the place to ask it —
//! named as the remedy rather than performed here.
//!
//! # Two levels of wrong
//!
//! [`Verdict::Fail`] is wrong now: the graph will lie, or the binary answering is not the
//! one this repository pins. [`Verdict::Warn`] is worth knowing and routinely lived with —
//! a light `reports` install legitimately has no vector index, and a repository pinned
//! three months ago is not broken. Only `Fail` exits nonzero, because a doctor that goes
//! red on the normal state of a normal install is one people learn to pass `|| true`.
//! `--strict` collapses the distinction for a CI job that wants it.

use anyhow::Result;
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use crate::paths::{pinned_binary, yidam_bin_path, yidam_index_dir, Pinned};
use crate::provenance::MANIFEST;

mod corpora;
mod corpus;
mod governance;
mod index;
mod kuten;
mod offers;
mod pin;
mod scaffold;
mod vault;

use corpora::*;
use corpus::*;
use governance::*;
use index::*;
use kuten::*;
use offers::*;
use pin::*;
use scaffold::*;
use vault::*;

/// What one check concluded.
///
/// Serialized lowercase, because a consumer switching on `"fail"` should not have to know
/// Rust's capitalization habits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Verdict {
    /// Nothing to do.
    Ok,
    /// Actionable, but a normal state to be in. Does not affect the exit code unless
    /// `--strict`.
    Warn,
    /// Wrong now. Exits nonzero.
    Fail,
    /// Not answerable — almost always because the repository check already failed and
    /// there is nothing to answer *about*.
    Skipped,
    /// Every open offer on the check was declined against a decision record this repository
    /// holds (RFC-0066 §2). Counted apart from `warn`, and never affects the exit code: the
    /// question was asked and answered in writing.
    Declined,
}

impl Verdict {
    fn tag(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Warn => "warn",
            Self::Fail => "fail",
            Self::Skipped => "skip",
            Self::Declined => "decl",
        }
    }
}

/// What one check concluded, before it is attached to the question it answers.
///
/// Split from [`Check`] because the id and the wording belong to [`ROSTER`], which names
/// each question once. Every check function used to spell both on every return, and
/// [`diagnose`]'s early-return arm spelled them a second time — which is how `vault` came to
/// ask about "its vault" on one path and "its vaults" on the other, and how `policy` came to
/// be absent from one path entirely (#656).
struct Answer {
    verdict: Verdict,
    detail: String,
    remedy: Option<String>,
    /// The offers this check asked about, each open or adopted. Empty on a check that
    /// carries none, and on an offer check that had nothing to ask (RFC-0066 §1).
    offers: Vec<Offered>,
}

impl Answer {
    /// Nothing to do — and therefore **no remedy**, which is the point of this constructor
    /// taking none. `remedy` answers *what resolves this finding*, so a verdict with no
    /// finding has none to give.
    ///
    /// Advice worth reading on a green line goes in `detail`, which is rendered on every
    /// line. [`check_catalog`] has always put it there; [`check_prelude`]'s pointer at the
    /// one networked command and [`check_policy`]'s at `yidam policy test` now do too. Both
    /// used to be remedies on an `ok` verdict, which the text renderer dropped — so the
    /// reader they were written for never saw them — and every JSON consumer kept, which is
    /// how a healthy check came to carry an action (#656).
    fn ok(detail: impl Into<String>) -> Self {
        Self {
            verdict: Verdict::Ok,
            detail: detail.into(),
            remedy: None,
            offers: vec![],
        }
    }

    /// Not answerable, and why. Almost always because the repository check already failed;
    /// [`check_path`] with no PATH to read is the other case.
    fn skipped(why: impl Into<String>) -> Self {
        Self {
            verdict: Verdict::Skipped,
            detail: why.into(),
            remedy: None,
            offers: vec![],
        }
    }

    /// Actionable, and a normal state to be in.
    fn warn(detail: impl Into<String>, remedy: Option<&str>) -> Self {
        Self {
            verdict: Verdict::Warn,
            detail: detail.into(),
            remedy: remedy.map(str::to_string),
            offers: vec![],
        }
    }

    /// Wrong now. The remedy is optional because a check that could not be *computed* has
    /// nothing to suggest — see [`check_regen`]'s error arm.
    fn fail(detail: impl Into<String>, remedy: Option<&str>) -> Self {
        Self {
            verdict: Verdict::Fail,
            detail: detail.into(),
            remedy: remedy.map(str::to_string),
            offers: vec![],
        }
    }

    /// A check that carries offers. `held` is the detail when every one asked is adopted;
    /// each entry is an offer asked about, with its finding when it is open.
    ///
    /// Settled at once against no declines, so a check called on its own reads as it always
    /// did. [`Question::ask`] settles it again against the repository's `[doctor.declined]`.
    fn offers(held: impl Into<String>, offers: Vec<Offered>) -> Self {
        Self {
            verdict: Verdict::Ok,
            detail: held.into(),
            remedy: None,
            offers,
        }
        .settle(&Declines::default())
    }

    /// Judge each offer on its own, then form the verdict from what is left (RFC-0066 §1–§3).
    ///
    /// Collected first and judged after, so a decline of one offer cannot hide another open
    /// offer on the same check: `check_scaffold` once returned at the first unmarked region,
    /// and a decline built on that return would have let `scaffold-regions` silence the hooks.
    fn settle(mut self, declines: &Declines) -> Self {
        if self.offers.is_empty() {
            return self;
        }
        let held = self.offers.iter().all(|o| o.open.is_none());
        let base = if held {
            std::mem::take(&mut self.detail)
        } else {
            String::new()
        };
        let mut open = vec![];
        let mut remedies: Vec<String> = vec![];
        let mut declined = vec![];
        let mut notes = vec![];
        for o in &mut self.offers {
            let since = Offer::named(o.id).map_or("", |r| r.since);
            let decline = declines.declined.get(o.id);
            o.state = match (&o.open, decline) {
                (Some(_), Some(record)) => match declines.find(record) {
                    Some(path) => {
                        declined.push(format!("`{}` declined in {path}", o.id));
                        o.record = Some(path);
                        OfferState::Declined
                    }
                    None => OfferState::Unhonoured,
                },
                (Some(_), None) => OfferState::Open,
                (None, Some(_)) => {
                    notes.push(format!(
                        "`[doctor.declined] {}` is contradicted: this repository adopted it",
                        o.id
                    ));
                    OfferState::Adopted
                }
                (None, None) => OfferState::Adopted,
            };
            let Some(finding) = &o.open else { continue };
            if o.state == OfferState::Declined {
                continue;
            }
            let mut line = format!("{}; {}", finding.detail, declines.dated(since));
            if let (OfferState::Unhonoured, Some(record)) = (o.state, decline) {
                let _ = write!(
                    line,
                    "; declined against `{record}`, which this repository does not hold"
                );
                remedies.push(format!(
                    "record `{record}` in .yidam/decisions/, or drop `[doctor.declined] {}`",
                    o.id
                ));
            }
            remedies.push(finding.remedy.clone());
            open.push(line);
        }
        // A remedy another one begins with is the shorter half of the same instruction:
        // `yidam migrate scaffold` beside `yidam migrate scaffold, then mise run …`.
        let all = remedies.clone();
        remedies.retain(|r| !all.iter().any(|o| o != r && o.starts_with(r.as_str())));
        remedies.dedup();
        if !open.is_empty() {
            open.extend(declined);
            open.extend(notes);
            self.verdict = Verdict::Warn;
            self.detail = open.join("; ");
            self.remedy = Some(remedies.join("; "));
        } else if !declined.is_empty() {
            declined.extend(notes);
            self.verdict = Verdict::Declined;
            self.detail = declined.join("; ");
            self.remedy = None;
        } else {
            notes.insert(0, base);
            self.verdict = Verdict::Ok;
            self.detail = notes.join("; ");
            self.remedy = None;
        }
        self
    }
}

/// One question, its answer, and what to do about it.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Check {
    /// Stable identifier. A consumer keys on this; the prose is free to change.
    pub id: &'static str,
    /// The question in the form a person would ask it.
    pub question: &'static str,
    pub verdict: Verdict,
    /// What was actually found.
    pub detail: String,
    /// The command or edit that resolves this finding. **`None` on `ok` and on `skipped`**,
    /// by construction: [`Answer::ok`] and [`Answer::skipped`] take no remedy, so a healthy
    /// check cannot reach a consumer that reads `remedy != null` as *there is something to
    /// do*. Two did until #656 — the prose suppressed them at render time and the JSON did
    /// not, and the invariant was in neither contract.
    pub remedy: Option<String>,
    /// The offers this check raises, each with the release that made it and where it stands
    /// (RFC-0066). Empty on a check that carries none.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub offers: Vec<Offered>,
}

impl Check {
    const REPOSITORY: &'static str = "repository";
    const MODE: &'static str = "mode";
    const PROVENANCE: &'static str = "provenance";
    const BINARY: &'static str = "binary";
    const COMMIT: &'static str = "commit";
    const PATH: &'static str = "path";
    const PRELUDE: &'static str = "prelude";
    const INDEX: &'static str = "index";
    const REGEN: &'static str = "regen";
    const ROUTES: &'static str = "routes";
    const SCAFFOLD: &'static str = "scaffold";
    const CI: &'static str = "ci";
    const COMPUTED: &'static str = "computed";
    const STAMPS: &'static str = "run-output-stamps-input-commit";
    const BUILD: &'static str = "build";
    const CATALOG: &'static str = "catalog";
    const CORPORA: &'static str = "corpora";
    const VAULT: &'static str = "vault";
    const REMOTE_INDEX: &'static str = "remote-index";
    const POLICY: &'static str = "policy";
    const GOVERNANCE: &'static str = "governance";
    const KUTEN: &'static str = "kuten";
    const KUTEN_READ: &'static str = "kuten-read";
    const CORPUS: &'static str = "corpus";
    const CONTRACT: &'static str = "contract";

    /// The checks about surfaces a projection never has: a vendored prelude, regenerated
    /// template blocks, scaffolded CI (RFC-0064 §5). Reported `skipped`, never dropped.
    const NOT_PROJECTED: [&'static str; 6] = [
        Self::PRELUDE,
        Self::REGEN,
        Self::ROUTES,
        Self::SCAFFOLD,
        Self::STAMPS,
        Self::CI,
    ];
}

#[derive(Debug, serde::Serialize)]
pub struct DoctorReport {
    /// Whether the exit code will be zero. Computed here rather than left to the consumer,
    /// per RFC-0016: the CLI computes verdicts.
    pub passed: bool,
    /// Whether `--strict` was in force, so a consumer can tell a passing run from a
    /// leniently-passing one.
    pub strict: bool,
    pub failed: usize,
    pub warned: usize,
    /// Checks whose every open offer was declined against a record this repository holds.
    /// Never affects the exit code (RFC-0066 §2).
    pub declined: usize,
    /// Keys under `[doctor.declined]` naming no offer: a typo, or a defect, which cannot be
    /// declined. Judged only on a full run, since under `--only` no selected check owns them.
    /// `--strict` fails on one.
    pub unknown_declines: Vec<String>,
    pub checks: Vec<Check>,
}

impl DoctorReport {
    #[cfg(test)]
    fn new(checks: Vec<Check>, strict: bool) -> Self {
        Self::judged(checks, strict, vec![])
    }

    fn judged(checks: Vec<Check>, strict: bool, unknown_declines: Vec<String>) -> Self {
        let count = |v: Verdict| checks.iter().filter(|c| c.verdict == v).count();
        let (failed, warned, declined) = (
            count(Verdict::Fail),
            count(Verdict::Warn),
            count(Verdict::Declined),
        );
        Self {
            passed: failed == 0 && !(strict && (warned > 0 || !unknown_declines.is_empty())),
            strict,
            failed,
            warned,
            declined,
            unknown_declines,
            checks,
        }
    }
}

// ── the checks ────────────────────────────────────────────────────────────────

/// Is this a repository yidam bootstrapped?
///
/// The test is `.yidam/`, not corpus content — the same test [`crate::paths::require_yidam_repo`]
/// makes, and for the same reason: a repository bootstrapped an hour ago has the directory
/// and no nodes in it, and that is a legitimately empty corpus rather than an absent one.
///
/// Two more states live at this same test, and the point of naming them is that both used
/// to be reported as "this is not a repository" — an answer whose remedy destroys or repeats
/// work that has already been done.
///
/// The first: `.yidam/` holding real corpus content — class
/// definitions or decision records, not just an empty scaffold — with git's `HEAD` unborn
/// (#579): a bootstrap that ran the ontology dialogue, wrote class definitions and decision
/// records, and stopped before step 8's genesis commit. The whole model rests on git history
/// being the graph, so this is not a lesser version of "bootstrapped" — there is no graph
/// yet, only a directory that looks like one, and it is silent in exactly the way that
/// matters: indistinguishable from an empty directory to anyone who does not already know to
/// check. Reported distinctly so it reads as neither "not started" nor "done".
///
/// Gated on corpus content, not merely on `.yidam/` existing with an unborn `HEAD`, because
/// a git-initialized-but-uncommitted `.yidam/` with nothing under it is also what a good
/// many tests in this workspace use as an isolated sandbox for an unrelated check —
/// `vault.rs`'s `repo()` helper among them. Only a directory that actually looks like a
/// stopped bootstrap should read as one.
fn check_repository(root: &Path) -> Answer {
    if root.join(".yidam").is_dir() {
        if has_corpus_content(root) && head_is_unborn(root) {
            return Answer::fail(
                "bootstrapped but never committed — .yidam/ holds corpus content and HEAD \
                 has no commits yet",
                Some("finish bootstrap step 8: write the genesis commit"),
            );
        }
        return Answer::ok(format!("{}", root.display()));
    }
    // #914: `.yidam.toml` and no `.yidam/`, on an empty or single-commit history — what
    // `yidam clone` leaves behind. `warn`, because it is actionable and a normal state to be
    // in, which is the whole of what this constructor means. Reported as `fail` it told the
    // reader to run `yidam overlay .` on a repository that already carries the template, or
    // to derive one with the command they had just run.
    if crate::paths::is_unbootstrapped_clone(root) {
        return Answer::warn(
            format!(
                "{} is derived from yidam and not bootstrapped yet — .yidam.toml is pinned, \
                 and .yidam/ is not written until genesis",
                root.display()
            ),
            Some("open this repository with an agent and start at BOOTSTRAP.md"),
        );
    }
    // `Git::here()`: this asks about the process's own working directory, the way
    // [`crate::paths::repo_root`] does, so it is one of the two callers with no root to name.
    let in_git = crate::git::Git::here()
        .args(["rev-parse", "--show-toplevel"])
        .succeeded();
    let (detail, remedy) = if in_git {
        (
            format!("{} is a git repository with no .yidam/", root.display()),
            "yidam overlay .",
        )
    } else {
        (
            format!("{} is not inside a git repository", root.display()),
            "run this from inside a derived repository",
        )
    };
    Answer::fail(detail, Some(remedy))
}

/// Does `.yidam/` hold anything a bootstrap actually writes — a class definition or a
/// decision record — rather than being an empty scaffold?
///
/// This is the discriminator between a stopped bootstrap (#579) and the many test fixtures
/// in this workspace that create an empty, uncommitted `.yidam/` purely for isolation. A
/// real interrupted run leaves class files under `.yidam/corpus/` and records under
/// `.yidam/decisions/` — the twelve class definitions and two decision records the reporting
/// repository actually had.
fn has_corpus_content(root: &Path) -> bool {
    for dir in [".yidam/corpus", ".yidam/decisions"] {
        let has_entry = std::fs::read_dir(root.join(dir))
            .map(|mut entries| entries.next().is_some())
            .unwrap_or(false);
        if has_entry {
            return true;
        }
    }
    false
}

/// Is git's `HEAD` unborn — no commit made on the current branch yet?
///
/// Answerable offline, from local git alone, which `doctor` already shells to. Errs toward
/// `false` (assume born) on anything ambiguous — no git binary, not a work tree at all — so
/// an environment where the question cannot be answered does not manufacture a false
/// "never committed".
fn head_is_unborn(root: &Path) -> bool {
    let in_work_tree = crate::git::Git::new(root)
        .args(["rev-parse", "--is-inside-work-tree"])
        .succeeded();
    if !in_work_tree {
        return false;
    }
    // Note the asymmetry: an unrunnable git leaves `in_work_tree` false and returns above,
    // so reaching here means git ran. `succeeded()` collapses "git refused" and "git could
    // not run" into false, which here reads as an unborn HEAD — the same answer the old
    // `.unwrap_or(true)` gave, since it is reachable only when git already ran once.
    !crate::git::Git::new(root)
        .args(["rev-parse", "--verify", "-q"])
        .rev("HEAD")
        .succeeded()
}

/// Which features does this binary have?
///
/// Never a verdict — a light build is the recommended install. It is here because
/// "command not found" and "this binary cannot do that" are different diagnoses that look
/// identical from a script, and this is the line that separates them.
fn check_build() -> Answer {
    let b = crate::report::YidamBlock::current();
    Answer::ok(format!(
        "{} ({}) with features: {}",
        b.version,
        b.commit,
        b.features.join(", ")
    ))
}

// ── assembly ──────────────────────────────────────────────────────────────────

/// What the checks are answered about.
///
/// Carried as one value so every question has one signature and can sit in [`ROSTER`]
/// beside its id and its wording. `running` and `path_var` are held rather than read from
/// the environment so the two environment-sensitive checks are testable; production hands
/// them [`std::env::current_exe`] and `$PATH`.
struct Subject {
    root: PathBuf,
    running: Option<PathBuf>,
    path_var: Option<std::ffi::OsString>,
    today: i64,
    /// `[doctor.declined]`, applied to each check that carries offers.
    declines: Declines,
}

/// Whether a question can be answered about a directory that is not a derived repository.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Asked {
    /// Anywhere. `repository` *is* the question, and `build` is about the binary rather
    /// than about the repository — it is exactly what a person debugging "it says this is
    /// not a repository" needs to see.
    Anywhere,
    /// Only of a derived repository. Answering these against a directory that is not one
    /// produces confident nonsense — "no index", "no provenance" — that reads as a list of
    /// things to fix rather than as one thing. A `.yidam/` with an unborn HEAD (#579) fails
    /// the same test for a different reason and is just as unanswerable: most of these read
    /// git history, which does not exist yet either. So does a clone that has not been
    /// bootstrapped (#914), which is the one of the three that is not a fault — the
    /// repository check `warn`s, and [`diagnose`] keys on "not `Ok`" rather than on "failed"
    /// so that this state skips the roster too.
    ///
    /// Reported `skipped` and never dropped — a check that vanishes cannot be told from one
    /// that never ran, which is what the report contract says about `checks` and what
    /// `policy` did on this path until #656.
    OfARepository,
}

/// One question `doctor` asks: its stable id, its wording, and what answers it.
struct Question {
    id: &'static str,
    text: &'static str,
    asked: Asked,
    answer: fn(&Subject) -> Answer,
}

impl Question {
    /// Ask it — or report it `skipped`, when `unanswerable` says why.
    fn ask(&self, subject: &Subject, unanswerable: Option<&'static str>) -> Check {
        let answer = match unanswerable {
            Some(why) if self.asked == Asked::OfARepository => Answer::skipped(why),
            _ if Check::NOT_PROJECTED.contains(&self.id)
                && crate::projection::producer(&subject.root).is_some() =>
            {
                Answer::skipped("projection")
            }
            _ => (self.answer)(subject).settle(&subject.declines),
        };
        debug_assert!(
            answer
                .offers
                .iter()
                .all(|o| Offer::named(o.id).is_some_and(|r| r.check == self.id)),
            "`{}` raised an offer OFFERS gives another check",
            self.id
        );
        Check {
            id: self.id,
            question: self.text,
            verdict: answer.verdict,
            detail: answer.detail,
            remedy: answer.remedy,
            offers: answer.offers,
        }
    }
}

/// Every question `doctor` asks, in the order it reports them.
///
/// **One list.** There were two — this one, and a hand-written roster of ids and questions
/// the early-return path built when the repository check failed — and keeping them in step
/// was nobody's job. They had come apart in both available ways (#656): `policy` was in the
/// first and not the second, so on a directory that is not a derived repository the check
/// did not appear *at all* rather than appearing as `skipped`; and `vault` asked about "its
/// vault" on one path and "its vaults" on the other. `KUTEN_QUESTION` was a const invented
/// to stop exactly that drift for exactly one of the fourteen; this is that fix, generalized
/// by removing the second list rather than by mirroring it more carefully.
const ROSTER: &[Question] = &[
    Question {
        id: Check::REPOSITORY,
        text: "Am I in a derived repository?",
        asked: Asked::Anywhere,
        answer: |s| check_repository(&s.root),
    },
    Question {
        id: Check::MODE,
        text: "Is this corpus authored, or a projection?",
        asked: Asked::OfARepository,
        answer: |s| check_mode(&s.root),
    },
    Question {
        id: Check::PROVENANCE,
        text: "Does this repository record where it came from?",
        asked: Asked::OfARepository,
        answer: |s| check_provenance(&s.root),
    },
    Question {
        id: Check::BINARY,
        text: "Is the running binary the one this repository pins?",
        asked: Asked::OfARepository,
        answer: |s| check_binary(&s.root, s.running.as_deref()),
    },
    Question {
        id: Check::COMMIT,
        text: "Was the running binary built at the commit this repository pins?",
        asked: Asked::OfARepository,
        answer: |s| check_build_commit(&s.root, env!("YIDAM_BUILD_COMMIT")),
    },
    Question {
        id: Check::PATH,
        text: "Is `.yidam/bin` ahead on PATH?",
        asked: Asked::OfARepository,
        answer: |s| check_path(&s.root, s.path_var.as_deref()),
    },
    Question {
        id: Check::PRELUDE,
        text: "How stale is the vendored prelude?",
        asked: Asked::OfARepository,
        answer: |s| check_prelude(&s.root, s.today),
    },
    Question {
        id: Check::INDEX,
        text: "Is the index built, and is it current?",
        asked: Asked::OfARepository,
        answer: |s| check_index(&s.root),
    },
    Question {
        id: Check::COMPUTED,
        text: "What has this corpus computed about itself, and does it still stand?",
        asked: Asked::OfARepository,
        answer: |s| check_computed(&s.root),
    },
    Question {
        id: Check::STAMPS,
        text: "Does any committed output stamp the commit its inputs came from?",
        asked: Asked::OfARepository,
        answer: |s| check_stamps(&s.root),
    },
    Question {
        id: Check::REGEN,
        text: "Are the REGEN blocks current?",
        asked: Asked::OfARepository,
        answer: |s| check_regen(&s.root),
    },
    Question {
        id: Check::ROUTES,
        text: "Does `AGENTS.md` carry the reading routes a re-vendor updates?",
        asked: Asked::OfARepository,
        answer: |s| check_routes(&s.root),
    },
    Question {
        id: Check::SCAFFOLD,
        text: "Do `ci.yml` and `CLAUDE.md` mark the part a re-vendor updates?",
        asked: Asked::OfARepository,
        answer: |s| check_scaffold(&s.root),
    },
    Question {
        id: Check::CI,
        text: "Does this repository's CI run a corpus gate?",
        asked: Asked::OfARepository,
        answer: |s| check_ci(&s.root),
    },
    Question {
        id: Check::CATALOG,
        text: "Have any source records aged out?",
        asked: Asked::OfARepository,
        answer: |s| check_catalog(&s.root, s.today),
    },
    Question {
        id: Check::CORPORA,
        text: "Did the corpora this repository depends on arrive?",
        asked: Asked::OfARepository,
        answer: |s| check_corpora(&s.root),
    },
    Question {
        id: Check::CORPUS,
        text: "Can every corpus file be read?",
        asked: Asked::OfARepository,
        answer: |s| check_corpus(&s.root),
    },
    Question {
        id: Check::CONTRACT,
        text: "Has the ontology said what its classes require?",
        asked: Asked::OfARepository,
        answer: |s| check_contract(&s.root),
    },
    Question {
        id: Check::VAULT,
        text: "Can this repository reach its vaults?",
        asked: Asked::OfARepository,
        answer: |s| check_vault(&s.root),
    },
    Question {
        id: Check::REMOTE_INDEX,
        text: "Is the declared remote vector index usable from here?",
        asked: Asked::OfARepository,
        answer: |s| check_remote_index(&s.root),
    },
    Question {
        id: Check::POLICY,
        text: "Do this repository's own rules compile, and which are its own?",
        asked: Asked::OfARepository,
        answer: |s| check_policy(&s.root),
    },
    Question {
        id: Check::GOVERNANCE,
        text: "Is this repository's governance mode carrying its own weight?",
        asked: Asked::OfARepository,
        answer: |s| check_governance(&s.root),
    },
    Question {
        id: Check::KUTEN,
        text: "Which kuten does this repository hold, and at what revision?",
        asked: Asked::OfARepository,
        answer: |s| check_kuten(&s.root),
    },
    Question {
        id: Check::KUTEN_READ,
        text: "Can anything in the loop read the kuten this repository holds?",
        asked: Asked::OfARepository,
        answer: |s| check_kuten_read(&s.root),
    },
    Question {
        id: Check::BUILD,
        text: "Which yidam is this, and what can it do?",
        asked: Asked::Anywhere,
        answer: |_| check_build(),
    },
];

/// Run the checks against `root`.
///
/// `running` and `path_var` are passed rather than read so the two environment-sensitive
/// checks are testable; production hands them [`std::env::current_exe`] and `$PATH`.
///
/// `only` names the questions to report — every one when it is empty.
///
/// The repository question is still *asked* when it is not reported, because its answer is
/// what makes the rest unanswerable. Asking `contract` of a directory that is not a
/// repository should say `skipped`, as the full run does, and not read an ontology that is
/// not there.
pub(crate) fn diagnose(
    root: &Path,
    running: Option<&Path>,
    path_var: Option<&std::ffi::OsStr>,
    today: i64,
    only: &[&str],
) -> Vec<Check> {
    let subject = Subject {
        root: root.to_path_buf(),
        running: running.map(Path::to_path_buf),
        path_var: path_var.map(std::ffi::OsStr::to_os_string),
        today,
        declines: Declines::read(root),
    };
    // Set the moment the repository check fails, which is what makes the rest of the roster
    // unanswerable. Every later question sees it; the two that can be asked anywhere ignore
    // it. See [`Asked`] for why this is the discriminator.
    let mut unanswerable: Option<&'static str> = None;
    let mut checks = Vec::with_capacity(ROSTER.len());
    for question in ROSTER {
        let reported = only.is_empty() || only.contains(&question.id);
        if !reported && question.id != Check::REPOSITORY {
            continue;
        }
        let check = question.ask(&subject, unanswerable);
        // `!= Ok` rather than `== Fail`: since #914 the repository check has a third
        // answer, and it is a `warn`. A clone that has not been bootstrapped has no more
        // history to read than one that is not a repository at all, so the rest of the
        // roster is just as unanswerable — and answering it anyway would print fifteen
        // findings against a repository whose only real state is "start at BOOTSTRAP.md".
        if question.id == Check::REPOSITORY && check.verdict != Verdict::Ok {
            unanswerable = Some(if root.join(".yidam").is_dir() {
                "bootstrapped but never committed"
            } else if crate::paths::is_unbootstrapped_clone(root) {
                "derived from yidam, not bootstrapped yet"
            } else {
                "not a yidam repository"
            });
        }
        if reported {
            checks.push(check);
        }
    }
    checks
}

/// Which mode `.yidam/config.toml` declares — RFC-0064. An `ok` either way: both are normal
/// states. A malformed `[corpus]` table fails, since every command that reads the mode would.
fn check_mode(root: &Path) -> Answer {
    match crate::projection::mode(root) {
        Err(e) => Answer::fail(
            format!("{e:#}"),
            Some("`[corpus]` in .yidam/config.toml takes `mode = \"projection\"` and `generated_by`, or neither"),
        ),
        Ok(mode) => match mode.generated_by() {
            None => Answer::ok("authored — no `[corpus]` in .yidam/config.toml"),
            Some(by) => Answer::ok(format!(
                "projection, generated by `{by}` — `[corpus]` in .yidam/config.toml"
            )),
        },
    }
}

/// The width of the id column: the longest id in [`ROSTER`], so every line holds its id and
/// the detail column starts in one place. Read from the roster and not a literal because a
/// literal was 12 while `run-output-stamps-input-commit` is 30, and the next long id would
/// have been out of column the same way (#1626). From the roster and not from the report so
/// that `--only` does not move the column.
fn id_width() -> usize {
    ROSTER.iter().map(|q| q.id.len()).max().unwrap_or_default()
}

pub(crate) fn render(report: &DoctorReport, root: &Path) -> String {
    let mut out = format!("yidam doctor — {}\n\n", root.display());
    let width = id_width();
    for c in &report.checks {
        let _ = writeln!(
            out,
            "  {:<5} {:<width$} {}",
            c.verdict.tag(),
            c.id,
            c.detail
        );
        // Printed wherever there is one, which is only ever a `warn` or a `fail`: a remedy
        // under a green line is how a report becomes something people skim past, and that
        // rule is [`Answer::ok`]'s now. It used to be enforced a second time here, and the
        // two halves disagreed for as long as both existed — the suppression was the
        // renderer's alone, so the JSON carried what this hid (#656).
        if let Some(remedy) = &c.remedy {
            let _ = writeln!(out, "  {:<5} {:<width$} → {remedy}", "", "");
        }
    }
    out.push('\n');
    let declined = match report.declined {
        0 => String::new(),
        n => format!(" {n} declined, each against a record in .yidam/decisions/."),
    };
    match (report.failed, report.warned) {
        // Not "everything checks out" when `--strict` is about to fail on the configuration.
        (0, 0) if report.strict && !report.unknown_declines.is_empty() => {
            out.push_str("Every check passes, but `[doctor.declined]` does not.");
        }
        (0, 0) => out.push_str("Everything checks out."),
        (0, w) => {
            let _ = write!(
                out,
                "{w} warning(s), nothing broken.{}",
                if report.strict {
                    " --strict: exiting nonzero."
                } else {
                    ""
                }
            );
        }
        (f, 0) => {
            let _ = write!(out, "{f} failing check(s).");
        }
        (f, w) => {
            let _ = write!(out, "{f} failing check(s), {w} warning(s).");
        }
    }
    out.push_str(&declined);
    // Last, because it is about the configuration rather than the repository: a key that
    // names no offer is a decline doing nothing, and the repository would never learn it.
    if !report.unknown_declines.is_empty() {
        let ids: Vec<&str> = OFFERS.iter().map(|o| o.id).collect();
        let _ = write!(
            out,
            "\n\n`[doctor.declined]` names no offer called {} — the offers are {}. A defect \
             cannot be declined.{}",
            report
                .unknown_declines
                .iter()
                .map(|k| format!("`{k}`"))
                .collect::<Vec<_>>()
                .join(", "),
            ids.join(", "),
            if report.strict {
                " --strict: exiting nonzero."
            } else {
                ""
            }
        );
    }
    out
}

/// The ids `only` may name, each one a [`ROSTER`] id. An unknown one is an error and not an
/// empty report: `--only contrat` answering nothing would read as a clean bill of health.
fn selected(only: &[String]) -> Result<Vec<&'static str>> {
    only.iter()
        .map(|want| {
            ROSTER
                .iter()
                .map(|q| q.id)
                .find(|id| id == want)
                .ok_or_else(|| {
                    let ids: Vec<&str> = ROSTER.iter().map(|q| q.id).collect();
                    anyhow::anyhow!(
                        "no doctor check `{want}`; the checks are: {}",
                        ids.join(", ")
                    )
                })
        })
        .collect()
}

/// `yidam doctor`. Read-only, and exits nonzero on anything actionable.
///
/// `only` narrows the report to the named checks. `yidam-vendor-update` asks `contract` this
/// way, so a re-vendor raises the question a corpus written before the contract existed was
/// never asked (#1078).
pub fn doctor(
    root: Option<&std::path::Path>,
    strict: bool,
    only: &[String],
    format: crate::report::Format,
) -> Result<()> {
    let only = selected(only)?;
    let root = crate::paths::resolve_root(root)?;
    let running = std::env::current_exe().ok();
    let path_var = std::env::var_os("PATH");
    let checks = diagnose(
        &root,
        running.as_deref(),
        path_var.as_deref(),
        crate::dates::today_days(),
        &only,
    );
    // Under `--only` no selected check owns an unknown key, so only a full run judges one:
    // `yidam-vendor-update`'s `--only contract --strict` must not fail on a typo elsewhere.
    let unknown = if only.is_empty() {
        Declines::read(&root).unknown()
    } else {
        vec![]
    };
    let report = DoctorReport::judged(checks, strict, unknown);
    let passed = report.passed;

    crate::report::gate(&root, format, report, passed, |r| {
        println!("{}", render(r, &root))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::fixture::git;
    use tempfile::TempDir;

    fn find<'a>(checks: &'a [Check], id: &str) -> &'a Check {
        checks.iter().find(|c| c.id == id).expect("check present")
    }

    /// One question from [`ROSTER`], asked the way [`diagnose`] asks it. `unanswerable` is
    /// what the repository check's failure would have set.
    fn ask(id: &str, unanswerable: Option<&'static str>) -> Check {
        let subject = Subject {
            root: PathBuf::from("/r"),
            running: None,
            path_var: None,
            today: 20_000,
            declines: Declines::default(),
        };
        ROSTER
            .iter()
            .find(|q| q.id == id)
            .expect("a question with that id")
            .ask(&subject, unanswerable)
    }

    /// The page both docs gates read, spelled once.
    fn troubleshooting_md() -> String {
        std::fs::read_to_string(
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/troubleshooting.md"),
        )
        .expect("docs/troubleshooting.md")
    }

    /// The body of the recorded `$ yidam doctor` run, found by its command line.
    ///
    /// By the command rather than by position: the page opens with this block today, and a
    /// gate that assumed so would start reading a different block the day something is
    /// written above it — and pass, because every other fence on the page is also output.
    fn doctor_transcript() -> String {
        troubleshooting_md()
            .split("```console")
            .skip(1)
            .map(|f| f.split("```").next().unwrap_or_default().to_string())
            .find(|body| body.lines().any(|l| l.trim() == "$ yidam doctor"))
            .expect("a ```console fence whose command is `$ yidam doctor`")
    }

    impl Verdict {
        /// Every variant, for the test that reads a verdict back by its tag.
        /// [`all_verdicts_lists_every_variant`] makes a variant missing here a compile error.
        const ALL: [Self; 5] = [
            Self::Ok,
            Self::Warn,
            Self::Fail,
            Self::Skipped,
            Self::Declined,
        ];
    }

    /// **`Verdict::ALL` holds every variant.**
    ///
    /// The `match` has no wildcard, so a sixth variant fails to compile here until it is
    /// given a position, and the position check then fails until it is in `ALL`.
    #[test]
    fn all_verdicts_lists_every_variant() {
        let position = |v: Verdict| match v {
            Verdict::Ok => 0,
            Verdict::Warn => 1,
            Verdict::Fail => 2,
            Verdict::Skipped => 3,
            Verdict::Declined => 4,
        };
        let positions: Vec<usize> = Verdict::ALL.iter().map(|v| position(*v)).collect();
        assert_eq!(positions, (0..positions.len()).collect::<Vec<_>>());
        assert_eq!(
            positions.len(),
            5,
            "a variant is missing from `Verdict::ALL`"
        );
    }

    /// A line of every verdict, `decl` included, is read back as a verdict line.
    #[test]
    fn every_verdict_tag_reads_back_as_a_verdict_line() {
        for v in Verdict::ALL {
            let line = format!("  {:<5} {:<30} some detail", v.tag(), "kuten");
            assert_eq!(
                verdict_line_id(&line),
                Some("kuten"),
                "`{}` is not read as a verdict line",
                v.tag()
            );
        }
        assert_eq!(verdict_line_id("                     → remedy"), None);
        assert_eq!(verdict_line_id("2 failing check(s)."), None);
    }

    /// Every id fits its column and the detail starts in one place on every line.
    #[test]
    fn rendered_detail_column_is_aligned() {
        let width = id_width();
        assert!(ROSTER.iter().all(|q| q.id.len() <= width));
        let checks = ROSTER
            .iter()
            .map(|q| Check {
                id: q.id,
                question: q.text,
                verdict: Verdict::Ok,
                detail: "detail".into(),
                remedy: None,
                offers: Vec::new(),
            })
            .collect();
        let report = DoctorReport::new(checks, false);
        let text = render(&report, Path::new("/x"));
        let starts: Vec<usize> = text
            .lines()
            .filter(|l| verdict_line_id(l).is_some())
            .map(|l| l.find("detail").unwrap_or_default())
            .collect();
        assert_eq!(starts.len(), ROSTER.len());
        assert!(starts.iter().all(|s| *s == starts[0]), "{starts:?}");
    }

    /// The check id on a verdict line, or `None` for anything else in the transcript.
    ///
    /// A verdict line is `  <verdict>  <id>  <detail>`; a remedy continues with `→` and the
    /// summary line starts with a count, so the first word decides.
    fn verdict_line_id(line: &str) -> Option<&str> {
        let mut words = line.split_whitespace();
        let verdict = words.next()?;
        Verdict::ALL
            .iter()
            .any(|v| v.tag() == verdict)
            .then(|| words.next())
            .flatten()
    }

    /// **Every question `doctor` asks is in the table `docs/troubleshooting.md` prints.**
    ///
    /// That table was a hand-written list and it had already fallen four behind — `vault`,
    /// `policy`, `governance` and `kuten` were in [`ROSTER`] and not in the docs, and
    /// nothing went red as they were added. A list nobody checks stops covering what is new
    /// without ever failing, so the set is discovered from `ROSTER` here rather than typed
    /// a third time.
    ///
    /// Ids and order, not wording: the docs are free to format a question (`PATH` is
    /// code-quoted there and not in the roster) and the id is what a consumer keys on.
    #[test]
    fn every_doctor_question_is_in_the_troubleshooting_table() {
        let docs = troubleshooting_md();

        // Bounded to the one table, by its header and then by its own last row. Scanning
        // the whole document picked up a second table further down whose first column is
        // also code-quoted — and a filter loose enough to exclude those rows by their shape
        // would be loose enough to drop a real id and pass.
        let table = docs
            .split("| Check | The question |")
            .nth(1)
            .expect("the check table, headed `| Check | The question |`");
        // The table ends where markdown says it ends: the blank line after its last row.
        let table = table.split("\n\n").next().unwrap_or(table);
        let documented: Vec<String> = table
            .lines()
            .filter_map(|l| l.strip_prefix("| `"))
            .filter_map(|l| l.split('`').next())
            .map(str::to_string)
            .collect();
        let asked: Vec<&str> = ROSTER.iter().map(|q| q.id).collect();

        assert_eq!(
            documented, asked,
            "docs/troubleshooting.md's check table does not match the roster"
        );
    }

    /// **And so is every question in the transcript above that table.**
    ///
    /// [`every_doctor_question_is_in_the_troubleshooting_table`] bounds itself to the table,
    /// by design — and the recorded `$ yidam doctor` run immediately above it was from
    /// `0.5.0` and showed ten of the sixteen. A reader scrolling up from a correct table met
    /// the exact drift that table had already been repaired for, and nothing could see it: a
    /// recorded transcript is a hardcoded claim written beside the thing that produces it,
    /// and the gate stopped at the table's header.
    ///
    /// Re-recording it fixes today and rots on the next check added, so the transcript is
    /// held to [`ROSTER`] the same way the table is. It can be, because it says the same
    /// thing in a different column: a verdict, then the id. Ids and order, not wording — the
    /// run illustrates one repository's state and its detail text is whatever that
    /// repository produced.
    #[test]
    fn every_doctor_question_is_in_the_recorded_transcript() {
        let transcript = doctor_transcript();
        let shown: Vec<&str> = transcript.lines().filter_map(verdict_line_id).collect();
        let asked: Vec<&str> = ROSTER.iter().map(|q| q.id).collect();

        assert_eq!(
            shown, asked,
            "the `$ yidam doctor` transcript in docs/troubleshooting.md does not show the \
             roster — re-record it against a real run"
        );
    }

    pub(super) fn derived_repo() -> TempDir {
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join(".yidam")).unwrap();
        // The inherited task layer, which is what makes `mise run yidam-vendor-update`
        // a thing that can be run here — and so what `check_provenance` and
        // `check_prelude` read to tell a derived repository from a corpus that was
        // copied out of one. A fixture without it is not the thing it is named after.
        std::fs::write(tmp.path().join(TASK_LAYER), "[yidam-vendor-update]\n").unwrap();
        tmp
    }

    // ── projection mode (RFC-0064 §5) ────────────────────────────────────────

    #[test]
    fn a_projection_skips_the_vendoring_checks_and_says_its_mode() {
        let tmp = derived_repo();
        let subject = |root: &Path| Subject {
            root: root.to_path_buf(),
            running: None,
            path_var: None,
            today: 20_000,
            declines: Declines::default(),
        };
        let authored = check_mode(tmp.path());
        assert_eq!(authored.verdict, Verdict::Ok);
        assert!(
            authored.detail.starts_with("authored"),
            "{}",
            authored.detail
        );

        std::fs::write(
            tmp.path().join(".yidam/config.toml"),
            "[corpus]\nmode = \"projection\"\ngenerated_by = \"site-scraper\"\n",
        )
        .unwrap();
        let projected = check_mode(tmp.path());
        assert_eq!(projected.verdict, Verdict::Ok);
        assert!(
            projected.detail.contains("`site-scraper`"),
            "the mode line names the producer: {}",
            projected.detail
        );

        let s = subject(tmp.path());
        for question in ROSTER {
            let check = question.ask(&s, None);
            if Check::NOT_PROJECTED.contains(&question.id) {
                assert_eq!(check.verdict, Verdict::Skipped, "{}", question.id);
                assert_eq!(check.detail, "projection", "{}", question.id);
            } else {
                assert_ne!(
                    check.detail, "projection",
                    "{} is not a vendoring surface and still runs",
                    question.id
                );
            }
        }

        std::fs::write(
            tmp.path().join(".yidam/config.toml"),
            "[corpus]\nmode = \"projection\"\n",
        )
        .unwrap();
        assert_eq!(check_mode(tmp.path()).verdict, Verdict::Fail);
    }

    // ── dates ────────────────────────────────────────────────────────────────

    #[test]
    fn the_epoch_is_day_zero() {
        assert_eq!(crate::dates::days_from_civil_str("1970-01-01"), Some(0));
    }

    /// The round trip against `cmd::status`'s forward conversion. If these two ever
    /// disagree, a pin date and an index date are being measured on different calendars.
    #[test]
    fn days_and_dates_round_trip() {
        for date in ["2026-07-01", "2000-02-29", "1999-12-31", "2026-08-23"] {
            let days = crate::dates::days_from_civil_str(date).unwrap();
            let secs = (days as u64) * 86400;
            assert_eq!(
                crate::cmd::status::unix_to_date_str(secs),
                date,
                "round trip failed for {date}"
            );
        }
    }

    #[test]
    fn an_unknown_pin_date_is_not_a_date() {
        assert_eq!(crate::dates::days_from_civil_str("unknown"), None);
        assert_eq!(crate::dates::days_from_civil_str(""), None);
        assert_eq!(crate::dates::days_from_civil_str("2026-13-01"), None);
    }

    // ── contract ─────────────────────────────────────────────────────────────

    pub(super) fn class(name: &str, body: &str) -> crate::corpus::Class {
        crate::corpus::Class::parse(format!(".yidam/corpus/{name}.ont.yml"), body)
    }

    pub(super) const EDGE: &str = "edges:\n  - relationship: part-of\n    target: place\n";

    pub(super) fn property(required: &str) -> String {
        format!("properties:\n  - name: series\n    type: string\n{required}")
    }

    // ── --only ───────────────────────────────────────────────────────────────

    /// `--only` reports what it names and nothing else, and the repository question still
    /// decides whether the rest can be answered — so a directory that is not a repository
    /// gets `skipped`, as a full run would say, and no ontology is read.
    #[test]
    fn only_reports_what_it_names_and_still_asks_the_repository() {
        let tmp = TempDir::new().unwrap();
        let checks = diagnose(tmp.path(), None, None, 20_000, &[Check::CONTRACT]);
        let ids: Vec<&str> = checks.iter().map(|c| c.id).collect();
        assert_eq!(ids, [Check::CONTRACT]);
        assert_eq!(checks[0].verdict, Verdict::Skipped);
    }

    /// An id that names nothing is an error. An empty report would pass, and read as a clean
    /// bill of health on a question nobody asked.
    #[test]
    fn only_refuses_an_unknown_check() {
        let err = selected(&["contrat".to_string()]).unwrap_err().to_string();
        assert!(err.contains("`contrat`"), "{err}");
        assert!(err.contains("contract"), "{err}");
        assert_eq!(
            selected(&["contract".to_string()]).unwrap(),
            [Check::CONTRACT]
        );
    }

    // ── repository ───────────────────────────────────────────────────────────

    /// The case this whole command exists for: run it somewhere that is not a derived
    /// repository and get one answer, not seven.
    ///
    /// **Every question is still reported**, and against [`ROSTER`] rather than against a
    /// list written here — which is the whole repair. `policy` was missing from the
    /// hand-written roster this path used to build, so it did not appear at all rather than
    /// appearing as `skipped`, and the two states are exactly what the report contract says
    /// `checks` must keep apart (#656). A list in this test would have been the third copy
    /// and would have gone on agreeing with whichever one it was written from.
    #[test]
    fn outside_a_derived_repository_only_the_first_question_is_answered() {
        let tmp = TempDir::new().unwrap();
        let checks = diagnose(tmp.path(), None, None, 20_000, &[]);

        let asked: Vec<&str> = checks.iter().map(|c| c.id).collect();
        let roster: Vec<&str> = ROSTER.iter().map(|q| q.id).collect();
        assert_eq!(
            asked, roster,
            "a question that vanishes here cannot be told from one that never ran"
        );

        assert_eq!(find(&checks, Check::REPOSITORY).verdict, Verdict::Fail);
        for question in ROSTER.iter().filter(|q| q.asked == Asked::OfARepository) {
            assert_eq!(
                find(&checks, question.id).verdict,
                Verdict::Skipped,
                "{} should not be answered outside a repository",
                question.id
            );
        }
        // Which binary is answering is knowable anywhere, and is exactly what a person
        // debugging "it says this is not a repository" needs to see.
        assert_eq!(find(&checks, Check::BUILD).verdict, Verdict::Ok);
        assert!(!DoctorReport::new(checks, false).passed);
    }

    /// #579: `.yidam/` present, class definitions on disk, and `HEAD` unborn — a bootstrap
    /// that stopped before step 8's genesis commit. This must read as its own state, not as
    /// "not a repository" (which would tell someone to run `yidam overlay .` and destroy
    /// what is actually there) and not as `Ok` (there is no graph — history is the graph).
    #[test]
    fn a_dot_yidam_with_an_unborn_head_is_its_own_state() {
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join(".yidam/corpus")).unwrap();
        std::fs::write(
            tmp.path().join(".yidam/corpus/thing.ont.yml"),
            "class: thing\n",
        )
        .unwrap();
        git(tmp.path(), &["init", "-q", "-b", "main"]);
        // Deliberately no `git add`, no commit: HEAD stays unborn.

        let c = check_repository(tmp.path());
        assert_eq!(c.verdict, Verdict::Fail, "detail was: {}", c.detail);
        assert!(
            c.detail.contains("never committed"),
            "detail should name the state, not just fail: {}",
            c.detail
        );
        assert!(
            c.remedy.as_deref().unwrap_or_default().contains("genesis"),
            "remedy should point at the genesis commit: {:?}",
            c.remedy
        );

        // And the rest of the checks are skipped for the same reason as "not a repository"
        // — most of them read git history that does not exist yet either — but the *why*
        // must not claim there is no `.yidam/` here, since there plainly is one.
        let checks = diagnose(tmp.path(), None, None, 20_000, &[]);
        let regen = find(&checks, Check::REGEN);
        assert_eq!(regen.verdict, Verdict::Skipped);
        assert!(!DoctorReport::new(checks, false).passed);
    }

    /// The committed sibling of the test above: a real genesis commit resolves `HEAD`, and
    /// the repository check goes back to answering the question it always answered.
    #[test]
    fn a_dot_yidam_with_a_born_head_is_ok() {
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join(".yidam")).unwrap();
        std::fs::write(tmp.path().join(".yidam/marker"), "").unwrap();
        crate::git::fixture::repo(tmp.path(), "genesis: test");

        let c = check_repository(tmp.path());
        assert_eq!(c.verdict, Verdict::Ok, "detail was: {}", c.detail);
    }

    /// [`derived_repo`] is a bare directory with no git at all — not the unborn-HEAD case,
    /// which requires an actual git work tree. `head_is_unborn` must say `false` here rather
    /// than misreading "no git" as "never committed".
    #[test]
    fn a_derived_repo_fixture_with_no_git_is_not_reported_as_unborn() {
        let tmp = derived_repo();
        assert!(!head_is_unborn(tmp.path()));
        assert_eq!(check_repository(tmp.path()).verdict, Verdict::Ok);
    }

    /// #914: `.yidam.toml` present, no `.yidam/`, nothing committed — exactly what
    /// `yidam clone` leaves behind, and the first thing its reader runs.
    ///
    /// This used to read `fail  repository  … is a git repository with no .yidam/` with the
    /// remedy `yidam overlay .`, which overlays the template onto a repository that already
    /// carries it, and the sixteen checks behind it reported nothing at all rather than
    /// skipping. A clone is not a broken repository; it is an unstarted one.
    #[test]
    fn a_fresh_clone_is_derived_but_not_yet_bootstrapped() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(
            tmp.path().join(crate::provenance::MANIFEST),
            "[yidam]\ncommit = \"abc1234\"\n",
        )
        .unwrap();
        git(tmp.path(), &["init", "-q", "-b", "main"]);
        // Deliberately no commit: `clone` runs `git init` and commits nothing.

        let c = check_repository(tmp.path());
        assert_eq!(c.verdict, Verdict::Warn, "detail was: {}", c.detail);
        assert!(
            c.detail.contains("not bootstrapped yet"),
            "the detail must name the state: {}",
            c.detail
        );
        assert!(
            c.remedy
                .as_deref()
                .unwrap_or_default()
                .contains("BOOTSTRAP.md"),
            "the remedy must point at the dialogue, not at the command already run: {:?}",
            c.remedy
        );

        let checks = diagnose(tmp.path(), None, None, 20_000, &[]);
        // Every question is still reported, and the ones that read history are skipped with
        // a reason that does not claim this is not a repository.
        for question in ROSTER.iter().filter(|q| q.asked == Asked::OfARepository) {
            let check = find(&checks, question.id);
            assert_eq!(
                check.verdict,
                Verdict::Skipped,
                "{} was answered against a repository with no corpus yet",
                question.id
            );
            assert!(
                check.detail.contains("not bootstrapped yet"),
                "{} skipped for the wrong reason: {}",
                question.id,
                check.detail
            );
        }
        // And it is not a red build. Nothing is wrong; something has not happened yet.
        assert!(
            DoctorReport::new(checks, false).passed,
            "a clone nobody has bootstrapped is not a failing repository"
        );
    }

    /// The commit-count condition, measured. A repository with a real history and no
    /// `.yidam/` has *lost* something, and telling its owner to run a bootstrap dialogue
    /// would be the wrong instruction. Without this the predicate would relabel every
    /// repository that happens to carry a `.yidam.toml` as brand new.
    #[test]
    fn a_repository_with_a_history_and_no_dot_yidam_is_not_a_fresh_clone() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(
            tmp.path().join(crate::provenance::MANIFEST),
            "[yidam]\ncommit = \"abc1234\"\n",
        )
        .unwrap();
        git(tmp.path(), &["init", "-q", "-b", "main"]);
        git(tmp.path(), &["config", "user.email", "doctor@yidam.test"]);
        git(tmp.path(), &["config", "user.name", "Doctor"]);
        for n in ["one", "two", "three"] {
            std::fs::write(tmp.path().join(n), "").unwrap();
            git(tmp.path(), &["add", "-A"]);
            git(tmp.path(), &["commit", "-q", "-m", n]);
        }

        assert!(!crate::paths::is_unbootstrapped_clone(tmp.path()));
        let c = check_repository(tmp.path());
        assert_eq!(c.verdict, Verdict::Fail, "detail was: {}", c.detail);
    }

    /// A template copied by hand, with no `git init`. `is_unbootstrapped_clone` requires a
    /// work tree, so this keeps the "not inside a git repository" message it needs — the
    /// remedy there is `git init`, not a bootstrap dialogue.
    #[test]
    fn a_pin_with_no_git_at_all_is_not_a_fresh_clone() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(
            tmp.path().join(crate::provenance::MANIFEST),
            "[yidam]\ncommit = \"abc1234\"\n",
        )
        .unwrap();
        assert!(!crate::paths::is_unbootstrapped_clone(tmp.path()));
    }

    pub(super) fn path_var(dirs: &[&Path]) -> std::ffi::OsString {
        std::env::join_paths(dirs.iter().map(|p| p.to_path_buf())).unwrap()
    }

    // ── report ───────────────────────────────────────────────────────────────

    /// Checks built past [`Answer`], because these are about how a report *counts* its
    /// verdicts and not about what any question answered.
    pub(super) fn checks_with(verdicts: &[Verdict]) -> Vec<Check> {
        verdicts
            .iter()
            .map(|v| Check {
                id: "x",
                question: "q",
                verdict: *v,
                detail: "d".into(),
                remedy: Some("r".into()),
                offers: vec![],
            })
            .collect()
    }

    /// A light install with no index is the recommended install. Going red on it is how a
    /// doctor gets `|| true` appended to it forever.
    #[test]
    fn warnings_alone_do_not_fail_the_run() {
        let r = DoctorReport::new(checks_with(&[Verdict::Ok, Verdict::Warn]), false);
        assert!(r.passed);
        assert_eq!(r.warned, 1);
        assert_eq!(r.failed, 0);
    }

    #[test]
    fn strict_promotes_warnings_to_a_nonzero_exit() {
        let r = DoctorReport::new(checks_with(&[Verdict::Ok, Verdict::Warn]), true);
        assert!(!r.passed);
    }

    #[test]
    fn a_single_failure_fails_the_run() {
        let r = DoctorReport::new(checks_with(&[Verdict::Ok, Verdict::Fail]), false);
        assert!(!r.passed);
    }

    #[test]
    fn a_skipped_check_is_neither_a_warning_nor_a_failure() {
        let r = DoctorReport::new(checks_with(&[Verdict::Skipped]), true);
        assert!(r.passed);
        assert_eq!((r.failed, r.warned), (0, 0));
    }

    /// Remedies are the reason to read this output, and a remedy under a green line is
    /// noise. Asserted at construction, which is where the rule now lives: the renderer used
    /// to drop a remedy it was handed on an `ok` verdict, so the rule held for the prose and
    /// for nothing else — the same check reached a JSON consumer with an action on it
    /// (#656). A verdict with no finding now has no remedy to render.
    #[test]
    fn a_verdict_with_no_finding_carries_no_remedy() {
        assert!(Answer::ok("fine").remedy.is_none());
        assert!(Answer::skipped("not answerable").remedy.is_none());

        let skipped = ask(Check::PRELUDE, Some("not a yidam repository"));
        assert!(skipped.remedy.is_none(), "{skipped:?}");
        let text = render(&DoctorReport::new(vec![skipped], false), Path::new("/r"));
        assert!(
            !text.contains('→'),
            "nothing is owed, so nothing is offered:\n{text}"
        );
    }

    /// And a remedy that *is* owed is still printed.
    #[test]
    fn a_remedy_is_rendered_where_something_is_wrong() {
        let checks = vec![Check {
            id: "red",
            question: "q",
            verdict: Verdict::Fail,
            detail: "broken".into(),
            remedy: Some("print-me".into()),
            offers: vec![],
        }];
        let text = render(&DoctorReport::new(checks, false), Path::new("/r"));
        assert!(text.contains("→ print-me"), "{text}");
    }

    #[test]
    fn the_summary_line_distinguishes_clean_from_merely_unbroken() {
        let clean = render(
            &DoctorReport::new(checks_with(&[Verdict::Ok]), false),
            Path::new("/r"),
        );
        assert!(clean.contains("Everything checks out."), "{clean}");
        let warned = render(
            &DoctorReport::new(checks_with(&[Verdict::Warn]), false),
            Path::new("/r"),
        );
        assert!(warned.contains("nothing broken"), "{warned}");
    }

    /// A repository with a decisions directory holding `records`, and declines naming them.
    pub(super) fn declining(
        records: &[&str],
        declined: &[(&str, &str)],
        pin: Option<&str>,
    ) -> (TempDir, Declines) {
        let tmp = derived_repo();
        let dir = crate::paths::yidam_decisions_dir(tmp.path());
        std::fs::create_dir_all(&dir).unwrap();
        for r in records {
            std::fs::write(
                dir.join(format!("{r}.yml")),
                "summary: Declined.\nrationale: The argument.\n",
            )
            .unwrap();
        }
        let declines = Declines {
            declined: declined
                .iter()
                .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
                .collect(),
            decisions: dir,
            pin: pin.map(str::to_string),
        };
        (tmp, declines)
    }
}
