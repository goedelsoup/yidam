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
//!
//! # A merge is counted and is never a point
//!
//! "Authored" is [`commits::is_merge`]'s word: a merge git named (`Merge branch '…'`) is
//! outside the population, and a merge a person wrote is in it. That is what a phase share
//! needs, because `phase settle --merge` settles a phase *with* a `--no-ff` merge whose
//! subject is `phase:`; leave such merges out and the share reads 0 forever.
//!
//! A counted merge must still never be a sampled point (#1392). `settle` can only commit the
//! `regen:` *before* the merge, on the phase branch, because `git merge --no-ff` refuses a dirty
//! index. So the merge is a commit no `regen:` can follow. If it landed on a checkpoint it would
//! leave main's block stale, and the gate red, until someone committed a `regen:` on main.
//! So the merge counts in the shares, and checkpoints fall only on commits with one parent.
//! When main has not moved since the branch, merging leaves every sampled sha and every
//! reading as they were, and the document is the same, byte for byte, as the one the branch's
//! `regen:` committed.

use anyhow::Result;
use std::fmt::Write as _;
use std::path::Path;

use crate::cmd::lint::commits::{self, Subject};
use crate::kuten;

/// Where the document lives in a derived repository. A repository without one has opted
/// out, and the generator is a no-op there — which is what lets `yidam regen` run the same
/// list everywhere.
const FILE: &str = "PRACTICE.md";

/// The most points the series holds. Enough to show a trajectory, few enough that the
/// document reads as a document; `replay` is the surface for every-commit resolution.
const MAX_POINTS: usize = 8;

/// `yidam practice`, run directly.
///
/// The one difference from the generator is the absent-file arm: a generator stays silent
/// there (the repository opted out), but a person who *ran the command* asked for the
/// document and is owed the reason there is none — and the section that opts back in.
pub fn run(root: Option<&std::path::Path>) -> Result<()> {
    let root = crate::paths::resolve_root(root)?;
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
    block(Some(&root))
}

