//! A text reading of a PDF artifact — the bytes a quotation of one is compared with (#1172).
//!
//! # Stored, not computed
//!
//! A quotation is checked verbatim, and a check is only verbatim over bytes whose digest git
//! records. Text extracted on every lint would be the output of whichever extractor this
//! binary linked, and a crate bump would change a finding with nothing in the corpus having
//! changed. So a reading is extracted once, filed in the vault cache under its own digest like
//! any artifact, and recorded beside the PDF it was read from:
//!
//! ```yaml
//! artifacts:
//!   - sha256: 2a985bfe…
//!     media_type: application/pdf
//!     text:
//!       sha256: 91c3e0d4…
//!       extractor: pdf-extract 0.12.1
//! ```
//!
//! Lint then reads text, as it does for any text artifact, and needs no PDF code at all. The
//! extractor is named so a person can reproduce the reading; it is never re-run to check one.
//!
//! # Why nested and not a second record
//!
//! A second record in `artifacts:` would be a second revision to `quotations`, and every
//! quotation of a one-PDF entry would suddenly need a `sha256:` pin. It would also be one to
//! every binary that predates this, which would report those quotations unresolved. A reading
//! is not a revision of the document: the pin still names the PDF, and the reading follows it.
//!
//! # Which extractor, measured
//!
//! Against 300 PDFs from a vault cache, `pdf-extract` 0.12.1 and `lopdf` 0.45 both read all
//! 300 without an error, and both gave byte-identical output across two processes. Scored
//! against `pdftotext` — 8-word spans drawn from its output, over the 68 of 100 with a text
//! layer — `pdf-extract` held 78% and `lopdf` 48%. The misses are reading order in tables and
//! line-end hyphens, not glyph mapping: the two agree on every non-ASCII character counted.
//!
//! Three of those 68 read as empty. A reading with no letters in it is refused rather than
//! recorded ([`extract`]), because compared against, it would report every quotation as drift.
//!
//! # Line-end hyphens
//!
//! Extracted text keeps the hyphen a line broke at, so a reading says `Hardin- Madison` where
//! the page shows `Hardin-Madison` over a line break, and `veto- ing` where it shows `veto-ing`.
//! Which one was meant is not in the bytes. So at a hyphen that ends a line, [`Reading::holds`]
//! accepts a span with the hyphen, without it, or as extracted. Everywhere else a hyphen is
//! compared like any other character.

/// The extractor a reading written by this build names. Equal to the exact `pdf-extract` pin
/// in `Cargo.toml`, which `the_extractor_named_is_the_one_pinned` holds.
pub const EXTRACTOR: &str = "pdf-extract 0.12.1";

/// Whether a media type is one this build can take a reading of.
pub fn is_pdf(media: &str) -> bool {
    media
        .split(';')
        .next()
        .unwrap_or_default()
        .trim()
        .eq_ignore_ascii_case("application/pdf")
}

/// Stands in for a line-end hyphen in the flattened reading. The reading is text, and a NUL
/// in it is replaced by a space before this is placed, so it marks nothing but a break.
const BREAK: u8 = 0;

/// A reading, prepared to be searched.
pub struct Reading {
    /// Whitespace-flattened, a line-end hyphen and the space after it replaced by [`BREAK`].
    marked: Vec<u8>,
    /// The same text with every [`BREAK`] as the reading has it, `- `, for the fast path.
    plain: String,
    /// Every hyphen and space removed, for a test that fails only where an exact one would.
    bare: String,
}

