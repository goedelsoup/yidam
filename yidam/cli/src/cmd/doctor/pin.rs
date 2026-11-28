use super::*;

/// How old a pin may get before it is worth a word.
///
/// Arbitrary, and deliberately generous. The prelude is not milk; a repository pinned in
/// the spring and untouched since is not misconfigured. The number exists so that
/// "pinned some time ago" becomes a date a person can weigh, and so that a repository
/// nobody has re-vendored in a season says so once rather than never.
const STALE_PRELUDE_DAYS: i64 = 90;

/// The remedy every provenance answer points at, and the file that defines it.
///
/// `mise.yidam.toml` is the inherited task layer: `yidam clone` and `yidam overlay` copy it
/// verbatim into a derived repository, and `[yidam-vendor-update]` is declared in it. So it
/// is not merely correlated with being a derived repository — it is *the thing that makes
/// this remedy runnable*, which is why the checks below discriminate on it rather than on,
/// say, the presence of `.yidam/`.
const VENDOR_UPDATE: &str = "mise run yidam-vendor-update";
pub(super) const TASK_LAYER: &str = "mise.yidam.toml";

/// Can the vendor tasks be run here at all?
///
/// A corpus copied out to be read or broken on purpose — `cp -R examples/streamflow /tmp`,
/// which is what the quickstart has a newcomer do in its first ten minutes — is a valid git
/// repository holding a valid corpus, and it is not derived from anything. Before #915 it
/// met a red `fail provenance` there, remedied by a task that directory has no `mise.toml`
/// to define. Both halves were wrong: nothing about the setup was broken, and the one
/// instruction offered could not be carried out.
fn has_task_layer(root: &Path) -> bool {
    root.join(TASK_LAYER).is_file()
}

/// The pin a derived repository is upgradable from.
///
/// A repository with no recorded origin cannot be upgraded: there is no baseline to compute
/// a forward change against. `yidam-build` refuses outright, which is a good failure at the
/// wrong moment — this is the moment.
pub(super) fn check_provenance(root: &Path) -> Answer {
    let manifest = root.join(MANIFEST);
    let Ok(text) = std::fs::read_to_string(&manifest) else {
        return if has_task_layer(root) {
            Answer::fail(format!("no {MANIFEST}"), Some(VENDOR_UPDATE))
        } else {
            Answer::skipped(format!(
                "no {MANIFEST} and no {TASK_LAYER} — not a derived repository, so it has no \
                 pin and is missing nothing"
            ))
        };
    };
    let pin = ManifestPin::parse(&text);
    match pin.commit.as_deref() {
        Some(commit) if commit != "unknown" => Answer::ok(format!(
            "pinned {} ({})",
            &commit[..commit.len().min(12)],
            pin.template.as_deref().unwrap_or("untagged")
        )),
        _ if has_task_layer(root) => Answer::fail(
            format!("{MANIFEST} records no resolvable commit"),
            Some(VENDOR_UPDATE),
        ),
        // The pin is there and unusable, which is wrong however the repository got that
        // way — but with no task layer there is no `yidam-vendor-update` here to offer.
        // Naming the reason beats naming a command that answers "no task yidam-vendor-update".
        _ => Answer::fail(
            format!(
                "{MANIFEST} records no resolvable commit, and with no {TASK_LAYER} this \
                 repository has no vendor task to rewrite it"
            ),
            None,
        ),
    }
}

