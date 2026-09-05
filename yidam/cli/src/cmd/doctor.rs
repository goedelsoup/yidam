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
            Verdict::Ok => "ok",
            Verdict::Warn => "warn",
            Verdict::Fail => "fail",
            Verdict::Skipped => "skip",
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
    /// The command or edit that resolves it. `None` when there is nothing to do.
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

    fn new(
        id: &'static str,
        question: &'static str,
        verdict: Verdict,
        detail: impl Into<String>,
        remedy: Option<&str>,
    ) -> Self {
        Self {
            id,
            question,
            verdict,
            detail: detail.into(),
            remedy: remedy.map(str::to_string),
        }
    }

    fn skipped(id: &'static str, question: &'static str, why: &str) -> Self {
        Self::new(id, question, Verdict::Skipped, why, None)
    }
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
fn check_repository(root: &Path) -> Check {
    if root.join(".yidam").is_dir() {
        return Check::new(
            Check::REPOSITORY,
            "Am I in a derived repository?",
            Verdict::Ok,
            format!("{}", root.display()),
            None,
        );
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
    Check::new(
        Check::REPOSITORY,
        "Am I in a derived repository?",
        Verdict::Fail,
        detail,
        Some(remedy),
    )
}

/// The pin a derived repository is upgradable from.
///
/// A repository with no recorded origin cannot be upgraded: there is no baseline to compute
/// a forward change against. `yidam-build` refuses outright, which is a good failure at the
/// wrong moment — this is the moment.
fn check_provenance(root: &Path) -> Check {
    const Q: &str = "Does this repository record where it came from?";
    let manifest = root.join(MANIFEST);
    let Ok(text) = std::fs::read_to_string(&manifest) else {
        return Check::new(
            Check::PROVENANCE,
            Q,
            Verdict::Fail,
            format!("no {MANIFEST}"),
            Some("mise run yidam-vendor-update"),
        );
    };
    let pin = ManifestPin::parse(&text);
    match pin.commit.as_deref() {
        Some(commit) if commit != "unknown" => Check::new(
            Check::PROVENANCE,
            Q,
            Verdict::Ok,
            format!(
                "pinned {} ({})",
                &commit[..commit.len().min(12)],
                pin.template.as_deref().unwrap_or("untagged")
            ),
            None,
        ),
        _ => Check::new(
            Check::PROVENANCE,
            Q,
            Verdict::Fail,
            format!("{MANIFEST} records no resolvable commit"),
            Some("mise run yidam-vendor-update"),
        ),
    }
}

/// Is the running binary the one this repository pins?
fn check_binary(root: &Path, running: Option<&Path>) -> Check {
    const Q: &str = "Is the running binary the one this repository pins?";
    match pinned_binary(root, running) {
        Pinned::Unpinned => Check::new(
            Check::BINARY,
            Q,
            Verdict::Ok,
            "this repository pins no binary — nothing can be shadowed",
            None,
        ),
        Pinned::Running => Check::new(
            Check::BINARY,
            Q,
            Verdict::Ok,
            format!("running the pin at {}", yidam_bin_path(root).display()),
            None,
        ),
        // Fail, not warn. This is the failure that reads as success: an older binary
        // missing a subcommand exits with `unrecognized subcommand`, which a script with
        // output redirected cannot tell from having done the work.
        Pinned::Shadowed { pinned, running } => Check::new(
            Check::BINARY,
            Q,
            Verdict::Fail,
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
fn check_path(root: &Path, path_var: Option<&std::ffi::OsStr>) -> Check {
    const Q: &str = "Is `.yidam/bin` ahead on PATH?";
    let pinned = yidam_bin_path(root);
    if !pinned.is_file() {
        return Check::new(
            Check::PATH,
            Q,
            Verdict::Ok,
            "this repository pins no binary — PATH order does not matter",
            None,
        );
    }
    let Some(path_var) = path_var else {
        return Check::skipped(Check::PATH, Q, "PATH is unset");
    };
    let real = |p: &Path| std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
    match first_yidam_on_path(path_var) {
        Some(first) if real(&first) == real(&pinned) => Check::new(
            Check::PATH,
            Q,
            Verdict::Ok,
            format!("PATH resolves yidam to the pin at {}", pinned.display()),
            None,
        ),
        Some(first) => Check::new(
            Check::PATH,
            Q,
            Verdict::Fail,
            format!(
                "PATH resolves yidam to {}, ahead of the pin at {}",
                first.display(),
                pinned.display()
            ),
            Some("add `_.path = [\".yidam/bin\"]` under `[env]` in this repo's mise.toml"),
        ),
        // Not a failure: the pin exists and is reachable, just not by bare name. Every
        // command run through `mise run` still resolves it.
        None => Check::new(
            Check::PATH,
            Q,
            Verdict::Warn,
            format!(
                "no yidam on PATH at all; the pin at {} is reachable only by path",
                pinned.display()
            ),
            Some("add `_.path = [\".yidam/bin\"]` under `[env]` in this repo's mise.toml"),
        ),
    }
}

/// How old is the vendored prelude?
///
/// Local only, on purpose — see this module's header. `committed` is the author date of the
/// pinned commit, not the date this repository ran the vendor step, so it answers how old
/// the prelude is rather than how recently someone typed a command.
fn check_prelude(root: &Path, today: i64) -> Check {
    const Q: &str = "How stale is the vendored prelude?";
    let vendored = root.join(".yidam").join(".vendor").join("prelude");
    if !vendored.is_dir() {
        return Check::new(
            Check::PRELUDE,
            Q,
            Verdict::Warn,
            "no .yidam/.vendor/prelude/ — this repository carries no vendored prelude",
            Some("mise run yidam-vendor-update"),
        );
    }
    let pin = std::fs::read_to_string(root.join(MANIFEST))
        .map(|t| ManifestPin::parse(&t))
        .unwrap_or_default();
    let Some(committed) = pin.committed.filter(|c| c != "unknown") else {
        return Check::new(
            Check::PRELUDE,
            Q,
            Verdict::Warn,
            format!("{MANIFEST} records no pin date — age is unknowable"),
            Some("mise run yidam-vendor-status"),
        );
    };
    let Some(days) = crate::dates::days_from_civil_str(&committed).map(|d| today - d) else {
        return Check::new(
            Check::PRELUDE,
            Q,
            Verdict::Warn,
            format!("{MANIFEST} records an unparseable pin date: {committed}"),
            Some("mise run yidam-vendor-status"),
        );
    };
    let verdict = if days > STALE_PRELUDE_DAYS {
        Verdict::Warn
    } else {
        Verdict::Ok
    };
    Check::new(
        Check::PRELUDE,
        Q,
        verdict,
        format!("pinned {committed} — {days} day(s) ago"),
        // Named on both verdicts: whether the origin has moved is a question this command
        // deliberately does not answer, and the reader should know where it is asked.
        Some("mise run yidam-vendor-status (compares against the origin; needs network)"),
    )
}

/// Is the index built, and is it stale against the corpus?
///
/// Never a failure. A light `reports` build cannot build one, and a repository that has no
/// use for semantic search is not misconfigured for lacking it.
fn check_index(root: &Path) -> Check {
    const Q: &str = "Is the index built, and is it current?";
    let data = crate::cmd::index_status_data(root);
    if !data.index_present {
        return Check::new(
            Check::INDEX,
            Q,
            Verdict::Warn,
            format!("no {}", yidam_index_dir(root).display()),
            Some("yidam index-build (needs the `index` feature)"),
        );
    }
    if !data.meta_present {
        return Check::new(
            Check::INDEX,
            Q,
            Verdict::Warn,
            "index present, but it carries no readable meta.json",
            Some("yidam index-build (needs the `index` feature)"),
        );
    }
    if data.stale_nodes > 0 {
        return Check::new(
            Check::INDEX,
            Q,
            Verdict::Warn,
            format!(
                "built {}, and {} corpus file(s) have changed since",
                data.built.clone().unwrap_or_default(),
                data.stale_nodes
            ),
            Some("yidam index-build (needs the `index` feature)"),
        );
    }
    Check::new(
        Check::INDEX,
        Q,
        Verdict::Ok,
        format!(
            "built {}, {} node(s), model {}",
            data.built.clone().unwrap_or_default(),
            data.node_count.unwrap_or(0),
            data.model.clone().unwrap_or_default()
        ),
        None,
    )
}

/// Are the REGEN blocks current?
///
/// Borrows `regen --check`'s non-writing mode rather than reimplementing the generator
/// list. That is the whole reason [`crate::cmd::stale_blocks`] exists as a
/// function: a second list would be the third one that command was written to prevent.
fn check_regen() -> Check {
    const Q: &str = "Are the REGEN blocks current?";
    match crate::cmd::stale_blocks() {
        Err(e) => Check::new(
            Check::REGEN,
            Q,
            Verdict::Fail,
            format!("could not be computed: {e:#}"),
            None,
        ),
        Ok(stale) if stale.is_empty() => Check::new(
            Check::REGEN,
            Q,
            Verdict::Ok,
            "every REGEN block holds what its generator produces",
            None,
        ),
        Ok(stale) => {
            let names: Vec<String> = stale
                .iter()
                .map(|s| format!("{} ({})", s.file, s.generator))
                .collect();
            Check::new(
                Check::REGEN,
                Q,
                Verdict::Fail,
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
fn check_build() -> Check {
    let b = crate::report::YidamBlock::current();
    Check::new(
        Check::BUILD,
        "Which yidam is this, and what can it do?",
        Verdict::Ok,
        format!(
            "{} ({}) with features: {}",
            b.version,
            b.commit,
            b.features.join(", ")
        ),
        None,
    )
}

// ── assembly ──────────────────────────────────────────────────────────────────

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
    let repository = check_repository(root);
    // Every remaining check asks something *about* a derived repository. Answering them
    // against a directory that is not one produces confident nonsense — "no index", "no
    // provenance" — that reads as a list of things to fix rather than as one thing.
    if repository.verdict == Verdict::Fail {
        let why = "not a yidam repository";
        return vec![
            repository,
            Check::skipped(
                Check::PROVENANCE,
                "Does this repository record where it came from?",
                why,
            ),
            Check::skipped(
                Check::BINARY,
                "Is the running binary the one this repository pins?",
                why,
            ),
            Check::skipped(Check::PATH, "Is `.yidam/bin` ahead on PATH?", why),
            Check::skipped(Check::PRELUDE, "How stale is the vendored prelude?", why),
            Check::skipped(Check::INDEX, "Is the index built, and is it current?", why),
            Check::skipped(Check::REGEN, "Are the REGEN blocks current?", why),
            Check::skipped(Check::CATALOG, "Have any source records aged out?", why),
            Check::skipped(
                Check::CORPORA,
                "Did the corpora this repository depends on arrive?",
                why,
            ),
            Check::skipped(Check::VAULT, "Can this repository reach its vault?", why),
            Check::skipped(
                Check::GOVERNANCE,
                "Is this repository's governance mode carrying its own weight?",
                why,
            ),
            check_build(),
        ];
    }
    vec![
        repository,
        check_provenance(root),
        check_binary(root, running),
        check_path(root, path_var),
        check_prelude(root, today),
        check_index(root),
        check_regen(),
        check_catalog(root, today),
        check_corpora(root),
        check_vault(root),
        check_policy(root),
        check_governance(root),
        check_build(),
    ]
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
///   it — `lint`'s `policy-override` reports each one at `Info` and gates on nothing.
fn check_policy(root: &Path) -> Check {
    const Q: &str = "Do this repository's own rules compile, and which are its own?";

    let policies = match crate::policy::Policies::load(root) {
        Err(e) => return Check::new(
            Check::POLICY,
            Q,
            Verdict::Fail,
            first_line(&e.to_string()),
            Some("`yidam policy check` names the file; a rule that cannot answer refuses nothing."),
        ),
        Ok(p) => p,
    };

    match policies.disallowed_builtins() {
        Err(e) => {
            return Check::new(
                Check::POLICY,
                Q,
                Verdict::Fail,
                first_line(&e.to_string()),
                Some("`yidam policy check` reports the same thing with the file and the call."),
            )
        }
        Ok(found) if !found.is_empty() => {
            return Check::new(
                Check::POLICY,
                Q,
                Verdict::Fail,
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
        return Check::new(
            Check::POLICY,
            Q,
            Verdict::Ok,
            format!("{total} decision(s), all inherited"),
            None,
        );
    }
    Check::new(
        Check::POLICY,
        Q,
        Verdict::Ok,
        format!(
            "{} of {total} decided by this repository: {}",
            local.len(),
            local.join(", ")
        ),
        Some(
            "`yidam policy test` runs the inherited cases against your rules and reports which \
             expectations they no longer meet.",
        ),
    )
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
fn check_governance(root: &Path) -> Check {
    const Q: &str = "Is this repository's governance mode carrying its own weight?";
    let electors_path = crate::paths::yidam_sangha_dir(root).join("electors.md");
    let Ok(text) = std::fs::read_to_string(&electors_path) else {
        return Check::new(
            Check::GOVERNANCE,
            Q,
            Verdict::Ok,
            "single-elector — no .yidam/sangha/electors.md",
            None,
        );
    };
    let electors = crate::cmd::sangha::parse_electors(&text);
    if electors.is_empty() {
        return Check::new(
            Check::GOVERNANCE,
            Q,
            Verdict::Ok,
            "electors.md present, but no ma/* elector is registered yet",
            None,
        );
    }
    let has_resolution = crate::git::phase_refs(root)
        .iter()
        .any(|r| r.kind == crate::git::RefKind::Evolution);
    if has_resolution {
        return Check::new(
            Check::GOVERNANCE,
            Q,
            Verdict::Ok,
            format!(
                "{} elector(s) registered; at least one rigpa/* resolution exists",
                electors.len()
            ),
            None,
        );
    }
    let commits = commit_count(root);
    if commits < GOVERNANCE_HISTORY_THRESHOLD {
        return Check::new(
            Check::GOVERNANCE,
            Q,
            Verdict::Ok,
            format!(
                "{} elector(s) registered, no resolution yet, {commits} commit(s) in — too \
                 early to tell",
                electors.len()
            ),
            None,
        );
    }
    Check::new(
        Check::GOVERNANCE,
        Q,
        Verdict::Warn,
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
fn check_vault(root: &Path) -> Check {
    const Q: &str = "Can this repository reach its vaults?";
    let config = crate::config::load_yidam_config(root).unwrap_or_default();
    let vaults =
        match crate::vault::resolve(&config.vault) {
            Err(e) => return Check::new(
                Check::VAULT,
                Q,
                Verdict::Fail,
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
            Check::new(Check::VAULT, Q, Verdict::Ok, "none declared", None)
        } else {
            Check::new(
                Check::VAULT,
                Q,
                Verdict::Warn,
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
        return Check::new(
            Check::VAULT,
            Q,
            Verdict::Warn,
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
            return Check::new(
                Check::VAULT,
                Q,
                Verdict::Warn,
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
        return Check::new(
            Check::VAULT,
            Q,
            Verdict::Warn,
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
            return Check::new(
                Check::VAULT,
                Q,
                Verdict::Warn,
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
        return Check::new(
            Check::VAULT,
            Q,
            Verdict::Warn,
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
    Check::new(
        Check::VAULT,
        Q,
        Verdict::Ok,
        format!("{configured}; {} artifact(s) cached", named.len()),
        None,
    )
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
fn check_corpora(root: &Path) -> Check {
    const Q: &str = "Did the corpora this repository depends on arrive?";
    const REMEDY: &str = "mise run tonpa-install";

    let config = crate::deps::load_config(&crate::paths::tonpa_config_path(root));
    if config.dependencies.is_empty() {
        // Not a warning, and not "0 missing". A repository that depends on nothing is not
        // half-provisioned, and a line reporting a count here would read as a verdict on a
        // question nobody put — the same reason check_catalog stays quiet without a TTL.
        return Check::new(Check::CORPORA, Q, Verdict::Ok, "none declared", None);
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
        Check::new(Check::CORPORA, Q, Verdict::Fail, detail, Some(REMEDY))
    } else if !unlocked.is_empty() {
        Check::new(Check::CORPORA, Q, Verdict::Warn, detail, Some(REMEDY))
    } else {
        Check::new(Check::CORPORA, Q, Verdict::Ok, detail, None)
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
fn check_catalog(root: &Path, today: i64) -> Check {
    const Q: &str = "Have any source records aged out?";
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
        return Check::new(
            Check::CATALOG,
            Q,
            Verdict::Ok,
            format!(
                "no TTL declared — {} source(s) never expire. Set `[catalog] ttl_days` or \
                 declare `ttl_days:` on an entry.",
                ages.len()
            ),
            None,
        );
    }
    let expired: Vec<&crate::cmd::lint::ttl::Age> =
        ages.iter().filter(|a| a.overdue_days().is_some()).collect();
    let undatable = ages.iter().filter(|a| a.undatable()).count();
    if expired.is_empty() && undatable == 0 {
        return Check::new(
            Check::CATALOG,
            Q,
            Verdict::Ok,
            format!("{governed} source(s) under a TTL, none expired"),
            None,
        );
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
    Check::new(
        Check::CATALOG,
        Q,
        Verdict::Warn,
        detail.join("; "),
        Some("yidam lint  # catalog-expired names each one"),
    )
}

pub(crate) fn render(report: &DoctorReport, root: &Path) -> String {
    let mut out = format!("yidam doctor — {}\n\n", root.display());
    for c in &report.checks {
        out.push_str(&format!(
            "  {:<5} {:<12} {}\n",
            c.verdict.tag(),
            c.id,
            c.detail
        ));
        // The remedy is only shown where it is owed. Printing one under every green line
        // is how a report becomes something people skim past.
        if let Some(remedy) = &c.remedy {
            if matches!(c.verdict, Verdict::Warn | Verdict::Fail) {
                out.push_str(&format!("  {:<5} {:<12} → {remedy}\n", "", "",));
            }
        }
    }
    out.push('\n');
    match (report.failed, report.warned) {
        (0, 0) => out.push_str("Everything checks out."),
        (0, w) => out.push_str(&format!(
            "{w} warning(s), nothing broken.{}",
            if report.strict {
                " --strict: exiting nonzero."
            } else {
                ""
            }
        )),
        (f, 0) => out.push_str(&format!("{f} failing check(s).")),
        (f, w) => out.push_str(&format!("{f} failing check(s), {w} warning(s).")),
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
    #[test]
    fn outside_a_derived_repository_only_the_first_question_is_answered() {
        let tmp = TempDir::new().unwrap();
        let checks = diagnose(tmp.path(), None, None, 20_000);
        assert_eq!(find(&checks, Check::REPOSITORY).verdict, Verdict::Fail);
        for id in [
            Check::PROVENANCE,
            Check::BINARY,
            Check::PATH,
            Check::PRELUDE,
            Check::INDEX,
            Check::REGEN,
        ] {
            assert_eq!(
                find(&checks, id).verdict,
                Verdict::Skipped,
                "{id} should not be answered outside a repository"
            );
        }
        // Which binary is answering is knowable anywhere, and is exactly what a person
        // debugging "it says this is not a repository" needs to see.
        assert_eq!(find(&checks, Check::BUILD).verdict, Verdict::Ok);
        assert!(!DoctorReport::new(checks, false).passed);
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

    /// The remedy is named whether or not the pin is old, because "has the origin moved"
    /// is a question this command deliberately declines to answer.
    #[test]
    fn the_prelude_check_always_points_at_the_networked_command() {
        let today = crate::dates::days_from_civil_str("2026-08-23").unwrap();
        let tmp = repo_with_prelude("2026-08-01");
        assert!(check_prelude(tmp.path(), today)
            .remedy
            .unwrap()
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
        git(tmp.path(), &["commit", "-q", "--allow-empty", "-m", "genesis"]);

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
                &["commit", "-q", "--allow-empty", "-m", &format!("commit {i}")],
            );
        }
        git(tmp.path(), &["branch", "rigpa/first-evolution"]);

        let c = check_governance(tmp.path());
        assert_eq!(c.verdict, Verdict::Ok, "detail was: {}", c.detail);
    }

    // ── report ───────────────────────────────────────────────────────────────

    fn checks_with(verdicts: &[Verdict]) -> Vec<Check> {
        verdicts
            .iter()
            .map(|v| Check::new("x", "q", *v, "d", Some("r")))
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
    /// noise. The prelude check carries one on every verdict precisely so this rule has to
    /// be enforced at render time rather than at construction.
    #[test]
    fn a_remedy_is_rendered_only_where_something_is_wrong() {
        let checks = vec![
            Check::new("green", "q", Verdict::Ok, "fine", Some("do-not-print-me")),
            Check::new("red", "q", Verdict::Fail, "broken", Some("print-me")),
        ];
        let text = render(&DoctorReport::new(checks, false), Path::new("/r"));
        assert!(!text.contains("do-not-print-me"), "{text}");
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
