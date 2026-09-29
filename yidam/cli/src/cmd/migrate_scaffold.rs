//! `yidam migrate scaffold` — the markers that let a re-vendor reach `ci.yml` and `CLAUDE.md`
//! (#1054).
//!
//! Genesis installs `.github/workflows/ci.yml` and `.claude/CLAUDE.md` from the scaffold, once.
//! `mise run yidam-vendor-update` rewrites the part of each that is yidam's, between a
//! `YIDAM:CI` or `YIDAM:CLAUDE` marker pair, and never reads the rest. A derivation made before
//! the markers existed has none, so a re-vendor reaches neither file, and a gate added upstream
//! never reaches its CI. Nine derived workflows were 225 to 715 lines behind the scaffold, one
//! of them behind the scaffold at its own pin. This installs the markers, once.
//!
//! # What goes inside the markers
//!
//! **Only what can be shown to be the template's.** In `ci.yml` that is the `privacy` and
//! `corpus` jobs, by name: they are the gate, and the jobs a domain adds or rewrites (`crates`,
//! `web`, its own) have other names. The two are moved together if something sits between them,
//! and job order means nothing to GitHub. A workflow with neither gets an **empty** region at
//! the end of `jobs:`, and the next re-vendor fills it — so a repository whose CI runs no gate
//! gets one from the pair of commands, without anybody writing YAML.
//!
//! In `CLAUDE.md` it is the run of sections from `## The short version` through the last of the
//! template's headings. An owner's own section *between* two of the template's refuses, because
//! wrapping it would hand it to the re-vendor. An owner's section after them stays outside.
//! A `CLAUDE.md` with none of the template's headings is the owner's whole, and is left alone:
//! two of seventeen derivations wrote their own, and an empty region there would put a second
//! summary into a file that already carries one.
//!
//! Wrapped content is written as it is. The re-vendor replaces it, and the diff it leaves is
//! where an owner sees a step they had added inside a gate job. The report says so up front.

use std::path::Path;

use super::migrate::MigrateReport;

/// The workflow, relative to the repository root.
pub(crate) const CI: &str = ".github/workflows/ci.yml";

/// The agent instructions, relative to the repository root.
pub(crate) const CLAUDE: &str = ".claude/CLAUDE.md";

/// The region tokens, as the scaffold and `yidam-vendor-update` spell them.
const CI_OPEN: &str = "<!-- YIDAM:CI -->";
const CI_CLOSE: &str = "<!-- /YIDAM:CI -->";
const CLAUDE_OPEN: &str = "<!-- YIDAM:CLAUDE -->";
const CLAUDE_CLOSE: &str = "<!-- /YIDAM:CLAUDE -->";

/// The jobs that are yidam's, in the order the scaffold writes them.
pub(crate) const GATE_JOBS: &[&str] = &["privacy", "corpus"];

/// The template's `CLAUDE.md` headings. The first is where the region starts.
const TEMPLATE_HEADINGS: &[&str] = &[
    "## The short version",
    "## Before writing",
    "## Before committing",
];

/// One file the migration reads, and what it does there.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ScaffoldFile {
    /// Repository-relative.
    pub file: String,
    /// The file already carries its region, so there is nothing to do.
    pub already: bool,
    /// The file does not exist, so there is nothing to mark.
    pub absent: bool,
    /// The file carries none of the template's content, so none of it is the re-vendor's.
    pub owned: bool,
    /// What goes inside the new region: job names, or headings. Empty for an empty region.
    pub wrapped: Vec<String>,
}

impl ScaffoldFile {
    /// Whether the migration writes this file.
    pub(crate) fn to_write(&self) -> bool {
        !(self.already || self.absent || self.owned)
    }
}

/// What the migration found in both files.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Scaffold {
    pub files: Vec<ScaffoldFile>,
}

impl Scaffold {
    /// Whether nothing would be written.
    pub(crate) fn nothing_to_do(&self) -> bool {
        self.files.iter().all(|f| !f.to_write())
    }
}

/// Whether a line is the marker `token`: after indentation and a YAML comment's `#`, it opens
/// with the token. The same rule `yidam-vendor-update`'s splice applies, so the two agree on
/// what a marker is.
fn marks(line: &str, token: &str) -> bool {
    line.trim_start_matches(|c: char| c.is_whitespace() || c == '#')
        .starts_with(token)
}

/// Whether `text` carries an opening and, after it, a closing marker.
fn has_region(text: &str, open: &str, close: &str) -> bool {
    let lines: Vec<&str> = text.lines().collect();
    lines
        .iter()
        .position(|l| marks(l, open))
        .is_some_and(|i| lines[i + 1..].iter().any(|l| marks(l, close)))
}

