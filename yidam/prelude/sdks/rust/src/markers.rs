#[derive(Debug, Clone, PartialEq)]
pub enum Marker {
    Template { instruction: String },
    Regen { command: String, content: String },
}

impl Marker {
    pub fn kind_str(&self) -> &'static str {
        match self {
            Marker::Template { .. } => "Template",
            Marker::Regen { .. } => "Regen",
        }
    }
}

/// What is wrong with a REGEN block the scan crossed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fault {
    /// The open tag ran onto further lines and its `-->` never arrived. Everything after it
    /// was consumed looking for one.
    OpenArrowMissing,
    /// `<!-- /REGEN -->` never arrived, so the rest of the input became this block's content.
    CloseTagMissing,
    /// The block closed — on a tag that belongs to a block opened inside its own body. A
    /// close tag is missing above, and this is the shape that shows up in a real file:
    /// `CloseTagMissing` needs the damaged block to be the last one in the document, and it
    /// usually is not.
    ClosedOnAnothersTag,
}

impl Fault {
    pub fn as_str(&self) -> &'static str {
        match self {
            Fault::OpenArrowMissing => "OpenArrowMissing",
            Fault::CloseTagMissing => "CloseTagMissing",
            Fault::ClosedOnAnothersTag => "ClosedOnAnothersTag",
        }
    }
}

/// A REGEN block whose extent the scan could not read the way it was meant.
///
/// In every case the block has taken lines that were not its content, and every marker among
/// them is a marker the caller never sees — which is what `swallowed_markers` counts and what
/// makes this worth reporting rather than leaving to be inferred from markers that are absent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MalformedBlock {
    /// The command on the open tag, as `parse_markers` read it.
    pub command: String,
    /// 1-indexed line the open tag sits on.
    pub line: usize,
    pub fault: Fault,
    /// Lines after the open tag that this block took as its own.
    pub swallowed_lines: usize,
    /// How many of those lines open a marker — markers that are now content.
    pub swallowed_markers: usize,
}

/// What one pass over the text found: the markers, and the blocks that are malformed.
///
/// `PartialEq` and not `Eq`, because `Marker` is `PartialEq` only. Widening `Marker` to `Eq`
/// for the sake of a derive here would be a change to a published type for the convenience of
/// a new one.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Scan {
    pub markers: Vec<Marker>,
    pub malformed: Vec<MalformedBlock>,
}

/// Whether a line opens a REGEN block. A body containing one means a close tag is missing
/// above it, which is what separates `ClosedOnAnothersTag` from a block that is merely long.
fn opens_a_regen(line: &str) -> bool {
    line.trim().starts_with("<!-- REGEN:")
}

/// Whether a line opens a marker of either kind.
fn opens_a_marker(line: &str) -> bool {
    let t = line.trim();
    t.starts_with("<!-- REGEN:") || t.starts_with("<!-- TEMPLATE:")
}

