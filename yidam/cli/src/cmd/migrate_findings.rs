//! `yidam migrate findings` — lifting a legacy `propose` paragraph into a record.
//!
//! #712 made a finding a record under the node's own `yidam:` key, with an id that is a digest
//! of the check and the finding's words. It deliberately kept
//! [`marked`](crate::cmd::propose::draft::marked) and
//! [`strip`](crate::cmd::propose::draft::strip), so a corpus carrying paragraphs written by a
//! released binary still has them recognised and still has them closed when the finding goes
//! away. Nothing stranded, and nothing was lifted either. This is the one-time lift.
//!
//! # What a paragraph was
//!
//! A released binary composed exactly one shape, and the whole of this migration is reading it
//! back:
//!
//! ```text
//! Opened by `yidam propose` at 8d35441 [orphan-in] — nothing links to this node. What follows
//! from that is unresolved here: `yidam propose` carries findings into the corpus and does not
//! answer them. [open]
//! ```
//!
//! The marker, a short commit, the check in brackets, the finding's own words, a fixed framing
//! sentence, and the `[open]` tag that put it in the corpus's claim count. The generator is
//! gone — #712 deleted it — so [`FRAMING`] is written here rather than imported. That is the
//! right home for it: it is not a sentence this tool composes any more, it is a string this
//! tool has to recognise in corpora written before it stopped.
//!
//! # A paragraph reworded past recognition stays prose
//!
//! The reconstruction is exact or it does not happen. The framing sentence must be present
//! verbatim, the `[open]` tag must close the paragraph, and the check must be readable from the
//! marker — [`marked`](crate::cmd::propose::draft::marked) already declines the last of those,
//! for the reason it gives: *this command must not remove a line it cannot account for.*
//!
//! Anything else is somebody's argument now. An author who rewrote the sentence around the
//! finding wrote prose, and there is no rule that says where the tool's words ended and theirs
//! began. The report counts those rather than listing them, for the reason
//! [`crate::cmd::migrate_references`] gives about its own prose details: printing them as work
//! to do would tell an author to fix sentences that are correct.
//!
//! # What the id can and cannot recover
//!
//! The generator wrote `{detail}` through `trim_end_matches('.')`, so a detail that ended in a
//! period comes back without it, and [`id_for`](crate::findings::id_for) over the recovered
//! words is then not the id a fresh `propose` run computes for the same live finding.
//!
//! **That is survivable, and it is worth being explicit about why**, because #728 raises it as
//! the thing to watch. `close:` matches on `(check, node)` and not on the id, and `propose`'s
//! re-open guard is keyed on the check as well. So a lifted record whose id drifts by a period
//! still closes when the check stops reporting, and still suppresses a second copy of the
//! question in the meantime. What the id governs is [`crate::findings::add`]'s dedupe within one
//! node, where the comparison is between records and not against a recomputed one.
//!
//! The alternative — restoring a period the corpus does not say was there — would change the
//! finding's words to make a digest come out even. `detail` is the field RFC-0020's carriage
//! constraint rests on. It is recovered as written or the paragraph is left alone.
//!
//! `opened_at` *is* recovered exactly: the short commit sits between the marker and the check,
//! and it is deliberately not part of the id, so the record says when the question was actually
//! opened rather than when it was lifted.

use std::path::Path;

use super::migrate::MigrateReport;
use super::propose::draft::{marked, strip, Marked, MARKER};
use crate::findings::{self, Finding};
use crate::paths::yidam_catalog_dir;

/// The sentence a released binary wrapped every finding in.
///
/// Not imported: #712 deleted the generator that wrote it. Recognising it is the last thing
/// this string is for.
const FRAMING: &str = "What follows from that is unresolved here: `yidam propose` carries \
                       findings into the corpus and does not answer them.";

/// The tag that put a carried finding in the corpus's own claim count — the first of the two
/// numbers this migration moves.
const OPEN: &str = "[open]";

/// One paragraph lifted into one record.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Lifted {
    /// The node or catalog entry, repository-relative.
    pub node: String,
    /// 1-based line the paragraph started on.
    pub line: usize,
    /// The check it carried.
    pub check: String,
    /// The record's id.
    pub id: String,
}

/// What one file's rewrite consists of. The single decision both `plan` and `apply` read.
///
/// The rewritten text is carried rather than a set of spans, for the reason
/// [`crate::cmd::migrate_references`] states about its own plan: the two passes are two reads of
/// the same file, and a recorded line number is invalid the moment a paragraph above it goes.
/// Here it is stronger still — [`findings::add`] rewrites the `yidam:` block, so every line
/// number in the file moves at least once.
#[derive(Debug, Default)]
struct FilePlan {
    lifts: Vec<Lifted>,
    /// Paragraphs reworded past recognition.
    prose: usize,
    /// The file as it would stand, when anything was lifted.
    out: Option<String>,
    /// The record could not be put where a reader would find it.
    no_region: bool,
}

