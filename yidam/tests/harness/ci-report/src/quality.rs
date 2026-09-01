//! `quality-report.json` — the measurements, under the envelope this repository already has.
//!
//! #462 and #464 produced numbers; both are readable only by whoever opens a workflow log.
//! This is the contract that lets something else read them, and the shape is deliberately
//! **not** a new one: [`Envelope`] mirrors `yidam/cli/src/report.rs` field for field, so a
//! consumer that already reads a yidam report reads this without learning a second shape and
//! `format_version` governs it the way it governs the other four. `quality_report.rs` in the
//! CLI's test suite asserts the mirror holds rather than trusting this comment.
//!
//! # Why a fragment and a merge
//!
//! The gates run on separate runners and cannot see each other. Each writes a fragment — a
//! whole envelope carrying its own gate — and a later job merges them. The alternative, one
//! job re-running every suite to report on them, is the same tests twice.
//!
//! # `asserted` is the field the pages are about
//!
//! A runtime skip is recorded by the runner as a pass: the process ran and did not fail.
//! `passed` therefore includes every gated skip, and a suite that asserted nothing has the
//! same `passed` as a suite that asserted everything. [`Totals::asserted`] is `passed` minus
//! the skips among them — the count of tests that actually exercised their subject.
//!
//! It is a field rather than a template's arithmetic on purpose. "A fully-skipped suite must
//! not render as a passing one" is RFC-0025's warning about exactly the place it is easy to
//! lose, and a number that a page can only get wrong by ignoring it is harder to lose than a
//! subtraction a page has to remember to do.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

use crate::census::{Kind, Skip};
use crate::coverage::{Absence, FileDiff};
use crate::junit::Run;

/// The contract's major version.
///
/// Not a second opinion about the number: `quality_report.rs` requires this to equal
/// `report::FORMAT_VERSION` in the CLI. It is spelled here because this crate is in another
/// workspace and depending on the CLI to read one constant would drag the whole library into
/// every gate that wants a summary — the third of the three reasons this crate exists at all.
pub const FORMAT_VERSION: &str = "1";

/// Sections the report declares whether or not anything measured them.
///
/// An absent section reads as "nothing to say"; a section that says `measured: false` and why
/// reads as what it is. Same argument the census makes about a skip nobody counted, applied
/// to a whole page. #468 replaces these with data; until it does, the report states the gap
/// rather than leaving the page to infer it.
pub const SECTIONS: &[(&str, &str)] = &[(
    "mutation",
    "`cargo-mutants` runs on the weekly schedule, not on the run this report describes. Its \
     survivors are in that job's summary; a per-commit report cannot state them.",
)];

/// Sections a run *does* measure, and where a reader finds them.
///
/// `bench` was in [`SECTIONS`] until #468, saying "`yidam bench` prints prose and no
/// machine-readable series exists yet". The second half was true and the first was not:
/// `yidam bench --format json` has emitted through this same envelope since the command was
/// written. The claim was made in #467 by reading the task rather than running the command —
/// the mistake this epic keeps finding, made by the phase that was cataloguing it.
///
/// It is measured now: `bench_baseline.rs` ratchets `--scaling` against a committed baseline,
/// and the series carries the headline cost per push.
pub const MEASURED_SECTIONS: &[(&str, &str)] = &[(
    "bench",
    "Ratcheted by `bench_baseline.rs` against `yidam/cli/tests/goldens/bench/scaling.json`. \
     The series on the `quality-series` branch carries the headline cost per push.",
)];

// ── the envelope ─────────────────────────────────────────────────────────────

/// The report contract's envelope, mirroring `yidam/cli/src/report.rs`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Envelope {
    pub format_version: String,
    pub yidam: YidamBlock,
    /// Absolute path to the repository the report was computed over.
    pub root: String,
    pub quality: Quality,
}

/// Which yidam the measurements are about.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct YidamBlock {
    pub version: String,
    pub commit: String,
    /// The union across gates. **A number is qualified by its own gate's `features`, never by
    /// this** — the gates compile different sets, and `ci (cli)` measures coverage of a build
    /// that does not contain the index path at all. The union is here because the envelope
    /// requires the field; [`Gate::features`] is the one a reader should be shown.
    pub features: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Quality {
    pub gates: Vec<Gate>,
    pub sections: BTreeMap<String, Section>,
    /// What the run's jobs concluded, when the merge was able to ask (#516).
    ///
    /// `None` in a fragment, in a locally-merged report, and in every report written before
    /// #516 — which is why it is optional rather than a bump of `format_version`. Adding a
    /// field does not change what an existing field means, and the site *refuses* a version
    /// it does not know: bumping would blank the quality pages until the next successful run
    /// on main replaced the report they read.
    #[serde(default)]
    pub run: Option<RunJobs>,
}

/// The jobs of one CI run, as they stood when the report was assembled.
///
/// # Why the report needs this at all
///
/// A gate's numbers come from JUnit XML, and JUnit describes test cases. A job that fails
/// outside its tests — `fmt --check`, `clippy -D warnings`, a coverage step, a packaging
/// check — is invisible to every number in this document. On run 33527632095 the report said
/// `failed: 0` while `ci (cli · full features)` was red, and a reader of /quality/ would have
/// been shown a clean bill of health for a broken main.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunJobs {
    /// Jobs that had finished when the report was assembled.
    pub jobs: Vec<Job>,
    /// Jobs that had not. The reporting job is always among them — it is running this code —
    /// and so is anything sequenced after it. Named rather than counted so that a reader can
    /// tell "not finished" from "not configured".
    pub pending: Vec<String>,
}

