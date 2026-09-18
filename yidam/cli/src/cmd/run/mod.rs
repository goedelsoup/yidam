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
//! — RFC-0026 §2. Here that rule is one refusal in [`manifest`]: a capability declaring an
//! epistemic verb does not load. It is Rust with no override path, which is RFC-0026 §3's
//! answer to the question RFC-0024 left open. A corpus that could write that permission into
//! its own policy could license its runs to author `establish:` on the baseline, and the
//! safety argument would be a config value.
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
use manifest::{Capability, Manifest};
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
    /// The branch that moved, and the commit written — absent when the run changed nothing.
    pub committed: Option<Committed>,
    /// Anything the step said on stderr, kept because a calculator's own account of what it
    /// did is evidence and the commit does not carry it.
    pub step_stderr: Option<String>,
}

#[derive(Debug, serde::Serialize)]
pub struct Committed {
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

    let committed = if already_landed(root, &full, &receipt_path, &input_state, &produced.outputs) {
        None
    } else {
        let mut landing: Vec<(String, Vec<u8>)> = produced.outputs.clone();
        landing.push((receipt_path.clone(), receipt.to_yaml()?.into_bytes()));
        commit(root, &branch, &full, &short, cap, step, &landing)?
    };

    Ok(RunReport {
        step: step.to_string(),
        kind: cap.kind.as_str(),
        verb: cap.verb.clone(),
        input_commit: short,
        inputs: inputs.files.len(),
        outputs: produced.outputs.iter().map(|(p, _)| p.clone()).collect(),
        receipt: receipt_path,
        committed,
        step_stderr: (!produced.stderr.is_empty()).then_some(produced.stderr),
    })
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
    branch: &str,
    parent: &str,
    short_parent: &str,
    cap: &Capability,
    step: &str,
    landing: &[(String, Vec<u8>)],
) -> Result<Option<Committed>> {
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

    let subject = subject(&cap.verb, step, landing.len(), short_parent);
    let message = message(&subject, cap, step, landing, short_parent);
    let sha = commit_tree(root, &tree, parent, &message, (AUTHOR_NAME, AUTHOR_EMAIL))?;

    // Compare-and-swap. `update-ref <ref> <new> <old>` refuses when the branch moved while
    // the step was running, which for a calculator is not a hypothetical interval.
    git(
        root,
        None,
        &["update-ref", &format!("refs/heads/{branch}"), &sha, parent],
        None,
    )
    .with_context(|| format!("{branch} moved while `{step}` was running; nothing was landed"))?;

    Ok(Some(Committed {
        branch: branch.to_string(),
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
    let _ = write!(
        body,
        "\nOperational: the pipeline advanced and no understanding changed. Nothing was\n\
         synthesized, no node was authored, and no claim was re-tagged — a run may author\n\
         none of those, and the manifest that licensed this one could not have declared them.\n"
    );
    body
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

    /// The subject a run writes is in the closed vocabulary, asserted through the parity
    /// function that decides it rather than by reading the format string.
    #[test]
    fn the_subject_a_run_writes_is_recognized_and_operational() {
        for verb in yidam_core::git::OPERATIONAL_VERBS {
            let s = subject(verb, "low-flow", 2, "abc1234");
            let e = yidam_core::git::classify_commit("h", &s);
            assert_eq!(e.verb, *verb);
            assert!(yidam_core::git::is_recognized_verb(&e.verb), "{s}");
            assert_eq!(e.kind, yidam_core::git::CommitKind::Operational, "{s}");
        }
    }

    #[test]
    fn one_file_is_not_pluralized() {
        assert_eq!(
            subject("compute", "low-flow", 1, "abc1234"),
            "compute: low-flow — 1 file at abc1234"
        );
    }
}
