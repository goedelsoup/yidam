### A catalog entry saved with CRLF line endings is read

**Frontmatter written with CRLF line endings is now read, so lint may report findings it missed (#1618).**

Before, such an entry read as having no header, and every check of its fields was silent.
Expect new findings on those entries. The catalog audit now fills the columns it showed as blank.
A column-zero line that only starts with `---`, such as `----`, no longer closes the header.
A header with nothing between its two `---` lines now reads as an empty header.
