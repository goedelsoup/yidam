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
//! # A bootstrap is an occasion too
//!
//! The recurring read is a set of occasions, one ceiling each (RFC-0039). A bootstrap is
//! another occasion, and the routes exclude the skill on purpose. [`BOOTSTRAP_CEILING`] holds the second — the ten files between a fresh
//! clone and a first node — because #933 set out to measure exactly that path, quoted a figure
//! from four of its ten files, and the largest of the ten grew twice during the work that was
//! meant to shrink it (#960).
//!
//! # A fragment is a claim about a heading
//!
//! [`every_why_link_resolves_to_a_section`] holds a rule's `[why]` link to its evidence file,
//! and nothing held any other `#fragment` into the prelude (#970). The docs site's
//! `check-anchors.mjs` grades fragments on its own pages only, and `installed_layout_links`
//! resolves a link to a *file*. So a link to `GRAPH.md#the-class-contact` would ship green
//! everywhere and put every reader at the top of `GRAPH.md`.
//! [`every_prelude_fragment_resolves_to_a_heading`] closes that for every tracked markdown
//! file under the prelude and both routes. It is the check RFC-0039's section links will stand
//! on, and it is written ahead of them.
//!
//! The slug is the docs site's, not GitHub's. The two differ only on a heading that ends in
//! punctuation after a space, where Astro drops the trailing `-`.
//! [`the_slugger_agrees_with_the_docs_render`] pins the rule to output read off a real Astro
//! render, not to a reading of `github-slugger`'s source.
//!
//! # A section is read by something, or says why not
//!
//! The ceilings hold each occasion to what it reads, and nothing held the prelude to being read
//! at all (#974). [`every_prelude_section_is_read_or_reference_only`] requires every `##` section
//! of the seven step-1 files to be reached on each route, by an occasion or by the bootstrap.
//! If it is not, [`REFERENCE_ONLY`] must list it with the occasion that would read it.

use std::collections::{BTreeMap, BTreeSet};

use crate::common;

use common::{
    blank_code_spans, install_of, repo_root, resolve, step_one_read_list, tracked_under,
    BOOTSTRAP_SKILL,
};

/// The suffix that marks an evidence file, and the thing that makes a pair discoverable.
const EVIDENCE_SUFFIX: &str = ".evidence.md";

/// Words below which an evidence section is not carrying an argument.
///
/// Deliberately low. This is not a quality bar — nothing here can judge an argument — it is the
/// line between a section that says something and a heading left behind after its prose was
/// deleted, which is the failure the floor above is aimed at and this check localizes.
const MIN_EVIDENCE_WORDS: usize = 25;

