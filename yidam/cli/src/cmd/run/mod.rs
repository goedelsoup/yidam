//! `yidam run` — the declared capabilities that are stale are invoked, in order.
//!
//! `prelude/GRAPH.md` closes the commit vocabulary, and four of its operational verbs —
//! `extract`, `refresh`, `compute`, `reconcile` — name acts nothing in this repository could
//! perform. They were written for a runtime that was never built. #471 built the narrowest
//! path through it: one declared capability, invoked, its output committed, a receipt written.
//! #472 is the generalisation that path was deliberately not — a plan rather than a step.
//!
//! # A run is a plan, and the plan is resolved before anything is invoked
//!
//! [`manifest::Manifest::plan`] orders the steps so every dependency precedes what declares it,
//! and a manifest that cannot be ordered stops the command rather than stopping it halfway
//! through. Each stale step is then invoked against HEAD **as it stands when that step is
//! reached**, which is what makes `after` mean something rather than describe something: the
//! upstream's output is in the tree the downstream is given because the upstream committed it
//! a moment earlier.
//!
//! A step is skipped when its input state matches its committed receipt, which is the equality
//! check RFC-0026 §1 asks for. `ageing_days` is the declaration that overrides it, for a
//! capability whose answer depends on something this repository does not hold; the interval
//! lives in the manifest and never in this binary, for `config.rs`'s reason. `--dry-run`
//! resolves the plan, reports each verdict, and writes nothing.
//!
//! # What a run may author, and what stops it authoring more
//!
//! > A run authors operational commits directly. Every epistemic commit it produces goes to a
//! > proposal branch, and nothing merges itself.
//!
//! The vocabulary's own split is the permission model, so no new authority concept is needed
//! — RFC-0026 §2. **Both halves of that sentence are structure now.** For the first slice only
//! the first half existed and the second was held by a refusal: a capability declaring an
//! epistemic verb did not load. That was honest about what was built and it made the entire
//! epistemic family undeclarable — a run could compute a number and could not open a question
//! about one, which is the act a classifier performs and a calculator never wanted to.
//!
//! What replaced the refusal is a second **destination**, not a permission.
//! [`manifest::Capability::route`] reads the verb, through the same families
//! `classify_commit` draws, and returns where the commit goes. An operational verb advances
//! the branch. An epistemic verb lands on `propose/<head>` and the branch does not move. There
//! is no field, no flag and no policy key, because a manifest that could state a destination
//! could state the wrong one, and then the safety argument would be a config value — which is
//! the contradiction RFC-0024 named and RFC-0026 §3.1 answered.
//!
//! The property to preserve when changing anything here: **no path, and no config value, by
//! which a run advances the current branch with an epistemic commit.** It is pinned by
//! [`manifest`]'s `routes_are_a_total_function_of_the_verb` and its
//! `a_manifest_cannot_declare_a_route`, and by `capability_run.rs`'s
//! `an_epistemic_run_lands_on_a_proposal_branch_and_the_branch_does_not_move` —
//! which runs the real binary and compares the branch sha before and after.
//!
//! # The commit lands on the branch; the checkout does not move
//!
//! RFC-0026 §5 names four properties this inherits from `cmd/propose/write.rs` and requires
//! every one: it touches neither the working tree nor `.git/index`, it is safe to run
//! mid-edit, it writes objects and one ref, and it separates author from committer so the
//! record says the tool ran it and a person invoked it.
//!
//! Those four and *"a run authors operational commits directly"* have a consequence worth
//! stating rather than discovering: the ref this advances is the current branch, and the
//! index still holds the tree from before, so **after a run your checkout is behind HEAD** by
//! as many commits as the plan landed. That is not a defect to be fixed by touching the index — §5 forbids it, and the
//! reason is the one that put `propose` on a temporary index in the first place: a command
//! that updated a checkout would fail halfway on a dirty tree and leave somebody's work
//! somewhere they did not put it. So the report says so, and names the one path-scoped
//! command that syncs it.

pub mod exec;
pub mod manifest;
pub mod receipt;

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::path::Path;

use anyhow::{bail, Context, Result};

use crate::cmd::propose::write::{commit_tree, current_branch, git, head, short_of, TempIndex};
use crate::paths::{repo_root, require_yidam_repo};
use manifest::{Capability, Manifest, Route};
use receipt::{sha256, File, Input, Receipt};

/// The author every commit a run writes carries.
///
/// Not `yidam propose`'s author, and the difference is the point of recording one at all: two
/// tools write commits here now, they are licensed by different rules, and a log that called
/// them the same author would have lost the only distinction the field exists to make.
const AUTHOR_NAME: &str = "yidam run";
const AUTHOR_EMAIL: &str = "run@yidam";

pub struct Options {
    pub format: crate::report::Format,
    /// Resolve the plan, decide what is stale, and write nothing.
    pub dry_run: bool,
}

/// Whether a step's committed result still stands, decided before it is invoked.
///
/// Three states rather than two, because a dry run genuinely cannot answer for a step whose
/// inputs an earlier step in the plan would have written. Reporting that as either `Fresh` or
/// `Stale` would be a guess printed in the column a reader trusts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Freshness {
    /// The committed result was computed from this input state and has not aged out.
    Fresh,
    /// Something it depends on moved, it has never run, or it aged past its declared rule.
    Stale,
    /// Undecidable here: a step earlier in this plan would run first and write what this one
    /// reads, so its input state does not exist yet. Only ever reached under `--dry-run`.
    Unknown,
}

