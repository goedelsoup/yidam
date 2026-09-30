### A re-vendor now reaches `ci.yml` and `CLAUDE.md`

**Scaffold regions (#1054).** Genesis installs `.github/workflows/ci.yml` and `.claude/CLAUDE.md`
once. Until now no re-vendor touched them again. So a gate added upstream never reached your CI.

The scaffold now marks the part of each file that is yidam's. In `ci.yml` that is the `privacy`
and `corpus` jobs, between `# <!-- YIDAM:CI -->` and `# <!-- /YIDAM:CI -->`. In `CLAUDE.md` it is
the template's sections, between `<!-- YIDAM:CLAUDE -->` and `<!-- /YIDAM:CLAUDE -->`.
`mise run yidam-vendor-update` rewrites each region from the scaffold at the new pin. It never
reads the rest of the file.

An existing repository has no markers. Install them once, then re-vendor:

```sh
yidam migrate --dry-run scaffold   # see what each region takes in
yidam migrate scaffold             # do it
mise run yidam-vendor-update       # fill the regions
```

Commit the migration as a `migrate:` commit. **Anything inside a region is replaced.** A step you
added to the `privacy` or `corpus` job shows in the re-vendor's diff as removed. Move it to a job
of your own, outside the markers.

A workflow with neither job gets an empty region, and the re-vendor fills it. A `CLAUDE.md` with
none of the template's headings is yours, and is left unmarked. The migration refuses when one of
your sections sits between two of the template's. Move it, then run it again.

`yidam doctor` has two new checks. `scaffold` warns while a file has no region. `ci` warns when no
workflow runs a corpus gate. It reads `.github/workflows/` only, and follows a `mise run` into
your own `mise.toml`.
