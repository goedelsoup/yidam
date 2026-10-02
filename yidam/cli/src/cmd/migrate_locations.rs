//! `yidam migrate locations` — `kind: doi` and `kind: pmc` become `kind: identifier` (#1314).
//!
//! Corpora that read papers wrote a location per identifier scheme, `kind: doi` beside
//! `kind: pmc`, and `catalog-location-malformed` refused both: the kind set was closed and had
//! no word for an identifier. RFC-0048 §2 gave it one, `identifier`, whose value is
//! `scheme:local-id`. This moves what was written onto it:
//!
//! ```yaml
//! - kind: doi                      - kind: identifier
//!   value: 10.1167/tvst.8.5.14  →    value: doi:10.1167/tvst.8.5.14
//! ```
//!
//! # What it will not guess
//!
//! The value moves under the scheme unchanged. A value already carrying its scheme keeps it
//! rather than gaining a second. **A value that is a URL is left alone and reported**:
//! `https://doi.org/10.1/x` holds a DOI, but taking a link apart to find one is a reading of it,
//! and the author can make that reading in one edit. So is a location written in a form this
//! does not rewrite, such as a flow mapping.
//!
//! Every rewrite is two line edits, as every ontology migration's is, and each file's edits are
//! checked before anything is written: the frontmatter is parsed with the edits applied, and
//! every location they touched must read back as `identifier` with the value it was meant to
//! have. A file that does not is reported and left as it was.

use std::path::Path;

use super::migrate::{raw_scalar_on, rel, unquoted, MigrateReport};
use super::rename::{Edit, Unhandled};
use crate::parse::{parse_frontmatter, CATALOG_LOCATION_KINDS_RETIRED};
use crate::paths::yidam_catalog_dir;
use crate::walk::walk_md_files;

/// The kind every retired one becomes.
const IDENTIFIER: &str = "identifier";

