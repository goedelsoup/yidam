//! A finding carried into the corpus, as a record rather than as a sentence.
//!
//! `yidam propose` opens a question by putting the finding where a reader of the node will
//! meet it. It used to do that by splicing a paragraph into the `description:` block, and
//! identifying it again on the way back out by searching the prose for the literal English
//! string `"Opened by \`yidam propose\` at "`. Four things followed, and none of them was a
//! bug in the code that implemented it:
//!
//! - **A reworded question could never be closed.** `close:` matched the marker, and editing
//!   a sentence sitting in the middle of your own argument is the likeliest thing an author
//!   does to it.
//! - **A finding could not be counted apart from a claim.** The paragraph ended in `[open]`,
//!   so it entered the corpus's own open-question tally beside the questions a person raised,
//!   and no corpus could ask how many of its open questions were its own.
//! - **Nothing could be asked of the set** — which questions are outstanding here, from which
//!   check, opened at which commit.
//! - The removal was line arithmetic over a block scalar, and it had already taken the rest
//!   of a node with it once.
//!
//! # The record
//!
//! ```yaml
//! yidam:
//!   findings:
//!     - id: 7f3a1c94b2e1
//!       check: orphan-in
//!       opened_at: 4f2a1c9
//!       detail: 'nothing links to this node — uncited since 2026-03-04, 3 commit(s)'
//!       standing: open
//! ```
//!
//! # Under `yidam:`, and not at the top level
//!
//! The obvious key is `findings:`, and it is taken. One derived corpus writes a top-level
//! `findings:` holding **prose** — its own research findings — and 117 nodes of 117 in
//! another carry keys this tool never declared. Claiming a bare noun at the top level would
//! collide with the corpus's own vocabulary in exactly the repositories that have one.
//!
//! So the tool claims one key, named after itself, and the corpus keeps the rest of the top
//! level. That also says what the block is: a reader meeting `yidam:` knows the contents are
//! the tool's record and not the corpus's content.
//!
//! # Re-serialized wholesale, which the prose could not be
//!
//! The prose form did textual surgery — locate the block scalar, measure its indent, splice
//! between lines — because re-emitting the document would rewrite key order, comments and
//! block style across a file somebody authored, and a proposal whose diff is the whole node
//! is not reviewable. That machinery is gone with the paragraphs it existed for.
//!
//! That argument does not reach here, and the difference is ownership. Everything under
//! `yidam:` was written by this tool, so nothing is lost by rendering it fresh: there are no
//! comments to drop and no author's line breaks to reflow. The surgery is therefore confined
//! to *finding the block* — the rest of the file is copied byte for byte.
//!
//! # The identity
//!
//! [`Finding::id`] is a digest of the check and the finding's own words, so the same finding
//! computes the same id at any HEAD. That is what makes a re-run additive rather than a
//! second copy of everything outstanding, and what lets `close:` retire a question whose
//! prose an author has since rewritten — the thing the marker could not survive.
//!
//! `opened_at` is recorded and deliberately **not** part of the id: the same finding opened
//! at a later commit is the same question, and keying on the commit would reopen it on every
//! run.

use sha2::{Digest, Sha256};

/// The container key. See the module note on why it is namespaced.
pub const CONTAINER: &str = "yidam";

/// One finding this tool carried into the corpus.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Finding {
    pub id: String,
    pub check: String,
    /// The commit the finding was computed against. Recorded, never part of [`Self::id`].
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub opened_at: String,
    /// The finding's own words, verbatim.
    ///
    /// RFC-0020's constraint is that a proposal may assert only what the finding already
    /// asserts, and the old design honoured it by quoting `detail` into a commit body. A
    /// record carries it in a field instead, which is strictly more faithful: there is no
    /// framing sentence around it to be composition.
    pub detail: String,
    /// `open` while the check still reports it.
    #[serde(default = "open_standing")]
    pub standing: String,
}

fn open_standing() -> String {
    "open".to_string()
}

impl Finding {
    /// A finding as this tool opens one.
    pub fn open(check: &str, detail: &str, opened_at: &str) -> Self {
        Self {
            id: id_for(check, detail),
            check: check.to_string(),
            opened_at: opened_at.to_string(),
            detail: detail.trim().to_string(),
            standing: open_standing(),
        }
    }
}

