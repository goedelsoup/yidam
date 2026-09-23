//! `yidam phase` — a phase declares what it began from, and says so after an interruption.
//!
//! `prelude/PHASES.md` has specified a phase as a branch, a declared input state, a body of
//! agent work and a `--no-ff` merge since long before anything could hold one. Nothing did.
//! `cmd/phases.rs` derived a phase's state from its ref's namespace, so *active* meant *a ref
//! matching a glob*: a phase whose run died halfway through and a phase somebody opened this
//! morning were the same row, and neither the repository nor a reader could tell them apart.
//!
//! Three verbs, and the division between them is RFC-0026's:
//!
//! | verb | what it does | what it writes |
//! |---|---|---|
//! | `start` | opens `phase/<slug>` and snapshots the input state | one `scaffold:` commit |
//! | `run` | invokes the manifest's plan, recording steps as they complete | one `scaffold:` commit per step, plus whatever the step itself lands |
//! | `settle` | checks the phase produced outputs and drafts the merge subject | **nothing** |
//!
//! # `settle` prepares; it does not merge
//!
//! `phase:` is an **epistemic** verb, and RFC-0026 §2's invariant is that *a run authors
//! operational commits directly, every epistemic commit it produces goes to a proposal branch,
//! and nothing merges itself.* A `settle` that wrote the `--no-ff` merge would breach that
//! invariant on this layer's second surface.
//!
//! The limit is not invented here. `cmd/due.rs` already reached it, of the phase clock:
//!
//! > A person. Merging a phase, or abandoning it, is not a mechanical consequence of a
//! > finding.
//!
//! So `settle` validates, drafts a vocabulary-checked subject, and hands back. Same shape as
//! `propose`, which is the point — and it is pinned by
//! `tests/phase_record.rs`'s `settle_authors_no_commit_and_moves_no_ref`.
//!
//! # Why `start` writes a `scaffold:` commit rather than a file
//!
//! RFC-0028 §3 makes the record authoritative for state *"where a run record exists **for a
//! ref**"*. `yidam phases` reads remote-tracking refs — the shape a fresh CI clone has — so a
//! record that lived only in a working tree would answer for one branch and leave every other
//! phase in the repository ref-inferred forever.
//!
//! `scaffold:` is GRAPH.md's *"structure created"*, and it is **operational**: the pipeline
//! advanced and no understanding changed. That keeps the invariant above untouched, because
//! the one verb this layer might have wanted is the one only a person may write.
//!
//! # The commit lands on the branch; the checkout does not move
//!
//! Inherited whole from `cmd/propose/write.rs`, for RFC-0026 §5's four properties: this
//! touches neither the working tree nor `.git/index`, it is safe to run mid-edit, it writes
//! objects and one ref, and it separates author from committer so the record says the tool
//! wrote it and a person invoked it.
//!
//! The consequence is the one `cmd/run` states rather than discovers: after `phase start` you
//! are still on the branch you were on, and `phase/<slug>` stands one commit ahead of it. The
//! report says so and names the `git switch` that takes you there. A command that updated the
//! checkout would fail halfway on a dirty tree and leave somebody's work somewhere they did
//! not put it, which is why `propose` went onto a temporary index in the first place.

pub mod record;

use std::fmt::Write as _;
use std::path::Path;

use anyhow::{bail, Context, Result};
use clap::Subcommand;

use crate::cmd::propose::write::{commit_tree, current_branch, git, head, short_of, TempIndex};
use crate::cmd::run::manifest::{Manifest, MANIFEST};
use crate::cmd::run::receipt::sha256;
use crate::paths::{repo_root, require_yidam_repo};
use crate::report::Format;
use record::{Input, Record, Step};

/// The author every commit `yidam phase` writes carries.
///
/// A third tool writes commits here now. `cmd/run` records why the field is a parameter at
/// all: *a log that called them the same author would have lost the only distinction the
/// field exists to make.* `propose` drafts what a person may merge, `run` advances the
/// pipeline, and this opens and records a phase — three licences, three authors.
const AUTHOR_NAME: &str = "yidam phase";
const AUTHOR_EMAIL: &str = "phase@yidam";