/// The markers, and the blocks that took lines which were not theirs.
///
/// One pass, two outputs. `parse_markers` is this without the second, and keeps its
/// signature: the marker sequence is a frozen parity contract and does not change here. What
/// changes is that a block reading past its own end is now something a caller can be told
/// about instead of something they infer from markers that are missing.
pub fn scan_markers(text: &str) -> Scan {
    let lines: Vec<&str> = text.lines().collect();
    let mut markers = Vec::new();
    let mut malformed = Vec::new();
    let mut i = 0;

    while i < lines.len() {
        let trimmed = lines[i].trim();

        // A TEMPLATE line that does not close on itself is not a marker and not a fault:
        // it falls through to the REGEN test, fails that too, and is skipped — which is
        // what the iterator version did by reaching the end of the loop body.
        if let Some(rest) = trimmed.strip_prefix("<!-- TEMPLATE:") {
            if let Some(raw) = rest.strip_suffix("-->") {
                markers.push(Marker::Template {
                    instruction: raw.trim().to_string(),
                });
                i += 1;
                continue;
            }
        }

        let Some(rest) = trimmed.strip_prefix("<!-- REGEN:") else {
            i += 1;
            continue;
        };

        let open_line = i;
        i += 1;
        let mut fault = None;

        let command = if let Some(cmd) = rest.trim_end().strip_suffix("-->") {
            // Single-line open tag: <!-- REGEN: cmd -->
            cmd.trim().to_string()
        } else {
            // Multi-line: the command is the rest of this line, and the inner lines run
            // until one ends the comment.
            let cmd = rest.trim().to_string();
            let mut closed = false;
            while i < lines.len() {
                let t = lines[i].trim();
                i += 1;
                if t == "-->" || t.ends_with("-->") {
                    closed = true;
                    break;
                }
            }
            if !closed {
                fault = Some(Fault::OpenArrowMissing);
            }
            cmd
        };

        let content_start = i;
        let mut content_end = lines.len();
        let mut closed = false;
        while i < lines.len() {
            if lines[i].trim() == "<!-- /REGEN -->" {
                content_end = i;
                i += 1;
                closed = true;
                break;
            }
            i += 1;
        }
        if fault.is_none() {
            fault = if !closed {
                Some(Fault::CloseTagMissing)
            } else if lines[content_start..content_end]
                .iter()
                .any(|l| opens_a_regen(l))
            {
                Some(Fault::ClosedOnAnothersTag)
            } else {
                None
            };
        }

        if let Some(fault) = fault {
            // From the open tag to wherever the content stopped, which in the `OpenArrow`
            // case is the end of the input: the body is empty there and everything was
            // consumed looking for the arrow, so a count over the body alone reports nothing.
            let swallowed = &lines[open_line + 1..content_end];
            malformed.push(MalformedBlock {
                command: command.clone(),
                line: open_line + 1,
                fault,
                swallowed_lines: swallowed.len(),
                swallowed_markers: swallowed.iter().filter(|l| opens_a_marker(l)).count(),
            });
        }

        let content = lines[content_start..content_end]
            .join("\n")
            .trim()
            .to_string();
        markers.push(Marker::Regen { command, content });
    }

    Scan { markers, malformed }
}

pub fn parse_markers(text: &str) -> Vec<Marker> {
    scan_markers(text).markers
}

