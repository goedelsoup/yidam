//! Citations that name a line, and whether the line still says it.
//!
//! `broken-prose-link` resolves a link's file and deliberately drops the anchor: a
//! `#some-heading` fragment is a rendering concern, and headings are not ours to verify.
//! `#L42` is one character away and a different thing entirely — it names a line range in
//! a file this repository owns, it moves whenever anything above it is edited, and it is
//! decidable. The two were treated as one, and the result was documented in #563: twelve
//! constitution citations in four RFCs were stale at once, off by the length of a
//! blockquote added long after they were written, and two of them briefly pointed at text
//! written *that morning* as if it were the passage they were arguing about. Nothing went
//! red at any point.
//!
//! Three checks, one partition — every line citation lands in exactly one:
//!
//! - **`dead-line-citation`** — the range is past the end of the file, backwards, or
//!   entirely blank. The cheapest strength, and the only one a quoteless citation gets.
//! - **`slid-line-citation`** — the document quotes the passage beside the citation, and
//!   the quoted words do not appear in the cited lines. This is the strength worth having:
//!   a citation that slid ten lines still lands on *some* text, and only the quote can
//!   tell that it is the wrong text.
//! - **`unverified-line-citation`** — the citation carries no quote, so nothing beyond
//!   existence can be checked. Reported rather than passed, at Info, because a reader
//!   meeting `file.rs:42` will assume somebody verified it and nobody can.
//!
//! # What counts as a quote
//!
//! The citations are written in one house style: the document quotes the passage and then
//! cites it. The quote is found **beside** the link, never merely near it —
//!
//! - in quotation marks (or italics) immediately before the link: `"…" ([cite])`;
//! - immediately after it, introduced by a colon or dash: `([cite]): "…"`;
//! - as the body of a blockquote whose attribution line carries the link: `> — [cite]`.
//!
//! Adjacency is what makes the pairing trustworthy. An early version adopted the nearest
//! quoted span in the paragraph and promptly paired one citation's quote with its
//! neighbour's link; requiring the gap between quote and link to be pure punctuation
//! removed every mispairing in the measured population without losing a real quote.
//!
//! # How a quote is compared
//!
//! On words, not characters: both sides are lowercased and every non-alphanumeric byte
//! becomes a space. A quotation restates emphasis, comment markers, wrapping and
//! punctuation in its own voice — `//!` continuation markers in a quoted doc comment, `>`
//! markers in a quoted blockquote, an em-dash where the original had a colon — and every
//! one of those was a false drift in the measured population. Words are what rot. An
//! ellipsis in the quote is an elision: each piece must appear in the cited lines, in
//! order.

use std::collections::BTreeMap;
use std::path::Path;

use super::checks::ProseLink;
use super::model::{Check, Severity, Violation};

/// A `#L<n>` or `#L<n>-L<m>` fragment, 1-indexed and inclusive at both ends.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LineFragment {
    pub start: usize,
    pub end: usize,
}

/// Parse a link fragment as a line range, or `None` for anything else.
///
/// Only the two forms the house style writes are read. Every other fragment is a heading
/// anchor and stays what it always was: not ours to verify.
pub fn parse_fragment(frag: &str) -> Option<LineFragment> {
    let rest = frag.strip_prefix('L')?;
    let (a, b) = match rest.split_once("-L") {
        Some((a, b)) => (a, b),
        None => (rest, rest),
    };
    if a.is_empty() || b.is_empty() {
        return None;
    }
    if !a.bytes().all(|c| c.is_ascii_digit()) || !b.bytes().all(|c| c.is_ascii_digit()) {
        return None;
    }
    Some(LineFragment {
        start: a.parse().ok()?,
        end: b.parse().ok()?,
    })
}

/// One line-anchored citation, with everything the three checks need to decide it.
///
/// Built by [`collect`], which does the reading; the checks stay pure.
pub struct LineCitation {
    /// The citing file, repo-relative, and the line the link sits on.
    pub file: String,
    pub line: usize,
    /// The link target as written, for the message.
    pub target: String,
    pub fragment: LineFragment,
    /// Line count of the target file.
    pub target_lines: usize,
    /// The cited lines, joined; empty when the range is out of bounds.
    pub range: String,
    /// Quote candidates found beside the link. Empty means unverifiable.
    pub quotes: Vec<String>,
}

