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
//! 4. **`no-restricted-syntax` is not a rule oxlint has.** An unknown rule key is accepted
//!    at load and ignored at run, so all 47 selectors — every prop contract, the hex rule,
//!    the px rule, the font rule — did nothing at all. (This was established from `oxlint
//!    --rules`, the linter's own inventory. From 1.66.0 that flag prints nothing — while
//!    still exiting zero — whenever oxlint detects it is being run by a coding agent, so the
//!    self-test reads the resolved config instead: #877, oxc-project/oxc#26343.)
//! 5. **`no-restricted-imports` is implemented, and was switched off everywhere.** An
//!    `overrides` block exempting `**/index.js` disabled the rule for every file, not for
//!    that one; and its patterns were written for bare specifiers (`components/core/**`)
//!    while every import in the tree is relative (`../core/Badge.jsx`), so it could not have
//!    matched even had it run.
//!
//! # And it could not reach the code that breaks it (#611)
//!
//! A fourth thing, found by asking where the surviving rule could ever fire. It forbids
//! reaching past `index.js` into a component's internals — and the task linted `yidam/design`,
//! so the only files in a position to do that were outside the path. The rule was live, proved
//! by a fixture, and unreachable by every consumer in the repository.
//!
//! The path is now the repository. Not a list of consumer directories: a list would stop
//! covering the next surface without going red, which is the same defect one layer up from the
//! extension rosters #611 found in `design_tokens.rs` and `design_system.rs`.
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

/// The `design-lint` task body with comments stripped.
///
/// Stripping is load-bearing and has been since the first version of this file: the note above
/// the task explaining *why* it passes `--deny-warnings` satisfied the check that it passes it,
/// and the flag could be deleted under a green test. Same shape as #461's guard reading its own
/// comment, and the reason every assertion below reads this rather than the file.
fn design_lint_task() -> String {
    let mise: String = read("mise.toml")
        .lines()
        .map(|l| l.split('#').next().unwrap_or(""))
        .collect::<Vec<_>>()
        .join("\n");
    mise.split("[tasks.design-lint]")
        .nth(1)
        .unwrap_or_else(|| panic!("mise.toml declares no `design-lint` task"))
        .split("\n[tasks.")
        .next()
        .unwrap_or_default()
        .to_string()
}

const SELFTEST: &str = "scripts/design-lint-selftest.sh";

/// The self-test script with whole-line comments stripped, for the same reason
/// [`design_lint_task`] strips them: that script is two-thirds prose, and every string these
/// assertions look for is discussed in it before it is used.
fn selftest_code() -> String {
    read(SELFTEST)
        .lines()
        .filter(|l| !l.trim_start().starts_with('#'))
        .collect::<Vec<_>>()
        .join("\n")
}

/// One oxlint invocation in the task: its flags, and the paths it lints.
struct Invocation {
    flags: Vec<String>,
    paths: Vec<String>,
}

impl Invocation {
    fn has(&self, flag: &str) -> bool {
        self.flags.iter().any(|f| f == flag)
    }
    /// The value given to a flag, for the flags that take one.
    fn value(&self, flag: &str) -> Option<&str> {
        self.flags
            .iter()
            .position(|f| f == flag)
            .and_then(|i| self.flags.get(i + 1))
            .map(String::as_str)
    }
}