impl Freshness {
    fn tag(self) -> &'static str {
        match self {
            Self::Fresh => "fresh",
            Self::Stale => "stale",
            Self::Unknown => "unknown",
        }
    }
}

/// What the executor did about a step.
///
/// Separate from [`Freshness`], which is why it was done. The two are not one field because
/// the same verdict produces different acts: a stale step is invoked, or — under `--dry-run` —
/// is only named, and a report that collapsed the two would say a step ran when nothing did.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Outcome {
    /// Invoked, and what it produced was considered for a commit.
    Ran,
    /// Not invoked, because its committed result still stands.
    Skipped,
    /// Would have been invoked. `--dry-run` only.
    Planned,
}

impl Outcome {
    /// The word the report prints, and the one `cmd/phase`'s record carries — so a step this
    /// says `ran` and a phase record saying `ran` are two records of one event rather than
    /// two vocabularies that happen to agree today.
    pub(crate) fn tag(self) -> &'static str {
        match self {
            Self::Ran => "ran",
            Self::Skipped => "skipped",
            Self::Planned => "planned",
        }
    }
}

#[derive(Debug, serde::Serialize)]
pub struct StepReport {
    pub step: String,
    pub kind: &'static str,
    pub verb: String,
    /// `branch` or `proposal` — where this capability's commits go, decided by its verb.
    /// Reported rather than inferred by a consumer, because the two routes leave the
    /// repository in visibly different states and only one of them moves your checkout.
    pub route: &'static str,
    /// What this step declared it waits for, as declared.
    pub after: Vec<String>,
    pub freshness: Freshness,
    /// Why, in the words a person would use. The report says which and why — a plan whose
    /// verdicts a reader has to reconstruct is a plan they will re-derive by hand.
    pub because: String,
    pub outcome: Outcome,
    /// The commit this step's inputs were materialized from. `None` where they were not —
    /// an unknown step under `--dry-run` was never given a tree.
    pub input_commit: Option<String>,
    /// How many files the declaration resolved to at that commit.
    pub inputs: usize,
    /// What the step produced, by repository-relative path.
    pub outputs: Vec<String>,
    /// Where the receipt lives.
    pub receipt: String,
    /// The ref that moved, and the commit written — absent when the step changed nothing.
    pub committed: Option<Committed>,
    /// Whether the step ran and produced the bytes that were already committed.
    ///
    /// The ordinary result of an ageing rule firing over something whose answer had not moved,
    /// and a materially different event from a step that recomputed and found a change: the
    /// commit that lands carries a new receipt and the same outputs, which is the record of
    /// *having looked*. `cmd/due.rs` draws the same distinction about a source's TTL — *"An
    /// expiry does not claim the upstream changed. It claims nobody has looked."* A report
    /// that said `wrote` here would be claiming the first thing while recording the second.
    pub unchanged_outputs: bool,
    /// Anything the step said on stderr, kept because a calculator's own account of what it
    /// did is evidence and the commit does not carry it.
    pub step_stderr: Option<String>,
}

#[derive(Debug, serde::Serialize)]
pub struct RunReport {
    /// The step that was asked for, or `None` where the whole manifest was.
    pub requested: Option<String>,
    pub dry_run: bool,
    /// The commit the plan was resolved against — the state before any step ran.
    pub input_commit: String,
    /// The plan, in the order it was resolved and run.
    pub steps: Vec<StepReport>,
    pub ran: usize,
    pub skipped: usize,
    /// How many steps landed a commit. Never more than `ran`, and often fewer: a step that
    /// was re-run under an ageing rule and computed the same bytes lands nothing.
    pub committed: usize,
}

#[derive(Debug, serde::Serialize)]
pub struct Committed {
    /// The ref that moved. The invoking branch for an operational run, `propose/<head>` for
    /// an epistemic one — and for an epistemic one it is never the invoking branch, which is
    /// the invariant this field makes visible in the report and in the JSON.
    pub branch: String,
    pub commit: String,
    pub subject: String,
}

pub fn run(step: Option<&str>, opts: Options) -> Result<()> {
    let root = repo_root()?;
    require_yidam_repo(&root)?;
    let report = plan_and_write(&root, step, opts.dry_run)?;
    crate::report::finish(&root, opts.format, report, |r| println!("{}", render(r)))
}

