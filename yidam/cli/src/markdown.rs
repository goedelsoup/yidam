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
///
/// **An unmatched backtick is literal and masks nothing.** That is CommonMark's own rule,
/// and it is here because the obvious alternative — a toggle reset at the head of every
/// line — reads a *closing* backtick as an opening one whenever a span wraps, and inverts
/// the mask on the line that closes it:
///
/// ```text
/// …the formula was `ADM × base
/// cost × valuation`. [verified — ¶18]
/// ```
///
/// The tail of the second line is the span's *end* and ordinary prose after it; the toggle
/// blanked all of it, so `[verified — ¶18]` vanished four words from a `¶17` that was
/// reported. Under the rule below neither line holds a pair, so neither is touched, and the
/// tag survives. 42 lines across 14 files of one derived corpus carry an odd backtick, and
/// the count only grows with the corpus.
///
/// The interior of a wrapping span is not masked either, and that is the deliberate half of
/// the trade. Carrying `in_code` from line to line would mask it, and measures identically
/// — 399 findings both ways on the corpus that reported this — but it buys a failure with no
/// bound: one stray backtick anywhere then inverts the mask for the rest of the file. The
/// paired rule has no such tail, and under-masking is the direction a *scanner* can survive:
/// a mention read as prose is a finding a reader can dismiss, where prose read as a mention
/// is a finding nobody ever sees.
pub fn mask_code_spans(line: &str) -> String {
    let ticks: Vec<usize> = line.match_indices('`').map(|(i, _)| i).collect();
    let mut out = String::with_capacity(line.len());
    let mut cursor = 0;
    // `chunks_exact` drops a trailing odd tick on the floor, which is the rule: it pairs
    // with nothing, so it delimits nothing.
    for pair in ticks.chunks_exact(2) {
        let (open, close) = (pair[0], pair[1]);
        out.push_str(&line[cursor..=open]);
        for c in line[open + 1..close].chars() {
            // Keep the byte count identical so offsets stay meaningful.
            for _ in 0..c.len_utf8() {
                out.push(' ');
            }
        }
        out.push('`');
        cursor = close + 1;
    }
    out.push_str(&line[cursor..]);
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
    fn an_unclosed_span_masks_nothing_at_all() {
        let masked = mask_code("a `[open] and\n[open] again\n");
        assert_eq!(
            masked.matches("[open]").count(),
            2,
            "a backtick that pairs with nothing delimits nothing — and above all it must \
             not swallow the rest of the line, let alone the document"
        );
    }

    /// The reported defect: the line carrying a wrapping span's **closing** backtick had its
    /// mask inverted, so the span's end and the prose after it were blanked instead of its
    /// contents. `¶17` was reported and `¶18`, four words later in the same sentence, was
    /// not.
    #[test]
    fn a_closing_backtick_does_not_invert_the_mask() {
        let text = "income and property wealth. [verified — ¶17] The formula was `ADM × base\n\
                    cost × cost-of-doing-business − 0.023 × valuation`. [verified — ¶18]\n";
        let masked = mask_code(text);
        assert!(
            masked.contains("[verified — ¶18]"),
            "the tag after the closing backtick must survive: {masked}"
        );
        assert!(
            masked.contains("[verified — ¶17]"),
            "and so must the one before the opening backtick: {masked}"
        );
        assert_eq!(masked.len(), text.len(), "byte offsets must not move");
    }

    /// The other half of the same defect, and the half this fix answers by *not* masking.
    ///
    /// A span crossing three lines left its middle line with the toggle false from its first
    /// character, so nothing on it was masked while the lines around it were half-masked.
    /// The paired rule makes that consistent rather than accidental: no line of a wrapping
    /// span holds a pair, so every line of it is left whole. The cost is that a token quoted
    /// inside such a span reads as prose — see [`mask_code_spans`] for why that direction was
    /// chosen over carrying state across lines, which measured identically and has no bound.
    #[test]
    fn every_line_of_a_wrapping_span_is_left_whole() {
        let text = "before `a rate of\n[open] per year\nand rising` after\n";
        let masked = mask_code(text);
        assert_eq!(
            masked, text,
            "no line here holds a pair, so none is touched"
        );
    }
}
