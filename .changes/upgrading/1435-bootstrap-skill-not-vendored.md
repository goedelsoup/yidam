### The bootstrap skill is no longer vendored

**A re-vendor now removes `.yidam/.vendor/prelude/skills/` (#1435).** The bootstrap skill made your repository and never runs in it again.

**Delete the bootstrap row from `.yidam/skills/README.md` by hand.** It sits under *Inherited from prelude*, and its link now goes nowhere.
A re-vendor never rewrites that file, so nothing else will remove it.
Delete the whole table if that row was its only entry.
