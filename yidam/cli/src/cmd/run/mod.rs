//! `yidam run` — a declared capability is invoked and its output becomes a commit.
//!
//! `prelude/GRAPH.md` closes the commit vocabulary, and four of its operational verbs —
//! `extract`, `refresh`, `compute`, `reconcile` — name acts nothing in this repository could
//! perform. They were written for a runtime that was never built. This is the first slice of
//! it, and it is deliberately narrower than the manifest deserves: one declared capability,
//! invoked, its output committed, a receipt written. No dependency resolution, no freshness
//! clocks, no second step, no `--dry-run` — those are #472, and building them here would be
//! the failure this slice exists to avoid rather than diligence about it.
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
//! index still holds the tree from before, so **after a run your checkout is one commit
//! behind**. That is not a defect to be fixed by touching the index — §5 forbids it, and the
//! reason is the one that put `propose` on a temporary index in the first place: a command
//! that updated a checkout would fail halfway on a dirty tree and leave somebody's work
//! somewhere they did not put it. So the report says so, and names the one path-scoped
//! command that syncs it.

pub mod exec;
pub mod manifest;
pub mod receipt;

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
}

#[derive(Debug, serde::Serialize)]
pub struct RunReport {
    pub step: String,
    pub kind: &'static str,
    pub verb: String,
    /// The commit the inputs were materialized from — the run's input state.
    pub input_commit: String,
    /// How many files the declaration resolved to at that commit.
    pub inputs: usize,
    /// What the step produced, by repository-relative path.
    pub outputs: Vec<String>,
    /// Where the receipt landed.
    pub receipt: String,
    /// `branch` or `proposal` — where this capability's commits go, decided by its verb.
    /// Reported rather than inferred by a consumer, because the two routes leave the
    /// repository in visibly different states and only one of them moves your checkout.
    pub route: &'static str,
    /// The ref that moved, and the commit written — absent when the run changed nothing.
    pub committed: Option<Committed>,
    /// Anything the step said on stderr, kept because a calculator's own account of what it
    /// did is evidence and the commit does not carry it.
    pub step_stderr: Option<String>,
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

pub fn run(step: &str, opts: Options) -> Result<()> {
    let root = repo_root()?;
    require_yidam_repo(&root)?;
    let report = plan_and_write(&root, step)?;
    if opts.format.is_json() {
        return crate::report::emit(&root, report);
    }
    println!("{}", render(&report));
    Ok(())
}

/// Everything between resolving the corpus and printing — the testable half.
pub fn plan_and_write(root: &Path, step: &str) -> Result<RunReport> {
    let Some(branch) = current_branch(root) else {
        bail!(
            "HEAD is detached, so there is no branch for a run to advance.\n  \
             A run's output is a commit on the branch it was invoked from; check one out first."
        );
    };
    let (full, short) = head(root)?;

    let m = Manifest::load(root)?;
    let cap = m.get(step)?;
    if !cap.kind.executable() {
        bail!(
            "`{step}` is a {} and this binary invokes calculators only.\n  \
             A connector re-runs against an external source, which is a network capability \
             and a credential path — see #472.",
            cap.kind.as_str()
        );
    }

    let inputs = exec::materialize(root, &full, cap)?;
    let produced = exec::invoke(cap, &inputs, step, &full)?;
    exec::check_declared(cap, &produced.outputs)?;

    let manifest_sha256 = digest_of(root, manifest::MANIFEST);
    let config_sha256 = digest_of(root, ".yidam/config.toml");
    let input_state = Receipt::input_state(cap, &manifest_sha256, &config_sha256, &inputs.files)?;
    let receipt = Receipt {
        format_version: receipt::FORMAT_VERSION,
        step: step.to_string(),
        kind: cap.kind.as_str(),
        verb: cap.verb.clone(),
        run: cap.run.clone(),
        input_state: input_state.clone(),
        input: Input {
            commit: full.clone(),
            manifest_sha256,
            config_sha256,
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
    let receipt_path = Receipt::path(step);

    // Where this capability's commits go, and what they are built on. Both follow from the
    // verb and neither is declarable — see [`Route`].
    //
    // The parent differs from the input state for an epistemic run and deliberately so: the
    // step read HEAD, which is what the receipt records, and the commit is stacked on whatever
    // the proposal branch already holds so that two epistemic runs at one HEAD accumulate the
    // way `propose`'s do rather than each discarding the other.
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

    let committed = if already_landed(
        root,
        &parent,
        &receipt_path,
        &input_state,
        &produced.outputs,
    ) {
        None
    } else {
        let mut landing: Vec<(String, Vec<u8>)> = produced.outputs.clone();
        landing.push((receipt_path.clone(), receipt.to_yaml()?.into_bytes()));
        commit(
            root,
            &Landing {
                target: &target,
                parent: &parent,
                expected: existing.as_deref(),
                short_input: &short,
                route,
            },
            cap,
            step,
            &landing,
        )?
    };

    Ok(RunReport {
        step: step.to_string(),
        kind: cap.kind.as_str(),
        verb: cap.verb.clone(),
        input_commit: short,
        inputs: inputs.files.len(),
        outputs: produced.outputs.iter().map(|(p, _)| p.clone()).collect(),
        receipt: receipt_path,
        route: route.as_str(),
        committed,
        step_stderr: (!produced.stderr.is_empty()).then_some(produced.stderr),
    })
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

/// Whether the commit already carries this exact result.
///
/// Two conditions, and the second is not redundant. The input state says the step was run
/// against this corpus before; the byte comparison says it produced the same answer. A
/// calculator that is not a function of its inputs — a clock, a random seed, a network read
/// it did not declare — would otherwise have its new output silently dropped on the grounds
/// that its inputs had not moved, which is the one failure that would make a receipt a lie.
fn already_landed(
    root: &Path,
    commit: &str,
    receipt_path: &str,
    input_state: &str,
    outputs: &[(String, Vec<u8>)],
) -> bool {
    let Ok(landed) = git(
        root,
        None,
        &["show", &format!("{commit}:{receipt_path}")],
        None,
    ) else {
        return false;
    };
    if Receipt::committed_state(&landed).as_deref() != Some(input_state) {
        return false;
    }
    // Compared as git object ids rather than as text. `git()` trims what it reads, so a
    // digest taken over that would call a file and the same file with a trailing blank line
    // identical — and `hash-object` is the identity the commit will be built from anyway.
    outputs.iter().all(|(path, bytes)| {
        let landed = git(
            root,
            None,
            &["rev-parse", &format!("{commit}:{path}")],
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

fn render(r: &RunReport) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "{} `{}` ran against {}, reading {} file(s).",
        r.kind, r.step, r.input_commit, r.inputs
    );
    if let Some(err) = &r.step_stderr {
        let _ = writeln!(out, "\n  it said:\n{}", indent(err));
    }
    let _ = writeln!(out, "\nwrote:");
    for p in &r.outputs {
        let _ = writeln!(out, "    {p}");
    }
    let _ = writeln!(out, "    {}  (the receipt)", r.receipt);

    match &r.committed {
        None => {
            let _ = write!(
                out,
                "\nNothing changed: the step reproduced the tree already at {}, so no commit \
                 was written.\nAn unchanged input state computes an unchanged output, and a \
                 commit saying so would\nrecord that you ran the command rather than that \
                 anything happened.",
                r.input_commit
            );
        }
        Some(c) if r.route == Route::Proposal.as_str() => {
            let _ = write!(
                out,
                "\n  {} {}\n  on {}\n\n\
                 `{}` is an epistemic verb, so your branch did not move and your checkout is \
                 unchanged.\nA run may propose a change to what the corpus holds and may not \
                 decide it.\n\n    git log --reverse {}..{}\n\n\
                 Merge that branch to accept it, or delete it to reject it. Nothing merges \
                 itself.",
                c.commit, c.subject, c.branch, r.verb, r.input_commit, c.branch
            );
        }
        Some(c) => {
            let _ = write!(
                out,
                "\n  {} {}\n  on {}\n\n\
                 Your working tree and index were not touched, which means they are now one \
                 commit behind\nHEAD — the paths above will read as deleted until you take \
                 them:\n\n    git restore --source=HEAD --worktree --staged -- {}\n\n\
                 Nothing was synthesized, no node was authored, and no claim was re-tagged.",
                c.commit,
                c.subject,
                c.branch,
                r.outputs
                    .iter()
                    .chain(std::iter::once(&r.receipt))
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(" ")
            );
        }
    }
    out
}

fn indent(text: &str) -> String {
    text.lines()
        .map(|l| format!("    {l}"))
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

    fn report(route: Route, verb: &str) -> RunReport {
        RunReport {
            step: "low-flow".into(),
            kind: "calculator",
            verb: verb.into(),
            input_commit: "abc1234".into(),
            inputs: 3,
            outputs: vec![".yidam/computed/low-flow.yml".into()],
            receipt: ".yidam/runs/low-flow.yml".into(),
            route: route.as_str(),
            committed: Some(Committed {
                branch: match route {
                    Route::Branch => "main".into(),
                    Route::Proposal => "propose/abc1234".into(),
                },
                commit: "def5678".into(),
                subject: format!("{verb}: low-flow — 2 files at abc1234"),
            }),
            step_stderr: None,
        }
    }

    /// The two routes say different things, and the epistemic one says the branch did not move.
    ///
    /// Worth a test because the render branches on `route` and the operational arm ends with a
    /// `git restore` that is actively wrong advice for a proposal: nothing was taken from the
    /// checkout, so there is nothing to restore.
    #[test]
    fn an_epistemic_run_is_reported_as_a_proposal_and_not_as_a_branch_advance() {
        let out = render(&report(Route::Proposal, "establish"));
        assert!(out.contains("propose/abc1234"), "{out}");
        assert!(out.contains("your branch did not move"), "{out}");
        assert!(out.contains("Nothing merges itself"), "{out}");
        assert!(
            !out.contains("git restore"),
            "a proposal took nothing from the checkout, so there is nothing to restore:\n{out}"
        );

        let out = render(&report(Route::Branch, "compute"));
        assert!(out.contains("git restore"), "{out}");
        assert!(!out.contains("Nothing merges itself"), "{out}");
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