/// One job of a run, and what it concluded.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Job {
    /// As the run displays it, e.g. `ci (cli · full features)`. Matched against
    /// [`Gate::job`], falling back to [`Gate::gate`] when a gate did not name one.
    pub name: String,
    /// GitHub's own word: `success`, `failure`, `cancelled`, `skipped`, `neutral`, …
    pub conclusion: String,
}

impl RunJobs {
    /// The jobs a reader should be told about: every one that did not succeed or skip.
    ///
    /// An allow-list rather than a deny-list of `failure`. GitHub has added conclusions
    /// before (`stale`, `timed_out`, `action_required`) and a deny-list would silently call
    /// each new one fine — the exact shape of defect this whole surface exists to remove.
    pub fn unsuccessful(&self) -> Vec<&Job> {
        self.jobs
            .iter()
            .filter(|j| !matches!(j.conclusion.as_str(), "success" | "skipped" | "neutral"))
            .collect()
    }
}

/// One CI job's run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Gate {
    /// The job name a reader sees, e.g. `ci (cli)`.
    pub gate: String,
    /// The cargo features this gate compiled. Empty for a gate that is not a cargo build.
    pub features: Vec<String>,
    pub totals: Totals,
    pub suites: Vec<Suite>,
    pub skipped: Vec<SkipRecord>,
    /// `None` when this gate produced no LCOV — three of the five do not.
    pub coverage: Option<Coverage>,
    /// The CI job this gate's numbers came from, when it is not the gate's own name (#516).
    ///
    /// Usually they are the same string and this is `None`. They are not always the same
    /// thing: `ci (parity)` runs the Rust, TypeScript and Python parity arms and summarises
    /// only the Rust one, so its gate is `ci (parity · rust sdk)` — a heading that is more
    /// truthful than the job name, and would never match it. Matching on the job name alone
    /// left that gate's conclusion permanently unknown; `the_gate_names_name_real_jobs` is
    /// what keeps the two in step.
    #[serde(default)]
    pub job: Option<String>,
    /// What the job that produced this fragment concluded (#516).
    ///
    /// Always `None` in the fragment itself: a gate cannot know its own outcome while it is
    /// still running to write this. [`merge`] stamps it from the run's job list. `None` in
    /// the merged report therefore means "nobody could say", which the pages render as its
    /// own state — never as a pass.
    #[serde(default)]
    pub conclusion: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Totals {
    /// `<testcase>` elements the runner wrote.
    pub cases: usize,
    pub failed: usize,
    /// `cases - failed`. Includes every gated skip, because the runner cannot tell them apart.
    pub passed: usize,
    /// The census: gated plus ignored.
    pub skipped: usize,
    /// Skips that are among `cases` — they ran, announced a reason, and asserted nothing.
    pub gated: usize,
    /// Skips that are **not** among `cases`. nextest omits `#[ignore]`d tests from JUnit
    /// entirely, so these are counted from `nextest list` and are invisible to the XML.
    pub ignored: usize,
    /// `passed - gated`: tests that actually exercised their subject. See the module header.
    pub asserted: usize,
}

