//! `yidam check-diff` — code that names something the ontology has never heard of.
//!
//! E1 made `.ont.yml` a contract that checks enforce: instance validation, property types,
//! edge licensing, target classes. Every one of those reads the *corpus*. Application code
//! is the ontology's implementation and it changes constantly — **32% of one derived
//! repository's history and 49% of another's touches `crates/`** — and a connector that
//! starts returning a new field, a calculator that introduces a domain type, a struct that
//! models a concept nobody wrote a class for are all invisible. `yidam diff` diffs the
//! corpus; nothing read the ontology from the code side.
//!
//! RFC-0021 is the specification. This is Phase A of #23: deterministic, no model call, no
//! vector index, no parity-surface change, no network.
//!
//! RFC-0022 is Phase B, and it changed nothing about that sentence. Its measurement is the
//! reason: the semantic pass #23 planned turns out to be a string rule, so a near-miss is one
//! optional field on the finding below rather than a second check. See [`near`].
//!
//! # Why a diff, and not the corpus
//!
//! Because the gap is enormous. One repository declares **15 classes** and its code defines
//! **275 types**, of which 20 match a declared name — 7%. The other two measure 4% and 2%.
//! Walked over the whole corpus this reports 255, 46 and 202 findings, and
//! `example_corpus.rs` states the rule that forbids it: *a permanently non-empty report is
//! where a real finding gets lost.*
//!
//! Scoped to a diff the same signal is tractable. Sampling the most recent 60 commits
//! touching `crates/` in each repository, **the median commit reports zero** and the 90th
//! percentile is 5, 2 and 1. It also dissolves the hardest open question: *is `Cell` a domain
//! concept or infrastructure?* needs no general answer, because a diff-scoped check asks
//! about each type once, at the commit that introduces it, with the author present.
//!
//! # It cannot fail, and that is deliberate
//!
//! Every finding is a Warn and the process exits 0. A concept the ontology has not modelled
//! is a question about the corpus, not a defect in the build, and the fix — a new class, or a
//! decision not to have one — is an author's judgement either way. Gating would also make
//! adoption a build break in every repository that predates it, which is the ratchet failure
//! `docs/post-genesis-measurement.md` recorded.

pub mod extract;
pub mod near;

use std::collections::BTreeSet;
use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result};

use crate::authorship::Authorship;
use crate::cmd::check_diff::near::Nearest;
use crate::cmd::lint;
use crate::paths::{repo_root, require_yidam_repo, yidam_corpus_dir};
use crate::report::Span;

/// The one finding this command reports, in either phase.
///
/// `CONFLICT` — code contradicting a claim — is **withdrawn rather than deferred**. It is a
/// fact about meaning, and RFC-0022 decided that nothing which may not call a remote model
/// can hold one; #23's table is closed rather than left implying a third phase is coming.
/// `ALIGNED` is not a finding at all: one per correctly implemented concept is a permanently
/// non-empty report by construction, so it is [`CheckDiffReport::aligned`], a number in the
/// summary line, which is the same thing a reader wanted at none of the cost.
pub const CHECK: &str = "unmodelled-concept";

/// A type the diff introduces that the ontology does not name.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Finding {
    /// Stable id. One check in Phase A, named anyway so a consumer branches on a token
    /// rather than on prose.
    pub check: &'static str,
    /// `warn`, or `info` where the file sits in a declared region.
    pub severity: &'static str,
    /// As written in the code: `AgendaItem`.
    pub concept: String,
    /// What the ontology would have to name for this to match: `agenda-item`.
    pub name: String,
    /// A declared name this one shares a root with, when the vocabulary holds one.
    ///
    /// A lead and not a verdict: it annotates a finding that exists either way, so a wrong
    /// candidate costs a reader one look and never a spurious row. See [`near`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nearest: Option<Nearest>,
    pub file: String,
    pub span: Span,
    /// Phrased as a question, deliberately. The answer is a person's.
    ///
    /// The register `citations::moved` established, and for the same reason: matching is by
    /// name, so a type that matches nothing may be a genuine gap in the ontology or a helper
    /// the ontology has no reason to know about. The check cannot tell, and must not phrase
    /// itself as though it could.
    pub question: String,
    /// Why a finding here is not this repository's to fix, and whose it is.
    ///
    /// Present only inside a `generated` or `imported` region; see [`crate::authorship`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub region: Option<String>,
}

