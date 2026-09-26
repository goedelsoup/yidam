//! A rule is separable from the essay that justifies it, and the split has to be held by
//! something that can tell *shorter* from *thinner* (#954).
//!
//! # What was measured
//!
//! The recurring read an agent is told to perform before substantive action was **24,019
//! words** in this repository and **29,326** in a derived one, of which `GRAPH.md`,
//! `directories.md` and `agent-conduct.md` were 22,381. Measuring per `##` section found the
//! weight was not spread across the three files: **six sections of forty-two carried 51% of
//! it.** The bold-lead sentence turned out to be a latent rule marker already — it appears
//! only in sections that grew an essay, and every section under ~250 words has none — so the
//! form below is discovered from the house style rather than imposed on it.
//!
//! # Why a word-count ceiling cannot be the whole gate
//!
//! #933 asked for the recurring read to land near 3,000 words *without losing the reasoning*,
//! and a ceiling alone rewards exactly the outcome it rules out: the cheapest way past it is
//! to delete the essays. So the ceiling is paired with a **floor on each split pair**. The
//! read must shrink; the rules and their evidence together may not. Shortening by moving prose
//! passes; shortening by deleting it fails, and fails naming the section that went missing.
//!
//! Both numbers are ratchets. Lowering the floor is a deliberate edit to a constant here, which
//! is the point — deleting reasoning should be a decision somebody records, not a side effect
//! of a tidy-up.
//!
//! # Per item, never per total
//!
//! A total is cleared by a scanner that looks at nothing, so every assertion below names the
//! item that failed: the rule whose `[why]` link dangles, the evidence section nothing reaches,
//! the section that was emptied. [`the_scan_sees_the_known_routes`] holds the discovery itself
//! against the two routes that are known to exist, because a route scan that silently returns
//! nothing would clear every other check in this file.
//!
//! # Scope
//!
//! Four files are split: `agent-conduct.md` as the worked example, then `GRAPH.md` and
//! `directories.md` against the proven form, then the bootstrap skill (#960). The checks here
//! are written over *discovered* pairs, so a fifth split comes under them the day it lands, with
//! no list here to remember to update.
//!
//! Two things the second pass established that the first could not. A **specification** splits
//! as cleanly as an essay — `GRAPH.md`'s class contract was the case the RFC flagged as the
//! likely limit, and its rule sentences came apart without loss. The real limit is length, not
//! kind: where an argument is a single *clause* rather than a paragraph, a section of its own
//! costs a heading, a link and a connective sentence to carry twenty words, so four of those
//! were inlined into the rules file instead. The form holds for a paragraph and is not worth its
//! scaffolding below about a sentence.
//!
//! The skill added a third: a document that is *executed* differs from one that is consulted in
//! that its arguments sit at the point of action — the fabrication argument where the empty
//! class is, the edge argument where the whole corpus is in view — and a bare `[why]` there
//! costs the agent the one sentence that was doing the work at that moment. So each moved essay
//! leaves its punchline behind, and the rules file is 7,358 rather than the ~6,400 a clean cut
//! would give. Two rules also share one section (`after-genesis-not-before`, stated in steps 7
//! and 8), which [`every_evidence_section_is_reached_by_a_rule`] admits and the reverse check
//! does not need to forbid.
//!
//! # Two occasions, two ceilings
//!
//! The recurring read is one occasion; a bootstrap is another, and the routes above exclude the
//! skill on purpose. [`BOOTSTRAP_CEILING`] holds the second — the ten files between a fresh
//! clone and a first node — because #933 set out to measure exactly that path, quoted a figure
//! from four of its ten files, and the largest of the ten grew twice during the work that was
//! meant to shrink it (#960).

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

mod common;

use common::{repo_root, step_one_read_list, tracked_under, BOOTSTRAP_SKILL};

/// The suffix that marks an evidence file, and the thing that makes a pair discoverable.
const EVIDENCE_SUFFIX: &str = ".evidence.md";

