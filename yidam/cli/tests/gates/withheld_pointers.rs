//! Documents this binary tells a reader to open, and whether that reader can open them.
//!
//! [`yidam::NOT_INHERITED`] is the list of top-level paths `yidam clone` withholds: yidam's
//! own scaffolding, which a derived repository is not supposed to receive. `docs` is one
//! entry and the whole directory. So a string printed by the binary that ends *"see
//! docs/configuration.md"* names a file that is present in exactly one of the two
//! repositories this binary runs in, and absent in the one that is far more likely to be
//! reading the sentence.
//!
//! It is worse than a dangling link, which is why this is a gate rather than a lint. The
//! path is repo-relative and a derived repository has a `docs/` of its own, so the reader
//! does not land on "no such file" from somewhere obviously foreign — they land in their
//! own documentation tree, where the answer is supposed to be and demonstrably is not, and
//! conclude that *they* failed to write a file they owed. #817 is one such report, and the
//! decision record it cost ran a trial config and read the output back to recover one word
//! that a shipped table already states.
//!
//! [`cmd::doctor`]'s `every_doctor_question_is_in_the_troubleshooting_table` already asks
//! the forward question — *is every check `doctor` runs in the document?* This asks the
//! converse, which nothing did: **of every document a user-facing string names, does it
//! reach the reader who is shown it?** Both sides are discovered. The withheld set is the
//! constant `clone` copies against, not a copy of it; the strings are read out of the
//! source, because a hardcoded list of strings is the same defect one level out — it stops
//! covering what is new without ever going red.
//!
//! # What counts as a pointer
//!
//! A string literal in production code naming a path that ends in `.md`. Each half of that
//! is load-bearing:
//!
//! - **String literals, not the file.** Doc comments name withheld paths constantly and
//!   correctly — this module's own prose names `docs/configuration.md` twice — and they are
//!   read by people standing in this repository. Only what the binary can print is in
//!   scope, so [`string_literals`] is a scanner over the grammar rather than a grep.
//! - **`.md`, not any path.** A pointer is to a document, which is a thing a reader opens.
//!   `schema` prints *"add to .vscode/settings.json"*, and an example of path-component
//!   matching (`docs/ref` does not cover `docs/reference/`); neither claims
//!   a document exists to be read, and neither is a defect. The extension is the line that
//!   separates them without an allowlist of excused strings.
//! - **Production, not tests.** Fixture corpora are full of `docs/x.md`.

use std::collections::BTreeMap;
use std::path::PathBuf;

use walkdir::WalkDir;

fn cli_src() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src")
}

/// The contents of every string literal in `src`, comments and char literals skipped.
///
/// Rust's string grammar, not a regular expression over it: raw strings hold what a
/// non-raw string would escape, `"//"` is not the start of a comment, and `'"'` is not the
/// start of a string. A line-continuation backslash is folded the way the compiler folds
/// it, so a message split across source lines is matched as the one sentence it prints —
/// the string this file was written for is wrapped mid-path.
fn string_literals(src: &str) -> Vec<String> {
    let b: Vec<char> = src.chars().collect();
    let n = b.len();
    let mut out = Vec::new();
    let mut i = 0;
    while i < n {
        // Line comment.
        if b[i] == '/' && i + 1 < n && b[i + 1] == '/' {
            while i < n && b[i] != '\n' {
                i += 1;
            }
            continue;
        }
        // Block comment. Rust nests them.
        if b[i] == '/' && i + 1 < n && b[i + 1] == '*' {
            let mut depth = 0;
            while i < n {
                if b[i] == '/' && i + 1 < n && b[i + 1] == '*' {
                    depth += 1;
                    i += 2;
                } else if b[i] == '*' && i + 1 < n && b[i + 1] == '/' {
                    depth -= 1;
                    i += 2;
                    if depth == 0 {
                        break;
                    }
                } else {
                    i += 1;
                }
            }
            continue;
        }
        // Raw string: `r"…"`, `r#"…"#`, `r##"…"##`.
        if b[i] == 'r' && i + 1 < n && (b[i + 1] == '"' || b[i + 1] == '#') {
            let mut j = i + 1;
            let mut hashes = 0;
            while j < n && b[j] == '#' {
                hashes += 1;
                j += 1;
            }
            if j < n && b[j] == '"' {
                j += 1;
                let start = j;
                while j < n {
                    if b[j] == '"' && b[j + 1..].iter().take(hashes).all(|c| *c == '#') {
                        break;
                    }
                    j += 1;
                }
                out.push(b[start..j.min(n)].iter().collect());
                i = (j + 1 + hashes).min(n);
                continue;
            }
        }
        if b[i] == '"' {
            i += 1;
            let mut buf = String::new();
            while i < n {
                if b[i] == '\\' {
                    // `\` before a newline eats the break and the indent after it.
                    if i + 1 < n && b[i + 1] == '\n' {
                        i += 2;
                        while i < n && (b[i] == ' ' || b[i] == '\t') {
                            i += 1;
                        }
                        continue;
                    }
                    buf.push(b[i]);
                    if i + 1 < n {
                        buf.push(b[i + 1]);
                    }
                    i += 2;
                    continue;
                }
                if b[i] == '"' {
                    i += 1;
                    break;
                }
                buf.push(b[i]);
                i += 1;
            }
            out.push(buf);
            continue;
        }
        // `'` is a char literal or a lifetime, and only the first can hide a `"`.
        if b[i] == '\'' {
            if i + 1 < n && b[i + 1] == '\\' {
                i += 2;
                while i < n && b[i] != '\'' {
                    i += 1;
                }
                i += 1;
                continue;
            }
            if i + 2 < n && b[i + 2] == '\'' {
                i += 3;
                continue;
            }
            i += 1;
            continue;
        }
        i += 1;
    }
    out
}