/// Every oxlint invocation the task runs, parsed into flags and paths.
///
/// A parse rather than a `contains`, because what is asserted below is *which paths* are
/// linted, and that is the thing a `contains("yidam/design")` cannot tell you: the invocation
/// that covers the consumers also names the config under `yidam/design/`. The check that
/// #611's finding is fixed has to be able to tell a config argument from a target.
fn oxlint_invocations() -> Vec<Invocation> {
    // Flags that consume the next token. Anything else starting with `-` is a bare flag, and
    // everything left is a path oxlint will walk.
    const TAKES_VALUE: &[&str] = &[
        "--config",
        "-c",
        "--ignore-pattern",
        "--ignore-path",
        "-A",
        "-D",
        "-W",
        "--allow",
        "--deny",
        "--warn",
        "--format",
        "-f",
        "--max-warnings",
        "--threads",
        "--tsconfig",
    ];
    let mut out = Vec::new();
    for line in design_lint_task().lines() {
        let line = line.trim().trim_matches(',').trim_matches('"').trim();
        if !line.starts_with("oxlint ") {
            continue;
        }
        let tokens: Vec<String> = line
            .split_whitespace()
            .skip(1)
            .map(|t| t.trim_matches('\'').to_string())
            .collect();
        let mut flags = Vec::new();
        let mut paths = Vec::new();
        let mut i = 0;
        while i < tokens.len() {
            let t = &tokens[i];
            if t.starts_with('-') {
                flags.push(t.clone());
                if TAKES_VALUE.contains(&t.as_str()) {
                    if let Some(v) = tokens.get(i + 1) {
                        flags.push(v.clone());
                    }
                    i += 1;
                }
            } else {
                paths.push(t.clone());
            }
            i += 1;
        }
        out.push(Invocation { flags, paths });
    }
    assert!(
        !out.is_empty(),
        "the `design-lint` task runs no oxlint invocation at all"
    );
    out
}

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

