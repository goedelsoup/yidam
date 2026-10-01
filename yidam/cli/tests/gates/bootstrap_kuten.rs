//! G10 — the bootstrap selects a kuten, and the scaffold records it.
//!
//! A kuten is vendored at genesis without anyone doing anything: step 8 moves the whole
//! `yidam/prelude/` tree into `.yidam/.vendor/prelude/`, so `kuten/inquiry/` lands exactly
//! where [`yidam::kuten::VENDORED_DIR`] points. What no step did was **write the record that
//! names it** — `kuten` appeared in no file under `yidam/prelude/skills/`, and
//! `.yidam/decisions/kuten.yml` existed in none of the eighteen derived corpora. Every
//! surface that reads a held kuten — the `AGENTS.md` REGEN block, `doctor`, `kuten check` —
//! therefore had no subject on the day it shipped.
//!
//! # Why none of this greps
//!
//! `bootstrap.md` now says "kuten" in a dozen sentences. A check for the word passes whether
//! or not the step survives, which is the failure this repository keeps re-finding: prose
//! satisfies a scan. So the assertions below read **structure** —
//!
//! - the record path stated as a fenced block whose entire body is the path, which is how
//!   this skill states every path it writes to;
//! - the `yaml` block that follows it, written to a real file and read back through the
//!   shipped reader ([`yidam::kuten::read_declaration`]), so what is checked is that the
//!   bytes the skill prescribes are accepted by the binary that will read them;
//! - the listing the dialogue learns the profile set from, as a fenced command;
//! - the prompt the user answers, as a blockquote.
//!
//! Prose mentioning any of those creates no fence and no blockquote. The one claim that is
//! irreducibly prose — that declining is offered — is bounded by two structural anchors and
//! read only from between them.
//!
//! # And nothing here is hardcoded
//!
//! The profile set is read out of `yidam/prelude/kuten/`, the tree that settles it. A second
//! profile shipping, or `inquiry` being revised, is measured against what is there.

