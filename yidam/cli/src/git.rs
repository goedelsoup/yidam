use std::path::Path;

/// Hash of the root (genesis) commit.
///
/// `git log --reverse --max-count=1` does NOT work for this: git applies the
/// count limit before reversing, returning the newest commit instead.
///
/// Public because it is the only identity every corpus has and no two corpora share. The RDF
/// export names its subjects with it — a declared package name would be readable and **zero of
/// sixteen corpora declare one**, so a subject built on it would have said `local` for all
/// sixteen and left RFC-0032 §2's collision exactly where it was.
///
/// **`None` unless `root` is the top of its own repository**, and that is the whole of #792.
/// Git walks *up*, so a corpus that is not itself a repository was described by whichever
/// repository encloses it — and every corpus nested in one host got the host's answer. All four
/// corpora under `examples/` minted subjects as `urn:yidam:094509a128f4`, which is this
/// template's own root commit, so their four `owl:Ontology` resources were one resource
/// asserting four different labels. A constant shared by every corpus inside a host is exactly
/// the conflation §2 exists to prevent, arriving through the code that implements it.
///
/// **Why refuse rather than fall back to the corpus's own first commit.** "The earliest commit
/// touching this directory" is available, is unique per corpus, and would have kept those four
/// exports working with four distinct identities. It also *changes* when the corpus is moved out
/// of its host into a repository of its own, and a published subject must not change — which is
/// the argument [`crate::model::Provenance::genesis_hash`] already makes against a declared
/// nickname. Refusing has the opposite shape: extracting a nested corpus gives it an identity it
/// did not have, and nothing that was already published moves.
///
/// Everything derived from this follows it. [`genesis_date`] and [`genesis_message`] both start
/// here, so a nested corpus no longer reports its host's genesis date as its own age, or the
/// host's genesis message as its domain.
pub fn genesis_hash(root: &Path) -> Option<String> {
    if !is_repository_root(root) {
        return None;
    }
    let out = std::process::Command::new("git")
        .current_dir(root)
        .args(["rev-list", "--max-parents=0", "HEAD"])
        .output()
        .ok()?;
    String::from_utf8(out.stdout)
        .ok()?
        .lines()
        .next()
        .map(str::to_string)
        .filter(|s| !s.is_empty())
}

/// Is `root` the top of the git repository it is in, rather than a directory inside one?
///
/// Both sides are canonicalised before comparing, because git answers with a resolved path and
/// the caller's may not be. On macOS `/tmp` is a symlink to `/private/tmp`, so a corpus under a
/// temporary directory compares unequal to itself without this — the test suite is entirely
/// built on such directories, so the naive comparison fails everywhere it is exercised and
/// nowhere a person would look.
///
/// `false` when there is no repository at all: nothing to be the top of.
fn is_repository_root(root: &Path) -> bool {
    let out = std::process::Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .ok();
    let Some(top) = out
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
    else {
        return false;
    };
    match (
        std::fs::canonicalize(root),
        std::fs::canonicalize(Path::new(&top)),
    ) {
        (Ok(a), Ok(b)) => a == b,
        // A path that cannot be resolved is not one we can claim is the repository root.
        _ => false,
    }
}

/// ISO date of the corpus's own genesis commit, or [`UNKNOWN_COMMIT`] when it has none.
///
/// The sentinel used to read `"no commits"`, which was true of the only case that could reach
/// it. Since #792 a second case can: a corpus that is a directory inside a repository has no
/// genesis of *its own* while the tree it sits in has plenty, and telling its author there are
/// no commits would be false. `"unknown"` is true of both, and is the word
/// [`head_commit_short`] already uses for the same absence.
pub fn genesis_date(root: &Path) -> String {
    genesis_hash(root)
        .and_then(|hash| {
            let out = std::process::Command::new("git")
                .current_dir(root)
                .args(["log", "-1", "--format=%as", &hash])
                .output()
                .ok()?;
            String::from_utf8(out.stdout).ok()
        })
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| UNKNOWN_COMMIT.to_string())
}

/// What [`head_commit_short`] answers where there is no commit to name — no git repository,
/// or one with no HEAD yet.
///
/// A named constant because two callers now branch on it: the MCP handshake reports staleness
/// as `null` rather than `false` when this is the commit, and a string literal compared in two
/// files is a contract nobody declared.
pub const UNKNOWN_COMMIT: &str = "unknown";

pub fn head_commit_short(root: &Path) -> String {
    let out = std::process::Command::new("git")
        .current_dir(root)
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok();
    out.and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| UNKNOWN_COMMIT.to_string())
}

pub fn genesis_message(root: &Path) -> String {
    genesis_hash(root)
        .and_then(|hash| {
            let out = std::process::Command::new("git")
                .current_dir(root)
                .args(["log", "-1", "--format=%B", &hash])
                .output()
                .ok()?;
            String::from_utf8(out.stdout).ok()
        })
        .map(|s| s.trim().to_string())
        .unwrap_or_default()
}

