//! Every recorded transcript in a walkthrough is re-run, and must still say what it says.
//!
//! The walkthrough pages are written as transcripts: a `$ yidam …` line and the output it
//! produced against the example the page names. Twenty-odd of them, and until this file
//! existed nothing re-ran a single one. A transcript is a hardcoded claim written beside the
//! thing that produces it, which is the shape #448 found in `example_corpus` — it stops
//! matching without ever going red.
//!
//! It had already happened. #456 added one instance to `examples/journalism/`; the page's
//! opening `graph-check` block still said eleven, the same page's closing block said twelve,
//! and `example_corpus` asserted twelve against the ontology. The stale line sat between two
//! things that knew better and `mise run ci-cli` was green (#493).
//!
//! # Discovered, not listed
//!
//! No page, corpus or command is named here. The pages come from `git ls-files`, each page's
//! corpus comes from the page's own `../../examples/<name>/` link, and the commands come from
//! parsing the fences. A fourth walkthrough is covered the day it is written.
//!
//! Both directions of the hole are closed, because only one of them is the obvious one:
//!
//! - a page that names an example — **every** ```console fence must be a runnable transcript,
//!   so a block cannot drop its `$` line and quietly stop being checked;
//! - a page that names none — must carry **no** ```console fence at all, so a transcript
//!   added to one of the three seed-set walkthroughs cannot be unchecked by being somewhere
//!   this file does not look.
//!
//! # What is redacted, and what is not
//!
//! A commit sha, the `--at` timestamp, SigV4's `x-amz-date` and an absolute path rooted at a
//! machine directory belong to the run rather than to the corpus, and are replaced on **both**
//! sides — the page's and the fresh output's. Nothing else is. A content hash is not
//! run-varying and stays checked in full, which is why the hex rule is bounded by length
//! rather than by shape: the 64-character hashes in the vault transcript are compared
//! literally, and so is the canonical request path they are signed with.
//!
//! # The page declares its own environment
//!
//! `vault push --dry-run` signs a request, so it needs credentials, a cache and an artifact
//! in it. Rather than carry that setup here as a per-page special case, a page marks a `sh`
//! fence with `<!-- transcript-setup -->` and this runs it — in the corpus, at the point on
//! the page where it appears, keeping whatever it exports. Blocks are walked in page order
//! for that reason: a transcript above the setup is checked without it, which is what the
//! reader gets there. So the block a reader copies is the block that produced the output
//! below it, and a page whose setup instructions have gone stale fails here rather than
//! under the reader.
//!
//! # Updating
//!
//! `UPDATE_TRANSCRIPTS=1 cargo test --test walkthrough_transcripts` rewrites every block in
//! place from a fresh run, following `query_goldens`' `UPDATE_GOLDENS=1`. The page is the
//! golden; the diff is the review.

mod common;

use std::collections::BTreeMap;
use std::path::Path;

use common::{examples, repo_root, tracked_under, Example};

const PAGES: &str = "docs/walkthroughs/";

/// The marker that makes a `sh` fence executable rather than illustrative.
///
/// Explicit because both kinds appear on one page: `property-research.md` introduces a query
/// in a `sh` fence it does not want run twice, and `investigative-journalism.md` has to set
/// up a cache before a vault transcript means anything.
const SETUP: &str = "<!-- transcript-setup -->";

// ── the page ──────────────────────────────────────────────────────────────────

/// One `$ …` line and the output recorded under it.
#[derive(Debug, Clone)]
struct Transcript {
    command: String,
    output: String,
}

/// A fenced block, with enough of its surroundings to put it back.
struct Fence {
    lang: String,
    /// Byte range of the block's *content*, exclusive of the fence lines.
    body: std::ops::Range<usize>,
    text: String,
    /// Whether [`SETUP`] appears on the line before the opening fence.
    marked_setup: bool,
}

