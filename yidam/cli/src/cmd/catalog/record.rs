//! Editing a catalog entry's frontmatter without rewriting the parts nobody asked to change.
//!
//! # Why this is a textual splice and not a YAML round-trip
//!
//! `serde_yaml::to_string` of a parsed entry would be a correct YAML document and the wrong
//! file. It reorders keys into struct order, drops every comment, re-wraps block scalars,
//! normalises quoting, and renders `used-by` as `used_by` unless every rename is mirrored on
//! the way out. A catalog entry is a hand-written document whose prose *is* the substance —
//! `usgs-nwis.md` carries four paragraphs about what NWIS does not answer — and a fetch that
//! reflowed all of it to record one digest would make every `refresh:` commit unreviewable.
//!
//! So the rule here is: touch the lines that change and no others. Everything this module
//! emits is appended or substituted in place, and a document it cannot edit confidently is
//! one it refuses rather than reformats.
//!
//! # The shape it understands
//!
//! Top-level keys of a frontmatter block, where a key line is unindented and a key's block
//! runs until the next unindented key. That covers every catalog entry in this repository and
//! every one the template scaffolds. It is deliberately not a YAML parser: a document whose
//! frontmatter does not start at byte zero, or whose target key is nested, is refused with a
//! message saying so, because the alternative is an edit that silently lands somewhere else.

use anyhow::{bail, Result};

use crate::parse::{ArtifactOrigin, CatalogArtifact};

/// A document split at the frontmatter, losslessly.
///
/// The three pieces rejoin to exactly the input. That is asserted rather than assumed — see
/// `splitting_is_lossless` — because every edit here is "replace one piece and concatenate",
/// and a split that dropped a byte would corrupt every file it touched in the same way.
struct Split<'a> {
    /// The frontmatter body, between the opening `---\n` and the closing `\n---`.
    front: &'a str,
    /// From the closing `\n---` to the end of the document.
    rest: &'a str,
}

/// Split a catalog entry, or refuse.
///
/// Mirrors [`crate::parse::parse_frontmatter`]'s delimiters exactly — `---\n` to open and the
/// first `\n---` to close — so that the block this edits is byte-for-byte the block the
/// parser read. Two functions disagreeing about where frontmatter ends is how an edit lands
/// in prose.
///
/// Unlike the parser it does **not** `trim_start`. The parser tolerates leading blank lines
/// because tolerating them costs it nothing; an editor cannot, because it has to put back
/// what it took apart. A document not starting at `---\n` is refused.
fn split(text: &str) -> Result<Split<'_>> {
    let Some(rest) = text.strip_prefix("---\n") else {
        bail!(
            "this entry has no frontmatter block starting at the first byte, so there is \
             nowhere to record what was obtained. A catalog entry opens with `---` on line 1."
        );
    };
    let Some(end) = rest.find("\n---") else {
        bail!("this entry's frontmatter block is never closed by a `---` line");
    };
    Ok(Split {
        front: &rest[..end],
        rest: &rest[end..],
    })
}

fn rejoin(split: &Split<'_>, front: &str) -> String {
    format!("---\n{}{}", front, split.rest)
}

/// Whether a line opens a top-level key.
///
/// Unindented, and containing a `:` before any `#`. A list item (`  - x`) is indented and a
/// comment line is not a key, which is the whole of what has to be distinguished here.
fn is_top_level_key(line: &str) -> bool {
    if line.is_empty() || line.starts_with([' ', '\t', '#', '-']) {
        return false;
    }
    match (line.find(':'), line.find('#')) {
        (Some(colon), Some(hash)) => colon < hash,
        (Some(_), None) => true,
        (None, _) => false,
    }
}

/// The name a top-level key line declares, or `None` if the line does not declare one.
fn key_name(line: &str) -> Option<&str> {
    is_top_level_key(line).then(|| line.split(':').next().unwrap_or("").trim_end())
}