/// Everything between resolving the corpus and printing — the testable half.
///
/// `step` is what was asked for: one capability and everything it waits for, or the whole
/// manifest. Either way the plan is resolved once, before anything is invoked, so a manifest
/// that cannot be ordered stops the command rather than stopping it halfway through.
///
/// # HEAD moves underneath this loop, and that is the mechanism
///
/// Each step is materialized from HEAD as it stands *when that step is reached*, not from the
/// commit the plan was resolved against. That is what makes `after` mean something: the
/// upstream's output is in the tree the downstream is given because the upstream committed it
/// a moment earlier. A loop that pinned one input commit for the whole plan would hand every
/// downstream step the tree from before its dependency ran, which is the failure the manifest's
/// own `deny_unknown_fields` note describes — running a dependent step before the step it
/// depends on and reporting success.
pub fn plan_and_write(root: &Path, step: Option<&str>, dry_run: bool) -> Result<RunReport> {
    let Some(branch) = current_branch(root) else {
        bail!(
            "HEAD is detached, so there is no branch for a run to advance.\n  \
             A run's output is a commit on the branch it was invoked from; check one out first."
        );
    };
    let (resolved_at, _) = head(root)?;

    let m = Manifest::load(root)?;
    let plan = m.plan(step)?;
    if plan.is_empty() {
        bail!(
            "{} declares no capabilities, so there is nothing to run",
            manifest::MANIFEST
        );
    }

    // Every step this binary cannot invoke, refused before any of them runs.
    //
    // In a pre-pass rather than when the step is reached, which is the rule the plan itself
    // follows: a manifest that cannot be ordered stops the command rather than stopping it
    // halfway through. A plan holding a connector behind two calculators would otherwise land
    // two commits and then refuse, leaving a corpus half advanced by a run that never had a
    // chance of finishing.
    let unrunnable: Vec<&str> = plan
        .iter()
        .copied()
        .filter(|n| m.get(n).is_ok_and(|c| !c.kind.executable()))
        .collect();
    if let Some(first) = unrunnable.first() {
        bail!(
            "`{first}` is a {} and this binary invokes calculators only, so this plan cannot \
             run.\n  \
             A connector re-runs against an external source, which is a network capability and \
             a credential path, and `vault/mod.rs` sets the rule those inherit.\n  \
             In this plan: {}",
            m.get(first)?.kind.as_str(),
            unrunnable.join(", ")
        );
    }

    let manifest_sha256 = digest_of(root, manifest::MANIFEST);
    let config_sha256 = digest_of(root, ".yidam/config.toml");
    let today = crate::dates::today_days();

    // Steps in this plan whose output the repository does not hold, accumulated as the loop
    // decides them. A step whose dependency is in here has inputs that do not exist yet, which
    // is decidable only because the plan is ordered — every dependency has already been decided
    // by the time its dependent is reached.
    //
    // **It is only ever non-empty under `--dry-run`.** A real run invokes a stale step and
    // commits what it wrote before moving to the next, so by the time a dependent is reached
    // its dependency's output is at HEAD and its input state is an ordinary thing to compute.
    // That is the whole difference between the two modes: a dry run cannot answer for a step
    // downstream of one it did not run, and says so rather than guessing.
    let mut pending: BTreeSet<&str> = BTreeSet::new();
    let mut steps: Vec<StepReport> = Vec::new();

    for name in plan {
        let cap = m.get(name)?;
        let receipt_path = Receipt::path(name);
        let (full, short) = head(root)?;
        let route = cap.route();
        let target = match route {
            Route::Branch => branch.clone(),
            Route::Proposal => crate::cmd::propose::write::branch_for(&short),
        };
        let existing = match route {
            Route::Branch => Some(full.clone()),
            Route::Proposal => tip_of(root, &target),
        };
        let parent = existing.clone().unwrap_or_else(|| full.clone());

        let waiting_on: Vec<&str> = cap
            .after
            .iter()
            .map(String::as_str)
            .filter(|d| pending.contains(d))
            .collect();
        if !waiting_on.is_empty() {
            // Undecidable here, so everything downstream of it is undecidable too.
            pending.insert(name);
            steps.push(StepReport {
                step: name.to_string(),
                kind: cap.kind.as_str(),
                verb: cap.verb.clone(),
                route: route.as_str(),
                after: cap.after.clone(),
                freshness: Freshness::Unknown,
                because: format!(
                    "{} would run first and write what this reads, so its input state does \
                     not exist yet",
                    waiting_on.join(", ")
                ),
                outcome: Outcome::Planned,
                input_commit: None,
                inputs: 0,
                outputs: Vec::new(),
                receipt: receipt_path,
                committed: None,
                unchanged_outputs: false,
                step_stderr: None,
            });
            continue;
        }

        let inputs = exec::materialize(root, &full, cap)?;
        let input_state =
            Receipt::input_state(cap, &manifest_sha256, &config_sha256, &inputs.files)?;
        let verdict = freshness(root, cap, &parent, &receipt_path, &input_state, today);

        let mut report = StepReport {
            step: name.to_string(),
            kind: cap.kind.as_str(),
            verb: cap.verb.clone(),
            route: route.as_str(),
            after: cap.after.clone(),
            freshness: verdict.freshness(),
            because: verdict.because().to_string(),
            outcome: Outcome::Skipped,
            input_commit: Some(short.clone()),
            inputs: inputs.files.len(),
            outputs: Vec::new(),
            receipt: receipt_path.clone(),
            committed: None,
            unchanged_outputs: false,
            step_stderr: None,
        };

        if verdict.freshness() == Freshness::Fresh {
            steps.push(report);
            continue;
        }
        if dry_run {
            pending.insert(name);
            report.outcome = Outcome::Planned;
            steps.push(report);
            continue;
        }

        let produced = exec::invoke(cap, &inputs, name, &full)?;
        exec::check_declared(cap, &produced.outputs)?;

        let receipt = Receipt {
            format_version: receipt::FORMAT_VERSION,
            step: name.to_string(),
            kind: cap.kind.as_str(),
            verb: cap.verb.clone(),
            run: cap.run.clone(),
            input_state,
            input: Input {
                commit: full.clone(),
                manifest_sha256: manifest_sha256.clone(),
                config_sha256: config_sha256.clone(),
                reads: cap.reads.clone(),
                files: inputs.files.clone(),
            },
            writes: cap.writes.clone(),
            outputs: produced
                .outputs
                .iter()
                .map(|(path, bytes)| File {
                    path: path.clone(),
                    sha256: sha256(bytes),
                })
                .collect(),
        };

        let mut landing: Vec<(String, Vec<u8>)> = produced.outputs.clone();
        landing.push((receipt_path.clone(), receipt.to_yaml()?.into_bytes()));

        report.outcome = Outcome::Ran;
        report.outputs = produced.outputs.iter().map(|(p, _)| p.clone()).collect();
        report.unchanged_outputs = outputs_already_committed(root, &parent, &produced.outputs);
        report.step_stderr = (!produced.stderr.is_empty()).then_some(produced.stderr);
        report.committed = commit(
            root,
            &Landing {
                target: &target,
                parent: &parent,
                expected: existing.as_deref(),
                short_input: &short,
                route,
            },
            cap,
            name,
            &landing,
        )?;
        steps.push(report);
    }

    let ran = steps.iter().filter(|s| s.outcome == Outcome::Ran).count();
    let skipped = steps
        .iter()
        .filter(|s| s.outcome == Outcome::Skipped)
        .count();
    let committed = steps.iter().filter(|s| s.committed.is_some()).count();
    Ok(RunReport {
        requested: step.map(str::to_string),
        dry_run,
        input_commit: short_of(root, &resolved_at),
        steps,
        ran,
        skipped,
        committed,
    })
}