/// What kind of thing a tracked ref is. The three namespaces are not interchangeable and
/// counting them as one is what [`active_phase_count`] used to do.
///
/// A derived repository reported **26 active phase(s)** while holding exactly one phase. The
/// 26 were three elector positions and twenty-three settled evolutions; its twenty-seven
/// `phase/*` refs — the namespace [`PHASES.md`] actually defines a phase in — were not read
/// at all. Two of those errors cancel into a plausible-looking number, which is why this is
/// typed rather than left to a prefix test at each call site.
///
/// [`PHASES.md`]: https://github.com/goedelsoup/yidam/blob/main/yidam/prelude/PHASES.md
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum RefKind {
    /// `ma/<elector>` — a standing elector position. Long-lived by design: divergence from
    /// the baseline is its purpose, so it is never settled and never counted as a phase.
    Position,
    /// `rigpa/<evolution>` — a resolution branch. Bounded; settles onto the baseline.
    Evolution,
    /// `phase/<name>` — a bounded investigation. Settles onto the baseline, and PHASES.md
    /// prescribes deleting the ref afterwards.
    Phase,
}

impl RefKind {
    /// Whether this kind is bounded work that ends by settling onto the baseline.
    ///
    /// A position is not: an elector's ref is *meant* to sit ahead of the baseline forever,
    /// so asking whether it has been merged is a category error rather than a hygiene check.
    pub fn settles(self) -> bool {
        matches!(self, Self::Evolution | Self::Phase)
    }
}

/// A tracked inquiry ref: a branch under `ma/*`, `rigpa/*` or `phase/*`, wherever it lives.
#[derive(Debug, PartialEq, Eq)]
pub struct PhaseRef {
    /// The ref, with any remote prefix removed: `ma/substrate-survey`.
    pub name: String,
    /// The ref to read for it — the local branch when there is one, else the
    /// remote-tracking ref. Never assume this equals `name`.
    pub git_ref: String,
    /// Which namespace it belongs to.
    pub kind: RefKind,
}

fn kind_of(name: &str) -> Option<RefKind> {
    match name.split_once('/')? {
        ("ma", _) => Some(RefKind::Position),
        ("rigpa", _) => Some(RefKind::Evolution),
        ("phase", _) => Some(RefKind::Phase),
        _ => None,
    }
}

fn is_phase(name: &str) -> bool {
    kind_of(name).is_some()
}

/// Reduce `git for-each-ref --format=%(refname:short) refs/heads refs/remotes` to phases.
///
/// Deduped by name, because `ma/foo` and `origin/ma/foo` are one phase seen from two
/// sides. Local wins: it is the ref whose tip somebody is actually moving. Sorted, because
/// this feeds a committed REGEN block and an unstable order is a diff on every run.
pub(crate) fn parse_phase_refs(out: &str) -> Vec<PhaseRef> {
    let mut by_name: std::collections::BTreeMap<String, String> = std::collections::BTreeMap::new();
    for line in out.lines().map(str::trim).filter(|l| !l.is_empty()) {
        // A local branch is already its own name. A remote-tracking ref carries the remote
        // as a first segment — `origin/ma/foo` — and names the same phase. `origin/HEAD`
        // and every ordinary branch fall through both arms and are skipped.
        let name = if is_phase(line) {
            line
        } else {
            match line.split_once('/') {
                Some((_remote, rest)) if is_phase(rest) => rest,
                _ => continue,
            }
        };
        let entry = by_name
            .entry(name.to_string())
            .or_insert_with(|| line.to_string());
        if line == name {
            *entry = line.to_string();
        }
    }
    by_name
        .into_iter()
        .filter_map(|(name, git_ref)| {
            let kind = kind_of(&name)?;
            Some(PhaseRef {
                name,
                git_ref,
                kind,
            })
        })
        .collect()
}

/// Every active phase, read from local **and** remote-tracking refs.
///
/// This read `git branch --list` — local only — and that made it unusable for the thing it
/// feeds. `actions/checkout` creates exactly one local branch, so CI counted 0 against a
/// repository holding 24, and [`crate::cmd::status`] writes that count into a REGEN block:
/// the committed block and the regenerated one disagreed on every push, and the graph gate
/// failed regardless of what the commit contained. A derived repository ran that way for an
/// extended period and shipped a local stopgap.
///
/// The contract this now keeps, which is the one anything feeding a REGEN block owes:
/// **the same bytes in a fresh clone as on a developer machine.**
///
/// Two consequences, both deliberate. A phase held only on an unpushed local branch is
/// still counted here but will not be counted in CI — push it, or accept the disagreement.
/// And a developer whose remote-tracking refs are stale sees what their last fetch saw.
pub fn phase_refs(root: &Path) -> Vec<PhaseRef> {
    let out = std::process::Command::new("git")
        .current_dir(root)
        .args([
            "for-each-ref",
            "--format=%(refname:short)",
            "refs/heads",
            "refs/remotes",
        ])
        .output()
        .ok();
    out.and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| parse_phase_refs(&s))
        .unwrap_or_default()
}

/// The branch bounded work settles onto: `main` if it exists, else `master`.
pub(crate) fn base_branch(root: &Path) -> Option<String> {
    ["main", "master"].into_iter().find_map(|name| {
        let out = std::process::Command::new("git")
            .current_dir(root)
            .args(["rev-parse", "--verify", "--quiet", name])
            .output()
            .ok()?;
        out.status.success().then(|| name.to_string())
    })
}