impl Reading {
    pub fn new(text: &str) -> Self {
        let text = text.replace('\0', " ");
        let mut marked = String::with_capacity(text.len());
        let mut words = text.split_whitespace().peekable();
        let mut rest = text.as_str();
        while let Some(w) = words.next() {
            let at = rest.find(w).unwrap_or(0);
            rest = &rest[at + w.len()..];
            marked.push_str(w);
            if words.peek().is_none() {
                break;
            }
            let gap = &rest[..rest
                .find(|c: char| !c.is_whitespace())
                .unwrap_or(rest.len())];
            let broke = w.len() > 1
                && w.ends_with('-')
                && w[..w.len() - 1]
                    .chars()
                    .next_back()
                    .is_some_and(char::is_alphanumeric)
                && gap.contains('\n');
            if broke {
                marked.pop();
                marked.push(BREAK as char);
            } else {
                marked.push(' ');
            }
        }
        let plain = marked.replace(BREAK as char, "- ");
        let bare = strip(&marked.replace(BREAK as char, ""));
        Self {
            marked: marked.into_bytes(),
            plain,
            bare,
        }
    }

    /// Whether the span is in the reading, whitespace flattened on both sides and a line-end
    /// hyphen read three ways.
    pub fn holds(&self, span: &str) -> bool {
        let span = span.split_whitespace().collect::<Vec<_>>().join(" ");
        if span.is_empty() || self.plain.contains(&span) {
            return true;
        }
        // Every alignment `continues` accepts differs from the span only in hyphens and spaces,
        // so a span absent from `bare` is absent — the answer for nearly every drift.
        if !self.bare.contains(&strip(&span)) {
            return false;
        }
        let s = span.as_bytes();
        (0..self.marked.len()).any(|i| continues(&self.marked, i, s, 0))
    }
}

fn strip(s: &str) -> String {
    s.chars().filter(|&c| c != '-' && c != ' ').collect()
}

/// Whether `s[j..]` is read at `h[i..]`, a [`BREAK`] matching `-`, `- `, or nothing.
fn continues(h: &[u8], mut i: usize, s: &[u8], mut j: usize) -> bool {
    loop {
        if j == s.len() {
            return true;
        }
        let Some(&c) = h.get(i) else { return false };
        if c == BREAK {
            if j > 0 && continues(h, i + 1, s, j) {
                return true;
            }
            if s[j] == b'-' {
                if continues(h, i + 1, s, j + 1) {
                    return true;
                }
                if s.get(j + 1) == Some(&b' ') && continues(h, i + 1, s, j + 2) {
                    return true;
                }
            }
            return false;
        }
        if c != s[j] {
            return false;
        }
        i += 1;
        j += 1;
    }
}

/// Take a reading of a PDF, or say why there is none.
///
/// `pdf-extract` panics on some malformed fonts rather than returning an error, and a panic in
/// one entry's reading must not end a run over a catalog, so it is caught and reported.
#[cfg(feature = "pdf-text")]
pub fn extract(bytes: &[u8]) -> Result<String, String> {
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let read = std::panic::catch_unwind(|| pdf_extract::extract_text_from_mem(bytes));
    std::panic::set_hook(hook);
    let text = match read {
        Ok(Ok(text)) => text,
        Ok(Err(e)) => return Err(format!("{EXTRACTOR} could not read it: {e}")),
        Err(_) => return Err(format!("{EXTRACTOR} failed partway through reading it")),
    };
    refuse_empty(text)
}

/// A build without the `pdf-text` feature links no extractor, and says so rather than
/// recording nothing. Lint needs none: it compares with readings already taken.
#[cfg(not(feature = "pdf-text"))]
pub fn extract(_: &[u8]) -> Result<String, String> {
    Err(
        "this build of yidam links no PDF extractor — it was built without the `pdf-text` \
         feature, which the default build has"
            .to_string(),
    )
}