/// Ceilings on the recurring read, by route and occasion, in words: **the measured figure, with
/// no slack.**
///
/// A route used to be one list of whole files, and this was one ceiling per route. It was a
/// ratchet from #954 to #1120: 24,019 and 29,326 words before the rules/evidence split, 17,332
/// and 22,639 after it, and 18,836 and 24,260 at the last raise. Every raise on the way had to
/// name the file and the words it bought, and the equality of the two routes' deltas was the
/// control that nothing else had grown with it. That history is in `git log -p` on this file.
///
/// The split could not reach #933's ~3,000 on its own. What was left in `directories.md` after
/// its essays moved was reference, not essay, and no further splitting retires reference.
/// RFC-0039 took the other move: a route names **occasions**, by the commit verb they end in,
/// and each occasion names the **sections** it needs (#972). So a ceiling is per occasion. An
/// occasion's read is the route file itself, plus *On every occasion*, plus that occasion's own
/// list. A link with a `#` is charged from its heading to the next heading at the same level or
/// higher; one without is charged the whole file. A section two lists share is charged once.
///
/// **There is no ceiling on the union**, and that is deliberate. The union is the old whole-file
/// read by another route, and a union ceiling would pass an edit that moved every word from one
/// occasion's list into another's. Per occasion, the same edit turns two entries red, each
/// naming its occasion.
///
/// **Measured at #973.** Against 18,836 and 24,260 for the whole-file read, the template's
/// five occasions are 6,157 (write a node), 4,584 (run a phase), 4,310 (cross a corpus
/// boundary), 4,564 (change a class) and 2,959 (retrieve). The core every occasion reads is
/// 1,092 of it: the glossary, the identity, and three sections of `GRAPH.md`. Writing a node is
/// the heaviest because it reads the whole class contract, 2,621 words, and the contract is rule
/// all the way down: its lead alone is 327 and says nothing about properties.
///
/// `sadhana/root/AGENTS.md` is exactly 1,194 words heavier on every occasion, because the route
/// file itself is: 1,920 words against the template's 726. The equal difference is the control
/// that the two routes read the same sections. The reference list at the bottom of each route
/// is not charged. It is what an occasion *not* named here reads, and none of these do.
///
/// The same raise discipline as before: a raise names the section and the words. Two occasions
/// moving by the same delta is the sign of a shared section; one moving alone is the sign of
/// a section on its list only.
///
/// **Raised by 22 on *Before you run a phase* when Investigation's missing verb was stated
/// (#1136).** The delta is **22 words in `PHASES.md`**, on that occasion's list only, and both
/// routes moved by it. The same change moved `extract`, `open` and `close` to *Before you write
/// or revise a node*, which costs nothing: a verb is in the route file, charged to every
/// occasion, and moving it leaves the file's length where it was.
///
/// **Raised on three occasions when a node could declare a refusal (#1053).** Two deltas, each
/// on the lists that link its section, and both routes moved by each:
///
/// - **91 words in `GRAPH.md`**, the `refuses:` subsection, on *Before you write or revise a
///   node* and *Before you change a class* — the two occasions that link *The class contract*.
///   It was drafted under *Nodes* first, which every occasion reads, and moved when that
///   charged all five for a key only a node's author writes.
/// - **72 words in `agent-conduct.md`**, the third outbound rule's grammar, on *Before a claim
///   crosses a corpus boundary* only.
///
/// **Raised on two occasions when an edge's `source:` was resolved (#1067).** Two deltas, each
/// on the lists that link its section, and both routes moved by each:
///
/// - **103 words in `GRAPH.md`**, the `edge-source-unresolved` row and paragraph under *What an
///   edge rests on*, on *Before you write or revise a node* and *Before you change a class* —
///   the two occasions that link *The class contract*, as the #1053 raise found.
/// - **85 words in `agent-conduct.md`**, the fourth bullet under *An edge is a claim*, on
///   *Before you write or revise a node* only.
///
/// The argument for the three spellings the check admits is in the evidence halves and charged
/// to [`PAIR_FLOOR`].
///
/// **Raised on two occasions when `GRAPH.md` gained the `quotation` rule (#1070, RFC-0046).**
/// The delta is **47 words in `GRAPH.md`**, under *The class contract*, on *Before you write or
/// revise a node* and *Before you change a class* — the two occasions that link it. The
/// argument is in `GRAPH.evidence.md` and charged to [`PAIR_FLOOR`].
///
/// **Raised on the same two occasions when `GRAPH.md` gained sink classes (#1072).** The delta
/// is **50 words in `GRAPH.md`**, under *Which classes are source classes*: the paragraph naming
/// the converse and the `only-instance-of` warning it exempts from. The measurement is in
/// `GRAPH.evidence.md` and charged to [`PAIR_FLOOR`].
///
/// **Raised on the same two occasions when `GRAPH.md` gained `interval:` (#1201).** The delta
/// is **73 words in `GRAPH.md`**: the `interval-overlap` row and the paragraph naming what the
/// declaration checks. The argument is in `GRAPH.evidence.md` and charged to
/// [`PAIR_FLOOR`].
///
/// **Raised again on both when `interval:` gained `capacity:` (#1205).** The delta is **13
/// words in `GRAPH.md`**: the check-table row and one clause of the same paragraph. The
/// measurement is in `GRAPH.evidence.md` and charged to [`PAIR_FLOOR`].
///
/// **Raised again on both when `interval:` gained `complete:` (#1213).** The delta is **74
/// words in `GRAPH.md`**: the `interval-gap` row and the paragraph naming the mark. The
/// measurement is in `GRAPH.evidence.md` and charged to [`PAIR_FLOOR`].
///
/// **Raised on *Before you retrieve*, on both routes, when `query` gained `--paths` (#1202).**
/// The delta is **30 words in `reading-the-corpus.md`**: one sentence under *`yidam query`* and
/// one row of the loop table. The worked example went to `docs/cli-reference.md` instead, where
/// no occasion pays for it.
const READ_CEILING: &[(&str, &str, usize)] = &[
    ("AGENTS.md", "Before you write or revise a node", 6_693),
    ("AGENTS.md", "Before you run a phase", 4_606),
    (
        "AGENTS.md",
        "Before a claim crosses a corpus boundary",
        4_382,
    ),
    ("AGENTS.md", "Before you change a class", 5_015),
    ("AGENTS.md", "Before you retrieve", 2_989),
    (
        "sadhana/root/AGENTS.md",
        "Before you write or revise a node",
        7_887,
    ),
    ("sadhana/root/AGENTS.md", "Before you run a phase", 5_800),
    (
        "sadhana/root/AGENTS.md",
        "Before a claim crosses a corpus boundary",
        5_576,
    ),
    ("sadhana/root/AGENTS.md", "Before you change a class", 6_209),
    ("sadhana/root/AGENTS.md", "Before you retrieve", 4_183),
];

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
///
/// **Raised again to 26,985 when the third capability type became declarable (#1027).** The delta
/// is **61 further words in `directories.md`** — 7,656 to 7,717, and 26,924 + 61 is this figure
/// exactly, so nothing else on the path moved under the same edit. The document typed the domain
/// computer as three kinds while the manifest comment beside its own example offered two, and a
/// derived repository could only find out which was true by reading `manifest.rs`, of which it has
/// no copy. 36 of the 61 are that rule and 25 are the declared value, which `featurizer` does not
/// lowercase out of *Feature engineering* the way the other two do. The 107 further words of
/// argument are in `directories.evidence.md` and charged to [`PAIR_FLOOR`].
///
/// **Raised to 27,013 when `GRAPH.md` gained the `number` rule (#1030, RFC-0040).** The delta is
/// **28 words in `GRAPH.md`**, the same 28 the two recurring routes moved by, and 26,985 + 28 is
/// this figure exactly, so nothing else on the path moved.
///
/// **Raised to 27,034 when a retype learned to requote (#1044).** The delta is **21 words in
/// `GRAPH.md`**, the same 21 the two recurring routes moved by, and 27,013 + 21 is this figure
/// exactly, so nothing else on the path moved. The 81 further words of argument are in
/// `GRAPH.evidence.md` and charged to [`PAIR_FLOOR`].
///
/// **Raised to 27,344 when a step learned it is handed a resolved corpus (#1080).** The delta is
/// **310 words in `directories.md`**, the same 310 the `AGENTS.md` route moved by, and 27,034 +
/// 310 is this figure exactly, so nothing else on the path moved. The 131 further words of
/// argument are in `directories.evidence.md` and charged to [`PAIR_FLOOR`].
///
/// **Raised to 27,553 when `run` gained its second arm (#1091, RFC-0042).** The delta is **209
/// words in `directories.md`**, the same 209 the two recurring routes moved by, and 27,344 + 209 is
/// this figure exactly, so nothing else on the path moved. The 100 further words of argument are in
/// `directories.evidence.md` and charged to [`PAIR_FLOOR`].
///
/// **Raised to 27,610 when the typed arm gained streamflow's chain rule (#1102).** The delta is
/// **57 words in `directories.md`**, the same 57 the two recurring routes moved by, and 27,553 +
/// 57 is this figure exactly. Recorded here after the fact: `3bcf2ed3` raised the number without
/// a note.
///
/// **Raised to 27,658 when `GRAPH.md` gained `claim-property-undeclared` (#1069).** The delta is
/// **48 words in `GRAPH.md`**, the same 48 the two recurring routes moved by, and 27,610 + 48 is
/// this figure exactly, so nothing else on the path moved. The 280 further words of argument are
/// in `GRAPH.evidence.md` and charged to [`PAIR_FLOOR`].
///
/// **Raised to 27,784 when step 8.5 moved `lint --init-baseline` behind `lint` (#1060).** The
/// delta is **126 words in `bootstrap.md`**, and 27,658 + 126 is this figure exactly, so nothing
/// else on the path moved. The 241 further words of argument are in `bootstrap.evidence.md` and
/// charged to [`PAIR_FLOOR`].
///
/// **Raised to 27,843 when `settle` learned to refresh the REGEN blocks (#1066).** The delta is
/// **59 words in `PHASES.md`**, the same 59 the derived-repository route moved by, and 27,784 +
/// 59 is this figure exactly, so nothing else on the path moved. `PHASES.md` has no evidence
/// half, so nothing is charged to [`PAIR_FLOOR`].
///
/// **Raised to 27,874 when `GRAPH.md` gained the `values:` rule (#1052, RFC-0044).** The delta
/// is **31 words in `GRAPH.md`**, the same 31 both recurring routes moved by, and 27,843 + 31 is
/// this figure exactly, so nothing else on the path moved. The 120 further words of argument
/// are in `GRAPH.evidence.md` and charged to [`PAIR_FLOOR`].
///
/// **Raised to 27,898 when `GRAPH.md` gained `migrate value` (#1120).** The delta is **24 words
/// in `GRAPH.md`**, the same 24 both recurring routes moved by, and 27,874 + 24 is this figure
/// exactly, so nothing else on the path moved.
///
/// **Raised to 27,908 when the scaffold gained `mise.overrides.toml` (#1064).** The delta is
/// **10 words in `bootstrap.md`**: the new file's name in step 3's read list and install table,
/// and the sentence saying it installs new. 27,898 + 10 is this figure exactly, so nothing else
/// on the path moved. Nothing is argued in the evidence half; the reason lives in the file
/// itself, which is what a derived repository reads.
///
/// **Raised to 27,930 when `PHASES.md` said Investigation has no verb of its own (#1136).** The
/// delta is **22 words in `PHASES.md`**, the same 22 *Before you run a phase* moved by on both
/// recurring routes, and 27,908 + 22 is this figure exactly, so nothing else on the path moved.
///
/// **Raised to 27,944 when `.yidam.toml` gained `cli` (#1076).** The delta is **14 words in
/// `directories.md`**, the manifest example's new line. 27,930 + 14 is this figure exactly, so
/// nothing else on the path moved. Neither recurring route moved: the line sits in a section no
/// occasion links, and a route is charged by section.
///
/// **Raised to 28,107 when a node could declare a refusal (#1053).** Two files, both on the
/// path whole: **91 words in `GRAPH.md`** (the `refuses:` subsection) and **72 in
/// `agent-conduct.md`** (the third outbound rule's grammar). 27,944 + 163 is this figure
/// exactly, and the two deltas are the ones [`READ_CEILING`] moved by.
///
/// **Raised to 28,295 when an edge's `source:` was resolved (#1067).** Two files, both on the
/// path whole: **103 words in `GRAPH.md`** (the `edge-source-unresolved` row and paragraph)
/// and **85 in `agent-conduct.md`** (the fourth bullet under *An edge is a claim*). 28,107 +
/// 188 is this figure exactly, and the two deltas are the ones [`READ_CEILING`] moved by.
///
/// **Raised to 28,342 when `GRAPH.md` gained the `quotation` rule (#1070, RFC-0046).** The delta
/// is **47 words in `GRAPH.md`**, the same 47 both recurring routes moved by, and 28,295 + 47 is
/// this figure exactly, so nothing else on the path moved.
///
/// **Raised to 28,398 when step 6 named the claim tags a seeded node carries (#968).** The
/// delta is **56 words in `bootstrap.md`**, and 28,342 + 56 is this figure exactly, so nothing
/// else on the path moved. The 119 further words of argument are in `bootstrap.evidence.md`
/// and charged to [`PAIR_FLOOR`].
///
/// **Lowered to 20,043 when step 1 stopped charging the sections about a corpus that exists
/// (#971, RFC-0039).** From here the path is measured per section, not per file: a step 1 row
/// may link sections to skip or to read alone, and [`step_one_rows`] charges what the row says.
/// That rests on #967's finding that an agent handed a section link reads the section and not
/// the file. Were it otherwise, this figure would describe a read nobody performs. Four files
/// fall, by **8,489 words**. Each figure is the whole file less what the row keeps:
///
/// - **`directories.md`, 4,745:** `.yidam/catalog/`, `.yidam/tonpa.toml`,
///   `.yidam/private-paths`, `.yidam/policy/`, `.yidam/bin/`, `.yidam/capabilities.toml` with
///   its six subsections, and `.yidam/authorship.yml`. This retires the #1028 note's reason for
///   charging `.yidam/computed/`. The scaffold copies a `gitignore` line for it, and nothing in
///   the skill needs the section to do that.
/// - **`agent-conduct.md`, 1,560:** *Prefer a base rate to a refusal* with its subsection, and
///   the five sections from *When claims leave the repository* to *The safeguards were built
///   against carelessness*. *Mark claim confidence* stays, because #968 found every derived
///   corpus tagging claims at genesis from it.
/// - **`PHASES.md`, 1,330:** everything but *Phase types*.
/// - **`GRAPH.md`, 854:** *Residence time*, *The baseline, and its own clock* and *Branches as
///   inquiry*.
///
/// The issue's census found 7,386; the other 1,103 are words these sections gained after it was
/// filed. Against that, **`bootstrap.md` rose by 134 words** to carry the links and the
/// paragraph saying what a skipped section is. 28,398 − 8,489 + 134 is this figure exactly.
///
/// **Raised to 20,073 when step 7's calculator stub gained `status: stub` (#1063).** The delta
/// is **30 words in `bootstrap.md`**: the stub's frontmatter and the sentence saying what the
/// field keeps `skills-index` from counting. 20,043 + 30 is this figure exactly, so nothing else
/// on the path moved. Nothing is argued in the evidence half.
///
/// **Raised on the same path when `GRAPH.md` gained sink classes (#1072).** The delta is **50
/// words in `GRAPH.md`**, the same 50 both recurring routes moved by. 20,073 + 50 is this figure
/// exactly, so nothing else on the path moved.
///
/// **Raised to 20,221 when a domain article moved to `.yidam/constitution/` (#593, RFC-0047).**
/// The delta is **98 words**: 70 in `directories.md` for the new section, which a bootstrap reads
/// because step 1 is where the article is written; 16 in `CONSTITUTION.md`'s Domain extensions;
/// and 12 in `bootstrap.md`'s augmentation bullet. 20,123 + 98 is this figure exactly. The
/// argument for sealing by the genesis commit is in `directories.evidence.md`.
///
/// **Raised to 20,294 when `GRAPH.md` gained `interval:` (#1201).** The delta is **73 words in
/// `GRAPH.md`**, the same 73 both recurring routes moved by. 20,221 + 73 is this figure
/// exactly, so nothing else on the path moved.
///
/// **Raised to 20,307 when `interval:` gained `capacity:` (#1205).** The delta is **13 words in
/// `GRAPH.md`**, the same 13 both recurring routes moved by. 20,294 + 13 is this figure exactly.
///
/// **Raised to 20,381 when `interval:` gained `complete:` (#1213).** The delta is **74 words in
/// `GRAPH.md`**, the same 74 both recurring routes moved by. 20,307 + 74 is this figure exactly.
///
/// **Raised to 20,415 when step 7 declared an unrun calculator instead of stubbing a skill
/// (#1184).** The delta is **34 words in `bootstrap.md`**: the manifest template and the sentences
/// saying what an absent `run` is and why the skill directory is the wrong home. `directories.md`
/// gained its own paragraph in the manifest section, which this path skips. 20,381 + 34 is this
/// figure exactly.
const BOOTSTRAP_CEILING: usize = 20_415;

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
/// **`GRAPH.md` re-measured to 9,037 for #1044.** 102 words arrived — 21 in the rules half, 81 in
/// the evidence half — and the floor moves by 219, because 117 had been standing as slack: the
/// #1030 raise moved [`READ_CEILING`] and [`BOOTSTRAP_CEILING`] for a change to `GRAPH.md` and
/// left this number where it was, and the 87 words of argument it names were never charged here.
/// Closed rather than carried forward, for the reason the `bootstrap.md` note gives.
///
/// **`directories.md` re-measured to 11,686 for #1028.** 725 words arrived — 361 in the rules
/// half, 364 in the evidence half — and the floor moves by all of it, because 10,961 was the
/// measurement and carried no slack. The two halves are near enough to equal on purpose: each
/// of the three rules the section states is a decision with a declined alternative, and the
/// alternative is what the evidence half is for.
///
/// **`directories.md` re-measured to 11,854 for #1027.** 168 further words arrived — 61 in the
/// rules half, 107 in the evidence half — and 11,686 + 168 is this figure exactly, so the pair
/// carried no slack before this edit and carries none after it. The rules half is 7,717 and the
/// evidence half 4,137, read out of [`a_split_pair_does_not_shrink`]'s own message rather than
/// added up by hand.
///
/// **`directories.md` re-measured to 12,295 for #1080.** 441 further words arrived — 310 in the
/// rules half and 131 in the evidence half — and 11,854 + 441 is this figure exactly, so the pair
/// carried no slack before this edit and carries none after it. The rules half is 8,027 and the
/// evidence half 4,268. The split is more lopsided than the #1028 one because the section states
/// one rule with three consequences rather than three rules: what the file is, that it is sliced,
/// and that its digest is in the input state. The evidence half carries the two decisions — why
/// one resolver rather than one per calculator, and why one builder rather than two.
///
/// **`directories.md` re-measured to 12,604 for #1091.** 309 further words arrived — 209 in the
/// rules half and 100 in the evidence half — and 12,295 + 309 is this figure exactly, so the pair
/// carried no slack before this edit and carries none after it. The rules half is 8,236 and the
/// evidence half 4,368. The split is lopsided the way the #1080 one is because the section states
/// one field's second shape and its three consequences: the script is still declared, the budget is
/// declarable, and the arm is not in the downloaded binary. The evidence half carries the one
/// decision — why the arm stays outside the default set when the policy engine did not.
/// **`GRAPH.md` re-measured to 9,365 for #1069.** 328 words arrived — 48 in the rules half and
/// 280 in the evidence half — and 9,037 + 328 is this figure exactly, so the pair carried no
/// slack before this edit and carries none after it. The rules half is 5,498 and the evidence
/// half 3,867. The split is lopsided because the rule is one line and the reason for it is a
/// measurement: the evidence half carries the per-corpus table of what the check found, and the
/// argument for why a finding whose repair is sometimes a rename must not gate.
///
/// **`bootstrap.md` re-measured to 10,800 for #1060.** 367 words arrived — 126 in the rules half,
/// where step 8.5's gate now runs `lint` before `lint --init-baseline`, and 241 in the evidence
/// half, which carries the argument for why the order is the whole of it. 10,433 + 367 is this
/// figure exactly, and the 126 are the same 126 [`BOOTSTRAP_CEILING`] moved by.
///
/// **`GRAPH.md` re-measured to 9,516 for #1052.** 151 words arrived — 31 in the rules half, the
/// sentence saying a declared `values:` set is closed and matched exactly, and 120 in the
/// evidence half, which carries the 40 properties and 33 drifted instances the field exists
/// for. 9,365 + 151 is this figure exactly, and the 31 are the same 31 the three ceilings
/// moved by.
///
/// **`GRAPH.md` re-measured to 9,540 for #1120.** 24 words arrived, all in the rules half: the
/// migrate table's row for `migrate value`. 9,516 + 24 is this figure exactly, and the 24 are
/// the same 24 the three ceilings moved by. Nothing arrived in the evidence half, because the
/// row states an operation and the argument for it is RFC-0044's own.
///
/// **`bootstrap.md` re-measured to 10,810 for #1064.** 10 words arrived, all in the rules half:
/// step 3 names `mise.overrides.toml` among the root files. 10,800 + 10 is this figure exactly,
/// and the 10 are the same 10 [`BOOTSTRAP_CEILING`] moved by.
///
/// **`directories.md` re-measured to 12,618 for #1076.** 14 words arrived, all in the rules
/// half: the manifest example's `cli` line. 12,604 + 14 is this figure exactly, and the 14 are
/// the same 14 [`BOOTSTRAP_CEILING`] moved by. Nothing arrived in the evidence half; the reason
/// is the field's own and lives in `VERSIONING.md`.
///
/// **`agent-conduct.md` re-measured to 5,471 and `GRAPH.md` to 9,631 for #1053.** 72 and 91
/// words arrived, all in the rules halves: the third outbound rule's grammar and the `refuses:`
/// subsection. 5,399 + 72 and 9,540 + 91 are these figures exactly, and they are the deltas
/// [`BOOTSTRAP_CEILING`] moved by. The argument is RFC-0045's own.
///
/// **`GRAPH.md` re-measured to 9,948 and `agent-conduct.md` to 5,686 for #1067.** 317 and 215
/// words arrived — 103 and 85 in the rules halves, the `edge-source-unresolved` row, paragraph
/// and bullet, which are the deltas [`BOOTSTRAP_CEILING`] moved by; 214 and 130 in the
/// evidence halves, which carry the measurement the check rests on: 1,985 link sources over
/// four derived corpora, three spellings, all resolving. 9,631 + 317 and 5,471 + 215 are these
/// figures exactly, so neither pair carried slack before this edit and neither carries any
/// after it.
///
/// **`bootstrap.md` re-measured to 10,985 for #968.** 175 words arrived — 56 in the rules half,
/// where step 6 now names the claim tags, and 119 in the evidence half, which carries the
/// measurement behind them. 10,810 + 175 is this figure exactly, so the pair carried no slack
/// before this edit and carries none after it, and the 56 are the same 56 [`BOOTSTRAP_CEILING`]
/// moved by.
///
/// **`bootstrap.md` re-measured to 11,119 for #971.** 134 words arrived, all in the rules half:
/// step 1's section links and the paragraph that says what a skipped section is. 10,985 + 134
/// is this figure exactly, and the 134 are the same 134 [`BOOTSTRAP_CEILING`] set against
/// what it dropped. Nothing arrived in the evidence half. The argument is #967's measurement
/// and RFC-0039's own.
///
/// **`bootstrap.md` re-measured to 11,149 for #1063.** 30 words arrived, all in the rules half:
/// step 7's stub frontmatter and its `status:` sentence. 11,119 + 30 is this figure exactly, and
/// the 30 are the same 30 [`BOOTSTRAP_CEILING`] moved by.
///
/// **`GRAPH.md` re-measured to 10,330 for #1072.** 218 words arrived — 50 in the rules half, the
/// sink-class paragraph that [`BOOTSTRAP_CEILING`] moved by, and 168 in the evidence half, which
/// carries the 79 / 28 / 51 measurement over sixteen derived corpora. The floor moves by 382,
/// because 164 had been standing as slack: the #1070 raise moved the ceilings for its words and
/// left this number where it was. 10,112 + 218 is this figure exactly, and the slack is closed
/// here, for the reason the `bootstrap.md` note gives.
///
/// **`directories.md` re-measured to 12,820 and `bootstrap.md` to 11,161 for #593.** The
/// directories pair gained 145 words — 70 in the rules half, the `.yidam/constitution/`
/// section, and 75 in the evidence half, why the genesis commit is the seal — and the floor
/// moves by 202, because 57 had been standing as slack. Closed rather than carried forward, for
/// the reason the `bootstrap.md` note gives. `bootstrap.md` gained 12 in its rules half and none
/// in its evidence half, and 11,149 carried no slack.
///
/// **`GRAPH.md` re-measured to 10,531 for #1201.** 201 words arrived — 73 in the rules half,
/// the `interval-overlap` row and paragraph that [`BOOTSTRAP_CEILING`] moved by, and 128 in the
/// evidence half, which argues the half-open reading and the single-holder limit. 10,330 +
/// 201 is this figure exactly, so the pair carried no slack before this edit and carries none
/// after it.
///
/// **`GRAPH.md` re-measured to 10,585 for #1205.** 54 words arrived — 13 in the rules half, the
/// `capacity:` clause and row that [`BOOTSTRAP_CEILING`] moved by, and 41 in the evidence half,
/// the allen-county-ohio measurement: 19 findings without `capacity:`, 6 with it. 10,531 + 54 is
/// this figure exactly.
///
/// **`GRAPH.md` re-measured to 10,830 for #1213.** 245 words arrived — 74 in the rules half, the
/// `interval-gap` row and paragraph that [`BOOTSTRAP_CEILING`] moved by, and 171 in the evidence
/// half, the allen-county-ohio measurement against its succession calculator. 10,585 + 245 is
/// this figure exactly.
const PAIR_FLOOR: &[(&str, usize)] = &[
    ("yidam/prelude/guidelines/agent-conduct.md", 5_686),
    ("yidam/prelude/GRAPH.md", 10_830),
    ("yidam/prelude/guidelines/directories.md", 12_820),
    ("yidam/prelude/skills/bootstrap.md", 11_161),
];