/// Every fenced block on a page, in order.
///
/// Deliberately literal: an opening fence is a line beginning with three backticks and a
/// closing fence is a line that is exactly three backticks. Nothing in `docs/` nests fences,
/// and a parser that guessed would be the thing this file is here to prevent.
fn fences(page: &str) -> Vec<Fence> {
    let mut out = Vec::new();
    let mut i = 0;
    let mut prev_line = "";
    while i < page.len() {
        let end = page[i..].find('\n').map(|n| i + n).unwrap_or(page.len());
        let line = &page[i..end];
        if let Some(lang) = line.strip_prefix("```") {
            let body_start = (end + 1).min(page.len());
            let mut j = body_start;
            let close = loop {
                if j >= page.len() {
                    panic!("unterminated ``` fence in a walkthrough page");
                }
                let e = page[j..].find('\n').map(|n| j + n).unwrap_or(page.len());
                if &page[j..e] == "```" {
                    break j;
                }
                j = (e + 1).min(page.len());
            };
            out.push(Fence {
                lang: lang.trim().to_string(),
                body: body_start..close,
                text: page[body_start..close].to_string(),
                marked_setup: prev_line.trim() == SETUP,
            });
            // Past the closing fence line, not onto it: `close` points at its first
            // backtick, and re-reading it would open a block with an empty language.
            i = page[close..]
                .find('\n')
                .map(|n| close + n + 1)
                .unwrap_or(page.len());
            prev_line = "```";
            continue;
        }
        prev_line = line;
        i = (end + 1).min(page.len());
    }
    out
}

/// Split a console block into the commands it records.
///
/// A transcript begins at a `$ ` line and runs to the next one. Trailing blank lines are the
/// separator between transcripts rather than part of the output, so they are dropped here and
/// re-inserted when a block is rewritten — a `vault push` transcript has blank lines *inside*
/// it and those are kept.
fn transcripts(block: &str) -> Vec<Transcript> {
    let mut out: Vec<Transcript> = Vec::new();
    for line in block.split('\n') {
        match line.strip_prefix("$ ") {
            Some(cmd) => out.push(Transcript {
                command: cmd.trim().to_string(),
                output: String::new(),
            }),
            None => {
                if let Some(last) = out.last_mut() {
                    // Trailing spaces are normalised away here rather than at comparison, so
                    // a block rewritten by UPDATE_TRANSCRIPTS cannot keep whitespace that
                    // compared equal and would be stripped by the next editor to open it.
                    last.output.push_str(line.trim_end());
                    last.output.push('\n');
                }
            }
        }
    }
    for t in &mut out {
        while t.output.ends_with("\n\n") {
            t.output.pop();
        }
    }
    out
}

/// Render transcripts back into a console block, in the shape [`transcripts`] reads.
fn render(ts: &[Transcript]) -> String {
    ts.iter()
        .map(|t| format!("$ {}\n{}", t.command, t.output))
        .collect::<Vec<_>>()
        .join("\n")
}

/// The example corpus a page is written against, from the page's own link.
///
/// `../../examples/<name>/` is how a page under `docs/walkthroughs/` reaches one, and the
/// name has to be an example this repository actually ships — the three seed-set walkthroughs
/// mention `samudaya/examples/<name>/`, which is a directory of markdown and not a corpus.
fn corpus_of(page: &str, known: &[String]) -> Option<String> {
    let mut found: Vec<String> = Vec::new();
    for name in known {
        if page.contains(&format!("../../examples/{name}/")) {
            found.push(name.clone());
        }
    }
    assert!(
        found.len() <= 1,
        "a walkthrough links more than one example corpus ({found:?}); a transcript would \
         have no unambiguous repository to run in"
    );
    found.pop()
}

// ── running ───────────────────────────────────────────────────────────────────

/// Split a recorded command line into argv, honouring the quoting the pages use.
///
/// Single and double quotes, no escapes and no expansion: a query is written
/// `'parcel <-conveys- instrument'` and nothing on any page needs more. Anything richer
/// belongs in a setup fence, which is handed to a real shell.
fn argv(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quote: Option<char> = None;
    let mut started = false;
    for c in line.chars() {
        match quote {
            Some(q) if c == q => quote = None,
            Some(_) => cur.push(c),
            None if c == '\'' || c == '"' => {
                quote = Some(c);
                started = true;
            }
            None if c.is_whitespace() => {
                if started || !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                    started = false;
                }
            }
            None => cur.push(c),
        }
    }
    assert!(quote.is_none(), "unbalanced quote in transcript: {line}");
    if started || !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// Run a page's setup fence in the corpus and keep what it exported.
