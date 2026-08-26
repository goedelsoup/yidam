//! Turning findings into proposals, and the rule that decides which turn into what.
//!
//! Every function here is pure: it takes findings and file text and returns proposals. It
//! opens no repository, writes nothing, and knows nothing about git — that is
//! [`super::write`]'s job, and the split is what makes the interesting part testable
//! without a fixture repository.
//!
//! # Carriage, not composition
//!
//! RFC-0020's whole design is one constraint: **a proposal may assert only what the finding
//! already asserts.** `prelude/GRAPH.md` licenses exactly one epistemic commit written
//! outside a resolution event — `transport` — and licenses it on that ground: carriage
//! "introduces no node, edge or claim that its author did not hold".
//!
//! Here the constraint is mechanical rather than a matter of judgement. Every proposal's
//! commit body **quotes its finding's `detail` verbatim**, [`Proposal::carries`] is the
//! predicate, and a proposal failing it is dropped and reported rather than written.
//! Paraphrase is composition. Quotation is carriage.
//!
//! The prose a proposal adds beyond the quotation is a *fixed framing* — the same sentence
//! every time, naming the finding as unresolved — and deliberately not a per-check
//! narration. A table of bespoke explanations would be this module deciding what each
//! finding means, which is the thing it must not do.

use std::collections::BTreeSet;

use crate::cmd::lint::history::Age;
use crate::cmd::lint::model::Check;

/// How an appended paragraph identifies itself.
///
/// Two jobs, and both are load-bearing. A reader can tell generated prose from authored
/// prose without consulting the log — the principle [`crate::authorship`] argues for whole
/// regions, applied at the paragraph. And a later run can find its own paragraphs again,
/// which is the only thing that makes [`Verb::Close`] possible: `propose` closes a question
/// `propose` opened, and never one a person wrote.
pub const MARKER: &str = "Opened by `yidam propose` at ";

/// The framing, appended after the finding and before the tag.
///
/// Fixed prose, identical on every proposal. It names the finding as unresolved and says who
/// is not resolving it; it does not interpret the finding, because interpreting it is the
/// composition this command exists to refuse.
const FRAMING: &str = "What follows from that is unresolved here: `yidam propose` carries \
                       findings into the corpus and does not answer them.";

/// Column the corpus's prose wraps at, block indent included.
///
/// Matched to what derived corpora actually write rather than chosen: the worked example's
/// longest prose line is 92 characters. A paragraph wrapped to a different width than its
/// neighbours reads as pasted in, which is a true thing to advertise but not this way.
const WRAP: usize = 95;

/// The three verbs, and the only three.
///
/// Each is in `GRAPH.md`'s closed epistemic vocabulary, so a proposal passes
/// `lint --commits` by construction rather than by luck. What is *not* here is the whole
/// argument: no `establish` (authoring a node), no `revise` (retagging a claim, or splitting
/// one node into two), no `synthesize` and no `resolve`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Verb {
    /// Deletes a node the corpus itself declared over-collected. Ordered first because a
    /// node being withdrawn must not also be asked a question.
    Withdraw,
    /// Records a finding's question against the node it is about.
    Open,
    /// Retires a question this command opened, whose finding is gone.
    Close,
}

impl Verb {
    pub fn as_str(self) -> &'static str {
        match self {
            Verb::Withdraw => "withdraw",
            Verb::Open => "open",
            Verb::Close => "close",
        }
    }
}

/// One edit a proposed commit makes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Change {
    /// Replace this repo-relative path's content.
    Write { path: String, content: String },
    /// Delete this repo-relative path.
    Remove { path: String },
}

impl Change {
    pub fn path(&self) -> &str {
        match self {
            Change::Write { path, .. } | Change::Remove { path } => path,
        }
    }
}

/// One proposed epistemic commit.
#[derive(Debug, Clone)]
pub struct Proposal {
    pub verb: Verb,
    /// The subject line after `<verb>: `.
    pub subject: String,
    pub body: String,
    /// The check this carries, and the node it is about — the same `(check, node)` identity
    /// the baseline compares on.
    pub check: String,
    pub node: String,
    /// The finding's own words. Quoted into [`Self::body`], and the thing
    /// [`Self::carries`] tests for.
    pub detail: String,
    /// Ordered by path so a commit's tree is a function of its proposal and not of a
    /// hash map's iteration.
    pub changes: Vec<Change>,
}