impl LineCitation {
    /// `path#L4` / `path#L4-L7`, as a reader would write it.
    fn anchor(&self) -> String {
        if self.fragment.start == self.fragment.end {
            format!("{}#L{}", self.target, self.fragment.start)
        } else {
            format!(
                "{}#L{}-L{}",
                self.target, self.fragment.start, self.fragment.end
            )
        }
    }

    fn dead_reason(&self) -> Option<String> {
        let LineFragment { start, end } = self.fragment;
        if start == 0 || end < start {
            return Some(format!("`{}` is not a line range", self.anchor()));
        }
        if start > self.target_lines || end > self.target_lines {
            return Some(format!(
                "`{}` points past the end of the file ({} lines)",
                self.anchor(),
                self.target_lines
            ));
        }
        if self.range.chars().all(char::is_whitespace) {
            return Some(format!("`{}` is blank", self.anchor()));
        }
        None
    }
}

/// Every line-anchored citation among `links`, decided against the files as `read` sees
/// them.
///
/// A link whose file does not resolve is skipped — that is `broken-prose-link`'s finding,
/// and reporting the line of a file that is not there would be blaming the fragment for
/// the path.
pub fn collect(
    root: &Path,
    links: &[ProseLink],
    read: &dyn Fn(&Path) -> String,
) -> Vec<LineCitation> {
    // The citing files, each read once: several citations per document is the norm.
    let mut sources: BTreeMap<&str, Vec<String>> = BTreeMap::new();
    let mut out = Vec::new();
    for link in links {
        let Some(fragment) = link.fragment else {
            continue;
        };
        if !link.resolved.is_file() {
            continue;
        }
        let target_text = read(&link.resolved);
        let target_lines: Vec<&str> = target_text.lines().collect();
        let total = target_lines.len();
        let range = match fragment.start >= 1 && fragment.start <= fragment.end {
            true => target_lines
                .get(fragment.start - 1..fragment.end.min(total))
                .unwrap_or(&[])
                .join("\n"),
            false => String::new(),
        };
        let citing = sources
            .entry(link.file.as_str())
            .or_insert_with(|| {
                read(&root.join(&link.file))
                    .lines()
                    .map(str::to_string)
                    .collect()
            })
            .clone();
        let quotes = match link.line >= 1 && link.line <= citing.len() {
            true => quotes_beside(&citing, link.line - 1, link.span),
            false => Vec::new(),
        };
        out.push(LineCitation {
            file: link.file.clone(),
            line: link.line,
            target: link.target.clone(),
            fragment,
            target_lines: total,
            range,
            quotes,
        });
    }
    out
}

// ── Finding the quote ────────────────────────────────────────────────────────

/// Leading blockquote markers stripped: `> > text` → `text`.
fn strip_blockquote(line: &str) -> &str {
    let mut s = line;
    loop {
        let t = s.trim_start();
        match t.strip_prefix('>') {
            Some(rest) => s = rest,
            None => return t,
        }
    }
}

fn is_blockquote(line: &str) -> bool {
    line.trim_start().starts_with('>')
}

/// Whether a line ends the paragraph a link sits in.
fn is_boundary(line: &str) -> bool {
    let t = line.trim();
    t.is_empty() || t.starts_with('#') || t.starts_with("```") || t.starts_with("~~~")
}

/// Every character a quote may be separated from its citation by.
///
/// Words in the gap are what disqualify a span: `"renaming severs edges" — [cite]` is a
/// quote and its citation, `"…". The next sentence ([cite])` is two sentences. A full stop
/// is excluded for exactly that case.
fn is_gap_char(c: char) -> bool {
    c.is_whitespace() || "()*_—–-:;,'\"".contains(c)
}

/// A quoted span: where it starts and ends in the haystack, and its content.
struct Span {
    start: usize,
    end: usize,
    text: String,
}