/// Whether `git_ref` is already an ancestor of the baseline — its work landed.
///
/// A repository with no baseline branch has nothing to have settled onto, so everything
/// reads as unsettled. That is the honest answer during bootstrap, before `main` exists.
///
/// **Ancestry is the whole test, and it is only true of the merge PHASES.md prescribes.** Two of
/// GitHub's three merge buttons write a single-parent commit onto the baseline and leave the
/// branch tip where it was, so this is false for them permanently. [`is_rewritten`] is the
/// second question, asked only when this one says no.
fn is_settled(root: &Path, git_ref: &str, base: Option<&str>) -> bool {
    let Some(base) = base else { return false };
    if base == git_ref {
        return false;
    }
    std::process::Command::new("git")
        .current_dir(root)
        .args(["merge-base", "--is-ancestor", git_ref, base])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn git_lines(root: &Path, args: &[&str]) -> Option<Vec<String>> {
    let out = std::process::Command::new("git")
        .current_dir(root)
        .args(args)
        .output()
        .ok()?;
    out.status.success().then(|| {
        String::from_utf8_lossy(&out.stdout)
            .lines()
            .map(str::to_string)
            .collect()
    })
}

/// Has `git_ref`'s work reached the baseline by a merge that rewrote its commits?
///
/// Asked only of a ref that failed [`is_settled`], because **ancestry is only true of the merge
/// `PHASES.md` prescribes** — `--no-ff`, keeping the synthesis event. GitHub's *Rebase and merge*
/// and *Squash and merge* both write a single-parent commit onto the baseline and leave the branch
/// tip untouched, so the ancestry test is false and stays false. The phase then reads `active`
/// forever, and nothing done inside the repository can change it: one derived repository reached
/// **22 phases** that way, every one of them complete, and a whole phase spent writing the
/// prescribed `phase:` commits did not move the number, because the number is computed over refs.
///
/// **Two tests, because the two strategies are not detectable the same way.** Measured on a
/// fixture built with each of git's three merges:
///
/// | strategy | ancestor of base | `git cherry` | touched files agree |
/// |---|---|---|---|
/// | `merge --no-ff` (prescribed) | **yes** | — | — |
/// | rebase merge | no | all `-` | yes |
/// | squash merge | no | all `+` | **yes** |
/// | genuinely in flight | no | `+` | no |
/// | partially merged | no | `-+` | no |
///
/// `git cherry` marks a commit `-` when the baseline already holds an equivalent patch, which
/// answers a rebase merge exactly and answers a squash merge **wrongly**: a squash *combines* the
/// patches, so no individual commit of the branch is upstream. That was the case the issue
/// flagged as unsettled, and it is why the second test exists — every file the branch touched
/// agreeing with the baseline is true of both strategies.
///
/// **The residue, stated rather than hidden.** A squash-merged branch whose files the baseline
/// then edits again satisfies neither test, and reads `active`. Closing that needs the branch's
/// *combined* patch-id searched for among the baseline's commits, which is exact and unbounded in
/// cost; this is the cheap 90% and it errs toward `active`, which is the safe direction — calling
/// finished work in-flight is a wrong number, and calling in-flight work finished hides it.
fn is_rewritten(root: &Path, git_ref: &str, base: Option<&str>) -> bool {
    let Some(base) = base else { return false };
    if base == git_ref {
        return false;
    }
    // A ref with no commits of its own has nothing to have been rewritten. `ref_state` cannot
    // reach this — such a ref is an ancestor of the baseline, so `is_settled` claimed it — but
    // the guard is load-bearing rather than defensive: `all()` over an empty iterator is
    // **vacuously true**, so without it a ref carrying nothing would report as settled.
    let Some(cherry) = git_lines(root, &["cherry", base, git_ref]) else {
        return false;
    };
    if cherry.is_empty() {
        return false;
    }

    // Rebase merge: every commit on the branch has an equivalent patch upstream.
    if cherry.iter().all(|l| l.starts_with('-')) {
        return true;
    }

    // Squash merge: the branch introduces no file content the baseline lacks. Compared against
    // the merge base's file list rather than the whole tree, so unrelated work on the baseline
    // does not mask the answer.
    let Some(merge_base) = git_lines(root, &["merge-base", base, git_ref])
        .and_then(|l| l.first().cloned())
        .filter(|s| !s.is_empty())
    else {
        return false;
    };
    let Some(files) = git_lines(root, &["diff", "--name-only", &merge_base, git_ref]) else {
        return false;
    };
    let files: Vec<&str> = files
        .iter()
        .map(String::as_str)
        .filter(|f| !f.is_empty())
        .collect();
    if files.is_empty() {
        return false;
    }
    let mut args = vec!["diff", "--quiet", base, git_ref, "--"];
    args.extend_from_slice(&files);
    std::process::Command::new("git")
        .current_dir(root)
        .args(&args)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// What the tracked refs actually hold, split by the four things they can be.
#[derive(Debug, Default, PartialEq, Eq, serde::Serialize)]
pub struct PhaseTally {
    /// Bounded work not yet on the baseline — the only number that means "in flight".
    pub active: usize,
    /// Bounded work already merged, whose ref outlived its settlement. PHASES.md
    /// prescribes deleting these; leaving them is the drift this number makes visible.
    pub settled: usize,
    /// Settled by a merge that rewrote its commits, so the ref is not an ancestor of the
    /// baseline and never will be. The same drift as `settled` plus a second fact: the merge did
    /// not keep the synthesis event `PHASES.md` asks for, which is a repository *setting* rather
    /// than anything a phase did.
    ///
    /// Separate from `settled` so the report can say the useful thing. "22 phases were settled
    /// with a button that dissolved the merge commit" names a fix — change the merge strategy —
    /// where "22 settled" reads as ordinary drift and "22 active" is simply false.
    pub rewritten: usize,
    /// Standing elector positions. Neither active work nor drift — a third thing.
    pub positions: usize,
}

/// Every value [`ref_state`] can return.
///
/// The roster exists because `report.schema.json` declares `state` as a **closed enum**, and
/// nothing compared the two. Adding `rewritten` widened a vocabulary a validating consumer would
/// have rejected, and every test stayed green — the report golden's fixture has no phase in that
/// state, so the enum was never exercised. A state the code can emit and the schema does not name
/// is a rejected report; one the schema names and the code cannot emit is a branch nobody can
/// take. `report_goldens.rs` reads this and compares both ways.
pub const REF_STATES: [&str; 4] = ["active", "settled", "rewritten", "position"];

/// What a ref currently is, in one word: `active`, `settled`, `rewritten`, or `position`.
///
/// The single classifier. `yidam status` counts these and `yidam phases` prints them, and
/// they must not be able to disagree — a derived repository once held three separate
/// implementations of "does this node cite that source" and only two of them agreed.
///
/// `rewritten` is asked only after `settled` says no, so the prescribed merge costs no extra git
/// invocation and a genuinely in-flight phase pays two.
pub(crate) fn ref_state(root: &Path, r: &PhaseRef, base: Option<&str>) -> &'static str {
    if !r.kind.settles() {
        "position"
    } else if is_settled(root, &r.git_ref, base) {
        "settled"
    } else if is_rewritten(root, &r.git_ref, base) {
        "rewritten"
    } else {
        "active"
    }
}

/// Classify every tracked ref against the baseline.
///
/// This replaced a plain `phase_refs(root).len()`, which counted all three namespaces as
/// active phases while reading none of `phase/*`. See [`RefKind`] for what that cost.
pub fn phase_tally(root: &Path) -> PhaseTally {
    let base = base_branch(root);
    let mut tally = PhaseTally::default();
    for r in phase_refs(root) {
        match ref_state(root, &r, base.as_deref()) {
            "position" => tally.positions += 1,
            "settled" => tally.settled += 1,
            "rewritten" => tally.rewritten += 1,
            _ => tally.active += 1,
        }
    }
    tally
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── #773: a phase merged with a rewriting button is not in flight ─────────

    /// A baseline with one commit, and three phase branches each carrying two commits.
    ///
    /// `merge.ff = true` explicitly: this machine's global config sets it to `false`, which makes
    /// `git merge --squash` refuse with "options '--squash' and '--no-ff' cannot be used
    /// together" — a fixture that silently did not squash, and read as evidence for a while.
    fn phases_repo() -> tempfile::TempDir {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path();
        git(root, &["init", "-q", "-b", "main"]);
        git(root, &["config", "user.email", "t@t.com"]);
        git(root, &["config", "user.name", "T"]);
        git(root, &["config", "commit.gpgsign", "false"]);
        git(root, &["config", "merge.ff", "true"]);
        commit_file(root, "base.txt", "one", "genesis: base");
        for name in ["ff", "rebase", "squash", "active", "partial"] {
            git(
                root,
                &["checkout", "-q", "-b", &format!("phase/{name}"), "main"],
            );
            commit_file(
                root,
                &format!("{name}-a"),
                "A",
                &format!("establish: {name} first"),
            );
            commit_file(
                root,
                &format!("{name}-b"),
                "B",
                &format!("establish: {name} second"),
            );
            git(root, &["checkout", "-q", "main"]);
        }
        // Unrelated baseline work, so the file comparison cannot pass by the trees being equal.
        commit_file(root, "other.txt", "x", "chore: unrelated work on main");
        tmp
    }

    fn commit_file(root: &Path, name: &str, body: &str, msg: &str) {
        std::fs::write(root.join(name), format!("{body}\n")).unwrap();
        git(root, &["add", "-A"]);
        git(root, &["commit", "-q", "--no-gpg-sign", "-m", msg]);
    }

    fn rev(root: &Path, r: &str) -> String {
        String::from_utf8(
            std::process::Command::new("git")
                .current_dir(root)
                .args(["rev-parse", r])
                .output()
                .unwrap()
                .stdout,
        )
        .unwrap()
        .trim()
        .to_string()
    }

    /// What GitHub's **Rebase and merge** does: rebase onto the baseline, fast-forward the
    /// baseline to it, and **leave the branch tip where it was**. The last part is the defect —
    /// a fixture that rebases the branch itself makes it an ancestor and tests nothing.
    fn rebase_merge(root: &Path, branch: &str) {
        let tip = rev(root, branch);
        git(root, &["checkout", "-q", "--detach", branch]);
        git(root, &["rebase", "-q", "main"]);
        let rebased = rev(root, "HEAD");
        git(root, &["checkout", "-q", "main"]);
        git(root, &["merge", "-q", "--ff-only", &rebased]);
        git(root, &["branch", "-f", branch, &tip]);
    }

    /// What **Squash and merge** does: one commit carrying the combined patch, branch untouched.
    fn squash_merge(root: &Path, branch: &str, msg: &str) {
        git(root, &["merge", "-q", "--squash", branch]);
        git(root, &["commit", "-q", "--no-gpg-sign", "-m", msg]);
    }

    fn state(root: &Path, branch: &str) -> &'static str {
        let r = PhaseRef {
            name: branch.to_string(),
            git_ref: branch.to_string(),
            kind: RefKind::Phase,
        };
        ref_state(root, &r, Some("main"))
    }

    /// The whole of #773 in one table, over the three merges git can perform.
    ///
    /// `is_settled` asks whether the ref is an ancestor of the baseline, which is true only of
    /// the `--no-ff` merge `PHASES.md` prescribes. GitHub's other two buttons write a
    /// single-parent commit and leave the tip alone, so the phase read `active` — the number
    /// `PhaseTally` documents as *"the only number that means 'in flight'"* — permanently. One
    /// derived repository reached 22 that way, every one complete, and a whole phase spent
    /// writing the prescribed `phase:` commits could not move it, because it is computed over
    /// refs.
    #[test]
    fn a_phase_merged_by_any_of_the_three_buttons_is_not_in_flight() {
        let tmp = phases_repo();
        let root = tmp.path();
        git(
            root,
            &[
                "merge",
                "-q",
                "--no-ff",
                "--no-edit",
                "-m",
                "phase: ff",
                "phase/ff",
            ],
        );
        rebase_merge(root, "phase/rebase");
        squash_merge(root, "phase/squash", "phase: squash");
        squash_merge(root, "phase/partial", "chore: only the first half");
        // Undo half of the squashed `partial` work, so its branch still holds content the
        // baseline lacks — merged in part is in flight.
        std::fs::remove_file(root.join("partial-b")).unwrap();
        git(root, &["add", "-A"]);
        git(
            root,
            &[
                "commit",
                "-q",
                "--no-gpg-sign",
                "-m",
                "chore: drop the second half",
            ],
        );

        assert_eq!(state(root, "phase/ff"), "settled", "the prescribed merge");
        assert_eq!(
            state(root, "phase/rebase"),
            "rewritten",
            "GitHub's rebase button"
        );
        assert_eq!(
            state(root, "phase/squash"),
            "rewritten",
            "GitHub's squash button"
        );
        assert_eq!(state(root, "phase/active"), "active", "never merged");
        assert_eq!(
            state(root, "phase/partial"),
            "active",
            "merged in part only"
        );
    }

    /// The two strategies are not detectable the same way, which is the question the issue left
    /// open. `git cherry` marks a commit `-` when the baseline holds an equivalent patch — exact
    /// for a rebase merge, and **wrong for a squash**, which combines the patches so no
    /// individual commit is upstream. Asserted on the git primitives rather than on
    /// `is_rewritten`, so the measurement the design rests on is itself a test.
    #[test]
    fn cherry_answers_a_rebase_merge_and_not_a_squash() {
        let tmp = phases_repo();
        let root = tmp.path();
        rebase_merge(root, "phase/rebase");
        squash_merge(root, "phase/squash", "phase: squash");

        let marks = |b: &str| {
            git_lines(root, &["cherry", "main", b])
                .unwrap()
                .iter()
                .filter_map(|l| l.chars().next())
                .collect::<String>()
        };
        assert_eq!(
            marks("phase/rebase"),
            "--",
            "a rebase merge is patch-equivalent upstream"
        );
        assert_eq!(
            marks("phase/squash"),
            "++",
            "a squash combines the patches, so no commit of the branch is upstream — this is why \
             `git cherry` alone cannot answer #773"
        );
        // Both are nonetheless settled, by the second test.
        assert_eq!(state(root, "phase/rebase"), "rewritten");
        assert_eq!(state(root, "phase/squash"), "rewritten");
    }

    /// The residue, asserted so it cannot be mistaken for a fix. A squash-merged branch whose
    /// files the baseline then edits again satisfies neither test and reads `active`. Closing it
    /// needs the branch's combined patch-id searched among the baseline's commits — exact and
    /// unbounded in cost. This test is here to make the limit visible and to go red if someone
    /// closes it, which is the moment to delete it.
    #[test]
    fn a_squash_the_baseline_then_edits_is_the_known_residue() {
        let tmp = phases_repo();
        let root = tmp.path();
        squash_merge(root, "phase/squash", "phase: squash");
        assert_eq!(state(root, "phase/squash"), "rewritten", "before the edit");
        commit_file(
            root,
            "squash-a",
            "edited",
            "chore: main edits the same file again",
        );
        assert_eq!(
            state(root, "phase/squash"),
            "active",
            "known residue: with the file changed again neither test can see the merge"
        );
    }

    /// A rebase-merged branch the baseline then edits is **not** in the residue: `git cherry`
    /// still marks its commits `-`, because patch equivalence is a fact about history rather than
    /// about the current tree.
    #[test]
    fn a_rebase_merge_survives_the_baseline_moving_on() {
        let tmp = phases_repo();
        let root = tmp.path();
        rebase_merge(root, "phase/rebase");
        commit_file(
            root,
            "rebase-a",
            "edited",
            "chore: main edits the same file again",
        );
        assert_eq!(state(root, "phase/rebase"), "rewritten");
    }

    /// An elector position is a third thing and must not be swept up by either test. `ma/*` is
    /// *meant* to sit ahead of the baseline forever, so asking whether it settled is a category
    /// error — and its files agreeing with the baseline would otherwise make it `rewritten`.
    #[test]
    fn an_elector_position_is_never_rewritten() {
        let tmp = phases_repo();
        let root = tmp.path();
        git(root, &["branch", "ma/auditor", "phase/rebase"]);
        rebase_merge(root, "phase/rebase");
        let r = PhaseRef {
            name: "ma/auditor".to_string(),
            git_ref: "ma/auditor".to_string(),
            kind: RefKind::Position,
        };
        assert_eq!(ref_state(root, &r, Some("main")), "position");
    }

    /// Every state `ref_state` actually returns is on the roster the schema is checked against.
    /// A literal added at the `match` and not to `REF_STATES` would leave the schema comparison
    /// passing over a value it has never seen — the same defect as the enum, one level in.
    #[test]
    fn every_state_ref_state_returns_is_on_the_roster() {
        let tmp = phases_repo();
        let root = tmp.path();
        git(
            root,
            &[
                "merge",
                "-q",
                "--no-ff",
                "--no-edit",
                "-m",
                "phase: ff",
                "phase/ff",
            ],
        );
        rebase_merge(root, "phase/rebase");
        git(root, &["branch", "ma/auditor", "main"]);
        let mut seen: Vec<&str> = Vec::new();
        for r in phase_refs(root) {
            let st = ref_state(root, &r, Some("main"));
            assert!(
                REF_STATES.contains(&st),
                "`{st}` is not on REF_STATES, so the schema comparison cannot see it"
            );
            if !seen.contains(&st) {
                seen.push(st);
            }
        }
        seen.sort_unstable();
        let mut all = REF_STATES;
        all.sort_unstable();
        assert_eq!(
            seen, all,
            "the fixture must reach every state on the roster, or this proves nothing about the \
             ones it misses"
        );
    }

    /// **Half a phase landed is still in flight.** `git cherry` then reports a *mixture* — `-`
    /// for the commit whose patch is upstream, `+` for the one that is not — and the test has to
    /// be `all`, not `any`. With `any` this branch reads `rewritten`, which is the one direction
    /// that matters: it would hide work still in progress behind a count of finished work.
    ///
    /// Built by cherry-picking only the branch's first commit onto the baseline, which is what a
    /// partly-landed phase looks like. An earlier fixture squash-merged the whole branch and then
    /// reverted half, and that produces `++` rather than `-+` — so it left the `all`/`any`
    /// mutation alive.
    #[test]
    fn a_branch_with_only_some_commits_upstream_is_in_flight() {
        let tmp = phases_repo();
        let root = tmp.path();
        let first = rev(root, "phase/partial~1");
        // `cherry-pick` has no `-q`; passing one fails the command rather than quieting it.
        git(root, &["cherry-pick", "--no-gpg-sign", &first]);

        let marks = git_lines(root, &["cherry", "main", "phase/partial"])
            .unwrap()
            .iter()
            .filter_map(|l| l.chars().next())
            .collect::<String>();
        assert_eq!(
            marks, "-+",
            "the fixture must produce a mixture, or it cannot measure `all` against `any`"
        );
        assert_eq!(state(root, "phase/partial"), "active");
    }

    /// The `cherry.is_empty()` guard, exercised directly because `ref_state` cannot reach it: a
    /// ref with no commits of its own is an ancestor of the baseline, so `is_settled` claims it
    /// first. The guard is still load-bearing — `all()` over an empty iterator is **vacuously
    /// true**, so without it a ref carrying nothing would report as settled-by-rewrite.
    #[test]
    fn a_ref_with_no_commits_of_its_own_is_not_rewritten() {
        let tmp = phases_repo();
        let root = tmp.path();
        git(root, &["branch", "alias", "main"]);
        assert!(
            git_lines(root, &["cherry", "main", "alias"])
                .unwrap()
                .is_empty(),
            "the fixture must produce an empty cherry, or the guard is not under test"
        );
        assert!(!is_rewritten(root, "alias", Some("main")));
    }

    /// A branch carrying only empty commits has produced nothing, and is in flight rather than
    /// settled. It is the case the `files.is_empty()` guard is for: with no touched files the
    /// file comparison is vacuously true, and without the guard such a branch would read
    /// `rewritten` — finished work — on the strength of having changed nothing.
    #[test]
    fn a_branch_of_only_empty_commits_is_still_in_flight() {
        let tmp = phases_repo();
        let root = tmp.path();
        git(root, &["checkout", "-q", "-b", "phase/empty", "main"]);
        git(
            root,
            &[
                "commit",
                "-q",
                "--allow-empty",
                "--no-gpg-sign",
                "-m",
                "scope: nothing yet",
            ],
        );
        git(root, &["checkout", "-q", "main"]);
        assert_eq!(state(root, "phase/empty"), "active");
    }

    /// The tally splits all four, and its total is the ref count. A state that stopped being
    /// counted would leave the numbers summing to less than the refs and nothing would say so.
    #[test]
    fn the_tally_accounts_for_every_ref_it_reads() {
        let tmp = phases_repo();
        let root = tmp.path();
        git(
            root,
            &[
                "merge",
                "-q",
                "--no-ff",
                "--no-edit",
                "-m",
                "phase: ff",
                "phase/ff",
            ],
        );
        rebase_merge(root, "phase/rebase");
        squash_merge(root, "phase/squash", "phase: squash");
        git(root, &["branch", "ma/auditor", "main"]);
        let t = phase_tally(root);
        assert_eq!(t.settled, 1, "{t:?}");
        assert_eq!(t.rewritten, 2, "{t:?}");
        assert_eq!(t.positions, 1, "{t:?}");
        assert_eq!(t.active, 2, "{t:?}");
        assert_eq!(
            t.active + t.settled + t.rewritten + t.positions,
            phase_refs(root).len(),
            "the four states must partition the refs: {t:?}"
        );
    }

    // ── #792: a corpus's identity is its own, or it has none ──────────────────

    /// A git repository with one commit at `dir`, whose content is `mark`.
    ///
    /// **`mark` has to differ between fixtures.** A commit hash is a function of the tree, the
    /// message, the author and the second — so two repositories built identically in the same
    /// second get the *same* hash, and a test asserting that two corpora have different
    /// identities then fails against correct code. That happened here, which is the
    /// clearer form of the lesson: a fixture uniform in the dimension under test cannot
    /// measure it. (`git` is defined further down, beside the test that first needed it.)
    fn repo_at(dir: &std::path::Path, mark: &str) {
        std::fs::create_dir_all(dir).unwrap();
        git(dir, &["init", "-q", "-b", "main"]);
        git(dir, &["config", "user.email", "t@t.com"]);
        git(dir, &["config", "user.name", "T"]);
        git(dir, &["config", "commit.gpgsign", "false"]);
        std::fs::write(dir.join("a.txt"), format!("{mark}\n")).unwrap();
        git(dir, &["add", "-A"]);
        git(
            dir,
            &[
                "commit",
                "-q",
                "--no-gpg-sign",
                "-m",
                &format!("genesis: {mark}"),
            ],
        );
    }

    /// The case that must keep working, and the one that proves the comparison is
    /// canonicalising: a `TempDir` on macOS sits under `/var`, which is a symlink to
    /// `/private/var`, so git's resolved answer and the caller's path are different strings
    /// for the same directory. Without `canonicalize` on both sides this fails here and
    /// nowhere a person would think to look.
    #[test]
    fn a_repository_root_answers_with_its_own_genesis() {
        let tmp = tempfile::TempDir::new().unwrap();
        repo_at(tmp.path(), "host");
        let hash = genesis_hash(tmp.path()).expect("a repository root has a genesis commit");
        assert_eq!(hash.len(), 40, "expected a full SHA, got {hash:?}");
        assert_ne!(genesis_date(tmp.path()), UNKNOWN_COMMIT);
    }

    /// The defect. Git walks *up*, so a corpus that is a directory inside a repository used to
    /// be handed the enclosing repository's genesis — which names a different corpus, and is
    /// the same string for every corpus nested in that host.
    #[test]
    fn a_directory_inside_a_repository_has_no_genesis_of_its_own() {
        let tmp = tempfile::TempDir::new().unwrap();
        repo_at(tmp.path(), "host");
        let nested = tmp.path().join("examples/one");
        std::fs::create_dir_all(&nested).unwrap();
        assert!(
            genesis_hash(tmp.path()).is_some(),
            "the host still has one, or this test proves nothing"
        );
        assert_eq!(
            genesis_hash(&nested),
            None,
            "a nested directory must not claim its host's identity"
        );
    }

    /// The consequence, stated as the property rather than as the mechanism: two corpora
    /// nested in one host must not end up with **one** identity between them. Before #792 both
    /// answered with the host's hash, so `urn:yidam:<host>` named both — which is the
    /// conflation RFC-0032 §2 exists to prevent.
    #[test]
    fn two_corpora_in_one_host_never_share_an_identity() {
        let tmp = tempfile::TempDir::new().unwrap();
        repo_at(tmp.path(), "host");
        let (a, b) = (tmp.path().join("ex/a"), tmp.path().join("ex/b"));
        std::fs::create_dir_all(&a).unwrap();
        std::fs::create_dir_all(&b).unwrap();
        match (genesis_hash(&a), genesis_hash(&b)) {
            (Some(x), Some(y)) => assert_ne!(x, y, "two corpora minted one identity"),
            (None, None) => {}
            other => {
                panic!("one of two nested corpora was identified and the other was not: {other:?}")
            }
        }
    }

    /// The repair the RDF refusal advises, held to actually working: give the nested corpus a
    /// repository of its own and it has an identity, distinct from its former host's. An error
    /// message that recommends a fix is a claim about that fix.
    #[test]
    fn giving_a_nested_corpus_its_own_repository_gives_it_an_identity() {
        let tmp = tempfile::TempDir::new().unwrap();
        repo_at(tmp.path(), "host");
        let nested = tmp.path().join("ex/one");
        std::fs::create_dir_all(&nested).unwrap();
        assert_eq!(genesis_hash(&nested), None);

        repo_at(&nested, "the extracted corpus");
        let own = genesis_hash(&nested).expect("its own repository has a genesis commit");
        assert_ne!(
            Some(own),
            genesis_hash(tmp.path()),
            "the extracted corpus must not still answer with the host's hash"
        );
    }

    /// No repository at all is the case that already worked, and must keep working.
    #[test]
    fn a_directory_in_no_repository_has_no_genesis() {
        let tmp = tempfile::TempDir::new().unwrap();
        assert_eq!(genesis_hash(tmp.path()), None);
    }

    /// Everything derived from the hash follows it. A nested corpus reporting its host's
    /// genesis *date* as its own age is the same defect one field over, and the sentinel says
    /// `unknown` rather than `no commits` because the tree it sits in has plenty of commits.
    #[test]
    fn the_date_and_the_domain_follow_the_hash() {
        let tmp = tempfile::TempDir::new().unwrap();
        repo_at(tmp.path(), "host");
        let nested = tmp.path().join("ex/one");
        std::fs::create_dir_all(&nested).unwrap();
        assert_eq!(genesis_date(&nested), UNKNOWN_COMMIT);
        assert_eq!(genesis_message(&nested), "");
        // And the host, which does have one, is unaffected.
        assert!(genesis_message(tmp.path()).starts_with("genesis: host"));
    }

    fn names(out: &str) -> Vec<String> {
        parse_phase_refs(out).into_iter().map(|p| p.name).collect()
    }

    /// The shape a fresh clone has: one local branch, everything else remote-tracking.
    /// This is what CI sees, and reading `refs/heads` alone scored it 0.
    #[test]
    fn phases_are_found_in_a_fresh_clone() {
        let out = "\
main
origin/HEAD
origin/main
origin/ma/auditor
origin/ma/advocate
origin/rigpa/payload-budget
";
        assert_eq!(
            names(out),
            ["ma/advocate", "ma/auditor", "rigpa/payload-budget"]
        );
    }

    /// A branch on both sides is one phase, not two.
    #[test]
    fn a_phase_on_both_sides_is_counted_once_and_read_locally() {
        let out = "ma/auditor\norigin/ma/auditor\n";
        let refs = parse_phase_refs(out);
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].name, "ma/auditor");
        // Local wins: it is the ref whose tip is actually moving.
        assert_eq!(refs[0].git_ref, "ma/auditor");
    }

    /// Order does not depend on which side git listed first — the rendered table is
    /// committed, so an unstable order is a diff on every run.
    #[test]
    fn remote_first_and_local_first_agree() {
        assert_eq!(
            parse_phase_refs("origin/ma/auditor\nma/auditor\n"),
            parse_phase_refs("ma/auditor\norigin/ma/auditor\n")
        );
    }

    /// A phase that exists only on a remote is read through the remote-tracking ref.
    #[test]
    fn a_remote_only_phase_carries_the_ref_that_resolves() {
        let refs = parse_phase_refs("origin/rigpa/schema-reach\n");
        assert_eq!(refs[0].name, "rigpa/schema-reach");
        assert_eq!(refs[0].git_ref, "origin/rigpa/schema-reach");
    }

    /// Ordinary branches are not phases, on either side, and `origin/HEAD` is not a
    /// branch at all.
    #[test]
    fn non_phase_refs_are_ignored() {
        let out = "main\ndevelop\nfeat/ma-something\norigin/HEAD\norigin/main\nmalformed\n";
        assert!(names(out).is_empty(), "{:?}", names(out));
    }

    /// `malformed` starts with the letters of `ma` and is not a phase; the boundary is
    /// the slash.
    #[test]
    fn the_prefix_boundary_is_a_slash() {
        assert!(names("malformed\nrigpaX\n").is_empty());
        assert_eq!(names("ma/x\n"), ["ma/x"]);
    }

    /// The namespace PHASES.md defines a phase in. It was read by nothing: a derived
    /// repository held twenty-seven of these and `yidam status` counted none of them.
    #[test]
    fn the_phase_namespace_is_read() {
        let refs = parse_phase_refs("phase/outcome-axis\norigin/phase/the-local-half\n");
        assert_eq!(refs.len(), 2);
        assert_eq!(refs[0].name, "phase/outcome-axis");
        assert_eq!(refs[0].kind, RefKind::Phase);
        assert_eq!(refs[1].git_ref, "origin/phase/the-local-half");
    }

    /// Each namespace is a different thing, and the count that conflated them reported 26
    /// active phases for a repository holding one.
    #[test]
    fn each_namespace_carries_its_own_kind() {
        let refs = parse_phase_refs("ma/auditor\nphase/outcome-axis\nrigpa/term-of-art\n");
        let kinds: Vec<RefKind> = refs.iter().map(|r| r.kind).collect();
        assert_eq!(
            kinds,
            [RefKind::Position, RefKind::Phase, RefKind::Evolution]
        );
    }

    /// A standing position never settles. Divergence from the baseline is what it is *for*,
    /// so asking whether it merged is a category error rather than a hygiene check.
    #[test]
    fn only_bounded_work_settles() {
        assert!(!RefKind::Position.settles());
        assert!(RefKind::Evolution.settles());
        assert!(RefKind::Phase.settles());
    }

    /// A remote is not always called `origin`.
    #[test]
    fn any_remote_name_works() {
        assert_eq!(names("upstream/rigpa/bar\n"), ["rigpa/bar"]);
    }

    fn git(dir: &Path, args: &[&str]) {
        let status = std::process::Command::new("git")
            .current_dir(dir)
            .args(args)
            .status()
            .unwrap();
        assert!(status.success(), "git {args:?} failed");
    }

    #[test]
    fn genesis_message_returns_root_commit_not_newest() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path();
        git(root, &["init", "-q", "-b", "main"]);
        git(root, &["config", "user.email", "t@t.co"]);
        git(root, &["config", "user.name", "Test"]);
        std::fs::write(root.join("a"), "a").unwrap();
        git(root, &["add", "."]);
        git(root, &["commit", "-q", "-m", "chore: genesis — my-domain"]);
        std::fs::write(root.join("b"), "b").unwrap();
        git(root, &["add", "."]);
        git(root, &["commit", "-q", "-m", "establish: something newer"]);

        assert_eq!(genesis_message(root), "chore: genesis — my-domain");
        assert!(!genesis_date(root).is_empty());
    }
}
