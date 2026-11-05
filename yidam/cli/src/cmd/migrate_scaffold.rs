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
//! template's headings, which a re-vendor then replaces with the scaffold's import of
//! `AGENTS.md`. An owner's own section *between* two of the template's refuses, because
//! wrapping it would hand it to the re-vendor. An owner's section after them stays outside.
//! A `CLAUDE.md` with none of the template's headings is the owner's whole, and is left alone:
//! two of seventeen derivations wrote their own, and an empty region there would put a second
//! summary into a file that already carries one.
//!
//! Wrapped content is written as it is. The re-vendor replaces it, and the diff it leaves is
//! where an owner sees a step they had added inside a gate job. The report says so up front.
//!
//! # The loop's hooks
//!
//! A third file is not a region (#1444). `.claude/settings.json` gets the two `yidam hook`
//! entries that hand a session `cycle --brief` and tell the person what each turn moved. JSON
//! has no comments to hang a marker on, and a re-vendor has nothing to update there: the hook
//! is one command line, and what it does is the binary's. So this installs them once, and that
//! is all.
//!
//! **Written as text, not re-serialized.** The entries go in right after the object's opening
//! brace and every other byte stays as the owner wrote it; the result is parsed back and must
//! equal the original plus `hooks`, or nothing is written. A settings file that already
//! declares `hooks` is refused, saying what to add: merging into an owner's own hooks is a
//! judgment this does not make. An absent file is created holding only the hooks.

use std::path::Path;

use super::migrate::MigrateReport;

/// The workflow, relative to the repository root.
pub(crate) const CI: &str = ".github/workflows/ci.yml";

/// The agent instructions, relative to the repository root.
pub(crate) const CLAUDE: &str = ".claude/CLAUDE.md";

/// The Claude Code settings, relative to the repository root.
pub(crate) const SETTINGS: &str = ".claude/settings.json";

/// The two hooks, as the value of `hooks` one level into the settings object. The scaffold's
/// `sadhana/root/claude-settings.json` holds exactly this, and a test holds the two equal.
///
/// The command names no absolute path, so the file is the same in every checkout (#1065): it
/// runs from the project root, prefers the binary `.yidam/bin` pins, and ends `exit 0` so a
/// missing or older binary is silence rather than a failed hook.
pub(crate) const HOOKS: &str = r#"{
    "SessionStart": [
      {
        "matcher": "startup|resume|clear|compact",
        "hooks": [
          {
            "type": "command",
            "command": "cd \"$CLAUDE_PROJECT_DIR\" && PATH=\".yidam/bin:$PATH\" yidam hook session-start 2>/dev/null; exit 0",
            "timeout": 30
          }
        ]
      }
    ],
    "Stop": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "cd \"$CLAUDE_PROJECT_DIR\" && PATH=\".yidam/bin:$PATH\" yidam hook stop 2>/dev/null; exit 0",
            "timeout": 30
          }
        ]
      }
    ]
  }"#;

/// The hook events, and the `yidam hook` argument each runs.
const HOOK_EVENTS: [(&str, &str); 2] = [("SessionStart", "session-start"), ("Stop", "stop")];

/// The settings file the scaffold ships: the hooks and nothing else.
pub(crate) fn settings_scaffold() -> String {
    format!("{{\n  \"hooks\": {HOOKS}\n}}\n")
}

/// Whether settings text runs `yidam hook <event>` under each event. Read from the parsed
/// JSON, so a command string elsewhere in the file — an allowed permission, say — is not one.
pub(crate) fn settings_has_hooks(text: &str) -> bool {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(text) else {
        return false;
    };
    HOOK_EVENTS.iter().all(|(event, arg)| {
        let want = format!("yidam hook {arg}");
        v.pointer(&format!("/hooks/{event}"))
            .and_then(serde_json::Value::as_array)
            .is_some_and(|groups| {
                groups.iter().any(|g| {
                    g.get("hooks")
                        .and_then(serde_json::Value::as_array)
                        .is_some_and(|hs| {
                            hs.iter().any(|h| {
                                h.get("command")
                                    .and_then(serde_json::Value::as_str)
                                    .is_some_and(|c| c.contains(&want))
                            })
                        })
                })
            })
    })
}

