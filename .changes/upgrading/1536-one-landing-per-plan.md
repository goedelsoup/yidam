### A plan lands once, as one commit per verb

**`yidam run` moves the branch once, after every operational step has run (#1536).**

Consecutive steps with one verb land as one commit, such as `compute: 8 steps — 23 files at <parent>`.
A tool that finds a step's commit by `: <step> ` in the subject now finds only a group of one.
The commit body names every step it carries.

A plan in which any operational step fails lands nothing.
Re-run it after the fix, and a calculator lands what it computed before.
A step that declares `ageing_days` runs again, and may give a different answer.
A branch that moves during a plan discards every step the plan had finished.

See [RFC-0067](rfcs/0067-one-landing-per-plan.md).
