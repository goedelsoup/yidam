use super::*;

/// Are the REGEN blocks current, and does a generator answer for each of them?
///
/// Borrows `regen --check`'s non-writing mode rather than reimplementing the generator
/// list. That is the whole reason [`crate::cmd::stale_blocks`] exists as a
/// function: a second list would be the third one that command was written to prevent.
/// [`crate::cmd::unclaimed_blocks`] is the other half and is borrowed for the same reason —
/// it reads the same `GENERATORS` list the gate does, so what `doctor` calls a generator and
/// what the gate calls one cannot drift.
///
/// It cannot borrow the *refusal*, though, and should not. `yidam regen` bails in a shallow clone
/// because a gate that cannot compute its verdict may not return one; `doctor` is the report the
/// person reaches for when something is refusing, so it answers instead — with the same cause and
/// the same remedy, at `warn`, because a depth-1 CI checkout is a normal thing to be in.
pub(super) fn check_regen(root: &std::path::Path) -> Answer {
    if crate::git::is_shallow(root) {
        return Answer::warn(
            "cannot be checked in a shallow clone: `genesis` reads the repository's first \
             commit, which a truncated history does not have",
            Some("git fetch --unshallow, or fetch-depth: 0 in CI"),
        );
    }
    // Both halves of the gate's question, or this report says a repository is fine while
    // `regen --check` fails it — which is the disagreement `stale_blocks` exists to avoid on
    // the other half.
    let unclaimed = match crate::cmd::unclaimed_blocks(root) {
        Ok(u) => u,
        Err(e) => return Answer::fail(format!("could not be computed: {e:#}"), None),
    };
    if !unclaimed.is_empty() {
        let names: Vec<String> = unclaimed
            .iter()
            .map(|u| format!("{} (yidam {})", u.file, u.generator))
            .collect();
        return Answer::fail(
            format!(
                "{} block(s) name a command no generator writes, so nothing writes them: {}",
                unclaimed.len(),
                names.join(", ")
            ),
            Some(
                "correct the command or delete the block — `yidam regen --check` lists the generators",
            ),
        );
    }
    match crate::cmd::stale_blocks(Some(root), crate::regen::Carrier::Checked) {
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

/// Does `AGENTS.md` carry the reading routes a re-vendor updates? — #1135.
///
/// A separate question from [`check_regen`], which judges the blocks a file carries and so
/// cannot see one that is missing. A derivation made before RFC-0039 has no `yidam routes`
/// markers, `update_file_regen` writes nothing where it finds none, and `regen --check`
/// reports every block current. Of sixteen route lines the template added before the block
/// existed, two reached a derivation, and nothing reported the other fourteen (#969).
///
/// A `warn` and not a `fail`: the whole-file list still works, and it is what every
/// derivation had until this landed. No `AGENTS.md` skips rather than passes, for the reason
/// [`check_kuten_read`] gives: there is nothing to carry the block.
pub(super) fn check_routes(root: &Path) -> Answer {
    let Ok(text) = std::fs::read_to_string(root.join("AGENTS.md")) else {
        return Answer::skipped("no `AGENTS.md`, so there is no reading list to route");
    };
    const HELD: &str = "`AGENTS.md` reads by occasion, from the vendored routes";
    if super::super::migrate_routes::has_block(&text) {
        return Answer::offers(HELD, vec![Offered::adopted("routes")]);
    }
    let remedy = if root.join(super::super::routes::VENDORED).exists() {
        "yidam migrate routes, committed as a `migrate:` commit"
    } else {
        "mise run yidam-vendor-update, then yidam migrate routes"
    };
    Answer::offers(
        HELD,
        vec![Offered::open(
            "routes",
            "`AGENTS.md` has no `yidam routes` block, so no re-vendor reaches its reading list",
            remedy,
        )],
    )
}

/// Do `ci.yml` and `CLAUDE.md` mark the part a re-vendor updates? — #1054.
///
/// The question [`check_routes`] asks of `AGENTS.md`, asked of the other two files genesis
/// installs once. Without markers `yidam-vendor-update` reaches neither, so a gate added
/// upstream never reaches this repository's CI, and nothing else would say so.
///
/// A `warn`, for the reason `routes` gives. A `CLAUDE.md` with none of the template's sections
/// is the owner's whole, and asks for no region: `migrate scaffold` leaves it unmarked, and a
/// finding it cannot resolve would be one to live with forever.
pub(super) fn check_scaffold(root: &Path) -> Answer {
    use super::super::migrate_scaffold as m;
    let ci = std::fs::read_to_string(root.join(m::CI)).ok();
    let claude = std::fs::read_to_string(root.join(m::CLAUDE)).ok();
    if ci.is_none() && claude.is_none() {
        return Answer::skipped(format!(
            "no `{}` and no `{}`, so there is nothing a re-vendor could update",
            m::CI,
            m::CLAUDE
        ));
    }
    let mut unmarked = vec![];
    if ci.as_deref().is_some_and(|t| !m::ci_has_region(t)) {
        unmarked.push(m::CI);
    }
    if claude
        .as_deref()
        .is_some_and(|t| m::claude_is_template(t) && !m::claude_has_region(t))
    {
        unmarked.push(m::CLAUDE);
    }
    // Both offers are collected before either is judged, so a decline of the regions
    // cannot hide missing hooks (RFC-0066 §1).
    let regions = if unmarked.is_empty() {
        Offered::adopted("scaffold-regions")
    } else {
        Offered::open(
            "scaffold-regions",
            format!(
                "no YIDAM region in {}, so no re-vendor reaches the part that is yidam's",
                unmarked
                    .iter()
                    .map(|f| format!("`{f}`"))
                    .collect::<Vec<_>>()
                    .join(" or ")
            ),
            "yidam migrate scaffold, then mise run yidam-vendor-update",
        )
    };
    let mut offers = vec![regions];
    // The hooks, asked of a repository an agent works in: one with a `CLAUDE.md`. Without
    // them no session is handed where the loop stands, and nothing else would say so (#1444).
    if claude.is_some() {
        let installed = std::fs::read_to_string(root.join(m::SETTINGS))
            .is_ok_and(|t| m::settings_has_hooks(&t));
        offers.push(if installed {
            Offered::adopted("scaffold-hooks")
        } else {
            Offered::open(
                "scaffold-hooks",
                format!(
                    "`{}` does not run `yidam hook`, so no session is handed `cycle --brief`",
                    m::SETTINGS
                ),
                "yidam migrate scaffold",
            )
        });
    }
    Answer::offers(
        "each file genesis installed marks the part a re-vendor updates, and the loop's \
         hooks are installed",
        offers,
    )
}

/// The commands that check a corpus. `yidam lint` counts unless it is one of the flags that
/// only prints or writes.
const CI_GATES: &[&str] = &["yidam graph-check", "yidam regen --check", "yidam lint"];

/// `yidam lint` flags that check nothing.
const NOT_A_GATE: &[&str] = &["--bless", "--explain", "--init-baseline"];

/// The inherited `mise.yidam.toml` tasks that run one of [`CI_GATES`]. A test reads each one's
/// `run` and holds it to that.
const GATE_TASKS: &[&str] = &[
    "graph-check",
    "graph-lint",
    "graph-lint-gate",
    "graph-lint-commits",
    "regen-check",
];

/// How many of the repository's own tasks deep a `mise run` is followed.
const TASK_DEPTH: usize = 3;

/// The corpus gates some shell text runs.
///
/// Comments are cut first, whole-line and trailing: the scaffold's workflow explains its gates
/// in comments, and a workflow whose every gate was deleted but whose prose still names them
/// must not pass. A `mise run` of an inherited gate task counts, addressed bare or as `//:`,
/// as does one of the repository's own tasks whose `run` reaches a gate: two of fifteen
/// derived workflows gate through `mise run ci` or `mise run //:graph-lint` and name no
/// `yidam` command at all.
fn gates_in(text: &str, own_task: &dyn Fn(&str) -> Option<String>, depth: usize) -> Vec<String> {
    gate_hits(text, own_task, depth)
        .into_iter()
        .map(|h| h.label)
        .collect()
}

/// One corpus gate some shell text runs, and whether it runs over the staged tree.
#[derive(Debug, PartialEq)]
struct GateHit {
    label: String,
    staged: bool,
}

/// [`gates_in`], keeping whether each gate carries `--staged` (RFC-0070 §5).
///
/// A gate counts as staged when its own command carries the flag. An inherited gate task
/// never does: its `run` is the working-tree command. One of the repository's own tasks
/// whose `run` reaches gates both ways is two hits, one of each.
fn gate_hits(text: &str, own_task: &dyn Fn(&str) -> Option<String>, depth: usize) -> Vec<GateHit> {
    let mut found: Vec<GateHit> = vec![];
    let mut add = |label: String, staged: bool| {
        if !found.iter().any(|h| h.label == label) {
            found.push(GateHit { label, staged });
        }
    };
    for line in text.lines() {
        let code = match line.find('#') {
            Some(i) if line[..i].trim().is_empty() => continue,
            Some(i) if line[..i].ends_with(char::is_whitespace) => &line[..i],
            _ => line,
        };
        for gate in CI_GATES {
            for (at, _) in code.match_indices(gate) {
                let rest = &code[at + gate.len()..];
                // A whole command, not a prefix of another: `yidam lint-x`.
                if rest.starts_with(|c: char| c.is_alphanumeric() || c == '-' || c == '_') {
                    continue;
                }
                if *gate == "yidam lint"
                    && NOT_A_GATE.iter().any(|f| rest.trim_start().starts_with(f))
                {
                    continue;
                }
                let command: String = rest
                    .chars()
                    .take_while(|c| !matches!(c, ';' | '&' | '|'))
                    .collect();
                let staged = command.split_whitespace().any(|w| w == "--staged");
                let label = if staged {
                    format!("{gate} --staged")
                } else {
                    (*gate).to_string()
                };
                add(label, staged);
            }
        }
        for (at, _) in code.match_indices("mise run ") {
            let token: String = code[at + "mise run ".len()..]
                .chars()
                .take_while(|c| {
                    !c.is_whitespace() && !matches!(c, '"' | '\'' | '`' | ';' | '&' | '|' | ')')
                })
                .collect();
            // `//:task` is the root's task; `//web:task` is a subproject's, and not inherited.
            let task = match token.strip_prefix("//:") {
                Some(t) => t,
                None if token.starts_with("//") => continue,
                None => token.as_str(),
            };
            if GATE_TASKS.contains(&task) {
                add(format!("mise run {token}"), false);
            } else if depth > 0 {
                if let Some(run) = own_task(task) {
                    let inner = gate_hits(&run, own_task, depth - 1);
                    for staged in [true, false] {
                        let these: Vec<&str> = inner
                            .iter()
                            .filter(|h| h.staged == staged)
                            .map(|h| h.label.as_str())
                            .collect();
                        if !these.is_empty() {
                            add(format!("mise run {token} ({})", these.join(", ")), staged);
                        }
                    }
                }
            }
        }
    }
    found
}

/// A task's `run`, from the repository's own `mise.toml`, as one piece of shell text.
fn own_task_run(mise: &toml::Value, task: &str) -> Option<String> {
    let run = mise.get("tasks")?.get(task)?.get("run")?;
    match run {
        toml::Value::String(s) => Some(s.clone()),
        toml::Value::Array(a) => Some(
            a.iter()
                .filter_map(toml::Value::as_str)
                .collect::<Vec<_>>()
                .join("\n"),
        ),
        _ => None,
    }
}

/// Does this repository's CI run a corpus gate? — #1054.
///
/// A workflow that runs none leaves a green check saying nothing about the graph. #1054
/// reported one derivation in that state; it gated through `mise run //:graph-lint` and named
/// `yidam` only in a comment, which is why [`gates_in`] cuts comments and follows tasks.
/// Measured 2026-09-29, all fourteen derived workflows gate. Read offline from
/// `.github/workflows/`: this cannot see CI that lives anywhere else, so a `warn` and never a
/// `fail`, and the detail says what it looked at.
///
/// A local hook that runs a gate counts too (RFC-0070 §5). #1519's corpus gated every commit
/// from a pre-commit hook and was warned on every run. The detail names the hook and what it
/// does not reach, so a hook is not mistaken for CI.
pub(super) fn check_ci(root: &Path) -> Answer {
    let mise = std::fs::read_to_string(root.join("mise.toml"))
        .ok()
        .and_then(|t| toml::from_str::<toml::Value>(&t).ok());
    let own_task = |task: &str| mise.as_ref().and_then(|m| own_task_run(m, task));
    let hooks = hook_gates(root, &own_task);
    let hook_text = hooks.held();

    let dir = root.join(".github/workflows");
    let Ok(entries) = std::fs::read_dir(&dir) else {
        if let Some(text) = hook_text {
            return Answer::offers(text, vec![Offered::adopted("ci")]);
        }
        // Both warn arms are the one `ci` offer: a repository with no workflows here usually
        // runs CI where this cannot read, and a decline naming where answers both.
        return Answer::offers(
            "",
            vec![Offered::open(
                "ci",
                format!(
                    "no `.github/workflows/`, so no CI this can read checks the corpus{}",
                    hooks.note()
                ),
                "a workflow step running `yidam graph-check` — or, with CI elsewhere, run it there",
            )],
        );
    };
    let mut files: Vec<_> = entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "yml" || x == "yaml"))
        .collect();
    files.sort();
    let mut gates: Vec<String> = vec![];
    for f in &files {
        let Ok(text) = std::fs::read_to_string(f) else {
            continue;
        };
        for g in gates_in(&text, &own_task, TASK_DEPTH) {
            if !gates.contains(&g) {
                gates.push(g);
            }
        }
    }
    if gates.is_empty() {
        if let Some(text) = hook_text {
            return Answer::offers(text, vec![Offered::adopted("ci")]);
        }
        return Answer::offers(
            "",
            vec![Offered::open(
                "ci",
                format!(
                    "none of {} workflow(s) runs a corpus gate, so a green check says nothing \
                     about the graph{}",
                    files.len(),
                    hooks.note()
                ),
                "yidam migrate scaffold, then mise run yidam-vendor-update",
            )],
        );
    }
    let mut held = format!("runs {}", gates.join(", "));
    if let Some(text) = hook_text {
        held.push_str("; ");
        held.push_str(&text);
    }
    Answer::offers(held, vec![Offered::adopted("ci")])
}