/// Words below which an evidence section is not carrying an argument.
///
/// Deliberately low. This is not a quality bar — nothing here can judge an argument — it is the
/// line between a section that says something and a heading left behind after its prose was
/// deleted, which is the failure the floor above is aimed at and this check localizes.
const MIN_EVIDENCE_WORDS: usize = 25;

/// Ceilings on the recurring read, by route, in words: **the measured post-split figure, with no
/// slack.**
///
/// A ratchet. 24,019 and 29,326 before the split; 17,332 and 22,639 with all three files split,
/// which is 6,687 words out of both routes. Set to the measurement rather than above it for the
/// same reason the floor is: slack is the room a regression needs, and here the regression is an
/// essay growing back into a rules file.
///
/// **Raised to 17,686 and 22,993 when the glossary arrived (#957).** No-slack is what makes this
/// number go red on a deliberate addition as readily as on a regression, so a raise has to say
/// what it bought. Both routes rose by **exactly 354** — the 334-word `GLOSSARY.md` and the
/// 20-word bullet that puts it on the list — and the equality across two routes with different
/// read lists is the evidence that nothing else grew under cover of the same edit. Check that
/// before raising this again: a raise whose delta is not attributable to a named file is a
/// regression being waved through, which is precisely what a ratchet exists to make visible.
///
/// **`sadhana/root/AGENTS.md` raised to 23,051 when the local gate grew the commit-vocabulary
/// check (#938).** The delta is **58 words in that one file** — `git show HEAD:…` measured 1,515
/// against 1,573 — and the evidence that nothing else rode along is that the *other* route did
/// not move at all. That is the right control here and equality would have been the wrong one:
/// the file is a derived repository's conduct doc and is on one of the two read lists, so a rise
/// on both would mean something else had grown.
///
/// The glossary is also the one file here that reduces the *effective* read rather than adding
/// to it — six of the files above use *rigpa*, *ma* and *tonpa* as though defined — so the
/// trade is 354 words against the vocabulary the other 17,332 assume.
///
/// **The form does not reach #933's ~3,000 on its own, and the remaining weight says why.**
/// `directories.md` is the largest file left on the read at 7,295 words, and what is left in it
/// after the essays moved is reference — what belongs in each of twenty directories, the catalog
/// frontmatter shape, the capability manifest shape, the authorship table. That is rule, not
/// essay, so no further splitting retires it. Getting under 3,000 needs a different move: a read
/// scoped to the occasion, where an agent about to write a node is handed the node conventions
/// and not the vault routing table. That is a separate change to how a route is written, and
/// this ceiling is what will hold it honest.
/// **Both routes raised by 361 when `directories.md` documented `.yidam/computed/` (#1028).**
/// 17,686 → 18,047 and 23,051 → 23,412. The delta is **361 words in that one file** — 7,295 to
/// 7,656 — and here *equality of the two deltas* is the control, which is the opposite of the
/// reading #938 needed: `directories.md` is on both read lists, so a raise of exactly the same
/// size on each is what says one shared file grew and nothing local to either route rode along.
/// A difference between them would be the thing to investigate.
///
/// The section is reference rather than essay — the shape of a signal table, how a row is keyed,
/// what happens to a file that declares no version — which is the category the paragraph above
/// says no further splitting retires. Its three arguments are 364 further words in
/// `directories.evidence.md`, charged to [`PAIR_FLOOR`] and reached only by a `[why]` link.
/// **Both routes raised by 28 when `GRAPH.md` gained the `number` rule (#1030, RFC-0040).**
/// 18,047 → 18,075 and 23,412 → 23,440. The delta is **28 words in that one file** — the
/// sentence saying a number is unquoted and its unit is on the class — and the equality of the
/// two deltas is again the control: `GRAPH.md` is on both lists. Its argument is 87 further
/// words in `GRAPH.evidence.md`, charged to [`PAIR_FLOOR`] and reached only by a `[why]` link.
const READ_CEILING: &[(&str, usize)] = &[("AGENTS.md", 18_075), ("sadhana/root/AGENTS.md", 23_440)];

