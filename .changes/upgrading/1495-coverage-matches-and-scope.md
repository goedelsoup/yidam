### A coverage universe can be narrowed, and its class scoped

**`[[due.coverage]]` takes two optional keys (#1495).** A table without them reads as before.

**`matches` filters a mixed list.** Point the clock at the published list itself, not at a copy
your fetcher filtered. Identifiers the pattern rejects are outside the universe. The detail
counts them:

```toml
matches = '^(HB|SB|HJR|SJR)\b'
```

**`scope` handles identifiers that repeat across periods.** Only an instance carrying the edge
covers a member. Declare one table per period:

```toml
scope = { relationship = "introduced-in", target = "general-assembly/136th-general-assembly.yml" }
```

If you added a property unique across periods only to suit the clock, `scope` replaces it.