/// The finding a paragraph carried, or `None` when it is no longer the tool's sentence.
///
/// `m.text` is the paragraph joined and whitespace-collapsed, which is what makes this a match
/// against one line rather than an unwrap of a wrapped block.
fn reconstruct(m: &Marked) -> Option<Finding> {
    let at = m.text.find(MARKER)?;
    let rest = &m.text[at + MARKER.len()..];
    let (opened_at, rest) = rest.split_once(" [")?;
    let (check, rest) = rest.split_once("] — ")?;
    // `marked` read the check out of the same bracket. Disagreement here means the shape is not
    // the one this understands, whatever else matched.
    if check != m.check {
        return None;
    }
    // `{detail}. {FRAMING} {OPEN}` — the generator's format string, read backwards. Nothing
    // else is accepted: see the module note on why a reworded paragraph stays prose.
    let detail = rest.strip_suffix(&format!(". {FRAMING} {OPEN}"))?;
    if detail.trim().is_empty() {
        return None;
    }
    // `opened_at` is the commit the paragraph recorded, not the one the lift runs at: a
    // migration that restamped every question to today would erase how long each has been open.
    Some(Finding::open(check, detail, opened_at))
}

/// Whether a record written to this document lands where a reader would find it.
///
/// Stricter than [`crate::cmd::propose`] on one point, deliberately. `propose` adds a record and
/// changes nothing else, so a catalog entry with no frontmatter costs a misplaced block. This
/// *removes the prose in the same breath*, so the same case would cost the question itself.
fn writable(file: &str, text: &str) -> bool {
    if file.ends_with(".md") && !text.trim_start_matches('\n').starts_with("---\n") {
        return false;
    }
    findings::write(text, &findings::parse(text)).is_some()
}

/// Every paragraph in one file, and the file as it would stand once they are records.
///
/// Strip first and add second, one paragraph at a time, re-reading after each removal — the
/// loop `propose`'s `close:` already runs, and for the same reason. The records go on at the end
/// because [`findings::add`] writes a block whose position no paragraph's line number survives.
fn plan_file(file: &str, text: &str) -> FilePlan {
    let mut plan = FilePlan::default();
    let mut out = text.to_string();
    let mut found: Vec<(Marked, Option<Finding>)> = marked(&out)
        .into_iter()
        .map(|m| {
            let f = reconstruct(&m);
            (m, f)
        })
        .collect();
    plan.prose = found.iter().filter(|(_, f)| f.is_none()).count();
    found.retain(|(_, f)| f.is_some());
    if found.is_empty() {
        return plan;
    }
    if !writable(file, text) {
        plan.no_region = true;
        return plan;
    }

    let mut records: Vec<Finding> = Vec::new();
    loop {
        let Some((m, f)) = marked(&out)
            .into_iter()
            .find_map(|m| reconstruct(&m).map(|f| (m, f)))
        else {
            break;
        };
        plan.lifts.push(Lifted {
            node: file.to_string(),
            line: m.from + 1,
            check: f.check.clone(),
            id: f.id.clone(),
        });
        out = strip(&out, &m);
        records.push(f);
    }
    for f in &records {
        // `None` is the node already carrying that id — a corpus part-way through a lift, or one
        // whose paragraph duplicated a record. The paragraph is still gone, which is the whole
        // point; there is nothing further to write.
        if let Some(next) = findings::add(&out, f) {
            out = next;
        }
    }
    plan.out = Some(out);
    plan
}

/// `(path, repository-relative path, text)` for every file a paragraph can be in.
///
/// Instances and catalog entries both — the pair `propose`'s `close:` walks, and not the
/// instances alone: a source record's expiry was carried as a paragraph in the entry's body.
fn documents(root: &Path, corpus: &Path) -> Vec<(std::path::PathBuf, String, String)> {
    crate::walk::walk_corpus_instances(corpus)
        .into_iter()
        .chain(crate::walk::walk_md_files(&yidam_catalog_dir(root)))
        .filter_map(|p| {
            let text = std::fs::read_to_string(&p).ok()?;
            let file = p
                .strip_prefix(root)
                .unwrap_or(&p)
                .to_string_lossy()
                .replace('\\', "/");
            Some((p, file, text))
        })
        .collect()
}