/// Ceiling on the bootstrap path, in words: **the measured figure at `b52e031`, with no slack.**
///
/// The path is everything an agent reads between a fresh clone and its first node:
/// `.claude/CLAUDE.md`, which sends it to `BOOTSTRAP.md`, which sends it to the skill, whose
/// step 1 lists the prelude. Ten files. #933 published 14,670 words for this path from four
/// of them, and the figure was quoted forward until #960 counted all ten: **34,142 before the
/// split, 27,883 after** — an 18.3% cut, and a different claim.
///
/// This exists because the split was work whose stated purpose was to shrink this path, and
/// `bootstrap.md` — the largest file on it at 8,918 words, 32% of the total — grew twice
/// during it (8,642 → 8,824 → 8,918) with nothing to say so. [`READ_CEILING`] could not: the
/// recurring read deliberately excludes the skill, because a bootstrap is a different occasion
/// from a session. So the two ceilings measure two occasions, and a file on both is charged
/// to both.
///
/// **Lowered to 26,323 when the skill was split (#960).** One file changed: `bootstrap.md`
/// went from 8,918 words to 7,358, and 27,883 − 1,560 is this figure exactly, so the whole
/// delta is attributable to the one file the edit named. The 2,606-word evidence file is not
/// on this path — it is reached by a `[why]` link, never by a step — and [`PAIR_FLOOR`] is
/// what keeps the moved reasoning from thinning.
///
/// **Raised to 26,392 when the scaffold step gained `sadhana/config.toml` (#916).** One file
/// changed: `bootstrap.md` went from 7,358 words to 7,427, and 26,323 + 69 is this figure
/// exactly. The step had to grow because the template is the deliverable — a config file that
/// is read, documented and never scaffolded is the defect #916 names, and the skill is what
/// puts it in a derived repository.
///
/// The same raise discipline as [`READ_CEILING`]: a raise has to name the file and the words.
///
/// **Raised to 26,563 when step 8.5 got a branch for an uninstallable CLI (#936).** The delta is
/// **171 words in `bootstrap.md`** — 7,427 to 7,598, and 26,392 + 171 is this figure exactly, so
/// nothing else on the path moved under the same edit. The step's whole argument for the branch
/// is 229 further words in `bootstrap.evidence.md`, which is reached by a `[why]` link and is
/// charged to [`PAIR_FLOOR`] rather than here: a rule that costs every bootstrap 171 words and
/// an argument that costs the ones who follow the link is the split working as intended.
/// **Raised to 26,924 when `directories.md` documented `.yidam/computed/` (#1028).** The delta
/// is **361 words in `directories.md`** — 7,295 to 7,656, and 26,563 + 361 is this figure
/// exactly, so nothing else on the path moved. A bootstrapping agent is charged for it because
/// the directory is one a scaffold may create and a run does write: the alternative is finding
/// out what `.yidam/computed/` is from a `doctor` warning.
/// **Raised to 26,952 when `GRAPH.md` gained the `number` rule (#1030, RFC-0040).** The delta is
/// **28 words in `GRAPH.md`**, the same 28 the two recurring routes moved by, and 26,924 + 28 is
/// this figure exactly, so nothing else on the path moved.
const BOOTSTRAP_CEILING: usize = 26_952;

/// The two files a fresh clone opens before anything under `yidam/prelude/`.
///
/// Fixed rather than discovered, because they are the entry by construction: the harness loads
/// `.claude/CLAUDE.md` unasked, and it names `BOOTSTRAP.md` as the next read. The chain from
/// there is verified — [`bootstrap_path`] asserts each link before charging the next file.
const ENTRY: &[&str] = &[".claude/CLAUDE.md", "BOOTSTRAP.md"];

