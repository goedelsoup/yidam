//! `yidam policy` — compile the rules, ask one, and test them.
//!
//! # Why a policy needs a command at all
//!
//! A committed `.rego` file is code a gate runs, and without a way to compile it, ask it, and
//! test it, the only way to learn what a rule does is to trip it — which for the disclosure
//! family means finding out at the moment somebody is trying to publish something.
//!
//! `check` answers *what are the rules and where did they come from*, `eval` answers *what would
//! they say about this*, and `test` answers *do they still say what somebody decided they
//! should*. RFC-0024 argues each; none of them gates anything in this phase.
//!
//! # No repository required
//!
//! Every subcommand works against the compiled-in default when there is no `.yidam/policy/`,
//! and `eval` reads its input from a file or stdin. That is deliberate: a person working out
//! why a push was refused should be able to ask the question without a checkout, and a person
//! writing a rule should be able to try it before committing it.

use std::io::Read;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::Subcommand;

use crate::paths::repo_root;
use crate::policy::{Decision, Origin, Policies};
use crate::report::Format;

#[derive(Debug, Subcommand)]
pub enum PolicyCommand {
    /// Compile every rule and report which decision each comes from
    Check {
        #[arg(long, value_enum, default_value_t = Format::Text)]
        format: Format,
    },
    /// Ask one decision about one situation
    Eval {
        /// `family/name` — see `yidam policy check` for the set
        #[arg(long)]
        decision: String,
        /// The decision input, as JSON. Reads stdin when absent
        #[arg(long)]
        input: Option<PathBuf>,
        /// Name every rule that fired, not just the verdict
        #[arg(long)]
        explain: bool,
        #[arg(long, value_enum, default_value_t = Format::Text)]
        format: Format,
    },
    /// Run every `test_*` rule in every `*_test.rego`
    Test {
        #[arg(long, value_enum, default_value_t = Format::Text)]
        format: Format,
    },
}

pub fn run(sub: PolicyCommand) -> Result<()> {
    // `unwrap_or_else` rather than `require_yidam_repo`: the compiled-in default is a complete
    // rule set, so every subcommand has something to answer with outside a repository.
    let root = repo_root().unwrap_or_else(|_| PathBuf::from("."));
    match sub {
        PolicyCommand::Check { format } => check(&root, format),
        PolicyCommand::Eval {
            decision,
            input,
            explain,
            format,
        } => eval(&root, &decision, input.as_deref(), explain, format),
        PolicyCommand::Test { format } => test(&root, format),
    }
}

/// What the rules are, where each came from, and whether any of them names a builtin this build
/// does not carry.
fn check(root: &Path, format: Format) -> Result<()> {
    let policies = Policies::load(root)?;
    let disallowed = policies.disallowed_builtins()?;

    let overridden: Vec<(&str, &Origin)> =
        policies.origins().filter(|(_, o)| o.is_local()).collect();

    if format.is_json() {
        let rows: Vec<serde_json::Value> = policies
            .origins()
            .map(|(d, o)| {
                serde_json::json!({
                    "decision": d,
                    "origin": if o.is_local() { "local" } else { "inherited" },
                    "source": match o {
                        Origin::Local(p) => Some(p.to_string_lossy().to_string()),
                        Origin::Inherited => None,
                    },
                })
            })
            .collect();
        let out = serde_json::json!({
            "decisions": rows,
            "disallowed_builtins": disallowed.iter()
                .map(|(p, c)| serde_json::json!({"policy": p, "call": c}))
                .collect::<Vec<_>>(),
            "ok": disallowed.is_empty(),
        });
        println!("{}", serde_json::to_string_pretty(&out)?);
    } else {
        println!("Decisions");
        for (decision, origin) in policies.origins() {
            match origin {
                Origin::Inherited => println!("  {decision:<22} inherited"),
                Origin::Local(p) => println!("  {decision:<22} local     {}", p.display()),
            }
        }
        if overridden.is_empty() {
            println!("\nEvery decision is the one this binary shipped.");
        } else {
            // Named rather than merely counted, and not reported as a problem. A repository is
            // entitled to decide; #441 is what makes the decision visible elsewhere too.
            println!(
                "\n{} decision(s) are this repository's own and supersede the default.",
                overridden.len()
            );
            println!(
                "  `policy check` compares text. Whether a local rule is more permissive than\n  \
                 the one it replaced is a question about every possible input, and nothing here\n  \
                 claims to have answered it."
            );
        }
        if !disallowed.is_empty() {
            println!("\nBuiltins this build does not carry:");
            for (policy, call) in &disallowed {
                println!("  {policy}: {call}");
            }
        }
    }

    if !disallowed.is_empty() {
        anyhow::bail!(
            "{} call(s) to a builtin this binary does not compile in. They parse and would fail \
             at the moment a decision is needed, which for a gate is the worst available time",
            disallowed.len()
        );
    }
    Ok(())
}