/// Every paragraph in the corpus, and the record it becomes.
pub(crate) fn plan(root: &Path, corpus: &Path, report: &mut MigrateReport) {
    for (_, file, text) in documents(root, corpus) {
        let p = plan_file(&file, &text);
        report.prose_findings += p.prose;
        if p.no_region {
            report.blocked.push(format!(
                "{file}: no frontmatter the record could be written into, so lifting the \
                 paragraph would lose the question"
            ));
            continue;
        }
        report.findings.extend(p.lifts);
    }
}

/// Write it. Re-derives the plan, for the reason [`FilePlan`] gives.
pub(crate) fn apply(root: &Path, corpus: &Path, report: &mut MigrateReport) -> anyhow::Result<()> {
    for (path, file, text) in documents(root, corpus) {
        let p = plan_file(&file, &text);
        let Some(out) = p.out else { continue };
        // The migration must not produce a state the corpus's own parser cannot read — the
        // argument `migrate references` makes about its own rewrite. A node whose YAML this
        // broke would fail `malformed-yaml` on the next lint, and the reader of that finding
        // would have no way to know a migration caused it.
        let yaml = if file.ends_with(".md") {
            out.trim_start_matches('\n')
                .strip_prefix("---\n")
                .and_then(|r| r.split_once("\n---"))
                .map(|(y, _)| y.to_string())
        } else {
            Some(out.clone())
        };
        if yaml.is_none_or(|y| serde_yaml::from_str::<serde_yaml::Value>(&y).is_err()) {
            report
                .blocked
                .push(format!("{file}: the rewrite would not parse as YAML"));
            continue;
        }
        std::fs::write(&path, out)?;
    }
    Ok(())
}