/// The identity of a finding: a digest of the check and its words.
///
/// Twelve hex characters. Short enough to read in a diff, and 48 bits against a per-node
/// population in the low tens — a collision would have to be between two findings on one
/// node, which is the only place ids are compared.
pub fn id_for(check: &str, detail: &str) -> String {
    let mut h = Sha256::new();
    h.update(check.trim().as_bytes());
    h.update([0]);
    h.update(detail.trim().as_bytes());
    h.finalize()
        .iter()
        .take(6)
        .map(|b| format!("{b:02x}"))
        .collect()
}

#[derive(Default, serde::Deserialize)]
struct Container {
    #[serde(default)]
    findings: Vec<Finding>,
}

#[derive(Default, serde::Deserialize)]
struct Document {
    #[serde(default)]
    yidam: Option<Container>,
}

/// Where a document's YAML lives: the whole file, or the frontmatter inside it.
///
/// A node is YAML. A catalog entry is Markdown with a frontmatter block, and the record
/// belongs in the frontmatter for the same reason it belongs in the node: that is the part a
/// tool owns.
struct Region {
    /// Byte range of the YAML within `text`.
    start: usize,
    end: usize,
}

fn region(text: &str) -> Option<Region> {
    let trimmed_at = text.len() - text.trim_start_matches('\n').len();
    let body = &text[trimmed_at..];
    if let Some(rest) = body.strip_prefix("---\n") {
        // Frontmatter: between the opening fence and the closing one.
        let start = trimmed_at + 4;
        let close = rest.find("\n---")?;
        return Some(Region {
            start,
            end: start + close + 1,
        });
    }
    Some(Region {
        start: 0,
        end: text.len(),
    })
}

/// Every finding recorded on this document.
pub fn parse(text: &str) -> Vec<Finding> {
    let Some(r) = region(text) else {
        return Vec::new();
    };
    let doc: Document = serde_yaml::from_str(&text[r.start..r.end]).unwrap_or_default();
    doc.yidam.map(|c| c.findings).unwrap_or_default()
}

/// Render the block, at the indentation the container implies.
fn render(findings: &[Finding]) -> String {
    use std::fmt::Write as _;
    let mut out = format!("{CONTAINER}:\n  findings:\n");
    for f in findings {
        let _ = writeln!(out, "    - id: {}", f.id);
        let _ = writeln!(out, "      check: {}", f.check);
        if !f.opened_at.is_empty() {
            let _ = writeln!(out, "      opened_at: {}", f.opened_at);
        }
        let _ = writeln!(out, "      detail: {}", quote(&f.detail));
        let _ = writeln!(out, "      standing: {}", f.standing);
    }
    out
}

/// A YAML single-quoted scalar. The detail is the finding's own words and may hold anything.
fn quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

/// The line range of an existing top-level `yidam:` block within `yaml`, if any.
fn block_lines(yaml: &str) -> Option<(usize, usize)> {
    let lines: Vec<&str> = yaml.split('\n').collect();
    let header = format!("{CONTAINER}:");
    let start = lines.iter().position(|l| l.trim_end() == header)?;
    let mut end = start + 1;
    while end < lines.len() {
        let l = lines[end];
        // Anything at column zero that is not blank ends the block.
        if !l.trim().is_empty() && !l.starts_with([' ', '\t']) {
            break;
        }
        end += 1;
    }
    while end > start + 1 && lines[end - 1].trim().is_empty() {
        end -= 1;
    }
    Some((start, end))
}

/// Write `findings` onto `text`, replacing any block already there.
///
/// An empty list removes the block rather than writing `findings: []`, so a node that has no
/// outstanding questions looks like a node that never had one.
///
/// `None` when the document has no YAML region to write into — a catalog entry with no
/// closing frontmatter fence. That is a refusal rather than a failure: putting the record
/// somewhere a reader would not find it is worse than reporting that it could not be put.
pub fn write(text: &str, findings: &[Finding]) -> Option<String> {
    let r = region(text)?;
    let yaml = &text[r.start..r.end];
    let mut lines: Vec<String> = yaml.split('\n').map(str::to_string).collect();

    if let Some((start, end)) = block_lines(yaml) {
        lines.drain(start..end);
        if !findings.is_empty() {
            let block: Vec<String> = render(findings)
                .trim_end()
                .split('\n')
                .map(str::to_string)
                .collect();
            for (i, l) in block.into_iter().enumerate() {
                lines.insert(start + i, l);
            }
        }
    } else if !findings.is_empty() {
        // Append at the end of the mapping, after any trailing blank lines.
        while lines.last().is_some_and(|l| l.trim().is_empty()) {
            lines.pop();
        }
        lines.extend(render(findings).trim_end().split('\n').map(str::to_string));
        lines.push(String::new());
    }

    let rebuilt = lines.join("\n");
    Some(format!("{}{}{}", &text[..r.start], rebuilt, &text[r.end..]))
}