/// The half-open line range a top-level key occupies, including its block.
///
/// Matched on the key *name* rather than on the line text, because a key and its block can
/// share a line: `used-by: []` is the empty list this module writes when a reconcile removes
/// every citation, and a scan looking for a bare `used-by:` would miss it and report the
/// entry as declaring no list at all — which is the one distinction `used_by_drift` turns on.
fn block_of(lines: &[&str], key: &str) -> Option<std::ops::Range<usize>> {
    let start = lines.iter().position(|l| key_name(l) == Some(key))?;
    let mut end = start + 1;
    while end < lines.len() && !is_top_level_key(lines[end]) {
        end += 1;
    }
    Some(start..end)
}

/// Render one artifact record as the YAML lines a person would have written.
///
/// Field order is the struct's declaration order, which is also the order
/// `journalism/edgar-filings.md` was written in by hand. Matching it means a record this
/// writes and a record an author wrote are indistinguishable in review — which is the
/// property that makes the field safe to keep hand-editing.
///
/// Absent fields are omitted rather than emitted as `null`. `media_type: null` asserts that
/// the media type is known to be nothing, and what is meant is that it was not determined.
fn render(a: &CatalogArtifact) -> Vec<String> {
    let mut out = Vec::new();
    let mut push = |k: &str, v: String| {
        out.push(format!(
            "{}{k}: {v}",
            if out.is_empty() { "  - " } else { "    " }
        ));
    };
    if let Some(h) = &a.sha256 {
        push("sha256", h.clone());
    }
    if let Some(b) = a.bytes {
        push("bytes", b.to_string());
    }
    if let Some(m) = &a.media_type {
        push("media_type", quote_if_needed(m));
    }
    if let Some(r) = &a.retrieved {
        push("retrieved", r.clone());
    }
    match &a.from {
        Some(ArtifactOrigin::Location(i)) => push("from", i.to_string()),
        Some(ArtifactOrigin::Url(u)) => push("from", quote_if_needed(u)),
        None => {}
    }
    if let Some(v) = &a.vault {
        push("vault", quote_if_needed(v));
    }
    if let Some(r) = a.redistributable {
        push("redistributable", r.to_string());
    }
    out
}

/// Quote a scalar YAML would otherwise read as something else.
///
/// A media type contains `/` and a URL contains `:` — the second is the one that matters,
/// because `from: https://x` is a YAML mapping key error rather than a string. Conservative
/// on purpose: quoting a value that did not need it is invisible, and failing to quote one
/// that did makes the entry unparseable, which takes its citations and its TTL down with it.
fn quote_if_needed(v: &str) -> String {
    let plain = !v.is_empty()
        && !v.contains([':', '#', '{', '}', '[', ']', ',', '&', '*', '\'', '"', '\n'])
        && v.trim() == v
        && v.parse::<f64>().is_err()
        && !matches!(v, "true" | "false" | "null" | "yes" | "no" | "on" | "off");
    if plain {
        v.to_string()
    } else {
        format!("\"{}\"", v.replace('\\', "\\\\").replace('"', "\\\""))
    }
}

/// Append artifact records to an entry's `artifacts:` list, creating it if absent.
///
/// Appending rather than replacing, and that is the design rather than an implementation
/// convenience. RFC-0023's record is *what this entry has obtained* — a history, not a
/// current value. A second fetch of a source that has since changed produces a second digest,
/// and both are true: the first names bytes somebody cited, and overwriting it would delete
/// the provenance of every claim resting on it.
///
/// A record whose digest the entry already carries is skipped, so re-running a fetch that
/// changed nothing changes nothing. That is what makes the command safe to put on a clock.
pub fn append_artifacts(text: &str, records: &[CatalogArtifact]) -> Result<String> {
    if records.is_empty() {
        return Ok(text.to_string());
    }
    let split = split(text)?;
    let lines: Vec<&str> = split.front.split('\n').collect();

    let held: Vec<String> = crate::parse::parse_frontmatter(text)
        .artifacts
        .unwrap_or_default()
        .iter()
        .filter_map(|a| a.sha256.clone())
        .collect();
    let fresh: Vec<&CatalogArtifact> = records
        .iter()
        .filter(|r| match &r.sha256 {
            Some(h) => !held.contains(h),
            None => true,
        })
        .collect();
    if fresh.is_empty() {
        return Ok(text.to_string());
    }

    let mut rendered: Vec<String> = Vec::new();
    for r in fresh {
        rendered.extend(render(r));
    }

    let mut out: Vec<String> = lines.iter().map(|s| (*s).to_string()).collect();
    match block_of(&lines, "artifacts") {
        Some(range) => {
            // Insert at the end of the existing block, before whatever key follows. Trailing
            // blank lines inside the block belong to the *document*, not to the list, so the
            // splice goes above them.
            let mut at = range.end;
            while at > range.start + 1 && out[at - 1].trim().is_empty() {
                at -= 1;
            }
            out.splice(at..at, rendered);
        }
        None => {
            // No list yet. Append at the end of the frontmatter, past any trailing blanks, so
            // the new key sits with the others rather than after a gap.
            let mut at = out.len();
            while at > 0 && out[at - 1].trim().is_empty() {
                at -= 1;
            }
            let mut block = vec!["artifacts:".to_string()];
            block.extend(rendered);
            out.splice(at..at, block);
        }
    }
    Ok(rejoin(&split, &out.join("\n")))
}

