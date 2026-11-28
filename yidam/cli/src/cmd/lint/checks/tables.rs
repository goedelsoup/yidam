use super::*;

// ── presentation ──────────────────────────────────────────────────────────────

/// Cells of a markdown table row, ignoring the leading and trailing pipes.
fn table_cells(line: &str) -> Vec<&str> {
    let t = line.trim();
    let t = t.strip_prefix('|').unwrap_or(t);
    let t = t.strip_suffix('|').unwrap_or(t);
    t.split('|').collect()
}

fn is_delimiter_row(line: &str) -> bool {
    let cells = table_cells(line);
    !cells.is_empty()
        && cells
            .iter()
            .all(|c| !c.trim().is_empty() && c.trim().chars().all(|ch| ch == '-' || ch == ':'))
}

pub const MALFORMED_REGEN_BLOCK: CheckSpec = CheckSpec {
    id: "malformed-regen-block",
    title: "REGEN block whose extent is not what it looks like",
    severity: Severity::Warn,
    rationale: "A REGEN block is three markers: the open tag, the `-->` that ends it, and \
         `<!-- /REGEN -->`. `yidam regen` finds all three or writes nothing at all, without \
         saying so — the section goes stale and looks like a generator that stopped running. \
         Where the missing tag is a close tag, the block also swallows whatever is below it, \
         so the *next* section stops updating for the same invisible reason.",
};

/// REGEN blocks that took lines which were not theirs.
///
/// `yidam regen` writes into a block by finding its open tag, its arrow and its close tag. If
/// any of the three is missing the write is a **silent no-op** — `update_regen` returns the
/// text unchanged — so the section simply stops updating, and the failure reads as "that part
/// never refreshes" rather than as a malformed file. Where a marker below it gets swallowed
/// as content, that marker's section stops updating too, and nothing anywhere says so.
///
/// The scan is `yidam_core::markers::scan_markers`, not a reader of this check's own. A
/// second scanner would be a second answer to "where does a block end", and the two would
/// drift without either going red — the argument `formal_specs.rs` makes about a workflow
/// that reproduces a task instead of running it. It is also this repository's first consumer
/// of a function that had three implementations, shared fixtures, and nothing calling it.
///
/// Warn rather than Error, and the reason is a limit of the scanner rather than a doubt about
/// the finding: it does not know about fenced code blocks, so a document explaining the marker
/// syntax with a deliberately unbalanced example is indistinguishable from a damaged file.
/// Gating on a check that cannot tell those apart is how `--warn-only` gets typed, which turns
/// every check off at once. Every markdown file in this repository passes it as written.
pub fn malformed_regen_block(files: &[(String, String)]) -> Check {
    use yidam_core::markers::Fault;
    let mut violations = Vec::new();
    for (rel, text) in files {
        for b in yidam_core::markers::scan_markers(text).malformed {
            let what = match b.fault {
                Fault::OpenArrowMissing => {
                    "its opening comment never closes: no line after it ends in `-->`, so \
                     everything below became part of the tag"
                }
                Fault::CloseTagMissing => {
                    "`<!-- /REGEN -->` never arrives, so the rest of the file became its body"
                }
                Fault::ClosedOnAnothersTag => {
                    "it closes on a `<!-- /REGEN -->` belonging to a block opened inside its \
                     own body — a close tag is missing between them"
                }
            };
            let lost = match b.swallowed_markers {
                0 => String::new(),
                n => format!(
                    ", {n} of which open{} a marker that is now content",
                    if n == 1 { "s" } else { "" }
                ),
            };
            violations.push(Violation::new(
                rel,
                format!(
                    "line {}: the `{}` block — {what}. It took the {} line(s) after it{lost}. \
                     `yidam regen` will not write to it.",
                    b.line, b.command, b.swallowed_lines
                ),
            ));
        }
    }
    Check::new(&MALFORMED_REGEN_BLOCK, violations)
}

pub const MALFORMED_TABLE: CheckSpec = CheckSpec {
    id: "malformed-table",
    title: "Markdown table whose rows disagree on width",
    severity: Severity::Warn,
    rationale: "A ragged table does not render as a table. Every reader of that file sees a \
         paragraph of pipes instead of the thing the author was trying to show, and the \
         author usually never looks at the rendered output again.",
};

