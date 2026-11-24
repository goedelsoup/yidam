### A pin names its release, and `due` counts the releases since

**`.yidam.toml` gains a sixth field, `release` (#1511, RFC-0065).** It names the `cli/vX.Y.Z`
release the pin installs. `cli` still names the binary that wrote the file, which is often not
that one. Your next `mise run yidam-vendor-update` writes the field, and nothing else is owed.

**`template` reads differently after that re-vendor.** It names the nearest template tag and the
commits past it, as `v0.13.0+6`. It read `untagged` whenever the pin was not on a tag.

**`yidam due` has an eighth clock, `template`, and it reads the network.** It makes one bounded
`git ls-remote` against the pin's `origin` and counts newer releases. An origin it cannot reach
reads `unmeasurable`, so no exit code changes. Declare an interval to make it come due:

```toml
[due]
template_after = 1
```

A repository with no `.yidam.toml` reports no `template` row. A consumer that counted seven
clocks now sees eight wherever there is a pin.