/// `src` with every `#[cfg(test)]` item removed, by brace matching.
///
/// The same shape `panic_paths` uses, for the same reason: unit tests live inside the files
/// they test, and their fixtures name documents freely.
fn without_test_items(src: &str) -> String {
    let b: Vec<char> = src.chars().collect();
    let n = b.len();
    let mut out = String::new();
    let mut i = 0;
    while i < n {
        if b[i..].iter().take(12).collect::<String>() == "#[cfg(test)]" {
            let mut j = i;
            while j < n && b[j] != '{' && b[j] != ';' {
                j += 1;
            }
            if j < n && b[j] == ';' {
                i = j + 1; // `#[cfg(test)] mod tests;` — nothing inline to skip.
                continue;
            }
            let mut depth = 0;
            while j < n {
                match b[j] {
                    '{' => depth += 1,
                    '}' => {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    }
                    _ => {}
                }
                j += 1;
            }
            i = j + 1;
            continue;
        }
        out.push(b[i]);
        i += 1;
    }
    out
}

/// Every `*.md` path named by a string literal in `text`.
///
/// A path is the run of path characters ending in `.md`, bounded on the left so that
/// `../../docs/x.md` yields the whole thing and not a suffix of it — the first component is
/// what decides inheritance, and reading it wrong is the difference between a verdict and a
/// false one.
///
/// Trailing `.` and `/` are trimmed before the extension is read, because a pointer is
/// usually the last thing in a sentence and `.` is a path character. Not trimming them is
/// how the first draft of this scanner returned nothing for the exact string it was
/// written to catch — *"see docs/configuration.md."* — and passed.
fn md_paths(text: &str) -> Vec<String> {
    const OK: fn(char) -> bool =
        |c: char| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | '/');
    let chars: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let mut start = None;
    for i in 0..=chars.len() {
        let is_path = i < chars.len() && OK(chars[i]);
        match (is_path, start) {
            (true, None) => start = Some(i),
            (false, Some(s)) => {
                let run: String = chars[s..i].iter().collect();
                let run = run.trim_end_matches(['.', '/']).to_string();
                if run.ends_with(".md") && run.len() > 3 {
                    out.push(run);
                }
                start = None;
            }
            _ => {}
        }
    }
    out
}

/// The component `clone` matches a path on: the first that names something.
///
/// `clone` excludes top-level entries by name, so `sadhana/docs/x.md` is a different
/// directory at a different depth and is delivered. A `./` or `../` prefix names no
/// directory and must not be mistaken for one, or a relative pointer reads as innocent.
fn leading_component(path: &str) -> &str {
    path.split('/')
        .find(|c| !c.is_empty() && *c != "." && *c != "..")
        .unwrap_or_default()
}