/// Delimited spans in `text`: `"…"`, `“…”`, and `*…*` (italics long enough to be a quoted
/// sentence rather than a stressed word).
fn spans(text: &str) -> Vec<Span> {
    let mut out = Vec::new();
    for (open, close, min) in [('"', '"', 8), ('“', '”', 8), ('*', '*', 25)] {
        let mut at = 0;
        while let Some(rel) = text[at..].find(open) {
            let start = at + rel;
            let body_at = start + open.len_utf8();
            let Some(rel_close) = text[body_at..].find(close) else {
                break;
            };
            let end = body_at + rel_close + close.len_utf8();
            let body = &text[body_at..body_at + rel_close];
            if body.len() >= min
                && body.len() <= 600
                && !body.starts_with(open)
                && !body.starts_with('\n')
            {
                out.push(Span {
                    start,
                    end,
                    text: body.to_string(),
                });
                at = end;
            } else {
                at = body_at;
            }
        }
    }
    out.sort_by_key(|s| s.start);
    out
}

/// The quote candidates beside the link at `(line_idx, span)` — see the module doc for
/// what "beside" means and why nearness is not enough.
fn quotes_beside(lines: &[String], line_idx: usize, span: (usize, usize)) -> Vec<String> {
    let mut out = Vec::new();
    let line = &lines[line_idx];

    // A blockquote attribution: the link's own line reads as `> — [cite]`, and the quote
    // is the block above it. A link merely mentioned inside an aside blockquote is not an
    // attribution, and the aside above it is not a quotation.
    if is_blockquote(line)
        && strip_blockquote(line)
            .trim_start()
            .starts_with(['—', '–', '-'])
    {
        let mut lo = line_idx;
        while lo > 0 && is_blockquote(&lines[lo - 1]) {
            lo -= 1;
        }
        let q = lines[lo..line_idx]
            .iter()
            .map(|l| strip_blockquote(l))
            .collect::<Vec<_>>()
            .join(" ");
        if !q.trim().is_empty() {
            out.push(q);
        }
    }

    let mut lo = line_idx;
    while lo > 0 && !is_boundary(&lines[lo - 1]) {
        lo -= 1;
    }
    let mut hi = line_idx;
    while hi + 1 < lines.len() && !is_boundary(&lines[hi + 1]) {
        hi += 1;
    }
    let (col, end_col) = span;

    // Before the link: the last quoted span, separated from it by punctuation alone.
    let mut before: Vec<&str> = lines[lo..line_idx].iter().map(String::as_str).collect();
    before.push(line.get(..col).unwrap_or(""));
    let before = before.join("\n");
    if let Some(s) = spans(&before).into_iter().next_back() {
        if before[s.end..].chars().all(is_gap_char) {
            out.push(s.text);
        }
    }

    // After the link: the first quoted span, introduced by a colon or a dash. A bare
    // punctuation gap is not enough on this side — `([cite]). "Term" is…` opens a new
    // sentence, not a quotation of the cited lines.
    let mut after: Vec<&str> = vec![line.get(end_col..).unwrap_or("")];
    after.extend(lines[line_idx + 1..=hi].iter().map(String::as_str));
    let after = after.join("\n");
    if let Some(s) = spans(&after).into_iter().next() {
        let gap = &after[..s.start];
        if gap.chars().all(is_gap_char) && gap.contains([':', '—', '–']) {
            out.push(s.text);
        }
    }

    out
}

// ── Comparing it ─────────────────────────────────────────────────────────────

/// Lowercased words: every non-alphanumeric byte becomes a space, runs collapse.
fn words(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_gap = true;
    for c in s.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
            in_gap = false;
        } else if !in_gap {
            out.push(' ');
            in_gap = true;
        }
    }
    if out.ends_with(' ') {
        out.pop();
    }
    out
}

/// Whether the quoted words appear in the cited lines — each elided piece in order, on
/// word boundaries.
fn quote_matches(quote: &str, range: &str) -> bool {
    let hay = format!(" {} ", words(range));
    let mut pos = 0;
    for piece in quote.split("...").flat_map(|p| p.split('…')) {
        let p = words(piece);
        if p.len() < 3 {
            continue;
        }
        let needle = format!(" {p} ");
        match hay[pos..].find(&needle) {
            Some(i) => pos = pos + i + p.len() + 1,
            None => return false,
        }
    }
    true
}

