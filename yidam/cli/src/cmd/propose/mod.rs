//! `yidam propose` — findings become a branch of proposed epistemic commits.
//!
//! Every gate this repository ships names a task and hands it to a human who may never come
//! back. `docs/post-genesis-measurement.md` measured which norms survive that handoff: the
//! commit vocabulary held across 201 commits in one derived repository because
//! `lint --commits` answers *during* the act, and orphan discipline lost in the same
//! repositories under the same prelude because nothing answers. **A proposal is reviewable in
//! a way a report is not** — it can be merged, amended, or deleted, and each of those is a
//! decision that leaves a trace.
//!
//! # What it does not do
//!
//! RFC-0020 is the argument and [`draft`] carries the rule; the short version is that
//! checking #269's four proposal rows against the code corrected three of them. The ontology
//! names a *class* and not a node, so "link it from the node that should cite it" is a choice
//! between every instance of that class — seven candidates in the eight-node worked example.
//! Nothing detects a catalog source moving, and E3's `citations::moved`, which detects the
//! nearest thing, deliberately emits a question rather than a retag. Deciding a question is
//! answered is the resolution event Article V confines to a sangha. There is no oversized-node
//! check, and a split authors nodes.
//!
//! So: **nothing merges itself, and nothing synthesizes.** Three acts, each asserting only
//! what a finding or the corpus's own declarations already assert — `open`, `withdraw`,
//! `close`. No edges, no retags, no new nodes.
//!
//! # Why a branch and not the working tree
//!
//! [`write`] builds commits against a temporary index, so a run changes nothing a person can
//! see and is safe mid-edit. The branch is reviewed as commits and rejected by deleting it.

pub mod draft;
pub mod write;

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::Result;

use crate::cmd::lint::{self, baseline, history};
use crate::paths::{repo_root, require_yidam_repo, yidam_catalog_dir, yidam_corpus_dir};
use draft::{Change, Proposal};

pub struct Options {
    /// Draft everything and write nothing.
    pub dry_run: bool,
    /// Replace an existing `propose/<head>` rather than refusing it.
    pub force: bool,
    pub format: crate::report::Format,
}

/// A finding this run could not draft a proposal for, and why.
///
/// Reported rather than swallowed. A finding silently not proposed about is the exact failure
/// this command exists to remove, and it would be an easy one to reintroduce here.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Skipped {
    pub check: String,
    pub node: String,
    pub reason: String,
}

#[derive(Debug, serde::Serialize)]
pub struct ProposeReport {
    pub branch: String,
    pub head: String,
    pub proposals: Vec<Drafted>,
    pub skipped: Vec<Skipped>,
    /// Absent on `--dry-run`, and on a run with nothing to propose.
    pub written: Option<write::Written>,
    /// What the corpus licensed. `null` means no withdrawal may ever be drafted here, which
    /// is every corpus that has not said otherwise.
    pub withdraw_uncited_after: Option<usize>,
}

#[derive(Debug, serde::Serialize)]
pub struct Drafted {
    pub verb: &'static str,
    pub subject: String,
    pub check: String,
    pub node: String,
    /// The finding's own words, which the commit body quotes. Carried into the report so a
    /// consumer can check the carriage rule for itself rather than trusting that it held.
    pub detail: String,
    pub paths: Vec<String>,
}

/// Read a repo-relative path, or empty.
fn read(root: &Path, rel: &str) -> String {
    std::fs::read_to_string(root.join(rel)).unwrap_or_default()
}

fn rel(root: &Path, p: &Path) -> String {
    p.strip_prefix(root)
        .unwrap_or(p)
        .to_string_lossy()
        .replace('\\', "/")
}

/// Every catalog entry as `(repo-relative path, text)`, for a withdrawal to edit `used-by:`.
fn catalogs(root: &Path) -> Vec<(String, String)> {
    crate::walk::walk_md_files(&yidam_catalog_dir(root))
        .iter()
        .map(|p| {
            let r = rel(root, p);
            let text = read(root, &r);
            (r, text)
        })
        .collect()
}

/// The `(check, node)` pairs the gate is failing on right now.
///
/// Introduced *and* expired, because both fail the gate and both are somebody's outstanding
/// work. They are different events — one is what this change did, the other is what the
/// repository agreed to deal with and has not — but a proposal carries the finding either
/// way, and the finding is the same.
fn gating(root: &Path, checks: &[lint::model::Check]) -> Result<BTreeSet<(String, String)>> {
    let committed = baseline::Baseline::load(root)?;
    let d = baseline::diff(checks, &committed, &history::corpus_commits(root));
    let mut out: BTreeSet<(String, String)> = d.introduced.into_iter().collect();
    out.extend(d.expired.into_iter().map(|e| (e.check, e.node)));
    Ok(out)
}

