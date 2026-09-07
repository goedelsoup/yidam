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

use crate::paths::{pinned_binary, repo_root, yidam_bin_path, yidam_index_dir, Pinned};
use crate::provenance::MANIFEST;

/// How old a pin may get before it is worth a word.
///
/// Arbitrary, and deliberately generous. The prelude is not milk; a repository pinned in
/// the spring and untouched since is not misconfigured. The number exists so that
/// "pinned some time ago" becomes a date a person can weigh, and so that a repository
/// nobody has re-vendored in a season says so once rather than never.
const STALE_PRELUDE_DAYS: i64 = 90;

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
}

impl Verdict {
    fn tag(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Warn => "warn",
            Self::Fail => "fail",
            Self::Skipped => "skip",
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
        }
    }

    /// Not answerable, and why. Almost always because the repository check already failed;
    /// [`check_path`] with no PATH to read is the other case.
    fn skipped(why: impl Into<String>) -> Self {
        Self {
            verdict: Verdict::Skipped,
            detail: why.into(),
            remedy: None,
        }
    }

    /// Actionable, and a normal state to be in.
    fn warn(detail: impl Into<String>, remedy: Option<&str>) -> Self {
        Self {
            verdict: Verdict::Warn,
            detail: detail.into(),
            remedy: remedy.map(str::to_string),
        }
    }

    /// Wrong now. The remedy is optional because a check that could not be *computed* has
    /// nothing to suggest — see [`check_regen`]'s error arm.
    fn fail(detail: impl Into<String>, remedy: Option<&str>) -> Self {
        Self {
            verdict: Verdict::Fail,
            detail: detail.into(),
            remedy: remedy.map(str::to_string),
        }
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
}

impl Check {
    const REPOSITORY: &'static str = "repository";
    const PROVENANCE: &'static str = "provenance";
    const BINARY: &'static str = "binary";
    const PATH: &'static str = "path";
    const PRELUDE: &'static str = "prelude";
    const INDEX: &'static str = "index";
    const REGEN: &'static str = "regen";
    const BUILD: &'static str = "build";
    const CATALOG: &'static str = "catalog";
    const CORPORA: &'static str = "corpora";
    const VAULT: &'static str = "vault";
    const POLICY: &'static str = "policy";
    const GOVERNANCE: &'static str = "governance";
    const KUTEN: &'static str = "kuten";
    const KUTEN_READ: &'static str = "kuten-read";
    const CORPUS: &'static str = "corpus";
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
    pub checks: Vec<Check>,
}