/// Add one finding, or leave the document alone when it already carries that id.
pub fn add(text: &str, f: &Finding) -> Option<String> {
    let mut all = parse(text);
    if all.iter().any(|x| x.id == f.id) {
        return None;
    }
    all.push(f.clone());
    write(text, &all)
}

/// Drop the finding with this id.
pub fn remove(text: &str, id: &str) -> Option<String> {
    let all = parse(text);
    let kept: Vec<Finding> = all.iter().filter(|f| f.id != id).cloned().collect();
    (kept.len() != all.len()).then(|| write(text, &kept))?
}

#[cfg(test)]
mod tests {
    use super::*;

    const NODE: &str = "class: gage\nlabel: Canyon\ndescription: |\n  It measures flow.\nproperties:\n  claim_tag: verified\nlinks:\n  - target: ../gage.ont.yml\n    relationship: instance-of\n";

    fn open(check: &str, detail: &str) -> Finding {
        Finding::open(check, detail, "4f2a1c9")
    }

    #[test]
    fn a_finding_round_trips_through_the_document() {
        let f = open("orphan-in", "nothing links to this node");
        let text = add(NODE, &f).expect("written");
        assert_eq!(parse(&text), vec![f]);
    }

    /// The whole point of the id: nothing else in the node is touched, so a diff a reviewer
    /// opens is the record and not the file.
    #[test]
    fn writing_a_record_leaves_every_other_byte_alone() {
        let text = add(NODE, &open("orphan-in", "nothing links here")).unwrap();
        assert!(text.starts_with(NODE.trim_end_matches('\n')), "{text}");
        assert!(text.contains("description: |\n  It measures flow."));
        assert!(text.contains("      check: orphan-in"));
    }

    /// A re-run at a later HEAD is additive, not a second copy of everything outstanding.
    #[test]
    fn the_same_finding_is_not_added_twice() {
        let text = add(NODE, &open("orphan-in", "nothing links here")).unwrap();
        assert!(add(&text, &open("orphan-in", "nothing links here")).is_none());
    }

    /// Opened at a later commit is the same question. Keying the id on the commit would
    /// reopen every question on every run.
    #[test]
    fn the_commit_is_recorded_and_is_not_part_of_the_identity() {
        let a = Finding::open("orphan-in", "d", "aaaaaaa");
        let b = Finding::open("orphan-in", "d", "bbbbbbb");
        assert_eq!(a.id, b.id);
        assert_ne!(a.opened_at, b.opened_at);
    }

    #[test]
    fn two_findings_on_one_node_have_different_ids() {
        assert_ne!(id_for("orphan-in", "d"), id_for("dangling-edge", "d"));
        assert_ne!(id_for("orphan-in", "one"), id_for("orphan-in", "two"));
    }

    /// The failure the marker could not survive.
    #[test]
    fn a_record_is_closable_after_the_nodes_prose_is_rewritten() {
        let f = open("orphan-in", "nothing links here");
        let text = add(NODE, &f).unwrap();
        let edited = text.replace("It measures flow.", "Rewritten entirely by a person.");
        let closed = remove(&edited, &f.id).expect("closable");
        assert!(parse(&closed).is_empty());
        assert!(closed.contains("Rewritten entirely by a person."));
    }

    /// A node with nothing outstanding looks like a node that never had anything.
    #[test]
    fn removing_the_last_finding_removes_the_block() {
        let f = open("orphan-in", "d");
        let text = add(NODE, &f).unwrap();
        let closed = remove(&text, &f.id).unwrap();
        assert!(!closed.contains("yidam:"), "{closed}");
        assert_eq!(closed.trim_end(), NODE.trim_end());
    }