/// Floors on a split pair's combined word count: **the measured post-split total, with no
/// slack.**
///
/// The other half of the discriminator, and the half that was wrong first. `agent-conduct.md`
/// was 4,723 words as one file and is 5,399 as two — headings, two file headers, and the
/// connective sentence each moved essay needs once it is no longer sitting under the rule it
/// explains. Setting the floor to the *pre-split* figure looks conservative and is the bug: it
/// leaves 676 words of slack, and a mutation that trimmed every evidence section to its first
/// two sentences deleted 531 words of reasoning and stayed green. Slack in this floor is
/// precisely the room a thinning edit needs.
///
/// So the floor is the measurement, and a copy-edit that genuinely retires a word turns it red.
/// That is the intended cost rather than friction to design around: the one outcome #933 rules
/// out is losing the reasoning, and making its removal a recorded edit to a constant here is
/// what "recorded" means. Re-measure, change the number, and say why in the commit.
///
/// A pair absent from this list is not exempt — [`every_pair_has_a_floor`] fails until somebody
/// records one.
///
/// **`bootstrap.md` re-measured to 10,433 for #936.** 400 words arrived — 171 in the rules half,
/// 229 in the evidence half — and the floor moves by 469, because 69 more had been standing as
/// slack before this edit: the raise before it raised [`BOOTSTRAP_CEILING`] for a change to the
/// same file and left this number where it was. That is the drift a no-slack floor is against,
/// so it is closed here rather than carried forward, and the two halves of the move are stated
/// separately so a later reader can tell them apart.
///
/// **`directories.md` re-measured to 11,686 for #1028.** 725 words arrived — 361 in the rules
/// half, 364 in the evidence half — and the floor moves by all of it, because 10,961 was the
/// measurement and carried no slack. The two halves are near enough to equal on purpose: each
/// of the three rules the section states is a decision with a declined alternative, and the
/// alternative is what the evidence half is for.
const PAIR_FLOOR: &[(&str, usize)] = &[
    ("yidam/prelude/guidelines/agent-conduct.md", 5_399),
    ("yidam/prelude/GRAPH.md", 8_818),
    ("yidam/prelude/guidelines/directories.md", 11_686),
    ("yidam/prelude/skills/bootstrap.md", 10_433),
];

fn read(rel: &str) -> String {
    let p = repo_root().join(rel);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{} is unreadable ({e})", p.display()))
}

fn words(s: &str) -> usize {
    s.split_whitespace().count()
}

// ── discovery ─────────────────────────────────────────────────────────────────

/// Every rules/evidence pair in the prelude, as `(rules path, evidence path)`.
///
/// Discovered from the tracked set by suffix, never listed: the whole point of the form is that
/// splitting another file brings it under these checks without editing them.
fn pairs() -> Vec<(String, String)> {
    let mut found = Vec::new();
    for path in tracked_under(&repo_root(), "yidam/prelude/") {
        if let Some(stem) = path.strip_suffix(EVIDENCE_SUFFIX) {
            found.push((format!("{stem}.md"), path.clone()));
        }
    }
    found.sort();
    found
}

/// The recurring-read routes, discovered rather than named.
///
/// A route is tracked markdown outside `docs/` that markdown-*links* all three of
/// `IDENTITY.md`, `GRAPH.md` and `agent-conduct.md`. Each clause earns its place:
///
/// - **Links, not mentions.** An RFC discussing the model names these files in prose;
///   `docs/rfcs/0028-kuten-layer.md` is picked up by a mention rule and is not a route.
/// - **Outside `docs/`.** The prose *about* the template is not the prose an agent is told to
///   read before acting.
/// - **All three.** `IDENTITY.md` + `GRAPH.md` alone also matches `yidam/README.md`,
///   `sadhana/root/README.md`, `yidam/prelude/README.md` and two kuten profiles — orientation
///   documents that link the model without prescribing the conduct read. Requiring
///   `agent-conduct.md` is what separates a route from a pointer, and it is why the ceiling
///   measures two files rather than seven.
fn routes() -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    for path in tracked_under(&repo_root(), ".") {
        if !path.ends_with(".md") || path.starts_with("docs/") {
            continue;
        }
        let body = read(&path);
        let links_to = |name: &str| {
            body.match_indices(name).any(|(at, _)| {
                body[..at]
                    .rfind("](")
                    .is_some_and(|open| !body[open..at].contains(')'))
            })
        };
        if links_to("IDENTITY.md") && links_to("GRAPH.md") && links_to("agent-conduct.md") {
            found.insert(path);
        }
    }
    found
}

