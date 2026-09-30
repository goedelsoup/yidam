### An interval's target may seat more than one

**`interval:` accepts `capacity:`, naming a property on the `exclusive_over` target (#1205).**
Write `capacity: seats` on a tenure class. Then an office with `seats: 3` may have three
holders at once. A target that omits the property holds one. Quoted digits such as `"3"` count.
A value that is not a whole number of at least one is reported on the target.

`interval-overlap` now reports each holder once, naming every earlier holder it overlaps. It
used to report once for each overlapping pair.