impl Proposal {
    /// The full commit message: subject, body, trailers.
    pub fn message(&self, head: &str) -> String {
        format!(
            "{}: {}\n\n{}\n\nFinding: {} {}\nProposed-from: {}\n",
            self.verb.as_str(),
            self.subject,
            wrap_body(self.body.trim_end()),
            self.check,
            self.node,
            head
        )
    }

    /// Whether this proposal carries its finding rather than describing it.
    ///
    /// RFC-0020's constitutional rule as a predicate, and it is enforced at runtime rather
    /// than only in tests: a proposal that fails it is **not drafted**, and the finding is
    /// reported as skipped instead. Failing closed is the point — the cost of dropping a
    /// proposal is that somebody reads a report line, and the cost of writing one is an
    /// epistemic commit asserting something no finding did.
    pub fn carries(&self) -> bool {
        !self.detail.trim().is_empty() && self.body.contains(self.detail.trim())
    }
}

/// A finding eligible to be proposed about: one that gates right now.
///
/// Not every finding. A warn or info finding is one the check itself says is *frequently
/// fine* — `orphan-in`'s own rationale concedes that a node authored this morning
/// legitimately has no inbound edges — and drafting a question about each would bury the
/// ones that matter under the ones that do not. A finding the baseline forgives is debt the
/// corpus already looked at and dispositioned.
///
/// What is left is exactly the set a person is being told to fix today and may never come
/// back to, which is E4's whole subject.
#[derive(Debug, Clone)]
pub struct Eligible {
    pub check: String,
    pub node: String,
    pub detail: String,
    pub age: Option<Age>,
}

/// The gating findings of a run, as [`Eligible`] rows.
///
/// `gating` is the `(check, node)` set the baseline diff reports as introduced or expired —
/// computed by the caller, because deciding what the gate forgives is the baseline's job and
/// not this module's.
pub fn eligible(checks: &[Check], gating: &BTreeSet<(String, String)>) -> Vec<Eligible> {
    let mut out: Vec<Eligible> = checks
        .iter()
        .flat_map(|c| c.violations.iter().map(move |v| (c, v)))
        .filter(|(c, v)| gating.contains(&(c.id.to_string(), v.node.clone())))
        .map(|(c, v)| Eligible {
            check: c.id.to_string(),
            node: v.node.clone(),
            detail: v.detail.clone(),
            age: v.age.clone(),
        })
        .collect();
    out.sort_by(|a, b| (&a.node, &a.check).cmp(&(&b.node, &b.check)));
    out
}

/// Every `orphan-in` finding with a clock, gating or not.
///
/// Withdrawal is licensed by the corpus's own `withdraw_uncited_after` and not by the gate,
/// so it reads the check directly. A corpus may reasonably declare that an uncited node is
/// over-collection after 400 commits without also declaring that it should fail the build,
/// and this is what lets it.
pub fn uncited(checks: &[Check]) -> Vec<Eligible> {
    let mut out: Vec<Eligible> = checks
        .iter()
        .filter(|c| c.id == "orphan-in")
        .flat_map(|c| c.violations.iter().map(move |v| (c, v)))
        .filter(|(_, v)| v.age.is_some())
        .map(|(c, v)| Eligible {
            check: c.id.to_string(),
            node: v.node.clone(),
            detail: v.detail.clone(),
            age: v.age.clone(),
        })
        .collect();
    out.sort_by(|a, b| a.node.cmp(&b.node));
    out
}

/// The node's stem, for a subject line: `.yidam/corpus/concept/low-flow.yml` → `low-flow`.
fn stem(node: &str) -> &str {
    node.rsplit('/')
        .next()
        .unwrap_or(node)
        .trim_end_matches(".yml")
        .trim_end_matches(".md")
}

/// A subject line's tail, cut to something a `git log --oneline` can hold.
///
/// The verb and stem are already spent, so this is what remains of 72 columns. Cut on a word
/// boundary: a subject ending mid-word reads as truncated output rather than as a sentence
/// somebody meant.
fn subject_tail(detail: &str, spent: usize) -> String {
    let flat = detail.split_whitespace().collect::<Vec<_>>().join(" ");
    let room = 72usize.saturating_sub(spent);
    if flat.chars().count() <= room {
        return flat;
    }
    let mut cut = String::new();
    for word in flat.split(' ') {
        if cut.chars().count() + word.chars().count() + 2 > room {
            break;
        }
        if !cut.is_empty() {
            cut.push(' ');
        }
        cut.push_str(word);
    }
    format!(
        "{}…",
        cut.trim_end()
            .trim_end_matches([',', ';', ':', '—', '-'])
            .trim_end()
    )
}