/// One test binary's cases. `asserted == 0` with `cases > 0` is the fully-skipped suite.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Suite {
    pub suite: String,
    pub totals: Totals,
    pub tests: Vec<Test>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Test {
    pub name: String,
    pub status: Status,
    /// Seconds. `None` when the runner reported no timing — which is not the same as zero.
    pub seconds: Option<f64>,
    /// Present on a failure, truncated by the producer. The JUnit artifact carries the whole.
    pub failure: Option<String>,
    /// Present when the test announced a skip. A `Status::Skipped` test always has one.
    pub skip_reason: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Passed,
    Failed,
    /// Ran and asserted nothing, or was never started. `kind` on the [`SkipRecord`] says which.
    Skipped,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkipRecord {
    pub suite: String,
    pub test: String,
    pub reason: String,
    /// `gated` or `ignored`.
    pub kind: String,
}

/// Diff coverage, and what the measured build could not see.
///
/// The two lists are never merged, for the reason `summary::render_coverage` gives: uncovered
/// lines are lines a test could have executed and did not; unmeasured files were not compiled
/// into the build that produced the LCOV. Adding them together is the number #464 exists to
/// prevent.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Coverage {
    /// What the measurement was taken under. Not decorative: a page that cannot show this
    /// cannot tell a thorough run from a narrow one.
    pub features: Vec<String>,
    /// Added lines in files this build compiled.
    pub added: usize,
    pub uncovered: usize,
    pub files: Vec<CoverageFile>,
    pub unmeasured: Vec<Unmeasured>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoverageFile {
    pub path: String,
    pub added: usize,
    /// Line numbers, so a page can link to them. Empty means fully covered.
    pub uncovered: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Unmeasured {
    pub path: String,
    pub added: usize,
    /// `gated`, `test-only`, `no-coverable-code`, or `unexplained`. Only the last is a
    /// finding; the page must not draw the others as a gap.
    pub reason: String,
    /// The feature the file sits behind, when `reason` is `gated`.
    pub feature: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Section {
    pub measured: bool,
    /// Why not, when `measured` is false. A page renders this instead of a zero.
    pub why: String,
}

// ── provenance ───────────────────────────────────────────────────────────────

/// Which yidam, and where. Read from the tree rather than passed in by a workflow, so a
/// fragment cannot be labelled with a commit the runner was not on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Provenance {
    pub version: String,
    pub commit: String,
    pub root: String,
}

impl Provenance {
    /// `version` from the CLI's manifest, `commit` from git, `root` from the path given.
    ///
    /// `unknown` for a commit git cannot answer for, never a guess — the same rule the CLI's
    /// `build.rs` follows, and for the same reason: "not recorded" and "absent" must stay
    /// distinguishable.
    pub fn read(repo_root: &Path) -> Result<Self> {
        let manifest = repo_root.join("yidam/cli/Cargo.toml");
        let text = std::fs::read_to_string(&manifest)
            .with_context(|| format!("reading {}", manifest.display()))?;
        let version = cargo_version(&text)
            .with_context(|| format!("no [package] version in {}", manifest.display()))?;
        Ok(Self {
            version,
            commit: git_commit(repo_root),
            root: repo_root.display().to_string(),
        })
    }
}

/// The `version` of the first `[package]` table. Written by hand rather than by parsing TOML:
/// a `toml` dependency for one field, in a crate whose manifest argues for four, is not worth
/// it — and the shape it reads is asserted by a test.
fn cargo_version(manifest: &str) -> Option<String> {
    let mut in_package = false;
    for line in manifest.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_package = line == "[package]";
            continue;
        }
        if !in_package {
            continue;
        }
        if let Some(rest) = line.strip_prefix("version") {
            let rest = rest.trim_start().strip_prefix('=')?.trim();
            return Some(rest.trim_matches('"').to_string());
        }
    }
    None
}

fn git_commit(repo_root: &Path) -> String {
    std::process::Command::new("git")
        .current_dir(repo_root)
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".to_string())
}

// ── building a fragment ──────────────────────────────────────────────────────

/// The suite half of a skip's identifier.
///
/// Both censuses spell a skip as `"{suite} {name}"` — `junit::Case::id` and
/// `census::ignored` build it the same way — and suite names are `crate::binary`, which
/// carry no space. The split is therefore exact rather than heuristic; a suite name that
/// grew a space would show up as a skip attributed to a suite that has no other tests,
/// which `skips_land_on_their_own_suite` is there to catch.
fn split_id(id: &str) -> (String, String) {
    match id.split_once(' ') {
        Some((suite, name)) => (suite.to_string(), name.to_string()),
        None => (String::new(), id.to_string()),
    }
}

/// One gate's contribution, from the same inputs the markdown summary is built from.
///
/// `coverage` is threaded through rather than recomputed: the JSON and the job summary must
/// describe one measurement, and two call sites computing diff coverage independently is how
/// they would come to disagree.
pub fn gate(
    name: &str,
    job: Option<&str>,
    features: &[String],
    run: &Run,
    skips: &[Skip],
    coverage: Option<Coverage>,
) -> Gate {
    let by_id: BTreeMap<String, &Skip> = skips.iter().map(|s| (s.test.clone(), s)).collect();

    // Suites in the order the runner reported them, deduplicated. A BTreeMap would sort
    // `yidam::a` before `yidam::b`, which is also stable — but the ignored census contributes
    // suites that have no cases at all, and those must appear too.
    let mut suites: BTreeMap<String, Vec<Test>> = BTreeMap::new();
    for case in &run.cases {
        let skip = by_id.get(&case.id());
        let status = if case.failure.is_some() {
            Status::Failed
        } else if skip.is_some() {
            Status::Skipped
        } else {
            Status::Passed
        };
        suites.entry(case.suite.clone()).or_default().push(Test {
            name: case.name.clone(),
            status,
            seconds: case.time,
            failure: case.failure.clone(),
            skip_reason: skip.map(|s| s.reason.clone()),
        });
    }

    // The ignored ones, which are in no JUnit document. A suite whose every test is
    // `#[ignore]`d has `tests="0"` and no children, so without this it is a suite that does
    // not appear — the exact shape of "a suite that ran nothing looks like a suite that
    // passed", one level up from the one the census fixed.
    let mut ignored_ids: BTreeSet<String> = BTreeSet::new();
    for skip in skips.iter().filter(|s| s.kind == Kind::Ignored) {
        let (suite, name) = split_id(&skip.test);
        ignored_ids.insert(skip.test.clone());
        suites.entry(suite).or_default().push(Test {
            name,
            status: Status::Skipped,
            seconds: None,
            failure: None,
            skip_reason: Some(skip.reason.clone()),
        });
    }

    let suites: Vec<Suite> = suites
        .into_iter()
        .map(|(suite, mut tests)| {
            tests.sort_by(|a, b| a.name.cmp(&b.name));
            let totals = totals_of(&tests, &ignored_ids, &suite);
            Suite {
                suite,
                totals,
                tests,
            }
        })
        .collect();

    let totals = suites
        .iter()
        .map(|s| s.totals)
        .fold(Totals::zero(), Totals::plus);

    let mut skipped: Vec<SkipRecord> = skips
        .iter()
        .map(|s| {
            let (suite, test) = split_id(&s.test);
            SkipRecord {
                suite,
                test,
                reason: s.reason.clone(),
                kind: s.kind.label().to_string(),
            }
        })
        .collect();
    skipped.sort_by(|a, b| (&a.suite, &a.test).cmp(&(&b.suite, &b.test)));

    Gate {
        gate: name.to_string(),
        features: features.to_vec(),
        totals,
        suites,
        skipped,
        coverage,
        job: job.map(str::to_string),
        // A gate writing its own fragment is, by definition, still running.
        conclusion: None,
    }
}