/// The `##` section of a route that prescribes the read: the one that links all three of
/// `IDENTITY.md`, `GRAPH.md` and `agent-conduct.md`.
///
/// Scoping to a section rather than reading the whole file is not tidiness. `AGENTS.md` links
/// `yidam/prelude/skills/bootstrap.md` from its *first* section, addressed to an agent
/// bootstrapping a fresh clone — a different occasion, and 8,824 words of it. Counting the file's
/// every link put bootstrap inside a figure that claims to be the per-session read and inflated
/// it by 36%. The section is found the same way the route is, so the two cannot disagree.
fn prescribing_section(route: &str) -> String {
    let body = read(route);
    let mut best = String::new();
    let mut current = String::new();
    let flush = |sec: &str, best: &mut String| {
        let links_all = ["IDENTITY.md", "GRAPH.md", "agent-conduct.md"]
            .iter()
            .all(|n| sec.contains(n));
        if links_all && sec.len() > best.len() {
            *best = sec.to_string();
        }
    };
    for line in body.lines() {
        if line.starts_with("## ") {
            flush(&current, &mut best);
            current.clear();
        }
        current.push_str(line);
        current.push('\n');
    }
    flush(&current, &mut best);
    assert!(
        !best.is_empty(),
        "{route} was discovered as a route but no single `##` section of it links all three of \
         IDENTITY.md, GRAPH.md and agent-conduct.md — the read list is spread across sections \
         and this measurement would be guessing at its extent"
    );
    best
}

/// Every prelude file a route's read list names, resolved against the route's own directory.
///
/// This is what makes the ceiling a measurement rather than a restatement: adding a file to a
/// route's read list moves the number, and so does growing a file already on it.
fn route_cost(route: &str) -> (usize, Vec<(String, usize)>) {
    let mut total = words(&read(route));
    let mut parts = vec![(route.to_string(), total)];
    let body = prescribing_section(route);
    let mut seen = BTreeSet::new();
    for (at, _) in body.match_indices("](") {
        let rest = &body[at + 2..];
        let Some(end) = rest.find(')') else { continue };
        let target = rest[..end].split('#').next().unwrap_or("");
        if !target.ends_with(".md") || target.starts_with("http") {
            continue;
        }
        // Routes are written for where they install, so a derived route's
        // `.yidam/.vendor/prelude/x` is this tree's `yidam/prelude/x`.
        let rel = match target.split_once(".yidam/.vendor/prelude/") {
            Some((_, tail)) => format!("yidam/prelude/{tail}"),
            None => {
                let dir = PathBuf::from(route)
                    .parent()
                    .map(|p| p.to_string_lossy().into_owned())
                    .unwrap_or_default();
                let joined = if dir.is_empty() {
                    target.to_string()
                } else {
                    format!("{dir}/{target}")
                };
                match joined.split_once("yidam/prelude/") {
                    Some((_, tail)) => format!("yidam/prelude/{tail}"),
                    None => continue,
                }
            }
        };
        if !repo_root().join(&rel).is_file() || !seen.insert(rel.clone()) {
            continue;
        }
        let w = words(&read(&rel));
        total += w;
        parts.push((rel, w));
    }
    (total, parts)
}

