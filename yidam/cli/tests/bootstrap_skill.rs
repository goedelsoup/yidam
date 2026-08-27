//! Internal consistency of `prelude/skills/bootstrap.md`.
//!
//! The bootstrap skill is executed by an agent, not by a program, so nothing about it has to
//! parse and nothing about it fails a build. Its consistency is therefore maintained entirely
//! by whoever last read it — which works until the file is long enough that nobody reads all
//! of it in one sitting, and it has been that long for a while.
//!
//! These are the claims the skill makes *about itself*, which are the ones a reader is least
//! likely to re-derive: how many sections a step has, and which decision records it defines.
//! Neither says anything about whether the skill is good advice.

use std::collections::BTreeMap;

fn skill() -> String {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../yidam/prelude/skills/bootstrap.md");
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// Step 9 promises a number of sections and then lists them.
///
/// It said "four sections" over five for as long as anyone can tell, and the count was
/// incremented to five when a sixth was added — a fix that carried the error forward, which
/// is the specific way a stated count goes wrong. A reader who trusts the number stops
/// looking for the last section, and the last section is `Next steps`.
#[test]
fn step_nine_lists_the_sections_it_promises() {
    let text = skill();
    let (before, after) = text
        .split_once("Output a structured handoff with ")
        .expect("step 9's section preamble");
    let stated_word = after.split_whitespace().next().unwrap_or("");
    let words: BTreeMap<&str, usize> = [
        ("three", 3),
        ("four", 4),
        ("five", 5),
        ("six", 6),
        ("seven", 7),
        ("eight", 8),
        ("nine", 9),
    ]
    .into_iter()
    .collect();
    let stated = *words.get(stated_word).unwrap_or_else(|| {
        panic!("step 9 promises {stated_word:?} sections and that is not a number this test knows")
    });

    // Sections run from the preamble to the closing prompt, and are the only bolded lines
    // starting at column zero — a numbered sub-item begins `1. **`, the prompt begins `> `.
    let body = after
        .split_once("\nThen ask:")
        .map(|(b, _)| b)
        .unwrap_or(after);
    let listed: Vec<&str> = body
        .lines()
        .filter(|l| l.starts_with("**"))
        .map(|l| l.trim_start_matches('*').split("**").next().unwrap_or(l))
        .collect();

    assert!(
        !before.is_empty() && !listed.is_empty(),
        "step 9 did not parse — the preamble or the section list moved"
    );
    assert_eq!(
        stated,
        listed.len(),
        "step 9 promises {stated} sections and lists {}: {listed:?}",
        listed.len()
    );
}

/// Every decision record the skill names carries the `id:` its filename implies.
///
/// `.yidam/decisions/<slug>.yml` is read back by `yidam decisions-log`, which walks the
/// directory rather than looking anything up, so a record whose `id:` disagrees with its
/// filename is not an error anywhere — it is just a decision that is hard to find twice.
///
/// Discovered from the file. A record added to the skill tomorrow is checked tomorrow.
#[test]
fn every_decision_record_the_skill_defines_matches_its_own_id() {
    let text = skill();

    let mut slugs: Vec<String> = Vec::new();
    for (i, _) in text.match_indices(".yidam/decisions/") {
        let tail = &text[i + ".yidam/decisions/".len()..];
        let Some(name) = tail.strip_suffix("").and_then(|t| {
            let end = t.find(|c: char| !c.is_ascii_alphanumeric() && c != '-' && c != '.')?;
            Some(&t[..end])
        }) else {
            continue;
        };
        let Some(slug) = name.strip_suffix(".yml") else {
            continue; // `.yidam/decisions/` as a directory, not a record
        };
        if slug.is_empty() || slug == "README" || slugs.iter().any(|s| s == slug) {
            continue;
        }
        slugs.push(slug.to_string());
    }

    assert!(
        slugs.len() >= 3,
        "found {} decision record(s) in the skill — the parse is broken, not the doc: {slugs:?}",
        slugs.len()
    );
    let missing: Vec<&String> = slugs
        .iter()
        .filter(|s| !text.contains(&format!("id: {s}\n")))
        .collect();
    assert!(
        missing.is_empty(),
        "the skill writes {missing:?} but defines no record with a matching `id:`. \
         `decisions-log` walks the directory and looks nothing up, so the mismatch is not an \
         error anywhere — it is a decision that is hard to find twice."
    );
}