/// Add the hooks to settings text, or say why not. Empty text is an absent file.
fn wrap_settings(text: &str) -> Planned {
    let events = || HOOK_EVENTS.iter().map(|(e, _)| e.to_string()).collect();
    if text.trim().is_empty() {
        return Ok((settings_scaffold(), events()));
    }
    let parsed: serde_json::Value = serde_json::from_str(text)
        .map_err(|e| format!("{SETTINGS} is not JSON ({e}), so no hook is added to it"))?;
    let Some(obj) = parsed.as_object() else {
        return Err(format!(
            "{SETTINGS} is not a JSON object, so no hook is added to it"
        ));
    };
    if obj.contains_key("hooks") {
        return Err(format!(
            "{SETTINGS} declares its own `hooks`, and this does not merge into them — add a \
             SessionStart and a Stop hook running `yidam hook session-start` and `yidam hook \
             stop` by hand, as `sadhana/root/claude-settings.json` does"
        ));
    }
    let out = if obj.is_empty() {
        settings_scaffold()
    } else {
        let brace = text.find('{').unwrap_or(0);
        format!(
            "{}{{\n  \"hooks\": {HOOKS},{}",
            &text[..brace],
            &text[brace + 1..]
        )
    };
    // The point is that nothing else moved: parse it back and compare.
    let mut back: serde_json::Value = serde_json::from_str(&out)
        .map_err(|e| format!("{SETTINGS}: adding the hooks would not parse ({e})"))?;
    back.as_object_mut().map(|o| o.remove("hooks"));
    if back != parsed {
        return Err(format!(
            "{SETTINGS}: adding the hooks would change something else, so nothing is written"
        ));
    }
    Ok((out, events()))
}

/// The region tokens, as the scaffold and `yidam-vendor-update` spell them.
const CI_OPEN: &str = "<!-- YIDAM:CI -->";
const CI_CLOSE: &str = "<!-- /YIDAM:CI -->";
const CLAUDE_OPEN: &str = "<!-- YIDAM:CLAUDE -->";
const CLAUDE_CLOSE: &str = "<!-- /YIDAM:CLAUDE -->";

/// The jobs that are yidam's, in the order the scaffold writes them.
pub(crate) const GATE_JOBS: &[&str] = &["privacy", "corpus"];

/// The template's `CLAUDE.md` headings, as every derivation made before the region existed
/// carries them. The first is where the region starts.
///
/// The scaffold no longer writes them: its region is now one import of `AGENTS.md`. They stay
/// because they are what tells a template `CLAUDE.md` from an owner's in a repository that has
/// never been migrated, and those files do not change because the scaffold did.
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
    /// An absent file is planned as empty text, which `wrap` creates, rather than skipped.
    create: bool,
}

/// The three files, in the order the report lists them. Every workflow is the template's to
/// mark: one with no gate job is the case an empty region exists for. Every settings file is
/// the template's to add hooks to, and an absent one is created.
const KINDS: [Kind; 3] = [
    Kind {
        rel: CI,
        has: ci_has_region,
        template: |_| true,
        wrap: wrap_ci,
        create: false,
    },
    Kind {
        rel: CLAUDE,
        has: claude_has_region,
        template: claude_is_template,
        wrap: wrap_claude,
        create: false,
    },
    Kind {
        rel: SETTINGS,
        has: settings_has_hooks,
        template: |_| true,
        wrap: wrap_settings,
        create: true,
    },
];

