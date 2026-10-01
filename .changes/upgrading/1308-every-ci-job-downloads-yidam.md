### Every scaffolded CI job downloads yidam before compiling it

**The privacy job and both release jobs now download a released binary first (#1308).**
Before, they cloned yidam and compiled it at every pin, released or not.
Only the `corpus` job tried the download.

**What changes for you depends on the file.**
The privacy job sits in `ci.yml`'s `YIDAM:CI` region, so your next re-vendor brings it.
`release.yml` is yours, so take the new `.github/workflows/release.yml` on upgrade.
Its `guard` and `bundle` jobs then skip the toolchain whenever your pin has a release.

`index.yml` still compiles, because no release carries `--features index`.