#[derive(Debug, serde::Serialize)]
pub struct CheckDiffReport {
    /// The range reported on, unnormalised — what the user typed is what the report says.
    ///
    /// When `check-diff` was run bare it is the default this resolved to (#389), so a
    /// consumer reading a range nobody typed is reading the one that was actually compared
    /// rather than a blank.
    pub range: String,
    /// Names the ontology declares: classes, properties and relationships.
    pub vocabulary: usize,
    /// Type declarations the diff introduces, after `excluded` regions are dropped.
    pub introduced: usize,
    /// How many of those match a declared name. **A count, never a finding.**
    pub aligned: usize,
    pub findings: Vec<Finding>,
}

/// The diff of `crates/` between two refs.
///
/// `--unified=0` because context lines are only opportunities to misread a declaration, and
/// `-M` so a file move produces no hunks rather than a hundred spurious introductions.
fn read_diff(root: &Path, before: &str, after: &str) -> Result<String> {
    let out = Command::new("git")
        .current_dir(root)
        .args([
            "diff",
            "-M",
            "--unified=0",
            &format!("{before}..{after}"),
            "--",
            "crates/",
        ])
        .output()
        .context("running git diff")?;
    if !out.status.success() {
        anyhow::bail!(
            "git diff failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    String::from_utf8(out.stdout).context("git diff output is not UTF-8")
}

/// The finding's sentence, with the near-miss carried as evidence when there is one.
///
/// **Three answers rather than two**, because a shared root rules none of them out: the type
/// may be what the ontology already declares under another word form, a concept the corpus
/// should model, or a helper it has no reason to know about. Phase A's two answers survive
/// verbatim, and the candidate is offered before them rather than instead of them.
///
/// The register is unchanged, and is `citations::moved`'s: phrased as a question,
/// deliberately, because the answer is a person's. What the candidate is not is a verdict —
/// a shared root is a fact about two strings, and [`near`] says why nothing here reports a
/// score alongside it.
fn question(name: &str, nearest: Option<&Nearest>) -> String {
    let Some(n) = nearest else {
        return format!(
            "nothing the ontology declares is named `{name}`. Is it a concept this corpus \
             should model, or a helper the ontology has no reason to know about?"
        );
    };
    format!(
        "nothing the ontology declares is named `{name}`, though `{}` shares the root `{}`. \
         Is that the same concept, one this corpus should model, or a helper the ontology \
         has no reason to know about?",
        n.name, n.shared
    )
}

/// The report, from a diff and a vocabulary. Pure — the subprocess is [`read_diff`]'s.
///
/// **Exclusions come from [`crate::authorship`] and nowhere else.** #23 lists "scope of
/// application code — test and fixture exclusions" as an open question needing a decision;
/// it does not, because that decision was made, argued and built after the issue was
/// written. Measured against the three instrumented repositories, a second vocabulary would
/// have bought almost nothing anyway: of the 68, 101 and 20 types introduced across their
/// most recent 60 code-touching commits, **2 apiece** sit under a `tests/` path.
fn build(
    range: &str,
    diff: &str,
    vocabulary: &BTreeSet<String>,
    authorship: &Authorship,
) -> CheckDiffReport {
    let mut introduced = 0usize;
    let mut aligned = 0usize;
    let mut findings = Vec::new();

    for d in extract::introduced(diff) {
        let region = authorship.covering(&d.file);
        // `excluded` is the one kind that means *do not look*, exactly as in `lint`. It is
        // not counted either: a type nobody may be asked about is not one this run saw.
        if region.is_some_and(|r| !r.kind.reportable()) {
            continue;
        }
        introduced += 1;
        let name = extract::kebab(&d.name);
        if vocabulary.contains(&name) {
            aligned += 1;
            continue;
        }
        let nearest = near::nearest(&name, vocabulary);
        findings.push(Finding {
            check: CHECK,
            // `generated` and `imported` are still reported, and reported to somebody: the
            // finding is real and it belongs to a generator or an upstream rather than to
            // whoever ran this.
            severity: if region.is_some() { "info" } else { "warn" },
            question: question(&name, nearest.as_ref()),
            region: region.map(|r| r.explain()),
            concept: d.name,
            nearest,
            name,
            file: d.file,
            span: Span { line: d.line },
        });
    }

    CheckDiffReport {
        range: range.to_string(),
        vocabulary: vocabulary.len(),
        introduced,
        aligned,
        findings,
    }
}

/// Where HEAD sits relative to the baseline, as git answered it.
///
/// A struct rather than four arguments so [`default_range`] — which is all the judgement —
/// is testable without building a repository per case. The four degenerate positions this
/// command can be run from are cheap to write down and expensive to reproduce.
#[derive(Debug, Clone)]
struct Position {
    /// `main`, `master`, or nothing in a repository that has neither yet.
    base: Option<String>,
    /// The tip of `base`.
    base_tip: Option<String>,
    /// `merge-base(base, HEAD)`, or nothing when git could name none — an unborn HEAD, or
    /// histories with no common ancestor.
    merge_base: Option<String>,
    /// HEAD's commit.
    head: Option<String>,
}

fn read_position(root: &Path) -> Position {
    let git = |args: &[&str]| -> Option<String> {
        let out = Command::new("git")
            .current_dir(root)
            .args(args)
            .output()
            .ok()?;
        out.status
            .success()
            .then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
            .filter(|s| !s.is_empty())
    };
    let base = crate::git::base_branch(root);
    let head = git(&["rev-parse", "HEAD"]);
    let (base_tip, merge_base) = match &base {
        Some(b) => (git(&["rev-parse", b]), git(&["merge-base", b, "HEAD"])),
        None => (None, None),
    };
    Position {
        base,
        base_tip,
        merge_base,
        head,
    }
}

/// What a bare `yidam check-diff` compares.
///
/// #389 settled that the range is optional and that this is the default, on the argument the
/// original explicit-range copy did not consider: `diff` compares two corpus states and
/// neither is privileged, while this command asks *what did this branch's worth of work name
/// that the ontology has not*, and that question has an obvious range. Measured over every
/// branch the derived corpora have merged into their baseline, **250 of 381 touched
/// `crates/`** — 90% and 88% in the two largest — so the default is asking about real code
/// most times it fires.
///
/// # Why the degenerate positions are errors and not empty reports
///
/// The same measurement found 8 of 13 corpora sitting *on* their baseline at any given
/// moment, where the merge-base is HEAD and the range is empty. An empty range renders as
/// "No type declaration was introduced in `crates/` by main..HEAD", which is true and is a
/// lie of omission: it is indistinguishable from the informative answer — *your branch
/// introduced no types* — and it is the failure mode `example_corpus.rs` argues against from
/// the other direction. So this refuses rather than reporting nothing.
///
/// Detached HEAD is **not** one of these cases, contrary to how the question was filed:
/// `merge-base main HEAD` resolves normally there and the default is exactly right.
fn default_range(p: &Position) -> Result<String> {
    let Some(base) = &p.base else {
        anyhow::bail!(
            "no range given, and this repository has no baseline to compare against\n  \
             A bare `yidam check-diff` compares your branch's work with the merge-base of \
             `main` (or `master`), and neither branch exists here yet. Pass a range, e.g. \
             `yidam check-diff HEAD~5`."
        )
    };
    let (Some(merge_base), Some(head)) = (&p.merge_base, &p.head) else {
        anyhow::bail!(
            "no range given, and `{base}` and HEAD have no common ancestor to compare from\n  \
             A bare `yidam check-diff` compares your branch's work with the merge-base of \
             `{base}`. Pass a range, e.g. `yidam check-diff HEAD~5`."
        )
    };
    if merge_base == head {
        anyhow::bail!(
            "no range given, and there is no branch's work to compare — you are on `{base}`\n  \
             A bare `yidam check-diff` asks what the work on this branch named that the \
             ontology has not, and the merge-base with `{base}` is HEAD itself, so that \
             question is empty. Pass a range, e.g. `yidam check-diff HEAD~5`."
        )
    }
    // `main..HEAD` whenever the baseline has not moved on since this branch left it, because
    // a two-dot range compares endpoints and there the endpoint *is* the merge-base.
    //
    // Once `main` is ahead the two differ, and the merge-base is the correct one rather than
    // the tidier one. Verified against a derived corpus: with a type that existed at the
    // branch point and that `main` has since deleted, `main..HEAD` sees it present at HEAD
    // and absent at the endpoint and reports it as newly introduced — a finding asking an
    // author about a concept their branch never touched. From the merge-base it is unchanged
    // and says nothing.
    Ok(match &p.base_tip {
        Some(tip) if tip == merge_base => format!("{base}..HEAD"),
        _ => format!("{merge_base}..HEAD"),
    })
}

pub fn check_diff(range: Option<String>, format: crate::report::Format) -> Result<()> {
    let root = repo_root()?;
    require_yidam_repo(&root)?;
    let range = match range {
        Some(r) => r,
        None => default_range(&read_position(&root))?,
    };
    let range = range.as_str();
    let (before, after) = crate::cmd::diff::parse_range(range);
    let diff = read_diff(&root, &before, &after)?;

    let corpus_dir = yidam_corpus_dir(&root);
    let classes = lint::checks::load_classes(
        &root,
        &crate::walk::walk_ont_files(&corpus_dir),
        &lint::Overlay::default(),
    );
    // A manifest that exists and cannot be parsed is an error rather than an empty one: a
    // degraded read would silently report an imported region as though this repository had
    // authored it.
    let authorship = Authorship::load(&root)?;

    let report = build(range, &diff, &extract::declared(&classes), &authorship);

    if format.is_json() {
        return crate::report::emit(&root, report);
    }
    println!("{}", render(&report));
    Ok(())
}

/// The text report.
///
/// The closing sentences are not optional, for the reason `propose`'s renderer gives about
/// its own: the failure mode here is a reader who takes a name match for agreement. Matching
/// is shallow and the report has to keep saying so.
fn render(r: &CheckDiffReport) -> String {
    if r.introduced == 0 {
        return format!(
            "No type declaration was introduced in `crates/` by {}.",
            r.range
        );
    }

    let mut out = format!(
        "{} of {} type(s) introduced in `crates/` by {} match a concept the ontology \
         declares.\n",
        r.aligned, r.introduced, r.range
    );
    if r.findings.is_empty() {
        out.push_str("\nNothing here is unmodelled.\n");
        return out;
    }

    let mut current = String::new();
    for f in &r.findings {
        if f.file != current {
            out.push_str(&format!("\n  {}\n", f.file));
            current.clone_from(&f.file);
        }
        out.push_str(&format!(
            "    line {} [{}] {} — {}\n",
            f.span.line, f.severity, f.concept, f.question
        ));
        if let Some(region) = &f.region {
            out.push_str(&format!("      {region}\n"));
        }
    }

    out.push_str(&format!(
        "\n{} concept(s) the ontology has not modelled, against the {} name(s) it declares.\n\
         Matching is by name alone: this cannot tell a gap in the ontology from a helper it \
         has no reason to know about, and does not claim to. Nothing was changed, and no \
         class was created.\n",
        r.findings.len(),
        r.vocabulary
    ));
    // Only when one was offered. A report that suggested nothing should not explain how it
    // would have — and a reader who sees the sentence has a candidate in front of them to
    // read it against.
    if r.findings.iter().any(|f| f.nearest.is_some()) {
        out.push_str(
            "A nearest name is two names sharing a root and nothing more. No model read \
             either of them, and the suggestion is a lead to check rather than a claim that \
             the two are the same thing.\n",
        );
    }
    out
}

#[cfg(test)]
mod tests;