/// Read one file and plan its region. `None` for a file there is nothing to write to.
fn plan_file(root: &Path, kind: &Kind) -> (ScaffoldFile, Option<Planned>) {
    let Kind {
        rel,
        has,
        template,
        wrap,
        create,
    } = kind;
    let mut f = ScaffoldFile {
        file: rel.to_string(),
        already: false,
        absent: false,
        owned: false,
        wrapped: vec![],
    };
    let text = match std::fs::read_to_string(root.join(rel)) {
        Ok(t) => t,
        Err(_) if *create => String::new(),
        Err(_) => {
            f.absent = true;
            return (f, None);
        }
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
    for Kind {
        rel,
        has,
        wrap,
        create,
        ..
    } in &KINDS
    {
        let planned = report
            .scaffold
            .as_ref()
            .and_then(|s| s.files.iter().find(|f| f.file == *rel))
            .is_some_and(ScaffoldFile::to_write);
        if !planned {
            continue;
        }
        let path = root.join(rel);
        let text = match std::fs::read_to_string(&path) {
            Err(e) if *create && e.kind() == std::io::ErrorKind::NotFound => String::new(),
            read => read?,
        };
        let (out, _) = wrap(&text).map_err(anyhow::Error::msg)?;
        // The point is that the re-vendor finds the region. If it would not, leave the file.
        if !has(&out) {
            anyhow::bail!("the region written into {rel} is not one a re-vendor would find");
        }
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(&path, out)?;
        written.push(rel.to_string());
    }
    Ok(written)
}

/// One line, for the report once planned.
pub(crate) fn summary(_report: &MigrateReport) -> String {
    "ci.yml and CLAUDE.md regions a re-vendor updates, and the loop's hooks in \
     .claude/settings.json"
        .to_string()
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

    /// The scaffold's settings file is the hooks this installs, byte for byte, and they run
    /// the two events `yidam hook` answers.
    #[test]
    fn the_scaffold_settings_are_the_hooks_this_installs() {
        let text = scaffold("sadhana/root/claude-settings.json");
        assert_eq!(text, settings_scaffold());
        assert!(settings_has_hooks(&text));
        for (_, arg) in HOOK_EVENTS {
            use clap::ValueEnum as _;
            assert!(
                crate::cmd::hook::HookEvent::from_str(arg, false).is_ok(),
                "`yidam hook {arg}` is not an event the binary takes"
            );
        }
    }

    /// An absent or empty settings file becomes the scaffold's; one with keys of its own keeps
    /// every byte of them.
    #[test]
    fn hooks_are_added_and_nothing_else_moves() {
        for empty in ["", "{}", "{ }\n"] {
            let (out, events) = wrap_settings(empty).unwrap();
            assert_eq!(out, settings_scaffold(), "{empty:?}");
            assert_eq!(events, ["SessionStart", "Stop"]);
        }
        let owner =
            "{\n  \"permissions\": {\n    \"allow\": [\"Bash(ls)\"]\n  },\n  \"zz\": 1\n}\n";
        let (out, _) = wrap_settings(owner).unwrap();
        assert!(settings_has_hooks(&out), "{out}");
        assert!(
            out.ends_with(&owner[1..]),
            "the owner's bytes moved:\n{out}"
        );
        // Idempotent through `has`: the plan does not wrap a file that already has them.
        assert!(!settings_has_hooks(owner));
    }

    /// An owner's own hooks are not merged into, and neither is a file that is not an object.
    #[test]
    fn settings_with_their_own_hooks_or_no_object_are_refused() {
        let why = wrap_settings("{\"hooks\": {\"Stop\": []}}").unwrap_err();
        assert!(why.contains("by hand"), "{why}");
        assert!(wrap_settings("[]").is_err());
        assert!(wrap_settings("{ not json").is_err());
    }

    /// A command naming the hook somewhere other than under its event is not the hook.
    #[test]
    fn a_hook_is_read_from_its_event() {
        let elsewhere = r#"{"permissions": {"allow": ["Bash(yidam hook session-start)", "Bash(yidam hook stop)"]}}"#;
        assert!(!settings_has_hooks(elsewhere));
        let swapped = settings_scaffold()
            .replace("hook session-start", "hook X")
            .replace("hook stop", "hook session-start")
            .replace("hook X", "hook stop");
        assert!(!settings_has_hooks(&swapped));
        // Half installed is not installed: a session briefed and never recorded, or the reverse.
        let start_only = settings_scaffold().replace("hook stop", "hook other");
        assert!(!settings_has_hooks(&start_only));
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

    /// The region a re-vendor writes is the import of `AGENTS.md`, so a derivation keeps one
    /// working contract and `CLAUDE.md` cannot drift from it.
    #[test]
    fn the_scaffold_claude_region_imports_agents_md() {
        let text = scaffold("sadhana/root/CLAUDE.md");
        let open = text.find(CLAUDE_OPEN).unwrap();
        let close = text.find(CLAUDE_CLOSE).unwrap();
        let region = &text[open..close];
        assert!(
            region.lines().any(|l| l.trim() == "@../AGENTS.md"),
            "the scaffold's CLAUDE.md region does not import AGENTS.md:\n{region}"
        );
        assert!(
            !TEMPLATE_HEADINGS.iter().any(|h| region.contains(h)),
            "the scaffold's CLAUDE.md region carries a section of its own again:\n{region}"
        );
    }

    /// The headings name something real: the template `CLAUDE.md` an unmigrated derivation
    /// still has, kept as a fixture because the scaffold stopped writing it. The first opens
    /// the run, which is where [`wrap_claude`] starts the region.
    #[test]
    fn the_template_headings_are_the_pre_region_template_s() {
        let text = scaffold("yidam/cli/tests/fixtures/scaffold/claude-before-regions.md");
        let at: Vec<usize> = TEMPLATE_HEADINGS
            .iter()
            .map(|h| {
                text.find(h)
                    .unwrap_or_else(|| panic!("the pre-region template has no `{h}`"))
            })
            .collect();
        assert!(at.windows(2).all(|w| w[0] < w[1]), "{at:?}");
        assert!(claude_is_template(&text));
        assert!(!claude_has_region(&text));
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