/// Column a commit message body wraps at.
///
/// Not [`WRAP`]: a message is read through `git log`, which indents it by four, and the
/// conventional 72 is what keeps that inside eighty. A body left unwrapped renders as one
/// line per paragraph and is unreadable in exactly the place it is meant to be reviewed.
const MESSAGE_WRAP: usize = 72;

/// Wrap `text` to `width` columns at `indent` spaces.
fn wrap_at(text: &str, indent: usize, width: usize) -> String {
    let mut out = String::new();
    let mut line = " ".repeat(indent);
    let base = line.len();
    for word in text.split_whitespace() {
        if line.len() > base && line.chars().count() + 1 + word.chars().count() > width {
            out.push_str(line.trim_end());
            out.push('\n');
            line = " ".repeat(indent);
        }
        if line.len() > base {
            line.push(' ');
        }
        line.push_str(word);
    }
    if line.len() > base {
        out.push_str(line.trim_end());
    }
    out
}

/// Wrap `text` to [`WRAP`] columns at `indent` spaces — corpus prose.
fn wrap(text: &str, indent: usize) -> String {
    wrap_at(text, indent, WRAP)
}

/// Wrap a commit body, paragraph by paragraph, leaving the blank lines between them.
fn wrap_body(text: &str) -> String {
    text.split("\n\n")
        .map(|para| wrap_at(para, 0, MESSAGE_WRAP))
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// The paragraph an `open:` proposal appends, unwrapped.
fn paragraph(head: &str, check: &str, detail: &str) -> String {
    format!(
        "{MARKER}{head} [{check}] — {}. {FRAMING} [open]",
        detail.trim().trim_end_matches('.')
    )
}

/// Where a node's `description:` block scalar begins and ends, and how far it is indented.
///
/// Returns `(first content line, one past the last, indent)`. `None` when the node has no
/// top-level `description:`, or has one that is not a block scalar — which is refused rather
/// than converted. Rewriting `description: One line.` into a `|` block would reformat a line
/// a person wrote, and a proposal whose diff is mostly re-indentation is one nobody can
/// review.
fn description_block(lines: &[&str]) -> Option<(usize, usize, usize)> {
    let start = lines.iter().position(|l| {
        let t = l.trim_end();
        t.starts_with("description:") && t.len() > "description:".len() && {
            let v = t["description:".len()..].trim();
            v == "|" || v == "|-" || v == "|+" || v == ">" || v == ">-"
        }
    })?;
    let indent = lines
        .get(start + 1)?
        .chars()
        .take_while(|c| *c == ' ')
        .count();
    if indent == 0 {
        return None;
    }
    let mut end = start + 1;
    while end < lines.len() {
        let l = lines[end];
        // A blank line belongs to the block; only content at or left of the key ends it.
        if !l.trim().is_empty() && l.chars().take_while(|c| *c == ' ').count() < indent {
            break;
        }
        end += 1;
    }
    // Trailing blanks belong to whatever follows, not to the block.
    while end > start + 1 && lines[end - 1].trim().is_empty() {
        end -= 1;
    }
    Some((start + 1, end, indent))
}

/// Append a paragraph to a node's `description:` block, leaving every other byte alone.
///
/// Textual, not a `serde_yaml` round-trip. Re-emitting the document would rewrite key order,
/// comments and block style across a file somebody authored, so the diff a reviewer opens
/// would be the whole node instead of the three lines that were added. A proposal has to be
/// readable as a diff or the branch is not reviewable, which is the entire premise.
pub fn append_to_description(text: &str, para: &str) -> Option<String> {
    let lines: Vec<&str> = text.split('\n').collect();
    let (_, end, indent) = description_block(&lines)?;
    let mut out: Vec<String> = lines[..end].iter().map(|s| s.to_string()).collect();
    out.push(String::new());
    out.push(wrap(para, indent));
    out.extend(lines[end..].iter().map(|s| s.to_string()));
    Some(out.join("\n"))
}

/// One paragraph this command wrote, found in a node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Marked {
    /// The check whose finding it carried.
    pub check: String,
    /// Line range within the file, for removal.
    pub from: usize,
    pub to: usize,
    /// The paragraph as it stands, joined — quoted into the `close:` commit body so the
    /// commit still carries what it retires.
    pub text: String,
}

