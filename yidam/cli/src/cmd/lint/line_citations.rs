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
//! Four checks, one partition — every line citation lands in exactly one, and which one it
//! lands in is decided by what the document says about the target beside the link:
//!
//! - **`dead-line-citation`** — the range is past the end of the file, backwards, or
//!   entirely blank. The cheapest strength, and the only one an unanchored citation gets.
//! - **`slid-line-citation`** — the document quotes the passage beside the citation, and
//!   the quoted words do not appear in the cited lines. This is the strength worth having:
//!   a citation that slid ten lines still lands on *some* text, and only the quote can
//!   tell that it is the wrong text.
//! - **`citation-label-not-cited`** — nothing quotes the passage, but the label names a
//!   symbol — `[`unknown-class`](checks.rs#L749)`, the house form for citing code — and the
//!   cited lines do not say it. The weaker anchor, read only where there is no quote.
//! - **`unverified-line-citation`** — neither anchor is present, so nothing beyond
//!   existence can be checked. Reported rather than passed, at Info, because a reader
//!   meeting `file.rs:42` will assume somebody verified it and nobody can.
//!
//! A fifth check stands outside that partition because it never opens the target file.
//! The house style states the range twice — `[`file.rs:12-19`](../file.rs#L12-L19)` — and
//! **`citation-range-stated-twice`** holds the two copies to each other. 134 of the 149
//! line citations this repository's own gate can see write both; nothing had ever read
//! the left-hand one, so half a repair would have shipped silently (#622).
//!
//! # The anchor, and why the label is one
//!
//! A quote is the strong anchor and most citations do not carry one: 119 of this
//! repository's 150 line citations quote nothing, and for as long as that was the whole
//! story every one of them was checked for existence and nothing else. A range that slides
//! onto lines that are neither blank nor quoted is invisible to the two enforcing halves
//! at once — it exists, so it is not dead; nothing quotes it, so nothing can say it slid.
//! Seven citations had already rotted through that gap when a person found them by hand
//! while looking at something else (#627, #632).
//!
//! Six of the seven were labelled with the check they cited. That is the second anchor, and
//! it was there all along: the house writes a citation of code with its symbol as the label
//! and the coordinate in the fragment, so the label is a claim about the target that a
//! check can decide. It is read only where nothing quotes the passage, because a quote is
//! better evidence and should decide alone. See [`label_symbols`] for what counts, and what
//! deliberately does not — 104 of the 119 label a line number rather than a symbol, and a
//! label that restates the coordinate anchors nothing.
//!
//! # Naming the repair
//!
//! A slid citation is reported with the range its passage moved to, when that range is
//! decidable. Deciding it is the same word-matching that found the slide, run over every
//! window of the target instead of the cited one — see [`relocate`]. The suggestion is
//! withheld unless exactly one window matches: a passage that now appears twice, or not
//! at all, is a judgement about the document.
//!
//! **The range named is where the quoted words are, and nothing wider.** A citation often
//! spans a whole comment block while quoting two of its lines, and the tight window is
//! the only part of that a check can measure — the extra lines were the author's
//! judgement and are not recoverable from the file. So the suggestion is a floor: take it
//! as written, or widen it back out to whatever the citation was covering.
//!
//! This is the whole reason the check exists in the form it does. RFC-0030 cites a
//! comment in `astro.config.mjs` whose `const sidebar` array sits 180 lines above it, and
//! adding an entry there is a required step for every new docs page. The citation slid
//! three times in five days, and three times the answer was re-derived by hand from
//! information the check already held.
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

/// One line-anchored citation, with everything the four checks need to decide it.
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
    /// Quote candidates found beside the link. Empty means no strong anchor.
    pub quotes: Vec<String>,
    /// The symbols the label names, when it names any — the weak anchor, and the only one
    /// most citations of code carry. See [`label_symbols`].
    pub symbols: Vec<String>,
    /// The link's label exactly as written, `astro.config.mjs:243-246` backticks and all.
    /// The house style states the cited range here as well as in the fragment, and
    /// [`citation_range_stated_twice`] is what holds the two together.
    pub label: String,
    /// Where the quoted passage actually is now, for a citation that no longer holds it.
    ///
    /// `None` covers three different situations and deliberately does not distinguish
    /// them, because the advice is the same in all three: the citation is fine, the
    /// passage is gone, or the passage now appears more than once. Only a single
    /// unambiguous window earns a suggestion — see [`relocate`].
    pub moved_to: Option<LineFragment>,
}

/// `L4` / `L4-L7`, the fragment without its file.
fn render_fragment(f: LineFragment) -> String {
    match f.start == f.end {
        true => format!("L{}", f.start),
        false => format!("L{}-L{}", f.start, f.end),
    }
}