pub fn update_regen(text: &str, command: &str, new_content: &str) -> String {
    let open_tag = format!("<!-- REGEN: {command}");
    let close_tag = "<!-- /REGEN -->";

    let Some(open_pos) = text.find(&open_tag) else {
        return text.to_string();
    };
    let after_open = open_pos + open_tag.len();
    let Some(arrow_rel) = text[after_open..].find("-->") else {
        return text.to_string();
    };
    let content_start = after_open + arrow_rel + 3;
    let Some(close_rel) = text[content_start..].find(close_tag) else {
        return text.to_string();
    };
    let close_abs = content_start + close_rel;
    if new_content.is_empty() {
        // Clear the body without leaving a blank line between the markers.
        format!("{}\n{}", &text[..content_start], &text[close_abs..])
    } else {
        format!(
            "{}\n{}\n{}",
            &text[..content_start],
            new_content,
            &text[close_abs..]
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scan(text: &str) -> Scan {
        scan_markers(text)
    }

    #[test]
    fn a_closed_block_reports_nothing() {
        let s = scan("<!-- REGEN: a -->\nbody\n<!-- /REGEN -->\n");
        assert_eq!(s.markers.len(), 1);
        assert!(s.malformed.is_empty(), "{:?}", s.malformed);
    }

    /// The example #524 was filed with. The second block is not a marker; it is the first
    /// block's content, and until now nothing said so.
    #[test]
    fn a_block_with_no_close_tag_swallows_the_markers_below_it() {
        let s = scan("<!-- REGEN: a -->\n<!-- REGEN: b -->\n");
        assert_eq!(
            s.markers,
            vec![Marker::Regen {
                command: "a".into(),
                content: "<!-- REGEN: b -->".into(),
            }],
            "the marker sequence is a parity contract and must not change here"
        );
        assert_eq!(
            s.malformed,
            vec![MalformedBlock {
                command: "a".into(),
                line: 1,
                fault: Fault::CloseTagMissing,
                swallowed_lines: 1,
                swallowed_markers: 1,
            }]
        );
    }

    /// A block that swallows nothing is still unterminated: `update_regen` will not touch it.
    #[test]
    fn an_unterminated_block_at_the_end_of_a_file_is_still_reported() {
        let s = scan("intro\n\n<!-- REGEN: a -->\n");
        assert_eq!(s.malformed.len(), 1);
        assert_eq!(s.malformed[0].line, 3);
        assert_eq!(s.malformed[0].swallowed_lines, 0);
        assert_eq!(s.malformed[0].swallowed_markers, 0);
    }

    /// The other way a block runs off the end: a multi-line open tag whose `-->` never lands.
    /// The body is empty in that case, so a count over the body would report nothing taken —
    /// which is why `swallowed_lines` is counted from the open tag instead.
    #[test]
    fn an_open_tag_whose_arrow_never_arrives_is_its_own_case() {
        let s = scan("<!-- REGEN: a\nFields: x.\nmore prose\n");
        assert_eq!(s.malformed.len(), 1);
        assert_eq!(s.malformed[0].fault, Fault::OpenArrowMissing);
        assert_eq!(s.malformed[0].command, "a");
        assert_eq!(s.malformed[0].swallowed_lines, 2);
        assert_eq!(s.malformed[0].swallowed_markers, 0);
    }

    /// Any line ending in `-->` closes a multi-line open tag, including one that is itself a
    /// marker. Written down because the first draft of the test above used a TEMPLATE line to
    /// stand for "ordinary prose" and got `CloseTag`: the TEMPLATE line had ended the open
    /// tag, and the block then ran off the end looking for its close instead.
    #[test]
    fn a_template_line_closes_a_multi_line_open_tag() {
        let s = scan("<!-- REGEN: a\n<!-- TEMPLATE: t -->\nbody\n<!-- /REGEN -->\n");
        assert_eq!(
            s.markers,
            vec![Marker::Regen {
                command: "a".into(),
                content: "body".into(),
            }],
            "the TEMPLATE line was read as the end of the open tag, not as a marker"
        );
        assert!(s.malformed.is_empty());
    }

    /// The shape a damaged file actually has, and the one `CloseTagMissing` cannot see.
    ///
    /// That fault needs the broken block to be the last in the document. Give it a sibling
    /// below and the scan runs straight past the sibling's open tag, closes on the sibling's
    /// close tag, and reports a single well-formed block — one marker where there were two,
    /// with nothing missing. Found while writing the parity fixture for the other case.
    #[test]
    fn a_block_that_closes_on_the_next_blocks_tag_is_reported_too() {
        let s = scan("<!-- REGEN: a -->\nfirst\n\n<!-- REGEN: b -->\nsecond\n<!-- /REGEN -->\n");
        assert_eq!(
            s.markers.len(),
            1,
            "b is this block's content, not a marker"
        );
        assert_eq!(
            s.malformed,
            vec![MalformedBlock {
                command: "a".into(),
                line: 1,
                fault: Fault::ClosedOnAnothersTag,
                swallowed_lines: 4,
                swallowed_markers: 1,
            }]
        );
    }

    #[test]
    fn two_well_formed_blocks_report_nothing() {
        let s =
            scan("<!-- REGEN: a -->\nx\n<!-- /REGEN -->\n<!-- REGEN: b -->\ny\n<!-- /REGEN -->\n");
        assert_eq!(s.markers.len(), 2);
        assert!(s.malformed.is_empty());
    }

    #[test]
    fn a_template_marker_is_untouched_by_any_of_this() {
        let s = scan("<!-- TEMPLATE: write something -->\n");
        assert_eq!(
            s.markers,
            vec![Marker::Template {
                instruction: "write something".into()
            }]
        );
        assert!(s.malformed.is_empty());
    }

    /// `parse_markers` is `scan_markers` without the second half, and nothing about the
    /// marker sequence moved. The parity fixtures grade this too; this states it locally.
    #[test]
    fn parse_markers_is_the_scan_without_the_diagnostics() {
        for text in [
            "<!-- REGEN: a -->\nbody\n<!-- /REGEN -->\n",
            "<!-- REGEN: a -->\n<!-- REGEN: b -->\n",
            "<!-- TEMPLATE: t -->\n<!-- REGEN: c\n-->\nz\n<!-- /REGEN -->\n",
            "",
        ] {
            assert_eq!(parse_markers(text), scan_markers(text).markers, "{text:?}");
        }
    }
}
