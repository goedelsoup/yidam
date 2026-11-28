use super::*;

// ── Prose links ──────────────────────────────────────────────────────────────
//
// `graph-check` validates the corpus's own `links:` edges. Nothing read the markdown
// links in the prose beside them, and they break silently: a node cites a resolution, a
// resolution cites a position, a convention cites a guideline, and any of those can be
// moved or removed by a commit that never touches the citing file.
//
// In a derived repository the gap had eleven live instances — four citations to position
// files that had never left their author's branch, and seven naming files that are not
// there, including two pointing one directory short at the code behind a published figure.

/// One markdown link found in prose.
pub struct ProseLink {
    pub file: String,
    pub line: usize,
    /// The target exactly as written, for the message.
    pub target: String,
    /// Where it resolves from the directory of the file it sits in — which is what a
    /// reader's markdown client does, and the whole reason a repo-root-relative link in
    /// a nested file is broken.
    pub resolved: PathBuf,
    /// A `#L<n>` / `#L<n>-L<m>` fragment, when the link carries one. A heading anchor
    /// is `None`: the file is what exists, the heading is not ours to verify. A line
    /// range is ours — see [`super::super::line_citations`].
    pub fragment: Option<super::super::line_citations::LineFragment>,
    /// The key values of a `#row=` fragment (RFC-0059 §2.5), decoded, when the link carries
    /// one. Resolved against the entry's declared table by `broken-prose-link`.
    pub row: Option<Vec<String>>,
    /// Byte columns of the whole `[label](target)` on its line, for finding the quoted
    /// passage beside it. Valid into the raw line: the mask that found the link keeps
    /// byte offsets.
    pub span: (usize, usize),
}

/// Whether a link target points outside the filesystem and is therefore not ours to check.
fn is_external(target: &str) -> bool {
    target.is_empty()
        || target.starts_with('#')
        || target.contains("://")
        || target.starts_with("mailto:")
        || target.starts_with("tel:")
        // Root-relative. In a repository these are ambiguous — repo root or site root —
        // and resolving them either way produces findings nobody can act on.
        || target.starts_with('/')
}

/// Strip a markdown title and any angle-bracket wrapper: `<a b> "title"` → `a b`, and
/// split off the fragment.
///
/// A heading anchor is dropped — the file is what exists, the heading is not ours to
/// verify. A `#L<n>` line fragment is different in kind: it names lines in a file this
/// repository owns and is decidable, so it is kept for the line-citation checks rather
/// than falling with the anchors (#563).
fn link_parts(raw: &str) -> (String, Option<super::super::line_citations::LineFragment>) {
    let (start, end, frag) = link_path_bounds(raw);
    (
        raw[start..end].to_string(),
        frag.and_then(super::super::line_citations::parse_fragment),
    )
}

/// Where [`link_parts`] finds the path inside the text between a link's parentheses, as a
/// byte range of `raw`, and the fragment after it.
///
/// One reading of the target, shared by the checks that resolve it and by `rename`, which
/// must put the new path in the same bytes the old one was read from (#1691).
fn link_path_bounds(raw: &str) -> (usize, usize, Option<&str>) {
    let t = raw.trim();
    let lead = raw.len() - raw.trim_start().len();
    let (inner, at) = match (t.strip_prefix('<'), t.find('>')) {
        (Some(_), Some(end)) => (&t[1..end], lead + 1),
        _ => (t.split_whitespace().next().unwrap_or(t), lead),
    };
    let (path, frag) = match inner.split_once('#') {
        Some((path, frag)) => (path, Some(frag)),
        None => (inner, None),
    };
    let start = at + path.len() - path.trim_start().len();
    (start, start + path.trim().len(), frag)
}

/// One `[label](target)` on a line, as [`prose_links`] reads it.
pub(crate) struct LineLink {
    /// Byte column of the `[` that opens the label.
    pub label_start: usize,
    /// Byte column just past the closing `)`.
    pub end: usize,
    /// The byte range of the target's path: the part [`prose_links`] resolves, without a
    /// title, angle brackets, or fragment.
    pub path: (usize, usize),
    /// The text between the parentheses, as written.
    target_raw: (usize, usize),
}

