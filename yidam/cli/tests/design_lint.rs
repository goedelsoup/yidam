//! The adherence lint is invoked, is loadable, and has something to say.
//!
//! `_adherence.oxlintrc.json` existed for exactly the drift #465 found, and three separate
//! things were wrong with it at once:
//!
//! 1. **Nothing invoked it.** Not `mise.toml`, not a workflow, not a `package.json`.
//! 2. **It could not have loaded.** It carried an `x-omelette` key holding a hand-maintained
//!    roster of 226 token names — a fourth copy of the palette — and oxlint rejects an
//!    unknown top-level key outright. The roster had already drifted: nine `-dark` tokens
//!    existed in the CSS and not in the list, so a component using one would have been told
//!    it was undeclared.
//! 3. **Every rule was `warn`.** Under a lint nobody runs the level does not matter; under
//!    one that runs in CI without `--deny-warnings`, `warn` means off.
//!
//! The third is why this file exists rather than trusting the job to fail. Emptying
//! `no-restricted-syntax` leaves `mise run design-lint` green — a lint with nothing to say
//! exits zero, and reads from the outside exactly like a lint with nothing to complain about.
//!
//! # And every one of those rules was inert anyway (#467)
//!
//! The three findings above were all true and none of them was the problem. #467 pointed a
//! deliberately-broken file at the lint and it reported nothing, because:
//!
//! 4. **`no-restricted-syntax` is not a rule oxlint has.** It is absent from `oxlint --rules`,
//!    and an unknown rule key is accepted at load and ignored at run. All 47 selectors —
//!    every prop contract, the hex rule, the px rule, the font rule — did nothing at all.
//! 5. **`no-restricted-imports` is implemented, and was switched off everywhere.** An
//!    `overrides` block exempting `**/index.js` disabled the rule for every file, not for
//!    that one; and its patterns were written for bare specifiers (`components/core/**`)
//!    while every import in the tree is relative (`../core/Badge.jsx`), so it could not have
//!    matched even had it run.
//!
//! The lesson is the one this file was already about, applied to itself: **reading a config
//! cannot tell you whether a linter enforces it.** Three tests here asserted the rules were
//! present, were errors, and were invoked, and all three were true of a lint that caught
//! nothing. `scripts/design-lint-selftest.sh` runs it against a file that breaks it, and the
//! prop contracts now live in `design_system.rs`, derived from each component's `.d.ts`
//! rather than transcribed into a regex.

use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn read(rel: &str) -> String {
    let p = repo_root().join(rel);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{} unreadable: {e}", p.display()))
}

const CONFIG: &str = "yidam/design/_adherence.oxlintrc.json";

fn config() -> serde_json::Value {
    serde_json::from_str(&read(CONFIG)).expect("_adherence.oxlintrc.json parses as JSON")
}

/// Keys oxlint accepts at the top level, from its own rejection message.
///
/// Written down because the failure is total: one unknown key and the whole config is
/// refused, so a rule added beside a stray key is a rule that never ran. That is how this
/// config spent its life — loadable by nothing, holding a roster nothing read.
const ALLOWED_KEYS: &[&str] = &[
    "$schema",
    "plugins",
    "jsPlugins",
    "categories",
    "rules",
    "settings",
    "env",
    "globals",
    "overrides",
    "ignorePatterns",
    "extends",
];

#[test]
fn the_config_holds_only_keys_oxlint_accepts() {
    let cfg = config();
    let unknown: Vec<&String> = cfg
        .as_object()
        .expect("the config is an object")
        .keys()
        .filter(|k| !ALLOWED_KEYS.contains(&k.as_str()))
        .collect();
    assert!(
        unknown.is_empty(),
        "oxlint refuses a config with an unknown top-level key, and refuses the whole file: \
         {unknown:?}. Whatever that key was for, it belongs somewhere a linter is not asked \
         to parse."
    );
}