use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn skill() -> String {
    let path = repo_root().join("yidam/prelude/skills/bootstrap.md");
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// One fenced code block: its info string, its body, and where it sits in the document.
struct Fence {
    info: String,
    body: String,
    /// Byte offset of the opening fence line.
    start: usize,
    /// Byte offset just past the closing fence line.
    end: usize,
}

/// Every fenced block in a markdown document, in order.
///
/// A fence is a line whose first three characters are backticks; the rest of that line is the
/// info string. Nothing here handles indented or tilde fences, because this document uses
/// neither — and a fence style it does not use appearing tomorrow shows up as a block this
/// function fails to return, which reddens the assertions below rather than passing them.
fn fences(text: &str) -> Vec<Fence> {
    let mut out = Vec::new();
    let mut open: Option<(String, Vec<&str>, usize)> = None;
    let mut at = 0usize;
    for line in text.split_inclusive('\n') {
        let trimmed = line.trim_end_matches('\n');
        if trimmed.starts_with("```") {
            match open.take() {
                None => open = Some((trimmed.trim_matches('`').trim().to_string(), vec![], at)),
                Some((info, body, start)) => out.push(Fence {
                    info,
                    body: body.join("\n"),
                    start,
                    end: at + line.len(),
                }),
            }
        } else if let Some((_, body, _)) = open.as_mut() {
            body.push(trimmed);
        }
        at += line.len();
    }
    out
}

/// The profiles this template ships, read out of the tree rather than named here.
fn shipped_profiles() -> Vec<(String, yidam::kuten::Profile)> {
    let dir = repo_root().join("yidam/prelude/kuten");
    let mut out = Vec::new();
    for entry in std::fs::read_dir(&dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display())) {
        let path = entry.expect("dir entry").path();
        let manifest = path.join("kuten.yml");
        if !manifest.is_file() {
            continue; // `README.md`, or a directory that is not a profile
        }
        let text = std::fs::read_to_string(&manifest).expect("read profile");
        let profile = yidam::kuten::Profile::parse(&text)
            .unwrap_or_else(|e| panic!("{} is not a profile: {e}", manifest.display()));
        let name = path
            .file_name()
            .expect("profile directory name")
            .to_string_lossy()
            .to_string();
        out.push((name, profile));
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    assert!(
        !out.is_empty(),
        "found no kuten profile under {} — the scan is broken, not the tree",
        dir.display()
    );
    out
}

/// Write the scaffolded record where the CLI looks for it, and read it back with the CLI.
fn read_back(record: &str) -> yidam::kuten::Declaration {
    let tmp = tempfile::tempdir().expect("tempdir");
    let path = tmp.path().join(yidam::kuten::DECISION_PATH);
    std::fs::create_dir_all(path.parent().expect("decisions dir")).expect("mkdir");
    std::fs::write(&path, record).expect("write record");
    yidam::kuten::read_declaration(tmp.path())
        .expect("the record the bootstrap scaffolds must parse as a kuten decision record")
        .expect("read_declaration found no record at the path it owns")
}

/// The fenced block whose whole body is the decision-record path.
fn record_path_fence(fences: &[Fence]) -> usize {
    fences
        .iter()
        .position(|f| f.info.is_empty() && f.body.trim() == yidam::kuten::DECISION_PATH)
        .unwrap_or_else(|| {
            panic!(
                "bootstrap.md scaffolds no `{}`. Every surface that reads a held kuten — the \
                 `AGENTS.md` block, `doctor`, `kuten check` — then has no subject at genesis, \
                 which is the surface-with-no-consumer failure RFC-0028 names by name. A \
                 sentence mentioning the path is not the step: this looks for a fenced block \
                 whose whole body is the path.",
                yidam::kuten::DECISION_PATH
            )
        })
}

/// The step exists, writes the record path, and scaffolds something the binary can read.
///
/// The four assertions are independent on purpose. Deleting the record block leaves the
/// listing and the prompt; deleting the prompt leaves the record. Deleting the whole step —
/// the mutation this guard was written against — takes all four.
#[test]
fn the_bootstrap_selects_a_kuten() {
    let text = skill();
    let fences = fences(&text);
    assert!(
        fences.len() > 10,
        "found {} fenced blocks in bootstrap.md — the parse is broken, not the doc",
        fences.len()
    );

    // ── the record path, stated the way this skill states every path it writes ──
    let at = record_path_fence(&fences);

    // ── and the record it prescribes, read by the reader that will read it ──
    let record = fences.get(at + 1).unwrap_or_else(|| {
        panic!(
            "bootstrap.md names {} and prescribes no record after it",
            yidam::kuten::DECISION_PATH
        )
    });
    assert_eq!(
        record.info,
        "yaml",
        "the block after {} is a `{}` block, not `yaml` — the record moved, or a block was \
         inserted between the path and its content",
        yidam::kuten::DECISION_PATH,
        record.info
    );
    let declaration = read_back(&record.body);

    // ── the profile it names ships, at the revision it records ──
    let profiles = shipped_profiles();
    let (_, profile) = profiles
        .iter()
        .find(|(dir, _)| *dir == declaration.name)
        .unwrap_or_else(|| {
            panic!(
                "the record scaffolds `kuten: {}` and no such profile ships. The template holds \
                 {:?}, and a genesis record naming a profile that was never vendored reads as a \
                 repository whose kuten cannot be found.",
                declaration.name,
                profiles.iter().map(|(d, _)| d).collect::<Vec<_>>()
            )
        });
    assert_eq!(
        declaration.revision, profile.revision,
        "the record scaffolds revision {} and `{}` ships at revision {}. The revision is what \
         makes a kuten readable at the vintage a repository holds; a scaffold teaching the next \
         genesis to write a stale one is the vintage error A0 exists to warn against.",
        declaration.revision, declaration.name, profile.revision
    );

    // ── the dialogue learns the set by listing it, not from a name in prose ──
    let profiles_dir = "yidam/prelude/kuten/";
    assert!(
        fences.iter().any(|f| f.body.contains(profiles_dir)),
        "no fenced command in bootstrap.md reaches `{profiles_dir}`. The dialogue has to read \
         the profiles to quote a gloss and copy a revision, and naming them in prose instead is \
         the drift that left `index.yml` uninstalled — step 3 argues this at length."
    );

    // ── and the user is actually asked ──
    let prompted = text
        .lines()
        .any(|l| l.starts_with("> **") && l.contains("kuten"));
    assert!(
        prompted,
        "bootstrap.md writes a kuten record and never states it to the user. Every other \
         decision this step records — the alignment, the seed count, the governance mode — is \
         asked as a `> **…**` prompt first, and a kuten adopted without the user seeing it is a \
         declaration that will be diverged from on the first commit."
    );
}

/// Holding no kuten stays a supported state, and the step that scaffolds one says so.
///
/// This is the half a scaffold quietly destroys. All eighteen existing corpora hold no
/// record; `render_block`, `doctor` and `Report` each carry a deliberate arm for that, and a
/// bootstrap that writes the file unconditionally turns "no kuten" from a state into an
/// omission — with no migration written for the repositories already in it.
///
/// The claim is prose and cannot be anything else. What is structural is *where* it is read
/// from: the region between the fence that lists the profiles and the fence that names the
/// record. The rest of the document — including this repository's own documentation of the
/// supported state — cannot satisfy it.
#[test]
fn declining_a_kuten_is_still_a_state_the_step_offers() {
    let text = skill();
    let fences = fences(&text);
    let at = record_path_fence(&fences);
    let listing = fences
        .iter()
        .take(at)
        .rposition(|f| f.body.contains("yidam/prelude/kuten/"))
        .expect("the profile listing — see `the_bootstrap_selects_a_kuten`");

    let region = &text[fences[listing].end..fences[at].start];
    assert!(
        region.contains("no kuten"),
        "the step that scaffolds {} never says that holding none is a state. All eighteen \
         corpora are in that state, `render_block` has an arm for it, and a step that offers no \
         way to decline writes the record for a user who did not choose it. Read from the {} \
         bytes between the profile listing and the record path.",
        yidam::kuten::DECISION_PATH,
        region.len()
    );
}

/// The vendor step still moves the tree the record's profile lives in.
///
/// The record is written in step 2 and the profile arrives in step 8. If the vendor step ever
/// stops moving `yidam/prelude/` wholesale — or starts pruning `kuten/` the way it prunes
/// `domains/` — the record names a profile that is not there, and every consumer reports a
/// repository whose declaration cannot be read.
#[test]
fn the_vendor_step_carries_the_profile_the_record_names() {
    let text = skill();
    let fences = fences(&text);
    let vendored_root = yidam::kuten::VENDORED_DIR
        .strip_suffix("/kuten")
        .expect("VENDORED_DIR sits under the vendored prelude");

    let moves = fences.iter().any(|f| {
        f.body
            .contains(&format!("mv yidam/prelude {vendored_root}"))
    });
    assert!(
        moves,
        "no fenced command in bootstrap.md moves `yidam/prelude` to `{vendored_root}`. `{}` is \
         where every consumer looks for the profile a genesis record names.",
        yidam::kuten::VENDORED_DIR
    );

    let pruned: Vec<&str> = fences
        .iter()
        .map(|f| f.body.as_str())
        .filter(|b| b.contains("rm") && b.contains(yidam::kuten::VENDORED_DIR))
        .collect();
    assert!(
        pruned.is_empty(),
        "the vendor step removes something under `{}`: {pruned:?}. The profile the genesis \
         record names has to survive it — `domains/` is dropped because nothing in a derived \
         repository builds it, and a kuten is read on every `yidam regen`.",
        yidam::kuten::VENDORED_DIR
    );
}

/// The `AGENTS.md` the scaffold installs is what reads the record at genesis.
///
/// A2 shipped the REGEN block; nothing gave it a subject. The block and the record are two
/// halves of one mechanism, and losing either leaves the other pointing at nothing.
#[test]
fn the_scaffolded_agents_md_reads_the_record() {
    let path = repo_root().join("sadhana/root/AGENTS.md");
    let agents =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    assert!(
        agents.contains("<!-- REGEN: yidam kuten"),
        "{} carries no `yidam kuten` REGEN block. The record the bootstrap writes is then read \
         by nothing an agent sees at session start.",
        path.display()
    );
    assert!(
        agents.contains(yidam::kuten::DECISION_PATH),
        "{}'s kuten block does not name `{}` as what it reads.",
        path.display(),
        yidam::kuten::DECISION_PATH
    );
}

#[test]
fn the_fence_parser_reads_info_strings_bodies_and_offsets() {
    let text = "prose\n```\na\n```\nmore\n```yaml\nb: 1\n```\n";
    let f = fences(text);
    assert_eq!(f.len(), 2);
    assert_eq!(f[0].info, "");
    assert_eq!(f[0].body, "a");
    assert_eq!(f[1].info, "yaml");
    assert_eq!(f[1].body, "b: 1");
    assert_eq!(&text[f[0].end..f[1].start], "more\n");
}

#[test]
fn a_path_named_only_in_prose_is_not_a_fence() {
    let text = format!(
        "The bootstrap writes {} at genesis.\n",
        yidam::kuten::DECISION_PATH
    );
    assert!(
        fences(&text).is_empty(),
        "a sentence naming the record path produced a fenced block — the guard would then be \
         satisfied by prose, which is the thing it exists to refuse"
    );
}
