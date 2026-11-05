### Sessions now open with the loop's brief

New derivations ship `.claude/settings.json` with two Claude Code hooks.
The SessionStart hook prints `yidam cycle --brief` into the session.
The Stop hook records the round and tells you what each turn moved.

Run `yidam migrate scaffold` to add the hooks to an existing repository.
It creates `.claude/settings.json`, or adds the hooks to yours.
It refuses a settings file that already declares `hooks`.
Add the two entries by hand there, as `sadhana/root/claude-settings.json` does.

`yidam doctor` warns when a repository with a `CLAUDE.md` has no hooks.
The hooks run the binary in `.yidam/bin`, so pin a release that has `yidam hook`.
An older binary makes the hooks silent, never a failure.