/// Write the `PRACTICE.md` REGEN block. The generator `yidam regen` runs.
pub fn block(root: Option<&std::path::Path>) -> Result<()> {
    let root = crate::paths::resolve_root(root)?;
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
    /// Authored, non-`regen:` commits reachable from here — a person's merges included.
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

/// Whether a commit in the population may be a sampled point: one parent, so never a merge.
/// See the module's "A merge is counted and is never a point".
fn is_checkpoint_candidate(s: &Subject) -> bool {
    in_population(s) && s.parents < 2
}

fn base_verb(verb: &str) -> &str {
    commits::split_scope(verb).map_or(verb, |(base, _)| base)
}

/// Which of `len` chronological positions to sample: fixed checkpoints, never "the newest".
///
/// The positions are `0, stride, 2·stride, …` up to `len - 1`, where `stride` is the
/// smallest power of two that keeps the series to at most `max` points. A point's reading
/// is a function of its own commit alone, so the document moves only when the population
/// reaches the next checkpoint — not on every authored commit, which is what #1465 found
/// it cost: a sampler that always kept the newest commit made every authored commit owe a
/// `regen:`, a quarter of one corpus's history. The newest reading belongs to
/// `kuten check`, which writes nothing.
///
/// Powers of two, so a growing stride only ever *drops* points: doubling it keeps the even
/// checkpoints, and no point already shown is replaced by one beside it. Over a history of
/// `len` authored commits the document moves about `max · log2(len / max)` times.
///
/// Deterministic, which is what the gate needs — two checkouts of one commit must sample
/// the same points.
fn sample_indices(len: usize, max: usize) -> Vec<usize> {
    if len == 0 || max == 0 {
        return Vec::new();
    }
    let mut stride = 1usize;
    // `(len - 1) / stride + 1` points fall on the checkpoints; widen until they fit.
    while (len - 1) / stride + 1 > max {
        stride *= 2;
    }
    (0..len).step_by(stride).collect()
}

/// The sampled series, oldest first.
///
/// The positions come from the population's single-parent commits, so a merge can neither
/// be a point nor shift the stride. [`point`] still counts the merges behind each point.
fn series(root: &Path) -> Result<Vec<Point>> {
    let subjects = commits::read_subjects(root, None);
    // Newest first from the reader; a conduct series reads forward.
    let mut candidates: Vec<&Subject> = subjects
        .iter()
        .filter(|s| is_checkpoint_candidate(s))
        .collect();
    candidates.reverse();
    sample_indices(candidates.len(), MAX_POINTS)
        .into_iter()
        .map(|i| point(root, &candidates[i].hash))
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
        "\nThe population is authored commits: a merge git named and `regen:` commits are \
         outside it, so the commit that carries this document is never counted by it. A \
         merge a person wrote, such as the `phase:` merge that settles a phase, is counted. \
         Points sit at fixed checkpoints among the population's single-parent commits — \
         every first, second, fourth…, whichever keeps the series short — so the newest \
         commit is usually not a point. An authored commit moves this document only when it \
         reaches the next checkpoint, and a merge never does. A `diverges` \
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
Reads: the git history (authored commits only — a merge git named and `regen:` commits
       are outside the population; a merge a person wrote, such as a `phase:` merge, is in
       it but is never a sampled point), and each sampled commit's
       `.yidam/decisions/kuten.yml`, vendored kuten profile and vendored `GRAPH.md`, via
       `git show`.
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

    /// Deterministic, genesis kept, never more than asked for, strictly increasing. The
    /// gate's requirement: two checkouts of one commit must sample the same points.
    #[test]
    fn sampling_is_checkpoints_and_deterministic() {
        assert_eq!(sample_indices(0, 8), Vec::<usize>::new());
        assert_eq!(sample_indices(1, 8), vec![0]);
        assert_eq!(sample_indices(3, 8), vec![0, 1, 2]);
        assert_eq!(sample_indices(8, 8), (0..8).collect::<Vec<_>>());
        assert_eq!(sample_indices(9, 8), vec![0, 2, 4, 6, 8]);
        let picked = sample_indices(200, 8);
        assert_eq!(picked, vec![0, 32, 64, 96, 128, 160, 192]);
        assert_eq!(picked, sample_indices(200, 8));
        for len in 1..2000 {
            let p = sample_indices(len, 8);
            assert!(!p.is_empty() && p.len() <= 8, "len {len}: {p:?}");
            assert_eq!(p[0], 0, "len {len}: genesis dropped");
            assert!(p.windows(2).all(|w| w[0] < w[1]), "len {len}: {p:?}");
            assert!(*p.last().unwrap() < len, "len {len}: {p:?}");
        }
    }

    /// **#1465: the newest commit is not a point.** Growing the population moves the series
    /// only at a checkpoint, and a point once shown is never replaced by a neighbour — a
    /// growing stride only drops points. Counted over a long history, the document moves
    /// on a small fraction of authored commits, where the old sampler moved on every one.
    #[test]
    fn an_authored_commit_moves_the_series_only_at_a_checkpoint() {
        let mut moves = 0;
        for len in 2..=4000 {
            let (before, after) = (sample_indices(len - 1, 8), sample_indices(len, 8));
            if before != after {
                moves += 1;
                assert!(
                    after.iter().all(|i| before.contains(i) || *i == len - 1),
                    "len {len}: the series gained a point that is neither old nor the new \
                     checkpoint: {before:?} -> {after:?}"
                );
            } else {
                assert!(!after.contains(&(len - 1)));
            }
        }
        assert!(
            moves < 100,
            "{moves} of 3999 authored commits moved the series — not a checkpoint sampler"
        );
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
            body: String::new(),
        };
        assert!(!in_population(&s("regen", "regen: blocks", 1)));
        assert!(!in_population(&s("regen(blocks)", "regen(blocks): x", 1)));
        assert!(in_population(&s("establish", "establish: a thing", 1)));
        assert!(in_population(&s("phase(x)", "phase(x): done", 1)));
        // Not a prefix match: a different verb that starts with the word is read whole.
        assert!(in_population(&s("regenerate", "regenerate: hm", 1)));
        // Merges git named are outside for `measure`'s own reason: nobody chose their verb.
        assert!(!in_population(&s("Merge", "Merge branch 'x'", 2)));
        // #1392: a merge a person wrote is counted, and is never a point.
        let settled = s("phase", "phase: x settled", 2);
        assert!(in_population(&settled));
        assert!(!is_checkpoint_candidate(&settled));
        assert!(is_checkpoint_candidate(&s("phase", "phase: squashed", 1)));
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
            text.contains("`regen:` commits are outside it") && text.contains("a merge never does"),
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

    /// **#1465, run for real.** Past the point where every commit is a checkpoint, an
    /// authored commit that falls between checkpoints leaves the document byte-identical —
    /// so it owes no `regen:` — and the next one, landing on a checkpoint, moves it.
    #[test]
    fn an_authored_commit_between_checkpoints_does_not_move_the_document() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path();
        crate::git::fixture::init(root);
        let commit = |msg: &str| {
            std::fs::write(root.join("n"), msg).unwrap();
            crate::git::fixture::commit(root, msg);
        };
        commit("genesis: a corpus");
        for i in 1..9 {
            commit(&format!("establish: thing {i}"));
        }
        // Nine authored commits: stride 2, checkpoints at 0, 2, 4, 6, 8 — the ninth is one.
        let nine = generate(root).unwrap();
        commit("establish: thing 9");
        let ten = generate(root).unwrap();
        assert_eq!(
            nine, ten,
            "the tenth authored commit is between checkpoints and still moved the document"
        );
        commit("establish: thing 10");
        let eleven = generate(root).unwrap();
        assert_ne!(
            ten, eleven,
            "the eleventh authored commit is a checkpoint and did not move the document"
        );
    }

    /// **#1392, run for real.** `phase settle --merge` commits the `regen:` on the phase branch
    /// and then the `--no-ff` `phase:` merge. Here the merge is the ninth authored commit,
    /// which is a checkpoint, because nine commits widen the stride to 2. The document on main
    /// after the merge must still equal the one the branch's `regen:` carried. The merge must
    /// also be counted, or the phase share it is the evidence for would read 0.
    #[test]
    fn a_phase_merge_is_counted_and_does_not_move_the_document() {
        use crate::git::fixture::{commit_at, git};
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path();
        crate::git::fixture::init(root);
        // Strictly increasing dates, so `git log`'s order is the order written here.
        let mut hour = 0;
        let mut commit = |msg: &str| {
            hour += 1;
            std::fs::write(root.join("n"), msg).unwrap();
            commit_at(root, msg, &format!("2026-01-01T{hour:02}:00:00Z"));
        };
        commit("genesis: a corpus");
        for i in 1..4 {
            commit(&format!("establish: thing {i}"));
        }
        git(root, &["switch", "-q", "-c", "phase/x"]);
        for i in 4..8 {
            commit(&format!("establish: thing {i}"));
        }
        commit("regen: blocks refreshed");
        let on_branch = generate(root).unwrap();

        git(root, &["switch", "-q", "main"]);
        git(
            root,
            &[
                "merge",
                "-q",
                "--no-ff",
                "--no-gpg-sign",
                "-m",
                "phase: x settled",
                "phase/x",
            ],
        );
        let on_main = generate(root).unwrap();
        assert_eq!(
            on_branch, on_main,
            "the phase merge moved the document, so main fails `regen --check` until a \
             `regen:` lands on it — #1392"
        );

        // The next authored commit is a checkpoint, and its count includes the merge: eight
        // commits before the branch's end, the merge, and itself.
        commit("establish: thing 8");
        let after = generate(root).unwrap();
        assert!(after.contains("| 10 | 10.0% |"), "{after}");
    }
}