/// Whether `ci.yml`'s text carries the region a re-vendor rewrites.
pub(crate) fn ci_has_region(text: &str) -> bool {
    has_region(text, CI_OPEN, CI_CLOSE)
}

/// Whether `CLAUDE.md`'s text carries the region a re-vendor rewrites.
pub(crate) fn claude_has_region(text: &str) -> bool {
    has_region(text, CLAUDE_OPEN, CLAUDE_CLOSE)
}

/// Whether a `CLAUDE.md` carries any of the template's sections. One that does not is the
/// owner's, and neither this migration nor `doctor` asks it for a region.
pub(crate) fn claude_is_template(text: &str) -> bool {
    text.lines()
        .any(|l| TEMPLATE_HEADINGS.contains(&l.trim_end()))
}

/// A job under `jobs:`: its name, and the lines it spans, leading comments included.
#[derive(Debug, PartialEq, Eq)]
struct Job {
    name: String,
    start: usize,
    end: usize,
}

/// The jobs in a workflow, and the line one past the end of `jobs:`.
///
/// A job is a two-space key under a top-level `jobs:`. It owns the comment lines directly above
/// it, since that is where the scaffold explains each one, and runs to the next job's comments
/// or key. Blank lines between jobs belong to the job above.
fn jobs(lines: &[&str]) -> Result<(Vec<Job>, usize), String> {
    let Some(top) = lines.iter().position(|l| l.trim_end() == "jobs:") else {
        return Err(format!("{CI} has no top-level `jobs:`"));
    };
    let mut end = lines.len();
    let mut keys: Vec<(usize, String)> = vec![];
    for (i, line) in lines.iter().enumerate().skip(top + 1) {
        if !line.is_empty() && !line.starts_with(' ') && !line.starts_with('#') {
            end = i;
            break;
        }
        if let Some(rest) = line.strip_prefix("  ") {
            if rest.starts_with(|c: char| c.is_ascii_alphanumeric() || c == '_') {
                if let Some((name, _)) = rest.split_once(':') {
                    keys.push((i, name.trim().to_string()));
                }
            }
        }
    }
    // Leave trailing blank lines outside the last job, so an insertion lands after its content.
    while end > top + 1 && lines[end - 1].trim().is_empty() {
        end -= 1;
    }
    let mut out = vec![];
    for (k, (at, name)) in keys.iter().enumerate() {
        let mut start = *at;
        while start > top + 1 && lines[start - 1].trim_start().starts_with('#') {
            start -= 1;
        }
        out.push(Job {
            name: name.clone(),
            start,
            end: 0,
        });
        if k > 0 {
            out[k - 1].end = start;
        }
    }
    if let Some(last) = out.last_mut() {
        last.end = end;
    }
    Ok((out, end))
}

/// The workflow with its gate jobs inside the region, and which jobs went in.
fn wrap_ci(text: &str) -> Result<(String, Vec<String>), String> {
    let lines: Vec<&str> = text.lines().collect();
    let (all, end) = jobs(&lines)?;
    let gates: Vec<&Job> = GATE_JOBS
        .iter()
        .filter_map(|g| all.iter().find(|j| j.name == *g))
        .collect();
    let mut region: Vec<&str> = vec![];
    let open =
        format!("  # {CI_OPEN} placed by `yidam migrate scaffold`; the next re-vendor fills it");
    let close = format!("  # {CI_CLOSE}");
    region.push(&open);
    for job in &gates {
        let mut body = &lines[job.start..job.end];
        while body.last().is_some_and(|l| l.trim().is_empty()) {
            body = &body[..body.len() - 1];
        }
        region.extend_from_slice(body);
    }
    region.push(&close);

    let at = gates.iter().map(|j| j.start).min().unwrap_or(end);
    let mut out: Vec<&str> = vec![];
    for (i, line) in lines.iter().enumerate() {
        if i == at {
            out.extend_from_slice(&region);
            if gates.is_empty() {
                // An empty region at the end of `jobs:`, separated from the job above it.
                out.insert(out.len() - region.len(), "");
            } else {
                out.push("");
            }
        }
        if gates.iter().any(|j| (j.start..j.end).contains(&i)) {
            continue;
        }
        out.push(line);
    }
    if at == lines.len() {
        out.push("");
        out.extend_from_slice(&region);
    }
    let mut s = out.join("\n");
    s.push('\n');
    Ok((s, gates.iter().map(|j| j.name.clone()).collect()))
}