impl DoctorReport {
    fn new(checks: Vec<Check>, strict: bool) -> Self {
        let failed = checks.iter().filter(|c| c.verdict == Verdict::Fail).count();
        let warned = checks.iter().filter(|c| c.verdict == Verdict::Warn).count();
        Self {
            passed: failed == 0 && !(strict && warned > 0),
            strict,
            failed,
            warned,
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
/// One more state lives at this same test: `.yidam/` holding real corpus content — class
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
    let in_git = std::process::Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
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
    let in_work_tree = std::process::Command::new("git")
        .current_dir(root)
        .args(["rev-parse", "--is-inside-work-tree"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if !in_work_tree {
        return false;
    }
    let head_resolves = std::process::Command::new("git")
        .current_dir(root)
        .args(["rev-parse", "--verify", "-q", "HEAD"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(true);
    !head_resolves
}

/// The pin a derived repository is upgradable from.
///
/// A repository with no recorded origin cannot be upgraded: there is no baseline to compute
/// a forward change against. `yidam-build` refuses outright, which is a good failure at the
/// wrong moment — this is the moment.
fn check_provenance(root: &Path) -> Answer {
    let manifest = root.join(MANIFEST);
    let Ok(text) = std::fs::read_to_string(&manifest) else {
        return Answer::fail(
            format!("no {MANIFEST}"),
            Some("mise run yidam-vendor-update"),
        );
    };
    let pin = ManifestPin::parse(&text);
    match pin.commit.as_deref() {
        Some(commit) if commit != "unknown" => Answer::ok(format!(
            "pinned {} ({})",
            &commit[..commit.len().min(12)],
            pin.template.as_deref().unwrap_or("untagged")
        )),
        _ => Answer::fail(
            format!("{MANIFEST} records no resolvable commit"),
            Some("mise run yidam-vendor-update"),
        ),
    }
}

/// Is the running binary the one this repository pins?
fn check_binary(root: &Path, running: Option<&Path>) -> Answer {
    match pinned_binary(root, running) {
        Pinned::Unpinned => Answer::ok("this repository pins no binary — nothing can be shadowed"),
        Pinned::Running => Answer::ok(format!(
            "running the pin at {}",
            yidam_bin_path(root).display()
        )),
        // Fail, not warn. This is the failure that reads as success: an older binary
        // missing a subcommand exits with `unrecognized subcommand`, which a script with
        // output redirected cannot tell from having done the work.
        Pinned::Shadowed { pinned, running } => Answer::fail(
            format!(
                "running {}, but this repository pins {}",
                running.display(),
                pinned.display()
            ),
            Some("put `.yidam/bin` first on PATH"),
        ),
    }
}

/// The first `yidam` a shell would resolve from `path_var`, or `None` if it would find
/// none.
///
/// Split out and given the PATH string rather than reading the environment, because the
/// interesting cases are orderings that are tedious to arrange in a live process.
fn first_yidam_on_path(path_var: &std::ffi::OsStr) -> Option<PathBuf> {
    std::env::split_paths(path_var)
        .map(|dir| dir.join(format!("yidam{}", std::env::consts::EXE_SUFFIX)))
        .find(|candidate| candidate.is_file())
}

/// Is `.yidam/bin` ahead on PATH?
///
/// Distinct from [`check_binary`], which compares the binary that *did* answer. This one
/// asks what the next invocation will resolve, and catches the case `check_binary` cannot:
/// a pinned binary invoked by absolute path, in a shell where the next `yidam` typed by
/// hand will come from somewhere else entirely.
fn check_path(root: &Path, path_var: Option<&std::ffi::OsStr>) -> Answer {
    const AHEAD: &str = "add `_.path = [\".yidam/bin\"]` under `[env]` in this repo's mise.toml";
    let pinned = yidam_bin_path(root);
    if !pinned.is_file() {
        return Answer::ok("this repository pins no binary — PATH order does not matter");
    }
    let Some(path_var) = path_var else {
        return Answer::skipped("PATH is unset");
    };
    let real = |p: &Path| std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
    match first_yidam_on_path(path_var) {
        Some(first) if real(&first) == real(&pinned) => Answer::ok(format!(
            "PATH resolves yidam to the pin at {}",
            pinned.display()
        )),
        Some(first) => Answer::fail(
            format!(
                "PATH resolves yidam to {}, ahead of the pin at {}",
                first.display(),
                pinned.display()
            ),
            Some(AHEAD),
        ),
        // Not a failure: the pin exists and is reachable, just not by bare name. Every
        // command run through `mise run` still resolves it.
        None => Answer::warn(
            format!(
                "no yidam on PATH at all; the pin at {} is reachable only by path",
                pinned.display()
            ),
            Some(AHEAD),
        ),
    }
}

/// How old is the vendored prelude?
///
/// Local only, on purpose — see this module's header. `committed` is the author date of the
/// pinned commit, not the date this repository ran the vendor step, so it answers how old
/// the prelude is rather than how recently someone typed a command.
fn check_prelude(root: &Path, today: i64) -> Answer {
    /// Where the question this command declines to answer *is* asked. Said on both
    /// verdicts, because whether the origin has moved is worth knowing at any pin age —
    /// and said in the `detail` on the healthy one, since that is the half of the line a
    /// reader is shown when nothing is wrong. It was a remedy on both until #656, which
    /// meant the green case named it to a JSON consumer and to nobody else.
    const ORIGIN: &str = "whether the origin has moved is `mise run yidam-vendor-status`, \
                          which needs network";
    let vendored = root.join(".yidam").join(".vendor").join("prelude");
    if !vendored.is_dir() {
        return Answer::warn(
            "no .yidam/.vendor/prelude/ — this repository carries no vendored prelude",
            Some("mise run yidam-vendor-update"),
        );
    }
    let pin = std::fs::read_to_string(root.join(MANIFEST))
        .map(|t| ManifestPin::parse(&t))
        .unwrap_or_default();
    let Some(committed) = pin.committed.filter(|c| c != "unknown") else {
        return Answer::warn(
            format!("{MANIFEST} records no pin date — age is unknowable"),
            Some("mise run yidam-vendor-status"),
        );
    };
    let Some(days) = crate::dates::days_from_civil_str(&committed).map(|d| today - d) else {
        return Answer::warn(
            format!("{MANIFEST} records an unparseable pin date: {committed}"),
            Some("mise run yidam-vendor-status"),
        );
    };
    let age = format!("pinned {committed} — {days} day(s) ago");
    if days > STALE_PRELUDE_DAYS {
        Answer::warn(
            age,
            Some("mise run yidam-vendor-status (compares against the origin)"),
        )
    } else {
        Answer::ok(format!("{age}; {ORIGIN}"))
    }
}

/// Is the index built, and is it stale against the corpus?
///
/// Never a failure. A light `reports` build cannot build one, and a repository that has no
/// use for semantic search is not misconfigured for lacking it.
fn check_index(root: &Path) -> Answer {
    const BUILD: &str = "yidam index-build (needs the `index` feature)";
    let data = crate::cmd::index_status_data(root);
    if !data.index_present {
        return Answer::warn(
            format!("no {}", yidam_index_dir(root).display()),
            Some(BUILD),
        );
    }
    if !data.meta_present {
        return Answer::warn(
            "index present, but it carries no readable meta.json",
            Some(BUILD),
        );
    }
    if data.stale_nodes > 0 {
        return Answer::warn(
            format!(
                "built {}, and {} corpus file(s) have changed since",
                data.built.clone().unwrap_or_default(),
                data.stale_nodes
            ),
            Some(BUILD),
        );
    }
    Answer::ok(format!(
        "built {}, {} node(s), model {}",
        data.built.clone().unwrap_or_default(),
        data.node_count.unwrap_or(0),
        data.model.clone().unwrap_or_default()
    ))
}

/// Are the REGEN blocks current?
///
/// Borrows `regen --check`'s non-writing mode rather than reimplementing the generator
/// list. That is the whole reason [`crate::cmd::stale_blocks`] exists as a
/// function: a second list would be the third one that command was written to prevent.
fn check_regen() -> Answer {
    match crate::cmd::stale_blocks() {
        Err(e) => Answer::fail(format!("could not be computed: {e:#}"), None),
        Ok(stale) if stale.is_empty() => {
            Answer::ok("every REGEN block holds what its generator produces")
        }
        Ok(stale) => {
            let names: Vec<String> = stale
                .iter()
                .map(|s| format!("{} ({})", s.file, s.generator))
                .collect();
            Answer::fail(
                format!("{} block(s) stale: {}", stale.len(), names.join(", ")),
                Some("yidam regen, committed as a `regen:` commit"),
            )
        }
    }
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
    /// git history, which does not exist yet either.
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
            _ => (self.answer)(subject),
        };
        Check {
            id: self.id,
            question: self.text,
            verdict: answer.verdict,
            detail: answer.detail,
            remedy: answer.remedy,
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
        id: Check::REGEN,
        text: "Are the REGEN blocks current?",
        asked: Asked::OfARepository,
        answer: |_| check_regen(),
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
        id: Check::VAULT,
        text: "Can this repository reach its vaults?",
        asked: Asked::OfARepository,
        answer: |s| check_vault(&s.root),
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

/// Run every check against `root`.
///
/// `running` and `path_var` are passed rather than read so the two environment-sensitive
/// checks are testable; production hands them [`std::env::current_exe`] and `$PATH`.
pub(crate) fn diagnose(
    root: &Path,
    running: Option<&Path>,
    path_var: Option<&std::ffi::OsStr>,
    today: i64,
) -> Vec<Check> {
    let subject = Subject {
        root: root.to_path_buf(),
        running: running.map(Path::to_path_buf),
        path_var: path_var.map(std::ffi::OsStr::to_os_string),
        today,
    };
    // Set the moment the repository check fails, which is what makes the rest of the roster
    // unanswerable. Every later question sees it; the two that can be asked anywhere ignore
    // it. See [`Asked`] for why this is the discriminator.
    let mut unanswerable: Option<&'static str> = None;
    let mut checks = Vec::with_capacity(ROSTER.len());
    for question in ROSTER {
        let check = question.ask(&subject, unanswerable);
        if question.id == Check::REPOSITORY && check.verdict == Verdict::Fail {
            unanswerable = Some(if root.join(".yidam").is_dir() {
                "bootstrapped but never committed"
            } else {
                "not a yidam repository"
            });
        }
        checks.push(check);
    }
    checks
}

/// Which declaration this repository adopted, and whether the vendored profile still matches
/// it — RFC-0028 §9.
///
/// **Holding none is `Ok`.** That was every one of the eighteen corpora A0 measured, and a
/// state eighteen repositories are in is not a fault to be listed under things to fix.
///
/// What warns is a declaration that cannot be honoured: a record naming a profile nothing
/// vendored, or one whose revision has moved out from under it. Without the revision, `score`
/// would score a repository against a kuten it may not hold and `fit` would compare two
/// holding different ones — A0's own confound designed into A0's deliverable.
fn check_kuten(root: &Path) -> Answer {
    let declaration = match crate::kuten::read_declaration(root) {
        Ok(d) => d,
        Err(e) => {
            return Answer::warn(
                e.to_string(),
                Some("fix the decision record, or delete it to hold no kuten"),
            )
        }
    };
    let Some(declaration) = declaration else {
        return Answer::ok("none — the loop runs on the template's defaults");
    };
    match crate::kuten::read_profile(root, &declaration.name) {
        Err(e) => Answer::warn(e.to_string(), Some("re-vendor the prelude")),
        Ok(None) => Answer::warn(
            format!(
                "`{}` is declared and no profile is vendored for it",
                declaration.name
            ),
            Some("mise run yidam-vendor-update"),
        ),
        Ok(Some(profile)) if profile.revision != declaration.revision => Answer::warn(
            format!(
                "`{}` at revision {}, vendored at revision {}",
                declaration.name, declaration.revision, profile.revision
            ),
            Some("re-vendor, or record a superseding `decide:` decision"),
        ),
        Ok(Some(profile)) => Answer::ok(format!(
            "`{}`, revision {} — {}",
            profile.name,
            profile.revision,
            object_state(profile.object.as_ref())
        )),
    }
}

/// Is the declaration somewhere an agent will meet it? — #694.
///
/// A separate question from [`check_kuten`], and separate because it has a different subject:
/// that one answers *which kuten does this repository hold*, from the decision record and the
/// vendored profile. This one asks whether anything in the loop **reads** it. A repository can
/// answer the first perfectly and the second not at all, and one of the two repositories that
/// adopted on day one did exactly that — `adopt` skipped the `AGENTS.md` write because there
/// was no `AGENTS.md`, and `regen --check`, `doctor` and `kuten` all reported it as fine.
///
/// The module `adopt` lives in names this outcome as the failure the layer exists to prevent:
/// *"A declaration nothing in the loop reads is this epic's own diagnosed failure aimed at its
/// centrepiece."*
///
/// **Warn, never fail.** The remedy is a document a person writes, the declaration is still
/// true, and nothing about the corpus is broken — the same reasoning `kuten check` runs on.
/// Holding no kuten skips the question entirely rather than passing it: there is nothing to
/// carry, and an `ok` there would read as *something checked and found present*.
fn check_kuten_read(root: &Path) -> Answer {
    let held = match crate::kuten::read_declaration(root) {
        Ok(Some(d)) => d,
        // Unreadable or absent: `kuten` above is the check that says so, and two verdicts on
        // one record would be two answers to one question.
        Ok(None) | Err(_) => {
            return Answer::skipped("no kuten is declared, so nothing carries one")
        }
    };
    match carriers(root).as_slice() {
        [] => Answer::warn(
            format!(
                "`{}` is declared and no file carries the `yidam kuten` block, so nothing an \
                 agent reads mentions it",
                held.name
            ),
            Some("add the `yidam kuten` REGEN section to AGENTS.md, then run `yidam kuten`"),
        ),
        [one] => Answer::ok(format!("`{}` — {one} carries the block", held.name)),
        many => Answer::ok(format!(
            "`{}` — {} files carry the block",
            held.name,
            many.len()
        )),
    }
}

/// Repository-relative paths carrying the `yidam kuten` REGEN marker.
///
/// `AGENTS.md` is where `adopt` and the scaffold put it and where `write_block` fills it, so
/// it is checked first and by name. The wider scan exists because the marker is what actually
/// decides: a corpus that briefs its agents in `CLAUDE.md` and pasted the section there has
/// not failed at anything, and a check that only knew one filename would tell it that it had.
fn carriers(root: &Path) -> Vec<String> {
    const MARKER: &str = "<!-- REGEN: yidam kuten";
    let mut found = Vec::new();
    for rel in ["AGENTS.md", "CLAUDE.md", "README.md", ".yidam/AGENTS.md"] {
        if std::fs::read_to_string(root.join(rel)).is_ok_and(|t| t.contains(MARKER)) {
            found.push(rel.to_string());
        }
    }
    found
}

/// Which way the arrow between corpus and object runs — RFC-0028 §6, and #582's acceptance
/// criterion.
///
/// **This line is why #582 closes.** An undeclared untracked corpus is today
/// indistinguishable from no corpus at all, and #582's third question was precisely whether
/// anything says which state a repository is in. It is on the existing `kuten` check's text
/// rather than a check of its own: the direction is part of the answer to *which kuten does
/// this repository hold*, not a second question, and every consumer of `doctor` keys on the
/// check id.
///
/// A profile that leaves the slot unpopulated says nothing here — the difference between
/// *this practice makes no claim* and *this repository was not read*, kept the same way
/// [`crate::kuten::compare`] keeps it.
fn object_state(object: Option<&crate::kuten::Object>) -> String {
    match object {
        None => "the object slot is unpopulated, so nothing is declared about direction".into(),
        Some(o) => format!("{} ({})", o.direction.describe(), o.direction.name()),
    }
}

/// Do this repository's rules compile, and which of them are its own?
///
/// **Where a broken policy is caught**, decided on RFC-0024's fourth open question. Not a new
/// mise task and not a new CI job in every derived repository: `doctor` is already offline and
/// read-only, this reads two directories, and derived CI already runs it. Machinery nobody
/// invokes is how a check comes to cover nothing.
///
/// Three verdicts, and the middle one is the point:
///
/// - a policy that does not compile, or that names a builtin this build does not carry, is a
///   **Fail** — the rule cannot answer, and a rule that cannot answer is not a permit;
/// - a repository whose rules are all inherited is **Ok** and says so in one line;
/// - a repository with local rules is **Ok** as well, and they are *named*. An override is a
///   decision the repository is entitled to make, and this is not the place that objects to
///   it — `lint`'s `policy-override` reports each one at `Info` and gates on nothing. What
///   `yidam policy test` is for is said in the `detail` for that reason: it is what is
///   available to a reader who has overridden something, not an action owed by a repository
///   that has done nothing wrong. It was a remedy on an `ok` verdict until #656, which the
///   prose dropped and every JSON consumer showed as a thing to do.
fn check_policy(root: &Path) -> Answer {
    let policies = match crate::policy::Policies::load(root) {
        Err(e) => return Answer::fail(
            first_line(&e.to_string()),
            Some("`yidam policy check` names the file; a rule that cannot answer refuses nothing."),
        ),
        Ok(p) => p,
    };

    match policies.disallowed_builtins() {
        Err(e) => {
            return Answer::fail(
                first_line(&e.to_string()),
                Some("`yidam policy check` reports the same thing with the file and the call."),
            )
        }
        Ok(found) if !found.is_empty() => {
            return Answer::fail(
                format!(
                    "{} call(s) to a builtin this build does not carry, first: {} in {}",
                    found.len(),
                    found[0].1,
                    found[0].0
                ),
                Some(
                    "These parse and fail at the moment a decision is needed. Remove the call; \
                     this binary compiles no network or clock builtins by design.",
                ),
            )
        }
        Ok(_) => {}
    }

    let local: Vec<&str> = policies
        .origins()
        .filter(|(_, o)| o.is_local())
        .map(|(d, _)| d)
        .collect();
    let total = policies.origins().count();

    if local.is_empty() {
        return Answer::ok(format!("{total} decision(s), all inherited"));
    }
    Answer::ok(format!(
        "{} of {total} decided by this repository: {}; `yidam policy test` runs the inherited \
         cases against them and reports which expectations they no longer meet",
        local.len(),
        local.join(", ")
    ))
}

/// How many commits `HEAD` carries. `0` on anything that cannot be counted, which reads the
/// same as "too early to tell" everywhere it is used.
fn commit_count(root: &Path) -> usize {
    std::process::Command::new("git")
        .current_dir(root)
        .args(["rev-list", "--count", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(0)
}

/// How much history has to pass before "the sangha has never run a resolution" is worth
/// saying rather than merely true. #581 measured three of four repositories that declared
/// collective governance and never ran one; a repository ten commits past genesis has not
/// had the chance yet, so this is well above the population's evolution-branch depths.
const GOVERNANCE_HISTORY_THRESHOLD: usize = 20;

/// Is this repository's declared governance mode carrying its own weight? (#581)
///
/// "Declaring collective" is read the way [`crate::cmd::sangha::sangha_data`] reads it — a
/// registered elector, `ma/*` row and all, in `.yidam/sangha/electors.md` — not the
/// `governance:` field bootstrap writes to a decision record, which nothing keeps in sync
/// with what actually happens after genesis. Three of four repositories in the #581
/// measurement chose `collective` in the decision record and never ran a resolution; the
/// electors table is the artifact that would show it either way.
///
/// Informational only, and silent for a long time on purpose: a young repository with real
/// electors and no resolution yet has done nothing wrong. This never fails — carrying
/// unused scaffolding is a cost, not a corruption — and only warns once enough history has
/// passed that "never" starts to mean something.
fn check_governance(root: &Path) -> Answer {
    let electors_path = crate::paths::yidam_sangha_dir(root).join("electors.md");
    let Ok(text) = std::fs::read_to_string(&electors_path) else {
        return Answer::ok("single-elector — no .yidam/sangha/electors.md");
    };
    let electors = crate::cmd::sangha::parse_electors(&text);
    if electors.is_empty() {
        return Answer::ok("electors.md present, but no ma/* elector is registered yet");
    }
    let has_resolution = crate::git::phase_refs(root)
        .iter()
        .any(|r| r.kind == crate::git::RefKind::Evolution);
    if has_resolution {
        return Answer::ok(format!(
            "{} elector(s) registered; at least one rigpa/* resolution exists",
            electors.len()
        ));
    }
    let commits = commit_count(root);
    if commits < GOVERNANCE_HISTORY_THRESHOLD {
        return Answer::ok(format!(
            "{} elector(s) registered, no resolution yet, {commits} commit(s) in — too early \
             to tell",
            electors.len()
        ));
    }
    Answer::warn(
        format!(
            "{} elector(s) registered under collective governance; zero rigpa/* resolutions \
             in {commits} commits — the sangha scaffold is carrying no weight",
            electors.len()
        ),
        Some(
            "run a resolution, or drop to single-elector and remove .yidam/sangha/ (it can \
             be re-scaffolded when a second elector actually appears)",
        ),
    )
}

/// Are the vaults configured, and is everything they need in place — **without asking them**.
///
/// `doctor` is documented read-only and offline, and this does not change that. Whether a
/// store is *reachable* is `yidam vault status --remote`'s question, and answering it here
/// would put a network call in the one command a person runs when the network is what they
/// suspect.
///
/// So what is checked is everything that can be settled locally: that the declared vaults
/// resolve, that each says who can read it, that each has its credentials in the environment,
/// that no artifact the corpus names has been left without a route, and that the artifacts are
/// cached. Each of those fails in a way that produces an unhelpful error much later — a `403`
/// names no variable, and a missing artifact surfaces as a broken citation.
///
/// # The one warning that is not about a failure
///
/// Two vaults resolving to the same credentials is **legal** — a corpus may genuinely use one
/// account and two buckets, and this repository is not in a position to say otherwise. It is
/// also exactly what a half-finished isolation setup looks like: somebody declared `sources`,
/// exported nothing for it, and it is quietly running on the account meant for public output.
/// The two are indistinguishable from here, so this reports the shape and lets the reader
/// decide, which is the only honest thing available.
fn check_vault(root: &Path) -> Answer {
    let config = crate::config::load_yidam_config(root).unwrap_or_default();
    let vaults =
        match crate::vault::resolve(&config.vault) {
            Err(e) => return Answer::fail(
                first_line(&e.to_string()),
                Some(
                    "Fix `[vault.…]` in `.yidam/config.toml`; `yidam vault list` shows the shape.",
                ),
            ),
            Ok(v) => v,
        };

    let named = crate::vault::named_artifacts(root);
    if vaults.is_empty() {
        // No vault is the common case and is not a defect — unless the corpus has already
        // started recording artifacts, which means it is relying on somewhere to keep them.
        return if named.is_empty() {
            Answer::ok("none declared")
        } else {
            Answer::warn(
                format!("{} artifact(s) recorded and no vault declared", named.len()),
                Some(
                    "Declare `[vault.default]` in `.yidam/config.toml`, or the bytes live \
                      only in whichever caches happen to hold them.",
                ),
            )
        };
    }

    // A record the config cannot place. Reported here rather than by `lint`, because the
    // defect is in `.yidam/config.toml` and lint's subject is the corpus — blaming a catalog
    // entry for a store nobody declared would point at the wrong file.
    let stranded: Vec<&crate::vault::Named> = named
        .iter()
        .filter(|a| {
            matches!(
                vaults.route(&a.kind, a.vault.as_deref()),
                crate::vault::Route::Unroutable(_)
            )
        })
        .collect();
    if let Some(first) = stranded.first() {
        let why = match vaults.route(&first.kind, first.vault.as_deref()) {
            crate::vault::Route::Unroutable(w) => first_line(&w),
            _ => unreachable!("filtered to unroutable"),
        };
        return Answer::warn(
            format!("{} artifact(s) have no route — {why}", stranded.len()),
            Some(
                "Add the kind to a vault's `holds` in `.yidam/config.toml`, or route the \
                  record itself with `vault:`.",
            ),
        );
    }

    // Credentials, for the stores that need them. A `file://` vault needs none, and demanding
    // them would report a healthy setup as broken.
    let mut principals: BTreeMap<String, Vec<&str>> = BTreeMap::new();
    for (name, cfg) in vaults.iter() {
        if !cfg.url.trim().starts_with("s3://") {
            continue;
        }
        if let Err(e) = crate::vault::credentials_available(name) {
            return Answer::warn(
                first_line(&e.to_string()),
                Some(
                    "Credentials come from the environment only — `.yidam/config.toml` is \
                      committed and must never carry one.",
                ),
            );
        }
        if let Some(id) = crate::vault::credential_principal(name) {
            principals.entry(id).or_default().push(name);
        }
    }
    // The access key id is compared and never printed: it identifies the account, and naming
    // it in a report that gets pasted into an issue helps nobody.
    if let Some((_, shared)) = principals.iter().find(|(_, v)| v.len() > 1) {
        return Answer::warn(
            format!(
                "{} resolve to the same credentials",
                shared
                    .iter()
                    .map(|n| format!("`{n}`"))
                    .collect::<Vec<_>>()
                    .join(" and ")
            ),
            Some(
                "Legal — one account can own two buckets. It is also what an unfinished \
                  isolation setup looks like; export YIDAM_VAULT_<NAME>_ACCESS_KEY_ID per \
                  vault if they were meant to differ.",
            ),
        );
    }

    let cache = match crate::vault::Cache::resolve(|k| std::env::var(k).ok()) {
        Ok(c) => c,
        Err(e) => {
            return Answer::warn(
                first_line(&e.to_string()),
                Some("Set YIDAM_VAULT_CACHE to say where artifacts should live."),
            )
        }
    };
    let configured = format!(
        "{} vault{} configured",
        vaults.len(),
        if vaults.len() == 1 { "" } else { "s" }
    );
    let uncached = named.iter().filter(|a| !cache.contains(&a.hash)).count();
    if uncached > 0 {
        return Answer::warn(
            format!(
                "{configured}; {uncached} of {} recorded artifact(s) not cached",
                named.len()
            ),
            Some(
                "`yidam vault pull` fetches them. Reachability is `yidam vault status \
                  --remote`; this check makes no network call.",
            ),
        );
    }
    Answer::ok(format!("{configured}; {} artifact(s) cached", named.len()))
}

/// The first line of an error, for a one-line verdict.
fn first_line(s: &str) -> String {
    s.lines().next().unwrap_or_default().trim().to_string()
}

/// Are the corpora this repository depends on actually unpacked, and are they the ones
/// `tonpa.lock` pins?
///
/// Nothing asked this before. It did not matter much while `yidam tonpa install` was a step
/// somebody ran deliberately — they saw it succeed or fail. #397 made it a `postinstall`
/// hook, and mise treats a failing postinstall as a *warning*:
///
/// ```text
/// [tonpa-install] ERROR task failed
/// mise WARN  Postinstall hook in <dir> failed: … exited with code 1
/// $ echo $?
/// 0
/// ```
///
/// So `mise install` goes green, the toolchains are fine, the binary is fine, and every
/// command that reads a dependency's corpus reads nothing. The one line that said otherwise
/// scrolled past. This is the check that can still say so afterwards.
///
/// **No network, and none is possible from here.** It compares what is on disk against a
/// lock file, which is the whole correctness story for a fetched corpus. `doctor` is exactly
/// the command someone runs when they suspect the network, so it must not need it.
///
/// **Read-only.** `cmd_install` writes `tonpa.lock` when anything changed; this shares the
/// *verification* and not the command, the way [`check_regen`] borrows `stale_blocks` rather
/// than reimplementing the generator list.
fn check_corpora(root: &Path) -> Answer {
    const REMEDY: &str = "mise run tonpa-install";

    let config = crate::deps::load_config(&crate::paths::tonpa_config_path(root));
    if config.dependencies.is_empty() {
        // Not a warning, and not "0 missing". A repository that depends on nothing is not
        // half-provisioned, and a line reporting a count here would read as a verdict on a
        // question nobody put — the same reason check_catalog stays quiet without a TTL.
        return Answer::ok("none declared");
    }

    let tonpa_dir = crate::paths::tonpa_dir(root);
    let lock = crate::deps::load_lock(&crate::paths::tonpa_lock_path(root)).unwrap_or_default();

    let (mut missing, mut corrupt, mut unlocked, mut ok, mut local) =
        (Vec::new(), Vec::new(), Vec::new(), 0usize, 0usize);

    for (name, dep) in &config.dependencies {
        // A path dependency is read where it sits and has nothing to fetch, which is exactly
        // what `cmd_install` decides about it. Counted, not graded.
        if dep.path.is_some() {
            local += 1;
            continue;
        }
        let Some(locked) = lock.packages.iter().find(|p| &p.name == name) else {
            // Declared and never pinned. The lock is the correctness story for a fetched
            // corpus, so a dependency outside it is not verifiable — but it is also the
            // normal state between `tonpa add` and the first install, so it is not a failure.
            unlocked.push(name.clone());
            continue;
        };
        match crate::deps::verify_installed(name, &tonpa_dir, locked) {
            Ok(true) => ok += 1,
            // `verify_installed` answers one question — is the bundle the pinned one — and
            // returns false for both "absent" and "different". A caller that can re-fetch
            // does not care which; this one cannot fetch anything, so the remedy it prints
            // is the same but the sentence it writes is not.
            Ok(false) if !tonpa_dir.join(name).join("bundle.yiz").exists() => {
                missing.push(name.clone())
            }
            Ok(false) => corrupt.push(name.clone()),
            // Unreadable is not intact. Reporting it as present would be the one wrong
            // answer here.
            Err(_) => corrupt.push(name.clone()),
        }
    }

    let mut detail = Vec::new();
    if ok > 0 {
        detail.push(format!("{ok} installed"));
    }
    if local > 0 {
        detail.push(format!("{local} path"));
    }
    if !missing.is_empty() {
        detail.push(format!("not installed: {}", missing.join(", ")));
    }
    if !corrupt.is_empty() {
        detail.push(format!("does not match tonpa.lock: {}", corrupt.join(", ")));
    }
    if !unlocked.is_empty() {
        detail.push(format!(
            "declared but never pinned: {}",
            unlocked.join(", ")
        ));
    }
    let detail = detail.join("; ");

    if !missing.is_empty() || !corrupt.is_empty() {
        Answer::fail(detail, Some(REMEDY))
    } else if !unlocked.is_empty() {
        Answer::warn(detail, Some(REMEDY))
    } else {
        Answer::ok(detail)
    }
}

/// Can every corpus file be read at all?
///
/// **The question under all the others.** A file that does not parse reaches a reader as an
/// *empty record*, and everything computed from it is then a statement about the emptiness
/// rather than about the file. `lint` learned this in #676 and `graph-check` in #721; this is
/// the same defect on the surface a person runs first when something is wrong, and where it
/// showed up as saying nothing at all.
///
/// **It does not re-report what `lint` reports.** One line and a count, with `yidam lint` as
/// the remedy — that command names the files and is the place the finding belongs. What
/// `doctor` adds is that the question gets *asked* here, so a corpus nothing can read is not
/// something you discover by noticing that a different command has gone quiet.
///
/// Silent on a corpus with no files in it, which is every repository between `bootstrap` and
/// the first node: `Ok` with `no corpus files yet` says the question was put and had no
/// subject, which is not the same as a clean bill of health over nothing.
fn check_corpus(root: &Path) -> Answer {
    let corpus = crate::paths::yidam_corpus_dir(root);
    let instances = crate::walk::walk_corpus_instances(&corpus);
    let ont_files = crate::walk::walk_ont_files(&corpus);
    let total = instances.len() + ont_files.len();
    if total == 0 {
        return Answer::ok("no corpus files yet");
    }

    let overlay = crate::cmd::lint::Overlay::default();
    let unreadable = crate::cmd::lint::checks::load_nodes(root, &instances, &overlay)
        .iter()
        .filter(|n| n.malformed.is_some())
        .count()
        + crate::cmd::lint::checks::load_classes(root, &ont_files, &overlay)
            .iter()
            .filter(|c| c.malformed.is_some())
            .count();

    match unreadable {
        0 => Answer::ok(format!("{total} file(s), all readable")),
        n => Answer::fail(
            format!("{n} of {total} corpus file(s) do not parse"),
            Some("yidam lint"),
        ),
    }
}

/// Have any source records aged past what the corpus said they may?
///
/// **No network, and none is possible from here.** This reads the entry's own `retrieved:`,
/// or the commit that last touched its file, against a TTL the corpus declared. It cannot
/// say the upstream changed and does not claim to — it says nobody has looked.
///
/// Silent where no TTL applies, which is every corpus that has not asked. A `doctor` line
/// saying "0 expired" on a repository that never declared a TTL would read as a clean bill of
/// health on a question nobody put.
fn check_catalog(root: &Path, today: i64) -> Answer {
    let dir = crate::paths::yidam_catalog_dir(root);
    let sources = crate::cmd::lint::checks::load_sources(
        root,
        &crate::walk::walk_md_files(&dir),
        &Default::default(),
    );
    let default_ttl = crate::config::load_yidam_config(root)
        .map(|c| c.catalog.ttl_days)
        .unwrap_or_default();
    let iso = crate::cmd::export::unix_to_iso(today as u64 * 86_400);
    let ages = crate::cmd::lint::ttl::ages(
        &sources,
        &crate::cmd::lint::ttl::committed_dates(root, &dir),
        default_ttl,
        iso.split('T').next().unwrap_or_default(),
    );

    let governed = ages.iter().filter(|a| a.ttl_days.is_some()).count();
    if governed == 0 {
        return Answer::ok(format!(
            "no TTL declared — {} source(s) never expire. Set `[catalog] ttl_days` or declare \
             `ttl_days:` on an entry.",
            ages.len()
        ));
    }
    let expired: Vec<&crate::cmd::lint::ttl::Age> =
        ages.iter().filter(|a| a.overdue_days().is_some()).collect();
    let undatable = ages.iter().filter(|a| a.undatable()).count();
    if expired.is_empty() && undatable == 0 {
        return Answer::ok(format!("{governed} source(s) under a TTL, none expired"));
    }
    let mut detail = Vec::new();
    if let Some(worst) = expired
        .iter()
        .max_by_key(|a| a.overdue_days().unwrap_or_default())
    {
        detail.push(format!(
            "{} of {governed} source(s) expired, worst {} day(s) past ({})",
            expired.len(),
            worst.overdue_days().unwrap_or_default(),
            worst.entry
        ));
    }
    if undatable > 0 {
        detail.push(format!(
            "{undatable} under a TTL with no date to measure against"
        ));
    }
    Answer::warn(
        detail.join("; "),
        Some("yidam lint  # catalog-expired names each one"),
    )
}

pub(crate) fn render(report: &DoctorReport, root: &Path) -> String {
    let mut out = format!("yidam doctor — {}\n\n", root.display());
    for c in &report.checks {
        let _ = writeln!(out, "  {:<5} {:<12} {}", c.verdict.tag(), c.id, c.detail);
        // Printed wherever there is one, which is only ever a `warn` or a `fail`: a remedy
        // under a green line is how a report becomes something people skim past, and that
        // rule is [`Answer::ok`]'s now. It used to be enforced a second time here, and the
        // two halves disagreed for as long as both existed — the suppression was the
        // renderer's alone, so the JSON carried what this hid (#656).
        if let Some(remedy) = &c.remedy {
            let _ = writeln!(out, "  {:<5} {:<12} → {remedy}", "", "",);
        }
    }
    out.push('\n');
    match (report.failed, report.warned) {
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
    out
}

/// `yidam doctor`. Read-only, and exits nonzero on anything actionable.
pub fn doctor(strict: bool, format: crate::report::Format) -> Result<()> {
    let root = repo_root()?;
    let running = std::env::current_exe().ok();
    let path_var = std::env::var_os("PATH");
    let checks = diagnose(
        &root,
        running.as_deref(),
        path_var.as_deref(),
        crate::dates::today_days(),
    );
    let report = DoctorReport::new(checks, strict);
    let passed = report.passed;

    if format.is_json() {
        crate::report::emit(&root, report)?;
    } else {
        println!("{}", render(&report, &root));
    }
    if !passed {
        std::process::exit(1);
    }
    Ok(())
}

// ── .yidam.toml ───────────────────────────────────────────────────────────────

/// The fields of `.yidam.toml` this command reads.
///
/// Parsed with `toml` rather than the `sed` the mise tasks use, because this is the one
/// consumer that can afford a real parser and the tasks are not.
#[derive(Debug, Default)]
struct ManifestPin {
    commit: Option<String>,
    template: Option<String>,
    committed: Option<String>,
}

impl ManifestPin {
    fn parse(text: &str) -> Self {
        let Ok(value) = toml::from_str::<toml::Value>(text) else {
            return Self::default();
        };
        let field = |k: &str| {
            value
                .get("yidam")
                .and_then(|y| y.get(k))
                .and_then(|v| v.as_str())
                .map(str::to_string)
        };
        Self {
            commit: field("commit"),
            template: field("template"),
            committed: field("committed"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
        };
        ROSTER
            .iter()
            .find(|q| q.id == id)
            .expect("a question with that id")
            .ask(&subject, unanswerable)
    }

    /// A repository declaring corpora, with control over what is on disk and in the lock.
    fn repo_with_corpora(deps: &str, lock: &str, bundles: &[(&str, &[u8])]) -> TempDir {
        let tmp = derived_repo();
        std::fs::write(crate::paths::tonpa_config_path(tmp.path()), deps).unwrap();
        let dir = crate::paths::tonpa_dir(tmp.path());
        std::fs::create_dir_all(&dir).unwrap();
        if !lock.is_empty() {
            std::fs::write(crate::paths::tonpa_lock_path(tmp.path()), lock).unwrap();
        }
        for (name, bytes) in bundles {
            std::fs::create_dir_all(dir.join(name)).unwrap();
            std::fs::write(dir.join(name).join("bundle.yiz"), bytes).unwrap();
        }
        tmp
    }

    fn locked(name: &str, bytes: &[u8]) -> String {
        format!(
            "[[package]]\nname = \"{name}\"\nurl = \"https://example.com/{name}.yiz\"\nsha256 = \"{}\"\n",
            crate::deps::sha256_hex(bytes)
        )
    }

    // ── #721: a corpus file that does not parse ─────────────────────────────────

    /// A corpus of one class and one instance, with control over both files' bytes.
    fn repo_with_corpus(instance: &str, schema: &str) -> TempDir {
        let tmp = derived_repo();
        let class = tmp.path().join(".yidam/corpus/gage");
        std::fs::create_dir_all(&class).unwrap();
        std::fs::write(tmp.path().join(".yidam/corpus/gage.ont.yml"), schema).unwrap();
        std::fs::write(class.join("canyon-outlet.yml"), instance).unwrap();
        tmp
    }

    const SOUND: &str = "class: gage\nlabel: canyon-outlet\n";
    /// The same bytes with one unclosed quote in `label:`.
    const BROKEN: &str = "class: gage\nlabel: \"canyon-outlet\n";

    /// **The arm that said nothing at all.** `doctor` is what a person runs first when
    /// something is wrong, and a corpus file nothing can read was invisible to it — the
    /// defect #676 fixed in `lint` and #721 in `graph-check`, on the surface where it read
    /// as a clean bill of health.
    #[test]
    fn a_corpus_file_that_does_not_parse_is_a_finding() {
        let sound = repo_with_corpus(SOUND, SOUND);
        let c = check_corpus(sound.path());
        assert_eq!(c.verdict, Verdict::Ok, "{}", c.detail);
        assert!(c.detail.contains('2'), "both files counted: {}", c.detail);

        for (instance, schema) in [(BROKEN, SOUND), (SOUND, BROKEN)] {
            let broken = repo_with_corpus(instance, schema);
            let c = check_corpus(broken.path());
            assert_eq!(c.verdict, Verdict::Fail, "{}", c.detail);
            assert!(c.detail.starts_with("1 of 2"), "{}", c.detail);
            // The remedy is the command that names the files. `doctor` counts; it does not
            // re-report what `lint` reports.
            assert_eq!(c.remedy.as_deref(), Some("yidam lint"));
        }
    }

    /// A repository between `bootstrap` and its first node has no corpus files, and that is
    /// not a clean bill of health over nothing — it is a question with no subject.
    #[test]
    fn a_corpus_with_no_files_yet_is_not_graded() {
        let tmp = derived_repo();
        let c = check_corpus(tmp.path());
        assert_eq!(c.verdict, Verdict::Ok);
        assert_eq!(c.detail, "no corpus files yet");
        assert!(c.remedy.is_none());
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
        let docs = std::fs::read_to_string(
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/troubleshooting.md"),
        )
        .expect("docs/troubleshooting.md");

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

    /// A repository that depends on nothing is not half-provisioned.
    ///
    /// It must not read as "0 corpora missing" either — a verdict on a question nobody put is
    /// the failure `check_catalog` avoids by staying quiet where no TTL applies.
    #[test]
    fn declaring_no_corpora_is_not_a_finding() {
        let tmp = derived_repo();
        let c = check_corpora(tmp.path());
        assert_eq!(c.verdict, Verdict::Ok);
        assert_eq!(c.detail, "none declared");
        assert!(c.remedy.is_none(), "nothing to do, so nothing to suggest");
    }

    /// The state #397's hook can leave behind: mise logged a WARN, `mise install` exited 0,
    /// and the corpus is not there. Nothing else in this repository reports it.
    #[test]
    fn a_declared_corpus_that_never_arrived_fails() {
        let tmp = repo_with_corpora(
            "[dependencies.hydrology]\nurl = \"https://example.com/h.yiz\"\n",
            &locked("hydrology", b"bundle bytes"),
            &[],
        );
        let c = check_corpora(tmp.path());
        assert_eq!(c.verdict, Verdict::Fail, "detail was: {}", c.detail);
        assert!(c.detail.contains("not installed") && c.detail.contains("hydrology"));
        assert_eq!(c.remedy.as_deref(), Some("mise run tonpa-install"));
    }

    /// Present but not the pinned bytes. Distinct from absent, and said differently — the
    /// shared `verify_installed` returns false for both, because the caller that can re-fetch
    /// does not care which.
    #[test]
    fn a_corpus_that_does_not_match_the_lock_fails_and_says_so() {
        let tmp = repo_with_corpora(
            "[dependencies.hydrology]\nurl = \"https://example.com/h.yiz\"\n",
            &locked("hydrology", b"what we pinned"),
            &[("hydrology", b"something else entirely")],
        );
        let c = check_corpora(tmp.path());
        assert_eq!(c.verdict, Verdict::Fail);
        assert!(
            c.detail.contains("does not match tonpa.lock"),
            "a corrupt corpus must not be reported as a missing one: {}",
            c.detail
        );
        assert!(!c.detail.contains("not installed"), "{}", c.detail);
    }

    /// Unpacked and matching the lock. The whole point is that this is answerable offline.
    #[test]
    fn a_corpus_matching_its_lock_entry_passes() {
        let tmp = repo_with_corpora(
            "[dependencies.hydrology]\nurl = \"https://example.com/h.yiz\"\n",
            &locked("hydrology", b"bundle bytes"),
            &[("hydrology", b"bundle bytes")],
        );
        let c = check_corpora(tmp.path());
        assert_eq!(c.verdict, Verdict::Ok, "detail was: {}", c.detail);
        assert!(c.detail.contains("1 installed"), "{}", c.detail);
    }

    /// Declared, never pinned. Normal between `tonpa add` and the first install, so a warning
    /// rather than a failure — but not silence: the lock is the entire correctness story for
    /// a fetched corpus, and a dependency outside it is not verifiable at all.
    #[test]
    fn a_dependency_with_no_lock_entry_warns_rather_than_fails() {
        let tmp = repo_with_corpora(
            "[dependencies.hydrology]\nurl = \"https://example.com/h.yiz\"\n",
            "",
            &[],
        );
        let c = check_corpora(tmp.path());
        assert_eq!(c.verdict, Verdict::Warn, "detail was: {}", c.detail);
        assert!(c.detail.contains("never pinned"), "{}", c.detail);
    }

    /// A path dependency has nothing to fetch, which is exactly what `cmd_install` decides
    /// about it. Counted, not graded — and specifically not reported as missing, since there
    /// is no bundle under `.yidam/tonpa/` for one and never will be.
    #[test]
    fn a_path_dependency_is_counted_and_not_graded() {
        let tmp = repo_with_corpora("[dependencies.sibling]\npath = \"../sibling\"\n", "", &[]);
        let c = check_corpora(tmp.path());
        assert_eq!(c.verdict, Verdict::Ok, "detail was: {}", c.detail);
        assert!(c.detail.contains("1 path"), "{}", c.detail);
    }

    /// An unreadable bundle is not an intact one.
    ///
    /// `verify_installed` returns `Err` rather than `false` when the file is there and cannot
    /// be read, and the one wrong answer here is to let that count as installed.
    #[test]
    fn an_unreadable_bundle_is_not_reported_as_present() {
        let tmp = repo_with_corpora(
            "[dependencies.hydrology]\nurl = \"https://example.com/h.yiz\"\n",
            &locked("hydrology", b"bundle bytes"),
            &[("hydrology", b"bundle bytes")],
        );
        // A directory where the bundle should be: exists, and cannot be read as a file.
        let bundle = crate::paths::tonpa_dir(tmp.path())
            .join("hydrology")
            .join("bundle.yiz");
        std::fs::remove_file(&bundle).unwrap();
        std::fs::create_dir(&bundle).unwrap();
        let c = check_corpora(tmp.path());
        assert_eq!(c.verdict, Verdict::Fail, "detail was: {}", c.detail);
    }

    fn derived_repo() -> TempDir {
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join(".yidam")).unwrap();
        tmp
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
        let checks = diagnose(tmp.path(), None, None, 20_000);

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
        let checks = diagnose(tmp.path(), None, None, 20_000);
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
        git(tmp.path(), &["init", "-q", "-b", "main"]);
        git(tmp.path(), &["config", "user.email", "doctor@yidam.test"]);
        git(tmp.path(), &["config", "user.name", "Doctor"]);
        git(tmp.path(), &["add", "-A"]);
        git(tmp.path(), &["commit", "-q", "-m", "genesis: test"]);

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

    fn git(dir: &Path, args: &[&str]) {
        let status = std::process::Command::new("git")
            .current_dir(dir)
            .args(args)
            .status()
            .expect("git");
        assert!(status.success(), "git {args:?} failed in {}", dir.display());
    }

    // ── provenance ───────────────────────────────────────────────────────────

    #[test]
    fn a_missing_manifest_fails_provenance() {
        let tmp = derived_repo();
        let c = check_provenance(tmp.path());
        assert_eq!(c.verdict, Verdict::Fail);
        assert!(c.remedy.is_some());
    }

    #[test]
    fn an_unknown_commit_is_not_a_pin() {
        let tmp = derived_repo();
        std::fs::write(
            tmp.path().join(MANIFEST),
            "[yidam]\ncommit = \"unknown\"\ntemplate = \"untagged\"\n",
        )
        .unwrap();
        assert_eq!(check_provenance(tmp.path()).verdict, Verdict::Fail);
    }

    #[test]
    fn a_resolvable_commit_passes_and_is_abbreviated() {
        let tmp = derived_repo();
        std::fs::write(
            tmp.path().join(MANIFEST),
            "[yidam]\ncommit = \"0123456789abcdef0123456789abcdef01234567\"\n\
             template = \"cli/v0.2.0\"\n",
        )
        .unwrap();
        let c = check_provenance(tmp.path());
        assert_eq!(c.verdict, Verdict::Ok);
        assert!(c.detail.contains("0123456789ab"), "{}", c.detail);
        assert!(c.detail.contains("cli/v0.2.0"), "{}", c.detail);
    }

    // ── binary ───────────────────────────────────────────────────────────────

    #[test]
    fn a_repository_with_no_pin_cannot_be_shadowed() {
        let tmp = derived_repo();
        let elsewhere = tmp.path().join("cargo/bin/yidam");
        assert_eq!(
            check_binary(tmp.path(), Some(&elsewhere)).verdict,
            Verdict::Ok
        );
    }

    #[test]
    fn a_shadowed_pin_fails_and_names_both_paths() {
        let tmp = derived_repo();
        let pinned = yidam_bin_path(tmp.path());
        std::fs::create_dir_all(pinned.parent().unwrap()).unwrap();
        std::fs::write(&pinned, b"#!/bin/sh\n").unwrap();
        let stale = tmp.path().join("elsewhere/yidam");
        std::fs::create_dir_all(stale.parent().unwrap()).unwrap();
        std::fs::write(&stale, b"#!/bin/sh\n").unwrap();

        let c = check_binary(tmp.path(), Some(&stale));
        assert_eq!(c.verdict, Verdict::Fail);
        assert!(
            c.detail.contains(&stale.display().to_string()),
            "{}",
            c.detail
        );
        assert!(
            c.detail.contains(&pinned.display().to_string()),
            "{}",
            c.detail
        );
    }

    // ── PATH ─────────────────────────────────────────────────────────────────

    fn dir_with_yidam(parent: &Path, name: &str) -> PathBuf {
        let dir = parent.join(name);
        std::fs::create_dir_all(&dir).unwrap();
        let bin = dir.join(format!("yidam{}", std::env::consts::EXE_SUFFIX));
        std::fs::write(&bin, b"#!/bin/sh\n").unwrap();
        dir
    }

    fn path_var(dirs: &[&Path]) -> std::ffi::OsString {
        std::env::join_paths(dirs.iter().map(|p| p.to_path_buf())).unwrap()
    }

    /// The hazard `check_binary` cannot see: the pin *did* answer this invocation — it was
    /// run by absolute path — while the next bare `yidam` in the same shell will not.
    #[test]
    fn another_yidam_ahead_of_the_pin_on_path_fails() {
        let tmp = derived_repo();
        let pin_dir = tmp.path().join(".yidam").join("bin");
        std::fs::create_dir_all(&pin_dir).unwrap();
        std::fs::write(yidam_bin_path(tmp.path()), b"#!/bin/sh\n").unwrap();
        let cargo_bin = dir_with_yidam(tmp.path(), "cargo-bin");

        let c = check_path(tmp.path(), Some(&path_var(&[&cargo_bin, &pin_dir])));
        assert_eq!(c.verdict, Verdict::Fail);
        assert!(
            c.detail.contains(&cargo_bin.display().to_string()),
            "the shadowing directory must be named: {}",
            c.detail
        );
    }

    #[test]
    fn the_pin_first_on_path_passes() {
        let tmp = derived_repo();
        let pin_dir = tmp.path().join(".yidam").join("bin");
        std::fs::create_dir_all(&pin_dir).unwrap();
        std::fs::write(yidam_bin_path(tmp.path()), b"#!/bin/sh\n").unwrap();
        let cargo_bin = dir_with_yidam(tmp.path(), "cargo-bin");

        let c = check_path(tmp.path(), Some(&path_var(&[&pin_dir, &cargo_bin])));
        assert_eq!(c.verdict, Verdict::Ok);
    }

    /// A pin nothing on PATH resolves is reachable by `mise run` and by absolute path.
    /// Worth saying; not worth failing over.
    #[test]
    fn a_pin_absent_from_path_warns_rather_than_fails() {
        let tmp = derived_repo();
        let pin_dir = tmp.path().join(".yidam").join("bin");
        std::fs::create_dir_all(&pin_dir).unwrap();
        std::fs::write(yidam_bin_path(tmp.path()), b"#!/bin/sh\n").unwrap();
        let empty = tmp.path().join("empty");
        std::fs::create_dir_all(&empty).unwrap();

        assert_eq!(
            check_path(tmp.path(), Some(&path_var(&[&empty]))).verdict,
            Verdict::Warn
        );
    }

    #[test]
    fn a_repository_with_no_pin_does_not_care_about_path_order() {
        let tmp = derived_repo();
        let cargo_bin = dir_with_yidam(tmp.path(), "cargo-bin");
        assert_eq!(
            check_path(tmp.path(), Some(&path_var(&[&cargo_bin]))).verdict,
            Verdict::Ok
        );
    }

    // ── prelude ──────────────────────────────────────────────────────────────

    fn repo_with_prelude(committed: &str) -> TempDir {
        let tmp = derived_repo();
        std::fs::create_dir_all(tmp.path().join(".yidam/.vendor/prelude")).unwrap();
        std::fs::write(
            tmp.path().join(MANIFEST),
            format!("[yidam]\ncommit = \"abc\"\ncommitted = \"{committed}\"\n"),
        )
        .unwrap();
        tmp
    }

    #[test]
    fn a_recent_pin_is_fine_and_still_says_how_old_it_is() {
        let today = crate::dates::days_from_civil_str("2026-08-23").unwrap();
        let tmp = repo_with_prelude("2026-08-01");
        let c = check_prelude(tmp.path(), today);
        assert_eq!(c.verdict, Verdict::Ok);
        assert!(c.detail.contains("22 day(s)"), "{}", c.detail);
    }

    #[test]
    fn a_pin_older_than_the_threshold_warns() {
        let today = crate::dates::days_from_civil_str("2026-08-23").unwrap();
        let tmp = repo_with_prelude("2026-01-01");
        assert_eq!(check_prelude(tmp.path(), today).verdict, Verdict::Warn);
    }

    /// The networked command is named whether or not the pin is old, because "has the origin
    /// moved" is a question this command deliberately declines to answer.
    ///
    /// **On the healthy verdict it is named in the `detail`**, which is the half of the line
    /// a reader is shown when nothing is wrong. It was a remedy on both verdicts until #656,
    /// and the renderer prints a remedy only where something is owed — so the sentence
    /// written to tell a reader where the question is asked reached no reader, and reached
    /// every JSON consumer as an action against a healthy check.
    #[test]
    fn the_prelude_check_always_points_at_the_networked_command() {
        let today = crate::dates::days_from_civil_str("2026-08-23").unwrap();

        let fresh = check_prelude(repo_with_prelude("2026-08-01").path(), today);
        assert_eq!(fresh.verdict, Verdict::Ok);
        assert!(fresh.remedy.is_none(), "{:?}", fresh.remedy);
        assert!(
            fresh.detail.contains("yidam-vendor-status"),
            "the green line must still say where the question is asked: {}",
            fresh.detail
        );

        let stale = check_prelude(repo_with_prelude("2026-01-01").path(), today);
        assert_eq!(stale.verdict, Verdict::Warn);
        assert!(stale
            .remedy
            .unwrap_or_default()
            .contains("yidam-vendor-status"));
    }

    #[test]
    fn no_vendored_prelude_warns() {
        let tmp = derived_repo();
        assert_eq!(check_prelude(tmp.path(), 20_000).verdict, Verdict::Warn);
    }

    // ── governance ───────────────────────────────────────────────────────────

    const ELECTOR_TABLE: &str = "| Name | Branch | Role |\n|---|---|---|\n\
                                  | `auditor` | `ma/auditor` | Holds a position. |\n";

    /// The common case: no `.yidam/sangha/` at all. Single-elector, nothing to weigh.
    #[test]
    fn no_sangha_directory_is_fine() {
        let tmp = derived_repo();
        let c = check_governance(tmp.path());
        assert_eq!(c.verdict, Verdict::Ok);
        assert!(c.detail.contains("single-elector"));
    }

    /// `electors.md` exists — it ships with the collective scaffold — but carries only the
    /// template's placeholder row. Not collective yet; nothing to weigh.
    #[test]
    fn an_electors_file_with_no_real_elector_is_fine() {
        let tmp = derived_repo();
        std::fs::create_dir_all(crate::paths::yidam_sangha_dir(tmp.path())).unwrap();
        std::fs::write(
            crate::paths::yidam_sangha_dir(tmp.path()).join("electors.md"),
            "| Name | Branch | Role |\n|---|---|---|\n\
             | *(no electors registered yet)* | | |\n",
        )
        .unwrap();
        let c = check_governance(tmp.path());
        assert_eq!(c.verdict, Verdict::Ok);
    }

    /// #581's actual measurement: electors registered, no `rigpa/*` resolution, and enough
    /// history that "never" means something. This is the one case that should warn.
    #[test]
    fn collective_with_no_resolution_after_a_long_history_warns() {
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(crate::paths::yidam_sangha_dir(tmp.path())).unwrap();
        std::fs::write(
            crate::paths::yidam_sangha_dir(tmp.path()).join("electors.md"),
            ELECTOR_TABLE,
        )
        .unwrap();
        git(tmp.path(), &["init", "-q", "-b", "main"]);
        git(tmp.path(), &["config", "user.email", "doctor@yidam.test"]);
        git(tmp.path(), &["config", "user.name", "Doctor"]);
        for i in 0..GOVERNANCE_HISTORY_THRESHOLD {
            std::fs::write(tmp.path().join("marker"), i.to_string()).unwrap();
            git(tmp.path(), &["add", "-A"]);
            git(tmp.path(), &["commit", "-q", "-m", &format!("commit {i}")]);
        }

        let c = check_governance(tmp.path());
        assert_eq!(c.verdict, Verdict::Warn, "detail was: {}", c.detail);
        assert!(c.detail.contains("rigpa"), "{}", c.detail);
        assert!(c.remedy.is_some());
    }

    /// The same shape, short of the threshold: a young repository has not had the chance to
    /// run a resolution yet, and this must not read as a problem.
    #[test]
    fn collective_with_no_resolution_but_young_history_is_fine() {
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(crate::paths::yidam_sangha_dir(tmp.path())).unwrap();
        std::fs::write(
            crate::paths::yidam_sangha_dir(tmp.path()).join("electors.md"),
            ELECTOR_TABLE,
        )
        .unwrap();
        git(tmp.path(), &["init", "-q", "-b", "main"]);
        git(tmp.path(), &["config", "user.email", "doctor@yidam.test"]);
        git(tmp.path(), &["config", "user.name", "Doctor"]);
        git(
            tmp.path(),
            &["commit", "-q", "--allow-empty", "-m", "genesis"],
        );

        let c = check_governance(tmp.path());
        assert_eq!(c.verdict, Verdict::Ok, "detail was: {}", c.detail);
    }

    /// A `rigpa/*` ref existing at all clears the finding, no matter how long the history —
    /// this is the repository actually using what it declared.
    #[test]
    fn collective_with_a_resolution_branch_is_fine_regardless_of_history() {
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(crate::paths::yidam_sangha_dir(tmp.path())).unwrap();
        std::fs::write(
            crate::paths::yidam_sangha_dir(tmp.path()).join("electors.md"),
            ELECTOR_TABLE,
        )
        .unwrap();
        git(tmp.path(), &["init", "-q", "-b", "main"]);
        git(tmp.path(), &["config", "user.email", "doctor@yidam.test"]);
        git(tmp.path(), &["config", "user.name", "Doctor"]);
        for i in 0..GOVERNANCE_HISTORY_THRESHOLD {
            git(
                tmp.path(),
                &[
                    "commit",
                    "-q",
                    "--allow-empty",
                    "-m",
                    &format!("commit {i}"),
                ],
            );
        }
        git(tmp.path(), &["branch", "rigpa/first-evolution"]);

        let c = check_governance(tmp.path());
        assert_eq!(c.verdict, Verdict::Ok, "detail was: {}", c.detail);
    }

    // ── report ───────────────────────────────────────────────────────────────

    /// Checks built past [`Answer`], because these are about how a report *counts* its
    /// verdicts and not about what any question answered.
    fn checks_with(verdicts: &[Verdict]) -> Vec<Check> {
        verdicts
            .iter()
            .map(|v| Check {
                id: "x",
                question: "q",
                verdict: *v,
                detail: "d".into(),
                remedy: Some("r".into()),
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
}
