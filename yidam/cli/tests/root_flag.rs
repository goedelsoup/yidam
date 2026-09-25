//! `--root <DIR>` reads the corpus it names, on every command that offers it.
//!
//! Forty-two commands accept the flag since #918, where two did before it. Declaring it is a
//! line in `main.rs`; *honouring* it is a parameter threaded from that line through the
//! command function to [`yidam::paths::resolve_root`], and the two are independent. A command
//! that declares the flag and drops the value compiles, accepts `--root`, prints a plausible
//! report, and answers about whichever directory the process started in. Nothing about the
//! output says which corpus it read, so no reader can tell — which is the silent failure
//! `export` was measured against in #428 and the one worth a gate.
//!
//! **The roster is discovered**: every command whose own `--help` lists `--root`, read from the
//! built binary. A list here would stop covering new commands without ever going red, which is
//! the rot #448 names. The table below supplies only what a command needs to *run* — a range, a
//! query, a subcommand — and which observable answers "did it read that corpus". Both
//! directions are asserted, so a forty-third rooted command fails this file until somebody
//! decides how to prove it.
//!
//! Every command is run from a directory that is not a repository at all. A dropped thread
//! then resolves *that* directory rather than this checkout, so the failure is a report about
//! nothing rather than a report about yidam — and, more to the point, this suite cannot
//! rewrite the working tree it is run from.
//!
//! This does not check the other half of #918's division: that the write commands **refuse**
//! the flag. `reading_surface.rs` holds the `*` marker those commands carry, and a command that
//! both writes a commit and takes a corpus to write it into is a design question rather than a
//! threading bug.

mod common;

use std::collections::BTreeSet;
use std::path::Path;