/// Draft every proposal this run has, in commit order.
///
/// Withdrawals first: a node being proposed for deletion must not also be asked a question,
/// and ordering the verbs is how that is arranged rather than by a special case.
fn plan(
    root: &Path,
    checks: &[lint::model::Check],
    threshold: Option<usize>,
    head: &str,
) -> Result<(Vec<Proposal>, Vec<Skipped>)> {
    let gates = gating(root, checks)?;

    // Findings that are live but do not gate, because the corpus licensed being asked about
    // them rather than the gate deciding. Today that is one check: an expired source record,
    // licensed by the `ttl_days` this corpus declared — the same shape as
    // `withdraw_uncited_after` licensing a deletion. Severity is not the licence; a
    // declaration is.
    let licensed: BTreeSet<(String, String)> = checks
        .iter()
        .filter(|c| c.id == "catalog-expired")
        .flat_map(|c| {
            c.violations
                .iter()
                .map(move |v| (c.id.to_string(), v.node.clone()))
        })
        .collect();
    // What `close:` measures against: everything still true, however it became true.
    let live: BTreeSet<(String, String)> = gates.union(&licensed).cloned().collect();
    let mut proposals = Vec::new();
    let mut skipped = Vec::new();

    // ── withdraw ────────────────────────────────────────────────────────────
    let cats = catalogs(root);
    let mut withdrawn: BTreeSet<String> = BTreeSet::new();
    if let Some(limit) = threshold {
        for e in draft::uncited(checks) {
            if let Some(p) = draft::withdraw_proposal(head, &e, limit, &cats) {
                withdrawn.insert(e.node.clone());
                proposals.push(p);
            }
        }
    }

    // ── open ────────────────────────────────────────────────────────────────
    //
    // Skipping a node already carrying this check's question is what makes a re-run at a
    // later HEAD additive rather than a second copy of everything still outstanding.
    let mut asked: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for e in draft::eligible(checks, &gates) {
        if withdrawn.contains(&e.node) {
            continue;
        }
        let text = read(root, &e.node);
        let already = asked
            .entry(e.node.clone())
            .or_insert_with(|| draft::marked(&text).into_iter().map(|m| m.check).collect());
        if already.contains(&e.check) {
            continue;
        }
        match draft::open_proposal(head, &e, &text) {
            Some(p) => {
                already.insert(e.check.clone());
                proposals.push(p);
            }
            None => skipped.push(Skipped {
                check: e.check.clone(),
                node: e.node.clone(),
                reason: "its `description:` is not a block scalar, so a paragraph cannot be \
                         appended without reformatting a line somebody wrote"
                    .into(),
            }),
        }
    }

    // ── open, on the sources themselves ─────────────────────────────────────
    for e in draft::eligible(checks, &licensed) {
        let text = read(root, &e.node);
        if draft::marked(&text).iter().any(|m| m.check == e.check) {
            continue;
        }
        match draft::open_source_proposal(head, &e, &text) {
            Some(p) => proposals.push(p),
            None => skipped.push(Skipped {
                check: e.check.clone(),
                node: e.node.clone(),
                reason: "it has no closing frontmatter fence, so a paragraph cannot be \
                         appended where a reader would find it"
                    .into(),
            }),
        }
    }

    // ── close ───────────────────────────────────────────────────────────────
    //
    // Every paragraph this command wrote whose finding is no longer gating. Read from the
    // corpus rather than from the run's proposals, because a question opened at an earlier
    // HEAD is exactly the one that needs retiring.
    let closable = crate::walk::walk_corpus_instances(&yidam_corpus_dir(root))
        .into_iter()
        .chain(crate::walk::walk_md_files(&yidam_catalog_dir(root)));
    for path in closable {
        let node = rel(root, &path);
        if withdrawn.contains(&node) {
            continue;
        }
        let mut text = read(root, &node);
        // Re-read after each removal: a second marked paragraph's line numbers move when the
        // first one goes.
        loop {
            let Some(m) = draft::marked(&text)
                .into_iter()
                .find(|m| !live.contains(&(m.check.clone(), node.clone())))
            else {
                break;
            };
            let p = draft::close_proposal(&node, &text, &m);
            text = match p.changes.first() {
                Some(Change::Write { content, .. }) => content.clone(),
                _ => break,
            };
            proposals.push(p);
        }
    }

    // The carriage rule, enforced rather than assumed. Nothing above should be able to
    // produce a proposal that fails it, which is exactly why the check is here: the failure
    // it guards against is a future edit to a message template, and that edit will not think
    // of itself as a constitutional change.
    proposals.retain(|p| {
        if p.carries() {
            return true;
        }
        skipped.push(Skipped {
            check: p.check.clone(),
            node: p.node.clone(),
            reason: "the drafted message does not quote the finding, so it would assert \
                     something the finding did not — refusing to draft it"
                .into(),
        });
        false
    });

    proposals.sort_by(|a, b| (a.verb, &a.node, &a.check).cmp(&(b.verb, &b.node, &b.check)));
    Ok((proposals, skipped))
}

