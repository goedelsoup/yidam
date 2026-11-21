//! The note a re-run writes when it reproduced its committed receipt (RFC-0052).
//!
//! A step that runs because its input state moved, or because it aged, and that produces the
//! bytes its committed receipt already records, has changed nothing a reviewer reads. A commit
//! for it would carry a receipt that differs only in the head and the digests of what was read.
//! What lands instead is one line in a note on `refs/notes/yidam-runs`, attached to the object id
//! of the committed receipt blob:
//!
//! ```text
//! format_version 1
//! reproduced <input_state> <input_commit>
//! ```
//!
//! **Attached to the blob, not to a commit.** A later commit that leaves the receipt alone
//! leaves the same blob at the head, so the note is still found; the next run that changes an
//! output commits a new blob with no note, so nothing has to invalidate the old one.
//!
//! **Lines, not YAML,** so that two clones' copies of a note merge with
//! `git notes merge -s cat_sort_uniq`. `format_version` sorts ahead of `reproduced`, so a merged
//! note still opens with it.
//!
//! The date a note was last appended to is the committer date of the notes commit that wrote
//! it, which git records. No date is written into the note, for the receipt's "No clock" reason.

use std::path::Path;

use anyhow::{bail, Context, Result};

use crate::cmd::propose::write::{git, short_of};

/// The ref every note lives on.
pub(crate) const REF: &str = "refs/notes/yidam-runs";

/// The first line of every note this binary writes.
const HEADER: &str = "format_version 1";

/// Git's spelling of *must not exist*, for the compare-and-swap on a notes ref not yet written.
const ABSENT: &str = "0000000000000000000000000000000000000000";

/// A note that was written, as the run report names it.
#[derive(Debug, serde::Serialize)]
pub struct Noted {
    /// The notes ref that moved. Always `refs/notes/yidam-runs`.
    pub notes_ref: String,
    /// The committed receipt blob the note is attached to, abbreviated.
    pub receipt_blob: String,
    /// The notes commit written, abbreviated.
    pub commit: String,
}

/// The object id of the receipt committed at `at`, or `None` where there is none.
pub(crate) fn receipt_blob(root: &Path, at: &str, receipt_path: &str) -> Option<String> {
    git(
        root,
        None,
        &[
            "rev-parse",
            "--verify",
            "--quiet",
            &format!("{at}:{receipt_path}"),
        ],
        None,
    )
    .ok()
    .filter(|s| !s.is_empty())
}

/// The note's text on `blob`, or `None` where it has none.
fn show(root: &Path, blob: &str) -> Option<String> {
    git(
        root,
        None,
        &["notes", &format!("--ref={REF}"), "show", blob],
        None,
    )
    .ok()
}

/// The note's lines, where it is one this binary can read.
///
/// `Err` for a note whose first line is not [`HEADER`], which a later format wrote. A reader
/// treats that as no note — the conservative direction, which re-runs the step — and a writer
/// refuses to append to it rather than mixing two formats in one note.
fn parse(text: &str) -> std::result::Result<Vec<&str>, ()> {
    let mut lines = text.lines().filter(|l| !l.trim().is_empty());
    match lines.next() {
        Some(first) if first.trim() == HEADER => Ok(lines.collect()),
        _ => Err(()),
    }
}

/// Whether the note on `blob` lists `input_state` as one a re-run reproduced the receipt from.
pub(crate) fn lists(root: &Path, blob: &str, input_state: &str) -> bool {
    let Some(text) = show(root, blob) else {
        return false;
    };
    let Ok(lines) = parse(&text) else {
        return false;
    };
    lines.iter().any(|l| {
        let mut fields = l.split_whitespace();
        fields.next() == Some("reproduced") && fields.next() == Some(input_state)
    })
}

/// How many days ago a non-merge notes commit last wrote the note on `blob`.
///
/// Non-merge, because a `git notes merge` is dated when the merge ran, not when the run did; the
/// commits it merged arrive as its parents with their own dates. `--full-history`, because the
/// default simplification would follow one parent of that merge and drop the other side's runs.
///
/// Every fanout git may store the note under is named: a notes tree splits its paths into
/// two-character directories once it holds enough notes, and which split this tree uses is
/// git's business, not this reader's.
pub(crate) fn age(root: &Path, blob: &str, today: i64) -> Option<i64> {
    if blob.len() < 6 {
        return None;
    }
    let fanouts = [
        blob.to_string(),
        format!("{}/{}", &blob[..2], &blob[2..]),
        format!("{}/{}/{}", &blob[..2], &blob[2..4], &blob[4..]),
    ];
    let mut args = vec![
        "log",
        "-1",
        "--no-merges",
        "--full-history",
        "--date=short",
        "--format=%cd",
        REF,
        "--",
    ];
    args.extend(fanouts.iter().map(String::as_str));
    let out = git(root, None, &args, None).ok()?;
    Some(today - crate::dates::days_from_civil_str(out.trim())?)
}