///
/// The environment is read back with `env -0` and diffed against the same shell without the
/// setup, so a variable the page sets is picked up without this needing to know its name —
/// and `TMPDIR` is pointed at the workspace so a setup that asks for a scratch directory gets
/// one that is cleaned up with it.
fn run_setup(ex: &Example, script: &str, base: &[(String, String)]) -> Vec<(String, String)> {
    let read_env = |body: &str| -> BTreeMap<String, String> {
        let out = std::process::Command::new("sh")
            .arg("-c")
            .arg(format!("set -e\n{body}\nenv -0"))
            .current_dir(ex.path())
            .envs(base.iter().map(|(k, v)| (k.as_str(), v.as_str())))
            .env("TMPDIR", ex.path().parent().unwrap())
            // The binary under test, not whatever `yidam` a developer has installed: a setup
            // fence calls `yidam vault put`, and the reader's copy of that line resolves to
            // the release they have. Resolving it here to anything but this build would make
            // the transcripts evidence about somebody else's binary.
            .env(
                "PATH",
                format!(
                    "{}:{}",
                    Path::new(env!("CARGO_BIN_EXE_yidam"))
                        .parent()
                        .unwrap()
                        .display(),
                    std::env::var("PATH").unwrap_or_default()
                ),
            )
            .output()
            .expect("sh");
        assert!(
            out.status.success(),
            "a transcript-setup fence failed:\n{}\n--- stderr ---\n{}",
            body,
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout)
            .split('\0')
            .filter_map(|kv| kv.split_once('='))
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    };
    let before = read_env("");
    let after = read_env(script);
    after
        .into_iter()
        .filter(|(k, v)| before.get(k) != Some(v))
        .collect()
}

/// Replace what belongs to the run rather than to the corpus.
///
/// Applied identically to the page and to the fresh output, so a value that varies is not
/// checked and a value that does not still is. The hex rule is bounded at twelve characters
/// on purpose: git abbreviates to seven or eight, and the artifact hashes in the vault
/// transcript are sixty-four and are compared in full. Forty is included because that is the
/// unabbreviated form, and `redact_run` in `query_goldens` redacts it for the same reason.
fn redact(text: &str, root: &Path) -> String {
    let iso = regex::Regex::new(r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:[+-]\d{2}:\d{2}|Z)")
        .expect("iso");
    let amz = regex::Regex::new(r"\d{8}T\d{6}Z").expect("amz");
    let hex = regex::Regex::new(r"\b[0-9a-f]{7,12}\b|\b[0-9a-f]{40}\b").expect("hex");
    // An absolute path rooted at a machine directory. `vault list` prints where the artifact
    // cache is, and no page can record that literally — the recorded copy came from whoever
    // wrote the page. Anchored on the roots rather than on "starts with a slash" so the
    // canonical request in the SigV4 transcript, `/yidam/sha256/…`, is still compared: that
    // is the line the signature is over and the one worth checking.
    //
    // The root list can go stale, and when it does this test goes **red** — a path it does
    // not recognise is a path that fails to match. That is the acceptable direction for a
    // list to rot in.
    let abspath =
        regex::Regex::new(r"/(?:Users|home|root|var|tmp|private)/[^\s,)]*").expect("abspath");

    // Trailing spaces are not content. `vault push` pads its blank separator lines, and a
    // page that recorded them would carry whitespace an editor strips on the next save —
    // turning a formatting habit into a red gate.
    let text: String = text
        .split('\n')
        .map(|l| l.trim_end())
        .collect::<Vec<_>>()
        .join("\n");

    let canonical = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    let text = text
        .replace(&canonical.display().to_string(), "<ROOT>")
        .replace(&root.display().to_string(), "<ROOT>");
    let text = iso.replace_all(&text, "<TIME>").into_owned();
    let text = amz.replace_all(&text, "<AMZDATE>").into_owned();
    let text = abspath.replace_all(&text, "<PATH>").into_owned();
    hex.replace_all(&text, "<HEX>").into_owned()
}

