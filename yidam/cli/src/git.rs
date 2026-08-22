use std::path::Path;

/// Hash of the root (genesis) commit.
///
/// `git log --reverse --max-count=1` does NOT work for this: git applies the
/// count limit before reversing, returning the newest commit instead.
fn genesis_hash(root: &Path) -> Option<String> {
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
        .unwrap_or_else(|| "no commits".to_string())
}

pub fn head_commit_short(root: &Path) -> String {
    let out = std::process::Command::new("git")
        .current_dir(root)
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok();
    out.and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".to_string())
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
        matches!(self, RefKind::Evolution | RefKind::Phase)
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

/// What the tracked refs actually hold, split by the three things they can be.
#[derive(Debug, Default, PartialEq, Eq, serde::Serialize)]
pub struct PhaseTally {
    /// Bounded work not yet on the baseline — the only number that means "in flight".
    pub active: usize,
    /// Bounded work already merged, whose ref outlived its settlement. PHASES.md
    /// prescribes deleting these; leaving them is the drift this number makes visible.
    pub settled: usize,
    /// Standing elector positions. Neither active work nor drift — a third thing.
    pub positions: usize,
}

/// What a ref currently is, in one word: `active`, `settled`, or `position`.
///
/// The single classifier. `yidam status` counts these and `yidam phases` prints them, and
/// they must not be able to disagree — a derived repository once held three separate
/// implementations of "does this node cite that source" and only two of them agreed.
pub(crate) fn ref_state(root: &Path, r: &PhaseRef, base: Option<&str>) -> &'static str {
    if !r.kind.settles() {
        "position"
    } else if is_settled(root, &r.git_ref, base) {
        "settled"
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
            _ => tally.active += 1,
        }
    }
    tally
}

#[cfg(test)]
mod tests {
    use super::*;

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