/// The links on one line, in order. `line` is what [`prose_lines`] hands out, with inline
/// code blanked; a line that has not been through it reads a link inside a code span.
pub(crate) fn line_links(line: &str) -> Vec<LineLink> {
    let mut out = Vec::new();
    let bytes = line.as_bytes();
    let mut j = 0;
    while j < bytes.len() {
        let Some(open) = line[j..].find("](") else {
            break;
        };
        let at = j + open;
        j = at + 2;
        let Some(close_rel) = line[j..].find(')') else {
            break;
        };
        let target_raw = (j, j + close_rel);
        j += close_rel + 1;
        // Walk back to the `[` that opens the label, for the link's full extent.
        let label_start = {
            let mut depth = 1usize;
            let mut k = at;
            loop {
                let Some(prev) = line[..k].rfind(['[', ']']) else {
                    break at;
                };
                k = prev;
                match bytes[k] {
                    b']' => depth += 1,
                    _ => {
                        depth -= 1;
                        if depth == 0 {
                            break k;
                        }
                    }
                }
            }
        };
        let (s, e, _) = link_path_bounds(&line[target_raw.0..target_raw.1]);
        out.push(LineLink {
            label_start,
            end: j,
            path: (target_raw.0 + s, target_raw.0 + e),
            target_raw,
        });
    }
    out
}

/// The lines of `text` a reader takes as prose: each with its 1-based number, and with inline
/// code spans blanked to spaces so byte offsets into it are offsets into the raw line.
///
/// Fenced blocks are left out whole. The marker is remembered, so a ```` ``` ```` inside a
/// `~~~` block does not close it.
///
/// [`prose_links`] reads these lines, and so does `source-unregistered` (#1068). One reading
/// of "what is an example" is what keeps a URL in a shell command from being a finding there
/// when a link in the same command is not one here.
pub(crate) fn prose_lines(text: &str) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    let mut fence: Option<String> = None;
    for (i, raw) in text.lines().enumerate() {
        let trimmed = raw.trim_start();
        if let Some(open) = &fence {
            if trimmed.starts_with(open.as_str()) {
                fence = None;
            }
            continue;
        }
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            fence = Some(trimmed.chars().take(3).collect());
            continue;
        }
        out.push((i + 1, crate::markdown::mask_code_spans(raw)));
    }
    out
}

/// Every checkable markdown link in one file.
///
/// `dir` is the directory the file sits in — links resolve against it, not against the
/// repository root.
///
/// **Fenced code blocks are skipped.** A path inside a fence is an example, a shell
/// command, or a directory tree drawing; these documents are full of them and every one
/// would be a false positive. Inline code spans are skipped for the same reason.
pub fn prose_links(file: &str, dir: &Path, text: &str) -> Vec<ProseLink> {
    let mut out = Vec::new();
    for (number, line) in prose_lines(text) {
        for link in line_links(&line) {
            let target_raw = &line[link.target_raw.0..link.target_raw.1];
            let (target, fragment) = link_parts(target_raw);
            if is_external(&target) {
                continue;
            }
            let row = target_raw
                .split_whitespace()
                .next()
                .and_then(|t| t.split_once('#'))
                .and_then(|(_, frag)| crate::table::row_fragment(frag.trim_end_matches('>')));
            out.push(ProseLink {
                file: file.to_string(),
                line: number,
                target: target.clone(),
                resolved: dir.join(&target),
                fragment,
                row,
                span: (link.label_start, link.end),
            });
        }
    }
    out
}

#[cfg(test)]
fn broken_prose_link(links: &[ProseLink]) -> Check {
    broken_prose_link_with_rows(links, Path::new(""), &[])
}

