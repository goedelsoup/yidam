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

/// Where a well-formed REGEN block sits in the text, so that a writer does not have to look
/// for it a second time.
///
/// The offsets are **byte** offsets, because this is Rust. They are not a parity contract and
/// no fixture asserts one: the TypeScript SDK would count UTF-16 code units and the Python SDK
/// code points, so a single number here is three different claims about any document holding
/// a character outside ASCII — the trap `find_reachable`'s sort order already fell into. What
/// the fixtures grade is what the scan and the writer *do*. See RFC-0043.
///
/// Only blocks the scan read to the end get one. A block whose open tag never closed, or whose
/// `<!-- /REGEN -->` never arrived, has no extent to write into, and `update_regen` left such a
/// block alone before this type existed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegenSpan {
    /// The command on the open tag, as the scan read it — trimmed, so it is what
    /// `update_regen` compares against and not the raw source text.
    pub command: String,
    /// Offset of the `<` that opens the block.
    pub open: usize,
    /// Offset just past the `-->` that ends the open tag. The body starts here.
    pub body: usize,
    /// Offset of the `<` in this block's `<!-- /REGEN -->`. The body ends here.
    pub close: usize,
    /// Whether the body holds no newline — the whole block sits on one line.
    ///
    /// This is the rule RFC-0043 states, applied to the text rather than to how the block was
    /// found: `-->44<!-- /REGEN -->` and `--><!-- /REGEN -->` are inline, and `-->\n<!-- /REGEN
    /// -->` — the shape a cleared section has — is not.
    pub inline: bool,
}

/// What one pass over the text found: the markers, the blocks that are malformed, and where
/// the well-formed ones are.
///
/// `PartialEq` and not `Eq`, because `Marker` is `PartialEq` only. Widening `Marker` to `Eq`
/// for the sake of a derive here would be a change to a published type for the convenience of
/// a new one.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Scan {
    pub markers: Vec<Marker>,
    pub malformed: Vec<MalformedBlock>,
    /// One per well-formed REGEN block, in document order. `update_regen` is defined over
    /// this and nothing else, which is what makes the reader and the writer agree about what
    /// a block is (#1094).
    pub regen: Vec<RegenSpan>,
}

const REGEN_OPEN: &str = "<!-- REGEN:";
const REGEN_CLOSE: &str = "<!-- /REGEN -->";
const ARROW: &str = "-->";

