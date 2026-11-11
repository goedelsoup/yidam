//! What a derived repository has said to the template it came from (#1469).
//!
//! `guidelines/upstream.md` says *a misfit that is worth a workaround is worth an issue*, and
//! the derived `AGENTS.md` repeats it. Measured over the twenty derived repositories on hand
//! on 2026-11-11, fourteen had never cited an upstream issue in any commit, and the twenty
//! together held 16 citations against 92 `from:derived-repo` issues filed. One corpus
//! carried four workarounds across ~4,500 commits and reported none until a person asked.
//! The norm is prose read once at session start, and nothing asked the question at the
//! moment a workaround was written.
//!
//! This module is the one reading both surfaces share, so they cannot disagree about what a
//! report looks like:
//!
//! - **What counts as a citation.** A commit message naming the template's own repository —
//!   `owner/repo#N`, or a `github.com/owner/repo/issues/N` (or `/pull/N`) link. A bare `#N`
//!   is the derived repository's own tracker and is never counted. `directories.md` asks a
//!   repository to cite the issue in the commit that carries the workaround, and this is the
//!   form it names.
//! - **Which repository is the template.** `.yidam.toml`'s `origin`, so a derivation of a
//!   fork counts citations of the fork. Where the pin records none, the slug the guideline
//!   itself tells a repository to file against.
//! - **Where a directory is not laid out.** The top-level paths `directories.md` gives a
//!   section of its own. A file added anywhere else is a file the conventions did not
//!   anticipate, which is the shape a workaround takes — a mirror, a pinned copy, an outbox.
//!
//! `phase settle` asks at the end of a phase; `yidam cohort` counts across a population.
//! Neither files anything and neither gates: the licence `propose` keeps is the one kept here.

use std::path::Path;

/// The repository `directories.md` tells a derived repository to file against.
pub const DEFAULT_SLUG: &str = "goedelsoup/yidam";

/// The label every downstream report carries, per `directories.md`.
pub const LABEL: &str = "from:derived-repo";

/// The top-level paths `directories.md` gives a section of its own.
///
/// **Compiled in and held to the document both ways** by the test below: every entry is a
/// `## \`<entry>…\`` heading there, and every top-level heading there is an entry. A section
/// added upstream without an entry here fails the test rather than turning every file under it
/// into a question. `samudaya/` and `sadhana/` have sections and are deliberately absent:
/// both are transient, and the document calls their presence after genesis an error state.
pub const LAID_OUT: &[&str] = &[
    ".yidam",
    ".yidam.toml",
    "agents",
    "crates",
    "docs",
    "packages",
    "web",
];

/// The sections of `directories.md` that are deliberately not in [`LAID_OUT`].
pub const TRANSIENT: &[&str] = &["samudaya", "sadhana"];

/// `owner/repo` from a git remote URL, lowercased. `None` for a remote that names no
/// `owner/repo` pair — a local path, `unknown`, an empty string.
///
/// Handles the three forms `.yidam.toml` has been written with: scp-like
/// (`git@github.com:owner/repo.git`), `https://host/owner/repo(.git)`, and `ssh://`.
pub fn slug_of(origin: &str) -> Option<String> {
    let s = origin.trim().trim_end_matches('/');
    let s = s.strip_suffix(".git").unwrap_or(s);
    let path = if let Some((_, rest)) = s.split_once("://") {
        rest.split_once('/')?.1
    } else if let Some((host, rest)) = s.split_once(':') {
        // scp-like: `user@host:path`. A Windows drive letter or a bare path has no `@` and no
        // dot in what precedes the colon; neither is a remote this can name.
        if !(host.contains('@') || host.contains('.')) {
            return None;
        }
        rest
    } else {
        return None;
    };
    let mut parts = path.rsplit('/').filter(|p| !p.is_empty());
    let repo = parts.next()?;
    let owner = parts.next()?;
    let ok = |p: &str| {
        p.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
    };
    (ok(owner) && ok(repo)).then(|| format!("{owner}/{repo}").to_ascii_lowercase())
}

/// The template's `owner/repo`, as this repository's `.yidam.toml` records it, or
/// [`DEFAULT_SLUG`].
pub fn template_slug(root: &Path) -> String {
    std::fs::read_to_string(root.join(crate::provenance::MANIFEST))
        .ok()
        .and_then(|t| toml::from_str::<toml::Value>(&t).ok())
        .and_then(|v| v.get("yidam")?.get("origin")?.as_str().and_then(slug_of))
        .unwrap_or_else(|| DEFAULT_SLUG.to_string())
}

/// The pinned template commit, when `.yidam.toml` records a usable one.
pub fn template_pin(root: &Path) -> Option<String> {
    let text = std::fs::read_to_string(root.join(crate::provenance::MANIFEST)).ok()?;
    let v = toml::from_str::<toml::Value>(&text).ok()?;
    let c = v.get("yidam")?.get("commit")?.as_str()?.trim();
    (!c.is_empty() && c != "unknown").then(|| c.to_string())
}