/// `CLAUDE.md` with the template's sections inside the region, and which headings went in.
fn wrap_claude(text: &str) -> Result<(String, Vec<String>), String> {
    let lines: Vec<&str> = text.lines().collect();
    let headings: Vec<(usize, &str)> = lines
        .iter()
        .enumerate()
        .filter(|(_, l)| l.starts_with("## "))
        .map(|(i, l)| (i, l.trim_end()))
        .collect();
    let Some(first) = headings
        .iter()
        .position(|(_, h)| *h == TEMPLATE_HEADINGS[0])
    else {
        // Some of the template's sections but not the first: the region's start is a guess.
        return Err(format!(
            "{CLAUDE} has none of the template's `{}` heading to start the region at. Put \
             `{CLAUDE_OPEN}` and `{CLAUDE_CLOSE}` on lines of their own around the part that \
             is the template's, and the next re-vendor replaces it",
            TEMPLATE_HEADINGS[0]
        ));
    };
    let last = headings
        .iter()
        .rposition(|(_, h)| TEMPLATE_HEADINGS.contains(h))
        // `first` is one, so the search never comes back empty.
        .unwrap_or(first);
    if let Some((i, h)) = headings[first..=last]
        .iter()
        .find(|(_, h)| !TEMPLATE_HEADINGS.contains(h))
    {
        return Err(format!(
            "{CLAUDE}:{} `{h}` sits between the template's sections, and wrapping it would \
             hand it to the re-vendor. Move it above `{}` or below `{}`, then run this again",
            i + 1,
            TEMPLATE_HEADINGS[0],
            headings[last].1
        ));
    }
    let start = headings[first].0;
    let mut end = headings.get(last + 1).map_or(lines.len(), |(i, _)| *i);
    while end > start && lines[end - 1].trim().is_empty() {
        end -= 1;
    }
    let mut out: Vec<&str> = vec![];
    out.extend_from_slice(&lines[..start]);
    out.push(CLAUDE_OPEN);
    out.push("");
    out.extend_from_slice(&lines[start..end]);
    out.push("");
    out.push(CLAUDE_CLOSE);
    if end < lines.len() {
        out.push("");
        let mut rest = &lines[end..];
        while rest.first().is_some_and(|l| l.trim().is_empty()) {
            rest = &rest[1..];
        }
        out.extend_from_slice(rest);
    }
    let mut s = out.join("\n");
    s.push('\n');
    let wrapped = headings[first..=last]
        .iter()
        .map(|(_, h)| h.to_string())
        .collect();
    Ok((s, wrapped))
}

/// What the migration would write to one file, or why it cannot.
type Planned = Result<(String, Vec<String>), String>;

/// How one file is read: whether it carries its region, whether any of it is the template's,
/// and how to wrap it.
struct Kind {
    rel: &'static str,
    has: fn(&str) -> bool,
    template: fn(&str) -> bool,
    wrap: fn(&str) -> Planned,
}

/// The two files, in the order the report lists them. Every workflow is the template's to
/// mark: one with no gate job is the case an empty region exists for.
const KINDS: [Kind; 2] = [
    Kind {
        rel: CI,
        has: ci_has_region,
        template: |_| true,
        wrap: wrap_ci,
    },
    Kind {
        rel: CLAUDE,
        has: claude_has_region,
        template: claude_is_template,
        wrap: wrap_claude,
    },
];

/// Read one file and plan its region. `None` for a file there is nothing to write to.
fn plan_file(root: &Path, kind: &Kind) -> (ScaffoldFile, Option<Planned>) {
    let Kind {
        rel,
        has,
        template,
        wrap,
    } = kind;
    let mut f = ScaffoldFile {
        file: rel.to_string(),
        already: false,
        absent: false,
        owned: false,
        wrapped: vec![],
    };
    let Ok(text) = std::fs::read_to_string(root.join(rel)) else {
        f.absent = true;
        return (f, None);
    };
    if has(&text) {
        f.already = true;
        return (f, None);
    }
    if !template(&text) {
        f.owned = true;
        return (f, None);
    }
    let planned = wrap(&text);
    if let Ok((_, wrapped)) = &planned {
        f.wrapped.clone_from(wrapped);
    }
    (f, Some(planned))
}

/// Plan it. Every refusal goes in `blocked`.
pub(crate) fn plan(root: &Path, report: &mut MigrateReport) {
    let mut files = vec![];
    for kind in &KINDS {
        let (f, planned) = plan_file(root, kind);
        if let Some(Err(why)) = planned {
            report.blocked.push(why);
        }
        files.push(f);
    }
    report.scaffold = Some(Scaffold { files });
}

