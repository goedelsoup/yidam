### `check-diff` stops offering a candidate on a repeated suffix

**Some words are no longer compared when `check-diff` looks for a near-miss (#1298).**
Such a word ends three or more of the report's type names and appears nowhere else in them.
Nine error types are no longer each offered `margin-of-error` because they share `error`.
The finding rows are unchanged; only the `nearest` lead goes.