fn totals_of(tests: &[Test], ignored_ids: &BTreeSet<String>, suite: &str) -> Totals {
    let failed = tests.iter().filter(|t| t.status == Status::Failed).count();
    let skipped = tests.iter().filter(|t| t.status == Status::Skipped).count();
    let ignored = tests
        .iter()
        .filter(|t| ignored_ids.contains(&format!("{suite} {}", t.name)))
        .count();
    let gated = skipped - ignored;
    // `cases` is what the runner wrote, so the ignored ones — which it did not — are not in
    // it. Counting them would make `passed + failed != cases` and quietly invent runs.
    let cases = tests.len() - ignored;
    let passed = cases - failed;
    Totals {
        cases,
        failed,
        passed,
        skipped,
        gated,
        ignored,
        asserted: passed - gated,
    }
}

impl Totals {
    /// The identity for [`Totals::plus`], so a fold over gates needs no special case.
    pub fn zero() -> Self {
        Self {
            cases: 0,
            failed: 0,
            passed: 0,
            skipped: 0,
            gated: 0,
            ignored: 0,
            asserted: 0,
        }
    }

    /// Componentwise. `asserted` adds like the rest: it is a count of tests, not a ratio.
    pub fn plus(self, other: Self) -> Self {
        Self {
            cases: self.cases + other.cases,
            failed: self.failed + other.failed,
            passed: self.passed + other.passed,
            skipped: self.skipped + other.skipped,
            gated: self.gated + other.gated,
            ignored: self.ignored + other.ignored,
            asserted: self.asserted + other.asserted,
        }
    }
}

/// The coverage block, from the same `FileDiff`s the markdown section renders.
pub fn coverage(features: &[String], files: &[FileDiff]) -> Coverage {
    let mut measured = Vec::new();
    let mut unmeasured = Vec::new();
    for f in files {
        match &f.uncovered {
            Some(lines) => measured.push(CoverageFile {
                path: f.path.clone(),
                added: f.added.len(),
                uncovered: lines.clone(),
            }),
            None => {
                let (reason, feature) = match &f.absence {
                    Some(Absence::Gated(feature)) => ("gated", Some(feature.clone())),
                    Some(Absence::TestOnly) => ("test-only", None),
                    Some(Absence::NoCoverableCode) => ("no-coverable-code", None),
                    Some(Absence::Unexplained) | None => ("unexplained", None),
                };
                unmeasured.push(Unmeasured {
                    path: f.path.clone(),
                    added: f.added.len(),
                    reason: reason.to_string(),
                    feature,
                });
            }
        }
    }
    Coverage {
        features: features.to_vec(),
        added: measured.iter().map(|f| f.added).sum(),
        uncovered: measured.iter().map(|f| f.uncovered.len()).sum(),
        files: measured,
        unmeasured,
    }
}

/// One gate's whole document.
pub fn fragment(provenance: &Provenance, gate: Gate) -> Envelope {
    Envelope {
        format_version: FORMAT_VERSION.to_string(),
        yidam: YidamBlock {
            version: provenance.version.clone(),
            commit: provenance.commit.clone(),
            features: gate.features.clone(),
        },
        root: provenance.root.clone(),
        quality: Quality {
            gates: vec![gate],
            sections: sections(),
            // Likewise: one runner cannot see the others. `merge` fills this in.
            run: None,
        },
    }
}

fn sections() -> BTreeMap<String, Section> {
    let unmeasured = SECTIONS.iter().map(|(name, why)| {
        (
            name.to_string(),
            Section {
                measured: false,
                why: why.to_string(),
            },
        )
    });
    let measured = MEASURED_SECTIONS.iter().map(|(name, why)| {
        (
            name.to_string(),
            Section {
                measured: true,
                why: why.to_string(),
            },
        )
    });
    unmeasured.chain(measured).collect()
}

// ── merging ──────────────────────────────────────────────────────────────────