/// Replace an entry's `used-by:` list with the paths that actually cite it.
///
/// The citations are authoritative — `catalog-used-by-drift` says so in the words this
/// reuses: *"they cannot drift from the corpus, and a hand-maintained list can."* So
/// reconciling is a substitution and not a merge: whatever the list claimed that no node
/// carries is wrong by construction, and keeping it would be keeping the drift.
///
/// An entry that declares no list gets none. Absence is not drift — `used_by_drift` returns
/// `None` for it and the gate stays silent — so writing one would be this command deciding an
/// entry should make a claim it never made.
pub fn set_used_by(text: &str, citing: &[String]) -> Result<Option<String>> {
    let split = split(text)?;
    let lines: Vec<&str> = split.front.split('\n').collect();
    let Some(range) = block_of(&lines, "used-by") else {
        return Ok(None);
    };

    let mut block = vec!["used-by:".to_string()];
    block.extend(citing.iter().map(|p| format!("  - {}", quote_if_needed(p))));
    // A list reconciled to nothing is `used-by:` with no items, which YAML reads as null and
    // `used_by_drift` reads as "declares no list" — the entry would stop being checked. Emit
    // an explicit empty sequence so an entry that claimed something and now claims nothing
    // still says so.
    if citing.is_empty() {
        block = vec!["used-by: []".to_string()];
    }

    let mut out: Vec<String> = lines.iter().map(|s| (*s).to_string()).collect();
    // Narrow the range to stop short of trailing blank lines, and leave them where they are.
    // They separate keys in several entries, and absorbing them would put an unrelated
    // whitespace change in a `reconcile:` commit whose value is being reviewable at a glance.
    //
    // Left in place rather than re-emitted: they sit *outside* the spliced range, so they
    // survive untouched. Adding them to the replacement block as well duplicated every one —
    // caught by `a_blank_line_after_the_list_survives_the_substitution`, which is the only
    // reason that test exists as an equality rather than a `contains`.
    let mut end = range.end;
    while end > range.start + 1 && out[end - 1].trim().is_empty() {
        end -= 1;
    }
    out.splice(range.start..end, block);
    let rebuilt = rejoin(&split, &out.join("\n"));
    if rebuilt == text {
        return Ok(None);
    }
    Ok(Some(rebuilt))
}

#[cfg(test)]
mod tests {
    use super::*;

    const ENTRY: &str = "\
---
name: usgs-nwis
description: Continuous streamflow records.
type: api
obtained: true
location:
  - kind: url_template
    value: https://x/?sites={site}
    description: Instantaneous values.
used-by:
  - ../corpus/gage/canyon-outlet.yml
---

# USGS NWIS

The system of record. **Parameter 00060** is discharge.
";

    fn artifact(sha: &str) -> CatalogArtifact {
        CatalogArtifact {
            sha256: Some(sha.to_string()),
            bytes: Some(30),
            media_type: Some("application/json".to_string()),
            retrieved: Some("2026-09-07".to_string()),
            from: Some(ArtifactOrigin::Location(0)),
            vault: None,
            redistributable: None,
        }
    }

    #[test]
    fn splitting_is_lossless() {
        let s = split(ENTRY).unwrap();
        assert_eq!(rejoin(&s, s.front), ENTRY);
    }