/// Every file on the bootstrap path, in read order, with its words.
///
/// Each hop is checked before it is followed: `.claude/CLAUDE.md` has to name `BOOTSTRAP.md`,
/// `BOOTSTRAP.md` has to link the skill, and the skill's step 1 has to list something. A path
/// assembled from constants would still total something if a hop were rewritten to point
/// elsewhere, and the ceiling would then be holding a route no agent walks.
fn bootstrap_path() -> Vec<(String, usize)> {
    let [claude_md, bootstrap_md] = ENTRY else {
        unreachable!("ENTRY is the two files a clone opens first")
    };
    assert!(
        read(claude_md).contains(&format!("`{bootstrap_md}`")),
        "{claude_md} no longer names {bootstrap_md}; the bootstrap path starts somewhere else now"
    );
    assert!(
        read(bootstrap_md).contains(&format!("]({BOOTSTRAP_SKILL})")),
        "{bootstrap_md} no longer links {BOOTSTRAP_SKILL}; the bootstrap path goes somewhere \
         else now"
    );
    let step_one = step_one_read_list();
    assert!(
        !step_one.is_empty(),
        "step 1 of {BOOTSTRAP_SKILL} lists no files; the parse broke rather than the list \
         emptying, and the ceiling would be measuring three files instead of ten"
    );
    ENTRY
        .iter()
        .map(|s| s.to_string())
        .chain(std::iter::once(BOOTSTRAP_SKILL.to_string()))
        .chain(step_one)
        .map(|rel| {
            let w = words(&read(&rel));
            (rel, w)
        })
        .collect()
}

/// `## slug` headings of an evidence file, with the words under each.
fn evidence_sections(rel: &str) -> BTreeMap<String, usize> {
    let body = read(rel);
    let mut sections = BTreeMap::new();
    let mut current: Option<String> = None;
    let mut count = 0usize;
    for line in body.lines() {
        if let Some(slug) = line.strip_prefix("## ") {
            if let Some(prev) = current.take() {
                sections.insert(prev, count);
            }
            current = Some(slug.trim().to_string());
            count = 0;
        } else if current.is_some() {
            count += words(line);
        }
    }
    if let Some(prev) = current {
        sections.insert(prev, count);
    }
    sections
}

/// Every `#slug` a rules file points into its own evidence file.
fn why_links(rules: &str, evidence: &str) -> Vec<String> {
    let body = read(rules);
    let file = evidence.rsplit('/').next().unwrap_or(evidence);
    let needle = format!("]({file}#");
    body.match_indices(&needle)
        .filter_map(|(at, _)| {
            let rest = &body[at + needle.len()..];
            rest.find(')').map(|end| rest[..end].to_string())
        })
        .collect()
}

// ── the checks ────────────────────────────────────────────────────────────────

#[test]
fn a_pair_is_discovered_and_both_halves_exist() {
    let pairs = pairs();
    assert!(
        !pairs.is_empty(),
        "no `*{EVIDENCE_SUFFIX}` file found under yidam/prelude/ — either the split was \
         reverted or this scan is looking at nothing, and every other check in this file \
         passes vacuously either way"
    );
    for (rules, evidence) in &pairs {
        assert!(
            repo_root().join(rules).is_file(),
            "{evidence} has no rules file beside it: expected {rules}. An evidence file \
             nothing points at is prose no reader will reach."
        );
    }
}

#[test]
fn every_why_link_resolves_to_a_section() {
    for (rules, evidence) in pairs() {
        let sections = evidence_sections(&evidence);
        let links = why_links(&rules, &evidence);
        assert!(
            !links.is_empty(),
            "{rules} points into {evidence} zero times — the rules were separated from their \
             evidence and nothing connects them back"
        );
        for slug in links {
            assert!(
                sections.contains_key(&slug),
                "{rules} links `#{slug}` and {evidence} has no `## {slug}`. Either the \
                 section was renamed and the rule not repointed, or the reasoning was deleted \
                 and the rule left behind pointing at nothing."
            );
        }
    }
}

#[test]
fn every_evidence_section_is_reached_by_a_rule() {
    for (rules, evidence) in pairs() {
        let links: BTreeSet<String> = why_links(&rules, &evidence).into_iter().collect();
        for slug in evidence_sections(&evidence).keys() {
            assert!(
                links.contains(slug),
                "{evidence} has `## {slug}` and no rule in {rules} links it. Evidence a \
                 reader cannot get to from a rule is the failure this split was supposed to \
                 prevent, arriving from the other direction."
            );
        }
    }
}