/// The `##` sections of the seven step-1 files that no read reaches, as `file#slug` under
/// `yidam/prelude/`, each with the occasion that would read it if one existed.
///
/// The converse of the ceilings. [`READ_CEILING`] and [`BOOTSTRAP_CEILING`] hold each read to
/// what it costs, and nothing held the prelude to *being read*. RFC-0039's census found
/// `.yidam/authorship.yml` by hand: a section vendored into every derivation and read on no
/// occasion. [`every_prelude_section_is_read_or_reference_only`] finds the next one.
///
/// A reason names an occasion, not a quality of the section. This list is where a later
/// occasion, or `yidam read --for`, grows from, and "rarely needed" would tell it nothing.
///
/// **Measured at #974.** Nine sections, every one of them skipped by step 1 after #971. The
/// other skipped sections are on an occasion: *Branches as inquiry* on *Before you run a
/// phase*, `.yidam/catalog/` and four of `agent-conduct.md`'s on *Before a claim crosses a
/// corpus boundary*. No section is read in part. Each is read from its heading or not at all.
const REFERENCE_ONLY: &[(&str, &str)] = &[
    (
        "GRAPH.md#residence-time",
        "an occasion for triaging a `yidam lint` finding, which none of the five is",
    ),
    (
        "GRAPH.md#the-baseline-and-its-own-clock",
        "the same triage occasion, and a vendor update, which runs `lint --init-baseline`",
    ),
    (
        "guidelines/agent-conduct.md#the-safeguards-were-built-against-carelessness-not-against-interest",
        "an occasion for work where the agent has an interest in the answer. No commit verb \
         marks that, so no route can name it",
    ),
    (
        "guidelines/directories.md#yidamtonpatoml-and-yidamtonpa",
        "an occasion for adding or fetching a dependency corpus. *Before a claim crosses a \
         corpus boundary* is about a citation, not the dependency behind it",
    ),
    (
        "guidelines/directories.md#yidamprivate-paths-optional",
        "an occasion for publishing, or for deciding what may sit in a public repository",
    ),
    (
        "guidelines/directories.md#yidampolicy-optional",
        "the same publishing occasion, where a disclosure decision is made",
    ),
    (
        "guidelines/directories.md#yidambin",
        "an occasion for installing or upgrading the pinned binary",
    ),
    (
        "guidelines/directories.md#yidamcapabilitiestoml-yidamruns-and-yidamcomputed-optional",
        "an occasion for a pipeline act: `extract`, `refresh`, `compute`, `reconcile`",
    ),
    (
        "guidelines/directories.md#yidamauthorshipyml-optional",
        "the triage occasion, which is where a finding in inherited material has to be told \
         apart from one this repository can fix",
    ),
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

/// A prelude file and the `#fragment` a link names in it, `None` for the whole file.
type Link = (String, Option<String>);

/// An occasion, the words its read costs, and those words by file.
type Cost = (String, usize, Vec<(String, usize)>);

/// The heading under a route's prescribing section that every occasion reads.
const ALWAYS: &str = "On every occasion";

/// An occasion heading starts with this. `routes.rs` holds `routes.yml` to it.
const OCCASION: &str = "Before ";

/// Each `###` list in a route's prescribing section, as `(heading, [(prelude file, fragment)])`.
///
/// Only [`ALWAYS`] and the [`OCCASION`] headings. The reference list is what an occasion not
/// named here reads, and it is not charged to any occasion. A link that lands outside the
/// prelude fails here rather than being skipped: a route whose link rotted would read cheaper
/// than it is, and the ceiling would pass it.
fn occasion_lists(route: &str) -> Vec<(String, Vec<Link>)> {
    let installed = installed_prelude();
    let mut lists: Vec<(String, Vec<Link>)> = Vec::new();
    let mut charged = false;
    for (n, raw) in prose_lines(&prescribing_section(route)) {
        if let Some(heading) = raw.strip_prefix("### ") {
            let heading = heading.trim();
            charged = heading == ALWAYS || heading.starts_with(OCCASION);
            if charged {
                lists.push((heading.to_string(), Vec::new()));
            }
            continue;
        }
        if !charged {
            continue;
        }
        let line = blank_code_spans(raw);
        let mut j = 0;
        while let Some(open) = line[j..].find("](") {
            let at = j + open + 2;
            let Some(close) = line[at..].find(')') else {
                break;
            };
            let t = raw[at..at + close].trim();
            j = at + close + 1;
            let (path, fragment) = match t.split_once('#') {
                Some((p, f)) => (p, Some(f.to_string())),
                None => (t, None),
            };
            let file = prelude_target(route, path, &installed).unwrap_or_else(|| {
                panic!(
                    "{route}'s prescribing section, line {n}, links `{t}` under an occasion, \
                     and it is not a prelude file. An occasion's read is measured from prelude \
                     files, and this link would be charged nothing."
                )
            });
            lists
                .last_mut()
                .expect("a charged line follows a charged heading")
                .1
                .push((file, fragment));
        }
    }
    lists
}

/// The 0-based lines a link is charged: its section, or the whole file with no fragment.
///
/// A section runs from its heading to the next heading at the same level or higher, which is
/// what the route tells the agent to read.
fn section_lines(file: &str, fragment: Option<&str>) -> std::ops::Range<usize> {
    let text = read(file);
    let total = text.lines().count();
    let Some(fragment) = fragment else {
        return 0..total;
    };
    let heads = headings(&text);
    let at = heads
        .iter()
        .position(|(_, _, slug)| slug == fragment)
        .unwrap_or_else(|| panic!("{file} has no heading `#{fragment}`"));
    let (line, level, _) = heads[at];
    let end = heads[at + 1..]
        .iter()
        .find(|(_, l, _)| *l <= level)
        .map_or(total, |(n, _, _)| n - 1);
    line - 1..end
}

/// Each occasion of a route with the words its read costs, and those words by file.
///
/// The route file is charged whole, as it always was: the agent reads it to find its occasion.
/// Within a prelude file the charged lines are a union, so a section on *On every occasion*
/// and again on an occasion's own list is charged once.
fn occasion_costs(route: &str) -> Vec<Cost> {
    let lists = occasion_lists(route);
    let always: Vec<Link> = lists
        .iter()
        .find(|(h, _)| h == ALWAYS)
        .map(|(_, links)| links.clone())
        .unwrap_or_else(|| panic!("{route} has no `### {ALWAYS}` list"));
    let own = words(&read(route));
    lists
        .iter()
        .filter(|(h, _)| h.starts_with(OCCASION))
        .map(|(heading, links)| {
            let mut lines: BTreeMap<String, BTreeSet<usize>> = BTreeMap::new();
            for (file, fragment) in always.iter().chain(links) {
                lines
                    .entry(file.clone())
                    .or_default()
                    .extend(section_lines(file, fragment.as_deref()));
            }
            let mut parts = vec![(route.to_string(), own)];
            for (file, charged) in &lines {
                let text = read(file);
                let w = text
                    .lines()
                    .enumerate()
                    .filter(|(i, _)| charged.contains(i))
                    .map(|(_, l)| words(l))
                    .sum();
                parts.push((file.clone(), w));
            }
            let total = parts.iter().map(|(_, w)| w).sum();
            (heading.clone(), total, parts)
        })
        .collect()
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
    let rows = step_one_rows();
    assert_eq!(
        rows.iter()
            .map(|(file, _)| file.clone())
            .collect::<Vec<_>>(),
        step_one,
        "this file's reading of step 1 and `step_one_read_list`'s disagree on which files it \
         lists; one of the two parses broke"
    );
    ENTRY
        .iter()
        .chain(std::iter::once(&BOOTSTRAP_SKILL))
        .map(|s| {
            let w = words(&read(s));
            (s.to_string(), w)
        })
        .chain(rows.into_iter().map(|(file, charged)| {
            let w = read(&file)
                .lines()
                .enumerate()
                .filter(|(i, _)| charged.contains(i))
                .map(|(_, l)| words(l))
                .sum();
            (file, w)
        }))
        .collect()
}

/// Each row of step 1's read list, as the file it names and the 0-based lines it charges.
///
/// A row with no section links charges the whole file. One that says **Skip** charges the file
/// less each linked section, and one that says **Read only** charges the linked sections alone;
/// a section runs to the next heading at its level or higher, as [`section_lines`] has it.
/// The links are what an agent is handed (#967 measured that it reads a linked section and not
/// the file), and [`every_prelude_fragment_resolves_to_a_heading`] holds each to a heading. A row
/// that links a section with neither word, or links a file other than its own, fails here
/// rather than being charged whole: it would read dearer than it is and nobody would ask why.
fn step_one_rows() -> Vec<(String, BTreeSet<usize>)> {
    let skill = read(BOOTSTRAP_SKILL);
    let (_, after) = skill
        .split_once("### 1. Internalize the prelude")
        .expect("step 1's heading");
    let body = after.split("\n### ").next().unwrap_or(after);
    let mut rows: Vec<(String, String)> = Vec::new();
    for line in body.lines() {
        let row = line
            .split_once(". `")
            .filter(|(n, _)| n.parse::<usize>().is_ok());
        if let Some((_, rest)) = row {
            let file = rest.split('`').next().unwrap_or_default().to_string();
            rows.push((file, format!("{line}\n")));
        } else if line.starts_with(' ') && !line.trim().is_empty() {
            if let Some((_, text)) = rows.last_mut() {
                text.push_str(line);
                text.push('\n');
            }
        } else if !rows.is_empty() {
            break;
        }
    }
    let installed = installed_prelude();
    rows.into_iter()
        .map(|(file, text)| {
            let total = read(&file).lines().count();
            let sections: BTreeSet<usize> = fragment_links(&text)
                .into_iter()
                .flat_map(|(_, target, fragment)| {
                    let landed = prelude_target(BOOTSTRAP_SKILL, &target, &installed);
                    assert_eq!(
                        landed.as_deref(),
                        Some(file.as_str()),
                        "step 1's row for {file} links `{target}#{fragment}`, which is not a \
                         section of the file the row names"
                    );
                    section_lines(&file, Some(&fragment))
                })
                .collect();
            let charged = if sections.is_empty() {
                (0..total).collect()
            } else if text.contains("Read only") {
                sections
            } else if text.contains("Skip") {
                (0..total).filter(|i| !sections.contains(i)).collect()
            } else {
                panic!(
                    "step 1's row for {file} links sections and says neither `Skip` nor `Read \
                     only`, so what it charges is a guess:\n{text}"
                )
            };
            (file, charged)
        })
        .collect()
}

/// The 0-based lines of each prelude file that some read charges, by route: the route's *On
/// every occasion* and occasion lists, and step 1 of the bootstrap, which every route shares.
/// A route's reference list is not a read, for the reason [`occasion_lists`] gives.
///
/// Per route rather than a union, because an agent reads one route: the template's `AGENTS.md`
/// in this repository and `sadhana/root/AGENTS.md` in a derived one. A section only the
/// template's route links is read by nobody in the repositories the prelude is vendored into.
fn read_lines() -> BTreeMap<String, BTreeMap<String, BTreeSet<usize>>> {
    let bootstrap = step_one_rows();
    routes()
        .into_iter()
        .map(|route| {
            let mut lines: BTreeMap<String, BTreeSet<usize>> = BTreeMap::new();
            for (file, fragment) in occasion_lists(&route).into_iter().flat_map(|(_, l)| l) {
                let charged = section_lines(&file, fragment.as_deref());
                lines.entry(file).or_default().extend(charged);
            }
            for (file, charged) in &bootstrap {
                lines.entry(file.clone()).or_default().extend(charged);
            }
            (route, lines)
        })
        .collect()
}

/// `##` sections of a prelude file, as `(line, slug)` with the line numbered from 1.
fn level_two(file: &str) -> Vec<(usize, String)> {
    headings(&read(file))
        .into_iter()
        .filter(|(_, level, _)| *level == 2)
        .map(|(line, _, slug)| (line, slug))
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

// ── fragments ─────────────────────────────────────────────────────────────────

/// Files whose `#fragment` links are graded: every tracked markdown file under the prelude,
/// and the two routes.
fn fragment_sources() -> Vec<String> {
    let mut found: Vec<String> = tracked_under(&repo_root(), "yidam/prelude/")
        .into_iter()
        .filter(|p| p.ends_with(".md"))
        .collect();
    found.extend(["AGENTS.md", "sadhana/root/AGENTS.md"].map(str::to_string));
    found
}

/// Every `[label](target#fragment)` outside fenced and inline code, as `(line, target,
/// fragment)`. An empty `target` is a link into the same file.
fn fragment_links(text: &str) -> Vec<(usize, String, String)> {
    let mut out = Vec::new();
    for (n, raw) in prose_lines(text) {
        let line = blank_code_spans(raw);
        let mut j = 0;
        while let Some(open) = line[j..].find("](") {
            let at = j + open + 2;
            let Some(close) = line[at..].find(')') else {
                break;
            };
            let t = line[at..at + close].trim();
            j = at + close + 1;
            let t = t.split_whitespace().next().unwrap_or(t);
            let Some((path, fragment)) = t.split_once('#') else {
                continue;
            };
            if fragment.is_empty() || path.contains("://") || path.starts_with('/') {
                continue;
            }
            out.push((n, path.to_string(), fragment.to_string()));
        }
    }
    out
}

/// Lines outside fenced code, numbered from 1.
fn prose_lines(text: &str) -> Vec<(usize, &str)> {
    let mut out = Vec::new();
    let mut fence: Option<String> = None;
    for (n, raw) in text.lines().enumerate() {
        let trimmed = raw.trim_start();
        if let Some(open) = &fence {
            if trimmed.starts_with(open.as_str()) {
                fence = None;
            }
            continue;
        }
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            fence = Some(trimmed.chars().take(3).collect());
            continue;
        }
        out.push((n + 1, raw));
    }
    out
}

/// The prelude file a link from `from` lands on, or `None` when it lands anywhere else.
///
/// Resolved in the tree the linking file is written for, which is not the one it sits in: a
/// prelude file installs to `.yidam/.vendor/prelude/` and `sadhana/root/AGENTS.md` to a
/// derived repository's root, so their links are resolved from there and mapped back to the
/// template file they came from. The template's own `AGENTS.md` installs nowhere and resolves
/// where it sits. Whether the *file* exists is `installed_layout_links`' question, so a link
/// to a missing one is skipped here rather than failed twice.
fn prelude_target(
    from: &str,
    target: &str,
    installed: &BTreeMap<String, String>,
) -> Option<String> {
    let dst = install_of(from).and_then(|(_, dst)| dst);
    let frame = dst.as_deref().unwrap_or(from);
    let landed = if target.is_empty() {
        frame.to_string()
    } else {
        resolve(frame.rsplit_once('/').map_or("", |(dir, _)| dir), target)?
    };
    let source = match dst {
        Some(_) => installed.get(&landed)?.clone(),
        None => landed,
    };
    (source.starts_with("yidam/prelude/") && source.ends_with(".md")).then_some(source)
}

/// Where each tracked prelude markdown file lands in a derived repository, mapped back to it.
fn installed_prelude() -> BTreeMap<String, String> {
    tracked_under(&repo_root(), "yidam/prelude/")
        .into_iter()
        .filter(|p| p.ends_with(".md"))
        .filter_map(|p| Some((install_of(&p)?.1?, p)))
        .collect()
}

/// `(level, slug)` for every ATX heading outside fenced code, in document order.
fn heading_slugs(text: &str) -> Vec<(usize, String)> {
    headings(text)
        .into_iter()
        .map(|(_, level, slug)| (level, slug))
        .collect()
}

/// `(line, level, slug)` for every ATX heading outside fenced code, in document order, with
/// lines numbered from 1.
///
/// Every level goes through one slugger, as it does in Astro's `rehype-heading-ids`, so a
/// second `## Dup` is `dup-1` and a `### Dup` after it is `dup-2`.
fn headings(text: &str) -> Vec<(usize, usize, String)> {
    let mut seen: BTreeMap<String, usize> = BTreeMap::new();
    let mut out = Vec::new();
    for (n, line) in prose_lines(text) {
        let level = line.chars().take_while(|&c| c == '#').count();
        let Some(rest) = line[level..].strip_prefix(' ') else {
            continue;
        };
        if !(1..=6).contains(&level) {
            continue;
        }
        let base = slug(&heading_text(rest));
        let mut result = base.clone();
        while seen.contains_key(&result) {
            let n = seen
                .get_mut(&base)
                .expect("the base slug is recorded before any suffix");
            *n += 1;
            result = format!("{base}-{n}");
        }
        seen.insert(result.clone(), 0);
        // Astro, after the slugger: one trailing `-` is dropped.
        if result.ends_with('-') {
            result.pop();
        }
        out.push((n, level, result));
    }
    out
}

/// A heading's rendered text: code spans kept literally, `*` emphasis, escapes, link targets
/// and a closing `#` sequence removed.
///
/// Not a markdown renderer, and not trying to be one. It covers the inline syntax the in-scope
/// headings use. `_` emphasis is left alone because `_` survives slugging, and no heading here
/// uses it; one that does fails [`the_slugger_agrees_with_the_docs_render`] once it is added
/// there, and should be.
fn heading_text(raw: &str) -> String {
    let raw = raw.trim_end();
    let raw = match raw.trim_end_matches('#') {
        t if t.len() < raw.len() && (t.is_empty() || t.ends_with(' ')) => t.trim_end(),
        _ => raw,
    };
    let mut out = String::new();
    let mut in_code = false;
    let mut chars = raw.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '`' => in_code = !in_code,
            _ if in_code => out.push(c),
            '*' => {}
            '\\' if chars.peek().is_some_and(char::is_ascii_punctuation) => {
                out.extend(chars.next());
            }
            ']' if chars.peek() == Some(&'(') => {
                for skipped in chars.by_ref() {
                    if skipped == ')' {
                        break;
                    }
                }
            }
            _ => out.push(c),
        }
    }
    out
}