/// Every paragraph in `text` that this command wrote.
///
/// Identified by [`MARKER`] and nothing else. A paragraph a person typed the marker into by
/// hand would be matched, and that is acceptable: the failure mode is a proposal to remove a
/// line claiming to be generated, which a reviewer reads and rejects.
pub fn marked(text: &str) -> Vec<Marked> {
    let lines: Vec<&str> = text.split('\n').collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let Some(at) = lines[i].find(MARKER) else {
            i += 1;
            continue;
        };
        let rest = &lines[i][at + MARKER.len()..];
        // `<sha> [<check>] — …`. A marker whose check id is unreadable is left alone rather
        // than guessed at: this command must not remove a line it cannot account for.
        let Some(check) = rest
            .split_once('[')
            .and_then(|(_, r)| r.split_once(']'))
            .map(|(c, _)| c.to_string())
        else {
            i += 1;
            continue;
        };
        let from = i;
        // The paragraph ends at a blank line *or* at the end of the block scalar holding it.
        // Stopping only on the blank was wrong in the one case that always occurs: an
        // appended paragraph is the last thing in `description:`, so the next non-blank line
        // is `properties:` — and the removal took the rest of the node with it.
        let indent = lines[i].chars().take_while(|c| *c == ' ').count();
        let mut to = i + 1;
        while to < lines.len() {
            let l = lines[to];
            if l.trim().is_empty() || l.chars().take_while(|c| *c == ' ').count() < indent {
                break;
            }
            to += 1;
        }
        out.push(Marked {
            check,
            from,
            to,
            text: lines[from..to]
                .join(" ")
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" "),
        });
        i = to;
    }
    out
}

/// Remove one marked paragraph, and the blank line that separated it from what precedes it.
pub fn strip(text: &str, m: &Marked) -> String {
    let lines: Vec<&str> = text.split('\n').collect();
    // The paragraph was appended after a blank; take that blank with it so the block does
    // not accumulate an empty line per closed question.
    let from = if m.from > 0 && lines[m.from - 1].trim().is_empty() {
        m.from - 1
    } else {
        m.from
    };
    let mut out: Vec<&str> = lines[..from].to_vec();
    out.extend(&lines[m.to..]);
    out.join("\n")
}

/// Drop `node` from a catalog entry's `used-by:` list.
///
/// A withdrawal that left the node listed would trade one finding for a
/// `catalog-used-by-drift`, which is not a proposal, it is a swap.
pub fn drop_used_by(text: &str, node: &str) -> Option<String> {
    let target = stem(node);
    let lines: Vec<&str> = text.split('\n').collect();
    let keep: Vec<&str> = lines
        .iter()
        .copied()
        .filter(|l| {
            let t = l.trim();
            !(t.starts_with("- ") && stem(t.trim_start_matches("- ").trim()) == target)
        })
        .collect();
    (keep.len() != lines.len()).then(|| keep.join("\n"))
}

/// The `open:` proposal for one gating finding, or `None` when the node cannot take one.
///
/// `None` is the node whose `description:` is a plain scalar. It is a refusal and not a
/// failure — see [`append_to_description`] — and the run reports it rather than swallowing
/// it, because a finding silently not proposed about is the failure mode this command exists
/// to remove.
pub fn open_proposal(head: &str, e: &Eligible, text: &str) -> Option<Proposal> {
    let para = paragraph(head, &e.check, &e.detail);
    let content = append_to_description(text, &para)?;
    let subject = format!(
        "{} — {}",
        stem(&e.node),
        subject_tail(&e.detail, stem(&e.node).chars().count() + 9)
    );
    let body = format!(
        "{}\n\nRecorded against `{}` as an [open] paragraph, so the question is where \
         `yidam open-questions` and the claim tools already look. Nothing else in the node \
         was touched, and no claim was re-tagged.",
        e.detail.trim(),
        e.node
    );
    Some(Proposal {
        verb: Verb::Open,
        subject,
        body,
        check: e.check.clone(),
        node: e.node.clone(),
        detail: e.detail.clone(),
        changes: vec![Change::Write {
            path: e.node.clone(),
            content,
        }],
    })
}

