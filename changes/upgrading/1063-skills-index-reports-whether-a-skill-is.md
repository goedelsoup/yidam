### `skills-index` reports whether a skill is built

**A skill's frontmatter may say `status: built` or `status: stub` (#1063).** `yidam
skills-index` adds a Status column and a count line above the table. A skill that says neither
is reported as `unstated`, never as built. The bootstrap now writes `status: stub` into each
calculator stub it leaves.

After this upgrade, `yidam regen --check` reports `.yidam/skills/README.md` as stale. Run
`yidam skills-index` and commit the table. Every existing skill reads `unstated` until you add
the field. Nothing gates on the value.

A built skill that another derivation also wrote can now be sent upstream. `upstream.md` names
the `from:derived-skill` label and its template.