// ── The three checks ─────────────────────────────────────────────────────────

pub fn dead_line_citation(citations: &[LineCitation]) -> Check {
    let violations = citations
        .iter()
        .filter_map(|c| {
            c.dead_reason()
                .map(|why| Violation::new(format!("{}:{}", c.file, c.line), why))
        })
        .collect();
    Check::new(
        "dead-line-citation",
        "A citation names lines that are not there",
        Severity::Error,
        "A `#L` fragment names a line range in a file this repository owns, and unlike a \
         heading anchor it is decidable: the range either exists and holds text or it does \
         not. The founding instance cited a blank line — the passage had moved and the \
         number kept pointing at where it used to be. Error severity, under the baseline \
         ratchet like every other citation defect a person here can fix.",
        violations,
    )
}

pub fn slid_line_citation(citations: &[LineCitation]) -> Check {
    let violations = citations
        .iter()
        .filter(|c| {
            c.dead_reason().is_none()
                && !c.quotes.is_empty()
                && !c.quotes.iter().any(|q| quote_matches(q, &c.range))
        })
        .map(|c| {
            Violation::new(
                format!("{}:{}", c.file, c.line),
                format!(
                    "the quoted words beside `{}` do not appear in the cited lines",
                    c.anchor()
                ),
            )
        })
        .collect();
    Check::new(
        "slid-line-citation",
        "The quoted passage is not in the cited lines",
        Severity::Error,
        "An edit above a cited range moves the range off its passage without breaking \
         anything a file-level check can see: the citation still resolves, the lines still \
         hold text, and the text is now something else. Twelve citations rotted that way \
         at once, silently, and two came to point at words written long after the \
         arguments that cite them. Where the document quotes the passage it cites — the \
         house style — the quote decides. Matched on words, in order, elisions honoured, \
         so wrapping, emphasis and comment markers cannot manufacture a drift.",
        violations,
    )
}

