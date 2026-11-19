### A pinned read comes due when its code changes

**`due`'s catalog clock measures a pinned `file` location by change, not by age (#1522).**

A pinned read is due when its file at the reference revision is a different object.
The reference is `HEAD` for this repository and the tonpa `rev` for a dependency.
`ttl_days` no longer applies to an entry whose every location is pinned.
Such an entry stops expiring in `due`, `lint` and `status`.

The `superseded` clock now treats a move of `commit:` as a new edition only if the file changed.
Re-dating a read of unchanged code asks nothing of the nodes that cite it.