pub const BROKEN_PROSE_LINK: CheckSpec = CheckSpec {
    id: "broken-prose-link",
    title: "A markdown link in prose does not resolve",
    severity: Severity::Error,
    rationale: "A link resolves against the directory of the file it sits in — that is what a \
         reader's client does — so a path spelled from the repository root is broken in \
         every file that is not at the root. Error severity, which puts it under the \
         baseline ratchet: a link listed there and since repaired fails the build exactly \
         as a new break does, because a list permitted to be wrong drifts and one that \
         over-lists silently re-permits whatever it over-lists. Fenced and inline code are \
         not read, so an example path in a shell command is not a finding.",
};

/// [`broken_prose_link`], with each `#row=` fragment resolved against the table its entry
/// declares (RFC-0059 §2.5). `tables` is [`super::super::tables::declared`]'s reading, and `root` is
/// what each declaring entry's path is relative to.
pub(crate) fn broken_prose_link_with_rows(
    links: &[ProseLink],
    root: &Path,
    tables: &[super::super::tables::Table],
) -> Check {
    let mut violations: Vec<Violation> = links
        .iter()
        .filter(|l| !l.resolved.exists())
        .map(|l| {
            Violation::new(
                format!("{}:{}", l.file, l.line),
                format!("`{}` does not resolve from this file's directory", l.target),
            )
        })
        .collect();
    for l in links.iter().filter(|l| l.resolved.exists()) {
        let Some(values) = &l.row else { continue };
        if let Some(why) = row_unresolved(l, values, root, tables) {
            violations.push(Violation::new(format!("{}:{}", l.file, l.line), why));
        }
    }
    Check::new(&BROKEN_PROSE_LINK, violations)
}

/// Why a `#row=` link names no row, or `None` when it names one.
///
/// An entry whose declaration is malformed is passed over: `table-declaration-invalid` has
/// already reported it, and naming every link into it again would say the same thing twice.
fn row_unresolved(
    link: &ProseLink,
    values: &[String],
    root: &Path,
    tables: &[super::super::tables::Table],
) -> Option<String> {
    use super::super::tables::Table;
    let target = normalize(&link.resolved);
    let entry_of = |rel: &str| normalize(&root.join(rel)) == target;
    let found = tables.iter().find(|t| match t {
        Table::Invalid { entry_rel, .. } => entry_of(entry_rel),
        Table::Read { decl, .. } => entry_of(&decl.entry_rel),
    });
    let (decl, rows) = match found {
        None => {
            return Some(format!(
                "`{}` cites a row, and the entry declares no table",
                link.target
            ))
        }
        Some(Table::Invalid { .. }) => return None,
        Some(Table::Read { decl, rows, .. }) => (decl, rows),
    };
    if decl.key.is_empty() {
        return Some(format!(
            "`{}` cites a row, and `{}` declares no `key:` to name one by",
            link.target, decl.entry
        ));
    }
    if values.len() != decl.key.len() {
        return Some(format!(
            "`{}` names {} key values, and `{}`'s key is {} columns: {}",
            link.target,
            values.len(),
            decl.entry,
            decl.key.len(),
            decl.key.join(", ")
        ));
    }
    (!crate::table::has_row(decl, rows, values)).then(|| {
        format!(
            "`{}` names no row of `{}` — no row's {} reads `{}`",
            link.target,
            decl.path,
            decl.key.join(", "),
            values.join(",")
        )
    })
}

pub const BROKEN_OBJECT_LINK: CheckSpec = CheckSpec {
    id: "broken-object-link",
    title: "A link from the artifact into the corpus does not resolve",
    severity: Severity::Warn,
    rationale:
        "The artifact that `[object] paths` declares cites its corpus the way the corpus cites \
         it: with a relative markdown link. Links from the corpus into the artifact are in \
         prose `broken-prose-link` already walks. The other direction was read by nothing: \
         deleting a node that a crate's README linked to produced twenty findings in a derived \
         corpus, and none of them named the README. Read over the tracked `.md`, `.rs` and \
         `.ts` files the object globs claim, which is where those links sit across the derived \
         corpora. Bare paths in code are not read, because a path string in code is as often \
         a file the code writes as one it cites. Warn and not Error: the finding lands on a \
         file the corpus gate has never judged, and the first run over a repository should \
         report what it finds, not refuse it. A `#L` range on one of these links is held by \
         the line-citation checks. Not read for a corpus whose kuten declares it `projected`, \
         because that corpus is regenerated from the artifact.",
};