pub fn unverified_line_citation(citations: &[LineCitation]) -> Check {
    let violations = citations
        .iter()
        .filter(|c| c.dead_reason().is_none() && c.quotes.is_empty())
        .map(|c| {
            Violation::new(
                format!("{}:{}", c.file, c.line),
                format!(
                    "`{}` names lines but quotes nothing beside the link, so only their \
                     existence was checked",
                    c.anchor()
                ),
            )
        })
        .collect();
    Check::new(
        "unverified-line-citation",
        "A line citation with no quote to hold it to",
        Severity::Info,
        "A citation that quotes nothing can only be checked for existence, and a range \
         that slid onto the wrong lines still exists. Most such citations point at code, \
         where the house style quotes nothing. Reported rather than passed, because a \
         reader meeting a bare `file.rs:42` will assume somebody verified it; Info \
         severity, because the fix — quote the passage, widen to a stable range, or drop \
         the fragment — is a judgement about the document, not a defect in the corpus. \
         Never gates, never baselined.",
        violations,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frag(s: &str) -> Option<(usize, usize)> {
        parse_fragment(s).map(|f| (f.start, f.end))
    }

    #[test]
    fn the_two_house_forms_parse_and_nothing_else_does() {
        assert_eq!(frag("L42"), Some((42, 42)));
        assert_eq!(frag("L42-L50"), Some((42, 50)));
        assert_eq!(frag("some-heading"), None);
        assert_eq!(frag("L42-50"), None, "not the house form");
        assert_eq!(frag("L"), None);
        assert_eq!(frag("L42x"), None);
        assert_eq!(frag(""), None);
    }

    // ── Fixture plumbing: a citing document and a target in one tempdir ──────

    fn cite(target_text: &str, doc: &str) -> (tempfile::TempDir, Vec<LineCitation>) {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join("docs")).unwrap();
        std::fs::write(tmp.path().join("target.md"), target_text).unwrap();
        std::fs::write(tmp.path().join("docs/doc.md"), doc).unwrap();
        let links = super::super::checks::prose_links("docs/doc.md", &tmp.path().join("docs"), doc);
        let read = |p: &Path| std::fs::read_to_string(p).unwrap_or_default();
        let cites = collect(tmp.path(), &links, &read);
        (tmp, cites)
    }

    const TARGET: &str = "\
one filler line
The graph does not merely contain knowledge — it holds the life of the knowing.
a trailing line
";

    #[test]
    fn a_quoted_citation_whose_line_says_it_is_clean() {
        let (_tmp, c) = cite(
            TARGET,
            "\"The graph does not merely contain knowledge\" ([`target.md:2`](../target.md#L2)).\n",
        );
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].quotes.len(), 1, "{:?}", c[0].quotes);
        assert!(dead_line_citation(&c).passed());
        assert!(slid_line_citation(&c).passed());
        assert!(unverified_line_citation(&c).passed());
    }

    /// #563's prescribed mutation: shift the target by inserting a line above the cited
    /// one, and the citation must go red — the range still exists, still holds text, and
    /// the text is now the wrong text.
    #[test]
    fn inserting_a_line_above_the_cited_passage_is_a_slide() {
        let shifted = format!("a new line, added later\n{TARGET}");
        let (_tmp, c) = cite(
            &shifted,
            "\"The graph does not merely contain knowledge\" ([`target.md:2`](../target.md#L2)).\n",
        );
        let check = slid_line_citation(&c);
        assert_eq!(check.violations.len(), 1);
        assert!(
            check.violations[0].detail.contains("target.md#L2"),
            "{}",
            check.violations[0].detail
        );
        // Still alive — the slide is exactly the case the dead check cannot see.
        assert!(dead_line_citation(&c).passed());
    }

    #[test]
    fn a_citation_past_the_end_of_the_file_is_dead() {
        let (_tmp, c) = cite(TARGET, "See [`target.md:9`](../target.md#L9).\n");
        let check = dead_line_citation(&c);
        assert_eq!(check.violations.len(), 1);
        assert!(check.violations[0].detail.contains("past the end"));
    }

    #[test]
    fn a_citation_of_a_blank_line_is_dead() {
        let (_tmp, c) = cite("text\n\nmore\n", "See [`target.md:2`](../target.md#L2).\n");
        let check = dead_line_citation(&c);
        assert_eq!(check.violations.len(), 1);
        assert!(
            check.violations[0].detail.contains("blank"),
            "{}",
            check.violations[0].detail
        );
    }

    #[test]
    fn a_backwards_range_is_dead() {
        let (_tmp, c) = cite(TARGET, "See [`target.md`](../target.md#L3-L2).\n");
        assert_eq!(dead_line_citation(&c).violations.len(), 1);
    }

    #[test]
    fn a_quoteless_citation_is_reported_unverified_not_passed() {
        let (_tmp, c) = cite(
            TARGET,
            "The rule is stated at [`target.md:2`](../target.md#L2).\n",
        );
        assert!(dead_line_citation(&c).passed());
        assert!(slid_line_citation(&c).passed());
        let check = unverified_line_citation(&c);
        assert_eq!(check.violations.len(), 1);
        assert_eq!(check.severity, Severity::Info);
    }

    /// The mispairing that shaped the design: two citations in one sentence, each with its
    /// own quote. Nearness would hand the second link the first link's quote.
    #[test]
    fn each_citation_pairs_with_its_own_quote() {
        let target = "renaming a node severs edges, so choose well\nrenaming severs edges here\n";
        let doc = "\"renaming a node severs edges, so choose well\" ([`t:1`](../target.md#L1)); \
                   \"renaming severs edges here\" ([`t:2`](../target.md#L2)).\n";
        let (_tmp, c) = cite(target, doc);
        assert_eq!(c.len(), 2);
        assert!(
            slid_line_citation(&c).passed(),
            "{:?}",
            slid_line_citation(&c).violations
        );
    }

    /// The house's other form: the quote follows the citation, introduced by a colon.
    #[test]
    fn a_quote_after_the_link_is_paired_when_introduced() {
        let (_tmp, c) = cite(
            TARGET,
            "[`target.md:2`](../target.md#L2): \"it holds the life of the knowing\".\n",
        );
        assert_eq!(c[0].quotes.len(), 1, "{:?}", c[0].quotes);
        assert!(slid_line_citation(&c).passed());
    }

    /// `([cite]). "Term" is…` opens a new sentence. Adopting it as the quote was a
    /// measured false positive.
    #[test]
    fn a_quoted_term_opening_the_next_sentence_is_not_the_quote() {
        let (_tmp, c) = cite(
            TARGET,
            "([`target.md:2`](../target.md#L2)). \"Some defined term of ours\" is discussed.\n",
        );
        assert!(c[0].quotes.is_empty(), "{:?}", c[0].quotes);
    }

    #[test]
    fn a_blockquote_with_an_attribution_line_is_a_quote() {
        let doc = "\
> The graph does not merely contain knowledge — it holds
> the life of the knowing.
> — [`target.md:2`](../target.md#L2), on holding
";
        let (_tmp, c) = cite(TARGET, doc);
        assert_eq!(c[0].quotes.len(), 1);
        assert!(
            slid_line_citation(&c).passed(),
            "{:?}",
            slid_line_citation(&c).violations
        );
    }

    /// An aside blockquote that happens to carry a citation mid-sentence is not quoting
    /// the lines it cites; holding its own prose against them was a measured false
    /// positive.
    #[test]
    fn an_aside_blockquote_is_not_an_attribution() {
        let doc = "\
> **Landed later**, and this aside explains where. The type is
> [`target.md:2`](../target.md#L2) and the rest moved on.
";
        let (_tmp, c) = cite(TARGET, doc);
        assert!(c[0].quotes.is_empty(), "{:?}", c[0].quotes);
    }

    /// Wrapping, emphasis, case and comment markers are a quotation's own voice; words
    /// are what must survive. The target here is a doc comment whose continuation
    /// markers interrupt every multi-line passage.
    #[test]
    fn markers_and_punctuation_do_not_manufacture_a_drift() {
        let target = "\
//! - **Path** — a sibling repository read where it sits. Not fetched, not hashed, not
//!   locked, because hashing a working tree that changes under you records nothing.
";
        let doc = "a path dependency is \"not fetched, not hashed, not locked, because hashing \
                   a working tree that changes under you records nothing\" \
                   ([`deps.rs:1-2`](../target.md#L1-L2)).\n";
        let (_tmp, c) = cite(target, doc);
        assert!(
            slid_line_citation(&c).passed(),
            "{:?}",
            slid_line_citation(&c).violations
        );
    }

    #[test]
    fn an_ellipsis_elides_and_order_still_binds() {
        let target = "Any elector may call one by naming the branch and then notifying the rest\n";
        let ok = cite(
            target,
            "\"call one by … notifying the rest\" ([`t:1`](../target.md#L1)).\n",
        )
        .1;
        assert!(
            slid_line_citation(&ok).passed(),
            "{:?}",
            slid_line_citation(&ok).violations
        );
        let out_of_order = cite(
            target,
            "\"notifying the rest … call one by\" ([`t:1`](../target.md#L1)).\n",
        )
        .1;
        assert_eq!(slid_line_citation(&out_of_order).violations.len(), 1);
    }

    /// Word boundaries: a quote must not match inside other words.
    #[test]
    fn a_quote_matches_whole_words_only() {
        let target = "the rationale is presented\n";
        let (_tmp, c) = cite(
            target,
            "\"ration ale is present\" ([`t:1`](../target.md#L1)).\n",
        );
        assert_eq!(slid_line_citation(&c).violations.len(), 1);
    }

    /// A heading anchor stays a heading anchor: no fragment, no citation, no finding.
    #[test]
    fn a_heading_anchor_is_not_a_line_citation() {
        let (_tmp, c) = cite(TARGET, "See [the section](../target.md#the-section).\n");
        assert!(c.is_empty());
    }

    /// The file not resolving is `broken-prose-link`'s finding; the fragment is not
    /// additionally blamed.
    #[test]
    fn a_missing_file_is_not_this_checks_finding() {
        let (_tmp, c) = cite(TARGET, "See [`gone`](../gone.md#L2).\n");
        assert!(c.is_empty());
    }

    #[test]
    fn no_citations_is_clean() {
        assert!(dead_line_citation(&[]).passed());
        assert!(slid_line_citation(&[]).passed());
        assert!(unverified_line_citation(&[]).passed());
    }
}