/// The hooks a commit or a push runs, of which only these can gate one.
const GATE_HOOKS: &[&str] = &["pre-commit", "pre-push"];

/// What the hooks git would run say about the corpus gates.
#[derive(Debug, Default)]
struct HookGates {
    /// Each hook that runs a gate, as it is named to the reader, with the gates it runs.
    gating: Vec<(String, Vec<String>)>,
    /// What every gating hook binds: this clone, or the checkouts that opt in.
    binds: &'static str,
    /// Gates a hook names that do not count, or count with a caveat, and why.
    notes: Vec<String>,
}

impl HookGates {
    /// The detail for a check held by these hooks, or `None` when none gates.
    fn held(&self) -> Option<String> {
        if self.gating.is_empty() {
            return None;
        }
        let each: Vec<String> = self
            .gating
            .iter()
            .map(|(hook, gates)| format!("runs {} in the hook at `{hook}`", gates.join(", ")))
            .collect();
        Some(format!(
            "{}, which binds only {}{}",
            each.join(" and "),
            self.binds,
            self.note()
        ))
    }

    /// Appended to the detail, so a person whose hook git skips, or whose gate does not count,
    /// is told why.
    fn note(&self) -> String {
        if self.notes.is_empty() {
            String::new()
        } else {
            format!("; {}", self.notes.join("; "))
        }
    }
}

