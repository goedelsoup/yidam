### `check-diff` offers fewer nearest names

**A `nearest` candidate now needs a shared root of five characters, up from four (#390).**
Across fourteen derived corpora, more than half of the four-character candidates were wrong.
Most shared a truncation such as `stat`, offering `status` for `statement`.

**What changes for you: fewer candidates, and no findings.** Every `unmodelled-concept` row is
still reported. A few right candidates on four-letter words are gone, such as `vote`.