/// The rule that survives is configured to catch what it was written to catch.
///
/// One rule, not forty-seven. The rest named `no-restricted-syntax`, which oxlint does not
/// implement, and #467 removed them rather than leave a config that reads as enforcement.
///
/// What is asserted here is the shape of the patterns, because that is where the second
/// defect was: `components/core/**` matches a bare specifier and every import in this tree is
/// relative. Whether the rule then *fires* is not knowable from here — oxlint is a task-scoped
/// tool and this gate has no npm — so `scripts/design-lint-selftest.sh` answers that by
/// running it, and `the_lint_is_proved_against_a_file_that_breaks_it` below requires the task
/// to invoke it.
#[test]
fn the_import_boundary_is_configured_to_match_the_imports_this_tree_has() {
    let cfg = config();
    let patterns = cfg["rules"]["no-restricted-imports"][1]["patterns"][0]["group"]
        .as_array()
        .unwrap_or_else(|| panic!("{CONFIG} declares no `no-restricted-imports` patterns"));
    assert!(
        !patterns.is_empty(),
        "the import boundary forbids nothing, which is the same nothing as not being there"
    );

    let groups: Vec<&str> = patterns.iter().filter_map(|p| p.as_str()).collect();
    let unmatched: Vec<&&str> = groups.iter().filter(|g| !g.starts_with("**/")).collect();
    assert!(
        unmatched.is_empty(),
        "these patterns are anchored where an import specifier is not: {unmatched:?}. A \
         sibling import reads `../core/Badge.jsx` and carries no `components/` segment, so a \
         pattern that names one matches only the barrel — which is how this rule spent its \
         life matching nothing but the file it was meant to exempt."
    );

    // Every component group is behind the boundary. Discovered, so a group added tomorrow is
    // covered without anyone remembering this file.
    let dir = repo_root().join("yidam/design/components");
    let missing: Vec<String> = std::fs::read_dir(&dir)
        .expect("yidam/design/components is readable")
        .filter_map(Result::ok)
        .filter(|e| e.path().is_dir())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .filter(|group| !groups.iter().any(|g| g.contains(group.as_str())))
        .collect();
    assert!(
        missing.is_empty(),
        "these component groups are not behind the import boundary, so their internals can \
         be imported directly: {missing:?}"
    );
}

/// Every rule is an error.
///
/// A lint that only warns in CI is a lint that is off — and the task passes
/// `--deny-warnings` precisely so that stays true for oxlint's own defaults too, one of
/// which was already firing on a stray escape when the lint was first run.
#[test]
fn no_rule_is_merely_a_warning() {
    let cfg = config();
    let mut warned = Vec::new();
    fn walk(node: &serde_json::Value, path: &str, out: &mut Vec<String>) {
        match node {
            serde_json::Value::String(s) if s == "warn" => out.push(path.to_string()),
            serde_json::Value::Array(a) => {
                for (i, v) in a.iter().enumerate() {
                    walk(v, &format!("{path}[{i}]"), out);
                }
            }
            serde_json::Value::Object(o) => {
                for (k, v) in o {
                    walk(v, &format!("{path}.{k}"), out);
                }
            }
            _ => {}
        }
    }
    walk(&cfg["rules"], "rules", &mut warned);
    walk(&cfg["overrides"], "overrides", &mut warned);
    assert!(
        warned.is_empty(),
        "these are set to `warn`, which in a gate is off: {warned:?}"
    );
}

