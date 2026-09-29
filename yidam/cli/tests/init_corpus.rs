//! What `yidam init` writes has to survive `yidam init`'s own next step.
//!
//! The command's whole claim is that a person can run it in their own project and have a
//! corpus — and "have a corpus" means the gates pass. #1035's argument for the command was
//! that the three existing routes were two commands that only run inside a yidam checkout
//! and a documentation page; a fourth route that produces a tree failing `graph-check` on
//! the first run would be worse than all three, because it fails *after* the person has
//! committed to the tool.
//!
//! So this file is not a shape test. Every assertion here is the output of a command the
//! generated `Next steps:` block tells the reader to run, over the tree the command just
//! wrote. `tests/first_corpus.rs` does the same thing for the page; this does it for the
//! command that replaced most of the page's job.
//!
//! **`graph-check` and `lint` are asserted on the exit code**, for `repo_detection.rs`'s
//! reason: a gate that says the right words and returns 0 is still broken.
//!
//! The class lists are varied deliberately. One class, two and three are three different
//! shapes of the instance ring — a doubled class, a mutual pair, and a cycle — and a
//! generator correct for the default and wrong for the others would pass a test that only
//! ran the default.

use std::path::Path;
use std::process::Command;

mod common;

struct Run {
    stdout: String,
    stderr: String,
    code: i32,
}

fn run(dir: &Path, args: &[&str]) -> Run {
    let out = Command::new(env!("CARGO_BIN_EXE_yidam"))
        .current_dir(dir)
        .args(args)
        .output()
        .unwrap();
    Run {
        stdout: String::from_utf8_lossy(&out.stdout).to_string(),
        stderr: String::from_utf8_lossy(&out.stderr).to_string(),
        code: out.status.code().unwrap_or(-1),
    }
}

/// A git repository with nothing in it — the state `init` is for, and the only one.
///
/// The identity is configured here rather than inherited. A developer's machine has one and a
/// runner does not, so for as long as only the developer ran this, the fixture was reading the
/// host's `user.name` — and `the_catalog_directory_survives_a_commit`, the one test in this
/// file that commits, passed locally and failed on CI with *"Author identity unknown"*. Every
/// other committing fixture in this suite sets it the same way, and `src/git/fixture.rs` does
/// on the library side.
///
/// It is deliberately not set once in `common::git::spawn` as an environment variable, which
/// would close this for every fixture at once: `mcp_act_tier.rs` asserts that a commit's
/// author and committer *differ* — the split RFC-0029 §2.2 rests on — and an inherited
/// identity there would flatten the two into one name.
fn empty_repo() -> tempfile::TempDir {
    let tmp = tempfile::tempdir().unwrap();
    for args in [
        vec!["init", "-q"],
        vec!["config", "user.email", "example@yidam.test"],
        vec!["config", "user.name", "Example"],
    ] {
        common::git::git(tmp.path(), &args);
    }
    tmp
}

/// Run `init` with these `--class` names (none = the default) and return the tree.
fn initialised(classes: &[&str]) -> tempfile::TempDir {
    let tmp = empty_repo();
    let mut args = vec!["init"];
    for c in classes {
        args.push("--class");
        args.push(c);
    }
    let r = run(tmp.path(), &args);
    assert_eq!(
        r.code,
        0,
        "`yidam {}` failed\nstdout: {}\nstderr: {}",
        args.join(" "),
        r.stdout,
        r.stderr
    );
    tmp
}

/// The class lists every test below runs over. Named once, so adding a shape covers all of
/// them rather than one.
const SHAPES: &[&[&str]] = &[&[], &["thing"], &["reach", "gage"], &["a", "b", "c"]];

#[test]
fn the_shapes_cover_more_than_the_default() {
    assert!(
        SHAPES.len() >= 3,
        "one shape is the default, and a generator wrong for every other class count would \
         pass every test in this file"
    );
    assert!(
        SHAPES.iter().any(|s| s.len() == 1),
        "the single-class ring is the one case that is not a cycle over distinct classes"
    );
}

/// The first command the generated `Next steps:` block names.
#[test]
fn graph_check_passes_over_what_init_wrote() {
    for classes in SHAPES {
        let d = initialised(classes);
        let r = run(d.path(), &["graph-check"]);
        assert_eq!(
            r.code,
            0,
            "graph-check failed over `init --class {}`\nstdout: {}\nstderr: {}",
            classes.join(" --class "),
            r.stdout,
            r.stderr
        );
    }
}