fn eval(
    root: &Path,
    decision: &str,
    input: Option<&Path>,
    explain: bool,
    format: Format,
) -> Result<()> {
    let text = match input {
        Some(p) => {
            std::fs::read_to_string(p).with_context(|| format!("reading {}", p.display()))?
        }
        None => {
            let mut buf = String::new();
            std::io::stdin()
                .read_to_string(&mut buf)
                .context("reading the decision input from stdin")?;
            buf
        }
    };
    let json: serde_json::Value =
        serde_json::from_str(&text).context("the decision input is not valid JSON")?;

    let mut policies = Policies::load(root)?;
    let verdict = policies.decide(decision, &json)?;
    render(decision, &verdict, explain, format)
}

fn render(decision: &str, verdict: &Decision, explain: bool, format: Format) -> Result<()> {
    if format.is_json() {
        let out = serde_json::json!({
            "decision": decision,
            "allow": verdict.allow,
            "deny": verdict.deny.iter()
                .map(|d| serde_json::json!({"rule": d.rule, "msg": d.msg}))
                .collect::<Vec<_>>(),
        });
        println!("{}", serde_json::to_string_pretty(&out)?);
        return Ok(());
    }

    if verdict.allow {
        println!("allow  {decision}");
        return Ok(());
    }
    println!("refuse {decision}");
    for d in &verdict.deny {
        if explain {
            println!("  [{}] {}", d.rule, d.msg);
        } else {
            println!("  {}", d.msg);
        }
    }
    Ok(())
}

/// Every `test_*` rule in every `*_test.rego`, asserted true.
///
/// The format is Rego's own, so a rule written here is one an `opa test` reader recognises. A
/// test that evaluates to `Undefined` is a failure and not a skip: a rule whose body did not
/// hold asserted nothing, and reporting that as a pass is how a test suite comes to cover less
/// than it claims.
fn test(root: &Path, format: Format) -> Result<()> {
    let mut policies = Policies::load(root)?;
    let results = policies.run_tests()?;
    let failed: Vec<&crate::policy::TestOutcome> =
        results.iter().filter(|t| t.is_failure()).collect();
    let changed: Vec<&crate::policy::TestOutcome> = results
        .iter()
        .filter(|t| !t.passed && t.overridden)
        .collect();

    if format.is_json() {
        let out = serde_json::json!({
            "tests": results.iter().map(|t| serde_json::json!({
                "rule": t.rule,
                "passed": t.passed,
                "detail": t.detail,
                "covers": t.covers,
                "overridden": t.overridden,
            })).collect::<Vec<_>>(),
            "passed": results.iter().filter(|t| t.passed).count(),
            "failed": failed.len(),
            "changed_by_override": changed.len(),
        });
        println!("{}", serde_json::to_string_pretty(&out)?);
    } else if results.is_empty() {
        println!(
            "No policy tests. A rule nobody has written a case for is a rule nobody has checked."
        );
    } else {
        for t in &results {
            let short = t.rule.rsplit('.').next().unwrap_or(&t.rule);
            match (t.passed, t.overridden, &t.detail) {
                (true, _, _) => println!("  ok       {short}"),
                (false, true, _) => println!("  changed  {short}"),
                (false, false, Some(w)) => println!("  FAIL     {short} — {w}"),
                (false, false, None) => println!("  FAIL     {short}"),
            }
        }
        println!(
            "\n{} passed, {} failed, {} changed by an override",
            results.iter().filter(|t| t.passed).count(),
            failed.len(),
            changed.len()
        );
        if !changed.is_empty() {
            // Not a failure, and the most useful thing this command says. `policy check`
            // compares text and cannot tell which way a local rule moved; an inherited case
            // that no longer holds names exactly what the override changed.
            println!();
            println!("The `changed` cases are the inherited expectations your own rules no longer");
            println!("meet. That is the override being visible rather than a defect — but it is");
            println!("the list to read before deciding the override says what you meant.");
        }
    }

    if !failed.is_empty() {
        anyhow::bail!(
            "{} policy test(s) failed against a rule this repository did not override",
            failed.len()
        );
    }
    Ok(())
}
