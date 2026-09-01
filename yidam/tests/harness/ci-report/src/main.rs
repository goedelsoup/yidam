//! Turn a nextest run into something a person can read without opening a log.
//!
//! `$GITHUB_STEP_SUMMARY` appeared zero times in 1,527 lines of workflow (#462). 1,399 tests
//! reported as a colored dot, `cargo test` emitted nothing a consumer could parse, and the
//! four suites that skip on an absent environment printed an honest reason into scrollback
//! nobody opens.
//!
//! ```text
//! ci-report --gate "ci (cli)" --junit path/to/junit.xml --list path/to/list.json
//! ```
//!
//! Markdown to stdout; a gate redirects it. Exits non-zero when the run it was pointed at
//! contains no tests — see [`main`].

use anyhow::{bail, Result};
use ci_report::{census, junit, summary};
use std::path::PathBuf;

struct Args {
    gate: String,
    junit: PathBuf,
    list: Option<PathBuf>,
}

fn parse_args() -> Result<Args> {
    let mut gate = None;
    let mut junit = None;
    let mut list = None;
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        let mut value = || {
            it.next()
                .ok_or_else(|| anyhow::anyhow!("{arg} needs a value"))
        };
        match arg.as_str() {
            "--gate" => gate = Some(value()?),
            "--junit" => junit = Some(PathBuf::from(value()?)),
            "--list" => list = Some(PathBuf::from(value()?)),
            other => bail!("unknown argument `{other}`"),
        }
    }
    Ok(Args {
        gate: gate.unwrap_or_else(|| "tests".to_string()),
        junit: junit.ok_or_else(|| anyhow::anyhow!("--junit is required"))?,
        list,
    })
}

/// # Why an empty run is an error
///
/// A summary step that emits nothing and exits zero is the failure mode of this whole phase,
/// one level up: the gate stays green, the summary is blank, and blank is indistinguishable
/// from "everything passed" to anyone reading a colored dot. A wrong `--junit` path, a
/// profile that wrote somewhere else, a build that produced no test binaries — each of them
/// arrives here as a document with no cases.
///
/// So a run with no tests is refused rather than rendered. The one shape that legitimately
/// has no cases is a binary whose every test is `#[ignore]`d, and that is not what this is
/// ever pointed at: it reads a whole gate's run.
fn main() -> Result<()> {
    let args = parse_args()?;
    let run = junit::read(&args.junit)?;

    if run.cases.is_empty() {
        bail!(
            "{} contains no test cases. Refusing to publish a summary of nothing: a blank \
             summary on a green check is what #462 exists to remove, and an empty run here \
             means the tests did not run rather than that they passed.",
            args.junit.display()
        );
    }

    let mut skips = census::gated(&run);
    if let Some(list) = &args.list {
        skips.extend(census::ignored_from_file(list)?);
    }
    skips.sort_by(|a, b| a.test.cmp(&b.test));

    print!("{}", summary::render(&args.gate, &run, &skips));
    Ok(())
}