/// What proves a command read the corpus it was handed.
///
/// Four, because there is no single observable across the surface and pretending otherwise
/// would mean a weaker assertion everywhere rather than a strong one where it is available.
/// Which one a command needs is itself worth recording: it says what that command's output
/// commits to.
enum Tell {
    /// The JSON report envelope names the root it resolved (RFC-0016).
    ///
    /// The strong one, and the default: `root` is a field of the envelope every `--format
    /// json` report carries, so the assertion is an equality against the directory passed in
    /// rather than a guess about the prose. Twenty-seven commands can answer this way, and
    /// `regen --check` was not one of them until this file measured it — it took `--root`,
    /// read the named corpus's blocks, and put `repo_root()` in the envelope.
    Envelope,
    /// A REGEN generator: the block it writes is a file **inside** the corpus.
    ///
    /// The named path is seeded with a block holding a placeholder, and the assertion is that
    /// the seeded file no longer holds it. Stronger than reading the generator's stdout, which
    /// is the same text wherever it was written; this is the write landing in the right tree.
    Writes(&'static str),
    /// The command names the directory it was pointed at, in its output.
    ///
    /// For the five that neither emit an envelope nor write a block. Each of them refuses, or
    /// reports a missing input, in a message that quotes the resolved path — so pointing them
    /// at a directory holding no corpus is what makes them speak. It is a *second* such
    /// directory, not the one they are run from: pointed at their own working directory a
    /// command that dropped the flag would name it and pass.
    Names,
    /// Not checkable here, and why.
    Elsewhere(&'static str),
}

/// Where a `{corpus}` argument goes. Substituted at run time; the table is a const.
const CORPUS: &str = "{corpus}";

/// Every rooted command: the arguments after its name, and its tell.
///
/// `--format json` is spelled in the arguments rather than appended by the runner, because
/// `policy` takes it on the subcommand and everything else takes it on the command. One less
/// special case than a runner that knows where to put it.
const INVOCATIONS: &[(&str, &[&str], Tell)] = &[
    ("agents-index", &[], Tell::Writes("agents/README.md")),
    ("bench", &[], Tell::Names),
    ("bundle-status", &[], Tell::Writes("web/README.md")),
    ("catalog-audit", &["--format", "json"], Tell::Envelope),
    (
        "check-diff",
        &["HEAD~1", "--format", "json"],
        Tell::Envelope,
    ),
    ("cohort", &[CORPUS, "--format", "json"], Tell::Envelope),
    ("corpus-index", &["--format", "json"], Tell::Envelope),
    ("crates-index", &[], Tell::Writes("crates/README.md")),
    ("cycle", &["--format", "json"], Tell::Envelope),
    (
        "decisions-log",
        &[],
        Tell::Writes(".yidam/decisions/README.md"),
    ),
    (
        "diff",
        &["HEAD~1..HEAD", "--format", "json"],
        Tell::Envelope,
    ),
    ("doctor", &["--format", "json"], Tell::Envelope),
    ("due", &["--format", "json"], Tell::Envelope),
    (
        "estimate",
        &["low-flow", "--format", "json"],
        Tell::Envelope,
    ),
    // `export`'s `--format` names an export format, not a report format, so it has no
    // envelope to carry a root. It is the command #428 measured in place, and
    // `example_corpus.rs` still measures that; what is checked here is only the refusal.
    ("export", &["--format", "graphml"], Tell::Names),
    ("graph", &["--format", "json"], Tell::Envelope),
    ("graph-check", &["--format", "json"], Tell::Envelope),
    (
        "index-push",
        &[],
        Tell::Elsewhere(
            "refuses for want of the `vector-read` feature before it resolves anything, so \
             the default-feature binary these tests build cannot reach the flag at all",
        ),
    ),
    ("index-status", &["--format", "json"], Tell::Envelope),
    ("index-verify", &[], Tell::Names),
    ("kuten", &[], Tell::Writes("AGENTS.md")),
    ("lint", &["--format", "json"], Tell::Envelope),
    ("log", &["--format", "json"], Tell::Envelope),
    (
        "neighbors",
        &["concept/low-flow", "--format", "json"],
        Tell::Envelope,
    ),
    ("open-questions", &["--format", "json"], Tell::Envelope),
    ("pack", &["low-flow", "--format", "json"], Tell::Envelope),
    ("packages-index", &[], Tell::Writes("packages/README.md")),
    ("phases", &["--format", "json"], Tell::Envelope),
    ("policy", &["check", "--format", "json"], Tell::Envelope),
    ("practice", &[], Tell::Writes("PRACTICE.md")),
    ("query", &["low-flow", "--format", "json"], Tell::Envelope),
    // `--check`, so this reads the blocks rather than rewriting the ones the `Writes` rows
    // seeded. The writing mode runs the same fourteen generators through the same list.
    ("regen", &["--check", "--format", "json"], Tell::Envelope),
    ("replay", &["--format", "json"], Tell::Envelope),
    (
        "retrieve",
        &["low-flow", "--format", "json"],
        Tell::Envelope,
    ),
    ("samudaya-audit", &[], Tell::Names),
    ("sangha", &["--format", "json"], Tell::Envelope),
    (
        "score",
        &["HEAD~1..HEAD", "--format", "json"],
        Tell::Envelope,
    ),
    // Closed stdin: `serve --mcp` prints its banner and exits at EOF. Pointed at a directory
    // with no corpus it refuses before serving, which is the behaviour quickstart §5 promises
    // a client configured to launch it from the wrong place.
    ("serve", &["--mcp"], Tell::Names),
    ("skills-index", &[], Tell::Writes(".yidam/skills/README.md")),
    ("status", &["--format", "json"], Tell::Envelope),
    ("vault-status", &[], Tell::Writes("README.md")),
    ("vocabulary", &["--format", "json"], Tell::Envelope),
];

/// The seed a `Writes` row's file holds before its generator runs.
///
/// A REGEN block, because that is the only thing `update_file_regen` will touch — it returns
/// early on a file without the markers, so an empty seed would make every `Writes` row pass
/// by writing nothing. The placeholder is content no generator produces, so the block is
/// always stale and the write always happens.
fn seed_block(command: &str) -> String {
    format!(
        "# Seeded by root_flag.rs\n\n<!-- REGEN: yidam {command}\n-->\n{PLACEHOLDER}\n\
         <!-- /REGEN -->\n"
    )
}

const PLACEHOLDER: &str = "_nothing any generator would write_";

/// Every command whose own `--help` lists `--root`.
///
/// Per-command `--help` rather than the top-level listing: the flag is declared per
/// subcommand (a global would change `yidam --help`, which seven other tests parse), so the
/// top level says nothing about who takes it.
fn rooted_commands() -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    for name in common::commands_from_help() {
        let out = std::process::Command::new(env!("CARGO_BIN_EXE_yidam"))
            .args([&name, "--help"])
            .output()
            .unwrap_or_else(|e| panic!("running `yidam {name} --help`: {e}"));
        let help = String::from_utf8_lossy(&out.stdout);
        if help.contains("--root <DIR>") {
            found.insert(name);
        }
    }
    assert!(
        found.len() > 30,
        "only {} command(s) parsed as taking `--root` — either the flag's spelling changed or \
         this is no longer reading the help: {found:?}",
        found.len()
    );
    found
}

#[test]
fn every_rooted_command_has_an_invocation_here() {
    let declared = rooted_commands();
    let covered: BTreeSet<String> = INVOCATIONS.iter().map(|(c, ..)| c.to_string()).collect();

    let unchecked: Vec<_> = declared.difference(&covered).collect();
    assert!(
        unchecked.is_empty(),
        "these commands declare `--root` and nothing here proves they read it: {unchecked:?}\n\
         Add a row to INVOCATIONS with the arguments it needs and the observable that answers \
         for it. A declaration that is never threaded through accepts the flag and reports on \
         the wrong corpus."
    );

    let stale: Vec<_> = covered.difference(&declared).collect();
    assert!(
        stale.is_empty(),
        "INVOCATIONS names commands that do not take `--root`: {stale:?}\n\
         Either the flag was removed — in which case remove the row — or the command was."
    );
}

/// A named `--root` holding no corpus is refused, by every command that takes the flag.
///
/// The companion to the test below, and the other half of what the flag promises. That one
/// asks whether a command reads the corpus it was *given*; this asks what it does when the
/// directory it was given is not a corpus at all. Both were silent failures, and they are
/// the same silence: nothing in the output says which directory was read, so a clean report
/// about nowhere is indistinguishable from a clean report about the corpus you meant.
///
/// Measured before the fix, against a git repository with one commit and no `.yidam/`: five
/// of the forty-two refused, because they call `require_yidam_repo` for themselves. The other
/// thirty-seven proceeded — `status` printing **0 nodes**, `open-questions` printing *no open
/// questions*, `regen` reporting every generator current because `update_file_regen` no-ops
/// on a file that is not there. Every one of those lines is also the honest answer for a real
/// corpus that is genuinely empty in that respect, which is exactly the observation
/// `require_yidam_repo`'s doc comment rejects.
///
/// **The refusal, not merely a failure.** Asserting a nonzero exit alone would be cleared by
/// a command that fell over for an unrelated reason — `HEAD~1` that does not resolve in a
/// one-commit repository, a query with nothing to parse. The message is asserted too, and it
/// must quote the directory the caller named: a refusal that does not say *which* directory
/// leaves the typo as hard to see as the clean report did.
///
/// A git repository rather than a bare directory, because that is the near miss worth
/// covering: `--root` pointing at a checkout one level above the corpus, or at the wrong
/// checkout entirely. `paths.rs` holds the unit tests for the other two shapes — a directory
/// in no repository at all, and an unbootstrapped `yidam clone`, which keeps its own message.
#[test]
fn every_rooted_command_refuses_a_named_directory_that_is_not_a_corpus() {
    // Derived from the table rather than listed here: a row excused from proving it reads a
    // corpus is excused from this for the same reason, and a *new* excuse has to be argued
    // in one place instead of two.
    let exempt: BTreeSet<&str> = INVOCATIONS
        .iter()
        .filter(|(.., tell)| matches!(tell, Tell::Elsewhere(_)))
        .map(|(c, ..)| *c)
        .collect();

    // A git repository with one commit and no `.yidam/` — the shape `--root` most plausibly
    // lands on by mistake.
    let named = tempfile::tempdir().unwrap();
    let dir = named.path();
    common::git::git(dir, &["init", "-q", "-b", "main"]);
    common::git::git(dir, &["config", "user.email", "runner@test"]);
    common::git::git(dir, &["config", "user.name", "Runner"]);
    common::git::git_at(
        dir,
        &["commit", "-q", "--allow-empty", "-m", "not a corpus"],
        common::git::FIXTURE_DATE,
    );
    assert!(
        !named.path().join(".yidam").is_dir(),
        "the fixture must hold no corpus, or this test asserts nothing"
    );

    // Where the process stands, which is not where it is pointed. A command that dropped the
    // flag would resolve this instead — also not a corpus, so it would still fail; what it
    // could not do is name the directory the assertion below looks for.
    let elsewhere = tempfile::tempdir().unwrap();

    let mut failures: Vec<String> = Vec::new();
    for (command, args, _) in INVOCATIONS.iter().filter(|(c, ..)| !exempt.contains(c)) {
        let path = named.path().display().to_string();
        let mut argv: Vec<String> = vec![command.to_string(), "--root".into(), path.clone()];
        argv.extend(args.iter().map(|a| match *a {
            CORPUS => path.clone(),
            other => other.to_string(),
        }));

        let out = std::process::Command::new(env!("CARGO_BIN_EXE_yidam"))
            .current_dir(elsewhere.path())
            .args(&argv)
            .stdin(std::process::Stdio::null())
            .output()
            .unwrap_or_else(|e| panic!("running `yidam {}`: {e}", argv.join(" ")));
        let both = format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );

        if out.status.success() {
            failures.push(format!(
                "{command}: exited 0 and reported on a directory holding no corpus: {both:.300}"
            ));
        } else if !both.contains("not a yidam repository") {
            failures.push(format!(
                "{command}: failed, but not with the refusal — something else went wrong \
                 first, so this row proves nothing about the flag: {both:.300}"
            ));
        } else if !both.contains(&path) {
            failures.push(format!(
                "{command}: refused without naming the directory it was given: {both:.300}"
            ));
        }
    }

