### A re-vendor names the workarounds it makes unnecessary

**Cited issues (#1054).** Record a workaround with the upstream issue it works around, as
`yidam#587` or as the issue's URL. After a re-vendor, `yidam-vendor-update` lists each cited issue that a commit at
the new pin fixes. The workaround for it can go. `git grep yidam#587` finds it.

It reads tracked files only, and skips `.yidam/.vendor/` and `.yidam/corpus/`. It needs no
network beyond the clone the re-vendor already makes.
