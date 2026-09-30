### A calculator the corpus cannot run yet is declared, not stubbed as a skill

**A capability in `.yidam/capabilities.toml` may omit `run` (#1184).** It parses as declared and not built.
Every `yidam run` plan holding it is refused by name, and nothing lands.

The bootstrap now declares an unrun calculator there instead of writing a skill stub in `.yidam/skills/`.
An existing calculator stub can move the same way. Declare its `reads`, `writes` and `verb`, then delete the skill.
Until it gains a `run`, a bare `yidam run` refuses the whole manifest.