/// The `withdraw:` proposal for an uncited node past the corpus's declared threshold.
///
/// `catalogs` is every catalog entry as `(repo-relative path, text)`; an entry listing this
/// node in `used-by:` is rewritten in the same commit.
pub fn withdraw_proposal(
    head: &str,
    e: &Eligible,
    threshold: usize,
    catalogs: &[(String, String)],
) -> Option<Proposal> {
    let age = e.age.as_ref()?;
    if age.commits < threshold {
        return None;
    }
    let mut changes = vec![Change::Remove {
        path: e.node.clone(),
    }];
    for (path, text) in catalogs {
        if let Some(content) = drop_used_by(text, &e.node) {
            changes.push(Change::Write {
                path: path.clone(),
                content,
            });
        }
    }
    changes.sort_by(|a, b| a.path().cmp(b.path()));
    let _ = head;
    let subject = format!(
        "{} — uncited for {} corpus commit(s)",
        stem(&e.node),
        age.commits
    );
    let body = format!(
        "{}\n\n`.yidam/config.toml` declares `[propose] withdraw_uncited_after = {threshold}`, \
         and this finding has held for {}. That declaration is what licenses the deletion: no \
         finding says a node should go, and this corpus said it about itself.\n\nNothing \
         replaces it. If that is wrong, the fix is an edge, and not this commit.",
        e.detail.trim(),
        age.commits
    );
    Some(Proposal {
        verb: Verb::Withdraw,
        subject,
        body,
        check: e.check.clone(),
        node: e.node.clone(),
        detail: e.detail.clone(),
        changes,
    })
}