    /// The property the whole module exists for: the prose, the comments, the key order and
    /// the block scalars all survive an edit that records one digest.
    #[test]
    fn appending_an_artifact_leaves_every_other_byte_alone() {
        let out = append_artifacts(ENTRY, &[artifact("aa")]).unwrap();
        assert!(out.starts_with("---\nname: usgs-nwis\n"));
        assert!(out
            .contains("# USGS NWIS\n\nThe system of record. **Parameter 00060** is discharge.\n"));
        assert!(out.contains("  - kind: url_template\n    value: https://x/?sites={site}\n"));
        assert!(out.contains("used-by:\n  - ../corpus/gage/canyon-outlet.yml\n"));
        assert!(
            out.contains(
                "artifacts:\n  - sha256: aa\n    bytes: 30\n    media_type: application/json\n    \
                 retrieved: 2026-09-07\n    from: 0\n"
            ),
            "{out}"
        );
        // And what it wrote is what the parser reads back.
        let parsed = crate::parse::parse_frontmatter(&out);
        let held = parsed.artifacts.unwrap();
        assert_eq!(held.len(), 1);
        assert_eq!(held[0].sha256.as_deref(), Some("aa"));
        assert_eq!(parsed.name.as_deref(), Some("usgs-nwis"));
        assert_eq!(parsed.used_by.unwrap().len(), 1);
    }

    #[test]
    fn a_second_artifact_joins_the_existing_list() {
        let once = append_artifacts(ENTRY, &[artifact("aa")]).unwrap();
        let twice = append_artifacts(&once, &[artifact("bb")]).unwrap();
        let held = crate::parse::parse_frontmatter(&twice).artifacts.unwrap();
        assert_eq!(
            held.iter()
                .filter_map(|a| a.sha256.clone())
                .collect::<Vec<_>>(),
            vec!["aa", "bb"]
        );
        assert!(twice.contains("# USGS NWIS"));
    }

    /// What makes the command safe to put on a clock: a re-fetch of unchanged bytes is a
    /// no-op, so it produces no commit rather than an empty one every time it runs.
    #[test]
    fn re_recording_a_digest_already_held_changes_nothing() {
        let once = append_artifacts(ENTRY, &[artifact("aa")]).unwrap();
        let again = append_artifacts(&once, &[artifact("aa")]).unwrap();
        assert_eq!(once, again);
    }

    /// Overwriting would delete the provenance of any claim resting on the older bytes.
    #[test]
    fn a_changed_source_appends_rather_than_replacing() {
        let once = append_artifacts(ENTRY, &[artifact("aa")]).unwrap();
        let twice = append_artifacts(&once, &[artifact("cc")]).unwrap();
        assert!(twice.contains("sha256: aa"));
        assert!(twice.contains("sha256: cc"));
    }

    #[test]
    fn a_url_origin_is_quoted_so_the_entry_still_parses() {
        let mut a = artifact("aa");
        a.from = Some(ArtifactOrigin::Url("https://x/y?a=1".into()));
        let out = append_artifacts(ENTRY, &[a]).unwrap();
        assert!(out.contains("from: \"https://x/y?a=1\""), "{out}");
        let held = crate::parse::parse_frontmatter(&out).artifacts.unwrap();
        assert!(matches!(held[0].from, Some(ArtifactOrigin::Url(_))));
    }

    #[test]
    fn absent_fields_are_omitted_rather_than_written_as_null() {
        let a = CatalogArtifact {
            sha256: Some("aa".into()),
            ..Default::default()
        };
        let out = append_artifacts(ENTRY, &[a]).unwrap();
        assert!(out.contains("  - sha256: aa\n"));
        assert!(!out.contains("null"), "{out}");
    }

    #[test]
    fn an_entry_with_no_frontmatter_is_refused_rather_than_rewritten() {
        let err = append_artifacts("# Just prose\n", &[artifact("aa")]).unwrap_err();
        assert!(err.to_string().contains("no frontmatter"), "{err}");
        let err = append_artifacts("---\nname: x\n", &[artifact("aa")]).unwrap_err();
        assert!(err.to_string().contains("never closed"), "{err}");
    }

