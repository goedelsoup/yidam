### `rename catalog/<entry>` rewrites `target:` edges into the entry

**`yidam rename catalog/<entry>` now rewrites an edge whose `target:` is the entry (#1692).**

Before, such an edge was listed as a prose reference left unrewritten.
It then pointed at nothing, and `yidam lint` failed with `dangling-edge`.
The `target:` is now rewritten from the file that holds it and counted with the other citations.
A `target:` that names anything else is left alone.