/// Ragged markdown tables in any committed prose.
///
/// Applied to catalog entries and the corpus's own READMEs — anywhere a reader meets a
/// table. A table whose rows disagree on width does not render as a table; it renders as
/// a paragraph of pipes.
pub fn malformed_table(files: &[(String, String)]) -> Check {
    let mut violations = Vec::new();
    for (rel, text) in files {
        let lines: Vec<&str> = text.lines().collect();
        let mut i = 0;
        while i < lines.len() {
            // A table is a header row followed by a delimiter row.
            if !lines[i].trim_start().starts_with('|') || i + 1 >= lines.len() {
                i += 1;
                continue;
            }
            if !is_delimiter_row(lines[i + 1]) {
                i += 1;
                continue;
            }
            let width = table_cells(lines[i]).len();
            if table_cells(lines[i + 1]).len() != width {
                violations.push(Violation::new(
                    rel,
                    format!(
                        "line {}: header has {width} cells, delimiter has {} — renders as a \
                         paragraph of pipes",
                        i + 1,
                        table_cells(lines[i + 1]).len()
                    ),
                ));
            }
            let mut j = i + 2;
            let mut ragged = Vec::new();
            while j < lines.len() && lines[j].trim_start().starts_with('|') {
                if table_cells(lines[j]).len() != width {
                    ragged.push(j + 1);
                }
                j += 1;
            }
            if !ragged.is_empty() {
                violations.push(Violation::new(
                    rel,
                    format!(
                        "row(s) at line {} have a different width than the {width}-cell header",
                        ragged
                            .iter()
                            .map(|n| n.to_string())
                            .collect::<Vec<_>>()
                            .join(", ")
                    ),
                ));
            }
            i = j;
        }
    }
    Check::new(&MALFORMED_TABLE, violations)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_well_formed_regen_block_passes() {
        let text = "## Status\n\n<!-- REGEN: yidam status -->\n12 nodes\n<!-- /REGEN -->\n";
        assert!(malformed_regen_block(&[("f.md".into(), text.into())]).passed());
    }

    #[test]
    fn prose_with_no_markers_at_all_passes() {
        let text = "# A document\n\nNothing generated here.\n";
        assert!(malformed_regen_block(&[("f.md".into(), text.into())]).passed());
    }

    /// The shape a damaged file has in practice: two blocks, one missing close tag, and the
    /// scan reads it as a single long block that closed normally. The second section stops
    /// updating and nothing about the file looks wrong.
    #[test]
    fn a_block_that_swallows_the_next_one_is_reported_with_the_line_and_what_was_lost() {
        let text = "<!-- REGEN: yidam status -->\n12 nodes\n\n                    <!-- REGEN: yidam open-questions -->\n- q\n<!-- /REGEN -->\n";
        let c = malformed_regen_block(&[("README.md".into(), text.into())]);
        assert_eq!(c.violations.len(), 1, "{:#?}", c.violations);
        let d = &c.violations[0].detail;
        assert!(d.contains("line 1"), "{d}");
        assert!(d.contains("yidam status"), "{d}");
        assert!(d.contains("belonging to a block opened inside"), "{d}");
        assert!(
            d.contains("1 of which opens a marker that is now content"),
            "the finding must say a marker was lost, not just that lines were taken: {d}"
        );
        assert!(d.contains("will not write to it"), "{d}");
    }

    #[test]
    fn a_block_with_no_close_tag_at_the_end_of_a_file_is_reported() {
        let text = "## Status\n\n<!-- REGEN: yidam status -->\n12 nodes\n";
        let c = malformed_regen_block(&[("README.md".into(), text.into())]);
        assert_eq!(c.violations.len(), 1);
        assert!(
            c.violations[0].detail.contains("line 3"),
            "{:?}",
            c.violations[0].detail
        );
        assert!(
            c.violations[0].detail.contains("never arrives"),
            "{:?}",
            c.violations[0].detail
        );
    }

    #[test]
    fn an_open_tag_that_never_closes_is_reported_as_its_own_fault() {
        let text = "<!-- REGEN: yidam status\nFields: count.\n\nmore prose\n";
        let c = malformed_regen_block(&[("README.md".into(), text.into())]);
        assert_eq!(c.violations.len(), 1);
        assert!(
            c.violations[0]
                .detail
                .contains("opening comment never closes"),
            "{:?}",
            c.violations[0].detail
        );
    }

    /// The finding names the file it is in, so a report over a hundred of them is actionable.
    #[test]
    fn each_file_is_scanned_and_named() {
        let bad = "<!-- REGEN: a -->\nx\n";
        let good = "<!-- REGEN: b -->\ny\n<!-- /REGEN -->\n";
        let c = malformed_regen_block(&[
            ("good.md".into(), good.into()),
            ("bad.md".into(), bad.into()),
        ]);
        assert_eq!(c.violations.len(), 1);
        assert_eq!(c.violations[0].node, "bad.md");
    }

    #[test]
    fn a_well_formed_table_passes() {
        let text = "intro\n\n| A | B |\n|---|---|\n| 1 | 2 |\n| 3 | 4 |\n\nafter\n";
        assert!(malformed_table(&[("f.md".into(), text.into())]).passed());
    }

    #[test]
    fn a_ragged_body_row_is_flagged_with_its_line_number() {
        let text = "| A | B |\n|---|---|\n| 1 | 2 |\n| 3 |\n";
        let c = malformed_table(&[("f.md".into(), text.into())]);
        assert_eq!(c.violations.len(), 1);
        assert!(
            c.violations[0].detail.contains('4'),
            "should name line 4: {}",
            c.violations[0].detail
        );
    }

    #[test]
    fn a_header_and_delimiter_of_different_widths_are_flagged() {
        let text = "| A | B | C |\n|---|---|\n";
        let c = malformed_table(&[("f.md".into(), text.into())]);
        assert!(c.violations[0].detail.contains("paragraph of pipes"));
    }

    #[test]
    fn a_line_of_pipes_that_is_not_a_table_is_ignored() {
        // No delimiter row, so this is prose that happens to contain pipes.
        let text = "the grammar is `a | b | c` in this notation\n";
        assert!(malformed_table(&[("f.md".into(), text.into())]).passed());
    }
}