#[test]
fn every_evidence_section_carries_an_argument() {
    for (_, evidence) in pairs() {
        for (slug, count) in evidence_sections(&evidence) {
            assert!(
                count >= MIN_EVIDENCE_WORDS,
                "{evidence} `## {slug}` is {count} words, under the {MIN_EVIDENCE_WORDS}-word \
                 floor. A heading whose prose was deleted still resolves every link pointing \
                 at it; this is the check that notices."
            );
        }
    }
}

#[test]
fn the_scan_sees_the_known_routes() {
    let found = routes();
    // Not the whole expected set — a *lower bound*, so a new route is not a failure while a
    // scan that stops seeing these two is. Without this, `routes()` returning empty would
    // leave `the_recurring_read_stays_under_its_ceiling` asserting nothing at all.
    for known in ["AGENTS.md", "sadhana/root/AGENTS.md"] {
        assert!(
            found.contains(known),
            "route discovery did not find {known}. Found: {found:?}. The scan is broken, or \
             that route stopped prescribing the conduct read."
        );
    }
}

#[test]
fn every_route_has_a_ceiling() {
    let ceilings: BTreeMap<&str, usize> = READ_CEILING.iter().copied().collect();
    for route in routes() {
        assert!(
            ceilings.contains_key(route.as_str()),
            "{route} prescribes the conduct read and has no entry in READ_CEILING. A new \
             recurring read is a cost somebody should have to write down."
        );
    }
}

#[test]
fn the_recurring_read_stays_under_its_ceiling() {
    for (route, ceiling) in READ_CEILING {
        let (total, parts) = route_cost(route);
        assert!(
            total <= *ceiling,
            "the recurring read at {route} is {total} words, over its {ceiling}-word \
             ceiling.\n{}",
            parts
                .iter()
                .map(|(p, w)| format!("  {w:>7}  {p}\n"))
                .collect::<String>()
        );
    }
}

#[test]
fn the_bootstrap_path_stays_under_its_ceiling() {
    let parts = bootstrap_path();
    let total: usize = parts.iter().map(|(_, w)| w).sum();
    assert!(
        total <= BOOTSTRAP_CEILING,
        "the bootstrap path is {total} words, over its {BOOTSTRAP_CEILING}-word ceiling. If a \
         file on it grew on purpose, raise the ceiling by exactly that file's delta and name it \
         in the commit; if nothing was meant to grow, this is the regression the ceiling is \
         for.\n{}",
        parts
            .iter()
            .map(|(p, w)| format!("  {w:>7}  {p}\n"))
            .collect::<String>()
    );
}

#[test]
fn every_pair_has_a_floor() {
    let floors: BTreeMap<&str, usize> = PAIR_FLOOR.iter().copied().collect();
    for (rules, _) in pairs() {
        assert!(
            floors.contains_key(rules.as_str()),
            "{rules} was split and has no entry in PAIR_FLOOR. Without one the pair may be \
             thinned to nothing and every other check here still passes."
        );
    }
}

#[test]
fn a_split_pair_does_not_shrink() {
    let floors: BTreeMap<&str, usize> = PAIR_FLOOR.iter().copied().collect();
    for (rules, evidence) in pairs() {
        let Some(floor) = floors.get(rules.as_str()) else {
            continue; // reported by `every_pair_has_a_floor`
        };
        let rules_words = words(&read(&rules));
        let evidence_words = words(&read(&evidence));
        let total = rules_words + evidence_words;
        assert!(
            total >= *floor,
            "{rules} ({rules_words}) + {evidence} ({evidence_words}) = {total} words, under \
             the {floor}-word floor this pair was split at. The read is allowed to get \
             shorter; the reasoning is not allowed to get thinner. If prose was genuinely \
             retired rather than moved, lower the floor in PAIR_FLOOR and say why in the \
             commit."
        );
    }
}