/// Everything one command wrote, in the order a terminal would have shown it.
///
/// stdout then stderr, and every command on every page writes to one or the other — the
/// query engine's `[absent]` diagnosis is on stdout with its result count, and an audit's
/// findings are on stderr with nothing on stdout. A command that used both would interleave
/// wrongly here, and would be worth splitting on the page rather than papering over.
fn output_of(ex: &Example, command: &str, env: &[(String, String)]) -> String {
    let parts = argv(command);
    assert_eq!(
        parts.first().map(String::as_str),
        Some("yidam"),
        "a transcript records a command that is not yidam: {command}. Setup belongs in a \
         `{SETUP}` fence, which is run by a shell"
    );
    let args: Vec<&str> = parts[1..].iter().map(String::as_str).collect();
    let (stdout, stderr, _) = ex.run_with_env(&args, env);
    format!("{stdout}{stderr}")
}

// ── the checks ────────────────────────────────────────────────────────────────

fn pages() -> Vec<String> {
    let found: Vec<String> = tracked_under(&repo_root(), PAGES)
        .into_iter()
        .filter(|p| p.ends_with(".md"))
        .collect();
    assert!(
        !found.is_empty(),
        "no walkthrough pages under {PAGES} — every check in this file would pass while \
         checking nothing"
    );
    found
}

/// **Fix this one first if the suite goes quiet.**
///
/// Every transcript below is reached by parsing, so a parser that finds nothing checks
/// nothing and says so by passing. The count is derived a second way — by scanning the raw
/// bytes of the tracked pages for `$ ` lines, which knows nothing about fences — because one
/// computation compared against itself cannot disagree with anything.
#[test]
fn every_recorded_command_is_found_by_the_parser() {
    let root = repo_root();
    let mut parsed = 0usize;
    let mut scanned = 0usize;
    for rel in pages() {
        let page = std::fs::read_to_string(root.join(&rel)).unwrap();
        scanned += page.lines().filter(|l| l.starts_with("$ ")).count();
        for f in fences(&page).iter().filter(|f| f.lang == "console") {
            parsed += transcripts(&f.text).len();
        }
    }
    assert!(
        parsed > 0,
        "no transcripts parsed out of {PAGES} — the fence parser has stopped matching"
    );
    assert_eq!(
        parsed, scanned,
        "the fence parser found {parsed} recorded commands and a raw scan of the same pages \
         found {scanned}. A `$ ` line outside a ```console fence is a transcript nothing \
         re-runs"
    );
}

/// A page with no corpus may not carry a transcript.
///
/// The seed-set walkthroughs — genealogy, museum provenance, language documentation — teach
/// from `samudaya/examples/`, which is markdown rather than a corpus, and have nothing to run
/// a command against. Without this, a console block added to one of them would be unchecked
/// by being somewhere the runner does not look, which is the same silent hole from the other
/// side.
#[test]
fn a_walkthrough_with_no_corpus_records_no_transcript() {
    let root = repo_root();
    let known = examples();
    for rel in pages() {
        let page = std::fs::read_to_string(root.join(&rel)).unwrap();
        if corpus_of(&page, &known).is_some() {
            continue;
        }
        let blocks = fences(&page)
            .into_iter()
            .filter(|f| f.lang == "console")
            .count();
        assert_eq!(
            blocks, 0,
            "{rel} carries {blocks} ```console block(s) and links no `../../examples/<name>/` \
             corpus, so nothing can re-run them. Either link the corpus the transcript came \
             from, or render it as ```text — which says it is an illustration rather than a \
             run"
        );
    }
}