/// Issue (or pull request) numbers `text` cites on `slug`'s tracker, in order of first
/// appearance and without repeats.
pub fn cited(text: &str, slug: &str) -> Vec<u64> {
    let lower = text.to_ascii_lowercase();
    let slug = slug.to_ascii_lowercase();
    let mut out = Vec::new();
    let mut push = |n: u64| {
        if !out.contains(&n) {
            out.push(n);
        }
    };
    let digits = |s: &str| -> Option<u64> {
        let end = s.find(|c: char| !c.is_ascii_digit()).unwrap_or(s.len());
        s[..end].parse().ok()
    };
    // `owner/repo#N`, where what precedes `owner` is not part of a longer name.
    let short = format!("{slug}#");
    let mut from = 0;
    while let Some(i) = lower[from..].find(&short) {
        let at = from + i;
        let bounded = lower[..at]
            .chars()
            .next_back()
            .is_none_or(|c| !(c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.')));
        if bounded {
            if let Some(n) = digits(&lower[at + short.len()..]) {
                push(n);
            }
        }
        from = at + short.len();
    }
    // `github.com/owner/repo/issues/N` and `/pull/N`.
    for kind in ["issues", "pull"] {
        let long = format!("github.com/{slug}/{kind}/");
        let mut from = 0;
        while let Some(i) = lower[from..].find(&long) {
            let at = from + i + long.len();
            if let Some(n) = digits(&lower[at..]) {
                push(n);
            }
            from = at;
        }
    }
    out
}

/// Commits reachable from `revs` whose message cites `slug`, and the distinct issues they
/// cite. `revs` is passed to `git log` as given — `["HEAD"]`, or `["phase/x", "^main"]`.
pub fn citations(root: &Path, revs: &[&str], slug: &str) -> Citations {
    let mut args = vec!["log", "--format=%B%x1e"];
    args.extend_from_slice(revs);
    let log = crate::git::Git::new(root)
        .args(&args)
        .try_run()
        .unwrap_or_default();
    let mut c = Citations::default();
    for body in log.split('\u{1e}') {
        let found = cited(body, slug);
        if found.is_empty() {
            continue;
        }
        c.commits += 1;
        for n in found {
            if !c.issues.contains(&n) {
                c.issues.push(n);
            }
        }
    }
    c.issues.sort_unstable();
    c
}

/// What [`citations`] counted.
#[derive(Debug, Default, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Citations {
    /// Commits whose message cites the template's tracker at least once.
    pub commits: usize,
    /// The distinct issue numbers cited, ascending.
    pub issues: Vec<u64>,
}

/// Whether a repository-relative path falls outside every directory `directories.md` lays out.
pub fn is_unlaid(path: &str) -> bool {
    let top = path.split('/').next().unwrap_or(path);
    !LAID_OUT.contains(&top)
}

/// `paths` counted by their first segment, in path order: `mirror/` (121), `corpora.toml` (1).
/// A directory is shown with its trailing slash, so a file and a directory cannot be confused.
pub fn group(paths: &[String]) -> Vec<(String, usize)> {
    let mut out: std::collections::BTreeMap<String, usize> = Default::default();
    for p in paths {
        let key = match p.split_once('/') {
            Some((top, _)) => format!("{top}/"),
            None => p.clone(),
        };
        *out.entry(key).or_default() += 1;
    }
    out.into_iter().collect()
}

/// A `from:derived-repo` issue body in the shape `directories.md` says makes a report
/// actionable, with what this repository can state filled in and the rest left to a person.
///
/// The pin and the paths are facts the tool holds. *The file and line in the prelude or CLI*
/// and *what it cost here* are not, and are left as prompts rather than guessed: the second is
/// the part `directories.md` calls the one upstream cannot reconstruct. No corpus content is
/// included — only paths, which the person sending it can still trim.
pub fn draft(slug: &str, pin: Option<&str>, phase: &str, unlaid: &[String]) -> String {
    let mut out = String::new();
    out.push_str(
        "## What is wrong\n\n\
         <The prelude, CLI or convention that did not fit, with its file and line.>\n\n\
         ## What it cost here\n\n\
         <Commits spent, checks that passed while wrong.>\n\n\
         ## The workaround\n\n",
    );
    let _ = std::fmt::Write::write_fmt(
        &mut out,
        format_args!("Built in phase `{phase}`, which added outside the laid-out directories:\n\n"),
    );
    for (dir, n) in group(unlaid) {
        let _ = std::fmt::Write::write_fmt(&mut out, format_args!("- `{dir}` ({n} file(s))\n"));
    }
    out.push_str("\n## Pin\n\n```\n");
    match pin {
        Some(c) => {
            let _ = std::fmt::Write::write_fmt(&mut out, format_args!("commit = \"{c}\"\n"));
        }
        None => out.push_str("<.yidam.toml records no commit>\n"),
    }
    let _ = std::fmt::Write::write_fmt(
        &mut out,
        format_args!(
            "```\n\nCite this issue as `{slug}#<n>` in the commit that carries the workaround, so \
             it can be found and removed when the fix lands.\n"
        ),
    );
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_draft_carries_the_pin_and_the_paths_and_leaves_the_cost_to_a_person() {
        let unlaid = vec![
            "mirror/a.ont.yml".to_string(),
            "mirror/b.ont.yml".to_string(),
            "corpora.toml".to_string(),
        ];
        let d = draft(
            "goedelsoup/yidam",
            Some("6ca345d"),
            "sibling-reads",
            &unlaid,
        );
        assert!(d.contains("commit = \"6ca345d\""), "{d}");
        assert!(d.contains("- `mirror/` (2 file(s))"), "{d}");
        assert!(d.contains("- `corpora.toml` (1 file(s))"), "{d}");
        assert!(d.contains("phase `sibling-reads`"), "{d}");
        assert!(d.contains("## What it cost here\n\n<"), "{d}");
        assert!(d.contains("goedelsoup/yidam#<n>"), "{d}");
        // Every heading directories.md asks an actionable report for is present.
        for h in [
            "What is wrong",
            "What it cost here",
            "The workaround",
            "Pin",
        ] {
            assert!(d.contains(&format!("## {h}")), "missing {h}");
        }
        let unpinned = draft("goedelsoup/yidam", None, "p", &unlaid);
        assert!(unpinned.contains("records no commit"), "{unpinned}");
    }

    #[test]
    fn slugs_come_from_every_origin_form_the_pin_has_been_written_with() {
        for (origin, want) in [
            (
                "git@github.com:goedelsoup/yidam.git",
                Some("goedelsoup/yidam"),
            ),
            (
                "https://github.com/goedelsoup/yidam.git",
                Some("goedelsoup/yidam"),
            ),
            (
                "https://github.com/GoedelSoup/Yidam/",
                Some("goedelsoup/yidam"),
            ),
            (
                "ssh://git@github.com/someone/fork.git",
                Some("someone/fork"),
            ),
            ("unknown", None),
            ("", None),
            ("/Users/me/Code/yidam", None),
            ("C:/src/yidam", None),
        ] {
            assert_eq!(slug_of(origin).as_deref(), want, "{origin}");
        }
    }

    #[test]
    fn a_citation_names_the_templates_tracker_and_a_bare_number_does_not() {
        let msg = "fix: carry the mirror until goedelsoup/yidam#1466 lands\n\n\
                   Refs #12 (ours). See https://github.com/goedelsoup/yidam/issues/1469 and \
                   https://github.com/goedelsoup/yidam/pull/1472, and again Goedelsoup/Yidam#1466.";
        assert_eq!(cited(msg, "goedelsoup/yidam"), vec![1466, 1469, 1472]);
        assert!(cited("Refs #1469", "goedelsoup/yidam").is_empty());
    }

    #[test]
    fn a_longer_name_ending_in_the_slug_is_not_a_citation() {
        assert!(cited("notgoedelsoup/yidam#3", "goedelsoup/yidam").is_empty());
        assert!(cited("goedelsoup/yidam-fork#3", "goedelsoup/yidam").is_empty());
        assert!(cited("goedelsoup/yidam#", "goedelsoup/yidam").is_empty());
        assert_eq!(cited("(goedelsoup/yidam#3)", "goedelsoup/yidam"), vec![3]);
    }

    #[test]
    fn unlaid_paths_are_those_outside_every_section() {
        assert!(!is_unlaid(".yidam/corpus/claim/x.yml"));
        assert!(!is_unlaid("crates/ground/src/lib.rs"));
        assert!(!is_unlaid(".yidam.toml"));
        assert!(is_unlaid("mirror/claim.ont.yml"));
        assert!(is_unlaid("corpora.toml"));
        assert!(is_unlaid("tools/fetch.sh"));
    }

    fn directories_md() -> String {
        let p = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../prelude/guidelines/directories.md"
        );
        std::fs::read_to_string(p).expect("directories.md is readable from the crate")
    }

    /// The first path segment of every `## \`…\`` heading in `directories.md`.
    fn sections(doc: &str) -> std::collections::BTreeSet<String> {
        doc.lines()
            .filter_map(|l| l.strip_prefix("## `"))
            .filter_map(|l| l.split('`').next())
            .map(|p| {
                p.trim_end_matches('/')
                    .split('/')
                    .next()
                    .unwrap_or(p)
                    .to_string()
            })
            .collect()
    }

    #[test]
    fn laid_out_is_the_documents_sections_both_ways() {
        let doc = directories_md();
        let headed = sections(&doc);
        // Floor first: a parser that read no headings would agree with an empty list.
        assert!(
            headed.len() >= 8,
            "read only {headed:?} from directories.md"
        );
        let listed: std::collections::BTreeSet<String> = LAID_OUT
            .iter()
            .chain(TRANSIENT)
            .map(|s| (*s).to_string())
            .collect();
        assert_eq!(
            headed, listed,
            "LAID_OUT + TRANSIENT must be exactly directories.md's top-level sections"
        );
    }

    #[test]
    fn the_default_slug_and_label_are_the_ones_the_guideline_files_with() {
        let doc = directories_md();
        assert!(
            doc.contains(&format!(
                "gh issue create --repo {DEFAULT_SLUG} --label \"{LABEL}\""
            )),
            "directories.md no longer files against {DEFAULT_SLUG} with {LABEL}"
        );
    }
}
