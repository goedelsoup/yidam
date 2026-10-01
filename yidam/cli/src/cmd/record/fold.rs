//! `yidam record --fold` — the consumption record, said by the history (#1018).
//!
//! `[serve] record` appends to a gitignored file, which was the right hot-path decision and
//! not the end state: *the history is the graph*, and a corpus that can say it was read only
//! from a file every `git clean` removes cannot say it from its history. This folds the file
//! into a tracked one and commits it.
//!
//! # What may author it
//!
//! RFC-0026 settled it before this existed: a run authors **operational** commits directly. A
//! record of what was retrieved is operational — closest to `index:` in the vocabulary's own
//! table — so the commit is `refresh:`, it lands on the current branch, and it is written
//! through [`crate::cmd::operational`], which classifies its own subject and refuses anything
//! epistemic before a byte is written. No new authority concept.
//!
//! # What the commit contains: a tally, not the lines
//!
//! [`COMMITTED`] holds a [`Tally`] — every count the report draws, merged exactly — and not the
//! raw lines. The lines are the evidence and the tally is what anyone reads, and folding the
//! lines would grow the history by one line per tool call forever, which is the cost `.jsonl`
//! was chosen to avoid. A tally grows with distinct things (tools, nodes, commits, empty
//! questions) and not with traffic. It also carries less than the lines do: a returned node
//! keeps a count and loses the call it came back with, so what reaches the history cannot
//! group one caller's answers together (see `tally.rs`).
//!
//! # What happens to the file: nothing
//!
//! It is never truncated or rotated. A fold that read and then truncated would lose any line a
//! concurrent server appended between the two — two servers can be appending to one corpus,
//! and `Record` opens `O_APPEND` precisely so they may. So the fold keeps a **watermark**
//! instead: [`Folded::sources`] maps each file, by the digest of its first line, to how many
//! bytes of it are already counted. A fold reads the file once, counts the complete lines past
//! the watermark, and moves the watermark to the end of the last complete line it read. A line
//! appended after that read, or half-written during it, is past the new watermark and is the
//! next fold's. Nothing is lost, and nothing is counted twice.
//!
//! The first line is the file's identity because nothing else about it is stable. A `git
//! clean` and a fresh server start a new file with a new first line, and the old watermark —
//! still in the map — names a file that no longer exists, which is harmless. A path or an
//! inode would name the new file with the old file's offset.
//!
//! The file still grows on disk. Deleting it after a fold loses nothing, and is the operator's
//! act rather than this command's, because only the operator knows no server is writing to it.
//!
//! # What runs it
//!
//! Nothing schedules anything in this repository; `yidam run` and a cluster's steps are the
//! nearest things, and either can invoke this. It is safe on a clock for the reason every
//! operational writer here is: with nothing new to fold it writes nothing and commits nothing.
//! One fold at a time is the caller's to arrange, as one `serve` per working tree is.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

use super::tally::Tally;
use crate::cmd::operational::{Commit, Who, Writer};
use crate::cmd::serve::record::PATH;
use crate::report::Format;

/// The tracked file a fold writes, relative to the corpus root.
///
/// Beside `.yidam/record/` rather than in it, because that directory is gitignored by a rule
/// `serve` refuses to start without, and this file is the one that must be committed.
pub(crate) const COMMITTED: &str = ".yidam/consumption.json";

/// The author of a fold's commit. The tool performed the act; whoever ran it is the committer.
const WHO: Who<'static> = ("yidam record", "record@yidam");

/// What [`COMMITTED`] holds.
///
/// Every field defaults, so a file written by an older binary reads as far as its keys go —
/// the rule the record's own reader follows, for the same reason: the file outlives the binary.
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct Folded {
    /// Each record file this corpus has folded, by [`source_of`], and the bytes of it counted.
    pub sources: BTreeMap<String, u64>,
    /// Everything counted so far.
    pub tally: Tally,
}

impl Folded {
    /// What `text` holds past this fold's watermark for it — all of it, for a file this has
    /// never seen. A torn last line is included: the report counts it as unreadable, as it
    /// always has, and the fold does not take it.
    pub fn unfolded<'a>(&self, text: &'a str) -> &'a str {
        let from = source_of(text)
            .and_then(|s| self.sources.get(&s).copied())
            .unwrap_or(0);
        text.get(from as usize..).unwrap_or_default()
    }
}

/// The identity of a record file: the digest of its first complete line, or `None` before it
/// has one. See the module doc for why not a path.
pub(crate) fn source_of(text: &str) -> Option<String> {
    let end = text.find('\n')?;
    Some(format!(
        "sha256:{}",
        crate::deps::sha256_hex(&text.as_bytes()[..=end])
    ))
}

/// The committed fold, or `None` where there is none.
pub(crate) fn load(root: &Path) -> Result<Option<Folded>> {
    let path = root.join(COMMITTED);
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => bail!("reading {} ({e})", path.display()),
    };
    serde_json::from_str(&text).map(Some).with_context(|| {
        format!(
            "{} is not a fold this binary can read.\n  It is a tracked file, so `git log -p \
             -- {COMMITTED}` shows what wrote it.",
            path.display()
        )
    })
}

