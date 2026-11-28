use super::*;

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
pub(super) fn check_policy(root: &Path) -> Answer {
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
    crate::git::Git::new(root)
        .args(["rev-list", "--count"])
        .rev("HEAD")
        .try_run()
        .and_then(|s| s.parse().ok())
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
pub(super) fn check_governance(root: &Path) -> Answer {
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

#[cfg(test)]
mod tests {
    use super::super::tests::*;
    use super::*;
    use crate::git::fixture::git;
    use tempfile::TempDir;

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
}
