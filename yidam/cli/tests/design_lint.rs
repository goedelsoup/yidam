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

/// The rules the lint exists for are present and say something.
///
/// Discovered by what the selectors match rather than by counting them: a config with three
/// entries none of which mentions a colour is the same nothing as a config with none.
#[test]
fn the_adherence_rules_still_forbid_what_they_were_written_to_forbid() {
    let cfg = config();
    let restricted = cfg["rules"]["no-restricted-syntax"]
        .as_array()
        .unwrap_or_else(|| panic!("{CONFIG} declares no `no-restricted-syntax`"));

    let selectors: String = restricted
        .iter()
        .filter_map(|e| e.get("selector").and_then(|s| s.as_str()))
        .collect::<Vec<_>>()
        .join("\n");

    for (what, needle) in [
        ("a raw hex colour", "#[0-9a-fA-F]"),
        ("a raw px value", "px"),
        ("an off-system font", "font-family"),
    ] {
        assert!(
            selectors.contains(needle),
            "no rule forbids {what} any more. `mise run design-lint` still exits zero with \
             an empty rule list, which is why this is asserted here and not left to the job."
        );
    }
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