/// Every gate's fragment into one report.
///
/// # Why a commit mismatch is fatal
///
/// The fragments come from separate runners and are joined by nothing but the artifact store.
/// A report whose gates were measured at different commits is not a report about a commit,
/// and it would look exactly like one: the page would draw a coverage bar from one tree
/// beside a test count from another. Refusing beats rendering it.
pub fn merge(fragments: Vec<Envelope>, run: Option<RunJobs>) -> Result<Envelope> {
    let Some(first) = fragments.first().cloned() else {
        bail!(
            "no fragments to merge. Every gate that runs tests writes one, so an empty set \
             means they were not uploaded or not downloaded — and a quality report with no \
             gates would publish as a page saying nothing failed."
        );
    };

    for f in &fragments {
        if f.format_version != first.format_version {
            bail!(
                "fragments disagree about format_version: {} and {}",
                first.format_version,
                f.format_version
            );
        }
        if f.yidam.commit != first.yidam.commit {
            bail!(
                "fragments were measured at different commits ({} and {}). They are not one \
                 run, and merging them would put one tree's coverage beside another tree's \
                 test count.",
                first.yidam.commit,
                f.yidam.commit
            );
        }
    }

    let mut gates: Vec<Gate> = fragments
        .iter()
        .flat_map(|f| f.quality.gates.clone())
        .collect();
    gates.sort_by(|a, b| a.gate.cmp(&b.gate));

    // The conclusion a fragment could not know when it was written, matched by the job name
    // the run displays. A gate whose heading is not a job name says which job it came from;
    // `the_gate_names_name_real_jobs` is what keeps those names real rather than this comment.
    if let Some(run) = &run {
        for gate in &mut gates {
            let job = gate.job.clone().unwrap_or_else(|| gate.gate.clone());
            gate.conclusion = run
                .jobs
                .iter()
                .find(|j| j.name == job)
                .map(|j| j.conclusion.clone());
        }
    }

    let features: BTreeSet<String> = gates.iter().flat_map(|g| g.features.clone()).collect();

    // Every section every fragment carried, with the strongest claim winning: a section one
    // gate measured is measured, whatever the gates that did not.
    let mut sections: BTreeMap<String, Section> = BTreeMap::new();
    for f in &fragments {
        for (name, section) in &f.quality.sections {
            let entry = sections
                .entry(name.clone())
                .or_insert_with(|| section.clone());
            if section.measured && !entry.measured {
                *entry = section.clone();
            }
        }
    }

    Ok(Envelope {
        format_version: first.format_version,
        yidam: YidamBlock {
            version: first.yidam.version,
            commit: first.yidam.commit,
            features: features.into_iter().collect(),
        },
        root: first.root,
        quality: Quality {
            gates,
            sections,
            run,
        },
    })
}

/// GitHub's job list for one run, as `gh api /repos/{repo}/actions/runs/{id}/jobs` returns it.
///
/// # Why this refuses a truncated page
///
/// The endpoint paginates at 30 by default and reports the true count in `total_count`. A
/// silently short page would omit jobs, and the jobs it omits are as likely to be the failed
/// ones as any other — a report that understates failures is the defect #516 is about, so a
/// count that does not match is an error rather than a shrug.
pub fn parse_jobs(json: &str) -> Result<RunJobs> {
    #[derive(Deserialize)]
    struct Response {
        total_count: usize,
        jobs: Vec<Entry>,
    }
    #[derive(Deserialize)]
    struct Entry {
        name: String,
        /// `null` until the job finishes.
        conclusion: Option<String>,
    }

    let response: Response =
        serde_json::from_str(json).context("parsing the run's job list as GitHub returns it")?;
    if response.total_count != response.jobs.len() {
        bail!(
            "the job list is truncated: {} jobs on this page, {} in the run. Ask for them all \
             (`per_page=100`, and paginate above that) — a short page hides jobs, and a hidden \
             job is as likely to be a failed one as not.",
            response.jobs.len(),
            response.total_count
        );
    }

    let mut jobs = Vec::new();
    let mut pending = Vec::new();
    for entry in response.jobs {
        match entry.conclusion {
            Some(conclusion) => jobs.push(Job {
                name: entry.name,
                conclusion,
            }),
            None => pending.push(entry.name),
        }
    }
    jobs.sort_by(|a, b| a.name.cmp(&b.name));
    pending.sort();
    Ok(RunJobs { jobs, pending })
}

/// Read every `*.json` under `dir`, recursively.
///
/// Recursive because `actions/download-artifact` with a pattern unpacks each artifact into a
/// directory of its own. A flat read would find nothing and [`merge`] would refuse — loudly,
/// which is the right failure, but one directory level is not worth a red build.
pub fn read_fragments(dir: &Path) -> Result<Vec<Envelope>> {
    let mut paths = Vec::new();
    collect_json(dir, &mut paths)?;
    paths.sort();
    let mut out = Vec::new();
    for path in paths {
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("reading {}", path.display()))?;
        let envelope: Envelope = serde_json::from_str(&text)
            .with_context(|| format!("parsing {} as a quality-report fragment", path.display()))?;
        out.push(envelope);
    }
    Ok(out)
}