/// Write it. Re-plans each file, for the reason `migrate routes` re-locates its list: the plan
/// and the apply are two reads of the same file.
pub(crate) fn apply(root: &Path, report: &MigrateReport) -> anyhow::Result<Vec<String>> {
    let mut written = vec![];
    for Kind { rel, has, wrap, .. } in &KINDS {
        let planned = report
            .scaffold
            .as_ref()
            .and_then(|s| s.files.iter().find(|f| f.file == *rel))
            .is_some_and(ScaffoldFile::to_write);
        if !planned {
            continue;
        }
        let path = root.join(rel);
        let text = std::fs::read_to_string(&path)?;
        let (out, _) = wrap(&text).map_err(anyhow::Error::msg)?;
        // The point is that the re-vendor finds the region. If it would not, leave the file.
        if !has(&out) {
            anyhow::bail!("the region written into {rel} is not one a re-vendor would find");
        }
        std::fs::write(&path, out)?;
        written.push(rel.to_string());
    }
    Ok(written)
}

/// One line, for the report once planned.
pub(crate) fn summary(_report: &MigrateReport) -> String {
    "ci.yml and CLAUDE.md regions a re-vendor updates".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A workflow from before the regions: `detect` between the two gate jobs, an owner's job
    /// at each end.
    const OLD_CI: &str = "\
name: ci
on: [push]

jobs:
  ours:
    runs-on: ubuntu-latest

  # The privacy job's explanation.
  privacy:
    runs-on: ubuntu-latest

  detect:
    runs-on: ubuntu-latest

  corpus:
    steps:
      - run: yidam graph-check

  web:
    needs: detect
";

    fn scaffold(rel: &str) -> String {
        std::fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../..")
                .join(rel),
        )
        .unwrap_or_else(|e| panic!("{rel}: {e}"))
    }

    #[test]
    fn the_scaffold_carries_both_regions_in_the_spelling_this_reads() {
        assert!(ci_has_region(&scaffold("sadhana/github/workflows/ci.yml")));
        assert!(claude_has_region(&scaffold("sadhana/root/CLAUDE.md")));
    }

    /// The region the scaffold carries is exactly the gate jobs, in this module's order. A job
    /// added to it upstream is one a migration would leave outside, as the owner's.
    #[test]
    fn the_scaffold_region_holds_the_gate_jobs_and_no_others() {
        let text = scaffold("sadhana/github/workflows/ci.yml");
        let lines: Vec<&str> = text.lines().collect();
        let (all, _) = jobs(&lines).unwrap();
        let open = lines.iter().position(|l| marks(l, CI_OPEN)).unwrap();
        let close = lines.iter().position(|l| marks(l, CI_CLOSE)).unwrap();
        let inside: Vec<&str> = all
            .iter()
            .filter(|j| {
                let key = j.start
                    + lines[j.start..]
                        .iter()
                        .position(|l| !l.trim_start().starts_with('#'))
                        .unwrap();
                key > open && key < close
            })
            .map(|j| j.name.as_str())
            .collect();
        assert_eq!(inside, GATE_JOBS);
    }

    #[test]
    fn the_scaffold_claude_region_opens_at_the_first_template_heading() {
        let text = scaffold("sadhana/root/CLAUDE.md");
        let open = text.find(CLAUDE_OPEN).unwrap();
        let first = text.find(TEMPLATE_HEADINGS[0]).unwrap();
        assert!(open < first);
        for h in TEMPLATE_HEADINGS {
            let at = text
                .find(h)
                .unwrap_or_else(|| panic!("the scaffold has no `{h}`"));
            assert!(at > open && at < text.find(CLAUDE_CLOSE).unwrap(), "{h}");
        }
    }

    /// `yidam-vendor-update` stays quiet about a `CLAUDE.md` with none of these headings, by a
    /// regex of its own. A heading added here and not there sends an owner to a migration
    /// that has nothing to do, or the other way round.
    #[test]
    fn the_re_vendor_reads_the_same_template_headings() {
        let task = scaffold("mise.yidam.toml");
        let line = task
            .lines()
            .find(|l| l.starts_with("splice_region .claude/CLAUDE.md"))
            .expect("the task splices CLAUDE.md");
        let alternation = line
            .split_once("'^## (")
            .and_then(|(_, r)| r.split_once(')'))
            .map(|(a, _)| a)
            .unwrap_or_else(|| panic!("no heading alternation in: {line}"));
        let theirs: Vec<String> = alternation.split('|').map(|h| format!("## {h}")).collect();
        assert_eq!(theirs, TEMPLATE_HEADINGS);
    }

    #[test]
    fn the_gate_jobs_move_together_and_nothing_else_moves() {
        let (out, wrapped) = wrap_ci(OLD_CI).unwrap();
        assert_eq!(wrapped, ["privacy", "corpus"]);
        assert!(ci_has_region(&out));
        let open = out.find(CI_OPEN).unwrap();
        let close = out.find(CI_CLOSE).unwrap();
        let region = &out[open..close];
        assert!(
            region.contains("# The privacy job's explanation.\n  privacy:"),
            "{out}"
        );
        assert!(region.contains("yidam graph-check"), "{out}");
        assert!(!region.contains("detect:"), "{out}");
        assert!(
            out.starts_with("name: ci\non: [push]\n\njobs:\n  ours:\n"),
            "{out}"
        );
        assert!(
            out[close..].contains("detect:") && out[close..].contains("web:"),
            "{out}"
        );
        assert_eq!(out.matches("corpus:").count(), 1, "{out}");
        assert_eq!(out.matches("privacy:").count(), 1, "{out}");
    }

    #[test]
    fn a_workflow_with_no_gate_gets_an_empty_region_at_the_end_of_jobs() {
        let text = "name: ci\njobs:\n  rust:\n    runs-on: x\n\n  deploy:\n    runs-on: y\n";
        let (out, wrapped) = wrap_ci(text).unwrap();
        assert!(wrapped.is_empty());
        assert!(
            out.ends_with("  deploy:\n    runs-on: y\n\n  # <!-- YIDAM:CI --> placed by `yidam migrate scaffold`; the next re-vendor fills it\n  # <!-- /YIDAM:CI -->\n"),
            "{out}"
        );

        // `jobs:` followed by another top-level key: the region goes before that key.
        let text = "jobs:\n  rust:\n    runs-on: x\n\nenv:\n  A: b\n";
        let (out, _) = wrap_ci(text).unwrap();
        let close = out.find(CI_CLOSE).unwrap();
        assert!(
            out.find("runs-on: x").unwrap() < close && close < out.find("env:").unwrap(),
            "{out}"
        );
    }

    #[test]
    fn a_workflow_with_no_jobs_key_is_refused() {
        assert!(wrap_ci("name: ci\n").unwrap_err().contains("jobs:"));
    }

    const OLD_CLAUDE: &str = "\
# Working in this repository