/// Append a `reproduced` line to the note on `blob`, as one commit to [`REF`].
///
/// The notes commit is written on a temporary ref under `refs/notes/`, so that `git notes`
/// lays out the tree and its fanout, and then [`REF`] is moved to it with the same
/// compare-and-swap `commit` uses on the branch. A second run that appended in between is a
/// refusal, not a lost line.
///
/// The line is appended even where the note already lists this input state. An aged re-run is
/// what resets the clock, and the notes commit it writes is the date [`age`] reads.
pub(crate) fn append(
    root: &Path,
    blob: &str,
    input_state: &str,
    input_commit: &str,
    author: (&str, &str),
) -> Result<Noted> {
    let old = git(
        root,
        None,
        &[
            "rev-parse",
            "--verify",
            "--quiet",
            &format!("{REF}^{{commit}}"),
        ],
        None,
    )
    .ok()
    .filter(|s| !s.is_empty());

    let text = match show(root, blob) {
        Some(existing) => {
            if parse(&existing).is_err() {
                bail!(
                    "the note on {blob} in {REF} is not `{HEADER}`, so it was written by a later \
                     yidam; nothing was appended"
                );
            }
            existing.trim_end().to_string()
        }
        None => HEADER.to_string(),
    };
    let text = format!("{text}\nreproduced {input_state} {input_commit}\n");

    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or_default();
    let scratch = format!("{REF}-tmp-{}-{nanos}", std::process::id());
    let written = write_on(root, &scratch, old.as_deref(), blob, &text, author);
    // Removed whether or not the write succeeded, so a refusal leaves no ref behind.
    let _ = git(root, None, &["update-ref", "-d", &scratch], None);
    let new = written?;

    git(
        root,
        None,
        &["update-ref", REF, &new, old.as_deref().unwrap_or(ABSENT)],
        None,
    )
    .with_context(|| format!("{REF} moved while this run was writing to it; nothing was noted"))?;

    Ok(Noted {
        notes_ref: REF.to_string(),
        receipt_blob: short_of(root, blob),
        commit: short_of(root, &new),
    })
}

/// The notes commit, written on `scratch` on top of `old`. Returns its sha.
fn write_on(
    root: &Path,
    scratch: &str,
    old: Option<&str>,
    blob: &str,
    text: &str,
    author: (&str, &str),
) -> Result<String> {
    if let Some(old) = old {
        git(root, None, &["update-ref", scratch, old, ABSENT], None)?;
    }
    let notes_ref = format!("--ref={scratch}");
    crate::git::Git::new(root)
        .args(["notes", notes_ref.as_str(), "add", "-f", "-F", "-", blob])
        .env("GIT_AUTHOR_NAME", author.0)
        .env("GIT_AUTHOR_EMAIL", author.1)
        .stdin(text.as_bytes().to_vec())
        .run()?;
    git(root, None, &["rev-parse", "--verify", scratch], None)
}

/// Whether this clone holds a notes ref that no remote's fetch refspec reads (RFC-0052 §2.4).
///
/// `false` where there is no local ref, and `false` where there is no remote at all: a clone
/// with nothing to share with has nothing to warn about. A missing note leaves no trace, so this
/// is the one half of sharing a reader can see.
pub(crate) fn unshared(root: &Path) -> bool {
    let held = git(root, None, &["rev-parse", "--verify", "--quiet", REF], None)
        .is_ok_and(|s| !s.is_empty());
    let remotes = git(root, None, &["remote"], None).unwrap_or_default();
    if !held || remotes.trim().is_empty() {
        return false;
    }
    let specs = git(
        root,
        None,
        &["config", "--get-regexp", r"^remote\..*\.fetch$"],
        None,
    )
    .unwrap_or_default();
    !specs
        .lines()
        .filter_map(|l| l.split_once(' ').map(|(_, spec)| spec))
        .any(reads)
}

/// Whether a fetch refspec's source side matches [`REF`].
fn reads(spec: &str) -> bool {
    let spec = spec.trim().trim_start_matches('+');
    let src = spec.split_once(':').map_or(spec, |(src, _)| src);
    match src.strip_suffix('*') {
        Some(prefix) => REF.starts_with(prefix),
        None => src == REF,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_note_opens_with_its_format_version() {
        assert_eq!(
            parse("format_version 1\nreproduced a b\n"),
            Ok(vec!["reproduced a b"])
        );
        assert!(parse("format_version 2\nreproduced a b\n").is_err());
        assert!(parse("").is_err());
    }

    #[test]
    fn a_fetch_refspec_reads_the_notes_ref_by_name_or_by_glob() {
        assert!(reads(
            "refs/notes/yidam-runs:refs/notes-remote/origin/yidam-runs"
        ));
        assert!(reads("+refs/notes/*:refs/notes/*"));
        assert!(reads("refs/notes/yidam-runs"));
        assert!(!reads("+refs/heads/*:refs/remotes/origin/*"));
        assert!(!reads("refs/notes/commits:refs/notes/commits"));
    }

    /// `cat_sort_uniq` sorts a merged note's lines, so the header has to sort first.
    #[test]
    fn the_header_sorts_ahead_of_every_entry() {
        assert!(HEADER < "reproduced");
    }
}