/// Whether a step's committed result still stands, and the sentence that says why.
///
/// Two questions, asked in this order, and the second only where the first is satisfied.
///
/// **Has anything it reads moved?** The input state is a digest over the declaration and over
/// every file the declaration resolved to, so this is the equality check RFC-0026 §1 asks for
/// rather than a heuristic about mtimes.
///
/// **Has it stood too long anyway?** A calculator is a function of what it reads, so an
/// unchanged input state computes an unchanged answer and re-running it would land nothing.
/// Something that reads a world the repository does not hold is not, and `already_landed` —
/// which this replaced — named the case exactly: *"a calculator that is not a function of its
/// inputs — a clock, a random seed, a network read it did not declare — would otherwise have
/// its new output silently dropped on the grounds that its inputs had not moved."* That is now
/// a thing a corpus **declares**, with `ageing_days`, rather than a cost every step pays on
/// every invocation; and the interval is in the manifest because a number in this binary would
/// be one corpus's judgement arriving in another that never agreed to it.
enum Verdict {
    Fresh(String),
    Stale(String),
}

impl Verdict {
    fn freshness(&self) -> Freshness {
        match self {
            Self::Fresh(_) => Freshness::Fresh,
            Self::Stale(_) => Freshness::Stale,
        }
    }

    fn because(&self) -> &str {
        match self {
            Self::Fresh(why) | Self::Stale(why) => why,
        }
    }
}

fn freshness(
    root: &Path,
    cap: &Capability,
    parent: &str,
    receipt_path: &str,
    input_state: &str,
    today: i64,
) -> Verdict {
    let Ok(landed) = git(
        root,
        None,
        &["show", &format!("{parent}:{receipt_path}")],
        None,
    ) else {
        return Verdict::Stale("it has never run against this corpus".to_string());
    };
    if Receipt::committed_state(&landed).as_deref() != Some(input_state) {
        return Verdict::Stale(
            "the input state moved — what it reads, or what it declares, is not what the \
             committed receipt was computed from"
                .to_string(),
        );
    }

    let Some(ageing) = cap.ageing_days else {
        return Verdict::Fresh(
            "the input state is unchanged, and it declares no `ageing_days`".to_string(),
        );
    };
    // The conservative direction, which is `cmd/due.rs`'s rule for a clock it cannot read: a
    // corpus that asked to be told when this aged out and cannot be told is owed the run, not
    // reassured. Reporting it fresh would be the flattering direction to be wrong in.
    let Some(age) = receipt_age(root, parent, receipt_path, today) else {
        return Verdict::Stale(format!(
            "it declares `ageing_days = {ageing}` and its receipt carries no committed date \
             to measure against"
        ));
    };
    match age >= i64::from(ageing) {
        true => Verdict::Stale(format!(
            "the input state is unchanged, and it last ran {age} day(s) ago — past \
             `ageing_days = {ageing}`"
        )),
        false => Verdict::Fresh(format!(
            "the input state is unchanged, and it last ran {age} day(s) ago of \
             `ageing_days = {ageing}`"
        )),
    }
}

/// How many days ago this step's receipt was last committed.
///
/// The committer date of the commit that last touched the receipt, which is the clock the
/// receipt deliberately does not carry — *"the commit it lands in has a committer date, which
/// is the real one; a second date written into the file would be the same fact recorded twice
/// and the only copy anyone could forge."*
fn receipt_age(root: &Path, parent: &str, receipt_path: &str, today: i64) -> Option<i64> {
    let out = git(
        root,
        None,
        &[
            "log",
            "-1",
            "--date=short",
            "--format=%ad",
            parent,
            "--",
            receipt_path,
        ],
        None,
    )
    .ok()?;
    Some(today - crate::dates::days_from_civil_str(out.trim())?)
}

/// The commit a ref points at, or `None` where the ref does not exist.
fn tip_of(root: &Path, branch: &str) -> Option<String> {
    git(
        root,
        None,
        &[
            "rev-parse",
            "--verify",
            "--quiet",
            &format!("refs/heads/{branch}"),
        ],
        None,
    )
    .ok()
    .map(|s| s.trim().to_string())
    .filter(|s| !s.is_empty())
}

