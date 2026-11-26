### The kuten profile gives the right reason for an empty `thresholds`

**`KUTEN.md` no longer says escalation arrives as a policy override (#1361).** `escalate_after` is
this corpus's own `[lint]` setting in `config.toml`. A kuten still proposes no threshold. Only
the stated reason changed, so a re-vendor shows a three-line diff and changes no behaviour.
