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
