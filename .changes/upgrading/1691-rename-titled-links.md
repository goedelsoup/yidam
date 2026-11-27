### `rename catalog/<entry>` rewrites a titled or angle-bracket link in place

**`yidam rename catalog/<entry>` now rewrites a markdown link that carries a title or angle brackets (#1691).**

Before, `[x](entry.md "title")` and `[x](<entry.md>)` were planned but could not be placed on their line.
Since #1614 that refused the whole rename, so one such link blocked it until you edited the link by hand.
The rename now reads each link the way lint does, and keeps the title or the brackets.
A link inside an inline code span is still left alone, as lint leaves it alone.