fn collect_json(dir: &Path, out: &mut Vec<std::path::PathBuf>) -> Result<()> {
    let entries = std::fs::read_dir(dir).with_context(|| format!("reading {}", dir.display()))?;
    for entry in entries {
        let path = entry?.path();
        if path.is_dir() {
            collect_json(&path, out)?;
        } else if path.extension().is_some_and(|e| e == "json") {
            out.push(path);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::junit;

    fn run_of(xml: &str) -> Run {
        junit::parse(xml).expect("fixture should parse")
    }

    fn provenance() -> Provenance {
        Provenance {
            version: "0.7.0".into(),
            commit: "abc1234".into(),
            root: "/repo".into(),
        }
    }

    const MIXED: &str = r#"<testsuites><testsuite name="s">
        <testcase name="asserts" classname="yidam::a" time="0.5"><system-out>ok</system-out></testcase>
        <testcase name="breaks" classname="yidam::a" time="2.5">
          <failure message="left != right">panicked</failure>
        </testcase>
        <testcase name="declines" classname="yidam::b" time="0.1">
          <system-out>YIDAM-SKIP: set YIDAM_S3_TEST=1</system-out>
        </testcase>
      </testsuite></testsuites>"#;

    fn mixed_gate() -> Gate {
        let run = run_of(MIXED);
        let skips = crate::census::gated(&run);
        gate("ci (cli)", None, &["reports".into()], &run, &skips, None)
    }

    /// The whole point of the field, and the fixture RFC-0025 warns is easy to lose.
    ///
    /// `yidam::b` ran one test, exited zero, and asserted nothing. Every count a runner
    /// produces calls that a pass.
    #[test]
    fn a_fully_skipped_suite_asserts_nothing_even_though_it_passed() {
        let g = mixed_gate();
        let b = g
            .suites
            .iter()
            .find(|s| s.suite == "yidam::b")
            .expect("the skipped suite is missing from the report entirely");
        assert_eq!(b.totals.passed, 1, "the runner recorded it as a pass");
        assert_eq!(b.totals.failed, 0);
        assert_eq!(
            b.totals.asserted, 0,
            "a suite that only announced a skip must not report an assertion"
        );
        assert_eq!(b.totals.gated, 1);
        assert_eq!(b.tests[0].status, Status::Skipped);
        assert_eq!(
            b.tests[0].skip_reason.as_deref(),
            Some("set YIDAM_S3_TEST=1"),
            "the reason is the whole value of counting the skip"
        );
    }

    /// An empty suite is not a passing suite either, and it is the one a `<testsuite>` with
    /// no children produces.
    #[test]
    fn a_run_with_no_cases_totals_nothing_rather_than_passing() {
        let run = run_of(r#"<testsuites><testsuite name="s"></testsuite></testsuites>"#);
        let g = gate("ci (empty)", None, &[], &run, &[], None);
        assert_eq!(g.totals.cases, 0);
        assert_eq!(g.totals.passed, 0);
        assert_eq!(g.totals.asserted, 0);
        assert!(g.suites.is_empty());
    }

    /// `#[ignore]`d tests are in no JUnit document, so a suite made entirely of them exists
    /// only in the listing. It must still be a suite in the report.
    #[test]
    fn an_ignored_only_suite_appears_with_no_cases() {
        let skips = vec![Skip {
            test: "yidam::slow a".into(),
            reason: "#[ignore] — see the test's own note".into(),
            kind: Kind::Ignored,
        }];
        let g = gate(
            "ci (cli)",
            None,
            &[],
            &run_of("<testsuites></testsuites>"),
            &skips,
            None,
        );
        let s = g
            .suites
            .iter()
            .find(|s| s.suite == "yidam::slow")
            .expect("an ignored-only suite vanished from the report");
        assert_eq!(
            s.totals.cases, 0,
            "an ignored test is not a case the runner ran"
        );
        assert_eq!(s.totals.ignored, 1);
        assert_eq!(s.totals.asserted, 0);
        assert_eq!(s.tests.len(), 1);
    }

    #[test]
    fn totals_add_up_across_suites() {
        let g = mixed_gate();
        assert_eq!(g.totals.cases, 3);
        assert_eq!(g.totals.failed, 1);
        assert_eq!(g.totals.passed, 2);
        assert_eq!(g.totals.skipped, 1);
        assert_eq!(g.totals.asserted, 1, "one test actually exercised anything");
    }

    /// A skip is attributed to the suite it came from, not to a suite of its own.
    #[test]
    fn skips_land_on_their_own_suite() {
        let g = mixed_gate();
        assert_eq!(g.skipped.len(), 1);
        assert_eq!(g.skipped[0].suite, "yidam::b");
        assert_eq!(g.skipped[0].test, "declines");
        assert_eq!(g.skipped[0].kind, "gated");
    }

    #[test]
    fn a_failure_carries_its_message() {
        let g = mixed_gate();
        let a = g.suites.iter().find(|s| s.suite == "yidam::a").unwrap();
        let broken = a.tests.iter().find(|t| t.name == "breaks").unwrap();
        assert_eq!(broken.status, Status::Failed);
        assert!(broken.failure.as_deref().unwrap().contains("panicked"));
    }

    // ── coverage ─────────────────────────────────────────────────────────────

    /// The assertion that decides whether the number is honest, in the JSON this time: a
    /// gated file contributes to `unmeasured` and to neither `added` nor `uncovered`.
    #[test]
    fn a_gated_file_is_unmeasured_and_not_uncovered() {
        let files = vec![
            FileDiff {
                path: "yidam/cli/src/parse.rs".into(),
                added: vec![10, 11],
                uncovered: Some(vec![11]),
                absence: None,
            },
            FileDiff {
                path: "yidam/cli/src/embedding.rs".into(),
                added: (1..=40).collect(),
                uncovered: None,
                absence: Some(Absence::Gated("vector-read".into())),
            },
        ];
        let c = coverage(&["reports".into()], &files);
        assert_eq!(c.added, 2, "gated lines entered the denominator");
        assert_eq!(c.uncovered, 1);
        assert_eq!(c.files.len(), 1);
        assert_eq!(c.unmeasured.len(), 1);
        assert_eq!(c.unmeasured[0].reason, "gated");
        assert_eq!(c.unmeasured[0].feature.as_deref(), Some("vector-read"));
    }

    #[test]
    fn an_unexplained_absence_keeps_its_name() {
        let files = vec![FileDiff {
            path: "yidam/cli/src/orphan.rs".into(),
            added: vec![1],
            uncovered: None,
            absence: Some(Absence::Unexplained),
        }];
        assert_eq!(coverage(&[], &files).unmeasured[0].reason, "unexplained");
    }

    // ── the envelope and the merge ───────────────────────────────────────────

    #[test]
    fn a_fragment_carries_the_envelope_and_one_gate() {
        let e = fragment(&provenance(), mixed_gate());
        assert_eq!(e.format_version, FORMAT_VERSION);
        assert_eq!(e.yidam.commit, "abc1234");
        assert_eq!(e.yidam.features, vec!["reports".to_string()]);
        assert_eq!(e.root, "/repo");
        assert_eq!(e.quality.gates.len(), 1);
    }

    /// Every section is declared, measured or not, and every one says where it stands.
    ///
    /// An absent section reads as "nothing to say"; this reads as what it is. Both halves
    /// carry a `why` — a measured section that could not say where its numbers are would send
    /// a reader looking for a chart this report does not hold.
    #[test]
    fn every_section_is_declared_with_its_standing_and_its_reason() {
        let e = fragment(&provenance(), mixed_gate());
        assert_eq!(
            e.quality.sections.len(),
            SECTIONS.len() + MEASURED_SECTIONS.len(),
            "a section is declared twice, or one name appears under both standings"
        );
        for (name, expected) in SECTIONS
            .iter()
            .map(|(n, _)| (n, false))
            .chain(MEASURED_SECTIONS.iter().map(|(n, _)| (n, true)))
        {
            let s = e
                .quality
                .sections
                .get(*name)
                .unwrap_or_else(|| panic!("no `{name}` section"));
            assert_eq!(
                s.measured, expected,
                "`{name}` is declared under the wrong standing"
            );
            assert!(!s.why.is_empty(), "`{name}` states a standing and not why");
        }
    }

    #[test]
    fn merging_orders_gates_and_unions_features() {
        let cli = fragment(
            &provenance(),
            gate(
                "ci (cli)",
                None,
                &["reports".into(), "tonpa".into()],
                &run_of(MIXED),
                &[],
                None,
            ),
        );
        let harness = fragment(
            &provenance(),
            gate("ci (harness)", None, &[], &run_of(MIXED), &[], None),
        );
        let merged = merge(vec![cli, harness], None).expect("two fragments of one run");
        assert_eq!(
            merged
                .quality
                .gates
                .iter()
                .map(|g| g.gate.as_str())
                .collect::<Vec<_>>(),
            vec!["ci (cli)", "ci (harness)"]
        );
        assert_eq!(merged.yidam.features, vec!["reports", "tonpa"]);
        assert_eq!(
            merged.quality.gates[1].features,
            Vec::<String>::new(),
            "the union must not be written back onto a gate that compiled none of it"
        );
    }

    /// The failure that would otherwise be invisible: two runs' numbers on one page.
    #[test]
    fn fragments_from_different_commits_are_refused() {
        let a = fragment(&provenance(), mixed_gate());
        let b = fragment(
            &Provenance {
                commit: "def5678".into(),
                ..provenance()
            },
            mixed_gate(),
        );
        let err = merge(vec![a, b], None).expect_err("a mixed-commit merge must fail");
        assert!(format!("{err}").contains("different commits"), "{err}");
    }

    /// An empty merge is refused rather than published, for the reason `main` refuses an
    /// empty run: a report with no gates renders as a page on which nothing failed.
    #[test]
    fn merging_nothing_is_an_error() {
        let err = merge(Vec::new(), None).expect_err("an empty merge must fail");
        assert!(format!("{err}").contains("no fragments"), "{err}");
    }

    // ── the run's job conclusions (#516) ─────────────────────────────────────

    const JOBS: &str = r#"{
      "total_count": 3,
      "jobs": [
        { "name": "ci (cli)", "conclusion": "success" },
        { "name": "ci (harness)", "conclusion": "failure" },
        { "name": "ci (quality report)", "conclusion": null }
      ]
    }"#;

    /// A finished job carries its word; an unfinished one is named as pending, not guessed.
    #[test]
    fn the_job_list_separates_what_concluded_from_what_is_still_running() {
        let run = parse_jobs(JOBS).expect("GitHub's own shape");
        assert_eq!(
            run.jobs
                .iter()
                .map(|j| (j.name.as_str(), j.conclusion.as_str()))
                .collect::<Vec<_>>(),
            vec![("ci (cli)", "success"), ("ci (harness)", "failure")]
        );
        assert_eq!(run.pending, vec!["ci (quality report)"]);
    }

    /// A short page is refused rather than quietly under-reporting.
    ///
    /// The endpoint paginates at 30. A truncated list drops jobs, and a dropped job is as
    /// likely to be a failed one as any other — which would reproduce the exact defect this
    /// field exists to fix, in the code that fixes it.
    #[test]
    fn a_truncated_job_list_is_refused() {
        let json =
            r#"{ "total_count": 14, "jobs": [ { "name": "ci (cli)", "conclusion": "success" } ] }"#;
        let err = parse_jobs(json).expect_err("a truncated page must not be accepted");
        assert!(format!("{err}").contains("truncated"), "{err}");
    }

    /// Anything that is not a success or a deliberate skip is something a reader is told.
    ///
    /// Written as an allow-list. `timed_out` is a real GitHub conclusion and a deny-list of
    /// `failure` would call it fine; so would whatever conclusion GitHub adds next.
    #[test]
    fn an_unrecognised_conclusion_counts_as_unsuccessful() {
        let run = RunJobs {
            jobs: vec![
                Job {
                    name: "green".into(),
                    conclusion: "success".into(),
                },
                Job {
                    name: "not run".into(),
                    conclusion: "skipped".into(),
                },
                Job {
                    name: "slow".into(),
                    conclusion: "timed_out".into(),
                },
                Job {
                    name: "stopped".into(),
                    conclusion: "cancelled".into(),
                },
            ],
            pending: Vec::new(),
        };
        assert_eq!(
            run.unsuccessful()
                .iter()
                .map(|j| j.name.as_str())
                .collect::<Vec<_>>(),
            vec!["slow", "stopped"],
            "a conclusion this code has never heard of must be surfaced, not assumed benign"
        );
    }

    /// The merge stamps each gate with its own job's conclusion, and invents none.
    ///
    /// The stamping is by the displayed job name, which is the same string the composite
    /// action is handed as `gate`. A gate with no matching job stays `None` — "nobody could
    /// say" — because the alternative is a page that renders an unknown as a pass.
    #[test]
    fn a_gate_is_stamped_with_its_own_jobs_conclusion() {
        let cli = fragment(
            &provenance(),
            gate("ci (cli)", None, &[], &run_of(MIXED), &[], None),
        );
        let harness = fragment(
            &provenance(),
            gate("ci (harness)", None, &[], &run_of(MIXED), &[], None),
        );
        let other = fragment(
            &provenance(),
            gate("ci (unlisted)", None, &[], &run_of(MIXED), &[], None),
        );

        let merged = merge(
            vec![cli, harness, other],
            Some(parse_jobs(JOBS).expect("jobs")),
        )
        .expect("three fragments of one run");

        let by_name = |name: &str| {
            merged
                .quality
                .gates
                .iter()
                .find(|g| g.gate == name)
                .unwrap_or_else(|| panic!("no gate {name}"))
                .conclusion
                .clone()
        };
        assert_eq!(by_name("ci (cli)"), Some("success".to_string()));
        assert_eq!(
            by_name("ci (harness)"),
            Some("failure".to_string()),
            "every test in this gate passed; the job did not, and that is the whole point"
        );
        assert_eq!(
            by_name("ci (unlisted)"),
            None,
            "a gate the job list does not mention is unknown, never a pass"
        );
    }

    /// Without a job list, nothing is claimed.
    #[test]
    fn a_merge_with_no_job_list_leaves_every_conclusion_unknown() {
        let merged = merge(vec![fragment(&provenance(), mixed_gate())], None).expect("merges");
        assert!(merged.quality.run.is_none());
        assert!(
            merged.quality.gates.iter().all(|g| g.conclusion.is_none()),
            "a merge that could not ask must not answer"
        );
    }

    /// A report written before #516 still parses, which is why these are additive.
    ///
    /// The site refuses a `format_version` it does not know, so bumping would blank the
    /// quality pages until the next successful run on main replaced the report they read.
    /// Adding optional fields costs an old consumer nothing and an old *document* nothing.
    #[test]
    fn a_report_from_before_the_conclusions_existed_still_parses() {
        let current = fragment(&provenance(), mixed_gate());
        let mut json = serde_json::to_value(&current).expect("serializes");
        json["quality"]
            .as_object_mut()
            .expect("quality object")
            .remove("run");
        for gate in json["quality"]["gates"]
            .as_array_mut()
            .expect("gates array")
        {
            gate.as_object_mut()
                .expect("gate object")
                .remove("conclusion");
        }

        let back: Envelope =
            serde_json::from_value(json).expect("a pre-#516 report must still be readable");
        assert!(back.quality.run.is_none());
        assert!(back.quality.gates.iter().all(|g| g.conclusion.is_none()));
    }

    #[test]
    fn a_fragment_round_trips_through_json() {
        let e = fragment(&provenance(), mixed_gate());
        let text = serde_json::to_string(&e).expect("serializes");
        let back: Envelope = serde_json::from_str(&text).expect("parses");
        assert_eq!(back, e);
    }

    #[test]
    fn the_manifest_version_is_read_from_the_package_table() {
        let manifest = "\
[package]\nname = \"yidam\"\nversion = \"0.7.0\"\n\n[dependencies]\nserde = \"1\"\nversion = \"9\"\n";
        assert_eq!(cargo_version(manifest).as_deref(), Some("0.7.0"));
    }
}