/// One line, for the commit subject and the record's summary.
pub(crate) fn summary(report: &MigrateReport) -> String {
    let nodes = report
        .findings
        .iter()
        .map(|l| l.node.as_str())
        .collect::<std::collections::BTreeSet<_>>()
        .len();
    format!(
        "{} legacy paragraph(s) into records across {nodes} node(s)",
        report.findings.len(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const NODE: &str = "class: gage\nlabel: Canyon\ndescription: |\n  It measures flow.\nproperties:\n  claim_tag: verified\n";

    /// A paragraph as a released binary actually wrote one.
    ///
    /// A literal fixture, for the reason `draft.rs`'s legacy tests give: the machinery that
    /// wrote these is gone, and composing one from this module's own [`FRAMING`] would test the
    /// constant against itself rather than against what a corpus holds.
    fn legacy(check: &str, detail: &str) -> String {
        NODE.replace(
            "properties:",
            &format!(
                "\n  Opened by `yidam propose` at 8d35441 [{check}] — {detail}. What follows \
                 from that is\n  unresolved here: `yidam propose` carries findings into the \
                 corpus and does\n  not answer them. [open]\nproperties:"
            ),
        )
    }

    fn only(text: &str) -> Marked {
        let m = marked(text);
        assert_eq!(m.len(), 1, "fixture should hold one paragraph");
        m.into_iter().next().unwrap()
    }

    #[test]
    fn a_paragraph_reconstructs_to_the_finding_it_carried() {
        let text = legacy("orphan-in", "nothing links to this node");
        let f = reconstruct(&only(&text)).expect("reconstructed");
        assert_eq!(f.check, "orphan-in");
        assert_eq!(f.detail, "nothing links to this node");
        assert_eq!(f.standing, "open");
    }

    /// The commit the question was opened at, not the commit it was lifted at. `opened_at` is
    /// not part of the id, so nothing forces this — which is exactly why it is asserted.
    #[test]
    fn the_commit_the_question_was_opened_at_is_recovered() {
        let text = legacy("orphan-in", "nothing links to this node");
        assert_eq!(reconstruct(&only(&text)).unwrap().opened_at, "8d35441");
    }

    #[test]
    fn a_reworded_paragraph_is_not_reconstructed() {
        let text = legacy("orphan-in", "nothing links to this node")
            .replace("What follows from that is", "I think what follows is");
        assert!(reconstruct(&only(&text)).is_none());
    }

    /// The paragraph goes and the record arrives, and nothing between them is touched.
    #[test]
    fn a_lift_replaces_the_paragraph_with_a_record() {
        let text = legacy("orphan-in", "nothing links to this node");
        let p = plan_file("corpus/gage/canyon.yml", &text);
        assert_eq!(p.lifts.len(), 1);
        assert_eq!(p.prose, 0);
        let out = p.out.expect("rewritten");
        assert!(!out.contains(MARKER), "the paragraph survived:\n{out}");
        let f = findings::parse(&out);
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].check, "orphan-in");
        assert_eq!(f[0].detail, "nothing links to this node");
        assert!(out.contains("It measures flow."));
        assert!(out.contains("claim_tag: verified"));
    }

    /// The `[open]` tag went with the paragraph. That is the claim count dropping, which is the
    /// first of the two numbers #728 says move.
    #[test]
    fn the_open_tag_leaves_the_prose() {
        let text = legacy("orphan-in", "nothing links to this node");
        let out = plan_file("corpus/gage/canyon.yml", &text).out.unwrap();
        assert!(!out.contains(OPEN), "{out}");
    }

    /// And the node is still an open question, because the record is. The second number.
    #[test]
    fn the_node_is_still_an_open_question() {
        let text = legacy("orphan-in", "nothing links to this node");
        let out = plan_file("corpus/gage/canyon.yml", &text).out.unwrap();
        let fields: Vec<String> = vec![];
        assert!(crate::claims::is_open_question("Canyon", &out, &fields));
    }

    #[test]
    fn a_reworded_paragraph_is_counted_and_left_where_it_stands() {
        let text = legacy("orphan-in", "nothing links to this node")
            .replace("carries findings into the", "carries the");
        let p = plan_file("corpus/gage/canyon.yml", &text);
        assert_eq!(p.prose, 1);
        assert!(p.lifts.is_empty());
        assert!(
            p.out.is_none(),
            "a file with nothing to lift is not rewritten"
        );
    }

    /// Two paragraphs on one node: the second's line numbers move when the first goes, which is
    /// the case the one-at-a-time loop exists for.
    #[test]
    fn two_paragraphs_on_one_node_both_lift() {
        let mut text = legacy("orphan-in", "nothing links to this node");
        text = text.replace(
            "properties:",
            "\n  Opened by `yidam propose` at 8d35441 [missing-description] — it says \
             nothing. What follows\n  from that is unresolved here: `yidam propose` \
             carries findings into the corpus\n  and does not answer them. [open]\n\
             properties:",
        );
        let p = plan_file("corpus/gage/canyon.yml", &text);
        assert_eq!(p.lifts.len(), 2);
        let out = p.out.unwrap();
        assert!(!out.contains(MARKER), "{out}");
        let checks: Vec<String> = findings::parse(&out).into_iter().map(|f| f.check).collect();
        assert_eq!(checks, vec!["orphan-in", "missing-description"]);
    }

    /// A lift is not a second copy of a question the node already records.
    #[test]
    fn a_lift_is_idempotent() {
        let text = legacy("orphan-in", "nothing links to this node");
        let once = plan_file("corpus/gage/canyon.yml", &text).out.unwrap();
        let twice = plan_file("corpus/gage/canyon.yml", &once);
        assert!(twice.lifts.is_empty());
        assert_eq!(twice.prose, 0);
        assert!(twice.out.is_none());
    }

    /// A catalog entry's paragraph goes to the frontmatter, not to the end of the prose.
    #[test]
    fn a_catalog_entry_lifts_into_its_frontmatter() {
        let text = format!(
            "---\ntitle: A report\nttl_days: 90\n---\n\n# A report\n\nIt says things.\n\n\
             Opened by `yidam propose` at 8d35441 [source-expired] — this record is older than \
             its ttl. {FRAMING} {OPEN}\n"
        );
        let p = plan_file("catalog/report.md", &text);
        assert_eq!(p.lifts.len(), 1);
        let out = p.out.expect("rewritten");
        assert!(!out.contains(MARKER), "{out}");
        assert!(out.contains("It says things."));
        let (front, body) = out.trim_start().split_once("\n---").expect("frontmatter");
        assert!(
            front.contains("yidam:"),
            "the record is not in the frontmatter:\n{out}"
        );
        assert!(!body.contains("yidam:"));
        assert_eq!(findings::parse(&out)[0].check, "source-expired");
    }

    /// The case this refuses, and why: removing the prose here would lose the question.
    #[test]
    fn a_markdown_file_with_no_frontmatter_is_refused_rather_than_stripped() {
        let text = format!(
            "# A report\n\nIt says things.\n\nOpened by `yidam propose` at 8d35441 \
             [source-expired] — this record is older than its ttl. {FRAMING} {OPEN}\n"
        );
        let p = plan_file("catalog/report.md", &text);
        assert!(p.no_region);
        assert!(p.lifts.is_empty());
        assert!(p.out.is_none());
    }
}