/// A link from the artifact into the corpus whose target is not there (#577).
///
/// `links` are the ones [`crate::coupling::inbound`] found: every link in the artifact whose
/// target lies under `.yidam/`, from files `broken-prose-link` does not already walk.
pub fn broken_object_link(links: &[ProseLink]) -> Check {
    let violations = links
        .iter()
        .filter(|l| !l.resolved.exists())
        .map(|l| {
            Violation::new(
                format!("{}:{}", l.file, l.line),
                format!(
                    "`{}` names a corpus file that is not there. The record was moved, renamed \
                     or removed without this citation being updated.",
                    l.target
                ),
            )
        })
        .collect();
    Check::new(&BROKEN_OBJECT_LINK, violations)
}

/// A broken prose link inside a region the repository declared it did not author, paired
/// with the declaration that explains it.
pub struct UnauthoredLink<'a> {
    pub region: &'a crate::authorship::Region,
    pub link: ProseLink,
}

pub const UNAUTHORED_PROSE_LINK: CheckSpec = CheckSpec {
    id: "unauthored-prose-link",
    title: "A broken prose link in material this repository did not author",
    severity: Severity::Info,
    rationale:
        "Error severity is the right verdict for a link somebody here can go and fix. It is \
         the wrong one for generated output, whose defect belongs to the generator and whose \
         file is rewritten by the next build, and for an unmodified import, where editing the \
         file to satisfy a linter falsifies the record it exists to keep. A consumer that met \
         43 broken links under `docs/` could act on 28: fifteen were inside a directory whose \
         own README says it is a frozen copy of an upstream project at a fork point. It \
         baselined them, which records `we accept this violation` when the truth is `this \
         file is not ours` — and a baselined entry that is later repaired fails the build, so \
         the gate came to depend on nobody ever re-syncing the import. Declared in \
         `.yidam/authorship.yml`. Reported rather than silenced, with the reason and whoever \
         can act on it, because these are real defects addressed to somebody else. Info \
         severity: never gates, never baselined.",
};