/// A reading with no letter or digit in it, which is what a scanned page gives.
///
/// Separate from [`extract`] so it is tested in every build — which is why, in a build without
/// `pdf-text`, it is compiled with nothing to call it.
#[cfg_attr(not(feature = "pdf-text"), allow(dead_code))]
pub(crate) fn refuse_empty(text: String) -> Result<String, String> {
    if text.chars().any(char::is_alphanumeric) {
        Ok(text)
    } else {
        Err(format!(
            "{EXTRACTOR} found no text in it — a scanned page has none to read, and a reading \
             of nothing would report every quotation of it as drift"
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PAGE: &str =
        "Plan for the Allen-Champaign-Hardin-\nMadison-Shelby-Union Joint Solid Waste\n\
                        Management District. The veto-\ning of this item is in the public inter-\n\
                        est, and the range is 3 - 4 days.";

    #[test]
    fn a_line_end_hyphen_is_read_with_it_without_it_or_as_extracted() {
        let r = Reading::new(PAGE);
        for span in [
            "Allen-Champaign-Hardin-Madison-Shelby-Union",
            "Allen-Champaign-HardinMadison-Shelby-Union",
            "Allen-Champaign-Hardin- Madison-Shelby-Union",
            "The vetoing of this item",
            "in the public interest",
            "Waste Management District",
        ] {
            assert!(r.holds(span), "{span:?}");
        }
    }

    #[test]
    fn a_hyphen_mid_line_is_compared_like_any_character() {
        let r = Reading::new(PAGE);
        for span in [
            "AllenChampaign",
            "Allen Champaign",
            "the range is 3 4 days",
            "the range is 3- 4 days",
            "in the public good",
        ] {
            assert!(!r.holds(span), "{span:?}");
        }
        assert!(r.holds("the range is 3 - 4 days"));
    }

    /// A dash standing alone at a line end is not a hyphen the line broke at: only a letter or
    /// digit before it makes a break, so `3 -\n4` keeps its dash.
    #[test]
    fn a_dash_standing_alone_at_a_line_end_is_not_a_break() {
        let r = Reading::new("from 3 -\n4 days");
        assert!(r.holds("from 3 - 4 days"));
        assert!(!r.holds("from 3 4 days"));
    }

    #[test]
    fn a_span_may_start_or_end_either_side_of_a_break() {
        let r = Reading::new("the inter-\nest rate");
        assert!(r.holds("est rate"));
        assert!(r.holds("interest rate"));
        assert!(r.holds("the inter-"));
        assert!(r.holds("the inter"));
    }

    #[test]
    fn a_nul_in_a_reading_is_a_space_and_not_a_break() {
        let r = Reading::new("public\0interest");
        assert!(r.holds("public interest"));
        assert!(!r.holds("publicinterest"));
    }

    #[test]
    fn a_reading_with_no_letters_is_refused() {
        assert!(refuse_empty("\n\n \u{c}\n".to_string()).is_err());
        assert!(refuse_empty("  .. -- \n".to_string()).is_err());
        assert!(refuse_empty("Item 12".to_string()).is_ok());
    }

    #[test]
    fn only_a_pdf_is_read() {
        assert!(is_pdf("application/pdf"));
        assert!(is_pdf("Application/PDF; charset=binary"));
        assert!(!is_pdf("text/plain"));
        assert!(!is_pdf("application/pdfx"));
    }

    /// A reading names the extractor that produced it, and a name that drifted from the pin
    /// would record a version nobody can reproduce with.
    #[test]
    fn the_extractor_named_is_the_one_pinned() {
        let manifest = include_str!("../Cargo.toml");
        let pin = manifest
            .lines()
            .find(|l| l.starts_with("pdf-extract ="))
            .expect("Cargo.toml declares pdf-extract");
        let version = EXTRACTOR.strip_prefix("pdf-extract ").unwrap();
        assert!(
            pin.contains(&format!("version = \"={version}\"")),
            "EXTRACTOR says {EXTRACTOR:?} and Cargo.toml pins {pin:?}"
        );
    }

    #[cfg(feature = "pdf-text")]
    #[test]
    fn something_that_is_not_a_pdf_is_an_error_and_not_a_panic() {
        let why = extract(b"%PDF-1.7 and then nothing").unwrap_err();
        assert!(why.contains(EXTRACTOR), "{why}");
    }
}
