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
//! through. Each stale step is then invoked against **the tree the steps before it wrote**,
//! which is what makes `after` mean something rather than describe something: the upstream's
//! output is in the tree the downstream is given because the upstream's commit object was built
//! a moment earlier. No ref points at that object. The operational steps land together, one
//! commit per run of consecutive steps with one verb, and the branch moves once at the end
//! (RFC-0067). A plan in which any operational step fails lands nothing.
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
#[cfg(test)]
mod kit;
pub mod manifest;
pub mod notes;
pub mod outputs;
pub mod receipt;
/// The resolved corpus a step is handed beside its bytes (#1080).
pub mod resolved;

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::path::Path;

use anyhow::{bail, Context, Result};

use crate::cmd::propose::write::{commit_tree, current_branch, git, head, short_of, TempIndex};
use crate::paths::require_yidam_repo;
use manifest::{Capability, Manifest, Route};
use receipt::{sha256, File, Input, Receipt};

/// The author every commit a run writes carries.
///
/// Not `yidam propose`'s author, and the difference is the point of recording one at all: two
/// tools write commits here now, they are licensed by different rules, and a log that called
/// them the same author would have lost the only distinction the field exists to make.
pub(crate) const AUTHOR_NAME: &str = "yidam run";
pub(crate) const AUTHOR_EMAIL: &str = "run@yidam";

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
    /// The note written in place of a commit, where the step reproduced its committed receipt
    /// (RFC-0052). `None` for every other step, including one that committed.
    pub noted: Option<notes::Noted>,
    /// Whether the step ran and produced the bytes that were already committed.
    ///
    /// The ordinary result of an ageing rule firing over something whose answer had not moved,
    /// or of an input moving without effect, and a materially different event from a step that
    /// recomputed and found a change. Where the receipt it would write says what the committed
    /// one says outside the input state, no commit lands: the re-run is a line in the note on
    /// the committed receipt, which is the record of *having looked* ([`Self::noted`]). On a
    /// pending proposal it is not recorded (#1162): the receipt names the head the inputs came
    /// from, which has not moved, so it reproduces the proposal already there and no commit
    /// lands. `cmd/due.rs` draws the same distinction about a source's TTL — *"An expiry does
    /// not claim the upstream changed. It claims nobody has looked."* A report that said
    /// `wrote` here would be claiming the first thing while recording the second.
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
    /// How many steps landed a commit. Never more than `ran`, and fewer when a step reproduced
    /// its committed receipt and wrote a note instead (RFC-0052), or when an epistemic step
    /// reproduced the proposal already on `propose/<head>` (#1162).
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
    let root = crate::paths::working_root()?;
    require_yidam_repo(&root)?;
    crate::projection::refuse_if_projection(&root, "run")?;
    let report = plan_and_write(&root, step, opts.dry_run)?;
    crate::report::finish(&root, opts.format, report, |r| println!("{}", render(r)))
}

