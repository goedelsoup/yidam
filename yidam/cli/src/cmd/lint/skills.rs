//! A domain skill that does not say whether it is built (#1182).
//!
//! #1063 gave a skill's frontmatter `status: built | stub`, and `skills-index` reports a skill
//! that says neither as `unstated`. That index was the only reader, so the only prompt to state
//! the field was a column in a regenerated README — and every skill written before the field
//! existed reads `unstated` after the upgrade. This is the second reader, and it reads the value
//! through [`SkillStatus::of`], the function the index reads it through, so the two cannot
//! disagree about what counts as stated.

use crate::cmd::registry::SkillStatus;

use super::{Check, Severity, Violation};

pub const SKILL_STATUS_UNSTATED: &str = "skill-status-unstated";

/// Every skill in `.yidam/skills/` whose `status:` is absent, or is neither `built` nor `stub`.
///
/// `skills` is `(repo-relative path, text)`, one per file `skills-index` lists.
pub fn skill_status_unstated(skills: &[(String, String)]) -> Check {
    let violations = skills
        .iter()
        .filter_map(|(rel, text)| {
            let fm = crate::parse::parse_frontmatter(text);
            let detail = match SkillStatus::of(fm.status.as_ref()) {
                SkillStatus::Built | SkillStatus::Stub => return None,
                SkillStatus::Unstated => "says no `status:` — `built` if an agent can follow it, \
                     `stub` if it only names a procedure. `skills-index` counts it as \
                     `unstated` until it says"
                    .to_string(),
                SkillStatus::Other(v) => format!(
                    "says `status: {v}`, which is neither `built` nor `stub` — `skills-index` \
                     lists it as written and counts it as neither"
                ),
            };
            Some(Violation::new(rel, detail))
        })
        .collect();
    Check::new(
        SKILL_STATUS_UNSTATED,
        "A domain skill that does not say whether it is built",
        Severity::Info,
        "A skill's frontmatter says `status: built` when an agent can follow it and `status: stub` \
         when it only names a procedure nobody has written. `yidam skills-index` counts the \
         three answers so the registry does not read as more capability than it holds, and it \
         counts a skill that says neither as `unstated` — never as built, because reading \
         silence as capability is the flattering direction. Info, not a gate: an unstated skill \
         is not wrong, only unreported, and every skill written before the field existed is \
         one. A value that is neither word is reported here too, since the index can only \
         print it back.",
        violations,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(skills: &[(&str, &str)]) -> Check {
        let owned: Vec<_> = skills
            .iter()
            .map(|(rel, text)| (rel.to_string(), text.to_string()))
            .collect();
        skill_status_unstated(&owned)
    }

    fn reported(check: &Check) -> Vec<&str> {
        check.violations.iter().map(|v| v.node.as_str()).collect()
    }

    /// `built` and `stub` pass; absent, no header at all, and a third word are reported.
    #[test]
    fn a_skill_that_says_neither_word_is_reported() {
        let check = run(&[
            (".yidam/skills/a.md", "---\nname: a\nstatus: built\n---\n"),
            (".yidam/skills/b.md", "---\nname: b\nstatus: stub\n---\n"),
            (".yidam/skills/c.md", "---\nname: c\n---\n"),
            (".yidam/skills/d.md", "# prose only\n"),
            (".yidam/skills/e.md", "---\nname: e\nstatus: done\n---\n"),
        ]);
        assert_eq!(
            reported(&check),
            [
                ".yidam/skills/c.md",
                ".yidam/skills/d.md",
                ".yidam/skills/e.md"
            ]
        );
        assert!(check.violations[2].detail.contains("`status: done`"));
    }

    /// Reported, never gated: every skill predating #1063 is in this population.
    #[test]
    fn the_check_reports_at_info() {
        let check = run(&[(".yidam/skills/a.md", "---\nname: a\n---\n")]);
        assert_eq!(check.severity, Severity::Info);
        assert_eq!(check.violations.len(), 1);
    }

    /// A non-string value is not stated either, and is printed as the index prints it.
    #[test]
    fn a_non_string_status_is_reported_as_written() {
        let check = run(&[(".yidam/skills/a.md", "---\nstatus: true\n---\n")]);
        assert!(
            check.violations[0].detail.contains("`status: true`"),
            "{check:?}"
        );
    }
}