impl LineCitation {
    /// `path#L4` / `path#L4-L7`, as a reader would write it.
    fn anchor(&self) -> String {
        format!("{}#{}", self.target, render_fragment(self.fragment))
    }

    /// ` — the passage is now at L255-L258`, or nothing when it cannot be located.
    ///
    /// Written as a clause so a message reads the same with and without it: the check
    /// says what is wrong first, and where to put it only when it knows.
    fn moved_clause(&self) -> String {
        match self.moved_to {
            Some(f) => format!(" — the passage is now at {}", render_fragment(f)),
            None => String::new(),
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
        let on_line = match link.line >= 1 && link.line <= citing.len() {
            true => citing[link.line - 1].as_str(),
            false => "",
        };
        let quotes = match link.line >= 1 && link.line <= citing.len() {
            true => quotes_beside(&citing, link.line - 1, link.span),
            false => Vec::new(),
        };
        let label = label_of(on_line, link.span);
        let symbols = label_symbols(&label, &link.target);
        // The quote decides alone where there is one; the label is consulted only in its
        // absence, so a citation carrying both is judged exactly as it was before the label
        // became readable.
        let anchors: &[String] = match quotes.is_empty() {
            true => &symbols,
            false => &quotes,
        };
        // The passage is looked for only when it is not where the citation says it is:
        // relocation reads every window of the target, and the overwhelming majority of
        // citations are correct and cost nothing.
        let adrift = !anchors.is_empty() && !anchors.iter().any(|q| quote_matches(q, &range));
        let moved_to = match adrift {
            true => anchors
                .iter()
                .find_map(|q| match relocate(q, &target_text) {
                    Relocation::Found(f) => Some(f),
                    Relocation::Absent | Relocation::Ambiguous => None,
                }),
            false => None,
        };
        out.push(LineCitation {
            file: link.file.clone(),
            line: link.line,
            target: link.target.clone(),
            fragment,
            target_lines: total,
            range,
            quotes,
            symbols,
            label,
            moved_to,
        });
    }
    out
}

/// The `[label]` of the link occupying `span` on `line`, brackets stripped.
///
/// The span is the whole `[label](target)`, measured on the masked copy of the line that
/// found the link; masking preserves byte offsets, so it slices the raw line unchanged —
/// which matters, because the label's own backticks are exactly what masking removes.
fn label_of(line: &str, span: (usize, usize)) -> String {
    let Some(whole) = line.get(span.0..span.1) else {
        return String::new();
    };
    let Some(rest) = whole.strip_prefix('[') else {
        return String::new();
    };
    match rest.rfind("](") {
        Some(i) => rest[..i].to_string(),
        None => String::new(),
    }
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
    quote_matches_words(quote, &words(range))
}

/// [`quote_matches`] against a haystack already reduced to words.
///
/// Split out for [`relocate`], which tests one quote against every window of a file and
/// would otherwise re-normalise the same lines a few thousand times.
fn quote_matches_words(quote: &str, hay_words: &str) -> bool {
    let hay = format!(" {hay_words} ");
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

// ── Where it went ────────────────────────────────────────────────────────────

/// The outcome of looking for a quoted passage in the file that was supposed to hold it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Relocation {
    /// Exactly one window of the file holds the quote. This is the only outcome that
    /// names a line range, and the only one a person can act on without re-reading.
    Found(LineFragment),
    /// The quote is nowhere in the file. The passage was reworded or deleted, and no
    /// number is the answer.
    Absent,
    /// More than one window holds it. A suggestion here would be a guess dressed as a
    /// measurement.
    Ambiguous,
}

/// The widest a relocated passage may be, in lines.
///
/// The widest range holding a quote in this repository is 19 lines and the median is 2,
/// so the cap is about three times the measured maximum. It is what keeps the search
/// linear in the file rather than quadratic, and what stops a quote of common words from
/// matching a window the size of the file.
const RELOCATE_SPAN: usize = 60;

/// Where `quote` now lives in `target_text`, by the same word-matching that decided it
/// had moved.
///
/// Every window up to [`RELOCATE_SPAN`] lines is tried. For each start line only the
/// first end line that completes the quote is kept: adding lines to the front of a window
/// can never break a match, so that minimum end is non-decreasing as the start advances,
/// and distinct occurrences of the passage are exactly the distinct ends. The tightest
/// window for an occurrence is then the *last* start that reaches its end.
pub fn relocate(quote: &str, target_text: &str) -> Relocation {
    let per_line: Vec<String> = target_text.lines().map(words).collect();
    let n = per_line.len();
    // Keyed by end line; the value is the largest start that still completes the quote.
    let mut tightest: BTreeMap<usize, usize> = BTreeMap::new();
    for s in 0..n {
        let mut hay = String::new();
        for (e, line_words) in per_line.iter().enumerate().skip(s).take(RELOCATE_SPAN) {
            if !line_words.is_empty() {
                if !hay.is_empty() {
                    hay.push(' ');
                }
                hay.push_str(line_words);
            }
            if quote_matches_words(quote, &hay) {
                tightest.insert(e, s);
                break;
            }
        }
        // Two occurrences are enough to be ambiguous; a third changes no advice.
        if tightest.len() > 1 {
            return Relocation::Ambiguous;
        }
    }
    match tightest.iter().next() {
        Some((&e, &s)) => Relocation::Found(LineFragment {
            start: s + 1,
            end: e + 1,
        }),
        None => Relocation::Absent,
    }
}

// ── The range, stated twice ──────────────────────────────────────────────────

/// The line range a link's label states, when it states one.
///
/// The house style writes the range in the label as well as the fragment —
/// `` `astro.config.mjs:243-246` `` beside `#L243-L246` — and the label is the copy a
/// reader's eye lands on. Labels vary in what precedes the colon (a filename, an RFC
/// number, nothing at all), so only the trailing `:<n>` or `:<n>-<m>` is read.
pub fn label_range(label: &str) -> Option<LineFragment> {
    let s = label.trim().trim_end_matches('`').trim_end();
    let digits_from = |s: &str| s.len() - s.chars().rev().take_while(char::is_ascii_digit).count();
    let at = digits_from(s);
    if at == s.len() {
        return None;
    }
    let end: usize = s[at..].parse().ok()?;
    let head = &s[..at];
    let (head, start) = match head.chars().next_back() {
        Some('-' | '\u{2013}') => {
            let head = &head[..head.len() - head.chars().next_back()?.len_utf8()];
            let at = digits_from(head);
            if at == head.len() {
                return None;
            }
            (&head[..at], head[at..].parse().ok()?)
        }
        _ => (head, end),
    };
    match head.ends_with(':') {
        true => Some(LineFragment { start, end }),
        false => None,
    }
}

// ── The label, as an anchor ──────────────────────────────────────────────────

/// The symbols a label names, or nothing when it names none.
///
/// The label is the only thing most citations of code say about their target, and it says
/// it in one of two house forms. `[`checks.rs:1109`](…#L1109)` restates the coordinate,
/// which [`label_range`] already holds to the fragment and which anchors nothing: a line
/// number that agrees with itself is still a line number. `[`unknown-class`](…#L749)` names
/// the thing the line is supposed to declare, and that is a claim about the target's
/// *content* — the same kind of claim a quote makes, weaker, and present on citations no
/// quote will ever reach.
///
/// What is refused, and why each refusal is a measured false positive rather than caution:
///
/// - **A label that is not a code span.** `[the identity gate](…#L42)` is prose about the
///   argument, not a name from the file. The backticks are the house's own mark for "this
///   is written in the target's language".
/// - **A label [`label_range`] reads**, and a bare `[`356`]` — the continuation form when
///   three lines of one file are cited in a row. Both restate the coordinate.
/// - **A label that is a path suffix of the target.** `[`lint/mod.rs`]` names the file it
///   links to; requiring a file to name itself on the cited line would report every one.
/// - **Segments under three characters**, which `quote_matches` would ignore anyway and
///   which would otherwise let a one-letter generic vouch for a whole citation.
///
/// An argument list is dropped and `::` splits, so `[`report::YidamBlock::current()`]`
/// offers three chances to match a signature line that spells only the last of them. Any
/// one segment is enough: the alternative — every segment must appear — reports a citation
/// of `current()` for not repeating its module path, which is a fact about Rust and not
/// about the citation.
pub fn label_symbols(label: &str, target: &str) -> Vec<String> {
    if label_range(label).is_some() {
        return Vec::new();
    }
    let Some(inner) = label
        .trim()
        .strip_prefix('`')
        .and_then(|s| s.strip_suffix('`'))
        .map(str::trim)
    else {
        return Vec::new();
    };
    if inner.is_empty() {
        return Vec::new();
    }
    // A bare line number, or a bare range of them.
    if inner
        .chars()
        .all(|c| c.is_ascii_digit() || c == '-' || c == '\u{2013}')
    {
        return Vec::new();
    }
    // The file, named as the label of a link to it.
    fn seg(s: &str) -> Vec<&str> {
        s.split('/').filter(|p| !p.is_empty()).collect()
    }
    let (l, t) = (seg(inner), seg(target));
    if !l.is_empty() && l.len() <= t.len() && t[t.len() - l.len()..] == l[..] {
        return Vec::new();
    }
    let head = match inner.find('(') {
        Some(i) => &inner[..i],
        None => inner,
    };
    head.split("::")
        .map(words)
        .filter(|s| s.len() >= 3)
        .collect()
}

// ── The checks ───────────────────────────────────────────────────────────────

pub fn dead_line_citation(citations: &[LineCitation]) -> Check {
    let violations = citations
        .iter()
        .filter_map(|c| {
            c.dead_reason().map(|why| {
                Violation::new(format!("{}:{}", c.file, c.line), why + &c.moved_clause())
            })
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
                    "the quoted words beside `{}` do not appear in the cited lines{}",
                    c.anchor(),
                    c.moved_clause()
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
         so wrapping, emphasis and comment markers cannot manufacture a drift. Where the \
         passage is still in the file and in exactly one place, the finding names the \
         range it moved to: the same matching that found the drift decides it, and \
         re-deriving that by hand was the whole recurring cost. That range is where the \
         quoted words are and nothing wider, so a citation that deliberately spanned more \
         than it quoted is widened back out by hand.",
        violations,
    )
}

pub fn citation_label_not_cited(citations: &[LineCitation]) -> Check {
    let violations = citations
        .iter()
        .filter(|c| {
            c.dead_reason().is_none()
                && c.quotes.is_empty()
                && !c.symbols.is_empty()
                && !c.symbols.iter().any(|s| quote_matches(s, &c.range))
        })
        .map(|c| {
            Violation::new(
                format!("{}:{}", c.file, c.line),
                format!(
                    "the label {} names something the lines `{}` cites do not say{}",
                    c.label,
                    c.anchor(),
                    c.moved_clause()
                ),
            )
        })
        .collect();
    Check::new(
        "citation-label-not-cited",
        "A citation's label names something its lines do not say",
        Severity::Error,
        "The strong anchor is a quote, and 119 of this repository's 150 line citations \
         carry none. Each of those was checked for existence and nothing else, which \
         leaves the whole of the interesting failure uncovered: a range that slides onto \
         lines that are neither blank nor quoted is invisible to `dead-line-citation` and \
         `slid-line-citation` at once. Seven citations had already rotted through that gap \
         when a person found them by hand while looking at something else (#627), and six \
         of the seven were labelled with the very check they cited. The label is the anchor \
         they were carrying all along: the house writes a citation of code with its symbol \
         in the label and its coordinate in the fragment, so the label is a claim about the \
         target's content that a check can decide. Read only where nothing quotes the \
         passage — a quote is better evidence and decides alone — and only where the label \
         names a symbol rather than restating the line number, which is the other house \
         form and 104 of the 119. Matched on words exactly as a quote is, so wrapping, \
         emphasis and an argument list cannot manufacture a drift, and any one `::` segment \
         is enough. Error, under the baseline ratchet like every other citation defect a \
         person here can fix: this is a latch on a population that is already correct, not \
         a backlog.",
        violations,
    )
}

pub fn unverified_line_citation(citations: &[LineCitation]) -> Check {
    let violations = citations
        .iter()
        .filter(|c| c.dead_reason().is_none() && c.quotes.is_empty() && c.symbols.is_empty())
        .map(|c| {
            Violation::new(
                format!("{}:{}", c.file, c.line),
                format!(
                    "`{}` names lines but nothing beside the link quotes them or names what \
                     they say, so only their existence was checked",
                    c.anchor()
                ),
            )
        })
        .collect();
    Check::new(
        "unverified-line-citation",
        "A line citation with nothing to hold it to",
        Severity::Info,
        "A citation with neither anchor can only be checked for existence, and a range that \
         slid onto the wrong lines still exists. Info is not the whole answer to that and \
         was never meant to be — seven citations rotted through this exact gap and nothing \
         found them (#632). What closes the part of it that the document can close is \
         `citation-label-not-cited`, which reads the symbol a code citation labels as the \
         anchor a quote would have been. What is left here is the residue: a citation whose \
         label restates the line number and whose prose says nothing about the passage. \
         There, the document holds no claim about the target at all, and only a recorded \
         digest of the cited lines could tell a slide from an edit — an artifact with a \
         staleness question of its own, weighed against this and declined (#632). So the \
         severity stays Info, because the remedy — quote the passage, label the symbol, \
         widen to a stable range, or drop the fragment — is a judgement about the document \
         and not a defect in the corpus. Never gates, never baselined. The count is the \
         thing to watch: it is how much of the citation surface is taken on trust.",
        violations,
    )
}

pub fn citation_range_stated_twice(citations: &[LineCitation]) -> Check {
    let violations = citations
        .iter()
        .filter_map(|c| {
            let stated = label_range(&c.label)?;
            if stated == c.fragment {
                return None;
            }
            Some(Violation::new(
                format!("{}:{}", c.file, c.line),
                format!(
                    "the label says `{}` and the link points at `{}`",
                    render_fragment(stated),
                    render_fragment(c.fragment)
                ),
            ))
        })
        .collect();
    Check::new(
        "citation-range-stated-twice",
        "A citation's label and its link name different lines",
        Severity::Error,
        "The house style states the cited range twice — `[`file.rs:12-19`](../file.rs#L12-L19)` \
         — and 134 of the 149 line citations in this repository write both copies. Until \
         #622 nothing read the left-hand one, so the two could disagree indefinitely: the \
         scan sees the fragment, and the label is what a reader's eye lands on. The failure \
         this forecloses is half a repair. A citation that slid is fixed by editing a \
         number, and editing the one number that resolves leaves the other one lying. No \
         citation disagreed on the day this landed, which is the point — it is a latch on a \
         population that is already correct, not a backlog. A label that names no range is \
         the other house form and not a finding: most citations of code label a symbol.",
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
        cite_named("target.md", target_text, doc)
    }

    /// As [`cite`], with the target under a name of the caller's choosing — the citations
    /// of code this module exists for do not live in `.md` files.
    fn cite_named(
        name: &str,
        target_text: &str,
        doc: &str,
    ) -> (tempfile::TempDir, Vec<LineCitation>) {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join("docs")).unwrap();
        std::fs::write(tmp.path().join(name), target_text).unwrap();
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
        // And it says where the passage went, which is the repair (#622).
        assert_eq!(c[0].moved_to, Some(LineFragment { start: 3, end: 3 }));
        assert!(
            check.violations[0]
                .detail
                .contains("the passage is now at L3"),
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
        assert!(citation_range_stated_twice(&[]).passed());
    }

    // ── Where it went (#622) ────────────────────────────────────────────────

    fn moved(quote: &str, text: &str) -> Relocation {
        relocate(quote, text)
    }

    #[test]
    fn a_passage_that_moved_is_found_at_its_new_lines() {
        let text = "alpha\nbeta\nthe passage worth citing\ngamma\n";
        assert_eq!(
            moved("the passage worth citing", text),
            Relocation::Found(LineFragment { start: 3, end: 3 })
        );
    }

    /// The window is the passage, not everything above it. Every start line before the
    /// passage also completes the quote; the last one is the answer.
    #[test]
    fn the_tightest_window_is_the_one_reported() {
        let text = "filler\nfiller\nfiller\nthe passage worth citing\n";
        assert_eq!(
            moved("the passage worth citing", text),
            Relocation::Found(LineFragment { start: 4, end: 4 })
        );
    }

    #[test]
    fn a_passage_spanning_lines_relocates_as_a_range() {
        let text = "one\ntwo\nthe passage worth citing runs on\nacross a second line\nlast\n";
        assert_eq!(
            moved(
                "the passage worth citing runs on across a second line",
                text
            ),
            Relocation::Found(LineFragment { start: 3, end: 4 })
        );
    }

    /// Comment markers and wrapping are the quotation's own voice on this side too —
    /// relocation and drift-detection must agree about what a match is, or a citation
    /// could be called slid and then relocated onto the very lines it already cited.
    #[test]
    fn relocation_reads_words_the_way_the_drift_check_does() {
        let text = "//! - **Path** — a sibling repository read where it sits. Not fetched,\n                    //!   not hashed, not locked.\n";
        assert_eq!(
            moved("not fetched, not hashed, not locked", text),
            Relocation::Found(LineFragment { start: 1, end: 2 })
        );
    }

    /// Two copies of the passage, and no number is the answer. Guessing the first would
    /// be a measurement's shape on a coin flip.
    #[test]
    fn a_passage_appearing_twice_is_ambiguous_not_the_first_one() {
        let text = "the passage worth citing\nfiller\nthe passage worth citing\n";
        assert_eq!(
            moved("the passage worth citing", text),
            Relocation::Ambiguous
        );
    }

    #[test]
    fn a_passage_that_is_gone_is_absent() {
        let text = "alpha\nbeta\ngamma\n";
        assert_eq!(moved("the passage worth citing", text), Relocation::Absent);
    }

    /// Absent and Ambiguous both mean silence: a slid citation still goes red, and the
    /// message simply stops after saying so.
    #[test]
    fn an_unlocatable_passage_reports_the_slide_and_suggests_nothing() {
        let (_tmp, c) = cite(
            "the passage was rewritten entirely\n",
            "\"The graph does not merely contain knowledge\" ([`t:1`](../target.md#L1)).\n",
        );
        let check = slid_line_citation(&c);
        assert_eq!(check.violations.len(), 1);
        assert_eq!(c[0].moved_to, None);
        assert!(
            !check.violations[0].detail.contains("now at"),
            "{}",
            check.violations[0].detail
        );
    }

    /// A citation past the end of the file can still be relocated, and a reader fixing it
    /// wants the same sentence a slide gives them.
    #[test]
    fn a_dead_citation_names_the_repair_when_its_quote_still_locates_it() {
        let (_tmp, c) = cite(
            TARGET,
            "\"it holds the life of the knowing\" ([`t:9`](../target.md#L9)).\n",
        );
        let check = dead_line_citation(&c);
        assert_eq!(check.violations.len(), 1);
        assert!(
            check.violations[0]
                .detail
                .contains("the passage is now at L2"),
            "{}",
            check.violations[0].detail
        );
    }

    /// A correct citation is never searched for: relocation is the expensive path and it
    /// must stay on the failing branch.
    #[test]
    fn a_citation_that_still_holds_its_passage_is_not_relocated() {
        let (_tmp, c) = cite(
            TARGET,
            "\"The graph does not merely contain knowledge\" ([`t:2`](../target.md#L2)).\n",
        );
        assert_eq!(c[0].moved_to, None);
    }

    // ── The range, stated twice (#622) ──────────────────────────────────────

    fn lr(s: &str) -> Option<(usize, usize)> {
        label_range(s).map(|f| (f.start, f.end))
    }

    #[test]
    fn the_label_forms_this_repository_writes_all_parse() {
        assert_eq!(lr("`astro.config.mjs:243-246`"), Some((243, 246)));
        assert_eq!(lr("`t:1`"), Some((1, 1)));
        assert_eq!(
            lr("`0002:139-142`"),
            Some((139, 142)),
            "an RFC number, not a file"
        );
        assert_eq!(lr("`:40-41`"), Some((40, 41)), "the file is understood");
        assert_eq!(lr("`checks.rs:12\u{2013}19`"), Some((12, 19)), "an en dash");
    }

    /// A label that names no range is the other house form and not a finding — most
    /// citations of code label the symbol, not the line.
    #[test]
    fn a_label_naming_no_range_yields_nothing_to_compare() {
        assert_eq!(lr("unknown-class"), None);
        assert_eq!(lr("RFC-0002"), None, "trailing digits, but no colon");
        assert_eq!(lr("the identity gate"), None);
        assert_eq!(lr("`Class::new`"), None);
        assert_eq!(lr(""), None);
    }

    #[test]
    fn a_label_agreeing_with_its_fragment_is_clean() {
        let (_tmp, c) = cite(
            TARGET,
            "See [`target.md:2`](../target.md#L2) for the rule.\n",
        );
        assert_eq!(c[0].label, "`target.md:2`");
        assert!(citation_range_stated_twice(&c).passed());
    }

    /// Half a repair: the fragment was re-pointed and the label was left behind. Nothing
    /// read the label until #622, so this shipped silently.
    #[test]
    fn a_label_disagreeing_with_its_fragment_is_a_finding() {
        let (_tmp, c) = cite(
            TARGET,
            "See [`target.md:2`](../target.md#L3) for the rule.\n",
        );
        let check = citation_range_stated_twice(&c);
        assert_eq!(check.violations.len(), 1);
        assert!(
            check.violations[0].detail.contains("`L2`")
                && check.violations[0].detail.contains("`L3`"),
            "{}",
            check.violations[0].detail
        );
    }

    #[test]
    fn a_range_label_disagreeing_at_one_end_is_a_finding() {
        let (_tmp, c) = cite(TARGET, "See [`target.md:1-2`](../target.md#L1-L3).\n");
        assert_eq!(citation_range_stated_twice(&c).violations.len(), 1);
    }

    // ── The label, as an anchor (#632) ──────────────────────────────────────

    fn syms(label: &str, target: &str) -> Vec<String> {
        label_symbols(label, target)
    }

    #[test]
    fn a_backticked_label_that_is_not_a_coordinate_names_a_symbol() {
        assert_eq!(syms("`unknown-class`", "../checks.rs"), ["unknown class"]);
        assert_eq!(
            syms("`report::YidamBlock::current()`", "../report.rs"),
            ["report", "yidamblock", "current"]
        );
        assert_eq!(
            syms("`run_checks_with(&root, &opts, &overlay)`", "../mod.rs"),
            ["run checks with"],
            "the argument list is the call site's, not the file's"
        );
    }

    /// Four refusals, each a measured false positive rather than caution. A label that
    /// restates the coordinate, or names the file, says nothing about what the lines hold.
    #[test]
    fn a_label_that_states_no_symbol_anchors_nothing() {
        assert!(
            syms("`checks.rs:1109`", "../checks.rs").is_empty(),
            "a range"
        );
        assert!(
            syms("`356`", "../due.rs").is_empty(),
            "the continuation form, three lines of one file cited in a row"
        );
        assert!(
            syms("the identity gate", "../checks.rs").is_empty(),
            "prose about the argument, not a name from the file"
        );
        assert!(
            syms("`lint/mod.rs`", "../../yidam/cli/src/cmd/lint/mod.rs").is_empty(),
            "the label is the file it links to"
        );
        assert!(syms("``", "../checks.rs").is_empty());
    }

    #[test]
    fn a_symbol_label_the_cited_lines_do_not_say_is_a_finding() {
        let target = "fn unrelated() {}\npub fn unknown_class(nodes: &[Node]) -> Check {\n";
        let (_tmp, c) = cite_named(
            "checks.rs",
            target,
            "the class an instance belongs to ([`unknown-class`](../checks.rs#L1), Error).\n",
        );
        let check = citation_label_not_cited(&c);
        assert_eq!(check.violations.len(), 1);
        assert!(
            check.violations[0].detail.contains("`unknown-class`")
                && check.violations[0].detail.contains("checks.rs#L1"),
            "{}",
            check.violations[0].detail
        );
        // The slide the two enforcing halves cannot see: the line exists and holds text.
        assert!(dead_line_citation(&c).passed());
        assert!(slid_line_citation(&c).passed());
        // And it is no longer merely reported — the partition moved it.
        assert!(unverified_line_citation(&c).passed());
    }

    #[test]
    fn a_symbol_label_the_cited_lines_do_say_is_clean() {
        let target = "fn unrelated() {}\npub fn unknown_class(nodes: &[Node]) -> Check {\n";
        let (_tmp, c) = cite_named(
            "checks.rs",
            target,
            "the class an instance belongs to ([`unknown-class`](../checks.rs#L2), Error).\n",
        );
        assert!(
            citation_label_not_cited(&c).passed(),
            "{:?}",
            citation_label_not_cited(&c).violations
        );
        assert!(unverified_line_citation(&c).passed());
    }

    /// The quote is the better evidence and decides alone. A citation carrying both is
    /// judged exactly as it was before the label became readable — otherwise every
    /// symbol-labelled citation of a *passage* would be asked to repeat its own label.
    #[test]
    fn a_quote_beside_the_link_decides_and_the_label_is_not_consulted() {
        let target = "The graph does not merely contain knowledge.\n";
        let (_tmp, c) = cite_named(
            "checks.rs",
            target,
            "\"The graph does not merely contain knowledge\" ([`some_symbol_absent_here`](../checks.rs#L1)).\n",
        );
        assert!(!c[0].quotes.is_empty(), "{:?}", c[0].quotes);
        assert!(slid_line_citation(&c).passed());
        assert!(
            citation_label_not_cited(&c).passed(),
            "{:?}",
            citation_label_not_cited(&c).violations
        );
    }

    /// One partition, four checks: a dead range is dead, and is not additionally blamed
    /// for what its label says.
    #[test]
    fn a_dead_citation_is_not_also_a_label_finding() {
        let (_tmp, c) = cite_named(
            "checks.rs",
            "pub fn unknown_class() {}\n",
            "([`unknown-class`](../checks.rs#L9)).\n",
        );
        assert_eq!(dead_line_citation(&c).violations.len(), 1);
        assert!(citation_label_not_cited(&c).passed());
        assert!(unverified_line_citation(&c).passed());
    }

    /// Any one `::` segment is enough. The alternative reports a citation of `current()`
    /// for not repeating its module path, which is a fact about Rust and not about the
    /// citation.
    #[test]
    fn one_segment_of_a_qualified_label_is_enough() {
        let (_tmp, c) = cite_named(
            "report.rs",
            "impl YidamBlock {\n    pub fn current() -> Self {\n",
            "the fields [`report::YidamBlock::current()`](../report.rs#L2) already assembles\n",
        );
        assert!(
            citation_label_not_cited(&c).passed(),
            "{:?}",
            citation_label_not_cited(&c).violations
        );
    }

    // ── The seven (#627, reconstructed) ─────────────────────────────────────

    /// The six RFC-0018 citations #627 repointed by hand, as the diff's two sides write
    /// them: `(label, line before the repair, line after it)`.
    const REPOINTED: &[(&str, usize, usize)] = &[
        ("unknown-class", 472, 749),
        ("undeclared-property", 823, 988),
        ("missing-property", 881, 1138),
        ("property-type", 1032, 1383),
        ("unlicensed-edge", 1098, 1443),
        ("edge-target-class", 1164, 1508),
    ];

    /// `checks.rs` as it stood when the seven were repointed, at every line either side of
    /// the repair cites. Filler elsewhere — non-blank filler, because a citation of a blank
    /// line is the one case `dead-line-citation` already had.
    fn checks_rs_at_the_repair() -> String {
        // The real content at each cited line, read out of the tree at 5a0249e.
        let real: &[(usize, &str)] = &[
            (472, "/// shape `analytic_note` is exempted for, one field over. Deciding automatically which"),
            (749, "pub fn unknown_class(nodes: &[Node], defined: &HashSet<String>) -> Check {"),
            (823, "         it is not in the graph at all.\","),
            (988, "pub fn undeclared_property("),
            // 881 is the one the #600 gate did catch: the repair above it left it blank.
            (881, ""),
            (1138, "pub fn missing_property(nodes: &[Node], classes: &[Class]) -> Check {"),
            (1032, "/// the others reports a statement the ontology actually made being contradicted: a property"),
            (1383, "pub fn property_type("),
            (1098, "    for n in nodes {"),
            (1443, "pub fn unlicensed_edge(nodes: &[Node], classes: &[Class]) -> Check {"),
            (1164, "                    class.rel,"),
            (1508, "pub fn edge_target_class(nodes: &[Node], classes: &[Class]) -> Check {"),
            // RFC-0019's citation, the one carrying a quote.
            (1460, "            }"),
            (1461, "            let v = Violation::new("),
            (1462, "                &n.rel,"),
            (1495, "         its own never claimed to be complete. Only links landing on another instance are \\"),
            (1496, "         read: a link to the class file or into the catalog is a citation, not a \\"),
            (1497, "         relationship. A class that declares no `edges:` has said nothing and is not \\"),
        ];
        let mut lines = vec!["    // a line of some other check entirely".to_string(); 1600];
        for (n, text) in real {
            lines[n - 1] = (*text).to_string();
        }
        lines.join("\n") + "\n"
    }

    /// RFC-0018's sentence and RFC-0019's, with the line numbers each side of 5a0249e
    /// writes. `at` picks the tuple element that supplies the fragment.
    fn the_seven_doc(before: bool) -> String {
        let pick = |i: usize| match before {
            true => REPOINTED[i].1,
            false => REPOINTED[i].2,
        };
        let mut s = String::from(
            "`.ont.yml` now declares, and lint now enforces: the class an instance belongs to\n",
        );
        for (i, (label, _, _)) in REPOINTED.iter().enumerate() {
            s.push_str(&format!(
                "([`{label}`](../checks.rs#L{}), Error), and\n",
                pick(i)
            ));
        }
        let (a, b) = match before {
            true => (1460, 1462),
            false => (1495, 1497),
        };
        s.push_str(&format!(
            "\n`unlicensed-edge`'s own rationale draws this line already \
             ([`checks.rs:{a}-{b}`](../checks.rs#L{a}-L{b})): *a link to the class file or \
             into the catalog is a citation, not a relationship.*\n"
        ));
        s
    }

    /// #632's proof obligation. Seven citations had already slid when a person found them
    /// by hand (#627); **five were invisible to every gate**, and those five are what this
    /// check exists for. The other two are the shapes the gate already had: one range went
    /// blank, and one carried a quote.
    #[test]
    fn the_seven_citations_627_repointed_by_hand_are_all_reported() {
        let (_tmp, c) = cite_named(
            "checks.rs",
            &checks_rs_at_the_repair(),
            &the_seven_doc(true),
        );
        assert_eq!(c.len(), 7, "the fixture must hold all seven");

        let label = citation_label_not_cited(&c);
        assert_eq!(
            label.violations.len(),
            5,
            "the five nothing could see:\n{:?}",
            label.violations
        );
        for (name, _, _) in REPOINTED.iter().filter(|(_, old, _)| *old != 881) {
            assert!(
                label
                    .violations
                    .iter()
                    .any(|v| v.detail.contains(&format!("`{name}`"))),
                "{name} is not among the findings: {:?}",
                label.violations
            );
        }
        assert_eq!(
            dead_line_citation(&c).violations.len(),
            1,
            "`missing-property` cited L881, which the repair above it had left blank"
        );
        assert_eq!(
            slid_line_citation(&c).violations.len(),
            1,
            "RFC-0019's citation quoted its passage, so the gate already had it"
        );
        // Seven citations, seven findings, no citation counted twice.
        assert!(unverified_line_citation(&c).passed());
    }

    /// The same seven, repointed as #627 repointed them, against the same file: silent for
    /// the right reason. Without this the test above passes on a check that reports
    /// everything.
    #[test]
    fn the_seven_go_quiet_once_repointed() {
        let (_tmp, c) = cite_named(
            "checks.rs",
            &checks_rs_at_the_repair(),
            &the_seven_doc(false),
        );
        assert_eq!(c.len(), 7);
        for check in [
            citation_label_not_cited(&c),
            dead_line_citation(&c),
            slid_line_citation(&c),
            citation_range_stated_twice(&c),
        ] {
            assert!(check.passed(), "{}: {:?}", check.id, check.violations);
        }
    }
}
