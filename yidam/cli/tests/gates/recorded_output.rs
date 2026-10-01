//! Recorded output in the documentation, and the claims it makes that nothing re-runs.
//!
//! A `$ yidam …` block followed by what it printed is a hardcoded claim written beside the
//! thing that produces it, and it stops being true without ever going red.
//! `walkthrough_transcripts` closes that for `docs/walkthroughs/` the strongest way there is
//! — it re-runs every block against the example the page names. The rest of the
//! documentation cannot be held that way: the transcripts there show a repository in a state
//! that took a broken provenance file and a missing corpus to produce, and building it to
//! diff one screen would be a fixture nobody maintains.
//!
//! So the claims that *can* be decided are decided one at a time, here.
//!
//! # The version is the one that always rots
//!
//! `0.5.0 (78544f8)` sat in two blocks across two pages for seven minor releases —
//! `docs/troubleshooting.md`, in the `doctor` screen and again under `unrecognized
//! subcommand`, and `docs/installation.md` under "Verify the install". Each release falsifies
//! such a pair the day it ships, and no reader can act on either half: the version they have
//! is whatever they installed, and the commit is not something they check.
//!
//! So the pages redact it, `<version> (<commit>)`, and this keeps a re-recording from pasting
//! a fresh one back — which is how the last one arrived. Nothing is lost: the sentences
//! beside those blocks are about the *features*, which is the part of `--version` a reader
//! acts on, and `light_build.rs` holds that half to the manifest.

use std::path::PathBuf;

use walkdir::WalkDir;

fn docs_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs")
}

/// `1.2.3` — three dot-separated runs of digits, and nothing else.
fn is_version(word: &str) -> bool {
    let parts: Vec<&str> = word.split('.').collect();
    parts.len() == 3
        && parts
            .iter()
            .all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))
}

/// **No recorded output in the documentation pins a version.**
///
/// Scoped to ```` ```console ```` fences, which are output. A ```` ```sh ```` fence is what
/// the reader types, and `YIDAM_REF=v0.2.0 mise run yidam-vendor-update` there is an example
/// argument rather than a claim about what the binary reports — a rule that graded input too
/// would have to argue with the page that teaches you to pass a tag.
#[test]
fn no_recorded_output_in_the_docs_pins_a_version() {
    let mut offenders: Vec<String> = Vec::new();
    let mut fences = 0;

    for entry in WalkDir::new(docs_dir())
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
    {
        let name = entry.file_name().to_string_lossy().to_lowercase();
        if !name.ends_with(".md") && !name.ends_with(".mdx") {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(entry.path()) else {
            continue;
        };
        let rel = entry
            .path()
            .strip_prefix(docs_dir())
            .unwrap_or(entry.path())
            .display()
            .to_string();

        for fence in text.split("```console").skip(1) {
            fences += 1;
            for line in fence.split("```").next().unwrap_or_default().lines() {
                if line
                    .split(|c: char| !matches!(c, '0'..='9' | '.'))
                    .any(is_version)
                {
                    offenders.push(format!("  docs/{rel} — {}", line.trim()));
                }
            }
        }
    }

    // #672: a fence scan that finds no fences passes.
    assert!(
        fences > 10,
        "{fences} ```console fences found under docs/ — the scan is not reaching them, and \
         this test now asserts nothing"
    );
    assert!(
        offenders.is_empty(),
        "{} line(s) of recorded output name a version, which the next release falsifies — \
         redact it as `<version>`:\n{}",
        offenders.len(),
        offenders.join("\n")
    );
}
