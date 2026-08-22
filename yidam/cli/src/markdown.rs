//! Telling what a document *says* from what it *shows*.
//!
//! A corpus is prose, and prose about a vocabulary contains the vocabulary. A node
//! explaining that a claim is not verified writes the token to name it — in backticks,
//! which is markdown's conventional signal for *mention rather than use*. A scanner reading
//! bytes cannot tell that apart from an assertion, and every scanner here reads bytes.
//!
//! This was found twice independently, in two checks that share no code: the claim-tag
//! counter behind `yidam status` counted a backticked `[verified]` as a verified claim, and
//! the link checker read a `[label](path)` shown as an example as a link that ought to
//! resolve. Two failures of one assumption is the argument for one implementation of the
//! answer, which is what this module is.
//!
//! Masking rather than stripping: every byte offset, line number, and line count survives,
//! so a caller can report a position in the masked text and have it mean something in the
//! original. That is why the delimiters stay and only their contents go.

/// Replace the contents of `` `…` `` spans on one line with spaces.
///
/// The backticks remain, so the result is the same length and a subsequent scan still sees
/// where the code was — only not what it said.
pub fn mask_code_spans(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut in_code = false;
    for c in line.chars() {
        if c == '`' {
            in_code = !in_code;
            out.push(c);
        } else if in_code {
            // Keep the byte count identical so offsets stay meaningful.
            for _ in 0..c.len_utf8() {
                out.push(' ');
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Replace the contents of fenced blocks and inline code spans with spaces.
///
/// Fenced lines are blanked whole, delimiters included: a fence's content is shown, not
/// said, and so is the fence. Line count and byte offsets are preserved.
///
/// Applied to YAML as readily as to markdown, because a corpus node is YAML whose
/// `description` is markdown, and the tokens this exists to protect live in that field.
pub fn mask_code(text: &str) -> String {
    mask(text, true)
}

/// Blank fenced blocks only, leaving inline code spans intact.
///
/// The claim counter wants this and not [`mask_code`]. A fenced block is shown rather than
/// said and holds no claims; an inline span is ordinary prose punctuation, and treating it as
/// a mention marker cost a derived corpus 80% of its `[open]` claims — see
/// [`crate::claims`], which decides mention-versus-claim grammatically instead.
pub fn mask_fenced(text: &str) -> String {
    mask(text, false)
}

fn mask(text: &str, spans: bool) -> String {
    let mut out = String::with_capacity(text.len());
    let mut fence: Option<String> = None;
    for raw in text.split_inclusive('\n') {
        let (line, nl) = match raw.strip_suffix('\n') {
            Some(l) => (l, "\n"),
            None => (raw, ""),
        };
        let trimmed = line.trim_start();
        let is_fence = trimmed.starts_with("```") || trimmed.starts_with("~~~");
        match &fence {
            // Inside a block: blank everything, and close on a matching marker. The marker
            // is compared rather than assumed so a ``` inside a ~~~ block does not end it.
            Some(open) => {
                if trimmed.starts_with(open.as_str()) {
                    fence = None;
                }
                blank_into(line, &mut out);
            }
            None if is_fence => {
                fence = Some(trimmed.chars().take(3).collect());
                blank_into(line, &mut out);
            }
            None if spans => out.push_str(&mask_code_spans(line)),
            None => out.push_str(line),
        }
        out.push_str(nl);
    }
    out
}

fn blank_into(line: &str, out: &mut String) {
    for _ in 0..line.len() {
        out.push(' ');
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_mention_in_backticks_is_masked_and_prose_is_not() {
        let masked = mask_code("The tag `[verified]` is not [verified] here.");
        assert!(!masked.contains("`[verified]`"));
        assert!(
            masked.contains("is not [verified] here"),
            "the used token survives: {masked}"
        );
    }

    #[test]
    fn offsets_and_line_counts_survive() {
        let text = "a `x` b\nplain\n```\nfenced\n```\ntail\n";
        let masked = mask_code(text);
        assert_eq!(masked.len(), text.len(), "byte offsets must not move");
        assert_eq!(masked.lines().count(), text.lines().count());
        assert_eq!(masked.lines().nth(1).unwrap(), "plain");
        assert_eq!(masked.lines().nth(3).unwrap().trim(), "");
        assert_eq!(masked.lines().nth(5).unwrap(), "tail");
    }

    /// Multi-byte characters inside a span cost more than one space if replaced naively,
    /// and every offset after them would shift.
    #[test]
    fn multibyte_content_keeps_its_width() {
        let text = "`— é 世`";
        assert_eq!(mask_code(text).len(), text.len());
    }

    #[test]
    fn a_fence_of_one_marker_does_not_close_on_another() {
        let text = "~~~\n[open]\n```\nstill inside [open]\n~~~\n[open]\n";
        let masked = mask_code(text);
        assert_eq!(
            masked.matches("[open]").count(),
            1,
            "only the last is prose"
        );
    }

    /// Indented fences are the common case in a corpus node: the block lives inside a YAML
    /// block scalar and every line carries the scalar's indentation.
    #[test]
    fn an_indented_fence_still_opens() {
        let yaml = "description: |\n  Example:\n  ```\n  [open]\n  ```\n  Really [open].\n";
        let masked = mask_code(yaml);
        assert_eq!(masked.matches("[open]").count(), 1);
        assert!(masked.contains("Really [open]."));
    }

    #[test]
    fn text_without_a_trailing_newline_is_not_given_one() {
        assert_eq!(mask_code("plain").len(), 5);
        assert!(!mask_code("plain").ends_with('\n'));
    }

    #[test]
    fn an_unclosed_span_masks_to_the_end_of_its_line_only() {
        let masked = mask_code("a `[open] and\n[open] again\n");
        assert_eq!(
            masked.matches("[open]").count(),
            1,
            "a stray backtick must not swallow the rest of the document"
        );
    }
}