/// The `close:` proposal retiring one paragraph this command wrote.
///
/// It says what it is doing and what it is not: the finding stopped being reported, which is
/// not the same event as somebody answering the question. Conflating them would be this
/// command deciding a question is settled, which is the resolution event Article V confines
/// to a sangha.
pub fn close_proposal(node: &str, text: &str, m: &Marked) -> Proposal {
    let body = format!(
        "{}\n\nThe [{}] finding this question carried is no longer reported at `{node}`. \
         Removing the paragraph retires the question `yidam propose` opened. It does not \
         decide that the question was answered, and it never touches prose a person wrote.",
        m.text, m.check
    );
    Proposal {
        verb: Verb::Close,
        subject: format!(
            "{} — the [{}] finding this question carried is gone",
            stem(node),
            m.check
        ),
        body,
        check: m.check.clone(),
        node: node.to_string(),
        detail: m.text.clone(),
        changes: vec![Change::Write {
            path: node.to_string(),
            content: strip(text, m),
        }],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cmd::lint::model::{Severity, Violation};

    fn aged(node: &str, detail: &str, commits: usize) -> Violation {
        Violation {
            node: node.into(),
            detail: detail.into(),
            severity: None,
            age: Some(Age {
                sha: "abc1234".into(),
                ts: 1_700_000_000,
                commits,
            }),
        }
    }

    fn check(id: &'static str, violations: Vec<Violation>) -> Check {
        Check::new(id, "T", Severity::Error, "r", violations)
    }

    const NODE: &str = "class: concept\nlabel: Low flow\ndescription: |\n  \
                        A statistic of the low end of the record. [verified]\n\n  \
                        It is only as meaningful as the record beneath it. [inference]\nproperties:\n  \
                        claim_tag: verified\nlinks:\n  - target: ../concept.ont.yml\n    \
                        relationship: instance-of\n";

    #[test]
    fn only_gating_findings_are_eligible() {
        let checks = vec![check(
            "orphan-in",
            vec![
                aged("a.yml", "nothing links to this node", 200),
                aged("b.yml", "nothing links to this node", 3),
            ],
        )];
        let gating = BTreeSet::from([("orphan-in".to_string(), "a.yml".to_string())]);
        let e = eligible(&checks, &gating);
        assert_eq!(
            e.len(),
            1,
            "a finding the baseline forgives is not proposed about"
        );
        assert_eq!(e[0].node, "a.yml");
    }

    /// Withdrawal reads the check and not the gate: a corpus may declare that an uncited
    /// node is over-collection without declaring that it should fail the build.
    #[test]
    fn withdrawal_candidates_do_not_depend_on_the_gate() {
        let checks = vec![check(
            "orphan-in",
            vec![aged("a.yml", "nothing links to this node", 500)],
        )];
        assert_eq!(eligible(&checks, &BTreeSet::new()).len(), 0);
        assert_eq!(uncited(&checks).len(), 1);
    }

    /// A finding with no clock cannot be withdrawn on a residence threshold, because there
    /// is nothing for the threshold to read.
    #[test]
    fn an_undated_orphan_is_not_a_withdrawal_candidate() {
        let checks = vec![check(
            "orphan-in",
            vec![Violation::new("a.yml", "nothing links to this node")],
        )];
        assert!(uncited(&checks).is_empty());
    }

    #[test]
    fn a_paragraph_lands_at_the_end_of_the_description_and_nowhere_else() {
        let para = paragraph("8d35441", "orphan-in", "nothing links to this node");
        let out = append_to_description(NODE, &para).expect("block description");
        let lines: Vec<&str> = out.split('\n').collect();
        let at = lines.iter().position(|l| l.contains(MARKER)).unwrap();
        let props = lines
            .iter()
            .position(|l| l.starts_with("properties:"))
            .unwrap();
        assert!(
            at < props,
            "the paragraph is inside the block, not after it:\n{out}"
        );
        assert!(lines[at].starts_with("  "), "it is indented into the block");
        assert!(
            out.contains("nothing links to this node"),
            "the finding is carried verbatim"
        );
        assert!(out.contains("[open]"), "and it is tagged as a question");
        // Every other line is untouched.
        for original in NODE.split('\n').filter(|l| !l.trim().is_empty()) {
            assert!(out.contains(original), "rewrote {original:?}");
        }
    }

    /// A plain scalar is refused rather than converted. Rewriting it into a block would
    /// reformat a line somebody wrote, and the diff would stop being reviewable.
    #[test]
    fn a_plain_scalar_description_is_refused() {
        let node = "class: concept\ndescription: One line.\n";
        assert!(append_to_description(node, "x").is_none());
    }

    #[test]
    fn a_paragraph_wraps_to_the_width_the_corpus_writes_at() {
        let para = paragraph(
            "8d35441",
            "orphan-in",
            "nothing links to this node — uncited since 2025-11-02, 214 commit(s)",
        );
        let out = append_to_description(NODE, &para).unwrap();
        for line in out.split('\n') {
            assert!(
                line.chars().count() <= WRAP,
                "over {WRAP} columns: {line:?}"
            );
        }
    }

    #[test]
    fn a_marked_paragraph_is_found_and_removed_and_leaves_the_node_as_it_was() {
        let para = paragraph("8d35441", "orphan-in", "nothing links to this node");
        let with = append_to_description(NODE, &para).unwrap();
        let found = marked(&with);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].check, "orphan-in");
        assert!(found[0].text.contains("nothing links to this node"));
        assert_eq!(strip(&with, &found[0]), NODE, "the round trip is exact");
    }

    /// Prose a person wrote is not this command's to remove, however much it looks like a
    /// question.
    #[test]
    fn an_unmarked_question_is_not_matched() {
        let node = NODE.replace(
            "[inference]",
            "[inference]\n\n  Whether this belongs here is unresolved. [open]",
        );
        assert!(marked(&node).is_empty());
    }

    #[test]
    fn two_marked_paragraphs_are_found_separately() {
        let one = append_to_description(NODE, &paragraph("aaa1111", "orphan-in", "first finding"))
            .unwrap();
        let two = append_to_description(
            &one,
            &paragraph("bbb2222", "dangling-edge", "second finding"),
        )
        .unwrap();
        let found = marked(&two);
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].check, "orphan-in");
        assert_eq!(found[1].check, "dangling-edge");
        // Removing the second restores the first exactly.
        assert_eq!(strip(&two, &found[1]), one);
    }

    #[test]
    fn a_withdrawn_node_leaves_the_catalog_that_listed_it() {
        let cat = "---\nname: usgs\nused-by:\n  - ../corpus/gage/a.yml\n  - ../corpus/gage/b.yml\n---\n\n# USGS\n";
        let out = drop_used_by(cat, ".yidam/corpus/gage/a.yml").expect("a listed node");
        assert!(!out.contains("gage/a.yml"));
        assert!(out.contains("gage/b.yml"), "and the others stay");
        assert!(
            drop_used_by(cat, ".yidam/corpus/gage/z.yml").is_none(),
            "unlisted is no change"
        );
    }

    #[test]
    fn a_subject_is_cut_on_a_word_boundary() {
        let long = "nothing links to this node and the sentence continues well past the room a subject line has";
        let tail = subject_tail(long, 20);
        assert!(tail.ends_with('…'));
        assert!(!tail.contains("  "));
        assert!(tail.chars().count() <= 72 - 20);
        assert!(long.starts_with(tail.trim_end_matches('…').trim_end()));
    }

    #[test]
    fn stems_drop_the_directory_and_the_extension() {
        assert_eq!(stem(".yidam/corpus/concept/low-flow.yml"), "low-flow");
        assert_eq!(stem(".yidam/catalog/usgs-nwis.md"), "usgs-nwis");
    }
}