/// Plan the rewrite of every catalog entry.
pub(crate) fn plan(root: &Path, report: &mut MigrateReport) {
    for path in walk_md_files(&yidam_catalog_dir(root)) {
        // README.md is `catalog-audit`'s REGEN target, not a source.
        if path.file_name().is_some_and(|n| n == "README.md") {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let (edits, skipped) = plan_entry(&rel(root, &path), &text);
        report.edits.extend(edits);
        report.unhandled.extend(skipped);
    }
}

/// How many locations a plan rewrites: one `kind` edit each.
pub(crate) fn rewritten(report: &MigrateReport) -> usize {
    report.edits.iter().filter(|e| e.to == IDENTIFIER).count()
}

fn indent(line: &str) -> usize {
    line.len() - line.trim_start().len()
}

/// The column a `key:` on this line starts at, past any `- `.
fn key_column(line: &str) -> usize {
    indent(line)
        + if line.trim_start().starts_with("- ") {
            2
        } else {
            0
        }
}

/// One entry's edits, and the locations it reports instead of rewriting.
fn plan_entry(file: &str, text: &str) -> (Vec<Edit>, Vec<Unhandled>) {
    let lines: Vec<&str> = text.lines().collect();
    let mut edits = Vec::new();
    let mut skipped = Vec::new();
    if lines.first().map(|l| l.trim_end()) != Some("---") {
        return (edits, skipped);
    }
    let Some(close) = lines
        .iter()
        .skip(1)
        .position(|l| l.trim_end() == "---")
        .map(|i| i + 1)
    else {
        return (edits, skipped);
    };
    let skip = |line: usize, why: String| Unhandled {
        file: file.to_string(),
        line: line + 1,
        text: why,
    };

    let mut found = 0;
    for i in 1..close {
        let Some((_, _, kind_raw)) = raw_scalar_on(lines[i], "kind") else {
            continue;
        };
        let kind = unquoted(&kind_raw).unwrap_or(&kind_raw).to_string();
        if !CATALOG_LOCATION_KINDS_RETIRED.contains(&kind.as_str()) {
            continue;
        }
        found += 1;
        // The list item this `kind:` belongs to: from its `- ` line to the first line indented
        // less than its keys. Its `value:` is the one at the same column.
        let col = key_column(lines[i]);
        let item_start = if lines[i].trim_start().starts_with("- ") {
            Some(i)
        } else {
            (1..i)
                .rev()
                .find(|&j| lines[j].trim_start().starts_with("- ") && key_column(lines[j]) == col)
        };
        let Some(item_start) = item_start else {
            skipped.push(skip(i, format!("kind `{kind}` outside a list item")));
            continue;
        };
        let item_end = (item_start + 1..close)
            .find(|&j| !lines[j].trim().is_empty() && indent(lines[j]) < col)
            .unwrap_or(close);
        let value = (item_start..item_end).find_map(|j| {
            (key_column(lines[j]) == col)
                .then(|| raw_scalar_on(lines[j], "value"))
                .flatten()
                .map(|(_, _, v)| (j, v))
        });
        let Some((j, value_raw)) = value else {
            skipped.push(skip(i, format!("kind `{kind}` with no `value` to move")));
            continue;
        };
        let inner = unquoted(&value_raw).unwrap_or(&value_raw);
        if matches!(value_raw.chars().next(), Some('|' | '>')) {
            skipped.push(skip(
                j,
                format!("kind `{kind}` with a block-scalar `value`"),
            ));
            continue;
        }
        if inner.starts_with("http://") || inner.starts_with("https://") {
            skipped.push(skip(
                j,
                format!(
                    "kind `{kind}` but the value is a URL — write it as `kind: url`, or as \
                         `kind: identifier` with `{kind}:` and the id the link holds"
                ),
            ));
            continue;
        }
        let prefix = format!("{kind}:");
        let scoped = if inner.to_ascii_lowercase().starts_with(&prefix) {
            inner.to_string()
        } else {
            format!("{prefix}{inner}")
        };
        let to = match value_raw.chars().next() {
            Some(q @ ('"' | '\'')) if unquoted(&value_raw).is_some() => format!("{q}{scoped}{q}"),
            _ => scoped,
        };
        edits.push(Edit {
            file: file.to_string(),
            line: i + 1,
            from: kind_raw.clone(),
            to: IDENTIFIER.to_string(),
        });
        if to != value_raw {
            edits.push(Edit {
                file: file.to_string(),
                line: j + 1,
                from: value_raw.clone(),
                to,
            });
        }
    }

    // A retired kind the line scan did not see — a flow mapping, say — is reported, not lost.
    let before = parse_frontmatter(text).location.unwrap_or_default();
    let written = before
        .iter()
        .filter(|l| {
            l.kind
                .as_deref()
                .is_some_and(|k| CATALOG_LOCATION_KINDS_RETIRED.contains(&k))
        })
        .count();
    if written > found {
        skipped.push(skip(
            0,
            format!(
                "{} `doi`/`pmc` location(s) written in a form this does not rewrite",
                written - found
            ),
        ));
    }

    if !edits.is_empty() && !reads_back(text, &edits) {
        let n = edits.iter().filter(|e| e.to == IDENTIFIER).count();
        skipped.push(skip(
            0,
            format!("{n} location(s) whose rewrite does not read back as written; left as it was"),
        ));
        edits.clear();
    }
    (edits, skipped)
}

/// Whether `text` with `edits` applied parses to what they meant: every location they touch is
/// `identifier`, with the value its old kind prefixed, and no other location has changed.
fn reads_back(text: &str, edits: &[Edit]) -> bool {
    let mut lines: Vec<String> = text.lines().map(str::to_string).collect();
    for e in edits {
        let Some(line) = lines.get_mut(e.line - 1) else {
            return false;
        };
        let key = if e.to == IDENTIFIER { "kind" } else { "value" };
        let Some((start, end, _)) = raw_scalar_on(line, key).filter(|(_, _, v)| *v == e.from)
        else {
            return false;
        };
        line.replace_range(start..end, &e.to);
    }
    let after = parse_frontmatter(&lines.join("\n"))
        .location
        .unwrap_or_default();
    let before = parse_frontmatter(text).location.unwrap_or_default();
    if before.len() != after.len() {
        return false;
    }
    let mut rewritten = 0;
    for (b, a) in before.iter().zip(&after) {
        let kind = b.kind.as_deref().unwrap_or("");
        let (bv, av) = (
            b.value.as_deref().unwrap_or(""),
            a.value.as_deref().unwrap_or(""),
        );
        if a.kind.as_deref() == Some(IDENTIFIER) && kind != IDENTIFIER {
            let prefix = format!("{kind}:");
            let meant = if bv.to_ascii_lowercase().starts_with(&prefix) {
                bv.to_string()
            } else {
                format!("{prefix}{bv}")
            };
            if !CATALOG_LOCATION_KINDS_RETIRED.contains(&kind) || av != meant {
                return false;
            }
            rewritten += 1;
        } else if b.kind != a.kind || bv != av || b.description != a.description {
            return false;
        }
    }
    rewritten == edits.iter().filter(|e| e.to == IDENTIFIER).count()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(location: &str) -> String {
        format!("---\nname: x\ntype: paper\nlocation:\n{location}---\n\nbody\n")
    }

    fn rewrite(text: &str) -> (String, Vec<Unhandled>) {
        let (edits, skipped) = plan_entry("e.md", text);
        let mut lines: Vec<String> = text.lines().map(str::to_string).collect();
        for e in &edits {
            let line = &mut lines[e.line - 1];
            let key = if e.to == IDENTIFIER { "kind" } else { "value" };
            let (s, t, _) = raw_scalar_on(line, key).unwrap();
            line.replace_range(s..t, &e.to);
        }
        (lines.join("\n") + "\n", skipped)
    }

    /// The RFC's own example, both schemes.
    #[test]
    fn a_doi_and_a_pmc_become_identifiers() {
        let text = entry(
            "  - kind: doi\n    value: 10.1167/tvst.8.5.14\n    description: publisher\n  \
             - kind: pmc\n    value: PMC6753881\n    description: full text\n",
        );
        let (out, skipped) = rewrite(&text);
        assert!(skipped.is_empty(), "{:?}", skipped.len());
        assert!(
            out.contains("  - kind: identifier\n    value: doi:10.1167/tvst.8.5.14\n"),
            "{out}"
        );
        assert!(
            out.contains("  - kind: identifier\n    value: pmc:PMC6753881\n"),
            "{out}"
        );
        // Nothing else on the page moved.
        assert!(out.contains("    description: publisher\n"));
        assert!(out.ends_with("---\n\nbody\n"));
    }

    #[test]
    fn value_first_quoted_and_already_scoped_are_each_kept_as_written() {
        let text = entry(
            "  - value: \"10.1/a\"\n    kind: doi\n    description: a\n  \
             - kind: 'pmc'\n    value: pmc:PMC1\n    description: b\n",
        );
        let (out, skipped) = rewrite(&text);
        assert!(skipped.is_empty());
        assert!(
            out.contains("  - value: \"doi:10.1/a\"\n    kind: identifier\n"),
            "{out}"
        );
        assert!(
            out.contains("  - kind: identifier\n    value: pmc:PMC1\n"),
            "{out}"
        );
    }

    /// A link holds an identifier, and taking it apart is a reading this will not make.
    #[test]
    fn a_url_value_is_reported_and_not_rewritten() {
        let text = entry("  - kind: doi\n    value: https://doi.org/10.1/a\n");
        let (edits, skipped) = plan_entry("e.md", &text);
        assert!(edits.is_empty());
        assert_eq!(skipped.len(), 1);
        assert!(skipped[0].text.contains("URL"), "{}", skipped[0].text);
    }

    #[test]
    fn a_flow_mapping_is_reported_and_not_lost() {
        let text = entry("  - {kind: doi, value: 10.1/a}\n");
        let (edits, skipped) = plan_entry("e.md", &text);
        assert!(edits.is_empty());
        assert_eq!(skipped.len(), 1);
        assert!(skipped[0].text.contains("does not rewrite"));
    }

    #[test]
    fn every_other_kind_and_the_body_are_left_alone() {
        let text = "---\nname: x\nlocation:\n  - kind: url\n    value: https://x.org\n---\n\n\
                    kind: doi\nvalue: 10.1/a\n";
        let (edits, skipped) = plan_entry("e.md", text);
        assert!(edits.is_empty() && skipped.is_empty());
    }
}