/// One fold's worth of new lines.
#[derive(Debug, PartialEq)]
struct Step {
    source: String,
    from: u64,
    to: u64,
    tally: Tally,
}

/// What a fold of `text` over `folded` would add, or `None` when it would add nothing.
///
/// Only complete lines: everything up to the last newline in what was read. The rest is a
/// line a server is writing now, and it is the next fold's.
fn plan(text: &str, folded: &Folded) -> Result<Option<Step>> {
    let Some(source) = source_of(text) else {
        return Ok(None);
    };
    let complete = text.rfind('\n').map_or(0, |i| i + 1);
    let from = folded.sources.get(&source).copied().unwrap_or(0);
    let Some(tail) = text.get(from as usize..complete) else {
        bail!(
            "{PATH} is shorter than the {from} bytes {COMMITTED} says were folded from it.\n  \
             The file was cut or rewritten after a fold, and counting from here would count \
             lines twice or not at all. Move it aside and let the server start a new one."
        );
    };
    let tally = Tally::of_text(tail);
    if tally.calls + tally.unreadable == 0 {
        return Ok(None);
    }
    Ok(Some(Step {
        source,
        from,
        to: complete as u64,
        tally,
    }))
}

/// The report, under one key, as every report is.
#[derive(Debug, Serialize)]
pub struct Report {
    pub fold: Outcome,
}

/// What one fold did.
#[derive(Debug, Default, Serialize)]
pub struct Outcome {
    /// The file folded from, always [`PATH`].
    pub record: &'static str,
    /// The tracked file folded into, always [`COMMITTED`].
    pub path: &'static str,
    /// Calls this fold counted. Zero when there was nothing new.
    pub calls: usize,
    /// Lines this fold counted as unreadable.
    pub unreadable: usize,
    /// The bytes of the file this fold counted, `[from, to)`. Equal when nothing was.
    pub from: u64,
    pub to: u64,
    /// The `refresh:` commit, or null when nothing was folded.
    pub commit: Option<Commit>,
    /// Calls counted in the history after this fold, all folds together.
    pub total: usize,
}

fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

/// The commit message for a step: the subject names the calls, the body says where from.
fn message(step: &Step) -> (String, String) {
    let subject = format!(
        "refresh: fold {} from the consumption record",
        plural(step.tally.calls, "call", "calls")
    );
    let mut body = format!(
        "Bytes {}..{} of {PATH} (first line {}), counted into {COMMITTED}.",
        step.from, step.to, step.source
    );
    if step.tally.unreadable > 0 {
        let _ = write!(
            body,
            "\n{} could not be read and {} counted as unreadable.",
            plural(step.tally.unreadable, "line", "lines"),
            if step.tally.unreadable == 1 {
                "is"
            } else {
                "are"
            }
        );
    }
    (subject, body)
}

/// Fold what the file holds past the watermark into [`COMMITTED`], and commit it.
pub(crate) fn run(root: &Path) -> Result<Outcome> {
    let mut out = Outcome {
        record: PATH,
        path: COMMITTED,
        ..Outcome::default()
    };
    crate::cmd::operational::require_clean(root, &[COMMITTED.to_string()])?;
    let mut folded = load(root)?.unwrap_or_default();
    out.total = folded.tally.calls;

    let path = root.join(PATH);
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(out),
        Err(e) => bail!("reading {} ({e})", path.display()),
    };
    let Some(step) = plan(&text, &folded)? else {
        if let Some(source) = source_of(&text) {
            let at = folded.sources.get(&source).copied().unwrap_or(0);
            (out.from, out.to) = (at, at);
        }
        return Ok(out);
    };

    folded.sources.insert(step.source.clone(), step.to);
    folded.tally.merge(&step.tally);
    let mut json = serde_json::to_string_pretty(&folded)?;
    json.push('\n');
    let (subject, body) = message(&step);
    let mut writer = Writer::WorkingTree;
    out.commit = writer.commit(root, WHO, &subject, &body, &[(COMMITTED.to_string(), json)])?;
    out.calls = step.tally.calls;
    out.unreadable = step.tally.unreadable;
    (out.from, out.to) = (step.from, step.to);
    out.total = folded.tally.calls;
    Ok(out)
}

pub(crate) fn render(o: &Outcome) -> String {
    match &o.commit {
        Some(c) => format!(
            "Folded {} from {} into {}: {} {}\n{} counted in the history now.\n",
            plural(o.calls, "call", "calls"),
            o.record,
            o.path,
            c.sha,
            c.subject,
            plural(o.total, "call is", "calls are"),
        ),
        None if o.to == 0 => format!(
            "Nothing to fold: {} is absent or holds no complete line. No commit was written.\n",
            o.record
        ),
        None => format!(
            "Nothing to fold: every complete line in {} is already in {}. No commit was \
             written.\n",
            o.record, o.path
        ),
    }
}