/// **Every document the binary points a reader at is a document that reader has.**
///
/// The failure this catches is silent in both directions: the string compiles, the file it
/// names exists *here*, and the only person who learns otherwise is standing in a
/// repository that cannot report it back. #817 took four commits past a release to arrive
/// and had been true since `424a7a3`.
#[test]
fn no_user_facing_string_points_at_a_document_clone_withholds() {
    let mut offenders: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut scanned = 0usize;
    let mut pointers = 0usize;

    for entry in WalkDir::new(cli_src()).into_iter().filter_map(Result::ok) {
        let path = entry.path();
        if path.extension().is_none_or(|e| e != "rs") {
            continue;
        }
        // Sibling test modules, excluded by name the way `panic_paths` excludes them:
        // `#[cfg(test)] mod tests;` leaves nothing inline to brace-match.
        if path.file_name().is_some_and(|f| f == "tests.rs") {
            continue;
        }
        scanned += 1;
        let src = std::fs::read_to_string(path).expect("read source");
        let rel = path
            .strip_prefix(cli_src())
            .unwrap_or(path)
            .display()
            .to_string();

        for literal in string_literals(&without_test_items(&src)) {
            for p in md_paths(&literal) {
                pointers += 1;
                // The first component is what `clone` matches on — it excludes top-level
                // entries by name, so `sadhana/docs/x.md` is a different directory and is
                // delivered.
                if yidam::NOT_INHERITED.contains(&leading_component(&p)) {
                    offenders.entry(p).or_default().push(rel.clone());
                }
            }
        }
    }

    // The paired assertion: a scanner that found nothing would pass the one below. #672's
    // lesson — a file-scanning test that looks at nothing is green.
    assert!(
        scanned > 50,
        "walked {scanned} source files — the scan is not reaching src/"
    );
    assert!(
        pointers > 10,
        "found {pointers} document pointers in {scanned} files — the literal scanner has \
         stopped reading strings, and this test now asserts nothing"
    );

    assert!(
        offenders.is_empty(),
        "these strings point a reader at a document `yidam clone` withholds \
         (NOT_INHERITED): {offenders:#?}\n\n\
         A derived repository has a `docs/` of its own, so the reader lands in their own \
         tree and reads the miss as a file they failed to write. Say the thing instead of \
         naming the document — `due` names the config key, which is true in both \
         repositories — or, if the document really must travel, vendor it under \
         `prelude/` and name it at its vendored path."
    );
}

/// The scanner, on the constructs that would quietly break it.
///
/// It is a hand-written lexer standing between a real defect and a green test, so the
/// shapes it has to get right are asserted rather than assumed.
#[test]
fn the_literal_scanner_reads_rust_and_not_prose() {
    // A doc comment naming a withheld document is not a pointer — this file has several.
    assert!(string_literals("/// see docs/configuration.md\nlet x = 1;").is_empty());
    assert!(string_literals("// docs/configuration.md").is_empty());
    assert!(string_literals("/* docs/a.md /* nested */ still */").is_empty());

    // A wrapped message is one string, and the wrap can fall inside the path.
    let folded = string_literals("\"see docs/config\\\n             uration.md.\"");
    assert_eq!(folded, vec!["see docs/configuration.md."]);
    assert_eq!(md_paths(&folded[0]), vec!["docs/configuration.md"]);

    // `//` inside a string is not a comment; a `"` inside a char literal is not a string.
    assert_eq!(
        string_literals("let u = \"https://x/docs/a.md\"; let q = '\"'; let r = 1;"),
        vec!["https://x/docs/a.md"]
    );
    assert_eq!(
        string_literals("fn f<'a>(s: &'a str) {}"),
        Vec::<String>::new()
    );
    assert_eq!(
        string_literals("r#\"docs/\"a\".md\"#"),
        vec!["docs/\"a\".md"]
    );

    // Paths are read whole: the first component decides, so a prefix must not be dropped.
    assert_eq!(md_paths("../../docs/a.md"), vec!["../../docs/a.md"]);
    assert_eq!(leading_component("../../docs/a.md"), "docs");
    assert_eq!(leading_component("sadhana/docs/a.md"), "sadhana");
    assert_eq!(md_paths("nothing here"), Vec::<String>::new());
    assert_eq!(md_paths("a.mdx b.md"), vec!["b.md"]);
    // A pointer is usually the last thing in a sentence, and `.` is a path character.
    assert_eq!(md_paths("see docs/a.md."), vec!["docs/a.md"]);
}
