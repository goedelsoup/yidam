use anyhow::Result;
use std::path::Path;

use crate::parse::parse_frontmatter;
use crate::paths::yidam_skills_dir;
use crate::regen::update_file_regen;
use crate::walk::walk_md_files;

/// What a skill's `status:` says it is (#1063).
///
/// Seven derived repositories held 45 local skills, most of them stubs, and an index that
/// listed name and description could not say which: it read as 45 capabilities. **Absent is
/// `Unstated`, not `Built`.** Every skill written before the field existed is absent, and
/// reading them as built is the flattering direction — the one `obtained` took, and #1056 is
/// what it cost.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum SkillStatus {
    Built,
    Stub,
    Unstated,
    /// A value that is neither word, reported as written so the typo is visible in the index.
    Other(String),
}

impl SkillStatus {
    /// Read by `skills-index` and by `lint`'s `skill-status-unstated` (#1182), so the two agree
    /// on what counts as stated.
    pub(crate) fn of(value: Option<&serde_yaml::Value>) -> Self {
        use serde_yaml::Value;
        match value {
            None | Some(Value::Null) => Self::Unstated,
            Some(Value::String(s)) if s == "built" => Self::Built,
            Some(Value::String(s)) if s == "stub" => Self::Stub,
            Some(Value::String(s)) => Self::Other(s.clone()),
            Some(other) => Self::Other(
                serde_yaml::to_string(other)
                    .unwrap_or_default()
                    .trim()
                    .replace('\n', " "),
            ),
        }
    }

    fn cell(&self) -> &str {
        match self {
            Self::Built => "built",
            Self::Stub => "stub",
            Self::Unstated => "unstated",
            Self::Other(s) => s,
        }
    }
}

pub(crate) fn render_skills_index(skills_dir: &Path) -> String {
    let skills = walk_md_files(skills_dir);
    if skills.is_empty() {
        return "_No domain-specific skills yet._".to_string();
    }
    let mut rows = vec![
        "| Skill | Status | Description |".to_string(),
        "|---|---|---|".to_string(),
    ];
    let mut statuses = Vec::with_capacity(skills.len());
    for path in &skills {
        let text = std::fs::read_to_string(path).unwrap_or_default();
        let fm = parse_frontmatter(&text);
        let name = fm.name.unwrap_or_else(|| {
            path.file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string()
        });
        let status = SkillStatus::of(fm.status.as_ref());
        let desc = fm.description.unwrap_or_else(|| "—".to_string());
        let filename = path.file_name().unwrap_or_default().to_string_lossy();
        rows.push(format!(
            "| [{name}]({filename}) | {} | {desc} |",
            status.cell()
        ));
        statuses.push(status);
    }
    format!("{}\n\n{}", status_summary(&statuses), rows.join("\n"))
}

/// The line above the table: how many of the listed skills an agent can actually follow.
///
/// A count and not only the column, because the column is what a reader scanning 45 rows
/// does not add up — and the total is the number the index used to leave standing alone.
fn status_summary(statuses: &[SkillStatus]) -> String {
    let count = |want: &SkillStatus| statuses.iter().filter(|s| *s == want).count();
    let other = statuses
        .iter()
        .filter(|s| matches!(s, SkillStatus::Other(_)))
        .count();
    let total = statuses.len();
    let noun = if total == 1 { "skill" } else { "skills" };
    let mut parts = vec![
        format!("{} built", count(&SkillStatus::Built)),
        format!("{} stub", count(&SkillStatus::Stub)),
        format!("{} unstated", count(&SkillStatus::Unstated)),
    ];
    if other > 0 {
        parts.push(format!("{other} neither `built` nor `stub`"));
    }
    format!("{total} {noun}: {}.", parts.join(", "))
}

pub fn agents_index(root: Option<&std::path::Path>) -> Result<()> {
    let root = crate::paths::resolve_root(root)?;
    let agents_dir = root.join("agents");
    let agents = walk_md_files(&agents_dir);

    let content = if agents.is_empty() {
        "_No domain-specific agents yet._".to_string()
    } else {
        let mut rows = vec![
            "| Agent | Description |".to_string(),
            "|---|---|".to_string(),
        ];
        for path in &agents {
            let text = std::fs::read_to_string(path).unwrap_or_default();
            let fm = parse_frontmatter(&text);
            let name = fm.name.unwrap_or_else(|| {
                path.file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string()
            });
            let desc = fm.description.unwrap_or_else(|| "—".to_string());
            let filename = path.file_name().unwrap_or_default().to_string_lossy();
            rows.push(format!("| [{name}]({filename}) | {desc} |"));
        }
        rows.join("\n")
    };

    crate::regen::emit(&content);
    update_file_regen(
        &agents_dir.join("README.md"),
        "yidam agents-index",
        &content,
    )
}

pub fn skills_index(root: Option<&std::path::Path>) -> Result<()> {
    let root = crate::paths::resolve_root(root)?;
    let skills_dir = yidam_skills_dir(&root);
    let content = render_skills_index(&skills_dir);
    crate::regen::emit(&content);
    update_file_regen(
        &skills_dir.join("README.md"),
        "yidam skills-index",
        &content,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn index(skills: &[(&str, &str)]) -> String {
        let dir = tempfile::tempdir().unwrap();
        for (file, text) in skills {
            std::fs::write(dir.path().join(file), text).unwrap();
        }
        render_skills_index(dir.path())
    }

    /// The four answers, each in its own row, and the count above them adding up to the rows.
    #[test]
    fn the_index_reports_each_skill_status_and_counts_them() {
        let out = index(&[
            ("a.md", "---\nname: a\nstatus: built\n---\n"),
            ("b.md", "---\nname: b\nstatus: stub\n---\n"),
            ("c.md", "---\nname: c\n---\n"),
            ("d.md", "---\nname: d\nstatus: done\n---\n"),
        ]);
        assert!(out.contains("| [a](a.md) | built |"), "{out}");
        assert!(out.contains("| [b](b.md) | stub |"), "{out}");
        assert!(out.contains("| [c](c.md) | unstated |"), "{out}");
        assert!(out.contains("| [d](d.md) | done |"), "{out}");
        assert!(
            out.starts_with("4 skills: 1 built, 1 stub, 1 unstated, 1 neither `built` nor `stub`."),
            "{out}"
        );
    }

    /// The regression the field exists to prevent: a skill that never said is not counted
    /// as a capability. Absent, and a file with no frontmatter at all, are both `unstated`.
    #[test]
    fn a_skill_that_does_not_say_is_not_read_as_built() {
        let out = index(&[("a.md", "---\nname: a\n---\n"), ("b.md", "# prose only\n")]);
        assert!(
            out.starts_with("2 skills: 0 built, 0 stub, 2 unstated."),
            "{out}"
        );
    }

    /// A non-string `status:` must not cost the skill its name — which it would, typed as a
    /// `String`, because the whole header would fail to parse.
    #[test]
    fn a_non_string_status_keeps_the_rest_of_the_header() {
        let out = index(&[("a.md", "---\nname: kept\nstatus: true\n---\n")]);
        assert!(out.contains("| [kept](a.md) | true |"), "{out}");
    }

    #[test]
    fn an_empty_directory_says_so_and_counts_nothing() {
        assert_eq!(index(&[]), "_No domain-specific skills yet._");
    }
}
