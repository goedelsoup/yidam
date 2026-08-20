//! Every commit message the template *tells an agent to run* must use a verb the template's
//! own lint recognizes.
//!
//! This is the drift that motivated the test: `bootstrap.md` prescribed
//! `consume(samudaya):` and `vendor(yidam):` in conventional-commits style, while
//! `verb_of` in `lint::commits` takes everything before the first `": "` as the verb. So the
//! verb was `consume(samudaya)`, which is in no list — `yidam lint --commits` reported the
//! first three commits of every derived repository, and `classify_commit` filed two
//! operational commits as Epistemic. Nothing failed loudly; the check is Warn severity and
//! history cannot be rewritten to fix a verb, so the defect survived in the template.
//!
//! Prose *about* a commit is not checked — only text an agent copies verbatim into a shell.

use std::path::{Path, PathBuf};

use walkdir::WalkDir;
use yidam_core::git::is_recognized_verb;

fn repo_root() -> PathBuf {
    // CARGO_MANIFEST_DIR = yidam/cli/
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// A `git commit … -m <quoted>` occurrence: the subject, and where it was written.
struct Prescribed {
    file: String,
    line: usize,
    subject: String,
}

/// Pull the `-m`-quoted subject out of every line that invokes `git commit` or `git merge`.
///
/// `git merge` is scanned for the same reason `git commit` is: a merge whose subject the
/// author supplies is checked by the lint like any other commit, so a template that
/// prescribes one is prescribing a verb. A bare `git merge --no-ff <ref>` writes no subject
/// and matches nothing here, which is correct — git's generated subject is exempt.
///
/// Deliberately line-scoped: a message that spans lines is not something an agent copies as
/// one shell command, and no template file writes one.
fn prescribed_commits(path: &Path, text: &str, root: &Path) -> Vec<Prescribed> {
    let file = path
        .strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string();
    text.lines()
        .enumerate()
        .filter_map(|(i, line)| {
            let start = ["git commit", "git merge"]
                .iter()
                .filter_map(|kw| line.find(kw))
                .min()?;
            let after_commit = &line[start..];
            let after_flag = after_commit.find("-m ").map(|p| &after_commit[p + 3..])?;
            let quote = after_flag.chars().next()?;
            if quote != '"' && quote != '\'' {
                return None;
            }
            let body = &after_flag[quote.len_utf8()..];
            let end = body.find(quote)?;
            Some(Prescribed {
                file: file.clone(),
                line: i + 1,
                subject: body[..end].to_string(),
            })
        })
        .collect()
}

/// Matches `verb_of` in `lint::commits` — everything before the first `": "`.
fn verb_of(subject: &str) -> &str {
    match subject.find(": ") {
        Some(pos) => subject[..pos].trim(),
        None => "",
    }
}

/// Byte ranges of every `"…"` span in `text`.
///
/// Used to tell a document *discussing* a forbidden phrasing from one prescribing it.
fn quoted_spans(text: &str) -> Vec<(usize, usize)> {
    let mut spans = Vec::new();
    let mut open: Option<usize> = None;
    for (i, c) in text.char_indices() {
        if c == '"' {
            match open {
                Some(start) => {
                    spans.push((start, i + 1));
                    open = None;
                }
                None => open = Some(i),
            }
        }
    }
    spans
}

/// A step may not tell a reader to make "an epistemic commit" and leave them to guess
/// which one.
///
/// The kind is *derived* from the verb — `classify_commit` reads the verb and returns
/// Epistemic or Operational — so naming the kind is naming the output of a function
/// instead of its input. A reader given the kind still has to pick a verb, and the whole
/// value of a closed vocabulary is that they do not have to.
///
/// Found by audit: `bootstrap.md` prescribed four commits this way, including two in
/// consecutive sentences ("as a single epistemic commit … as a single operational
/// commit"). Every one had an obvious right verb that nobody had written down.
///
/// Deliberately narrow. It cannot see a step that says "commit this" with no
/// qualification at all, and no check can — that is what review is for. It catches the
/// one shape that reads as though it *has* named something.
#[test]
fn no_step_names_a_commit_kind_instead_of_its_verb() {
    let root = repo_root();
    let targets = [
        root.join("yidam/prelude"),
        root.join("sadhana"),
        root.join("mise.yidam.toml"),
    ];
    // Prose *about* the split is legitimate and common — GRAPH.md defines both words, and
    // the conventions discuss them. Only an instruction to produce one is a finding, so
    // the match requires the imperative shape: "commit … as a[n] <kind> commit".
    let shapes = [
        "as an epistemic commit",
        "as a single epistemic commit",
        "as an operational commit",
        "as a single operational commit",
    ];

    let mut bad = Vec::new();
    let mut excused = 0usize;
    for target in &targets {
        for entry in WalkDir::new(target).into_iter().filter_map(Result::ok) {
            if !entry.file_type().is_file() {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(entry.path()) else {
                continue;
            };
            let file = entry
                .path()
                .strip_prefix(&root)
                .unwrap_or(entry.path())
                .display()
                .to_string();
            // Join wrapped lines: these documents hard-wrap at 90 columns, so the phrase
            // is routinely split across two lines and a line-scoped scan misses it.
            let flat = text.replace('\n', " ");
            let quoted = quoted_spans(&flat);
            for shape in shapes {
                let mut from = 0;
                while let Some(rel) = flat[from..].find(shape) {
                    let at = from + rel;
                    from = at + shape.len();
                    // A document that DEFINES the rule has to be able to quote what it
                    // forbids. The exemption is the quotation marks, not the filename:
                    // an occurrence inside a "…" span is discussing the shape, one
                    // outside is prescribing it. GRAPH.md's paragraph on this rule is the
                    // reason the exemption exists, and it survives being moved.
                    if quoted
                        .iter()
                        .any(|(s, e)| at >= *s && at + shape.len() <= *e)
                    {
                        excused += 1;
                        continue;
                    }
                    bad.push(format!("  {file} — \"{shape}\""));
                }
            }
        }
    }

    // The exemption is measured, not assumed: if nothing is being excused, the quotation
    // carve-out is dead code covering whatever comes next, and this says so.
    assert!(
        excused > 0,
        "no quoted occurrence found — the quotation exemption is excusing nothing. \
         Delete it rather than leaving it to cover something else later."
    );

    assert!(
        bad.is_empty(),
        "a step names the commit KIND instead of its verb:\n{}\n\
         The kind is derived from the verb, so this leaves the reader to pick one. Name \
         the verb beside the instruction — see GRAPH.md, \"Commit vocabulary\".",
        bad.join("\n")
    );
}

#[test]
fn every_prescribed_commit_uses_a_recognized_verb() {
    let root = repo_root();
    // The three places that prescribe commands an agent runs: the vendored prelude, the
    // scaffold template a derived repo is built from, and the inherited task layer.
    let targets = [
        root.join("yidam/prelude"),
        root.join("sadhana"),
        root.join("mise.yidam.toml"),
    ];

    let mut found = Vec::new();
    for target in &targets {
        for entry in WalkDir::new(target).into_iter().filter_map(Result::ok) {
            if !entry.file_type().is_file() {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(entry.path()) else {
                continue; // not UTF-8 — no shell commands in it
            };
            found.extend(prescribed_commits(entry.path(), &text, &root));
        }
    }

    assert!(
        !found.is_empty(),
        "found no `git commit -m` at all under {targets:?} — the scan is broken, not the docs"
    );

    let bad: Vec<String> = found
        .iter()
        .filter(|p| !is_recognized_verb(verb_of(&p.subject)))
        .map(|p| {
            format!(
                "  {}:{} — `{}` is not in the vocabulary (subject: {:?})",
                p.file,
                p.line,
                verb_of(&p.subject),
                p.subject
            )
        })
        .collect();

    assert!(
        bad.is_empty(),
        "template prescribes commits its own lint rejects:\n{}\n\
         The verb is everything before the first `: ` — a `(scope)` suffix makes it \
         unrecognizable and misclassifies the commit. See GRAPH.md, \"Commit vocabulary\".",
        bad.join("\n")
    );
}

#[test]
fn a_scoped_verb_is_caught() {
    // The exact form bootstrap.md used to prescribe.
    let root = Path::new("/repo");
    let found = prescribed_commits(
        &root.join("skills/bootstrap.md"),
        "git commit -m \"vendor(yidam): prelude into .yidam/.vendor/\"",
        root,
    );
    assert_eq!(found.len(), 1);
    assert_eq!(verb_of(&found[0].subject), "vendor(yidam)");
    assert!(!is_recognized_verb(verb_of(&found[0].subject)));
    // …and the bare form it prescribes now.
    assert!(is_recognized_verb(verb_of("vendor: yidam prelude into …")));
}

#[test]
fn the_scan_reads_both_quote_styles_and_extra_flags() {
    let root = Path::new("/repo");
    let text = "git commit --allow-empty -m \"consume: samudaya — ...\"\n\
                echo \"  git commit -m 'vendor: re-vendor prelude at $commit'\"\n\
                this line merely mentions the vendor: commit in prose\n";
    let found = prescribed_commits(&root.join("f.md"), text, root);
    assert_eq!(found.len(), 2, "prose must not be picked up");
    assert_eq!(found[0].subject, "consume: samudaya — ...");
    assert_eq!(found[1].subject, "vendor: re-vendor prelude at $commit");
    assert_eq!(found[1].line, 2);
}