/// `str::lines()`, keeping each line's byte offset into the original text.
///
/// Written out rather than derived from `lines()` because the offset is the whole point and
/// recovering it afterwards means either pointer arithmetic or re-searching. The line's own
/// content is what `lines()` yields: the terminator is not part of it, and a `\r` before it
/// is not either.
fn lines_with_offsets(text: &str) -> Vec<(usize, &str)> {
    let mut out = Vec::new();
    let mut start = 0;
    let bytes = text.as_bytes();
    for (i, b) in bytes.iter().enumerate() {
        if *b == b'\n' {
            let mut end = i;
            if end > start && bytes[end - 1] == b'\r' {
                end -= 1;
            }
            out.push((start, &text[start..end]));
            start = i + 1;
        }
    }
    if start < text.len() {
        out.push((start, &text[start..]));
    }
    out
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
    let lines: Vec<(usize, &str)> = lines_with_offsets(text);
    let mut markers = Vec::new();
    let mut malformed = Vec::new();
    let mut regen: Vec<RegenSpan> = Vec::new();
    let mut i = 0;

    while i < lines.len() {
        let (line_at, line) = lines[i];
        let trimmed = line.trim();

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

        // Every block this line opens *and closes*, left to right — the inline form. A line
        // may carry more than one, and a block found here is finished: nothing below runs
        // for it. `col` walks past each one so the next search starts after its close tag
        // rather than inside its body.
        let mut col = 0;
        let mut block_open = None;
        while let Some(rel) = line[col..].find(REGEN_OPEN) {
            let open_col = col + rel;
            let after_open = open_col + REGEN_OPEN.len();
            let Some(arrow_rel) = line[after_open..].find(ARROW) else {
                block_open = Some(open_col);
                break;
            };
            let body_col = after_open + arrow_rel + ARROW.len();
            let Some(close_rel) = line[body_col..].find(REGEN_CLOSE) else {
                block_open = Some(open_col);
                break;
            };
            let close_col = body_col + close_rel;
            let command = line[after_open..after_open + arrow_rel].trim().to_string();
            let content = line[body_col..close_col].trim().to_string();
            regen.push(RegenSpan {
                command: command.clone(),
                open: line_at + open_col,
                body: line_at + body_col,
                close: line_at + close_col,
                inline: true,
            });
            markers.push(Marker::Regen { command, content });
            col = close_col + REGEN_CLOSE.len();
        }

        // What is left is an open tag with no close beside it — the block form, which is
        // only a block when it starts its line. A `<!-- REGEN:` in the middle of a sentence
        // with no close tag after it is prose, and was prose before this function learned
        // the inline form; reading it as a block would make it swallow the rest of the file.
        let Some(open_col) = block_open.filter(|&c| line[..c].trim().is_empty()) else {
            i += 1;
            continue;
        };
        let rest = &line[open_col + REGEN_OPEN.len()..];

        let open_line = i;
        i += 1;
        let mut fault = None;
        let mut body_at = None;

        let command = if let Some(cmd) = rest.trim_end().strip_suffix(ARROW) {
            // Single-line open tag: <!-- REGEN: cmd -->
            //
            // The arrow is the last thing on the line by that test, so the body starts where
            // the line's own trailing whitespace does — which is where `update_regen` put it
            // when it searched for the arrow itself.
            body_at = Some(line_at + line.trim_end().len());
            cmd.trim().to_string()
        } else {
            // Multi-line: the command is the rest of this line, and the inner lines run
            // until one ends the comment.
            let cmd = rest.trim().to_string();
            let mut closed = false;
            while i < lines.len() {
                let (at, l) = lines[i];
                let t = l.trim();
                i += 1;
                if t == ARROW || t.ends_with(ARROW) {
                    body_at = Some(at + l.trim_end().len());
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
        let mut close_at = None;
        let mut closed = false;
        while i < lines.len() {
            let (at, l) = lines[i];
            if l.trim() == REGEN_CLOSE {
                content_end = i;
                close_at = Some(at + (l.len() - l.trim_start().len()));
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
                .any(|(_, l)| opens_a_regen(l))
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
                swallowed_markers: swallowed.iter().filter(|(_, l)| opens_a_marker(l)).count(),
            });
        }

        // An extent, but only for a block with two ends. `OpenArrowMissing` has no body to
        // start, and `CloseTagMissing` none to end; `update_regen` returned such a file
        // unchanged when it did its own searching, and it returns it unchanged now.
        if let (Some(body), Some(close)) = (body_at, close_at) {
            regen.push(RegenSpan {
                command: command.clone(),
                open: line_at + open_col,
                body,
                close,
                inline: !text[body..close].contains('\n'),
            });
        }

        let content = lines[content_start..content_end]
            .iter()
            .map(|(_, l)| *l)
            .collect::<Vec<_>>()
            .join("\n")
            .trim()
            .to_string();
        markers.push(Marker::Regen { command, content });
    }

    Scan {
        markers,
        malformed,
        regen,
    }
}

pub fn parse_markers(text: &str) -> Vec<Marker> {
    scan_markers(text).markers
}

/// The body to write between a block's markers, or `None` for a block that cannot hold it.
///
/// An inline block keeps its line: the author wrote it that way and no regeneration
/// second-guesses them. Every other block is bracketed by newlines, and the empty case is
/// collapsed so that clearing a section does not leave a blank line between the markers.
fn regen_body(new_content: &str, inline: bool, stands_alone: bool) -> Option<String> {
    // `inline` is the form the block was written in; `!contains('\n')` is whether the new
    // content can still be written that way. Both, because writing a multi-line value into a
    // one-line block would leave a document whose form no longer matches its body — and the
    // next run would read it as a block-form block and rewrite it differently. Deciding on
    // both is what makes a second run a no-op.
    //
    // A value that does not fit goes in block form, and block form is a *line* form: the scan
    // reads one only where the open tag starts its line and the close tag stands alone on
    // its own (#1137). An inline block with anything else on its line cannot be given one. In
    // the middle of a sentence the open tag becomes prose and the block is never read again;
    // with text after it, the close tag no longer stands alone, and the next run reads a block
    // that runs on to some later block's close tag and writes over that block too. So such a
    // block is left as it is — which is what this function already did with a block the scan
    // cannot read.
    if inline && !new_content.contains('\n') {
        Some(new_content.to_string())
    } else if inline && !stands_alone {
        None
    } else if new_content.is_empty() {
        Some("\n".to_string())
    } else {
        Some(format!("\n{new_content}\n"))
    }
}

/// Whether nothing but whitespace shares the block's line: before its open tag, and after its
/// close tag. For an inline block, that is whether block form can be written in its place.
fn stands_alone(text: &str, span: &RegenSpan) -> bool {
    let start = text[..span.open].rfind('\n').map_or(0, |i| i + 1);
    let end = span.close + REGEN_CLOSE.len();
    let stop = text[end..].find('\n').map_or(text.len(), |i| end + i);
    text[start..span.open].trim().is_empty() && text[end..stop].trim().is_empty()
}

/// Replace the body of every REGEN block named `command`.
///
/// Defined over `scan_markers`, which is the point: before #1094 this searched the text
/// itself, and the two searches disagreed. A block the scan cannot read is a block this
/// leaves alone, and a block it reads inline stays inline — or, given a value that needs more
/// than one line, takes the block form if the line is its own and is left alone if not.
///
/// Two things changed with the second search's removal, and both close a defect rather than
/// open a feature. The command matches **exactly**, where the old prefix search let
/// `yidam status` claim a `yidam status-quo` block (#1058). And **every** block with the
/// command is rewritten, where the old search rewrote the first and left a second copy of
/// the same figure stale with nothing to report it.
pub fn update_regen(text: &str, command: &str, new_content: &str) -> String {
    let spans = scan_markers(text).regen;
    let mut matched = false;
    let mut out = String::with_capacity(text.len());
    let mut cursor = 0;
    for span in spans.iter().filter(|s| s.command == command) {
        let Some(body) = regen_body(new_content, span.inline, stands_alone(text, span)) else {
            continue;
        };
        matched = true;
        out.push_str(&text[cursor..span.body]);
        out.push_str(&body);
        cursor = span.close;
    }
    if !matched {
        return text.to_string();
    }
    out.push_str(&text[cursor..]);
    out
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

    // ---------------------------------------------------------------------------------
    // The inline form, and the defect that made writing one destructive (#1094, RFC-0043).
    // ---------------------------------------------------------------------------------

    /// The sentence #1071 wants to be able to write. Before this, the scan returned no
    /// marker at all and `update_regen` rewrote the block into block form, breaking the
    /// sentence around it — two answers, neither reported.
    #[test]
    fn a_block_in_the_middle_of_a_sentence_is_a_block() {
        let text = "Ohio has <!-- REGEN: yidam count district -->44<!-- /REGEN --> districts.\n";
        let s = scan(text);
        assert_eq!(
            s.markers,
            vec![Marker::Regen {
                command: "yidam count district".into(),
                content: "44".into(),
            }]
        );
        assert!(s.malformed.is_empty(), "{:?}", s.malformed);
        assert!(s.regen[0].inline);
        assert_eq!(
            &text[s.regen[0].open..s.regen[0].close],
            "<!-- REGEN: yidam count district -->44"
        );
    }

    /// And it stays a sentence when it is written. The regression this closes is the whole
    /// reason the marker format had to change before `count` could exist.
    #[test]
    fn writing_an_inline_block_leaves_the_sentence_alone() {
        let text = "Ohio has <!-- REGEN: yidam count district -->44<!-- /REGEN --> districts.\n";
        assert_eq!(
            update_regen(text, "yidam count district", "99"),
            "Ohio has <!-- REGEN: yidam count district -->99<!-- /REGEN --> districts.\n"
        );
    }

    /// The worse half of #1094. At column 0 the old scan found no `-->` at end of line, took
    /// the multi-line branch, and ran forward to the *next block's* open tag — returning one
    /// marker whose command was `a -->1<!-- /REGEN -->`, whose content was the next block's
    /// body, and whose `malformed` was empty. A block was consumed and nothing said so.
    #[test]
    fn an_inline_block_at_column_zero_does_not_swallow_the_block_below_it() {
        let s =
            scan("<!-- REGEN: a -->1<!-- /REGEN -->\n\n<!-- REGEN: b -->\nbody\n<!-- /REGEN -->\n");
        assert_eq!(
            s.markers,
            vec![
                Marker::Regen {
                    command: "a".into(),
                    content: "1".into(),
                },
                Marker::Regen {
                    command: "b".into(),
                    content: "body".into(),
                },
            ]
        );
        assert!(s.malformed.is_empty(), "{:?}", s.malformed);
    }

    /// The form is read off the block, not passed in, so one line may carry two of them and
    /// each is written independently. This is the shape a sentence with two figures has.
    #[test]
    fn one_line_can_carry_two_blocks() {
        let text = "<!-- REGEN: a -->1<!-- /REGEN --> of <!-- REGEN: b -->2<!-- /REGEN -->\n";
        let s = scan(text);
        assert_eq!(s.markers.len(), 2);
        assert_eq!(
            update_regen(text, "b", "7"),
            "<!-- REGEN: a -->1<!-- /REGEN --> of <!-- REGEN: b -->7<!-- /REGEN -->\n"
        );
    }

    /// The three bodies the rule has to separate, stated as the RFC states it: a block is
    /// inline when its body holds no newline. The third is what `update_regen` itself writes
    /// when it clears a section, and it must stay a block or clearing one would collapse it.
    #[test]
    fn inline_is_a_body_with_no_newline() {
        let cases = [
            ("<!-- REGEN: a -->44<!-- /REGEN -->\n", true),
            ("<!-- REGEN: a --><!-- /REGEN -->\n", true),
            ("<!-- REGEN: a -->\n<!-- /REGEN -->\n", false),
            ("<!-- REGEN: a -->\nbody\n<!-- /REGEN -->\n", false),
        ];
        for (text, inline) in cases {
            let s = scan(text);
            assert_eq!(s.regen.len(), 1, "{text:?}");
            assert_eq!(s.regen[0].inline, inline, "{text:?}");
        }
    }

    /// Clearing an inline block empties it without turning it into two lines, and clearing a
    /// block-form one still leaves no blank line — the behaviour `ClearingASectionLeavesNoBlankLine`
    /// proves and the `empty-new-content` fixture grades.
    #[test]
    fn clearing_respects_the_form_the_block_was_written_in() {
        assert_eq!(
            update_regen("x <!-- REGEN: a -->44<!-- /REGEN --> y\n", "a", ""),
            "x <!-- REGEN: a --><!-- /REGEN --> y\n"
        );
        assert_eq!(
            update_regen("<!-- REGEN: a -->\nbody\n<!-- /REGEN -->\n", "a", ""),
            "<!-- REGEN: a -->\n<!-- /REGEN -->\n"
        );
    }

    /// A generator that returns more than one line cannot be written into a one-line block:
    /// the result would be a document whose body no longer matches the form it was written
    /// in, and the *next* run would read it as a block-form block and rewrite it again. So
    /// the block form is used where the block has its line to itself, and a second run is a
    /// no-op — which is what `regen --check` needs to be true to report drift rather than
    /// manufacture it.
    #[test]
    fn multi_line_content_takes_the_block_form_on_a_line_of_its_own() {
        let once = update_regen("<!-- REGEN: a -->44<!-- /REGEN -->  \n", "a", "one\ntwo");
        assert_eq!(once, "<!-- REGEN: a -->\none\ntwo\n<!-- /REGEN -->  \n");
        assert_eq!(update_regen(&once, "a", "one\ntwo"), once);
        let s = scan(&once);
        assert_eq!(s.regen.len(), 1);
        assert_eq!(
            (s.regen[0].command.as_str(), s.regen[0].inline),
            ("a", false)
        );
    }

    /// …and nowhere else, because the block form is read by line and the line is not the
    /// block's to break (#1137). Written anyway, the first document lost its block — the open
    /// tag became prose — and the second gained one: the close tag no longer stood alone, so
    /// the next run read `a` on down to `b`'s close tag and deleted `b`.
    #[test]
    fn multi_line_content_leaves_a_block_that_shares_its_line() {
        for text in [
            "x <!-- REGEN: a -->44<!-- /REGEN --> y\n",
            "<!-- REGEN: a -->44<!-- /REGEN --> y\n<!-- REGEN: b -->\nfoo\n<!-- /REGEN -->\n",
            "x <!-- REGEN: a -->44<!-- /REGEN -->\n<!-- REGEN: b -->\nfoo\n<!-- /REGEN -->\n",
            "<!-- REGEN: a -->1<!-- /REGEN --><!-- REGEN: a -->2<!-- /REGEN -->\n",
        ] {
            assert_eq!(update_regen(text, "a", "one\ntwo"), text, "{text:?}");
        }
        // A block it cannot write does not stop it writing one it can.
        assert_eq!(
            update_regen(
                "x <!-- REGEN: a -->1<!-- /REGEN -->\n<!-- REGEN: a -->2<!-- /REGEN -->\n",
                "a",
                "3\n4"
            ),
            "x <!-- REGEN: a -->1<!-- /REGEN -->\n<!-- REGEN: a -->\n3\n4\n<!-- /REGEN -->\n"
        );
    }

    /// A `<!-- REGEN:` mid-sentence with no close tag beside it is prose, and was prose
    /// before the inline form existed. Reading it as a block would make it swallow the rest
    /// of the file, which is a louder version of the bug this change removes.
    #[test]
    fn a_mid_line_open_tag_with_no_close_beside_it_is_not_a_block() {
        let text = "The open tag is <!-- REGEN: a --> and that is all.\nmore prose\n";
        let s = scan(text);
        assert!(s.markers.is_empty(), "{:?}", s.markers);
        assert!(s.malformed.is_empty(), "{:?}", s.malformed);
        assert_eq!(update_regen(text, "a", "x"), text, "and nothing writes it");
    }

    /// #1058, closed as a consequence. The old prefix search let a shorter command claim a
    /// longer one's block — and with `count` the argument is *in* the command, so the
    /// collision stops being a naming accident and becomes a property of the query text.
    #[test]
    fn a_command_is_matched_exactly_and_not_as_a_prefix() {
        let text = "<!-- REGEN: yidam status-quo -->old<!-- /REGEN -->\n";
        assert_eq!(
            update_regen(text, "yidam status", "new"),
            text,
            "`yidam status` is not `yidam status-quo`"
        );
        assert_eq!(
            update_regen(text, "yidam status-quo", "new"),
            "<!-- REGEN: yidam status-quo -->new<!-- /REGEN -->\n"
        );
    }

    /// The other consequence: two blocks asking the same question both get the answer. The
    /// old search wrote the first and left the second stale forever — and `--check` could not
    /// report it, because a block nothing writes is a block nothing compares.
    #[test]
    fn every_block_with_the_command_is_written_not_just_the_first() {
        let text = "<!-- REGEN: a -->1<!-- /REGEN -->\nmid\n<!-- REGEN: a -->1<!-- /REGEN -->\n";
        assert_eq!(
            update_regen(text, "a", "2"),
            "<!-- REGEN: a -->2<!-- /REGEN -->\nmid\n<!-- REGEN: a -->2<!-- /REGEN -->\n"
        );
    }

    /// A block the scan cannot read to the end has no extent, so the writer leaves it alone —
    /// which is what it did when it searched for itself, and the one behaviour worth keeping
    /// from that search.
    #[test]
    fn a_block_with_one_end_is_not_written() {
        for text in [
            "<!-- REGEN: a -->\nbody with no close tag\n",
            "<!-- REGEN: a\nan open tag whose arrow never lands\n",
        ] {
            assert!(scan(text).regen.is_empty(), "{text:?}");
            assert_eq!(update_regen(text, "a", "x"), text, "{text:?}");
        }
    }

    /// The Dafny model's witness for this document is `ACloseTagMustStandAlone` in
    /// `graph.dfy`. Both scans hold a block-form close tag to a line of its own, so both read
    /// no block here and leave the document alone. Change either side and one of the two goes
    /// red.
    #[test]
    fn a_close_tag_that_does_not_stand_alone_is_not_a_block_form_close() {
        let text = "<!-- REGEN: a -->\nx<!-- /REGEN -->";
        let s = scan(text);
        assert!(s.regen.is_empty(), "{:?}", s.regen);
        assert_eq!(s.malformed.len(), 1);
        assert_eq!(s.malformed[0].fault, Fault::CloseTagMissing);
        assert_eq!(update_regen(text, "a", "y"), text);
    }

    /// Offsets are bytes, and a document is not ASCII. Stated here because the extents are
    /// the one part of this that is *not* a parity contract: the same block is at a different
    /// number in each SDK, and a fixture asserting one would be asserting three things.
    #[test]
    fn an_extent_is_a_byte_offset_and_survives_a_multibyte_line() {
        let text = "Ohio — “the state” — has <!-- REGEN: a -->44<!-- /REGEN -->.\n";
        let s = scan(text);
        assert_eq!(&text[s.regen[0].body..s.regen[0].close], "44");
        assert_eq!(
            update_regen(text, "a", "45"),
            "Ohio — “the state” — has <!-- REGEN: a -->45<!-- /REGEN -->.\n"
        );
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