/// The namespace `PHASES.md` defines a phase in. One phase, one branch.
const PHASE_PREFIX: &str = "phase/";

#[derive(Debug, Subcommand)]
pub enum PhaseCommand {
    /// Open `phase/<name>` and snapshot the state the phase begins from
    ///
    /// The snapshot is RFC-0026 §1's input state — the commit, the manifest digest and the
    /// config digest — plus the kuten revision the declared type was validated against, so
    /// that a phase resumed across a re-vendor is detectable by comparing two fields.
    ///
    /// Writes objects and one ref. The working tree, the index and HEAD are untouched, so it
    /// is safe to run mid-edit; the report names the `git switch` that takes you there.
    Start {
        /// The phase name, in kebab-case — `outcome-axis`, `the-local-half`
        name: String,
        /// The phase type, checked against the vendored kuten's list where one is held
        #[arg(long = "type", value_name = "TYPE")]
        kind: String,
        /// Output format. `json` emits the machine-readable report contract (RFC-0016)
        #[arg(long, value_enum, default_value_t = Format::Text)]
        format: Format,
    },
    /// Invoke this phase's plan, recording each step in the phase record as it completes
    ///
    /// Resumable: a step already recorded is not re-invoked, so a run killed partway
    /// completes rather than restarts. Until every step of the resolved plan is recorded the
    /// phase reads as `interrupted` in `yidam phases` and in `yidam status`.
    Run {
        /// Resolve the plan and report what is left, invoking nothing and writing nothing
        #[arg(long)]
        dry_run: bool,
        /// Output format. `json` emits the machine-readable report contract (RFC-0016)
        #[arg(long, value_enum, default_value_t = Format::Text)]
        format: Format,
    },
    /// Check this phase produced outputs and draft the merge subject — a person merges
    ///
    /// Authors nothing. `phase:` is an epistemic verb and RFC-0026 §2 forbids this layer
    /// writing one, so the merge is printed for a person to run. `cmd/due.rs` reached the
    /// same limit first: merging a phase is not a mechanical consequence of a finding.
    Settle {
        /// Output format. `json` emits the machine-readable report contract (RFC-0016)
        #[arg(long, value_enum, default_value_t = Format::Text)]
        format: Format,
    },
}

pub fn run(sub: PhaseCommand) -> Result<()> {
    match sub {
        PhaseCommand::Start { name, kind, format } => start(&name, &kind, format),
        PhaseCommand::Run { dry_run, format } => run_phase(dry_run, format),
        PhaseCommand::Settle { format } => settle(format),
    }
}

// ── the kuten the type is validated against ───────────────────────────────────

/// What this repository declares about phase types, and at which revision.
///
/// Every field is optional because no repository holds a kuten today — 0 of 18 derived corpora
/// at A0 — and a layer that refused to open a phase without one would be unusable in every
/// repository that exists.
#[derive(Debug, Default)]
struct Held {
    name: Option<String>,
    revision: Option<u32>,
    /// The valid types, where the held profile declares them. **Never a default list.**
    /// RFC-0028 Erratum 1 struck the no-kuten default for the reason that implementing it is
    /// the only way to create the artefact the section objects to — a phase-type list
    /// compiled into this binary.
    types: Option<Vec<String>>,
}

fn held_kuten(root: &Path) -> Result<Held> {
    let Some(decl) = crate::kuten::read_declaration(root)? else {
        return Ok(Held::default());
    };
    let profile = crate::kuten::read_profile(root, &decl.name)?;
    Ok(Held {
        types: profile
            .as_ref()
            .and_then(|p| p.phases.as_ref())
            .map(|p| p.types.clone()),
        name: Some(decl.name),
        revision: Some(decl.revision),
    })
}