/// The plugin list is not trimmed to the rules that happen to name it.
///
/// `react` is in `plugins` and no rule in this config is a react rule — #881 removed the one
/// that was, `react/forbid-elements`, which had sat at `"forbid": []` since the initial
/// design-tool sync and so forbade no element on any file. The obvious tidy-up is to drop
/// `react` from the list next to it. That would be a silent loss of 18 rules: a `plugins` key
/// replaces oxlint's default list rather than adding to it, and the react plugin's own
/// correctness rules are carrying the JSX in `yidam/design/components/` whether or not this
/// config names one of them by hand. Measured on the pinned linter: 74 rules with `react`
/// present, 56 without.
///
/// The same argument holds for `import`, which no rule here names either.
#[test]
fn the_plugin_list_is_not_trimmed_to_the_rules_that_name_it() {
    let cfg = config();
    let plugins: Vec<&str> = cfg["plugins"]
        .as_array()
        .unwrap_or_else(|| panic!("{CONFIG} declares no `plugins` list"))
        .iter()
        .filter_map(|p| p.as_str())
        .collect();
    for required in ["react", "import"] {
        assert!(
            plugins.contains(&required),
            "`{required}` is gone from the `plugins` list in {CONFIG}. A `plugins` key \
             replaces oxlint's defaults rather than extending them, so dropping one removes \
             every rule it contributes — 18 of them for `react` on the pinned version. If it \
             was dropped because no rule here names it, that is exactly the reasoning the \
             comment on this test exists to answer."
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
    let body = design_lint_task();

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

/// The lint reaches the code that can break it.
///
/// #611's half of this file. `no-restricted-imports` forbids importing a component's internals
/// instead of the barrel, and for the life of the rule the task linted `yidam/design` — where
/// the only file that imports internals is `index.js`, the barrel itself, which the config
/// ignores by name. A rule that cannot fire is the same nothing as a rule that is not there,
/// and this one was in the stronger-looking position of having a fixture prove it worked.
///
/// What is asserted is that some invocation lints the *repository*, not that it lints a list of
/// places consumers currently live. Those are different gates: the second stops covering the
/// next surface silently, which is how three extension rosters in this repository came to have
/// the same hole in them.
#[test]
fn the_lint_reaches_every_consumer_and_not_by_a_roster() {
    let invocations = oxlint_invocations();
    let repo_wide: Vec<&Invocation> = invocations
        .iter()
        .filter(|i| i.paths.iter().any(|p| p == "." || p == "./"))
        .collect();
    assert_eq!(
        repo_wide.len(),
        1,
        "exactly one oxlint invocation should lint the repository, and {} do. Every path this \
         task lints: {:?}. A path under `yidam/design` covers the design system and no consumer \
         of it, which is the state #611 found: the import boundary could not fire on the only \
         code in a position to cross it. A roster of consumer directories is not the repair — \
         it goes quiet the moment a surface appears somewhere the roster does not name.",
        repo_wide.len(),
        invocations
            .iter()
            .flat_map(|i| i.paths.clone())
            .collect::<Vec<_>>()
    );
    let repo_wide = repo_wide[0];

    assert!(
        repo_wide.has("--deny-warnings"),
        "the repository-wide pass no longer passes `--deny-warnings`, so a rule that reports at \
         warning level reports and exits zero"
    );

    // `-A correctness`, and the assertion is that it is *there*. Without it this config becomes
    // a second opinion about correctness over the whole tree: measured, it reports
    // `const { home, ...v }` as an unused binding, which the root `.oxlintrc.json` exempts on
    // purpose for three packages. Two configs answering one question differently is how an
    // exemption stops being a decision and starts being an argument.
    assert_eq!(
        repo_wide.value("-A").or_else(|| repo_wide.value("--allow")),
        Some("correctness"),
        "the repository-wide pass no longer suppresses oxlint's default category. Correctness \
         has one home here — the root `.oxlintrc.json` three packages extend — and this config \
         is about adherence. `scripts/design-lint-selftest.sh` proves the flag does not also \
         suppress the two rules this config names."
    );

    // The one exclusion, and it is checked for staleness rather than merely for presence. A
    // pattern naming a directory that has moved excludes nothing and says nothing; a pattern
    // that has grown excludes the repository and still reads like an exclusion.
    let ignored: Vec<&String> = repo_wide
        .flags
        .windows(2)
        .filter(|w| w[0] == "--ignore-pattern")
        .map(|w| &w[1])
        .collect();
    assert_eq!(
        ignored.len(),
        1,
        "the repository-wide pass excludes {} path patterns; it should exclude exactly one, the \
         self-test fixture that breaks the rule on purpose: {ignored:?}",
        ignored.len()
    );
    let ignored = ignored[0].trim_matches('\'');
    let dir = ignored.trim_end_matches("/**").trim_end_matches('*');
    assert!(
        repo_root().join(dir).is_dir(),
        "the repository-wide pass excludes `{ignored}`, and `{dir}` is not a directory. An \
         exclusion naming a path that has moved excludes nothing and reads exactly like one \
         that works."
    );
    assert!(
        dir.starts_with("yidam/tests/"),
        "the only thing this pass may exclude is the self-test's deliberately-broken fixture, \
         and `{dir}` is not under `yidam/tests/`. Everything else in the repository is either a \
         consumer or silent about the design system, and silence costs this lint nothing."
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
    let script = SELFTEST;
    let selftest = read(script);
    assert!(
        selftest.contains("--print-config"),
        "{script} no longer checks that the config names rules oxlint implements — the defect \
         that hid 47 dead selectors for the life of the config. It is checked by reading \
         oxlint's resolved config, because the inventory it used to read answers differently \
         depending on who runs it — empty, and still exit zero, under a coding agent (#877)."
    );
    // The negative control is the half that keeps the half above honest, so it is registered
    // too. `oxlint --rules` did not fail when it stopped answering — it kept exiting zero and
    // printing nothing, and did it only in the environments nobody runs CI in. A check that
    // proves its own mechanism still discriminates cannot go quiet that way; one that drops
    // the proof can.
    assert!(
        selftest.contains("no-such-rule-as-this"),
        "{script} no longer spikes the config with a rule that cannot exist. Without that \
         probe, nothing notices if `--print-config` stops telling an implemented rule from \
         an invented one — which is exactly how `--rules` rotted undetected (#877)."
    );

    // The strongest of the three, and the last to arrive: resolving is not enforcing. #881's
    // `react/forbid-elements` resolved, was an error, and was configured to forbid nothing,
    // so it passed every check this file had while catching nothing on any file. The script
    // now requires each rule to report on the fixture by name.
    assert!(
        selftest.contains("did not fire"),
        "{script} no longer requires every rule to FIRE on the fixture. Resolving only says \
         the linter knows the name; a rule can resolve, be an error, and still enforce \
         nothing — by an `overrides` block (#467) or by being configured against nothing at \
         all (#881). Both defects lived in this config and both passed every weaker check."
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

    let task = design_lint_task();
    assert!(
        task.contains(script),
        "the `design-lint` task no longer runs {script}, so the lint is back to being \
         checked only by reading it"
    );
}

/// What the gates declare in scope is what oxlint is made to prove it can read.
///
/// The last silence #611 left. `scripts/design-lint-selftest.sh` discovers file types from the
/// tree, which proves a type the moment a file of it exists — one commit *after* the surface
/// whose coverage was the point. `tsx` is that case: three gates put it in scope and this tree
/// holds no `.tsx` file, so discovery alone would have proved every type except the one the
/// issue was about.
///
/// The repair was not to hardcode `tsx` in the script. That is the roster a fourth time, and it
/// covers `tsx` while going quiet for whatever comes next; the script unions in
/// `CONSUMER_EXTENSIONS` instead, read out of the gate that declares it. So an extension cannot
/// enter the token gate's scope without this script demanding oxlint prove it can read that
/// type — and if oxlint cannot, the script says so rather than the type being covered by nobody.
///
/// Both halves of that derivation are checked here, because either can fail silently: the union
/// can be deleted, and the `sed` that reads the const can stop matching if the const is
/// reformatted. The second is checked by running the script's own expression rather than a
/// second copy of it — a guard comparing two hardcoded copies is the defect #831 found.
#[test]
fn the_selftest_proves_the_extensions_the_gates_declare() {
    let code = selftest_code();
    assert!(
        code.contains("CONSUMER_EXTENSIONS"),
        "{SELFTEST} no longer reads `CONSUMER_EXTENSIONS` out of `design_tokens.rs`, so the set \
         of file types it makes oxlint prove is whatever the tree happens to hold. A type \
         declared in scope with no file of it yet — `tsx`, on the day #611 landed — is then \
         proved by nothing, which is the hole that issue was filed about."
    );

    // The script's own extraction, run against the file the script names — both halves taken
    // from the script rather than retyped. That is the whole point of running it: an expression
    // or a path copied into this test would go on passing here while the script read nothing.
    // What this cannot see is the extraction being neutered in the shell rather than in its
    // arguments; the script's own `[ -n "$declared" ]` floor covers that, in `design-lint`.
    let (program, rest) = code
        .split_once("sed -n '")
        .and_then(|(_, rest)| rest.split_once('\''))
        .unwrap_or_else(|| panic!("{SELFTEST} runs no `sed -n '…'`; the extraction has moved"));
    let source = rest
        .split_once("\"$root/")
        .and_then(|(_, r)| r.split_once('"'))
        .map(|(path, _)| path.to_string())
        .unwrap_or_else(|| {
            panic!("{SELFTEST} passes `sed` no `\"$root/…\"` file; the extraction has moved")
        });
    assert!(
        repo_root().join(&source).is_file(),
        "{SELFTEST} reads `CONSUMER_EXTENSIONS` out of `{source}`, which is not a file. A `sed` \
         over a path that does not exist reads nothing, exits zero under `-n`, and leaves the \
         declared set empty — the script probes whatever the tree holds and says nothing about \
         it."
    );
    let out = std::process::Command::new("sed")
        .arg("-n")
        .arg(program)
        .arg(repo_root().join(&source))
        .output()
        .unwrap_or_else(|e| panic!("sed did not run: {e}"));
    let declared = String::from_utf8_lossy(&out.stdout);
    assert!(
        !declared.trim().is_empty(),
        "{SELFTEST}'s expression `{program}` reads nothing out of `{source}`, so the \
         declared set it unions in is empty and the script probes only what the tree holds. \
         Nothing fails when that happens except the script's own floor assertion, and that \
         runs in `design-lint`, not here."
    );
    // `tsx` specifically, because it is the member that discovery cannot supply: no `.tsx` file
    // exists in this tree. If one is added this assertion keeps passing and stops being the
    // interesting one — the derivation is still what covers the member after it.
    assert!(
        declared.contains("tsx"),
        "{SELFTEST}'s expression `{program}` reads `{}` out of `CONSUMER_EXTENSIONS`, and `tsx` \
         is not in it. That is the one extension in scope with no file of its type in the tree, \
         so it is the one the union exists for.",
        declared.trim()
    );
}