/// Something runs it, and runs it strictly.
///
/// The pattern #194's audit named and #461 found in `verify`: a surface with no consumer.
/// Both halves are checked — a task that nothing invokes is the same silence as a config
/// that no task reads.
#[test]
fn a_task_runs_the_lint_and_a_workflow_runs_the_task() {
    // Comments stripped before anything is asserted about the task. Without that, the
    // note above the task explaining *why* it passes `--deny-warnings` satisfies the check
    // that it passes it — which is how the first version of this test stayed green while
    // the flag was deleted. Same shape as #461's guard reading its own comment.
    let mise: String = read("mise.toml")
        .lines()
        .map(|l| l.split('#').next().unwrap_or(""))
        .collect::<Vec<_>>()
        .join("\n");
    let task = mise
        .split("[tasks.design-lint]")
        .nth(1)
        .unwrap_or_else(|| panic!("mise.toml declares no `design-lint` task"));
    let body = task.split("\n[tasks.").next().unwrap_or("");

    assert!(
        body.contains("oxlint") && body.contains(CONFIG.trim_start_matches("yidam/design/")),
        "the `design-lint` task no longer runs oxlint against the adherence config"
    );
    assert!(
        body.contains("--deny-warnings"),
        "`design-lint` no longer passes `--deny-warnings`, so oxlint's own default rules \
         report and pass"
    );

    let workflows = repo_root().join(".github/workflows");
    let runs = std::fs::read_dir(&workflows)
        .expect("no .github/workflows")
        .filter_map(Result::ok)
        .filter(|e| e.path().extension().is_some_and(|x| x == "yml"))
        .any(|e| {
            let text = std::fs::read_to_string(e.path()).unwrap_or_default();
            // Comments stripped: a job's prose about the lint is not the job running it.
            text.lines()
                .map(|l| l.split('#').next().unwrap_or(""))
                .any(|l| l.contains("mise run design-lint"))
        });
    assert!(
        runs,
        "no workflow runs `mise run design-lint`. The lint would be back where #465 found \
         it: a config for a rule nobody checks."
    );
}

/// The lint is proved by running it, not by reading it.
///
/// The whole of #467's finding in one assertion. Three tests in this file were green against
/// a lint that enforced nothing, because all three read the config. `oxlint` is provisioned by
/// the `design-lint` task and is not on this gate's PATH, so what is checked here is that the
/// proof exists and that the task performs it; the proof itself lives in the script.
#[test]
fn the_lint_is_proved_against_a_file_that_breaks_it() {
    let script = "scripts/design-lint-selftest.sh";
    let selftest = read(script);
    assert!(
        selftest.contains("--rules"),
        "{script} no longer checks that the config names rules oxlint implements — the defect \
         that hid 47 dead selectors for the life of the config"
    );

    let fixture_dir = repo_root().join("yidam/tests/design-lint-selftest");
    let fixtures: Vec<PathBuf> = std::fs::read_dir(&fixture_dir)
        .unwrap_or_else(|e| panic!("{} is unreadable ({e})", fixture_dir.display()))
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "jsx"))
        .collect();
    assert!(
        !fixtures.is_empty(),
        "{} holds no fixture, so the self-test proves the lint reports on nothing",
        fixture_dir.display()
    );
    // The fixture has to actually break a rule the config still carries. One that stopped
    // doing so would leave the self-test passing and the lint unproven.
    let breaks: Vec<&PathBuf> = fixtures
        .iter()
        .filter(|p| {
            let text = std::fs::read_to_string(p).unwrap_or_default();
            text.lines()
                .filter(|l| l.trim_start().starts_with("import "))
                .any(|l| l.contains("/components/"))
        })
        .collect();
    assert!(
        !breaks.is_empty(),
        "no fixture in {} imports a component internal any more, so the self-test asserts \
         that a clean file is clean",
        fixture_dir.display()
    );

    // Comments stripped: the task's own note explains what the self-test is for, and a check
    // satisfied by that note is the mistake this file has now found three times.
    let mise: String = read("mise.toml")
        .lines()
        .map(|l| l.split('#').next().unwrap_or(""))
        .collect::<Vec<_>>()
        .join("\n");
    let task = mise
        .split("[tasks.design-lint]")
        .nth(1)
        .expect("mise.toml declares no `design-lint` task")
        .split("\n[tasks.")
        .next()
        .unwrap_or_default();
    assert!(
        task.contains(script),
        "the `design-lint` task no longer runs {script}, so the lint is back to being \
         checked only by reading it"
    );
}