/// `github-slugger`'s rule: lowercase, drop everything but letters, digits, `_`, `-` and
/// space, then each space becomes `-`.
fn slug(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .filter(|&c| c.is_alphanumeric() || matches!(c, '_' | '-' | ' '))
        .map(|c| if c == ' ' { '-' } else { c })
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
fn every_prelude_fragment_resolves_to_a_heading() {
    let installed = installed_prelude();
    let mut slugs: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut graded: BTreeSet<String> = BTreeSet::new();
    let mut dead = Vec::new();
    for from in fragment_sources() {
        for (line, target, fragment) in fragment_links(&read(&from)) {
            let Some(to) = prelude_target(&from, &target, &installed) else {
                continue;
            };
            graded.insert(from.clone());
            let known = slugs.entry(to.clone()).or_insert_with(|| {
                heading_slugs(&read(&to))
                    .into_iter()
                    .filter(|(level, _)| matches!(level, 2 | 3))
                    .map(|(_, slug)| slug)
                    .collect()
            });
            if !known.contains(&fragment) {
                dead.push(format!("  {from}:{line} → {to}#{fragment}"));
            }
        }
    }
    // A lower bound on discovery, as for the routes: one file linking across files and one
    // linking within itself. A scan that stopped seeing either would pass on nothing.
    for known in [BOOTSTRAP_SKILL, "yidam/prelude/GRAPH.evidence.md"] {
        assert!(
            graded.contains(known),
            "the fragment scan graded no link from {known}. Graded: {graded:?}. The scan is \
             broken, or that file stopped linking into the prelude by fragment."
        );
    }
    assert!(
        dead.is_empty(),
        "{} link(s) name a heading the target has no `##` or `###` for. A reader following one \
         lands at the top of the right file and is told nothing. Slugs are the docs site's: \
         lowercase, punctuation dropped, spaces to `-`.\n{}",
        dead.len(),
        dead.join("\n")
    );
}

#[test]
fn the_slugger_agrees_with_the_docs_render() {
    // Each `(level, slug)` on the right was read off Astro's `createMarkdownProcessor` render of
    // the heading on the left, in this order, in one document — the duplicates depend on it.
    let rendered: &[(&str, usize, &str)] = &[
        ("# Title", 1, "title"),
        ("## `.yidam/corpus/`", 2, "yidamcorpus"),
        (
            "## Why a *ceiling* — not a floor",
            2,
            "why-a-ceiling--not-a-floor",
        ),
        ("### The [class](x.md) contract", 3, "the-class-contract"),
        ("## Dup", 2, "dup"),
        ("## Dup", 2, "dup-1"),
        ("### Dup", 3, "dup-2"),
        ("## ends with ?", 2, "ends-with"),
        (
            "### `$YIDAM_GRAPH` — the corpus already parsed, and already resolved",
            3,
            "yidam_graph--the-corpus-already-parsed-and-already-resolved",
        ),
        (
            "### 1.5. Explore the existing repository — existing-repo mode only",
            3,
            "15-explore-the-existing-repository--existing-repo-mode-only",
        ),
        (
            "## `sadhana/` (transient — present only during bootstrap)",
            2,
            "sadhana-transient--present-only-during-bootstrap",
        ),
        (
            "### `[verified]` is a claim about provenance, not about confidence",
            3,
            "verified-is-a-claim-about-provenance-not-about-confidence",
        ),
        (
            "## The diagnostic_severity fixtures",
            2,
            "the-diagnostic_severity-fixtures",
        ),
        ("## Mañjuśrī and \\*escaped\\*", 2, "mañjuśrī-and-escaped"),
        ("## Closing hashes ##", 2, "closing-hashes"),
    ];
    let doc: Vec<&str> = rendered.iter().map(|(h, _, _)| *h).collect();
    let ours = heading_slugs(&doc.join("\n\n"));
    assert_eq!(
        ours.len(),
        rendered.len(),
        "a heading was not recognised: {ours:?}"
    );
    for ((heading, level, want), (got_level, got)) in rendered.iter().zip(&ours) {
        assert_eq!(
            (got_level, got.as_str()),
            (level, *want),
            "`{heading}` slugs differently here than on the docs site"
        );
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

/// Both directions, per occasion: an occasion a route names with no ceiling, and a ceiling
/// naming an occasion the route no longer has. The second is how a renamed heading would
/// otherwise leave its ceiling guarding nothing.
#[test]
fn every_route_has_a_ceiling() {
    let ceilings: BTreeSet<(&str, &str)> = READ_CEILING.iter().map(|(r, o, _)| (*r, *o)).collect();
    let mut named: BTreeSet<(String, String)> = BTreeSet::new();
    for route in routes() {
        let occasions = occasion_costs(&route);
        assert!(
            !occasions.is_empty(),
            "{route} prescribes the conduct read and names no `### {OCCASION}…` occasion. A \
             route is a set of occasions (RFC-0039), and one with none is measured by nothing."
        );
        for (occasion, _, _) in occasions {
            assert!(
                ceilings.contains(&(route.as_str(), occasion.as_str())),
                "{route} names the occasion `{occasion}` and READ_CEILING has no entry for it. \
                 A new occasion is a cost somebody should have to write down."
            );
            named.insert((route.clone(), occasion));
        }
    }
    for (route, occasion, _) in READ_CEILING {
        assert!(
            named.contains(&(route.to_string(), occasion.to_string())),
            "READ_CEILING holds `{occasion}` for {route}, and {route} names no such occasion. \
             The heading was renamed or removed, and this ceiling is guarding nothing."
        );
    }
}

#[test]
fn the_recurring_read_stays_under_its_ceiling() {
    let mut over = Vec::new();
    for route in routes() {
        for (occasion, total, parts) in occasion_costs(&route) {
            let Some((_, _, ceiling)) = READ_CEILING
                .iter()
                .find(|(r, o, _)| *r == route && *o == occasion)
            else {
                continue; // every_route_has_a_ceiling's failure, not this one's
            };
            if total > *ceiling {
                over.push(format!(
                    "{route} `{occasion}` is {total} words, over its {ceiling}-word ceiling.\n{}",
                    parts
                        .iter()
                        .map(|(p, w)| format!("  {w:>7}  {p}\n"))
                        .collect::<String>()
                ));
            }
        }
    }
    assert!(over.is_empty(), "{}", over.join("\n"));
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

/// Three directions, each per section. A `##` section of a step-1 file that a route's reads do
/// not reach and [`REFERENCE_ONLY`] does not list. An entry naming a section that no longer
/// exists. An entry naming a section every route now reads. The second and third keep the list
/// from rotting into names that mean nothing (#660: everything asked whether what is emitted is
/// declared, and nothing asked the other way round).
///
/// A section is read when its heading line is charged: a link to it, to the whole file, or a
/// step 1 row that does not skip it. A link to one of its `###` subsections does not read it,
/// because the lines between its heading and that subsection are then read by nobody.
#[test]
fn every_prelude_section_is_read_or_reference_only() {
    const PRELUDE: &str = "yidam/prelude/";
    let by_route = read_lines();
    let listed: BTreeMap<&str, &str> = REFERENCE_ONLY.iter().copied().collect();
    let mut found: BTreeSet<String> = BTreeSet::new();
    let mut unread = Vec::new();
    let mut stale = Vec::new();
    for file in step_one_read_list() {
        let short = file.strip_prefix(PRELUDE).unwrap_or(&file);
        for (line, slug) in level_two(&file) {
            let name = format!("{short}#{slug}");
            let missed: Vec<&str> = by_route
                .iter()
                .filter(|(_, lines)| !lines.get(&file).is_some_and(|l| l.contains(&(line - 1))))
                .map(|(route, _)| route.as_str())
                .collect();
            match (missed.is_empty(), listed.contains_key(name.as_str())) {
                (false, false) => {
                    unread.push(format!(
                        "  {file}:{line} → {name}, on {}",
                        missed.join(", ")
                    ));
                }
                (true, true) => stale.push(format!("  {name}")),
                _ => {}
            }
            found.insert(name);
        }
    }
    let gone: Vec<String> = listed
        .keys()
        .filter(|name| !found.contains(**name))
        .map(|name| format!("  {name}"))
        .collect();
    let mut failures = Vec::new();
    if !unread.is_empty() {
        failures.push(format!(
            "{} section(s) are read on no occasion of a route and by no bootstrap, and \
             REFERENCE_ONLY does not list them. Link each from the occasion that needs it, or \
             list it with the occasion that would read it if one existed:\n{}",
            unread.len(),
            unread.join("\n")
        ));
    }
    if !gone.is_empty() {
        failures.push(format!(
            "{} REFERENCE_ONLY entr(ies) name a section none of the seven step-1 files has. It \
             was renamed or removed, and the entry now excuses nothing:\n{}",
            gone.len(),
            gone.join("\n")
        ));
    }
    if !stale.is_empty() {
        failures.push(format!(
            "{} REFERENCE_ONLY entr(ies) name a section every route now reads. Remove the \
             entry; the occasion its reason named exists now:\n{}",
            stale.len(),
            stale.join("\n")
        ));
    }
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}