/// Every ```console block on a corpus-backed page is a transcript, and still says what it says.
///
/// One test rather than one per page: the set of pages is discovered, and a `#[test]` per
/// discovered item is not a thing Rust can express. The failure names the page, the command
/// and the diff, which is what a per-page test would have bought.
#[test]
fn every_walkthrough_transcript_is_still_the_output() {
    let root = repo_root();
    let known = examples();
    let update = std::env::var("UPDATE_TRANSCRIPTS").is_ok();
    let mut checked = 0usize;

    for rel in pages() {
        let path = root.join(&rel);
        let page = std::fs::read_to_string(&path).unwrap();
        let Some(corpus) = corpus_of(&page, &known) else {
            continue;
        };

        let ex = Example::materialize(&corpus);
        // Before anything runs, and not because a page asks: the artifact cache defaults to
        // `~/.cache/yidam/vault`, so without this a transcript would be reading whatever the
        // person running the tests happens to have fetched. `vault push --dry-run` reports on
        // what is cached, which would make the page's output a property of the machine.
        let mut env: Vec<(String, String)> = vec![(
            "YIDAM_VAULT_CACHE".to_string(),
            // Relative, and the same value the journalism page exports: resolved against the
            // corpus it runs in, so it is inside the temp tree, and printed by `vault list`
            // as written rather than as an absolute path no page could have recorded.
            ".vault-cache".to_string(),
        )];

        // Byte ranges shift as blocks are rewritten, so edits are collected and applied from
        // the end of the file backwards.
        let mut edits: Vec<(std::ops::Range<usize>, String)> = Vec::new();

        // In page order, and that is load-bearing rather than tidy: a setup fence changes
        // what the commands after it print, so running every setup first would check the
        // blocks above one against an environment the reader does not have there yet. The
        // page is a sequence, and this reads it as one.
        for f in fences(&page) {
            if f.marked_setup {
                assert_eq!(
                    f.lang, "sh",
                    "{rel}: a `{SETUP}` fence is `{}`, not `sh` — it is run by a shell and \
                     the label should say so",
                    f.lang
                );
                let base = env.clone();
                env.extend(run_setup(&ex, &f.text, &base));
                continue;
            }
            if f.lang != "console" {
                continue;
            }
            let recorded = transcripts(&f.text);
            assert!(
                !recorded.is_empty(),
                "{rel}: a ```console block records no `$ ` line, so nothing says which \
                 command produced it. Give it its command, or render it as ```text if it is \
                 an illustration rather than a run"
            );
            let mut fresh = Vec::new();
            for t in &recorded {
                let got = output_of(&ex, &t.command, &env);
                let got = got
                    .trim_end_matches('\n')
                    .split('\n')
                    .map(str::trim_end)
                    .collect::<Vec<_>>()
                    .join("\n");
                checked += 1;
                if !update {
                    let want = redact(t.output.trim_end_matches('\n'), &ex.path());
                    let have = redact(&got, &ex.path());
                    assert_eq!(
                        want,
                        have,
                        "\n{rel}: `$ {}` no longer produces what the page records.\n\
                         Re-run with UPDATE_TRANSCRIPTS=1 to rewrite the block, and read the \
                         diff — a changed transcript is either a corpus that moved or a page \
                         that was wrong.\n\n--- the page says ---\n{}\n\n--- it now says \
                         ---\n{}\n",
                        t.command,
                        t.output.trim_end_matches('\n'),
                        got
                    );
                }
                // A block whose only difference is a fresh commit sha keeps the text it
                // has: rewriting it would churn the page on every update and bury the one
                // block that actually moved.
                let settled = if redact(t.output.trim_end_matches('\n'), &ex.path())
                    == redact(&got, &ex.path())
                {
                    t.output.trim_end_matches('\n').to_string()
                } else {
                    got
                };
                fresh.push(Transcript {
                    command: t.command.clone(),
                    output: format!("{settled}\n"),
                });
            }
            if update {
                edits.push((f.body.clone(), render(&fresh)));
            }
        }

        if update && !edits.is_empty() {
            let mut text = page.clone();
            for (range, replacement) in edits.into_iter().rev() {
                text.replace_range(range, &replacement);
            }
            std::fs::write(&path, text).unwrap();
        }
    }

    assert!(
        checked > 0,
        "no transcript was run — every corpus-backed walkthrough has stopped being recognised"
    );
    assert!(
        !update,
        "UPDATE_TRANSCRIPTS rewrote the pages; re-run without it to check them"
    );
}
