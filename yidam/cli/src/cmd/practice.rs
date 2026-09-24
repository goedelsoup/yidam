//! `yidam practice` — a corpus's practice is in its history, written where it can be read.
//!
//! #287's charge, under the boundary RFC-0028 §8 decided: the document measures **conduct
//! against the declared kuten**, sharing `kuten check`'s divergence semantics — divergence
//! is a question, vintage is never divergence, and the document is regenerated, never
//! authored. Where a repository holds no kuten, it describes conduct without a baseline and
//! says so. The constitutional argument is made once, in that RFC, and cited rather than
//! remade here.
//!
//! # What the committed form may read
//!
//! The #647 contract: a gated REGEN block is a function of what every checkout of the
//! commit shares — the tree, and HEAD-ancestry history. So each sampled point's baseline is
//! read out of *that commit's own tree* (`git show <sha>:path`), never out of today's, and
//! nothing here reads a ref: the stalled-phases evidence the original sketch named is open
//! phase *branches*, and it stays with the live surfaces (`status`, `due`).
//!
//! # The fixed point
//!
//! A history-derived committed document would make its own gate unwinnable — the `regen:`
//! commit that carries it changes the history it derives from, so `regen --check` would
//! always find it one commit stale. The population is therefore authored commits whose base
//! verb is not `regen`: the commit that carries this document can never be in the
//! population the document measures, and regeneration converges.

use anyhow::Result;
use std::fmt::Write as _;
use std::path::Path;

use crate::cmd::lint::commits::{self, Subject};
use crate::kuten;

/// Where the document lives in a derived repository. A repository without one has opted
/// out, and the generator is a no-op there — which is what lets `yidam regen` run the same
/// list everywhere.
const FILE: &str = "PRACTICE.md";

/// How many points the series holds. Enough to show a trajectory, few enough that the
/// document reads as a document; `replay` is the surface for every-commit resolution.
const MAX_POINTS: usize = 8;

/// `yidam practice`, run directly.
///
/// The one difference from the generator is the absent-file arm: a generator stays silent
/// there (the repository opted out), but a person who *ran the command* asked for the
/// document and is owed the reason there is none — and the section that opts back in.
pub fn run() -> Result<()> {
    let root = crate::paths::repo_root()?;
    if !root.join(FILE).exists() {
        println!("There is no {FILE} here, so this corpus keeps no practice document.");
        println!("That is a supported state: the scaffold ships one, and a repository");
        println!("that predates it was never handed the marker.");
        println!();
        println!("To opt in, create {FILE} holding this, and `yidam regen` fills it:");
        println!();
        for line in PRACTICE_SECTION.lines() {
            println!("    {line}");
        }
        return Ok(());
    }
    block()
}

/// Write the `PRACTICE.md` REGEN block. The generator `yidam regen` runs.
pub fn block() -> Result<()> {
    let root = crate::paths::repo_root()?;
    // Not just an economy: `update_file_regen` would no-op on the absent file anyway, but
    // the series below walks the history once per sampled point, and a repository that
    // opted out should not pay for a document it does not keep.
    if !root.join(FILE).exists() {
        return Ok(());
    }
    let content = generate(&root)?;
    crate::regen::emit(&content);
    crate::regen::update_file_regen(&root.join(FILE), "yidam practice", &content)
}

/// The document's text for this repository.
fn generate(root: &Path) -> Result<String> {
    let declaration = kuten::read_declaration(root)?;
    let profile = match &declaration {
        Some(d) => kuten::read_profile(root, &d.name)?,
        None => None,
    };
    let points = series(root)?;
    Ok(render(declaration.as_ref(), profile.as_ref(), &points))
}

// ── the series ────────────────────────────────────────────────────────────────