/// Where one run's commit lands, resolved before anything is written.
///
/// A struct rather than five arguments because four of them are shas and branch names, and the
/// one that matters — `expected` — is the compare-and-swap value. Passing those positionally is
/// how a caller eventually swaps two of them and the CAS starts checking the wrong ref.
struct Landing<'a> {
    /// The ref to move.
    target: &'a str,
    /// The commit the new one is built on.
    parent: &'a str,
    /// What `target` must currently point at, or `None` to require that it does not exist.
    expected: Option<&'a str>,
    /// The commit the step read, for the subject line.
    short_input: &'a str,
    route: Route,
}

/// Whether every byte this step produced is already what the parent commit holds.
///
/// Compared as git object ids rather than as text, which is the comparison `already_landed`
/// made before the freshness check replaced it, kept for the reason it gave: `git()` trims what
/// it reads, so a digest taken over that would call a file and the same file with a trailing
/// blank line identical — and `hash-object` is the identity the commit will be built from
/// anyway.
///
/// This decides what the report says, never whether the commit is written. A step that ran
/// because its corpus asked to be re-checked has looked, and the receipt that records it is
/// worth a commit whether or not the answer moved.
fn outputs_already_committed(root: &Path, parent: &str, outputs: &[(String, Vec<u8>)]) -> bool {
    !outputs.is_empty()
        && outputs.iter().all(|(path, bytes)| {
            let landed = git(
                root,
                None,
                &["rev-parse", &format!("{parent}:{path}")],
                None,
            );
            let produced = git(
                root,
                None,
                &["hash-object", "--stdin"],
                Some(&String::from_utf8_lossy(bytes)),
            );
            matches!((landed, produced), (Ok(a), Ok(b)) if a == b)
        })
}

/// The digest of a repository file, or of nothing where there is none.
///
/// A corpus with no `.yidam/config.toml` has a real input state rather than an unknown one,
/// and the digest of the empty string says so without a second representation for absence.
fn digest_of(root: &Path, rel: &str) -> String {
    sha256(&std::fs::read(root.join(rel)).unwrap_or_default())
}

/// Land the outputs and the receipt as one commit on `branch`.
///
/// Returns `None` when the resulting tree is the one already at `HEAD`. A re-run against an
/// unchanged input state produces byte-identical bytes — the receipt carries no clock, which
/// is what makes that true — and an empty commit per invocation would turn the log into a
/// record of how often somebody ran the command rather than of what changed.
fn commit(
    root: &Path,
    at: &Landing<'_>,
    cap: &Capability,
    step: &str,
    landing: &[(String, Vec<u8>)],
) -> Result<Option<Committed>> {
    let parent = at.parent;
    let scratch = TempIndex::new(root, "run")?;
    let index = scratch.path().to_path_buf();
    git(root, Some(&index), &["read-tree", parent], None)?;

    for (path, bytes) in landing {
        let content = String::from_utf8(bytes.clone()).with_context(|| {
            format!("`{path}` is not text, and a corpus commits text — see .yidam/vault for bytes")
        })?;
        let blob = git(
            root,
            Some(&index),
            &["hash-object", "-w", "--stdin"],
            Some(&content),
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
    }

    let tree = git(root, Some(&index), &["write-tree"], None)?;
    let head_tree = git(
        root,
        None,
        &["rev-parse", &format!("{parent}^{{tree}}")],
        None,
    )?;
    if tree == head_tree {
        return Ok(None);
    }

    let subject = subject(&cap.verb, step, landing.len(), at.short_input);
    let message = message(&subject, cap, step, landing, at.short_input, at.route);
    let sha = commit_tree(root, &tree, parent, &message, (AUTHOR_NAME, AUTHOR_EMAIL))?;

    // Compare-and-swap. `update-ref <ref> <new> <old>` refuses when the ref moved while the
    // step was running, which for a calculator is not a hypothetical interval. `expected` is
    // `None` only for a proposal branch this run is creating, and the all-zero sha is git's
    // spelling of *must not exist* — which is the same guarantee in the one case where there
    // is no old value to name.
    const ABSENT: &str = "0000000000000000000000000000000000000000";
    let target = at.target;
    git(
        root,
        None,
        &[
            "update-ref",
            &format!("refs/heads/{target}"),
            &sha,
            at.expected.unwrap_or(ABSENT),
        ],
        None,
    )
    .with_context(|| format!("{target} moved while `{step}` was running; nothing was landed"))?;

    Ok(Some(Committed {
        branch: target.to_string(),
        commit: short_of(root, &sha),
        subject,
    }))
}

/// The subject line, which `yidam lint --commits` reads.
///
/// The verb is the manifest's, checked against the operational family when the manifest
/// loaded — so a subject this builds is in the closed vocabulary by construction rather than
/// by a second check here agreeing with the first.
fn subject(verb: &str, step: &str, files: usize, short_parent: &str) -> String {
    format!(
        "{verb}: {step} — {files} file{} at {short_parent}",
        if files == 1 { "" } else { "s" }
    )
}

fn message(
    subject: &str,
    cap: &Capability,
    step: &str,
    landing: &[(String, Vec<u8>)],
    short_parent: &str,
    route: Route,
) -> String {
    let mut body = format!("{subject}\n\n");
    let _ = writeln!(
        body,
        "`{}` ran against {short_parent}, reading what `{step}` declares it reads and nothing\nelse from the repository.\n",
        cap.run.join(" ")
    );
    for (path, bytes) in landing {
        let _ = writeln!(body, "  {path}  sha256:{}", &sha256(bytes)[..16]);
    }
    let _ = write!(body, "\n{}", closing(route));
    body
}

/// The paragraph that says what kind of act this commit was, and what it is not.
///
/// Two of them, because the first one was true of every commit a run could write and is now
/// true of half. A reader meeting an `establish:` commit whose body says *"no understanding
/// changed"* would be reading the record of the exact act the vocabulary exists to mark.
fn closing(route: Route) -> &'static str {
    match route {
        Route::Branch => concat!(
            "Operational: the pipeline advanced and no understanding changed. Nothing was\n",
            "synthesized, no node was authored, and no claim was re-tagged — a run may author\n",
            "none of those on a branch, and the manifest that licensed this one could not have\n",
            "declared them.\n",
        ),
        Route::Proposal => concat!(
            "Epistemic: this proposes a change to what the corpus holds, and proposing is the\n",
            "whole of what it does. It is on a proposal branch because a run may not decide\n",
            "this; nothing merges itself, and until somebody reads it and merges it the corpus\n",
            "is unchanged. Review it, then merge or delete the branch.\n",
        ),
    }
}