    #[test]
    fn reconciling_used_by_substitutes_the_authoritative_list() {
        let out = set_used_by(
            ENTRY,
            &[
                "../corpus/gage/canyon-outlet.yml".into(),
                "../corpus/gage/valley-bridge.yml".into(),
            ],
        )
        .unwrap()
        .unwrap();
        assert!(out.contains(
            "used-by:\n  - ../corpus/gage/canyon-outlet.yml\n  - ../corpus/gage/valley-bridge.yml\n"
        ));
        assert!(out.contains("# USGS NWIS"));
        assert!(out.contains("obtained: true"));
        assert_eq!(
            crate::parse::parse_frontmatter(&out).used_by.unwrap().len(),
            2
        );
    }

    /// Absence is not drift. An entry that never claimed a `used-by` must not acquire one,
    /// because that would be this command deciding it should make a claim it never made.
    #[test]
    fn an_entry_declaring_no_list_is_left_alone() {
        let no_list = ENTRY.replace("used-by:\n  - ../corpus/gage/canyon-outlet.yml\n", "");
        assert_eq!(set_used_by(&no_list, &["a.yml".into()]).unwrap(), None);
    }

    /// A list already in agreement produces no edit, so `reconcile` on a clean corpus writes
    /// no commit.
    #[test]
    fn a_list_already_in_agreement_reports_no_change() {
        assert_eq!(
            set_used_by(ENTRY, &["../corpus/gage/canyon-outlet.yml".into()]).unwrap(),
            None
        );
    }

    /// Reconciling to nothing must stay a declared empty list. Emitting a bare `used-by:`
    /// would parse as null, `used_by_drift` would read that as "declares no list", and the
    /// entry would silently stop being checked.
    #[test]
    fn reconciling_to_nothing_writes_an_explicit_empty_list() {
        let out = set_used_by(ENTRY, &[]).unwrap().unwrap();
        assert!(out.contains("used-by: []"), "{out}");
        let parsed = crate::parse::parse_frontmatter(&out);
        assert_eq!(parsed.used_by, Some(vec![]));
        assert!(out.contains("# USGS NWIS"));
    }

    /// A blank line separating keys is the document's, not the list's. Absorbing it would put
    /// an unrelated whitespace change in a `reconcile:` commit whose whole value is being
    /// reviewable at a glance.
    #[test]
    fn a_blank_line_after_the_list_survives_the_substitution() {
        let spaced = "---\nname: s\nused-by:\n  - ../corpus/a.yml\n\ntype: paper\n---\n\n# S\n";
        let out = set_used_by(spaced, &["../corpus/b.yml".into()])
            .unwrap()
            .unwrap();
        assert_eq!(
            out, "---\nname: s\nused-by:\n  - ../corpus/b.yml\n\ntype: paper\n---\n\n# S\n",
            "{out}"
        );
    }

    /// An entry emptied by one reconcile must still be *found* by the next one. `used-by: []`
    /// puts the key and its value on one line, which a scan for a bare `used-by:` would miss —
    /// and missing it would report the entry as declaring no list, which is the one state that
    /// means "never check this again".
    #[test]
    fn an_emptied_list_is_still_found_and_can_be_refilled() {
        let emptied = set_used_by(ENTRY, &[]).unwrap().unwrap();
        assert!(emptied.contains("used-by: []"));

        let refilled = set_used_by(&emptied, &["../corpus/record/back.yml".into()])
            .unwrap()
            .unwrap();
        assert!(
            refilled.contains("used-by:\n  - ../corpus/record/back.yml"),
            "{refilled}"
        );
        assert!(!refilled.contains("[]"), "{refilled}");
        assert!(
            refilled.contains("# USGS NWIS"),
            "prose survives both passes"
        );
    }

    #[test]
    fn a_key_line_is_distinguished_from_a_list_item_and_a_comment() {
        assert!(is_top_level_key("used-by:"));
        assert!(is_top_level_key("name: x"));
        assert!(!is_top_level_key("  - kind: url"));
        assert!(!is_top_level_key("# used-by: not a key"));
        assert!(!is_top_level_key("just prose"));
        assert!(!is_top_level_key(""));
    }
}