/// The second. Clean, not merely non-erroring: a corpus that greets its author with
/// warnings about the file the tool just wrote teaches them that findings are noise.
#[test]
fn lint_is_clean_over_what_init_wrote() {
    for classes in SHAPES {
        let d = initialised(classes);
        let r = run(d.path(), &["lint"]);
        assert_eq!(
            r.code,
            0,
            "lint failed over `init --class {}`\nstdout: {}\nstderr: {}",
            classes.join(" --class "),
            r.stdout,
            r.stderr
        );
        assert!(
            r.stdout.contains("0 finding(s)"),
            "lint reported findings against the tree `init` wrote — a first corpus that \
             arrives with warnings teaches that warnings are the normal state:\n{}",
            r.stdout
        );
    }
}

/// #1035's second requirement: the first run of this report must not be empty.
///
/// It is a weak assertion on its own and is kept for what it is — the report the issue
/// names, run over the tree, exiting 0 with a row per node. It does **not** establish that
/// either tag `init` writes is load-bearing: the nodes carry the standing twice, once as
/// `[open]` in prose and once as a bare `claim_tag: open`, and `has_open_claim` is a
/// disjunction over both. Deleting either one alone leaves this passing.
///
/// [`the_open_tally_counts_both_forms_the_nodes_teach`] is the test that holds them.
#[test]
fn open_questions_answers_with_every_node_init_wrote() {
    for classes in SHAPES {
        let d = initialised(classes);
        let r = run(d.path(), &["open-questions"]);
        assert_eq!(r.code, 0, "stderr: {}", r.stderr);

        let listed = r.stdout.lines().filter(|l| l.starts_with("- [")).count();
        // One node per class, and never fewer than two: a single class gets a second
        // instance rather than a node linking to itself.
        let expected = std::cmp::max(classes.len(), 2);
        assert_eq!(
            listed,
            expected,
            "`open-questions` listed {listed} of {expected} node(s) after \
             `init --class {}`. Every node `init` writes carries an `[open]` claim, and a \
             first report that answers with nothing is a report a reader concludes is \
             broken.\n{}",
            classes.join(" --class "),
            r.stdout
        );
    }
}

/// `status` reads the same tree through a third path — the claim tally rather than the
/// question list — and it is what the README block of a derived corpus is built from.
#[test]
fn status_counts_the_nodes_and_the_open_claims() {
    for classes in SHAPES {
        let d = initialised(classes);
        let n = std::cmp::max(classes.len(), 2);
        let r = run(d.path(), &["status"]);
        assert_eq!(r.code, 0, "stderr: {}", r.stderr);
        assert!(
            r.stdout.contains(&format!("**{n} nodes**")) && r.stdout.contains(&format!("{n} open")),
            "status does not see the {n} nodes as open: {}",
            r.stdout
        );
    }
}

/// **The tally is exactly two open claims per node, and that is the assertion.**
///
/// Every node `init` writes states its standing twice: `[open]` at the end of the prose,
/// and a bare `claim_tag: open` under `properties:`. That is deliberate — the file is the
/// only worked example most readers will meet, and the two forms are not
/// interchangeable — but it makes every *weaker* check on this file a disjunction.
/// `open-questions` is satisfied by either; so is `status`'s `N open`; so is any assertion
/// that some claim was counted. Each of the three mutations below survived the rest of
/// this file:
///
/// - `type: claim` → `type: text` on the class, which stops the bare value being read
/// - the `[open]` deleted from the description, which stops the prose scan seeing one
/// - `claim_tag: open` → `claim_tag: verified`, caught only because it then trips `lint`
///
/// Two claims per node is the one statement that fails on all three, because it is the
/// only one that says *both* forms were counted rather than at least one. Measured, not
/// assumed: `count_in_node` adds the prose scan to the structural read and subtracts what
/// both saw, and a bare value is not bracketed, so the two do not collapse.
///
/// If a future `init` deliberately stops writing one of the forms, this test should be
/// changed to `1` in the same commit — not deleted. The number is the contract.
#[test]
fn the_open_tally_counts_both_forms_the_nodes_teach() {
    for classes in SHAPES {
        let d = initialised(classes);
        let n = std::cmp::max(classes.len(), 2);
        let r = run(d.path(), &["status"]);
        assert_eq!(r.code, 0, "stderr: {}", r.stderr);
        let want = format!("claims 0v / 0i / {}o", 2 * n);
        assert!(
            r.stdout.contains(&want),
            "expected `{want}` after `init --class {}` — {n} node(s), each tagged once in \
             prose and once as a typed property. A tally of {n} means one of the two forms \
             is no longer being read, and the example file is teaching a spelling the tool \
             ignores.\n{}",
            classes.join(" --class "),
            r.stdout
        );
    }
}