    assert!(
        failures.is_empty(),
        "these commands accept a named `--root` that is not a corpus:\n  {}\n\
         `--root <DIR>` is an assertion — the caller said that directory *is* the corpus — so \
         `paths::resolve_root` refuses one holding no `.yidam/`. A command that reports \
         cleanly here buys a clean bill of health for a corpus that was never read.",
        failures.join("\n  ")
    );
}

#[test]
fn every_rooted_command_reads_the_corpus_it_is_given() {
    // Held, not shadowed: `Example` owns the temporary directory, and dropping it here would
    // delete the corpus before the first command was run.
    let example = common::Example::materialize("streamflow");
    let corpus = example.path();

    // One commit is the example's genesis; the seeds are the second, so `HEAD~1` is a
    // revision and the three range-taking commands have something to resolve.
    for (command, _, tell) in INVOCATIONS {
        if let Tell::Writes(rel) = tell {
            let path = corpus.join(rel);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, seed_block(command)).unwrap();
        }
    }
    common::git::git(&corpus, &["add", "-A"]);
    common::git::git_at(
        &corpus,
        &["commit", "-qm", "add: REGEN blocks for the root gate"],
        common::git::FIXTURE_DATE,
    );

    // Not a repository, and not this one. Every command below is told where to look, so this
    // is only what a dropped thread would fall back to.
    let elsewhere = tempfile::tempdir().unwrap();
    // What the `Names` rows are pointed *at*: a second rootless directory, distinct from the
    // one above so that naming it cannot be naming the working directory by accident.
    let rootless = tempfile::tempdir().unwrap();

    let mut failures: Vec<String> = Vec::new();
    for (command, args, tell) in INVOCATIONS {
        // A `Names` row is pointed at the directory with no corpus: the refusal that quotes a
        // path is the only place these five say which one they resolved.
        let named: &Path = match tell {
            Tell::Names => rootless.path(),
            _ => &corpus,
        };
        let mut argv: Vec<String> = vec![command.to_string(), "--root".into()];
        argv.push(named.display().to_string());
        argv.extend(
            args.iter()
                .map(|a| match *a {
                    CORPUS => named.display().to_string(),
                    other => other.to_string(),
                })
                .collect::<Vec<_>>(),
        );

        let out = std::process::Command::new(env!("CARGO_BIN_EXE_yidam"))
            .current_dir(elsewhere.path())
            .args(&argv)
            .stdin(std::process::Stdio::null())
            .output()
            .unwrap_or_else(|e| panic!("running `yidam {}`: {e}", argv.join(" ")));
        let stdout = String::from_utf8_lossy(&out.stdout).to_string();
        let both = format!("{stdout}{}", String::from_utf8_lossy(&out.stderr));

        let verdict = match tell {
            // The excuse is asserted, not taken: if this command *does* name the corpus it
            // was given, it reached the flag after all and owes a real tell.
            Tell::Elsewhere(why) => match both.contains(&named.display().to_string()) {
                false => Ok(()),
                true => Err(format!(
                    "named the corpus, so it is no longer true that it {why} — give this row                      a tell of its own"
                )),
            },
            Tell::Envelope => match serde_json::from_str::<serde_json::Value>(&stdout) {
                Ok(v) => match v.get("root").and_then(|r| r.as_str()) {
                    Some(r) if Path::new(r) == named => Ok(()),
                    Some(r) => Err(format!("envelope says root={r:?}")),
                    None => Err("the envelope has no `root`".to_string()),
                },
                Err(e) => Err(format!("stdout is not a JSON report ({e}): {both:.400}")),
            },
            Tell::Writes(rel) => {
                let after = std::fs::read_to_string(corpus.join(rel)).unwrap_or_default();
                match after.contains(PLACEHOLDER) {
                    false => Ok(()),
                    true => Err(format!(
                        "{rel} in the corpus still holds the placeholder, so the block was \
                         written somewhere else or not at all: {both:.400}"
                    )),
                }
            }
            Tell::Names => match both.contains(&named.display().to_string()) {
                true => Ok(()),
                false => Err(format!("no mention of the directory it was given: {both:.400}")),
            },
        };
        if let Err(why) = verdict {
            failures.push(format!("{command}: {why}"));
        }
    }

    assert!(
        failures.is_empty(),
        "these commands accept `--root` and did not report on the corpus it named:\n  {}\n\
         The flag is declared in `main.rs` and honoured by threading the value to \
         `paths::resolve_root`; a command that declares it and resolves its own root reports \
         on whatever directory it was started in.",
        failures.join("\n  ")
    );
}
