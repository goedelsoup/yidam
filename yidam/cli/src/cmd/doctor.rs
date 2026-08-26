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
        check_build(),
    ]
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