/// Read the `pre-commit` and `pre-push` hooks git would run in `root` for corpus gates.
///
/// The directory is `git rev-parse --git-path hooks`, which follows `core.hooksPath`. Only
/// the two hook names are read, so a `.sample` file never is. A hook that is not executable
/// on a platform that checks the bit is not run by git and so gates nothing; it is named in
/// [`HookGates::skipped`] when it would otherwise have counted.
fn hook_gates(root: &Path, own_task: &dyn Fn(&str) -> Option<String>) -> HookGates {
    let Some(raw) = crate::git::Git::new(root)
        .args(["rev-parse", "--git-path", "hooks"])
        .try_run()
    else {
        return HookGates::default();
    };
    let dir = root.join(raw.trim());
    // The common directory, not this worktree's: a linked worktree runs the hooks under the
    // main repository's `.git/hooks`.
    let git_dir = crate::git::Git::new(root)
        .args(["rev-parse", "--git-common-dir"])
        .try_run()
        .map(|d| root.join(d.trim()));
    let canon = |p: &Path| p.canonicalize().unwrap_or_else(|_| p.to_path_buf());
    let local = git_dir.is_some_and(|g| canon(&dir).starts_with(canon(&g)));
    let mut out = HookGates {
        binds: if local {
            "this clone and its linked worktrees"
        } else {
            "the checkouts that set `core.hooksPath` to it"
        },
        ..HookGates::default()
    };
    for name in GATE_HOOKS {
        let path = dir.join(name);
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let hits = gate_hits(&text, own_task, TASK_DEPTH);
        if hits.is_empty() {
            continue;
        }
        let shown = canon(&path)
            .strip_prefix(canon(root))
            .map_or_else(|_| path.display().to_string(), |p| p.display().to_string());
        let labels = |staged: Option<bool>| {
            hits.iter()
                .filter(|h| staged.is_none_or(|s| h.staged == s))
                .map(|h| h.label.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        };
        if !runnable(&path) {
            out.notes.push(format!(
                "`{shown}` runs {} but is not executable, so git does not run it",
                labels(None)
            ));
            continue;
        }
        // A pre-commit gate without `--staged` checks the working tree, not the commit: the
        // defect RFC-0070 fixes, so it is not counted. A push has no staged tree, so a
        // pre-push gate counts as written, with what it reads named.
        let counted: Vec<String> = if *name == "pre-commit" {
            let unstaged = labels(Some(false));
            if !unstaged.is_empty() {
                out.notes.push(format!(
                    "`{shown}` runs {unstaged} without `--staged`, so it checks the working \
                     tree, not the commit"
                ));
            }
            hits.iter()
                .filter(|h| h.staged)
                .map(|h| h.label.clone())
                .collect()
        } else {
            out.notes.push(format!(
                "`{shown}` reads the working tree, which can hold edits the pushed commits do \
                 not"
            ));
            hits.iter().map(|h| h.label.clone()).collect()
        };
        if !counted.is_empty() {
            out.gating.push((shown, counted));
        }
    }
    out
}

/// Would git run this hook? On unix, only if it is executable.
fn runnable(path: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::metadata(path).is_ok_and(|m| m.permissions().mode() & 0o111 != 0)
    }
    #[cfg(not(unix))]
    {
        path.is_file()
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::*;
    use super::*;
    use crate::git::fixture::git;
    use tempfile::TempDir;

    fn no_tasks(_: &str) -> Option<String> {
        None
    }

    #[test]
    fn a_gate_named_only_in_a_comment_is_not_run() {
        let text = "\
# yidam graph-check, from column 0
jobs:
  corpus:
    # runs `yidam graph-check` and `mise run graph-lint`
    steps:
      - run: echo ok # then yidam regen --check
      - run: yidam lint --bless
      - run: yidam lint-report
      - run: mise run //web:graph-check
";
        assert_eq!(gates_in(text, &no_tasks, TASK_DEPTH), Vec::<String>::new());
    }

    #[test]
    fn a_gate_is_found_run_directly_or_through_an_inherited_task() {
        let text = "\
      - run: yidam lint --commits --range origin/main..HEAD
      - run: yidam regen --check
      - run: mise run //:graph-lint
      - run: \"mise run regen-check\"
";
        assert_eq!(
            gates_in(text, &no_tasks, TASK_DEPTH),
            [
                "yidam lint",
                "yidam regen --check",
                "mise run //:graph-lint",
                "mise run regen-check"
            ]
        );
    }

    /// ohio-budget's workflow runs `mise run ci`, and its own `[tasks.ci]` runs the gate.
    #[test]
    fn a_repository_task_is_followed_to_the_gate_it_runs() {
        let mise: toml::Value = toml::from_str(
            "[tasks.ci]\nrun = [\"cargo test\", \"mise run check\"]\n\
             [tasks.check]\nrun = \"yidam graph-check\"\n\
             [tasks.loop]\nrun = \"mise run loop\"\n\
             [tasks.web]\nrun = \"pnpm test\"\n",
        )
        .unwrap();
        let own = |t: &str| own_task_run(&mise, t);
        assert_eq!(
            gates_in("- run: mise run ci", &own, TASK_DEPTH),
            ["mise run ci (mise run check (yidam graph-check))"]
        );
        // A task that runs itself ends at the depth limit, and one that gates nothing is not a gate.
        assert!(gates_in("- run: mise run loop && mise run web", &own, TASK_DEPTH).is_empty());
        // The depth is a limit: a gate one task down is not seen with none to spend.
        assert!(gates_in("- run: mise run check", &own, 0).is_empty());
    }

    /// Every inherited task [`GATE_TASKS`] counts runs a gate, read from the task layer itself.
    /// A task renamed or re-pointed there would otherwise pass every workflow that calls it.
    #[test]
    fn every_gate_task_runs_a_gate_in_the_task_layer() {
        let text = std::fs::read_to_string(
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../mise.yidam.toml"),
        )
        .expect("mise.yidam.toml");
        let layer: toml::Value = toml::from_str(&text).expect("mise.yidam.toml parses");
        for task in GATE_TASKS {
            let run = layer
                .get(task)
                .and_then(|t| t.get("run"))
                .and_then(toml::Value::as_str)
                .unwrap_or_else(|| panic!("mise.yidam.toml has no `[{task}]` with a `run`"));
            assert!(
                !gates_in(run, &no_tasks, 0).is_empty(),
                "`[{task}]` runs `{run}`, which is no corpus gate"
            );
        }
    }

    /// The scaffold's own workflow passes, and every gate it runs is one this reads: a gate
    /// added to its region under a new command would otherwise reach every derivation and be
    /// invisible here.
    #[test]
    fn the_scaffold_s_gates_are_the_ones_this_reads() {
        let ci = std::fs::read_to_string(
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../sadhana/github/workflows/ci.yml"),
        )
        .expect("the scaffold workflow");
        let region = ci
            .split("<!-- YIDAM:CI -->")
            .nth(1)
            .and_then(|r| r.split("<!-- /YIDAM:CI -->").next())
            .expect("the scaffold's YIDAM:CI region");
        assert_eq!(
            gates_in(region, &no_tasks, TASK_DEPTH),
            ["yidam graph-check", "yidam lint", "yidam regen --check"]
        );
    }

    /// A derived repository under git whose `core.hooksPath` hook holds `body`.
    fn hooked(body: &str, executable: bool) -> TempDir {
        let tmp = derived_repo();
        crate::git::fixture::init(tmp.path());
        git(tmp.path(), &["config", "core.hooksPath", ".githooks"]);
        let hook = tmp.path().join(".githooks/pre-commit");
        std::fs::create_dir_all(hook.parent().unwrap()).unwrap();
        std::fs::write(&hook, body).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = if executable { 0o755 } else { 0o644 };
            std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(mode)).unwrap();
        }
        #[cfg(not(unix))]
        let _ = executable;
        tmp
    }

    const STAGED_HOOK: &str = "#!/bin/sh\nset -e\nyidam lint --staged\n";

    /// #1519's corpus: no workflow, and every commit gated by a hook. A pass, naming the hook
    /// and what it does not reach.
    #[test]
    fn a_hook_that_runs_a_gate_passes_ci_and_is_named() {
        let tmp = hooked(STAGED_HOOK, true);
        let a = check_ci(tmp.path());
        assert_eq!(a.verdict, Verdict::Ok, "{}", a.detail);
        assert!(
            a.detail
                .contains("runs yidam lint --staged in the hook at `.githooks/pre-commit`"),
            "{}",
            a.detail
        );
        assert!(
            a.detail
                .contains("binds only the checkouts that set `core.hooksPath`"),
            "{}",
            a.detail
        );
    }

    /// A hook under `.git/hooks` binds this clone and its linked worktrees.
    #[test]
    fn a_pre_push_hook_in_the_git_dir_counts_as_written_and_binds_this_clone() {
        let tmp = derived_repo();
        crate::git::fixture::init(tmp.path());
        let hook = tmp.path().join(".git/hooks/pre-push");
        // `git init` writes `.git/hooks` only from a template, and a template can be empty.
        std::fs::create_dir_all(hook.parent().unwrap()).unwrap();
        std::fs::write(&hook, "yidam graph-check\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        let a = check_ci(tmp.path());
        assert_eq!(a.verdict, Verdict::Ok, "{}", a.detail);
        assert!(
            a.detail
                .contains("runs yidam graph-check in the hook at `.git/hooks/pre-push`"),
            "{}",
            a.detail
        );
        assert!(
            a.detail
                .contains("binds only this clone and its linked worktrees"),
            "{}",
            a.detail
        );
        assert!(
            a.detail.contains(
                "`.git/hooks/pre-push` reads the working tree, which can hold edits the pushed \
                 commits do not"
            ),
            "{}",
            a.detail
        );
    }

    /// A pre-commit hook running a gate as written checks the working tree, the defect
    /// RFC-0070 fixes, so it does not count, and the detail names the missing flag.
    #[test]
    fn a_pre_commit_gate_without_staged_does_not_pass_ci() {
        let tmp = hooked("#!/bin/sh\nyidam lint\n", true);
        let a = check_ci(tmp.path());
        assert_eq!(a.verdict, Verdict::Warn, "{}", a.detail);
        assert!(
            a.detail.contains(
                "`.githooks/pre-commit` runs yidam lint without `--staged`, so it checks the \
                 working tree, not the commit"
            ),
            "{}",
            a.detail
        );
    }

    /// The same through `mise run`: the task's own command must carry the flag, and an
    /// inherited gate task never does.
    #[test]
    fn a_pre_commit_gate_through_a_task_needs_staged_in_the_task() {
        let tmp = hooked("#!/bin/sh\nmise run check\nmise run graph-lint\n", true);
        std::fs::write(
            tmp.path().join("mise.toml"),
            "[tasks.check]\nrun = \"yidam lint\"\n",
        )
        .unwrap();
        let a = check_ci(tmp.path());
        assert_eq!(a.verdict, Verdict::Warn, "{}", a.detail);
        assert!(
            a.detail.contains(
                "runs mise run check (yidam lint), mise run graph-lint without `--staged`"
            ),
            "{}",
            a.detail
        );
        std::fs::write(
            tmp.path().join("mise.toml"),
            "[tasks.check]\nrun = \"yidam lint --staged\"\n",
        )
        .unwrap();
        let a = check_ci(tmp.path());
        assert_eq!(a.verdict, Verdict::Ok, "{}", a.detail);
        assert!(
            a.detail
                .contains("runs mise run check (yidam lint --staged) in the hook at"),
            "{}",
            a.detail
        );
    }

    /// `git init` writes `pre-commit.sample`, which git never runs.
    #[test]
    fn a_sample_hook_does_not_pass_ci() {
        let tmp = derived_repo();
        crate::git::fixture::init(tmp.path());
        let hooks = tmp.path().join(".git/hooks");
        std::fs::create_dir_all(&hooks).unwrap();
        std::fs::write(hooks.join("pre-commit.sample"), STAGED_HOOK).unwrap();
        assert_eq!(check_ci(tmp.path()).verdict, Verdict::Warn);
    }

    /// A hook git would not run gates nothing, and the warning says why it did not count.
    #[cfg(unix)]
    #[test]
    fn a_hook_that_is_not_executable_does_not_pass_ci() {
        let tmp = hooked(STAGED_HOOK, false);
        let a = check_ci(tmp.path());
        assert_eq!(a.verdict, Verdict::Warn, "{}", a.detail);
        assert!(
            a.detail.contains(
                "`.githooks/pre-commit` runs yidam lint --staged but is not executable, so git does not \
                 run it"
            ),
            "{}",
            a.detail
        );
    }

    /// A workflow gate still holds the check, and a hook gate is named beside it.
    #[test]
    fn a_workflow_gate_passes_with_the_hook_beside_it() {
        let tmp = hooked(STAGED_HOOK, true);
        let workflows = tmp.path().join(".github/workflows");
        std::fs::create_dir_all(&workflows).unwrap();
        std::fs::write(workflows.join("ci.yml"), "- run: yidam graph-check\n").unwrap();
        let a = check_ci(tmp.path());
        assert_eq!(a.verdict, Verdict::Ok, "{}", a.detail);
        assert!(
            a.detail
                .starts_with("runs yidam graph-check; runs yidam lint --staged in the hook at"),
            "{}",
            a.detail
        );
    }
}