/// One sampled point: the corpus's conduct as of one commit, read against the declaration
/// that commit's own tree held.
struct Point {
    /// Abbreviated commit id — display only; the full id pins the reads.
    sha: String,
    /// Author date, UTC. Rendered as a date: a conduct series is about seasons, not seconds.
    date: String,
    /// Authored, non-`regen:` commits reachable from here.
    commits: usize,
    phase_share: String,
    off_vocabulary_share: String,
    /// The verdicts, in `kuten check`'s vocabulary — or the reason there are none.
    reading: String,
}

/// Whether a commit is in the population this document measures.
///
/// Authored (a git-generated merge subject names no verb anybody chose), and not a
/// `regen:` commit — the fixed point. The base verb, so `regen(blocks):` is excluded with
/// `regen:` for the same reason `phase(x):` counts with `phase:` in [`kuten::compare`]'s
/// world: the suffix is a spelling, not a different act.
fn in_population(s: &Subject) -> bool {
    !commits::is_merge(&s.text, s.parents) && base_verb(&s.verb) != "regen"
}

fn base_verb(verb: &str) -> &str {
    commits::split_scope(verb).map_or(verb, |(base, _)| base)
}

/// Which of `len` chronological positions to sample, first and last always kept.
///
/// The same shape as `replay`'s sampler: strictly increasing because `len - 1 >= n - 1`,
/// and deterministic, which is what the gate needs — two checkouts of one commit must
/// sample the same points.
fn sample_indices(len: usize, max: usize) -> Vec<usize> {
    if len == 0 {
        return Vec::new();
    }
    let n = len.min(max);
    if n == 1 {
        return vec![0];
    }
    (0..n).map(|i| i * (len - 1) / (n - 1)).collect()
}

/// The sampled series, oldest first.
fn series(root: &Path) -> Result<Vec<Point>> {
    let subjects = commits::read_subjects(root, None);
    // Newest first from the reader; a conduct series reads forward.
    let mut population: Vec<&Subject> = subjects.iter().filter(|s| in_population(s)).collect();
    population.reverse();
    sample_indices(population.len(), MAX_POINTS)
        .into_iter()
        .map(|i| point(root, &population[i].hash))
        .collect()
}

/// Read one point: the measurement up to `sha`, against the baseline `sha`'s tree vendored.
///
/// Contemporaneous on purpose. Reading every point against *today's* declaration would
/// report a corpus's early history as divergent from a practice it had not yet adopted —
/// the vintage error, one level up. An adoption or a re-vendor appears as the reading
/// changing between points, which is the honest shape of the event.
fn point(root: &Path, sha: &str) -> Result<Point> {
    let subjects = commits::read_subjects(root, Some(sha));
    let population: Vec<&Subject> = subjects.iter().filter(|s| in_population(s)).collect();
    let m = kuten::commit_measurement(&population);

    let reading = match show(root, sha, kuten::DECISION_PATH)
        .and_then(|t| kuten::Declaration::parse(&t).ok())
    {
        None => "no kuten".to_string(),
        Some(d) => {
            let profile_rel = format!("{}/{}/kuten.yml", kuten::VENDORED_DIR, d.name);
            match show(root, sha, &profile_rel).and_then(|t| kuten::Profile::parse(&t).ok()) {
                None => format!("kuten `{}` declared, no profile vendored", d.name),
                Some(mut p) => {
                    // The two slots a historical point cannot answer. `question_pressure`
                    // reads the corpus's *state* — its open questions as the working tree
                    // holds them — and no walk of `git show` reconstructs that honestly;
                    // a zero read as "no open questions" would fabricate a divergence at
                    // every point. `rubric` names score's criteria and measures nothing.
                    p.question_pressure = None;
                    p.rubric = None;
                    let vintage = show(root, sha, ".yidam/.vendor/prelude/GRAPH.md")
                        .map_or_else(kuten::Vintage::absent, |t| kuten::Vintage::read(&t));
                    reading_of(&kuten::compare(&p, &m, &vintage), d.revision != p.revision)
                }
            }
        }
    };

    let share = |n: usize| {
        if m.commits == 0 {
            "—".to_string()
        } else {
            kuten::percent(n as f64 / m.commits as f64)
        }
    };
    Ok(Point {
        sha: sha.chars().take(7).collect(),
        date: commit_date(root, sha)?,
        commits: m.commits,
        phase_share: share(m.phase_commits),
        off_vocabulary_share: share(m.off_vocabulary_commits),
        reading,
    })
}