/// Is the running binary the one this repository pins?
pub(super) fn check_binary(root: &Path, running: Option<&Path>) -> Answer {
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

/// Was the running binary built at the commit this repository pins?
///
/// [`check_binary`] asks *which file* answered; this asks *what that file was built from*.
/// The two come apart on the ordinary re-vendor: `yidam-vendor-update` rewrites
/// `.yidam.toml`'s `commit` and leaves `.yidam/bin/yidam` where it was, so the pin is the
/// file that answers and the prelude it reads is one commit ahead of the code reading it.
///
/// `built` is the binary's own stamp — [`env!`]`("YIDAM_BUILD_COMMIT")` in production, the
/// same value `yidam --version` prints in parentheses — and is passed rather than read so
/// every case is testable from one binary. It is `git rev-parse --short`, whose width follows
/// the clone, while the pin is a full hash; so they agree when one is a prefix of the other.
///
/// A mismatch fails only when this repository pins a binary of its own. With no
/// `.yidam/bin/yidam`, whatever answered is a `yidam` from somewhere else, which this
/// repository never claimed to control — worth a word, not a red build.
///
/// A missing or unresolvable pin is [`check_provenance`]'s finding, so it is `skipped` here
/// rather than reported twice.
pub(super) fn check_build_commit(root: &Path, built: &str) -> Answer {
    const BUILD: &str = "mise run yidam-build";
    let pin = std::fs::read_to_string(root.join(MANIFEST))
        .map(|t| ManifestPin::parse(&t))
        .unwrap_or_default();
    let Some(pinned) = pin
        .commit
        .map(|c| c.trim().to_ascii_lowercase())
        .filter(|c| !c.is_empty() && c != "unknown")
    else {
        return Answer::skipped(format!(
            "{MANIFEST} records no resolvable commit to compare against — see provenance"
        ));
    };
    let built = built.trim().to_ascii_lowercase();
    if built.is_empty() || built == "unknown" {
        return Answer::warn(
            format!(
                "this binary records no build commit, so it cannot be matched to the pin {}",
                short(&pinned)
            ),
            Some(BUILD),
        );
    }
    if pinned.starts_with(&built) || built.starts_with(&pinned) {
        return Answer::ok(format!("built at {}, the pinned commit", short(&built)));
    }
    let detail = format!(
        "this binary was built at {}, but {MANIFEST} pins {}",
        short(&built),
        short(&pinned)
    );
    if yidam_bin_path(root).is_file() {
        Answer::fail(detail, Some(BUILD))
    } else {
        Answer::warn(
            format!("{detail}; this repository pins no binary of its own"),
            Some(BUILD),
        )
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
pub(super) fn check_path(root: &Path, path_var: Option<&std::ffi::OsStr>) -> Answer {
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
pub(super) fn check_prelude(root: &Path, today: i64) -> Answer {
    /// Where the question this command declines to answer *is* asked. Said on both
    /// verdicts, because whether the origin has moved is worth knowing at any pin age —
    /// and said in the `detail` on the healthy one, since that is the half of the line a
    /// reader is shown when nothing is wrong. It was a remedy on both until #656, which
    /// meant the green case named it to a JSON consumer and to nobody else.
    const ORIGIN: &str = "whether the origin has moved is `mise run yidam-vendor-status`, \
                          which needs network";
    let vendored = root.join(".yidam").join(".vendor").join("prelude");
    if !vendored.is_dir() {
        // Same discrimination as `check_provenance`, and for the same reason: a corpus that
        // was never derived carries no vendored prelude and is not missing one.
        return if has_task_layer(root) {
            Answer::warn(
                "no .yidam/.vendor/prelude/ — this repository carries no vendored prelude",
                Some(VENDOR_UPDATE),
            )
        } else {
            Answer::skipped(format!(
                "no .yidam/.vendor/prelude/ and no {TASK_LAYER} — nothing was vendored here \
                 because nothing was derived here"
            ))
        };
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

// ── .yidam.toml ───────────────────────────────────────────────────────────────

/// The fields of `.yidam.toml` this command reads.
///
/// Parsed with `toml` rather than the `sed` the mise tasks use, because this is the one
/// consumer that can afford a real parser and the tasks are not.
#[derive(Debug, Default)]
pub(super) struct ManifestPin {
    commit: Option<String>,
    template: Option<String>,
    committed: Option<String>,
    /// The release this pin names (RFC-0065 §1). `None` on a pin written before it.
    pub(super) release: Option<String>,
}

impl ManifestPin {
    pub(super) fn parse(text: &str) -> Self {
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
            release: field("release"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::*;
    use super::*;
    use tempfile::TempDir;

    /// A corpus copied out to be read: valid, committed, derived from nothing.
    ///
    /// `cp -R examples/streamflow /tmp && git init` — the quickstart's own third step.
    fn copied_corpus() -> TempDir {
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join(".yidam").join("corpus")).unwrap();
        tmp
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

    /// #915. A corpus with no pin *and* no task layer is not a broken derived repository;
    /// it is a corpus nobody derived, and there is nothing for it to be missing.
    ///
    /// `Skipped` rather than `Ok` because the question genuinely was not answered, and
    /// rather than `Warn` because a warn is something owed. It carries no remedy by
    /// construction — which is the half of this that matters, since the defect was an
    /// instruction naming a task the directory has no `mise.toml` to define.
    #[test]
    fn a_corpus_that_was_never_derived_is_not_missing_a_pin() {
        let tmp = copied_corpus();
        let c = check_provenance(tmp.path());
        assert_eq!(c.verdict, Verdict::Skipped, "{}", c.detail);
        assert!(c.remedy.is_none(), "{:?}", c.remedy);
        assert!(c.detail.contains(TASK_LAYER), "{}", c.detail);
    }

    /// The same discrimination, and the same reason: the warn named an unrunnable task.
    #[test]
    fn a_corpus_that_was_never_derived_vendored_no_prelude() {
        let tmp = copied_corpus();
        let c = check_prelude(tmp.path(), 20_000);
        assert_eq!(c.verdict, Verdict::Skipped, "{}", c.detail);
        assert!(c.remedy.is_none(), "{:?}", c.remedy);
    }

    /// An unusable pin is wrong wherever it is found — but where the vendor task does not
    /// exist, saying so beats naming it. A remedy a reader cannot run is worse than none:
    /// they run it, get `no task yidam-vendor-update`, and now have two problems.
    #[test]
    fn an_unusable_pin_without_the_task_layer_still_fails_and_offers_nothing() {
        let tmp = copied_corpus();
        std::fs::write(tmp.path().join(MANIFEST), "[yidam]\ncommit = \"unknown\"\n").unwrap();
        let c = check_provenance(tmp.path());
        assert_eq!(c.verdict, Verdict::Fail, "{}", c.detail);
        assert!(c.remedy.is_none(), "{:?}", c.remedy);
        assert!(c.detail.contains(TASK_LAYER), "{}", c.detail);
    }

    /// The failing arms must keep naming a command, or the fix above has turned the
    /// remedy off for the repositories that do need it.
    #[test]
    fn a_derived_repository_still_gets_the_vendor_remedy() {
        let missing = check_provenance(derived_repo().path());
        assert_eq!(missing.verdict, Verdict::Fail);
        assert_eq!(missing.remedy.as_deref(), Some(VENDOR_UPDATE));

        let tmp = derived_repo();
        std::fs::write(tmp.path().join(MANIFEST), "[yidam]\ncommit = \"unknown\"\n").unwrap();
        let unusable = check_provenance(tmp.path());
        assert_eq!(unusable.verdict, Verdict::Fail);
        assert_eq!(unusable.remedy.as_deref(), Some(VENDOR_UPDATE));
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

    // ── commit ───────────────────────────────────────────────────────────────

    const PIN: &str = "0123456789abcdef0123456789abcdef01234567";

    fn pinned_at(commit: Option<&str>, with_binary: bool) -> TempDir {
        let tmp = derived_repo();
        let manifest = match commit {
            Some(c) => format!("[yidam]\ncommit = \"{c}\"\n"),
            None => "[yidam]\ntemplate = \"cli/v0.2.0\"\n".to_string(),
        };
        std::fs::write(tmp.path().join(MANIFEST), manifest).unwrap();
        if with_binary {
            let bin = yidam_bin_path(tmp.path());
            std::fs::create_dir_all(bin.parent().unwrap()).unwrap();
            std::fs::write(&bin, b"#!/bin/sh\n").unwrap();
        }
        tmp
    }

    /// The build stamp is `git rev-parse --short`, so its width is the clone's choice (7 in a
    /// shallow clone, 8 in this one) and never the pin's 40. Equality would fail every build.
    #[test]
    fn a_short_build_commit_matches_the_full_pin_it_abbreviates() {
        let tmp = pinned_at(Some(PIN), true);
        for built in ["0123456", "01234567", PIN, "0123456789ABCDEF"] {
            let c = check_build_commit(tmp.path(), built);
            assert_eq!(c.verdict, Verdict::Ok, "{built}: {}", c.detail);
            assert!(c.remedy.is_none());
        }
    }

    /// The re-vendor that moved `.yidam.toml` and left `.yidam/bin/yidam` behind: the pin
    /// answers, and it is not the pin any more.
    #[test]
    fn a_pinned_binary_built_elsewhere_fails_and_names_both_commits() {
        let tmp = pinned_at(Some(PIN), true);
        let c = check_build_commit(tmp.path(), "fedcba9");
        assert_eq!(c.verdict, Verdict::Fail);
        assert!(c.detail.contains("fedcba9"), "{}", c.detail);
        assert!(c.detail.contains(short(PIN)), "{}", c.detail);
        assert_eq!(c.remedy.as_deref(), Some("mise run yidam-build"));
    }

    /// No `.yidam/bin/yidam`: whatever answered was never this repository's to control.
    #[test]
    fn a_mismatch_with_no_pinned_binary_only_warns() {
        let tmp = pinned_at(Some(PIN), false);
        let c = check_build_commit(tmp.path(), "fedcba9");
        assert_eq!(c.verdict, Verdict::Warn);
        assert!(c.detail.contains("fedcba9"), "{}", c.detail);
        assert!(c.detail.contains(short(PIN)), "{}", c.detail);
        assert_eq!(c.remedy.as_deref(), Some("mise run yidam-build"));
    }

    /// `build.rs` stamps `unknown` when it finds no git to ask. That binary cannot be held to
    /// the pin, and saying so is the finding — not a pass, and not a mismatch.
    #[test]
    fn a_binary_with_no_build_commit_warns_rather_than_matching_or_failing() {
        let tmp = pinned_at(Some(PIN), true);
        for built in ["unknown", "", "  "] {
            let c = check_build_commit(tmp.path(), built);
            assert_eq!(c.verdict, Verdict::Warn, "{built:?}: {}", c.detail);
            assert!(c.detail.contains("no build commit"), "{}", c.detail);
        }
    }

    /// No commit to compare is `provenance`'s finding. Reporting it here too would be one
    /// defect counted twice — and an empty pin is a prefix of every build commit, which is
    /// the false pass this guards.
    #[test]
    fn a_manifest_with_no_commit_is_skipped_not_matched() {
        for commit in [None, Some(""), Some("unknown")] {
            let tmp = pinned_at(commit, true);
            let c = check_build_commit(tmp.path(), "0123456");
            assert_eq!(c.verdict, Verdict::Skipped, "{commit:?}: {}", c.detail);
        }
        let tmp = derived_repo();
        assert_eq!(
            check_build_commit(tmp.path(), "0123456").verdict,
            Verdict::Skipped
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
}