impl Held {
    /// Refuse a type the vendored kuten does not declare.
    ///
    /// RFC-0028 §3 puts the enforcing consumer here: *"the enforcing consumer stays where this
    /// section already puts it — #473's `phase start`, validating a declared type against the
    /// vendored list."* Against the **vendored** list, never the template's current one, which
    /// is §2's rule and A0's whole retraction: a repository whose vendored prelude could not
    /// have known about a type has not declined to use it.
    fn validate(&self, kind: &str) -> Result<()> {
        let Some(types) = self.types.as_deref().filter(|t| !t.is_empty()) else {
            return Ok(());
        };
        if types.iter().any(|t| t == kind) {
            return Ok(());
        }
        bail!(
            "`{kind}` is not a phase type the vendored kuten declares.\n  \
             {} declares: {}\n  \
             The list is the one this repository vendored, not yidam's current one — see \
             .yidam/decisions/kuten.yml.",
            self.name.as_deref().unwrap_or("the kuten"),
            types.join(", ")
        )
    }
}

// ── start ─────────────────────────────────────────────────────────────────────

#[derive(Debug, serde::Serialize)]
struct StartReport {
    phase: String,
    r#type: String,
    branch: String,
    /// The branch `start` was invoked from, which is still the checkout — see the module doc.
    from: String,
    commit: String,
    record: String,
    input: Input,
}