pub fn unauthored_prose_link(links: &[UnauthoredLink<'_>]) -> Check {
    let violations = links
        .iter()
        .filter(|u| !u.link.resolved.exists())
        .map(|u| {
            Violation::new(
                format!("{}:{}", u.link.file, u.link.line),
                format!(
                    "`{}` does not resolve — {}",
                    u.link.target,
                    u.region.explain()
                ),
            )
        })
        .collect();
    Check::new(&UNAUTHORED_PROSE_LINK, violations)
}

pub const AUTHORSHIP_REGION_STALE: CheckSpec = CheckSpec {
    id: "authorship-region-stale",
    title: "A declared authorship region matches nothing on disk",
    severity: Severity::Warn,
    rationale: "A manifest permitted to be wrong drifts, exactly as a lint baseline does, and the \
         entry that outlives the directory it describes is the one that quietly excuses a \
         path somebody later creates under the same name. `generated` regions are exempt: \
         they are written by a build and are frequently git-ignored, so absence on a fresh \
         clone carries no information. Warn rather than Error, unlike the baseline's own \
         staleness rule, and for the reason this check exists — an imported region is \
         re-synced by somebody upstream on their schedule, and gating on it would make the \
         build's colour depend on that.",
};

pub fn authorship_region_stale(stale: &[&crate::authorship::Region]) -> Check {
    let violations = stale
        .iter()
        .map(|r| {
            Violation::new(
                crate::authorship::MANIFEST,
                format!(
                    "`{}` is declared `{}` but matches nothing on disk",
                    r.path,
                    r.kind.as_str()
                ),
            )
        })
        .collect();
    Check::new(&AUTHORSHIP_REGION_STALE, violations)
}

#[cfg(test)]
mod prose_link_tests {
    use super::*;

    fn scan(dir: &Path, text: &str) -> Vec<ProseLink> {
        prose_links("docs/x.md", dir, text)
    }

    fn fixture() -> tempfile::TempDir {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join("sub")).unwrap();
        std::fs::write(tmp.path().join("there.md"), "x").unwrap();
        std::fs::write(tmp.path().join("sub/deep.md"), "x").unwrap();
        tmp
    }

    /// The defect this exists for: a path spelled from the repository root, sitting in a
    /// file that is not at the root.
    #[test]
    fn a_root_relative_link_in_a_nested_file_is_broken() {
        let tmp = fixture();
        let links = scan(tmp.path(), "See [it](docs/there.md).");
        assert_eq!(links.len(), 1);
        let c = broken_prose_link(&links);
        assert_eq!(c.violations.len(), 1);
        assert!(c.violations[0].detail.contains("docs/there.md"));
    }

    #[test]
    fn a_link_relative_to_its_own_file_resolves() {
        let tmp = fixture();
        let links = scan(tmp.path(), "See [it](there.md) and [deep](sub/deep.md).");
        assert_eq!(links.len(), 2);
        assert!(broken_prose_link(&links).passed());
    }

    /// Every document here is full of example paths in shell blocks and tree drawings.
    /// Reading them would make the check unusable on its first run.
    #[test]
    fn fenced_code_is_not_read() {
        let tmp = fixture();
        let text = "\
Before.

```
git checkout ma/x -- [a](does/not/exist.md)
```

After [it](there.md).
";
        let links = scan(tmp.path(), text);
        assert_eq!(
            links.len(),
            1,
            "{:?}",
            links.iter().map(|l| &l.target).collect::<Vec<_>>()
        );
        assert_eq!(links[0].target, "there.md");
    }

    #[test]
    fn tilde_fences_and_nested_backticks_are_handled() {
        let tmp = fixture();
        let text = "~~~\n[a](nope.md)\n```\n[b](nope2.md)\n~~~\n[c](there.md)\n";
        let links = scan(tmp.path(), text);
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].target, "there.md");
    }

    #[test]
    fn inline_code_is_not_read() {
        let tmp = fixture();
        let links = scan(tmp.path(), "Write `[label](path/to/thing.md)` in the file.");
        assert!(
            links.is_empty(),
            "{:?}",
            links.iter().map(|l| &l.target).collect::<Vec<_>>()
        );
    }

    #[test]
    fn external_and_anchor_targets_are_skipped() {
        let tmp = fixture();
        let text = "[a](https://example.com/x) [b](#section) [c](mailto:x@y.z) [d](/abs/path)";
        assert!(scan(tmp.path(), text).is_empty());
    }

    /// The file is what exists; the heading is not ours to verify.
    #[test]
    fn an_anchor_is_stripped_before_resolving() {
        let tmp = fixture();
        let links = scan(tmp.path(), "[a](there.md#a-heading)");
        assert_eq!(links[0].target, "there.md");
        assert!(broken_prose_link(&links).passed());
    }

    #[test]
    fn titles_and_angle_brackets_are_stripped() {
        let tmp = fixture();
        let links = scan(tmp.path(), "[a](there.md \"Title\") and [b](<there.md>)");
        assert_eq!(links.len(), 2);
        assert!(broken_prose_link(&links).passed());
    }

    #[test]
    fn two_links_on_one_line_are_both_found() {
        let tmp = fixture();
        let links = scan(tmp.path(), "[a](there.md) then [b](gone.md)");
        assert_eq!(links.len(), 2);
        assert_eq!(broken_prose_link(&links).violations.len(), 1);
    }

    #[test]
    fn a_link_to_a_directory_resolves() {
        let tmp = fixture();
        let links = scan(tmp.path(), "[the subdir](sub/)");
        assert!(broken_prose_link(&links).passed());
    }

    #[test]
    fn no_links_is_clean() {
        assert!(broken_prose_link(&[]).passed());
    }
}
