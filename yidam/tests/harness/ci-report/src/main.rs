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
use ci_report::{census, coverage, junit, summary};
use std::path::PathBuf;

struct Args {
    gate: String,
    junit: PathBuf,
    list: Option<PathBuf>,
    /// LCOV from the same run. Optional: three of the four gates produce no coverage, and a
    /// summary without a coverage section is better than one that invents an empty table.
    lcov: Option<PathBuf>,
    /// Unified diff, `-U0`, of the change being graded.
    diff: Option<PathBuf>,
    /// Source root the coverage is about, for classifying what the build did not compile.
    src: Option<PathBuf>,
    /// The cargo features the measurement was taken under. Rendered verbatim, because a
    /// coverage number whose build is unstated is the number #464 is about.
    features: Vec<String>,
}

fn parse_args() -> Result<Args> {
    let mut gate = None;
    let mut junit = None;
    let mut list = None;
    let mut lcov = None;
    let mut diff = None;
    let mut src = None;
    let mut features = Vec::new();
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
            "--lcov" => lcov = Some(PathBuf::from(value()?)),
            "--diff" => diff = Some(PathBuf::from(value()?)),
            "--src" => src = Some(PathBuf::from(value()?)),
            "--features" => {
                features = value()?
                    .split(',')
                    .map(|f| f.trim().to_string())
                    .filter(|f| !f.is_empty())
                    .collect()
            }
            other => bail!("unknown argument `{other}`"),
        }
    }
    Ok(Args {
        gate: gate.unwrap_or_else(|| "tests".to_string()),
        junit: junit.ok_or_else(|| anyhow::anyhow!("--junit is required"))?,
        list,
        lcov,
        diff,
        src,
        features,
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

    if let Some(lcov_path) = &args.lcov {
        // Every input is required together. A coverage section built from an LCOV with no
        // diff would grade the whole repository against a pull request, and one with no
        // source root cannot tell a gated file from an untested one — which is the single
        // distinction the section exists to draw. Refusing beats rendering half of it.
        let (Some(diff_path), Some(src)) = (&args.diff, &args.src) else {
            bail!(
                "--lcov needs --diff and --src: without them the coverage section cannot \
                   tell an unmeasured file from an untested one, which is the whole of what \
                   it is for"
            );
        };
        if args.features.is_empty() {
            bail!(
                "--lcov needs --features: a coverage number whose build is unstated is the \
                   number #464 exists to remove"
            );
        }
        let lcov = coverage::read_lcov(lcov_path)?;
        let diff = std::fs::read_to_string(diff_path)?;
        let repo_root = src
            .ancestors()
            .find(|a| a.join(".git").exists())
            .unwrap_or(std::path::Path::new("."));
        let absences = coverage::absences(src, repo_root, &lcov);
        let src_prefix = src.to_string_lossy().replace('\\', "/");
        let files = coverage::diff_coverage(&diff, &lcov, &absences, &src_prefix);
        print!("{}", summary::render_coverage(&args.features, &files));
    }

    Ok(())
}