pub fn propose(opts: Options) -> Result<()> {
    let root = repo_root()?;
    require_yidam_repo(&root)?;
    write::require_committed_corpus(&root)?;

    let (_, head) = write::head(&root)?;
    let cfg = crate::config::load_yidam_config(&root)?;
    let checks = lint::run_checks(
        &root,
        &lint::Options {
            warn_only: true,
            explain: false,
            commits: false,
            range: None,
            bless: false,
            init_baseline: false,
            format: crate::report::Format::Text,
        },
    );
    let (proposals, skipped) = plan(&root, &checks, cfg.propose.withdraw_uncited_after, &head)?;

    let written = if opts.dry_run || proposals.is_empty() {
        None
    } else {
        Some(write::write(&root, &proposals, opts.force)?)
    };

    let report = ProposeReport {
        branch: write::branch_for(&head),
        head: head.clone(),
        proposals: proposals
            .iter()
            .map(|p| Drafted {
                verb: p.verb.as_str(),
                subject: p.subject.clone(),
                check: p.check.clone(),
                node: p.node.clone(),
                detail: p.detail.clone(),
                paths: p.changes.iter().map(|c| c.path().to_string()).collect(),
            })
            .collect(),
        skipped,
        written,
        withdraw_uncited_after: cfg.propose.withdraw_uncited_after,
    };

    if opts.format.is_json() {
        return crate::report::emit(&root, report);
    }
    println!("{}", render(&report, opts.dry_run));
    Ok(())
}

/// The text report.
///
/// The closing sentences are not optional. The failure mode of a command that writes
/// epistemic commits is a reader who assumes they landed — the same reason
/// `render_movements` prints *nothing was changed, and no claim was re-tagged* rather than a
/// count that reads as a verdict.
fn render(r: &ProposeReport, dry_run: bool) -> String {
    if r.proposals.is_empty() {
        let mut out = "Nothing to propose: no finding is failing the gate.".to_string();
        if r.withdraw_uncited_after.is_none() {
            out.push_str(
                "\n\n`[propose] withdraw_uncited_after` is not declared, so no withdrawal is \
                 ever drafted here. That is the default, and turning it on is a decision \
                 about this corpus.",
            );
        }
        return out;
    }

    let mut out = String::new();
    let mut current = String::new();
    for p in &r.proposals {
        if p.node != current {
            out.push_str(&format!("\n  {}\n", p.node));
            current = p.node.clone();
        }
        out.push_str(&format!("    {}: {}\n", p.verb, p.subject));
    }

    if !r.skipped.is_empty() {
        out.push_str(&format!(
            "\n{} finding(s) with no proposal:\n",
            r.skipped.len()
        ));
        for s in &r.skipped {
            out.push_str(&format!("    [{}] {} — {}\n", s.check, s.node, s.reason));
        }
    }

    let head = match (&r.written, dry_run) {
        (Some(w), _) => format!(
            "{} proposal(s) on {} — nothing was merged, and no claim was re-tagged.",
            w.commits.len(),
            w.branch
        ),
        (None, true) => format!(
            "{} proposal(s) drafted and not written — this is --dry-run.",
            r.proposals.len()
        ),
        (None, false) => format!("{} proposal(s).", r.proposals.len()),
    };

    let tail = match (&r.written, dry_run) {
        (Some(w), _) => format!(
            "\nReview them as commits: `git log --reverse HEAD..{}`.\nReject them by deleting \
             the branch: `git branch -D {}`.\n",
            w.branch, w.branch
        ),
        _ => format!(
            "\nRe-run without --dry-run to write them to `{}`.\n",
            r.branch
        ),
    };

    format!("{head}\n{out}{tail}")
}

#[cfg(test)]
mod tests;
