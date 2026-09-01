//! Turn a nextest run into something a person can read without opening a log, and into
//! something a page can render.
//!
//! `$GITHUB_STEP_SUMMARY` appeared zero times in 1,527 lines of workflow (#462). 1,399 tests
//! reported as a colored dot, `cargo test` emitted nothing a consumer could parse, and the
//! four suites that skip on an absent environment printed an honest reason into scrollback
//! nobody opens.
//!
//! ```text
//! ci-report --gate "ci (cli)" --junit path/to/junit.xml --list path/to/list.json
//! ci-report merge --from artifacts/ --out quality-report.json
//! ```
//!
//! Markdown to stdout; a gate redirects it. `--json` additionally writes this gate's
//! `quality-report.json` fragment, which the merge mode joins into the one document #467's
//! pages read. Exits non-zero when the run it was pointed at contains no tests — see [`main`].

use anyhow::{bail, Result};
use ci_report::{census, coverage, junit, quality, summary};
use std::path::{Path, PathBuf};

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
    /// Where to write this gate's `quality-report.json` fragment, when a gate wants one.
    json: Option<PathBuf>,
}

/// Where the merge reads fragments and writes the report.
struct MergeArgs {
    from: PathBuf,
    out: PathBuf,
}

fn parse_args() -> Result<Args> {
    let mut gate = None;
    let mut junit = None;
    let mut list = None;
    let mut lcov = None;
    let mut diff = None;
    let mut src = None;
    let mut json = None;
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
            "--json" => json = Some(PathBuf::from(value()?)),
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
        json,
    })
}

fn parse_merge_args() -> Result<MergeArgs> {
    let mut from = None;
    let mut out = None;
    let mut it = std::env::args().skip(2);
    while let Some(arg) = it.next() {
        let mut value = || {
            it.next()
                .ok_or_else(|| anyhow::anyhow!("{arg} needs a value"))
        };
        match arg.as_str() {
            "--from" => from = Some(PathBuf::from(value()?)),
            "--out" => out = Some(PathBuf::from(value()?)),
            other => bail!("unknown argument `{other}`"),
        }
    }
    Ok(MergeArgs {
        from: from.ok_or_else(|| anyhow::anyhow!("merge needs --from <dir>"))?,
        out: out.ok_or_else(|| anyhow::anyhow!("merge needs --out <path>"))?,
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
    if std::env::args().nth(1).as_deref() == Some("merge") {
        return merge();
    }

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

    // Built once and rendered twice. The markdown a person reads and the JSON a page reads
    // describe one measurement; computing diff coverage at two call sites is how they would
    // come to disagree about a number nobody could then reconcile.
    let mut measured = None;
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
            .unwrap_or(Path::new("."));
        let absences = coverage::absences(src, repo_root, &lcov);
        let src_prefix = src.to_string_lossy().replace('\\', "/");
        let files = coverage::diff_coverage(&diff, &lcov, &absences, &src_prefix);
        print!("{}", summary::render_coverage(&args.features, &files));
        measured = Some(files);
    }

    if let Some(path) = &args.json {
        let provenance = quality::Provenance::read(&repo_root_of(&args.junit))?;
        let gate = quality::gate(
            &args.gate,
            &args.features,
            &run,
            &skips,
            measured
                .as_deref()
                .map(|files| quality::coverage(&args.features, files)),
        );
        write_json(path, &quality::fragment(&provenance, gate))?;
    }

    Ok(())
}

fn merge() -> Result<()> {
    let args = parse_merge_args()?;
    let fragments = quality::read_fragments(&args.from)?;
    let report = quality::merge(fragments)?;
    write_json(&args.out, &report)?;
    eprintln!(
        "{}: {} gate(s) at {}",
        args.out.display(),
        report.quality.gates.len(),
        report.yidam.commit
    );
    Ok(())
}

/// Pretty, and with a trailing newline. The file is read by a diff as often as by a program:
/// it is goldened, and a one-line document makes a review of it useless.
fn write_json<T: serde::Serialize>(path: &Path, value: &T) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut text = serde_json::to_string_pretty(value)?;
    text.push('\n');
    std::fs::write(path, text)?;
    Ok(())
}

/// The repository a path sits in, for reading provenance out of the tree.
///
/// Walks up from the JUnit document rather than from the process's working directory: the
/// gates invoke this from the repository root today, and a `dir =` on some future task would
/// otherwise relabel a fragment with whatever commit the parent directory happened to be on.
fn repo_root_of(path: &Path) -> PathBuf {
    // Absolute first. The gates pass a relative `--junit`, and a relative walk terminates at
    // the empty path — whose `.git` resolves against the working directory, so the search
    // "succeeds" and yields `""` as the root. An envelope whose `root` is the empty string is
    // the kind of quietly wrong field this contract exists to make impossible.
    let absolute = std::fs::canonicalize(path)
        .or_else(|_| std::env::current_dir().map(|cwd| cwd.join(path)))
        .unwrap_or_else(|_| path.to_path_buf());
    absolute
        .ancestors()
        .find(|a| a.join(".git").exists())
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
}