    #[test]
    fn removing_one_of_two_keeps_the_other() {
        let a = open("orphan-in", "one");
        let b = open("dangling-edge", "two");
        let text = add(&add(NODE, &a).unwrap(), &b).unwrap();
        let closed = remove(&text, &a.id).unwrap();
        assert_eq!(parse(&closed), vec![b]);
    }

    #[test]
    fn removing_an_id_that_is_not_there_changes_nothing() {
        assert!(remove(NODE, "deadbeefcafe").is_none());
    }

    /// A catalog entry is markdown with frontmatter, and the record belongs inside it —
    /// not appended to the prose below, where nothing structured would find it.
    #[test]
    fn a_catalog_entry_takes_the_record_in_its_frontmatter() {
        let entry = "---\nname: usgs-nwis\ntype: api\n---\n\nWhat the source holds.\n";
        let f = open("catalog-expired", "the record is older than its ttl");
        let text = add(entry, &f).expect("written");
        assert_eq!(parse(&text), vec![f]);
        let (fm, body) = text.split_once("\n---\n").unwrap();
        assert!(
            fm.contains("yidam:"),
            "the record is in the frontmatter: {text}"
        );
        assert!(
            !body.contains("yidam:"),
            "and not in the prose below it: {text}"
        );
        assert!(body.contains("What the source holds."));
    }

    /// Putting the record where a reader would not find it is worse than saying it could
    /// not be put.
    #[test]
    fn an_entry_with_no_closing_fence_is_refused() {
        assert!(add("---\nname: half-written\n", &open("c", "d")).is_none());
    }

    /// The detail is the finding's own words and may hold anything YAML would read as
    /// structure.
    #[test]
    fn a_detail_carrying_quotes_and_colons_survives_verbatim() {
        let detail = "target does not exist: '../gone.yml' — it's gone";
        let f = open("dangling-edge", detail);
        let text = add(NODE, &f).unwrap();
        assert_eq!(parse(&text)[0].detail, detail);
    }

    /// The consequence the paragraph could not avoid, and the reason it matters most.
    ///
    /// The appended paragraph ended in `[open]`, so a question this tool carried entered the
    /// corpus's own open-question tally beside the questions a person raised — and no corpus
    /// could ask how many of its open questions were its own. A record carries the same
    /// finding and is counted as nothing.
    #[test]
    fn a_carried_finding_is_not_counted_as_a_claim_the_corpus_makes() {
        let before = crate::claims::count_in_node(NODE, &[]);
        let text = add(NODE, &open("orphan-in", "nothing links to this node")).unwrap();
        assert_eq!(
            crate::claims::count_in_node(&text, &[]),
            before,
            "carrying a question must not change what the corpus is counted as claiming"
        );
    }

    /// And it stays visible where RFC-0020 put it.
    ///
    /// The separation is between *what the corpus claims* and *what is outstanding here*,
    /// not between visible and hidden. Carrying a finding into the corpus and then hiding it
    /// would defeat the reason `propose` writes to the node at all.
    #[test]
    fn a_carried_finding_is_still_an_open_question_on_the_node() {
        let text = add(NODE, &open("orphan-in", "nothing links to this node")).unwrap();
        assert!(crate::claims::is_open_question("Canyon", &text, &[]));
        assert!(
            !crate::claims::is_open_question("Canyon", NODE, &[]),
            "and the node without one is not"
        );
    }

    /// And the finding is still legible as a question — by asking the record, which is the
    /// thing the prose form could not be asked.
    #[test]
    fn the_set_of_carried_questions_is_answerable() {
        let text = add(NODE, &open("orphan-in", "nothing links here")).unwrap();
        let text = add(&text, &open("dangling-edge", "target does not exist")).unwrap();
        let mut checks: Vec<String> = parse(&text).into_iter().map(|f| f.check).collect();
        checks.sort();
        assert_eq!(checks, ["dangling-edge", "orphan-in"]);
        assert!(parse(&text).iter().all(|f| f.opened_at == "4f2a1c9"));
    }

    /// A block written by an earlier run is replaced, not duplicated.
    #[test]
    fn a_second_finding_extends_the_existing_block() {
        let text = add(NODE, &open("orphan-in", "one")).unwrap();
        let text = add(&text, &open("dangling-edge", "two")).unwrap();
        assert_eq!(text.matches("yidam:").count(), 1, "{text}");
        assert_eq!(parse(&text).len(), 2);
    }
}