/// The text report: the plan, in the order it was resolved, then what it left behind.
///
/// One block per step, because the two things a reader wants are per-step — *did this run, and
/// why* — and the two things they want after are about the repository. A single paragraph per
/// step, which is what the one-capability slice printed, stopped reading as a report the moment
/// there were three of them.
fn render(r: &RunReport) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "yidam run{} — {} step(s) in dependency order at {}\n",
        if r.dry_run { " --dry-run" } else { "" },
        r.steps.len(),
        r.input_commit
    );

    for s in &r.steps {
        let _ = writeln!(out, "  {:<8} {}", s.outcome.tag(), s.step);
        if !s.after.is_empty() {
            let _ = writeln!(out, "           after {}", s.after.join(", "));
        }
        let _ = writeln!(out, "           {}: {}", s.freshness.tag(), s.because);
        if let Some(err) = &s.step_stderr {
            let _ = writeln!(out, "           it said:\n{}", indent(err));
        }
        for p in &s.outputs {
            let _ = writeln!(
                out,
                "           {} {p}",
                if s.unchanged_outputs {
                    "unchanged"
                } else {
                    "wrote    "
                }
            );
        }
        if s.unchanged_outputs {
            let _ = writeln!(
                out,
                "           the answer had not moved; the commit records that it was checked"
            );
        }
        match &s.committed {
            Some(c) => {
                let _ = writeln!(out, "           {}  {}", c.commit, c.subject);
                let _ = writeln!(out, "           on {}", c.branch);
            }
            // Only ever reached for a step that ran: a skipped step wrote nothing to say this
            // about, and a planned one was never invoked.
            None if s.outcome == Outcome::Ran => {
                let _ = writeln!(
                    out,
                    "           it reproduced the tree already at {}, so no commit was written",
                    s.input_commit.as_deref().unwrap_or("HEAD")
                );
            }
            None => {}
        }
    }

    let _ = write!(out, "\n{}", summary(r));
    if r.dry_run {
        let _ = write!(
            out,
            "\n\nNothing was invoked and nothing was written. A step reported `unknown` reads \
             what an\nearlier step in this plan would write, so its input state does not exist \
             to compare against\nuntil that step has run."
        );
        return out;
    }

    // Both closings, and only for a route something actually landed on. A plan may be all
    // operational, all epistemic, or both, and a report that printed the `git restore` line
    // after a plan that took nothing from the checkout would be giving advice about a file
    // that is exactly where the reader left it.
    let landed_on = |route: Route| {
        r.steps
            .iter()
            .any(|s| s.route == route.as_str() && s.committed.is_some())
    };
    if landed_on(Route::Branch) {
        let taken: Vec<&str> = r
            .steps
            .iter()
            .filter(|s| s.route == Route::Branch.as_str() && s.committed.is_some())
            .flat_map(|s| {
                s.outputs
                    .iter()
                    .map(String::as_str)
                    .chain(std::iter::once(s.receipt.as_str()))
            })
            .collect();
        let _ = write!(
            out,
            "\n\nYour working tree and index were not touched, which means they are now behind \
             HEAD —\nthe paths above will read as deleted until you take them:\n\n    \
             git restore --source=HEAD --worktree --staged -- {}\n\n\
             Nothing was synthesized, no node was authored, and no claim was re-tagged.",
            taken.join(" ")
        );
    }
    if landed_on(Route::Proposal) {
        let mut branches: Vec<&str> = r
            .steps
            .iter()
            .filter(|s| s.route == Route::Proposal.as_str())
            .filter_map(|s| s.committed.as_ref())
            .map(|c| c.branch.as_str())
            .collect();
        // One name per branch. Every epistemic step in a plan lands on `propose/<head>` for
        // the head it was resolved against, so a plan of three names one branch three times.
        branches.dedup();
        let _ = write!(
            out,
            "\n\nAn epistemic verb proposes a change to what the corpus holds, so your branch \
             did not move\nand your checkout is unchanged. Those commits are on {}.\n\n    \
             git log --reverse {}..{}\n\n\
             Merge to accept, or delete the branch to reject. Nothing merges itself.",
            branches.join(", "),
            r.input_commit,
            branches.first().copied().unwrap_or_default()
        );
    }
    out
}