/// Whether a name is already the kebab-case `PHASES.md` prescribes.
///
/// Refused rather than kebab-ified, which is this repository's rule wherever a guess is
/// available: the slug is a ref name, a file stem and a record's identity at once, and a
/// command that quietly renamed what it was given would make those three agree with each
/// other and with nothing the person typed.
fn is_kebab(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with('-')
        && !name.ends_with('-')
        && !name.contains("--")
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

fn kebab(name: &str) -> String {
    let mut out = String::new();
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            out.extend(c.to_lowercase());
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    out.trim_matches('-').to_string()
}

fn branch_exists(root: &Path, branch: &str) -> bool {
    git(
        root,
        None,
        &[
            "show-ref",
            "--verify",
            "--quiet",
            &format!("refs/heads/{branch}"),
        ],
        None,
    )
    .is_ok()
}

pub fn start(name: &str, kind: &str, format: Format) -> Result<()> {
    let root = repo_root()?;
    require_yidam_repo(&root)?;

    let slug = name.strip_prefix(PHASE_PREFIX).unwrap_or(name);
    if !is_kebab(slug) {
        let suggestion = kebab(slug);
        bail!(
            "`{slug}` is not a phase name — PHASES.md names a phase in kebab-case, and the \
             name is the ref, the record's file stem and its identity at once.\n  \
             Did you mean `{suggestion}`?"
        );
    }
    let branch = format!("{PHASE_PREFIX}{slug}");
    if branch_exists(&root, &branch) {
        bail!(
            "{branch} already exists — a phase is opened once.\n  \
             Read it with `yidam phases`, or `git switch {branch}` to continue it."
        );
    }

    let held = held_kuten(&root)?;
    held.validate(kind)?;

    let Some(from) = current_branch(&root) else {
        bail!(
            "HEAD is detached, so there is no baseline for a phase to branch from.\n  \
             PHASES.md branches a phase from the baseline — `main`, or the elector's \
             `ma/<name>` position. Check one out first."
        );
    };
    let (parent, short_parent) = head(&root)?;

    let input = Input {
        commit: parent.clone(),
        manifest_sha256: digest_of(&root, MANIFEST),
        config_sha256: digest_of(&root, ".yidam/config.toml"),
        kuten: held.name.clone(),
        kuten_revision: held.revision,
    };
    let rec = Record::opened(slug, kind, input);
    let subject = format!("scaffold: phase {slug} — {kind} opened at {short_parent}");
    let message = start_message(&subject, &rec, &from);
    let sha = land(&root, &branch, &parent, None, &rec, &message)?;

    let report = StartReport {
        phase: slug.to_string(),
        r#type: kind.to_string(),
        branch,
        from,
        commit: short_of(&root, &sha),
        record: record::path(slug),
        input: rec.input,
    };
    if format.is_json() {
        return crate::report::emit(&root, report);
    }
    println!("{}", render_start(&report));
    Ok(())
}

fn start_message(subject: &str, rec: &Record, from: &str) -> String {
    let mut body = format!("{subject}\n\n");
    let _ = writeln!(
        body,
        "The input state this phase begins from, so that an interrupted one is legible and\n\
         `has this already run against this corpus` is an equality check.\n"
    );
    let _ = writeln!(body, "  commit    {}", rec.input.commit);
    let _ = writeln!(
        body,
        "  manifest  sha256:{}",
        &rec.input.manifest_sha256[..16]
    );
    let _ = writeln!(
        body,
        "  config    sha256:{}",
        &rec.input.config_sha256[..16]
    );
    match (&rec.input.kuten, rec.input.kuten_revision) {
        (Some(k), Some(r)) => {
            let _ = writeln!(body, "  kuten     {k} revision {r}");
        }
        _ => {
            let _ = writeln!(
                body,
                "  kuten     none declared — the type is recorded and validated against nothing"
            );
        }
    }
    let _ = write!(
        body,
        "\nOperational: the pipeline advanced and no understanding changed. Branched from\n\
         {from}, which is still checked out — nothing in the working tree or the index moved.\n"
    );
    body
}

fn render_start(r: &StartReport) -> String {
    let mut out = format!(
        "Opened {} — {} phase, at {}\n\n",
        r.branch,
        r.r#type,
        &r.input.commit[..7]
    );
    let _ = writeln!(out, "  {}  {}", r.commit, r.record);
    let _ = write!(
        out,
        "\nYou are still on {}. Continue the phase with:\n\n  git switch {}\n",
        r.from, r.branch
    );
    out
}

// ── the branch a phase command acts on ────────────────────────────────────────

/// The phase the checkout is standing in, and its record.
///
/// Both `run` and `settle` act on *this* phase rather than on a named one, which is
/// `PHASES.md`'s model rather than a convenience: one phase, one branch, and the branch you
/// are on is the phase you are in. A `--phase <name>` flag would let a run land steps on a
/// branch nobody was looking at.
fn current_phase(root: &Path) -> Result<(String, String, Record)> {
    let Some(branch) = current_branch(root) else {
        bail!("HEAD is detached, so there is no phase branch to act on");
    };
    let Some(slug) = branch.strip_prefix(PHASE_PREFIX).map(str::to_string) else {
        bail!(
            "`{branch}` is not a phase branch — PHASES.md defines a phase in the `phase/*` \
             namespace.\n  Open one with `yidam phase start <name> --type <type>`."
        );
    };
    // Read out of `HEAD` rather than off the working tree, and the difference is not
    // fastidiousness. Every commit this module lands moves the ref and leaves the checkout
    // where it was — RFC-0026 §5's four properties, inherited from `cmd/propose/write.rs` —
    // so after one step of a run the file on disk is the record from *before* that step. A
    // resumption that read it would re-invoke work it had already recorded, which is the
    // failure this whole surface exists to prevent.
    let Some(rec) = record::read(root, "HEAD", &slug)? else {
        bail!(
            "{branch} carries no phase record at {} — it was opened by hand rather than by \
             `yidam phase start`, so nothing snapshotted what it began from.\n  \
             Open a recorded phase with `yidam phase start <name> --type <type>`.",
            record::path(&slug)
        )
    };
    Ok((branch, slug, rec))
}

/// The digest of a repository file, or of nothing where there is none.
///
/// `cmd/run`'s, restated rather than shared because that one is private to the executor and
/// the two answer for different files: a corpus with no `.yidam/config.toml` has a real input
/// state rather than an unknown one, and the digest of the empty string says so without a
/// second representation for absence.
fn digest_of(root: &Path, rel: &str) -> String {
    sha256(&std::fs::read(root.join(rel)).unwrap_or_default())
}

/// Land the record as one commit on `branch`, and move the ref.
///
/// `expected` is the sha the ref is believed to stand at, or `None` where this call creates
/// it. `update-ref <ref> <new> <old>` is a compare-and-swap, and the all-zero sha is git's
/// spelling of *must not exist* — the same guarantee in the one case where there is no old
/// value to name. `cmd/run` makes the argument for it; a phase run is a longer interval than
/// a calculator's and wants it more.
fn land(
    root: &Path,
    branch: &str,
    parent: &str,
    expected: Option<&str>,
    rec: &Record,
    message: &str,
) -> Result<String> {
    const ABSENT: &str = "0000000000000000000000000000000000000000";

    let scratch = TempIndex::new(root, "phase")?;
    let index = scratch.path().to_path_buf();
    git(root, Some(&index), &["read-tree", parent], None)?;

    let path = record::path(&rec.phase);
    let blob = git(
        root,
        Some(&index),
        &["hash-object", "-w", "--stdin"],
        Some(&rec.to_yaml()?),
    )?;
    git(
        root,
        Some(&index),
        &[
            "update-index",
            "--add",
            "--cacheinfo",
            &format!("100644,{blob},{path}"),
        ],
        None,
    )?;
    let tree = git(root, Some(&index), &["write-tree"], None)?;
    let sha = commit_tree(root, &tree, parent, message, (AUTHOR_NAME, AUTHOR_EMAIL))?;
    git(
        root,
        None,
        &[
            "update-ref",
            &format!("refs/heads/{branch}"),
            &sha,
            expected.unwrap_or(ABSENT),
        ],
        None,
    )
    .with_context(|| {
        format!("{branch} moved while the phase record was being written; nothing was landed")
    })?;
    Ok(sha)
}

// ── run ───────────────────────────────────────────────────────────────────────

#[derive(Debug, serde::Serialize)]
struct PhaseRunReport {
    phase: String,
    r#type: String,
    branch: String,
    dry_run: bool,
    /// The plan as resolved, in the order it was resolved.
    plan: Vec<String>,
    /// Steps this invocation recorded — never the ones an earlier one already had.
    completed: Vec<Step>,
    /// Steps of the plan this phase has not got through.
    ///
    /// **Non-empty only under `--dry-run`.** A run that produces this report has completed
    /// its plan; a step that failed stops the command with an error rather than a report, and
    /// that error is where the outstanding count is said. The field is still emitted on a
    /// completed run — as `[]` — because a consumer must not have to tell "none outstanding"
    /// from "this binary does not report it".
    remaining: Vec<String>,
    /// Steps a previous invocation had already recorded, which this one did not re-enter the
    /// plan at. The number that makes resumption visible rather than merely true.
    ///
    /// Not a promise that none of them ran. A skipped step is still re-invoked when a later
    /// step declares itself `after` it and its own input state has moved — `after` doing its
    /// job — and when that happens it appears in `completed` beside the steps this invocation
    /// planned. The two fields together say what happened; either alone would round it.
    resumed: usize,
    /// `{recorded, held}` where the kuten moved under this phase — RFC-0028 §2's
    /// annotate-never-silently-proceed, applied to a phase in flight across a re-vendor.
    #[serde(skip_serializing_if = "Option::is_none")]
    revision_skew: Option<Skew>,
}

#[derive(Debug, serde::Serialize)]
struct Skew {
    recorded: Option<u32>,
    held: Option<u32>,
}

fn run_phase(dry_run: bool, format: Format) -> Result<()> {
    let root = repo_root()?;
    require_yidam_repo(&root)?;
    let (branch, slug, mut rec) = current_phase(&root)?;

    let manifest = Manifest::load(&root)?;
    let plan: Vec<String> = manifest
        .plan(None)?
        .into_iter()
        .map(str::to_string)
        .collect();
    if plan.is_empty() {
        bail!("{MANIFEST} declares no capabilities, so this phase has nothing to run");
    }

    let held = held_kuten(&root)?;
    let skew = rec
        .revision_skew(held.revision)
        .map(|(recorded, held)| Skew { recorded, held });

    let resumed = plan.iter().filter(|s| rec.is_completed(s)).count();
    let outstanding: Vec<String> = plan
        .iter()
        .filter(|s| !rec.is_completed(s))
        .cloned()
        .collect();

    if dry_run {
        let report = PhaseRunReport {
            phase: slug,
            r#type: rec.r#type.clone(),
            branch,
            dry_run: true,
            plan,
            completed: Vec::new(),
            remaining: outstanding,
            resumed,
            revision_skew: skew,
        };
        if format.is_json() {
            return crate::report::emit(&root, report);
        }
        println!("{}", render_run(&report));
        return Ok(());
    }

    // The plan is recorded before the first step is invoked, and that order is what makes an
    // interruption legible. A record that learned its plan on the way through could only ever
    // say "these steps ran" — never "and these did not", which is the whole of the difference
    // between `active` and `interrupted`.
    let mut recorded = Vec::new();
    if rec.plan.as_deref() != Some(plan.as_slice()) {
        rec.plan = Some(plan.clone());
        commit_record(
            &root,
            &branch,
            &rec,
            &format!("scaffold: phase {slug} — a plan of {} step(s)", plan.len()),
            &format!(
                "The plan `yidam phase run` resolved, written before the first step is\n\
                 invoked so that a run interrupted partway says which steps did not happen.\n\n\
                 {}\n\nOperational: the pipeline advanced and no understanding changed.\n",
                plan.iter()
                    .map(|s| format!("  {s}"))
                    .collect::<Vec<_>>()
                    .join("\n")
            ),
        )?;
    }

    for step in &plan {
        if rec.is_completed(step) {
            continue;
        }
        // One step at a time, through the executor that already owns freshness, routing and
        // the commit. `plan_and_write` re-resolves this step's `after` closure on each call,
        // and every dependency it finds is already fresh — its receipt matches — so the
        // repeated resolution costs a comparison and lands nothing. The alternative, a
        // callback into the executor's loop, would put this module inside the one code path
        // whose invariant `run_route_claims.rs` exists to pin.
        let report =
            crate::cmd::run::plan_and_write(&root, Some(step), false).with_context(|| {
                // The step failed, and everything before it is already committed, so the repair
                // is to fix the step and re-run rather than to start over. Said here because
                // this is the one path that produces no report to say it.
                let left = plan.iter().filter(|s| !rec.is_completed(s)).count();
                format!(
                    "`{step}` did not complete, so the phase stops here with {} of {} step(s) \
                 recorded.\n  {branch} now reads `interrupted`. Fix the step and re-run \
                 `yidam phase run` — the {left} outstanding step(s) are what it will invoke, \
                 and nothing already recorded is re-entered.",
                    rec.completed.len(),
                    plan.len(),
                )
            })?;
        for s in &report.steps {
            let done = Step {
                step: s.step.clone(),
                outcome: s.outcome.tag().to_string(),
                commit: s.committed.as_ref().map(|c| c.commit.clone()),
            };
            // Two things arrive here that look alike and are not. `plan_and_write` re-resolves
            // this step's `after` closure on every call, so its report names the upstreams
            // too — and an upstream is either **fresh**, in which case the executor skipped
            // it and this invocation learned nothing the record did not hold, or it was
            // re-invoked because its own input state moved, which is `after` doing its job.
            //
            // Only the second is written. Overwriting a recorded `ran` with a later
            // observation of `skipped` would drop the commit sha that entry exists to carry,
            // and reporting it would put a step nothing did into a list of what this run did.
            let known = rec.completed.iter_mut().find(|e| e.step == done.step);
            match (known, done.outcome.as_str()) {
                (Some(existing), "ran") => *existing = done.clone(),
                (Some(_), _) => continue,
                (None, _) => rec.completed.push(done.clone()),
            }
            match recorded
                .iter_mut()
                .find(|e: &&mut Step| e.step == done.step)
            {
                Some(existing) => *existing = done,
                None => recorded.push(done),
            }
        }
        commit_record(
            &root,
            &branch,
            &rec,
            &format!(
                "scaffold: phase {slug} — {step} recorded, {}/{} done",
                rec.completed.len(),
                plan.len()
            ),
            &format!(
                "`{step}` completed and the phase record says so, committed before the next\n\
                 step is invoked. A run killed here resumes at the step after this one.\n\n\
                 Operational: the pipeline advanced and no understanding changed.\n"
            ),
        )?;
    }

    let report = PhaseRunReport {
        phase: slug,
        r#type: rec.r#type.clone(),
        branch,
        dry_run: false,
        plan,
        completed: recorded,
        remaining: rec.remaining(),
        resumed,
        revision_skew: skew,
    };
    if format.is_json() {
        return crate::report::emit(&root, report);
    }
    println!("{}", render_run(&report));
    Ok(())
}

/// Land the record as it now stands, unless it is already the committed bytes.
///
/// Returns without writing when the tree would not move, which is `cmd/run`'s rule and its
/// reason: the record carries no clock, so a re-run against an unchanged state produces
/// byte-identical bytes, and an empty commit per invocation would turn the log into a record
/// of how often somebody ran the command.
fn commit_record(
    root: &Path,
    branch: &str,
    rec: &Record,
    subject: &str,
    body: &str,
) -> Result<Option<String>> {
    let (parent, _) = head(root)?;
    let committed = git(
        root,
        None,
        &[
            "rev-parse",
            &format!("{parent}:{}", record::path(&rec.phase)),
        ],
        None,
    );
    let produced = git(
        root,
        None,
        &["hash-object", "--stdin"],
        Some(&rec.to_yaml()?),
    )?;
    if matches!(committed, Ok(ref c) if *c == produced) {
        return Ok(None);
    }
    let message = format!("{subject}\n\n{body}");
    land(root, branch, &parent, Some(&parent), rec, &message).map(Some)
}

fn render_run(r: &PhaseRunReport) -> String {
    let mut out = format!("{} — {} phase", r.branch, r.r#type);
    if r.dry_run {
        out.push_str("  (dry run — nothing invoked, nothing written)");
    }
    out.push_str("\n\n");
    for step in &r.plan {
        let mark = match r.completed.iter().find(|s| &s.step == step) {
            Some(s) => format!("{:<9}", s.outcome),
            None if r.remaining.iter().any(|s| s == step) => "pending  ".to_string(),
            None => "recorded ".to_string(),
        };
        let _ = writeln!(out, "  {mark} {step}");
    }
    if r.resumed > 0 {
        let _ = writeln!(
            out,
            "\n{} step(s) were already recorded and were not re-invoked.",
            r.resumed
        );
    }
    // Only under `--dry-run`. A run that reaches its report has completed its plan: a step
    // that failed returned an error from the loop above, which is where the outstanding
    // count is reported instead. A branch here for the non-dry case would be one nobody
    // can take.
    if !r.remaining.is_empty() && r.dry_run {
        let _ = writeln!(
            out,
            "\n{} step(s) outstanding. The phase reads `interrupted` until they complete.",
            r.remaining.len()
        );
    }
    if let Some(s) = &r.revision_skew {
        let _ = writeln!(
            out,
            "\nThe kuten moved under this phase: opened at revision {}, this repository holds \
             {}.\n  The declared type was validated against the list as it stood then.",
            s.recorded.map_or("none".to_string(), |r| r.to_string()),
            s.held.map_or("none".to_string(), |r| r.to_string()),
        );
    }
    out
}

// ── settle ────────────────────────────────────────────────────────────────────

#[derive(Debug, serde::Serialize)]
struct SettleReport {
    phase: String,
    r#type: String,
    branch: String,
    /// The baseline this phase settles onto — `main`, or `master` where that is the name.
    baseline: String,
    /// Commits unique to the branch. `PHASES.md`: *a phase that never produces commits is an
    /// open inquiry thread, not a settled phase.*
    commits: usize,
    /// Repository-relative paths the phase changed against the baseline.
    outputs: Vec<String>,
    /// Steps the phase's plan has not got through. A phase settled over these settles work
    /// that did not finish, which is why they are reported rather than assumed absent.
    remaining: Vec<String>,
    /// Whether this phase has produced what `PHASES.md` requires before a merge.
    ready: bool,
    /// The `--no-ff` merge subject, checked against the closed vocabulary. **Drafted, never
    /// written** — see the module doc.
    subject: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    revision_skew: Option<Skew>,
}

fn settle(format: Format) -> Result<()> {
    let root = repo_root()?;
    require_yidam_repo(&root)?;
    let (branch, slug, rec) = current_phase(&root)?;

    let Some(baseline) = crate::git::base_branch(&root) else {
        bail!("no `main` or `master` to settle onto — PHASES.md settles a phase onto the baseline");
    };

    let not_base = format!("^{baseline}");
    let commits = git(
        &root,
        None,
        &["rev-list", "--count", &branch, &not_base],
        None,
    )
    .ok()
    .and_then(|s| s.trim().parse::<usize>().ok())
    .unwrap_or(0);
    let outputs: Vec<String> = git(
        &root,
        None,
        &["diff", "--name-only", &format!("{baseline}...{branch}")],
        None,
    )
    .unwrap_or_default()
    .lines()
    .map(str::trim)
    .filter(|l| !l.is_empty())
    .map(str::to_string)
    .collect();

    let remaining = rec.remaining();
    let ready = commits > 0 && !outputs.is_empty() && remaining.is_empty();
    let subject = merge_subject(&slug, commits, outputs.len());
    // The subject this drafts carries `phase`, which GRAPH.md closes and `lint --commits`
    // reads. Checked here rather than assumed, because a drafted subject a person pastes is
    // one this tool is answerable for — the same reason `cmd/run` builds its subject from a
    // verb the manifest already validated.
    debug_assert!(
        yidam_core::git::is_recognized_verb(
            subject
                .split_once(": ")
                .map_or(subject.as_str(), |(v, _)| v)
        ),
        "the drafted merge subject is outside the closed vocabulary: {subject}"
    );

    let held = held_kuten(&root)?;
    let report = SettleReport {
        phase: slug,
        r#type: rec.r#type.clone(),
        branch,
        baseline,
        commits,
        outputs,
        remaining,
        ready,
        subject,
        revision_skew: rec
            .revision_skew(held.revision)
            .map(|(recorded, held)| Skew { recorded, held }),
    };
    if format.is_json() {
        return crate::report::emit(&root, report);
    }
    println!("{}", render_settle(&report));
    Ok(())
}

fn merge_subject(slug: &str, commits: usize, files: usize) -> String {
    format!(
        "phase: {slug} — {commits} commit{} across {files} file{}",
        if commits == 1 { "" } else { "s" },
        if files == 1 { "" } else { "s" }
    )
}

fn render_settle(r: &SettleReport) -> String {
    let mut out = format!("{} — {} phase\n\n", r.branch, r.r#type);
    let _ = writeln!(
        out,
        "  {} commit(s) ahead of {}, touching {} file(s)",
        r.commits,
        r.baseline,
        r.outputs.len()
    );
    if !r.remaining.is_empty() {
        let _ = writeln!(
            out,
            "  {} step(s) of the plan never completed: {}",
            r.remaining.len(),
            r.remaining.join(", ")
        );
    }
    if let Some(s) = &r.revision_skew {
        let _ = writeln!(
            out,
            "  the kuten moved under this phase: opened at revision {}, held {}",
            s.recorded.map_or("none".to_string(), |v| v.to_string()),
            s.held.map_or("none".to_string(), |v| v.to_string()),
        );
    }
    out.push('\n');

    if !r.ready {
        let _ = write!(
            out,
            "Not ready to settle.\n\n  \
             PHASES.md: a phase that never produces commits is an open inquiry thread, not\n  \
             a settled phase. If it has stalled, open a question naming what is blocking it\n  \
             and return to {}.\n",
            r.baseline
        );
        return out;
    }

    let _ = write!(
        out,
        "Ready. `phase:` is an epistemic verb, so this drafts the merge and a person writes\n\
         it — nothing here merges itself:\n\n  \
         git switch {}\n  \
         git merge --no-ff -m \"{}\" {}\n  \
         git branch -d {}\n\n\
         The subject is a draft. Replace the tally with what the phase produced; it is checked\n\
         against the vocabulary like any other commit, and a git-generated one is exempt.\n",
        r.baseline, r.subject, r.branch, r.branch
    );
    out
}

#[cfg(test)]
mod tests;