/// Everything between resolving the corpus and printing — the testable half.
///
/// `step` is what was asked for: one capability and everything it waits for, or the whole
/// manifest. Either way the plan is resolved once, before anything is invoked, so a manifest
/// that cannot be ordered stops the command rather than stopping it halfway through.
///
/// # The tip moves underneath this loop, and that is the mechanism
///
/// Each step is materialized from the batch's tip as it stands *when that step is reached*: the
/// commit the plan was resolved against, with every landing step before it built on top as a
/// commit object no ref points at (RFC-0067 §1). That is what makes `after` mean something:
/// the upstream's output is in the tree the downstream is given because the upstream's object
/// was built a moment earlier. The branch moves once, after the last operational step, and the
/// receipt names the base the step's group is built on, which is on the branch by then (§3).
/// A loop that pinned one input tree for the whole plan would hand every
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
    //
    // The reason comes from the declaration rather than from here, and it quotes back the part of
    // it that refused — see [`manifest::Capability::unrunnable_because`]. Three refusals share this
    // site and they are unrelated: a connector reaches a network, nobody built a featurizer's
    // executor, and a typed calculator needs a feature this build may not carry. A message written
    // once, in the words of whichever one was unrunnable when it was written, would send a
    // featurizer's author to read about credentials.
    //
    // Each step is collected with its reason rather than with its name alone, so that the reason
    // is the thing that put it in the list. Looking the declaration back up at the bail site would
    // make the reason an `Option` that cannot be `None` — a panic path, and an assertion about
    // this filter restated a few lines below it.
    let unrunnable: Vec<(&str, manifest::Unrunnable)> = plan
        .iter()
        .copied()
        .filter_map(|n| Some((n, m.get(n).ok()?.unrunnable_because()?)))
        .collect();
    if let Some((first, why)) = unrunnable.first() {
        let names: Vec<&str> = unrunnable.iter().map(|(n, _)| *n).collect();
        bail!(
            "`{first}` declares {}, which this binary does not invoke, so this plan cannot \
             run.\n  \
             {}.\n  \
             In this plan: {}",
            why.declared,
            why.because,
            names.join(", ")
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
    // builds what it wrote onto the batch's tip before moving to the next, so by the time a
    // dependent is reached its dependency's output is in the tree it is materialized from and
    // its input state is an ordinary thing to compute. That is the whole difference between the
    // two modes: a dry run cannot answer for a step downstream of one it did not run, and says
    // so rather than guessing.
    let mut pending: BTreeSet<&str> = BTreeSet::new();
    let mut steps: Vec<StepReport> = Vec::new();
    let mut batch = Batch::new(root, &resolved_at);

    for name in plan {
        let cap = m.get(name)?;
        let receipt_path = Receipt::path(name);
        let route = cap.route();

        // Epistemic steps are ordered last, so this is reached once every operational step
        // has run: the batch lands before the first of them, which is materialized from the
        // head it made (RFC-0067 §2).
        if route == Route::Proposal && !dry_run {
            batch.land(root, &branch, &mut steps)?;
        }

        // Where the step is materialized from, and the commit its receipt names. On the branch
        // these differ: the step reads the batch's tip, which no ref points at, and records the
        // base, which is on the branch once the plan lands (RFC-0067 §1, §3).
        let (at, input, short) = match route {
            Route::Branch => (
                batch.tip.clone(),
                batch.base.clone(),
                batch.short_base.clone(),
            ),
            Route::Proposal => {
                let (full, short) = head(root)?;
                (full.clone(), full, short)
            }
        };
        let target = match route {
            Route::Branch => branch.clone(),
            Route::Proposal => crate::cmd::propose::write::branch_for(&short),
        };
        let existing = match route {
            Route::Branch => Some(at.clone()),
            Route::Proposal => tip_of(root, &target),
        };
        let mut parent = existing.clone().unwrap_or_else(|| at.clone());

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
                noted: None,
                unchanged_outputs: false,
                step_stderr: None,
            });
            continue;
        }

        // On the branch, a step that does not finish lands nothing from the whole plan
        // (RFC-0067 §4), and the error says so rather than leaving a reader to look. Every
        // fallible call from here to the step's objects being built carries it, so a failure
        // reading its inputs says so as plainly as a failure running it.
        let lost = || match route {
            Route::Branch => format!("`{name}` did not finish, so nothing in this plan was landed"),
            Route::Proposal => format!("`{name}` did not finish"),
        };

        let state = |at: &str, parent: &str| -> Result<Prepared> {
            let inputs = exec::materialize(root, at, cap)?;
            let resolved_digest = inputs.resolved.as_ref().map(|r| r.sha256.clone());
            let script_digest = Receipt::script_sha256(cap, &inputs.files);
            let tables_digest = inputs.tables_sha256.clone();
            let input_state = Receipt::input_state(
                cap,
                &manifest_sha256,
                &config_sha256,
                &inputs.files,
                resolved_digest.as_deref(),
                script_digest.as_deref(),
                tables_digest.as_deref(),
            )?;
            let verdict = freshness(root, cap, parent, &receipt_path, &input_state, today);
            Ok(Prepared {
                inputs,
                resolved_digest,
                script_digest,
                tables_digest,
                input_state,
                verdict,
            })
        };
        let mut prepared = state(&at, &parent).with_context(lost)?;
        let (mut input, mut short) = (input, short);

        // A stale step with another verb than the open group's closes that group before it
        // runs. Its input commit has to be known when it is invoked, because it is exported to
        // the step, and the group commit it names is built here. The group commit carries the
        // fold, so the step is materialized again from it.
        if route == Route::Branch
            && !dry_run
            && prepared.verdict.freshness() == Freshness::Stale
            && batch.splits(&cap.verb)
        {
            batch.close(root).with_context(lost)?;
            input.clone_from(&batch.base);
            short.clone_from(&batch.short_base);
            parent.clone_from(&batch.tip);
            prepared = state(&batch.tip, &parent).with_context(lost)?;
        }
        let Prepared {
            inputs,
            resolved_digest,
            script_digest,
            tables_digest,
            input_state,
            verdict,
        } = prepared;

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
            noted: None,
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

        // Before the step is invoked, so a refused step costs nothing (#1452). Proposal-route
        // steps run last in plan order, so every branch-route step ahead of this one landed.
        if route == Route::Proposal && existing.is_none() {
            crate::cmd::propose::write::require_room(root, &target).with_context(|| {
                format!("`{name}` was not run; the steps ahead of it in the plan landed")
            })?;
        }
        exec::check_tables_read(&inputs).with_context(lost)?;
        let produced = exec::invoke(cap, &inputs, name, &input).with_context(lost)?;
        exec::check_declared(cap, &produced.outputs).with_context(lost)?;
        exec::check_tables_written(&inputs.tables, &produced.outputs).with_context(lost)?;
        outputs::check_readable(root, &parent, &produced.outputs).with_context(lost)?;

        let receipt = Receipt {
            format_version: receipt::FORMAT_VERSION,
            step: name.to_string(),
            kind: cap.kind.as_str(),
            verb: cap.verb.clone(),
            run: cap.run.clone(),
            input_state,
            input: Input {
                commit: input.clone(),
                manifest_sha256: manifest_sha256.clone(),
                config_sha256: config_sha256.clone(),
                reads: cap.reads.clone(),
                files: inputs.files.clone(),
                resolved_graph_sha256: resolved_digest.clone(),
                script_sha256: script_digest.clone(),
                tables_sha256: tables_digest.clone(),
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
            model: None,
            version: cap.run.gluon().and_then(|_| receipt::this_version()),
            config: None,
            image_digest: None,
        };

        report.outcome = Outcome::Ran;
        report.outputs = produced.outputs.iter().map(|(p, _)| p.clone()).collect();
        report.unchanged_outputs = outputs_already_committed(root, &parent, &produced.outputs);
        report.step_stderr = (!produced.stderr.is_empty()).then_some(produced.stderr);

        // RFC-0052 §2.2: a run that reproduced its committed receipt writes a note on it, not a
        // commit. On the branch, and on the proposal route only with no pending proposal, where
        // the parent is HEAD (§2.5). A pending proposal keeps #1162's behaviour: a note on its
        // receipt would vouch for a tree the branch does not hold.
        //
        // On the branch the note waits for the landing (RFC-0067 §2): a plan that lands nothing
        // writes no note, and the commit it records is the base, which is on the branch.
        //
        // An output the step declares under `stamps_input_commit` is compared with this run's
        // commit masked (§2.7), so a consumer that requires the stamp does not make every run
        // a commit. Where the masked run does not reproduce, the bytes land as produced.
        let may_note = route == Route::Branch || existing.is_none();
        let worth_reading = report.unchanged_outputs || !cap.stamps_input_commit.is_empty();
        if may_note && worth_reading {
            if let Some(blob) = notes::receipt_blob(root, &parent, &receipt_path) {
                let committed =
                    git(root, None, &["cat-file", "blob", &blob], None).with_context(lost)?;
                let reproduced = (report.unchanged_outputs && receipt.reproduces(&committed))
                    || reproduces_masked(
                        root,
                        &parent,
                        cap,
                        &receipt,
                        &produced.outputs,
                        &committed,
                    );
                if reproduced {
                    report.unchanged_outputs = true;
                    match route {
                        Route::Branch => batch.notes.push(PendingNote {
                            report: steps.len(),
                            blob,
                            input_state: receipt.input_state.clone(),
                            commit: input.clone(),
                        }),
                        Route::Proposal => {
                            report.noted = Some(notes::append(
                                root,
                                &blob,
                                &receipt.input_state,
                                &input,
                                (AUTHOR_NAME, AUTHOR_EMAIL),
                            )?);
                        }
                    }
                    steps.push(report);
                    continue;
                }
            }
        }

        let mut landing: Vec<(String, Vec<u8>)> = produced.outputs.clone();
        landing.push((
            receipt_path.clone(),
            receipt.to_yaml().with_context(lost)?.into_bytes(),
        ));
        match route {
            Route::Branch => {
                // Objects only, folded quietly, so a dependent later in the group reads the
                // REGEN blocks this step made stale as they will land rather than as they were.
                // The group commit folds again against the base, which is what lands and what
                // says so (RFC-0067 §1).
                if let Some(built) = build_step(
                    root,
                    &parent,
                    cap,
                    name,
                    &landing,
                    &short,
                    route,
                    Fold::Quiet,
                )
                .with_context(lost)?
                {
                    batch.add(steps.len(), cap, name, landing, built.sha);
                }
            }
            Route::Proposal => {
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
            }
        }
        steps.push(report);
    }
    if !dry_run {
        batch.land(root, &branch, &mut steps)?;
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

/// What a step's inputs resolved to, and whether its committed result still stands.
struct Prepared {
    inputs: exec::Inputs,
    resolved_digest: Option<String>,
    script_digest: Option<String>,
    tables_digest: Option<String>,
    input_state: String,
    verdict: Verdict,
}

/// The operational half of a plan, built as objects and landed with one ref move (RFC-0067).
///
/// The **tip** is what the next step is materialized from: every landing step so far, built
/// one on another, with no ref pointing at it. The **base** is the last group commit built, or
/// HEAD before the plan, and it is what a step's receipt names. A **group** is a run of
/// consecutive landing steps with one verb, and it lands as one commit on the base.
struct Batch {
    /// HEAD before the plan, which the one compare-and-swap expects the branch still to be.
    start: String,
    base: String,
    short_base: String,
    tip: String,
    open: Option<Group>,
    /// Each group commit built, with the steps it carries, in order.
    built: Vec<(Built, Vec<usize>)>,
    notes: Vec<PendingNote>,
}

struct Group {
    verb: String,
    members: Vec<Member>,
}

/// One step a group carries: its index in the report, and what it wrote.
struct Member {
    report: usize,
    step: String,
    shown: String,
    landing: Vec<(String, Vec<u8>)>,
}

/// A note a step earned, written only once the plan has landed.
struct PendingNote {
    report: usize,
    blob: String,
    input_state: String,
    commit: String,
}

impl Batch {
    fn new(root: &Path, head: &str) -> Self {
        Self {
            start: head.to_string(),
            base: head.to_string(),
            short_base: short_of(root, head),
            tip: head.to_string(),
            open: None,
            built: Vec::new(),
            notes: Vec::new(),
        }
    }

    /// Whether a step with `verb` would end the open group.
    fn splits(&self, verb: &str) -> bool {
        self.open.as_ref().is_some_and(|g| g.verb != verb)
    }

    /// A landing step, whose objects-only commit `sha` is the new tip.
    fn add(
        &mut self,
        report: usize,
        cap: &Capability,
        step: &str,
        landing: Vec<(String, Vec<u8>)>,
        sha: String,
    ) {
        let group = self.open.get_or_insert_with(|| Group {
            verb: cap.verb.clone(),
            members: Vec::new(),
        });
        group.members.push(Member {
            report,
            step: step.to_string(),
            shown: cap.run.shown(),
            landing,
        });
        self.tip = sha;
    }

    /// Build the open group's commit on the base, which then becomes the base and the tip.
    fn close(&mut self, root: &Path) -> Result<()> {
        let Some(group) = self.open.take() else {
            return Ok(());
        };
        let built = build_group(root, &self.base, &self.short_base, &group).with_context(|| {
            format!(
                "the `{}` group's commit could not be built, so nothing in this plan was landed",
                group.verb
            )
        })?;
        if let Some(built) = built {
            self.base.clone_from(&built.sha);
            self.short_base = short_of(root, &built.sha);
            self.built
                .push((built, group.members.iter().map(|m| m.report).collect()));
        }
        self.tip.clone_from(&self.base);
        Ok(())
    }

    /// Move the branch once, then write the notes the plan earned.
    ///
    /// What landed is then where the batch starts, so a second call with nothing new built
    /// moves nothing.
    fn land(&mut self, root: &Path, branch: &str, steps: &mut [StepReport]) -> Result<()> {
        self.close(root)?;
        let moved = || format!("{branch} moved while the plan was running; nothing was landed");
        // Where the branch is once the ref moves, for a note that fails after it has: the
        // commits are on the branch by then, and an error that read as a failed plan would
        // send somebody to re-run what already landed.
        let mut landed: Option<String> = None;
        if let Some(last) = self.built.last().map(|(b, _)| b.sha.clone()) {
            git(
                root,
                None,
                &[
                    "update-ref",
                    &format!("refs/heads/{branch}"),
                    &last,
                    &self.start,
                ],
                None,
            )
            .with_context(moved)?;
            landed = Some(short_of(root, &last));
            self.start = last;
            for (built, members) in std::mem::take(&mut self.built) {
                let commit = short_of(root, &built.sha);
                for i in members {
                    steps[i].committed = Some(Committed {
                        branch: branch.to_string(),
                        commit: commit.clone(),
                        subject: built.subject.clone(),
                    });
                }
            }
        } else if !self.notes.is_empty() {
            // Nothing to land, and the notes still vouch for the branch as it was. So it must
            // still be there.
            let now = tip_of(root, branch).unwrap_or_default();
            if now != self.start {
                bail!("{}", moved());
            }
        }
        for note in std::mem::take(&mut self.notes) {
            let step = &steps[note.report].step;
            let noted = notes::append(
                root,
                &note.blob,
                &note.input_state,
                &note.commit,
                (AUTHOR_NAME, AUTHOR_EMAIL),
            )
            .with_context(|| match &landed {
                Some(at) => format!(
                    "{branch} moved to {at} and the plan's commits landed, but a note for \
                     `{step}` could not be written to {}",
                    notes::REF
                ),
                None => format!(
                    "a note for `{step}` could not be written to {}; the branch did not move",
                    notes::REF
                ),
            })?;
            steps[note.report].noted = Some(noted);
        }
        Ok(())
    }
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
pub(crate) enum Verdict {
    Fresh(String),
    Stale(String),
}

impl Verdict {
    pub(crate) fn freshness(&self) -> Freshness {
        match self {
            Self::Fresh(_) => Freshness::Fresh,
            Self::Stale(_) => Freshness::Stale,
        }
    }

    pub(crate) fn because(&self) -> &str {
        match self {
            Self::Fresh(why) | Self::Stale(why) => why,
        }
    }
}

pub(crate) fn freshness(
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
    let basis = if Receipt::committed_state(&landed).as_deref() == Some(input_state) {
        Basis::Receipt
    } else if notes::receipt_blob(root, parent, receipt_path)
        .is_some_and(|blob| notes::lists(root, &blob, input_state))
    {
        Basis::Note
    } else {
        return Verdict::Stale(
            "the input state moved — what it reads, or what it declares, is not what the \
             committed receipt was computed from"
                .to_string(),
        );
    };

    // The conservative direction, which is `cmd/due.rs`'s rule for a clock it cannot read: a
    // corpus that asked to be told when this aged out and cannot be told is owed the run, not
    // reassured. Reporting it fresh would be the flattering direction to be wrong in.
    aged(root, cap, parent, receipt_path, today, basis)
}

/// Why the input state stands: it is the committed receipt's own, or it is one the note on that
/// receipt lists as having reproduced it (RFC-0052 §2.3).
#[derive(Clone, Copy)]
pub(crate) enum Basis {
    Receipt,
    Note,
}

impl Basis {
    fn because(self) -> &'static str {
        match self {
            Self::Receipt => "the input state is unchanged",
            Self::Note => {
                "the input state moved, and a re-run against it reproduced the committed receipt \
                 (noted in refs/notes/yidam-runs)"
            }
        }
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
pub(crate) fn tip_of(root: &Path, branch: &str) -> Option<String> {
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
/// Where each declared step stands against the tree you are standing in, for a reader.
///
/// `doctor` asks this, and it is a deliberately *different* question from the one
/// [`plan_and_write`] asks. A run resolves both halves against **HEAD**, because a step is
/// invoked in a tree checked out of a commit. Somebody running `doctor` is standing in a
/// checkout with edits in it, and the useful question there is *"is what is on disk still what
/// the committed answer was computed from"* — so the receipt comes from HEAD and the input
/// state comes from `git ls-files` and the bytes on disk.
///
/// **The two sides are deliberately not both read from the same place.** A receipt read off
/// disk would answer "has never run" for the whole of the interval between a run and the
/// `git restore` that syncs a checkout back up to it — the run commits and does not touch the
/// tree, by the rule in this module's header — which is the one moment the question is most
/// likely to be asked and the answer that is least true. Reading it from HEAD makes that
/// interval report what it actually is: the answer stands, and the files it produced are not in
/// your checkout.
///
/// It is one rule and not a second approximation of the first: the digest is
/// [`Receipt::input_state`], the same function the executor commits, fed the working tree
/// instead of a materialized scratch. Nothing here invokes anything, writes anything, or
/// touches `.git/index`.
///
/// A repository that declares no capabilities answers `Ok(vec![])` rather than an error. That
/// is every repository but one today, and a check that reported a missing manifest as a
/// problem would be a finding against fifteen of sixteen derived corpora for not using a
/// feature.
pub(crate) fn standing(root: &Path) -> Result<Vec<Standing>> {
    if !root.join(manifest::MANIFEST).exists() {
        return Ok(Vec::new());
    }
    let m = Manifest::load(root)?;
    let manifest_sha256 = digest_of(root, manifest::MANIFEST);
    let config_sha256 = digest_of(root, ".yidam/config.toml");
    let today = crate::dates::today_days();
    let tracked = tracked_paths(root);
    let held: BTreeSet<&str> = tracked.iter().map(String::as_str).collect();
    let read = crate::corpus::Corpus::open(root);
    // The declarations the run would read out of HEAD's catalog, read here out of the working
    // tree as everything else in this function is (RFC-0059 §2.4).
    let declared = crate::table::declarations(read.sources());

    let mut out = Vec::new();
    for name in m.plan(None)? {
        let cap = m.get(name)?;
        let receipt_path = Receipt::path(name);
        let landed = git(root, None, &["show", &format!("HEAD:{receipt_path}")], None)
            .ok()
            .as_deref()
            .and_then(Receipt::landed);
        let files = resolve_reads(root, cap, &tracked);
        // The same document the run would hand this step, built by the same function over the
        // working tree — see [`resolved::build`] for why it must be the same function and not a
        // second reading. `read` is opened once outside the loop: every step asks about the
        // same corpus, and `Corpus` caches its walk.
        let resolved_digest = resolved::build(
            &read,
            &|p| cap.reads.iter().any(|g| crate::kuten::glob_covers(g, p)),
            &|p| held.contains(p),
        )
        .map(|text| sha256(text.as_bytes()));
        let input_state = Receipt::input_state(
            cap,
            &manifest_sha256,
            &config_sha256,
            &files,
            resolved_digest.as_deref(),
            Receipt::script_sha256(cap, &files).as_deref(),
            crate::table::input_digest(&declared, &|p| files.iter().any(|f| f.path == p), &|p| {
                cap.writes.iter().any(|g| crate::kuten::glob_covers(g, p))
            })
            .as_deref(),
        )?;

        let verdict = match &landed {
            // Before "never run", which is true and sends its reader to run it (#1184).
            None if matches!(cap.run, manifest::Run::Unbuilt) => Verdict::Stale(
                "it is declared and not built — it has no `run`, so nothing can compute it"
                    .to_string(),
            ),
            None => Verdict::Stale("it has never run against this corpus".to_string()),
            Some(l) if l.input_state.as_deref() == Some(input_state.as_str()) => {
                aged(root, cap, "HEAD", &receipt_path, today, Basis::Receipt)
            }
            Some(_)
                if notes::receipt_blob(root, "HEAD", &receipt_path)
                    .is_some_and(|blob| notes::lists(root, &blob, &input_state)) =>
            {
                aged(root, cap, "HEAD", &receipt_path, today, Basis::Note)
            }
            Some(_) => Verdict::Stale(
                "what it reads, or what it declares, is not what its receipt was computed from"
                    .to_string(),
            ),
        };

        let outputs = landed
            .as_ref()
            .map(|l| {
                l.outputs
                    .iter()
                    .map(|f| Output {
                        path: f.path.clone(),
                        unchanged: std::fs::read(root.join(&f.path))
                            .ok()
                            .map(|bytes| sha256(&bytes) == f.sha256),
                    })
                    .collect()
            })
            .unwrap_or_default();

        out.push(Standing {
            step: name.to_string(),
            freshness: verdict.freshness(),
            because: verdict.because().to_string(),
            outputs,
        });
    }
    Ok(out)
}

/// Every committed output whose bytes contain its receipt's full `input.commit` (RFC-0052 §2.6).
///
/// Read from HEAD on both sides, because the question is about what was committed: an output
/// that stamps the commit it was computed from is new at every run, so it hides whether the
/// answer moved and defeats the note a run writes when it reproduced its receipt. The receipt
/// already records the commit, and that is where a reader should find it.
///
/// Returned as `step: path`, split by whether the step names the path under
/// `stamps_input_commit` (RFC-0052 §2.7): a declared stamp is one a consumer requires, and
/// [`run`] masks it before judging reproduction. Empty for a repository that declares no
/// capabilities.
pub(crate) fn stamped(root: &Path) -> Result<Stamped> {
    let mut out = Stamped::default();
    if !root.join(manifest::MANIFEST).exists() {
        return Ok(out);
    }
    let m = Manifest::load(root)?;
    for name in m.plan(None)? {
        let declared = &m.get(name)?.stamps_input_commit;
        let receipt_path = Receipt::path(name);
        let Some(landed) = git(root, None, &["show", &format!("HEAD:{receipt_path}")], None)
            .ok()
            .as_deref()
            .and_then(Receipt::landed)
        else {
            continue;
        };
        // The full sha only: an abbreviation is short enough to occur by chance in a digest.
        let Some(commit) = landed
            .input
            .and_then(|i| i.commit)
            .filter(|c| c.len() >= 40)
        else {
            continue;
        };
        for f in &landed.outputs {
            let held = git(root, None, &["show", &format!("HEAD:{}", f.path)], None);
            if held.is_ok_and(|text| text.contains(&commit)) {
                let found = format!("{name}: {}", f.path);
                match declared.contains(&f.path) {
                    true => out.declared.push(found),
                    false => out.undeclared.push(found),
                }
            }
        }
    }
    Ok(out)
}

/// What [`stamped`] found: outputs whose step declares the stamp, and outputs whose step does
/// not.
#[derive(Debug, Default)]
pub(crate) struct Stamped {
    pub declared: Vec<String>,
    pub undeclared: Vec<String>,
}

/// What a reader needs to know about one declared step. See [`standing`].
pub(crate) struct Standing {
    pub step: String,
    pub freshness: Freshness,
    pub because: String,
    pub outputs: Vec<Output>,
}

/// One file a step's receipt says it wrote, and whether it is still those bytes.
pub(crate) struct Output {
    pub path: String,
    /// Whether the file on disk is the bytes the receipt recorded. `None` where it is gone —
    /// which is not the same finding as an edit, and a boolean would have to call it one.
    pub unchanged: Option<bool>,
}

/// Every path this repository tracks, as `exec::materialize` lists them.
///
/// Empty where git cannot answer, which makes every step's input state the digest of an empty
/// file list — stale rather than fresh. That is [`freshness`]'s direction to be wrong in.
fn tracked_paths(root: &Path) -> Vec<String> {
    git(root, None, &["ls-files", "-z"], None)
        .map(|listed| {
            listed
                .split('\0')
                .filter(|p| !p.is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

/// The declared `reads`, resolved against the working tree rather than against a commit.
///
/// Sorted by path and hashed in that order, because [`Receipt::input_state`] digests the list
/// as given and `exec::materialize` sorts before it hashes. A digest taken in a different
/// order would report every step stale forever, and it would look like a freshness bug rather
/// than a hashing one.
///
/// A tracked file that is not on disk hashes as empty, for [`digest_of`]'s reason: a deleted
/// input is a real input state and a different one, and it needs no second representation.
fn resolve_reads(root: &Path, cap: &Capability, tracked: &[String]) -> Vec<File> {
    let mut wanted: Vec<&String> = tracked
        .iter()
        .filter(|p| cap.reads.iter().any(|g| crate::kuten::glob_covers(g, p)))
        .collect();
    wanted.sort();
    wanted
        .into_iter()
        .map(|path| File {
            sha256: sha256(&std::fs::read(root.join(path)).unwrap_or_default()),
            path: path.clone(),
        })
        .collect()
}

/// The `ageing_days` arm of [`freshness`], asked of a receipt whose input state already matched.
///
/// Split out so [`standing`] and [`freshness`] share it rather than each having a reading of
/// the declaration. The age is measured from the later of two dates git wrote: the commit that
/// last touched the receipt, and the notes commit that last appended to the note on it
/// (RFC-0052 §2.3) — an uncommitted receipt has no date, and the conservative answer is the run.
fn aged(
    root: &Path,
    cap: &Capability,
    parent: &str,
    receipt_path: &str,
    today: i64,
    basis: Basis,
) -> Verdict {
    let unchanged = basis.because();
    let Some(ageing) = cap.ageing_days else {
        return Verdict::Fresh(format!("{unchanged}, and it declares no `ageing_days`"));
    };
    let noted = notes::receipt_blob(root, parent, receipt_path)
        .and_then(|blob| notes::age(root, &blob, today));
    let Some(age) = [receipt_age(root, parent, receipt_path, today), noted]
        .into_iter()
        .flatten()
        .min()
    else {
        return Verdict::Stale(format!(
            "it declares `ageing_days = {ageing}` and its receipt carries no committed date \
             to measure against"
        ));
    };
    match age >= i64::from(ageing) {
        true => Verdict::Stale(format!(
            "{unchanged}, and it last ran {age} day(s) ago — past `ageing_days = {ageing}`"
        )),
        false => Verdict::Fresh(format!(
            "{unchanged}, and it last ran {age} day(s) ago of `ageing_days = {ageing}`"
        )),
    }
}

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

/// Whether this run reproduces `committed` once each output the step declares under
/// `stamps_input_commit` has this run's commit replaced by the one `committed` records
/// (RFC-0052 §2.7).
///
/// Both of §2.2's tests, over the masked bytes: they must be what the parent holds, and the
/// receipt built from their digests must say what the committed one says. Only the full sha is
/// replaced, for the reason [`stamped`] reads only the full sha. A step that declares nothing,
/// or a committed receipt that records no full commit, answers `false`, which commits.
fn reproduces_masked(
    root: &Path,
    parent: &str,
    cap: &Capability,
    receipt: &Receipt,
    outputs: &[(String, Vec<u8>)],
    committed: &str,
) -> bool {
    let now = receipt.input.commit.as_bytes();
    let Some(was) = Receipt::landed(committed)
        .and_then(|l| l.input)
        .and_then(|i| i.commit)
        .filter(|c| c.len() >= 40)
    else {
        return false;
    };
    if cap.stamps_input_commit.is_empty() || now.len() < 40 {
        return false;
    }
    let masked: Vec<(String, Vec<u8>)> = outputs
        .iter()
        .map(
            |(path, bytes)| match cap.stamps_input_commit.contains(path) {
                true => (path.clone(), replace_all(bytes, now, was.as_bytes())),
                false => (path.clone(), bytes.clone()),
            },
        )
        .collect();
    if !outputs_already_committed(root, parent, &masked) {
        return false;
    }
    let mut ours = receipt.clone();
    ours.outputs = masked
        .iter()
        .map(|(path, bytes)| File {
            path: path.clone(),
            sha256: sha256(bytes),
        })
        .collect();
    ours.reproduces(committed)
}

/// `hay` with every occurrence of `from` replaced by `to`. `from` is never empty here.
fn replace_all(hay: &[u8], from: &[u8], to: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(hay.len());
    let mut i = 0;
    while i < hay.len() {
        match !from.is_empty() && hay[i..].starts_with(from) {
            true => {
                out.extend_from_slice(to);
                i += from.len();
            }
            false => {
                out.push(hay[i]);
                i += 1;
            }
        }
    }
    out
}

/// Whether every byte this step produced is already what the parent commit holds.
///
/// Compared as git object ids rather than as text, which is the comparison `already_landed`
/// made before the freshness check replaced it, kept for the reason it gave: `git()` trims what
/// it reads, so a digest taken over that would call a file and the same file with a trailing
/// blank line identical — and `hash-object` is the identity the commit will be built from
/// anyway.
///
/// This is the first of RFC-0052 §2.2's tests, and alone it is not enough to skip a commit: the
/// receipt the run would write must also say what the committed one says outside the input
/// state ([`Receipt::reproduces`]). Where both hold, the re-run is a note on the committed
/// receipt, so the check is recorded without a commit whose only news is the head. On a pending
/// proposal the head has not moved, the tree is the one already proposed, and nothing lands
/// (#1162). The pending proposal is the answer until a person acts on it.
pub(crate) fn outputs_already_committed(
    root: &Path,
    parent: &str,
    outputs: &[(String, Vec<u8>)],
) -> bool {
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
pub(crate) fn digest_of(root: &Path, rel: &str) -> String {
    sha256(&std::fs::read(root.join(rel)).unwrap_or_default())
}

/// A commit that exists as an object and is pointed at by nothing.
///
/// The half of [`commit`] that needs no permission, split out for `cmd/cluster`: a step pod
/// builds one of these and hands its sha to a lander, and the lander is the only process that
/// holds a credential for the ref. What this returns is a fact about the object store; what
/// [`commit`] adds is the one ref write, and the two are separated so that separation can be
/// deployed rather than described (RFC-0026 §7).
pub(crate) struct Built {
    pub sha: String,
    pub subject: String,
}

/// Build the outputs and the receipt as one commit object on `parent`, writing no ref.
///
/// Returns `None` when the resulting tree is the one already at `parent`. A re-run against an
/// unchanged input state produces byte-identical bytes — the receipt carries no clock, which
/// is what makes that true — and an empty commit per invocation would turn the log into a
/// record of how often somebody ran the command rather than of what changed.
///
/// With `fold`, a commit that does change the tree also carries the REGEN blocks it makes
/// stale, refreshed (RFC-0054 §2.2), so no `regen:` commit has to follow it. A fold that
/// cannot be made — a shallow clone, a generator that fails — lands the step without it and
/// says so: a step that computed correctly is not lost to a README.
#[allow(clippy::too_many_arguments)]
pub(crate) fn build_commit(
    root: &Path,
    parent: &str,
    cap: &Capability,
    step: &str,
    landing: &[(String, Vec<u8>)],
    short_input: &str,
    route: Route,
    fold: bool,
) -> Result<Option<Built>> {
    build_step(
        root,
        parent,
        cap,
        step,
        landing,
        short_input,
        route,
        if fold { Fold::Said } else { Fold::Off },
    )
}

/// Whether a commit refreshes the REGEN blocks it makes stale, and whether a fold that cannot be
/// made is said on stderr.
///
/// `Quiet` is for a group member's objects-only commit (RFC-0067 §1): the group commit folds
/// again against the base, and that is the one whose failure a reader is told about, once.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Fold {
    Off,
    Said,
    Quiet,
}

/// [`build_commit`], with the fold spelled as a [`Fold`].
#[allow(clippy::too_many_arguments)]
fn build_step(
    root: &Path,
    parent: &str,
    cap: &Capability,
    step: &str,
    landing: &[(String, Vec<u8>)],
    short_input: &str,
    route: Route,
    fold: Fold,
) -> Result<Option<Built>> {
    let subject = subject(&cap.verb, step, landing.len(), short_input);
    let message = message(
        &subject,
        &cap.run.shown(),
        step,
        landing,
        short_input,
        route,
    );
    build(root, parent, landing, fold, subject, message)
}

/// A group of landing steps as one commit on `base` (RFC-0067 §2).
///
/// Its tree is `base` with every file the group wrote, the last writer winning, and one REGEN
/// fold over all of them against `base`. A group of one reads as the commit that step would
/// have landed alone.
fn build_group(root: &Path, base: &str, short_base: &str, group: &Group) -> Result<Option<Built>> {
    let mut landing: Vec<(String, Vec<u8>)> = Vec::new();
    for member in &group.members {
        for (path, bytes) in &member.landing {
            landing.retain(|(p, _)| p != path);
            landing.push((path.clone(), bytes.clone()));
        }
    }
    let (subject, message) = match group.members.as_slice() {
        [one] => {
            let subject = subject(&group.verb, &one.step, one.landing.len(), short_base);
            let message = message(
                &subject,
                &one.shown,
                &one.step,
                &one.landing,
                short_base,
                Route::Branch,
            );
            (subject, message)
        }
        members => {
            let subject = format!(
                "{}: {} steps — {} file{} at {short_base}",
                group.verb,
                members.len(),
                landing.len(),
                if landing.len() == 1 { "" } else { "s" }
            );
            let mut body = format!("{subject}\n\n");
            let _ = writeln!(
                body,
                "{} steps ran in this order. The first ran against {short_base}; each later step\nran against {short_base} and what the steps above it wrote. Each read what it\ndeclares it reads and nothing else from the repository.\n",
                members.len()
            );
            for (i, member) in members.iter().enumerate() {
                let _ = writeln!(body, "`{}` ({}):", member.shown, member.step);
                for (path, bytes) in &member.landing {
                    // A path a later step in the group wrote again is not in this commit's tree
                    // with these bytes, so the digest is marked rather than read as landed.
                    let later = members[i + 1..]
                        .iter()
                        .rev()
                        .find(|m| m.landing.iter().any(|(p, _)| p == path));
                    let _ = match later {
                        Some(m) => writeln!(
                            body,
                            "  {path}  sha256:{}  (replaced by `{}`)",
                            &sha256(bytes)[..16],
                            m.step
                        ),
                        None => writeln!(body, "  {path}  sha256:{}", &sha256(bytes)[..16]),
                    };
                }
                body.push('\n');
            }
            body.push_str(closing(Route::Branch));
            (subject, body)
        }
    };
    build(root, base, &landing, Fold::Said, subject, message)
}

/// `landing` on `parent` as one commit object, with `fold`'s REGEN paragraph after `message`.
fn build(
    root: &Path,
    parent: &str,
    landing: &[(String, Vec<u8>)],
    fold: Fold,
    subject: String,
    mut message: String,
) -> Result<Option<Built>> {
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

    let folded = if fold == Fold::Off {
        Vec::new()
    } else {
        fold_blocks(root, &index, parent, landing, fold == Fold::Said)?
    };
    let tree = if folded.is_empty() {
        tree
    } else {
        git(root, Some(&index), &["write-tree"], None)?
    };

    if !folded.is_empty() {
        let _ = write!(
            message,
            "\nREGEN blocks this commit made stale, refreshed in it:\n\n{}\n",
            folded
                .iter()
                .map(|l| format!("  {l}"))
                .collect::<Vec<_>>()
                .join("\n")
        );
    }
    let sha = commit_tree(root, &tree, parent, &message, (AUTHOR_NAME, AUTHOR_EMAIL))?;
    Ok(Some(Built { sha, subject }))
}

/// Refresh the REGEN blocks `landing` makes stale into `index`, and return one line per block.
///
/// Empty when nothing is stale, and when no fold can be made — which, with `say`, is said on
/// stderr with the command that makes it afterwards, because stdout may be a JSON report.
fn fold_blocks(
    root: &Path,
    index: &Path,
    parent: &str,
    landing: &[(String, Vec<u8>)],
    say: bool,
) -> Result<Vec<String>> {
    let fold = match crate::cmd::fold_for(root, parent, landing) {
        Ok(Some(fold)) => fold,
        Ok(None) if !say => return Ok(Vec::new()),
        Err(_) if !say => return Ok(Vec::new()),
        Ok(None) => {
            eprintln!(
                "note: a shallow clone cannot generate REGEN blocks, so this step landed without \
                 them. After `git fetch --unshallow`, sync the checkout and run \
                 `yidam regen --fold`."
            );
            return Ok(Vec::new());
        }
        Err(e) => {
            eprintln!(
                "note: the REGEN blocks could not be refreshed for this step ({e:#}), so it \
                 landed without them. Sync the checkout and run `yidam regen --fold`."
            );
            return Ok(Vec::new());
        }
    };
    for (path, bytes) in &fold.files {
        let content = String::from_utf8(bytes.clone())
            .with_context(|| format!("`{path}` holds a REGEN block and is not text"))?;
        let blob = git(
            root,
            Some(index),
            &["hash-object", "-w", "--stdin"],
            Some(&content),
        )?;
        git(
            root,
            Some(index),
            &[
                "update-index",
                "--add",
                "--cacheinfo",
                &format!("100644,{blob},{path}"),
            ],
            None,
        )?;
    }
    Ok(fold.lines)
}

/// Land the outputs and the receipt as one commit on `branch`.
///
/// [`build_commit`], then the one ref write. `None` for the reason given there.
fn commit(
    root: &Path,
    at: &Landing<'_>,
    cap: &Capability,
    step: &str,
    landing: &[(String, Vec<u8>)],
) -> Result<Option<Committed>> {
    let Some(built) = build_commit(
        root,
        at.parent,
        cap,
        step,
        landing,
        at.short_input,
        at.route,
        true,
    )?
    else {
        return Ok(None);
    };

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
            &built.sha,
            at.expected.unwrap_or(ABSENT),
        ],
        None,
    )
    .with_context(|| format!("{target} moved while `{step}` was running; nothing was landed"))?;

    Ok(Some(Committed {
        branch: target.to_string(),
        commit: short_of(root, &built.sha),
        subject: built.subject,
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
    shown: &str,
    step: &str,
    landing: &[(String, Vec<u8>)],
    short_parent: &str,
    route: Route,
) -> String {
    let mut body = format!("{subject}\n\n");
    let _ = writeln!(
        body,
        "`{shown}` ran against {short_parent}, reading what `{step}` declares it reads and nothing\nelse from the repository.\n"
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
        match &s.committed {
            Some(c) => {
                if s.unchanged_outputs {
                    let _ = writeln!(
                        out,
                        "           the outputs had not moved and the receipt did, so the receipt \
                         was committed"
                    );
                }
                let _ = writeln!(out, "           {}  {}", c.commit, c.subject);
                let _ = writeln!(out, "           on {}", c.branch);
            }
            // Only ever reached for a step that ran: a skipped step wrote nothing to say this
            // about, and a planned one was never invoked.
            //
            // Named by where the tree was compared, which is not the same commit on both routes
            // (#1162). An epistemic step lands on top of the proposal it already made, so what it
            // reproduced is that proposal's tip, not the head its inputs came from.
            None if s.noted.is_some() => {
                if let Some(n) = &s.noted {
                    let _ = writeln!(
                        out,
                        "           it reproduced its committed receipt, so no commit was written"
                    );
                    let _ = writeln!(
                        out,
                        "           noted on receipt {} in {} ({})",
                        n.receipt_blob, n.notes_ref, n.commit
                    );
                }
            }
            None if s.outcome == Outcome::Ran => {
                let short = s.input_commit.as_deref().unwrap_or("HEAD");
                let _ = match s.route == Route::Proposal.as_str() {
                    true => writeln!(
                        out,
                        "           it reproduced what {} already proposes, so no commit was \
                         written",
                        crate::cmd::propose::write::branch_for(short)
                    ),
                    false => writeln!(
                        out,
                        "           it reproduced the tree already at {short}, so no commit was \
                         written"
                    ),
                };
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
    // Steps in one group share a commit (RFC-0067 §2), so commits are counted by sha.
    let commits: BTreeSet<&str> = r
        .steps
        .iter()
        .filter_map(|s| s.committed.as_ref())
        .map(|c| c.commit.as_str())
        .collect();
    let mut out = format!(
        "{} step(s) ran, {} skipped, {} commit(s) written.",
        r.ran,
        r.skipped,
        commits.len()
    );
    // Said rather than left to the arithmetic. A run that invoked something and landed
    // nothing is the ordinary result of an ageing rule firing over an epistemic step whose
    // pending proposal already holds the answer (#1162), and a reader who has just watched
    // three steps run needs to be told that an empty log is what success looks like.
    if r.ran > 0 && r.committed < r.ran {
        let _ = write!(
            out,
            "\n{} step(s) ran and computed what was already committed, so no commit was written \
             for them — an\nempty commit per invocation would record how often you ran the \
             command rather than what changed.{}",
            r.ran - r.committed,
            match r.steps.iter().filter(|s| s.noted.is_some()).count() {
                0 => String::new(),
                n => format!(
                    "\n{n} of them reproduced a committed receipt, and that is noted in {}.",
                    notes::REF
                ),
            }
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
            noted: None,
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

    /// An aged re-run that lands nothing on its proposal says where it looked, and does not
    /// claim a commit it did not write.
    #[test]
    fn an_aged_proposal_that_reproduced_itself_names_the_proposal_and_claims_no_commit() {
        // #1162: the report said "the commit records that it was checked" and then "no commit
        // was written" about one step, and named the head where the tree it matched was the
        // proposal's tip.
        let mut ran = step(
            "tier",
            Route::Proposal,
            "revise",
            Outcome::Ran,
            Freshness::Stale,
        );
        ran.committed = None;
        ran.unchanged_outputs = true;
        let out = render(&plan(false, vec![ran]));
        assert!(
            out.contains("it reproduced what propose/abc1234 already proposes, so no commit"),
            "{out}"
        );
        assert!(!out.contains("so the receipt was committed"), "{out}");
        assert!(!out.contains("already at abc1234"), "{out}");
        assert!(!out.contains("noted on receipt"), "{out}");

        // The branch, where the receipt itself moved outside its input state, says it landed.
        let mut landed = step(
            "tier",
            Route::Branch,
            "compute",
            Outcome::Ran,
            Freshness::Stale,
        );
        landed.unchanged_outputs = true;
        let out = render(&plan(false, vec![landed]));
        assert!(
            out.contains(
                "the outputs had not moved and the receipt did, so the receipt was committed"
            ),
            "{out}"
        );

        // And the branch, where the look is a note and not a commit (RFC-0052), names the note.
        let mut noted = step(
            "tier",
            Route::Branch,
            "compute",
            Outcome::Ran,
            Freshness::Stale,
        );
        noted.committed = None;
        noted.unchanged_outputs = true;
        noted.noted = Some(notes::Noted {
            notes_ref: notes::REF.into(),
            receipt_blob: "0a1b2c3".into(),
            commit: "4d5e6f7".into(),
        });
        let out = render(&plan(false, vec![noted]));
        assert!(
            out.contains("it reproduced its committed receipt, so no commit was written"),
            "{out}"
        );
        assert!(
            out.contains("noted on receipt 0a1b2c3 in refs/notes/yidam-runs (4d5e6f7)"),
            "{out}"
        );
        assert!(
            out.contains("1 of them reproduced a committed receipt, and that is noted in refs/notes/yidam-runs."),
            "{out}"
        );
        assert!(!out.contains("already at abc1234"), "{out}");
        assert!(!out.contains("git restore"), "{out}");
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
            run: manifest::Run::Argv(vec!["true".into()]),
            reads: vec![],
            writes: vec![".yidam/computed/**".into()],
            verb: "compute".into(),
            after: vec![],
            ageing_days: None,
            cluster: None,
            stamps_input_commit: vec![],
        };
        let landing = vec![(".yidam/computed/x.yml".to_string(), b"x".to_vec())];
        for route in [Route::Branch, Route::Proposal] {
            let body = message(
                "compute: x",
                &cap.run.shown(),
                "x",
                &landing,
                "abc1234",
                route,
            );
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