/// The findings as one cell, in the verdict vocabulary `kuten check` prints.
fn reading_of(findings: &[kuten::Finding], revision_skew: bool) -> String {
    let mut parts: Vec<String> = findings
        .iter()
        .map(|f| format!("{} {}", f.slot, f.verdict.tag()))
        .collect();
    if parts.is_empty() {
        parts.push("declared, no bands".to_string());
    }
    if revision_skew {
        parts.push("revision skew".to_string());
    }
    parts.join(" · ")
}

/// A commit's author date, UTC — the runner's timezone must not move a committed block.
fn commit_date(root: &Path, sha: &str) -> Result<String> {
    let out = crate::git::Git::new(root)
        .args(["show", "-s", "--format=%at"])
        .rev(sha)
        .output()?;
    anyhow::ensure!(out.status.success(), "git show -s {sha} failed");
    let secs: u64 = String::from_utf8_lossy(&out.stdout).trim().parse()?;
    // The date half only: a series of seasons does not need seconds.
    Ok(crate::cmd::export::unix_to_iso(secs)[..10].to_string())
}

/// A file as one commit's tree holds it. `None` covers both "not there yet" and "not a
/// commit this clone has" — and the second cannot happen under `regen`'s own whole-history
/// precondition, which is the same clone-shape contract this block lives under.
fn show(root: &Path, sha: &str, path: &str) -> Option<String> {
    // `.output()`, not `.try_run()`: this is file content, and a trimmed file is not the file.
    let out = crate::git::Git::new(root)
        .arg("show")
        .rev(format!("{sha}:{path}"))
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

// ── rendering ─────────────────────────────────────────────────────────────────

/// Render the document from what was read. Pure, so every arm is testable without a
/// repository — the same split `kuten`'s `render_block` makes.
fn render(
    declaration: Option<&kuten::Declaration>,
    profile: Option<&kuten::Profile>,
    points: &[Point],
) -> String {
    let mut out = match (declaration, profile) {
        (Some(_), Some(p)) => format!(
            "**This corpus's practice is `{}`, at revision {}.** What follows is its \
             conduct, measured over its authored history and read, at each point, against \
             the declaration that point's own tree held — so an adoption or a re-vendor \
             appears as the reading changing, not as an error.\n",
            p.name, p.revision
        ),
        (Some(d), None) => format!(
            "**Kuten `{}` is declared and no profile is vendored to read it against.** \
             What follows is this corpus's conduct, measured; the readings below use \
             whatever baseline each point's own tree still held.\n",
            d.name
        ),
        _ => "_This repository holds no kuten._ What follows describes its conduct \
              without a baseline, and says so: the shares are measured, and nothing here \
              judges them.\n"
            .to_string(),
    };

    if points.is_empty() {
        out.push_str("\nNo authored history to read yet.\n");
    } else {
        out.push_str(
            "\n| commit | date | commits | phase share | off-vocabulary | reading |\n\
             |---|---|---|---|---|---|\n",
        );
        for p in points {
            let _ = writeln!(
                out,
                "| `{}` | {} | {} | {} | {} | {} |",
                p.sha, p.date, p.commits, p.phase_share, p.off_vocabulary_share, p.reading
            );
        }
    }

    out.push_str(
        "\nThe population is authored commits: merges and `regen:` commits are outside it, \
         so the commit that carries this document is never counted by it. A `diverges` \
         reading is a question for a person, not a defect — a kuten binds nobody. \
         `yidam kuten check` reads today's corpus in full, including the corpus-state \
         metrics no historical point can reconstruct.",
    );
    out
}

// ── the scaffold's section ────────────────────────────────────────────────────

/// The document `sadhana/root/PRACTICE.md` ships, and the text [`run`] offers a repository
/// that predates it.
///
/// **A second copy, held equal by a test** — the same bargain `kuten adopt` strikes with
/// its `AGENTS.md` section, and for the same reason: the scaffold is consumed at genesis,
/// so an existing repository can only be handed the marker by the binary, and a mangled
/// copy fails silently (`update_file_regen` writes nothing where the marker is absent).
const PRACTICE_SECTION: &str = "\
# What this corpus has practiced

<!-- REGEN: yidam practice
Regenerated by: `yidam practice`
Fields: the kuten this corpus declares today, and a sampled series of points across its
        authored history — commit count, phase-commit share and off-vocabulary share at
        each point, read against the declaration that point's own tree held.
Reads: the git history (authored commits only — merges and `regen:` commits are outside
       the population), and each sampled commit's `.yidam/decisions/kuten.yml`, vendored
       kuten profile and vendored `GRAPH.md`, via `git show`.
-->
_Run `yidam practice` to populate._
<!-- /REGEN -->

A corpus's practice is in its history and is never derived from it by hand: this document
is regenerated, never authored. Each point is read against the declaration its own tree
held, so an adoption or a re-vendor appears as the reading changing — not as an error. A
kuten binds nobody: a `diverges` reading is a question for a person, not a defect.
";

#[cfg(test)]
mod tests {
    use super::*;

    /// **The section [`run`] offers is the scaffold's, character for character** — the same
    /// guard `kuten adopt` carries, because the failure is the same: `update_file_regen`
    /// writes nothing at all when the marker it looks for is absent or malformed, so a
    /// drifted copy would leave the block permanently unfilled and say nothing about it.
    #[test]
    fn the_offered_section_is_the_scaffolds() {
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../sadhana/root/PRACTICE.md");
        let scaffold = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("{} is unreadable ({e})", path.display()));
        assert!(
            scaffold.contains(PRACTICE_SECTION),
            "`sadhana/root/PRACTICE.md` no longer contains the section `yidam practice` \
             offers a retrofitting repository. A new repository would get one wording and \
             an existing one another, and the two are the same document."
        );
        assert!(
            PRACTICE_SECTION.contains("<!-- REGEN: yidam practice"),
            "the offered section carries no marker, so `yidam practice` would write \
             nothing into it — silently, which is how it fails"
        );
    }

    /// Deterministic, ends kept, never more than asked for. The gate's requirement: two
    /// checkouts of one commit must sample the same points.
    #[test]
    fn sampling_keeps_both_ends_and_is_deterministic() {
        assert_eq!(sample_indices(0, 8), Vec::<usize>::new());
        assert_eq!(sample_indices(1, 8), vec![0]);
        assert_eq!(sample_indices(3, 8), vec![0, 1, 2]);
        let picked = sample_indices(200, 8);
        assert_eq!(picked.len(), 8);
        assert_eq!(picked[0], 0);
        assert_eq!(picked[7], 199);
        assert_eq!(picked, sample_indices(200, 8));
        let mut sorted = picked.clone();
        sorted.dedup();
        assert_eq!(sorted, picked, "indices repeat: {picked:?}");
    }

    /// The fixed point's predicate: `regen:` is outside the population whatever it wears,
    /// and the exclusion is the base verb, not a prefix.
    #[test]
    fn regen_commits_are_outside_the_population() {
        let s = |verb: &str, text: &str, parents: usize| Subject {
            hash: "x".into(),
            verb: verb.into(),
            text: text.into(),
            parents,
            paths: vec![],
        };
        assert!(!in_population(&s("regen", "regen: blocks", 1)));
        assert!(!in_population(&s("regen(blocks)", "regen(blocks): x", 1)));
        assert!(in_population(&s("establish", "establish: a thing", 1)));
        assert!(in_population(&s("phase(x)", "phase(x): done", 1)));
        // Not a prefix match: a different verb that starts with the word is read whole.
        assert!(in_population(&s("regenerate", "regenerate: hm", 1)));
        // Merges are outside for `measure`'s own reason: nobody chose their verb.
        assert!(!in_population(&s("Merge", "Merge branch 'x'", 2)));
    }

    #[test]
    fn the_unheld_arm_is_a_state_and_not_a_fault() {
        let text = render(None, None, &[]);
        assert!(text.contains("holds no kuten"), "{text}");
        assert!(text.contains("without a baseline"), "{text}");
        assert!(text.contains("No authored history"), "{text}");
        assert!(text.contains("binds nobody"), "{text}");
        assert!(
            !text.to_lowercase().contains("fail"),
            "conduct without a baseline must not read as a failure: {text}"
        );
    }

    #[test]
    fn the_held_arm_names_the_practice_and_the_population_rule() {
        let profile = kuten::Profile::parse("kuten: inquiry\nrevision: 1\n").unwrap();
        let declaration = kuten::Declaration::parse("kuten: inquiry\nrevision: 1\n").unwrap();
        let points = vec![Point {
            sha: "abc1234".into(),
            date: "2026-01-02".into(),
            commits: 41,
            phase_share: "17.1%".into(),
            off_vocabulary_share: "0.0%".into(),
            reading: "phases ok · vocabulary ok".into(),
        }];
        let text = render(Some(&declaration), Some(&profile), &points);
        assert!(text.contains("`inquiry`"), "{text}");
        assert!(text.contains("| `abc1234` | 2026-01-02 | 41 |"), "{text}");
        assert!(text.contains("that point's own tree held"), "{text}");
        assert!(
            text.contains("`regen:` commits are outside it"),
            "the population rule is the fixed point's public face: {text}"
        );
    }

    /// The reading cell speaks `kuten check`'s vocabulary and never invents a fifth tag.
    #[test]
    fn a_reading_is_the_verdicts_or_the_reason_there_are_none() {
        use crate::kuten::{Finding, Verdict};
        let f = |slot: &'static str, verdict: Verdict| Finding {
            slot,
            metric: "m",
            verdict,
            declared: String::new(),
            measured: String::new(),
            question: None,
        };
        assert_eq!(
            reading_of(
                &[
                    f("phases", Verdict::Conforming),
                    f("vocabulary", Verdict::Divergent)
                ],
                false
            ),
            "phases ok · vocabulary diverges"
        );
        assert_eq!(reading_of(&[], false), "declared, no bands");
        assert_eq!(
            reading_of(&[f("phases", Verdict::Vintage)], true),
            "phases vintage · revision skew"
        );
    }

    /// **The fixed point, run for real.** The document generated before a `regen:` commit
    /// is byte-identical to the one generated after it — that is what makes the gate
    /// winnable — and an *authored* commit does move it, so the invariance is the
    /// exclusion working and not the generator ignoring history.
    #[test]
    fn a_regen_commit_does_not_move_the_document_and_an_authored_one_does() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path();
        crate::git::fixture::init(root);
        let commit = |msg: &str| {
            std::fs::write(root.join("n"), msg).unwrap();
            crate::git::fixture::commit(root, msg);
        };
        commit("genesis: a corpus");
        commit("establish: one thing");
        commit("phase: a bound");

        let before = generate(root).unwrap();
        commit("regen: blocks refreshed");
        let after = generate(root).unwrap();
        assert_eq!(
            before, after,
            "a `regen:` commit moved the document it would carry — the gate is unwinnable"
        );

        commit("establish: another thing");
        let moved = generate(root).unwrap();
        assert_ne!(
            moved, after,
            "an authored commit did not move the document, so the invariance above is the \
             generator ignoring history rather than the population rule working"
        );
        assert!(moved.contains("no kuten"), "{moved}");
    }
}