This repository studies rivers.

## The short version

- a

## Before committing

- b

## Ours

Domain notes.
";

    #[test]
    fn the_template_sections_are_wrapped_and_the_owner_s_are_not() {
        let (out, wrapped) = wrap_claude(OLD_CLAUDE).unwrap();
        assert_eq!(wrapped, ["## The short version", "## Before committing"]);
        assert!(claude_has_region(&out));
        assert!(out.starts_with("# Working in this repository\n\nThis repository studies rivers.\n\n<!-- YIDAM:CLAUDE -->\n\n## The short version\n"), "{out}");
        assert!(
            out.contains("- b\n\n<!-- /YIDAM:CLAUDE -->\n\n## Ours\n\nDomain notes.\n"),
            "{out}"
        );
    }

    #[test]
    fn an_owner_section_between_template_sections_is_refused() {
        let text = OLD_CLAUDE.replace(
            "## Before committing",
            "## Ours first\n\nx\n\n## Before committing",
        );
        let why = wrap_claude(&text).unwrap_err();
        assert!(
            why.contains("`## Ours first`") && why.contains("between"),
            "{why}"
        );
    }

    #[test]
    fn a_claude_md_with_no_template_heading_is_the_owner_s() {
        assert!(!claude_is_template("# Hi\n\n## Ours\n"));
        assert!(claude_is_template(OLD_CLAUDE));
        // Some of the template's sections, but not the one the region starts at.
        let text = OLD_CLAUDE.replace("## The short version", "## Ours first");
        assert!(wrap_claude(&text)
            .unwrap_err()
            .contains("The short version"));
    }

    #[test]
    fn a_marker_in_a_yaml_comment_or_on_its_own_line_is_the_same_marker() {
        assert!(marks("  # <!-- YIDAM:CI --> trailing words", CI_OPEN));
        assert!(marks("<!-- /YIDAM:CLAUDE -->", CLAUDE_CLOSE));
        assert!(!marks("  # see <!-- YIDAM:CI --> above", CI_OPEN));
        assert!(!has_region(
            "<!-- /YIDAM:CI -->\n<!-- YIDAM:CI -->\n",
            CI_OPEN,
            CI_CLOSE
        ));
    }
}