/// The catalog directory exists and is tracked, which is why it holds a file at all: git
/// stores no empty directory, so a corpus whose `.yidam/catalog/` was created empty arrives
/// at the other end of a clone without it.
#[test]
fn the_catalog_directory_survives_a_commit() {
    let d = initialised(&[]);
    common::git::git(d.path(), &["add", "-A"]);
    common::git::git_at(
        d.path(),
        &["commit", "-qm", "genesis: the example domain"],
        common::git::FIXTURE_DATE,
    );

    let tracked = common::git::out(d.path(), &["ls-files", ".yidam/catalog"]);
    assert!(
        !tracked.trim().is_empty(),
        "`.yidam/catalog/` tracks no file, so it does not survive a clone"
    );
}

/// `init` does not commit, and that is a decision rather than an omission: the first
/// commit's subject is a `genesis:` naming the domain, `export` and `bundle` read the domain
/// off it, and only the person knows what it is.
#[test]
fn init_leaves_the_genesis_commit_to_the_person() {
    let d = initialised(&[]);
    let count = common::git::out(d.path(), &["rev-list", "--count", "--all"]);
    assert_eq!(
        count, "0",
        "`init` committed. The first commit names the domain in its `genesis:` subject, \
         and a command that has never seen the domain cannot write that subject."
    );
}

/// Running it twice is the mistake somebody makes on their second day, and the answer has to
/// be a refusal rather than an overwrite: the second run's placeholders would replace a
/// class the person had spent the afternoon writing.
#[test]
fn a_second_run_refuses_rather_than_overwriting_the_first() {
    let d = initialised(&["reach", "gage"]);
    let edited = d.path().join(".yidam/corpus/reach.ont.yml");
    let before = std::fs::read_to_string(&edited).unwrap();
    let mine = before.replace("Rewrite this sentence.", "A length of river between two");
    assert_ne!(
        mine, before,
        "the placeholder sentence moved; fix this fixture"
    );
    std::fs::write(&edited, &mine).unwrap();

    let r = run(d.path(), &["init"]);
    assert_ne!(r.code, 0, "the second run succeeded: {}", r.stdout);
    assert_eq!(
        std::fs::read_to_string(&edited).unwrap(),
        mine,
        "the second run overwrote an edited class file"
    );
}

/// The `Next steps:` block is the only instruction most readers will see, so the commands it
/// names have to be ones the binary has. It is prose in a `println!` and nothing else checks
/// it — and it is the surface most likely to name a command by the name it had last quarter.
#[test]
fn the_next_steps_name_commands_the_binary_has() {
    let d = empty_repo();
    let r = run(d.path(), &["init"]);
    assert_eq!(r.code, 0, "stderr: {}", r.stderr);

    let commands = common::commands_from_help();

    // Discovered from the block rather than listed here, so a command added to it is
    // covered the day it is added.
    let words: Vec<&str> = r.stdout.split_whitespace().collect();
    let named: Vec<String> = words
        .windows(2)
        .filter(|w| w[0] == "yidam" || w[0] == "`yidam")
        .map(|w| w[1].trim_end_matches(['&', ';', '`', '.', ',']).to_string())
        .filter(|n| !n.is_empty() && n.chars().all(|c| c.is_ascii_lowercase() || c == '-'))
        .collect();

    for name in &named {
        assert!(
            commands.contains(name),
            "`init` tells the reader to run `yidam {name}`, which `--help-all` does not \
             list:\n{}",
            r.stdout
        );
    }

    // The floor, asserted per command rather than as a count: a scanner blind to the whole
    // block clears `named.len() >= 3` the moment anything else in stdout happens to read
    // as an instruction.
    for expected in ["graph-check", "lint", "schema"] {
        assert!(
            named.iter().any(|n| n == expected),
            "the next-steps block no longer names `yidam {expected}`, or the scanner \
             stopped seeing it. Found {named:?} in:\n{}",
            r.stdout
        );
    }
}
