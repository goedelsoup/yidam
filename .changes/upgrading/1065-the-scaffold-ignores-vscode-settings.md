### The scaffolded `.gitignore` ignores `.vscode/settings.json`

**The yidam extension writes this file itself (#1065).** At activation its vendor guard marks
`.yidam/.vendor/**` read-only. Its schema wiring adds the `yidam schema --settings` mapping. Both
land in `.vscode/settings.json`. A `git add -A` then commits the file. Three derived repositories
did.

*Why that matters.* Other extensions write machine paths into the same file, such as a Python
interpreter under your home directory. A tracked copy publishes the next one.

*What to do.* A new repository gets the rule at genesis. It matches at any depth. An existing
repository can copy it from `sadhana/root/gitignore`. If the file is already committed,
`git rm --cached .vscode/settings.json` untracks it. The extension re-applies the guard in every
window, so no protection is lost. To keep sharing the file on purpose, add
`!.vscode/settings.json` after the rule.
