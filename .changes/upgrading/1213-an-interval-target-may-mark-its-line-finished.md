### An interval's target may mark its line of holders finished

**`interval:` accepts `complete:`, naming a property on the `exclusive_over` target (#1213).**
Write `complete: line_complete` on a tenure class, and `line_complete: true` on each finished office.

The new `interval-gap` check then warns on a span inside that line held by fewer than it seats.
Spans before the first holder and after the last are not gaps. Targets without the mark are unchecked.