/// `yidam record --fold`.
pub fn fold(root: Option<&Path>, format: Format) -> Result<()> {
    let root = crate::paths::resolve_root(root)?;
    let report = Report { fold: run(&root)? };
    crate::report::finish(&root, format, report, |r| print!("{}", render(&r.fold)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(n: u64) -> String {
        format!(
            r#"{{"at":{n},"commit":"abc1234","tool":"retrieve","args_digest":"sha256:{n}","outcome":"ok","results":1,"node_ids":["concept/a"],"degraded":true,"rejected":false,"ms":{n}}}"#
        ) + "\n"
    }

    /// A line half-written while the fold reads is not taken, and the next fold takes it.
    #[test]
    fn a_torn_last_line_is_left_for_the_next_fold() {
        let whole = line(1) + &line(2);
        let torn = &whole[..whole.len() - 10];
        let step = plan(torn, &Folded::default()).unwrap().unwrap();
        assert_eq!(step.tally.calls, 1);
        assert_eq!(step.tally.unreadable, 0, "the torn line is not counted yet");
        assert_eq!(step.to as usize, line(1).len());

        let mut folded = Folded::default();
        folded.sources.insert(step.source.clone(), step.to);
        folded.tally.merge(&step.tally);
        let next = plan(&whole, &folded).unwrap().unwrap();
        assert_eq!(next.tally.calls, 1);
        assert_eq!(next.from, step.to);
    }

    /// The report over a fold and the file is the report over the file alone: the fold moved
    /// where a call is counted, not how often.
    #[test]
    fn folding_does_not_change_the_count() {
        let text = line(1) + &line(2) + &line(3);
        let step = plan(&text[..line(1).len() + 5], &Folded::default())
            .unwrap()
            .unwrap();
        let mut folded = Folded::default();
        folded.sources.insert(step.source, step.to);
        folded.tally.merge(&step.tally);
        assert_eq!(folded.unfolded(&text), line(2) + &line(3));

        let mut both = folded.tally.clone();
        both.merge(&Tally::of_text(folded.unfolded(&text)));
        assert_eq!(both, Tally::of_text(&text));
    }

    /// Nothing new is no step, and so no commit.
    #[test]
    fn a_folded_file_has_nothing_to_fold() {
        let text = line(1);
        let step = plan(&text, &Folded::default()).unwrap().unwrap();
        let mut folded = Folded::default();
        folded.sources.insert(step.source, step.to);
        assert_eq!(plan(&text, &folded).unwrap(), None);
        assert_eq!(plan("", &folded).unwrap(), None);
        assert_eq!(plan("{\"no newline yet", &folded).unwrap(), None);
    }

    /// A new file — a `git clean` and a fresh server — is a new source, counted from zero.
    #[test]
    fn a_new_file_is_a_new_source() {
        let old = line(1) + &line(2);
        let step = plan(&old, &Folded::default()).unwrap().unwrap();
        let mut folded = Folded::default();
        folded.sources.insert(step.source, step.to);

        let new = line(9);
        let step = plan(&new, &folded).unwrap().unwrap();
        assert_eq!(step.from, 0);
        assert_eq!(step.tally.calls, 1);
    }

    /// A file cut below its watermark is refused rather than counted from a guess.
    #[test]
    fn a_file_shorter_than_its_watermark_is_refused() {
        let text = line(1) + &line(2);
        let mut folded = Folded::default();
        folded
            .sources
            .insert(source_of(&text).unwrap(), (text.len() + 40) as u64);
        let err = plan(&text, &folded).unwrap_err();
        assert!(err.to_string().contains("is shorter than"), "{err}");
    }

    #[test]
    fn the_subject_names_the_calls_and_is_operational() {
        let text = line(1) + &line(2) + "torn\n";
        let step = plan(&text, &Folded::default()).unwrap().unwrap();
        let (subject, body) = message(&step);
        assert_eq!(subject, "refresh: fold 2 calls from the consumption record");
        assert!(crate::cmd::operational::is_operational(&subject));
        assert!(body.contains("1 line could not be read"), "{body}");
    }

    /// An older fold's file, missing keys, still reads.
    #[test]
    fn a_fold_missing_keys_still_reads() {
        let f: Folded = serde_json::from_str(r#"{"tally":{"calls":3}}"#).unwrap();
        assert_eq!(f.tally.calls, 3);
        assert!(f.sources.is_empty());
    }

    #[test]
    fn the_prose_says_whether_anything_was_committed() {
        let mut o = Outcome {
            record: PATH,
            path: COMMITTED,
            ..Outcome::default()
        };
        assert!(
            render(&o).contains("holds no complete line"),
            "{}",
            render(&o)
        );
        (o.from, o.to) = (90, 90);
        assert!(render(&o).contains("already in .yidam/consumption.json"));
        o.calls = 1;
        o.total = 4;
        o.commit = Some(Commit {
            sha: "abc1234".into(),
            subject: "refresh: fold 1 call from the consumption record".into(),
        });
        let text = render(&o);
        assert!(
            text.starts_with("Folded 1 call from .yidam/record/calls.jsonl into"),
            "{text}"
        );
        assert!(
            text.contains("4 calls are counted in the history now."),
            "{text}"
        );
    }
}