/// The one sentence a reader who skimmed the blocks should still be able to act on.
fn summary(r: &RunReport) -> String {
    if r.dry_run {
        let would = r
            .steps
            .iter()
            .filter(|s| s.outcome == Outcome::Planned)
            .count();
        return format!("{would} step(s) would run, {} would be skipped.", r.skipped);
    }
    let mut out = format!(
        "{} step(s) ran, {} skipped, {} commit(s) written.",
        r.ran, r.skipped, r.committed
    );
    // Said rather than left to the arithmetic. A run that invoked something and landed
    // nothing is the ordinary result of an ageing rule firing over a calculator whose answer
    // did not move, and a reader who has just watched three steps run needs to be told that
    // an empty log is what success looks like.
    if r.ran > 0 && r.committed < r.ran {
        let _ = write!(
            out,
            "\n{} step(s) ran and computed what was already committed, so nothing was written \
             for them — an\nempty commit per invocation would record how often you ran the \
             command rather than what changed.",
            r.ran - r.committed
        );
    }
    out
}

fn indent(text: &str) -> String {
    text.lines()
        .map(|l| format!("             {l}"))
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The subject a run writes is in the closed vocabulary and classifies as the family its
    /// verb belongs to, asserted through the parity function that decides it rather than by
    /// reading the format string.
    ///
    /// Both families, since [`Route`] made both declarable. The subject format is one string
    /// for both and the classifier is what has to tell them apart — so a subject that read as
    /// operational with an epistemic verb in it would route to a proposal branch and log as an
    /// advance, which is the invariant readable from the wrong end.
    #[test]
    fn the_subject_a_run_writes_classifies_as_its_verbs_family() {
        for (verbs, kind) in [
            (
                yidam_core::git::OPERATIONAL_VERBS,
                yidam_core::git::CommitKind::Operational,
            ),
            (
                yidam_core::git::EPISTEMIC_VERBS,
                yidam_core::git::CommitKind::Epistemic,
            ),
        ] {
            for verb in verbs {
                let s = subject(verb, "low-flow", 2, "abc1234");
                let e = yidam_core::git::classify_commit("h", &s);
                assert_eq!(e.verb, *verb);
                assert!(yidam_core::git::is_recognized_verb(&e.verb), "{s}");
                assert_eq!(e.kind, kind, "{s}");
            }
        }
    }

    #[test]
    fn one_file_is_not_pluralized() {
        assert_eq!(
            subject("compute", "low-flow", 1, "abc1234"),
            "compute: low-flow — 1 file at abc1234"
        );
    }

    /// One step of a plan, with everything the render branches on settable.
    fn step(
        name: &str,
        route: Route,
        verb: &str,
        outcome: Outcome,
        freshness: Freshness,
    ) -> StepReport {
        StepReport {
            step: name.into(),
            kind: "calculator",
            verb: verb.into(),
            route: route.as_str(),
            after: vec![],
            freshness,
            because: "why".into(),
            outcome,
            input_commit: Some("abc1234".into()),
            inputs: 3,
            outputs: vec![format!(".yidam/computed/{name}.yml")],
            receipt: format!(".yidam/runs/{name}.yml"),
            committed: (outcome == Outcome::Ran).then(|| Committed {
                branch: match route {
                    Route::Branch => "main".into(),
                    Route::Proposal => "propose/abc1234".into(),
                },
                commit: "def5678".into(),
                subject: format!("{verb}: {name} — 2 files at abc1234"),
            }),
            unchanged_outputs: false,
            step_stderr: None,
        }
    }

    fn plan(dry_run: bool, steps: Vec<StepReport>) -> RunReport {
        let ran = steps.iter().filter(|s| s.outcome == Outcome::Ran).count();
        let skipped = steps
            .iter()
            .filter(|s| s.outcome == Outcome::Skipped)
            .count();
        let committed = steps.iter().filter(|s| s.committed.is_some()).count();
        RunReport {
            requested: None,
            dry_run,
            input_commit: "abc1234".into(),
            steps,
            ran,
            skipped,
            committed,
        }
    }

    /// The two routes say different things, and the epistemic one says the branch did not move.
    ///
    /// Worth a test because the render branches on `route` and the operational closing ends
    /// with a `git restore` that is actively wrong advice for a proposal: nothing was taken
    /// from the checkout, so there is nothing to restore.
    #[test]
    fn an_epistemic_run_is_reported_as_a_proposal_and_not_as_a_branch_advance() {
        let out = render(&plan(
            false,
            vec![step(
                "low-flow",
                Route::Proposal,
                "establish",
                Outcome::Ran,
                Freshness::Stale,
            )],
        ));
        assert!(out.contains("propose/abc1234"), "{out}");
        assert!(out.contains("your branch did not move"), "{out}");
        assert!(out.contains("Nothing merges itself"), "{out}");
        assert!(
            !out.contains("git restore"),
            "a proposal took nothing from the checkout, so there is nothing to restore:\n{out}"
        );

        let out = render(&plan(
            false,
            vec![step(
                "low-flow",
                Route::Branch,
                "compute",
                Outcome::Ran,
                Freshness::Stale,
            )],
        ));
        assert!(out.contains("git restore"), "{out}");
        assert!(!out.contains("Nothing merges itself"), "{out}");
    }

    /// A plan that landed on both routes says both things, and the `git restore` names only
    /// what the branch took.
    ///
    /// The near-miss this is against: one closing chosen by the plan's *first* step, which
    /// would give a mixed plan a restore line listing paths that are on a proposal branch
    /// somebody has not merged.
    #[test]
    fn a_mixed_plan_reports_each_route_about_its_own_steps() {
        let out = render(&plan(
            false,
            vec![
                step(
                    "tier",
                    Route::Branch,
                    "compute",
                    Outcome::Ran,
                    Freshness::Stale,
                ),
                step(
                    "question",
                    Route::Proposal,
                    "establish",
                    Outcome::Ran,
                    Freshness::Stale,
                ),
            ],
        ));
        assert!(out.contains("git restore"), "{out}");
        assert!(out.contains("Nothing merges itself"), "{out}");
        assert!(
            out.contains(".yidam/computed/tier.yml"),
            "the restore line does not name what the branch took:\n{out}"
        );
        assert!(
            !out.contains(
                "git restore --source=HEAD --worktree --staged -- .yidam/computed/question.yml"
            ),
            "the restore line offers to take a file that is on a proposal branch:\n{out}"
        );
    }

    /// The definition of done's third bullet, at the render: which steps, and why.
    #[test]
    fn the_report_says_which_steps_ran_and_which_were_skipped_and_why() {
        let mut fresh = step(
            "upstream",
            Route::Branch,
            "compute",
            Outcome::Skipped,
            Freshness::Fresh,
        );
        fresh.because = "the input state is unchanged".into();
        fresh.outputs = vec![];
        let mut stale = step(
            "downstream",
            Route::Branch,
            "compute",
            Outcome::Ran,
            Freshness::Stale,
        );
        stale.because = "the input state moved".into();
        stale.after = vec!["upstream".into()];

        let out = render(&plan(false, vec![fresh, stale]));
        assert!(out.contains("skipped  upstream"), "{out}");
        assert!(out.contains("fresh: the input state is unchanged"), "{out}");
        assert!(out.contains("ran      downstream"), "{out}");
        assert!(out.contains("stale: the input state moved"), "{out}");
        assert!(out.contains("after upstream"), "{out}");
        assert!(
            out.contains("1 step(s) ran, 1 skipped, 1 commit(s) written"),
            "{out}"
        );
    }

    /// A dry run says what would happen and says that nothing did.
    ///
    /// The property that matters is the second half: a reader who ran this and then read a
    /// block listing a step and a verdict needs the report to be unambiguous that no commit
    /// exists — and a dry run never has one to print, so the sentence is what carries it.
    #[test]
    fn a_dry_run_reports_a_plan_and_says_nothing_was_written() {
        let mut planned = step(
            "tier",
            Route::Branch,
            "compute",
            Outcome::Planned,
            Freshness::Stale,
        );
        planned.outputs = vec![];
        let mut unknown = step(
            "envelope",
            Route::Branch,
            "compute",
            Outcome::Planned,
            Freshness::Unknown,
        );
        unknown.outputs = vec![];
        unknown.input_commit = None;
        unknown.because = "tier would run first and write what this reads".into();

        let out = render(&plan(true, vec![planned, unknown]));
        assert!(out.contains("yidam run --dry-run"), "{out}");
        assert!(out.contains("planned  tier"), "{out}");
        assert!(out.contains("unknown: tier would run first"), "{out}");
        assert!(out.contains("2 step(s) would run"), "{out}");
        assert!(
            out.contains("Nothing was invoked and nothing was written"),
            "{out}"
        );
        assert!(
            !out.contains("git restore"),
            "a dry run took nothing from the checkout:\n{out}"
        );
    }

    /// A step that ran and landed nothing says so rather than leaving a reader to subtract.
    #[test]
    fn a_step_that_reproduced_the_committed_tree_is_reported_as_having_landed_nothing() {
        let mut ran = step(
            "tier",
            Route::Branch,
            "compute",
            Outcome::Ran,
            Freshness::Stale,
        );
        ran.committed = None;
        let out = render(&plan(false, vec![ran]));
        assert!(out.contains("no commit was written"), "{out}");
        assert!(
            out.contains("1 step(s) ran, 0 skipped, 0 commit(s) written"),
            "{out}"
        );
        assert!(
            out.contains("computed what was already committed"),
            "the summary leaves the reader to subtract:\n{out}"
        );
    }

    /// No line of a commit body carries stray indentation, which is a guard and not style.
    ///
    /// The first version of [`closing`] lost its `\n\` line continuations to a patch script
    /// that read them as its own, and every line but the first came out with thirteen spaces
    /// in front of it. Nothing would have caught that: the message is never asserted on, and
    /// `git log` indents a body by four anyway, so the result reads as a quotation of
    /// something rather than as damage. The two-space file lines are the one deliberate
    /// indent, and they are exempted by name.
    #[test]
    fn no_line_of_a_commit_body_carries_stray_indentation() {
        let cap = Capability {
            kind: crate::cmd::run::manifest::Kind::Calculator,
            run: vec!["true".into()],
            reads: vec![],
            writes: vec![".yidam/computed/**".into()],
            verb: "compute".into(),
            after: vec![],
            ageing_days: None,
        };
        let landing = vec![(".yidam/computed/x.yml".to_string(), b"x".to_vec())];
        for route in [Route::Branch, Route::Proposal] {
            let body = message("compute: x", &cap, "x", &landing, "abc1234", route);
            for line in body.lines() {
                assert!(
                    !line.starts_with(' ') || line.starts_with("  ."),
                    "a body line is indented:\n{line:?}\nin:\n{body}"
                );
            }
        }
    }

    /// The two closings are two, and neither is true of the other route.
    #[test]
    fn the_commit_body_says_which_kind_of_act_it_was() {
        assert!(closing(Route::Branch).contains("no understanding changed"));
        assert!(closing(Route::Proposal).contains("proposes a change"));
        assert!(
            !closing(Route::Proposal).contains("no understanding changed"),
            "an epistemic commit must not carry the operational sentence, which is the exact \
             claim it falsifies"
        );
    }
}
