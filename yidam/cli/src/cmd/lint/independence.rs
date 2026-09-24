//! What a resolution's `independence:` states, against what its seats derive.
//!
//! `PROTOCOL.md` puts `independence:` on the resolution record with a closed vocabulary of
//! three — `distinct-seats`, `shared-configuration`, `unrecorded` — and says in as many words
//! that **nothing computes it**. That is the hedge this module closes (#823). The value is a
//! function of things already written down: the record names its `tips:`, each tip names a
//! seat, and `electors.md` carries `Kind`, `Model`, `Version` and `Config`. A stated value
//! that disagrees with the derived one was, until this check, a defect nothing could see.
//!
//! # Which registry — the one question that decided the shape
//!
//! A seat's row is mutable. `PROTOCOL.md` says *"A material change is recorded as an update,
//! so the ancestry of a position includes the state of the agent that held it; a model upgrade
//! is material."* So the registry at HEAD and the registry at a resolution's tips disagree for
//! any record old enough to matter, and the two readings are not the same check.
//!
//! **Decided: at the tips, and each seat at its own tip.** Reading HEAD would re-judge history
//! every time a seat is updated — a resolution correct the day it was written goes red on
//! somebody else's model bump, which is a check that reports the reader's clock rather than
//! the record. Reading each seat's row out of the blob at that seat's own `ma/<elector>@<hash>`
//! is what *"the state of the agent that held it"* means literally, and it is the only reading
//! under which a settled record's derived value never changes again.
//!
//! **A tip that cannot be read yields `unrecorded`, and there is no second reading path.** A
//! shallow checkout or a single-branch clone does not have the `ma/*` commits — [`super::scope`]
//! measured that exactly, and 15 of 70 tips in the one corpus that has run this protocol
//! resolve only through the `ma/*` branches themselves. Falling back to HEAD there would make
//! the derived value depend on how the repository was fetched, with nothing in the answer
//! saying which registry produced it. `unrecorded` is already the word for *the registry does
//! not say*, the amendment already forbids reading it as either other value, and an unreadable
//! tip is exactly that state. So the thin clone gets the honest answer rather than a different
//! answer, and [`independence_mismatch`]'s detail says the clone is why.
//!
//! # Where `unrecorded` comes from, and what beats it
//!
//! `PROTOCOL.md` defines the three arms, and the order they are tested in follows from its own
//! wording rather than from taste:
//!
//! - `shared-configuration` — *"two or more participating agent seats carry the same `Model`,
//!   `Version` and `Config`."* Nothing in that sentence is conditional on the other seats, so a
//!   demonstrated collision decides even when some third seat is blank.
//! - `unrecorded` — *"a participating agent seat leaves `Model`, `Version` or `Config` blank,
//!   **so the question cannot be answered from the registry at all**."* That trailing clause is
//!   the reason the arm exists, and it is false once a collision has been demonstrated. So a
//!   gap decides only when nothing was demonstrated.
//! - `distinct-seats` — every participating seat described, and no two agent seats alike.
//!
//! `electors.md` states the blank rule from the other side — *"A blank column yields
//! `unrecorded`, which is neither an accusation nor a clearance"* — and that is why a blank is
//! never read as a collision. Two seats that are both blank are two seats the registry does not
//! describe, not two seats it says are the same.
//!
//! **A human seat is never a gap and never a collision.** `PROTOCOL.md`: *"Human seats differ
//! by being different people."* The whole argument is about agent seats — *"a failure that
//! cannot occur in a sangha of humans"* — and `electors.md` spells a human's agent columns `—`
//! on purpose. Reading that blank as a gap would report every mixed sangha as `unrecorded`
//! forever. A seat whose `Kind` is blank is *not* given that exemption: an empty field means
//! the registry does not say, which is different from there being nothing to say.
//!
//! # Why this reports rather than gates
//!
//! **Info, carrying the derived answer** — the arrangement [`super::lineage::baseline_undeclared`]
//! already uses. Every resolution written before the amendment carries no value at all, so a
//! gate would be red on the entire corpus the day it landed, which is how a gate gets switched
//! off rather than answered. And in the one corpus that has run this protocol the registry
//! binds none of the four columns, so every record there derives `unrecorded` — the check is
//! silent on all 29 and arms itself as registries fill in.
//!
//! That silence is deliberate in one specific place: **a record that states nothing, whose
//! seats derive `unrecorded`, is not reported.** There is nothing to write down. Asking 29
//! authors to add `independence: unrecorded` would be asking them to record *we do not know*,
//! which is already what absence means.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use super::history::read_blobs;
use super::model::{Check, Severity, Violation};
use crate::cmd::sangha::{parse_electors, ElectorRow, Resolution};

/// The registry, at whatever revision it is being read from.
const REGISTRY: &str = ".yidam/sangha/electors.md";

/// The closed vocabulary `PROTOCOL.md` draws `independence:` from.
///
/// Three arms and no fourth, including no *not applicable*: the amendment's whole point is
/// that **a consumer MUST NOT read `unrecorded` as `distinct-seats`, and MUST NOT read it as
/// `shared-configuration` either**, so a fourth arm meaning *we did not look* would be the
/// same confusion under a new name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Independence {
    DistinctSeats,
    SharedConfiguration,
    Unrecorded,
}

impl Independence {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::DistinctSeats => "distinct-seats",
            Self::SharedConfiguration => "shared-configuration",
            Self::Unrecorded => "unrecorded",
        }
    }

    /// The vocabulary member a record stated, or `None` for a value outside it.
    ///
    /// Exact, and deliberately not generous: `distinct_seats` and `Distinct-Seats` are not
    /// this vocabulary, and silently accepting them would mean the file and the check disagree
    /// about what the closed set is. A record spelling it wrong is told so.
    fn parse(stated: &str) -> Option<Self> {
        match stated {
            "distinct-seats" => Some(Self::DistinctSeats),
            "shared-configuration" => Some(Self::SharedConfiguration),
            "unrecorded" => Some(Self::Unrecorded),
            _ => None,
        }
    }
}

/// One participating seat, as the registry described it at that seat's own tip.
///
/// Taken as data rather than read off disk — [`super::attest`]'s discipline, and the reason
/// every judgement below is testable without a repository. [`audit`] is the only part that
/// needs git.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SeatAtTip {
    /// The `ma/<elector>` branch this tip names, without its hash.
    pub branch: String,
    /// The tip verbatim — `ma/<elector>@<hash>` — for a message that quotes the record.
    pub tip: String,
    /// The seat's registry row, read at this tip.
    ///
    /// `None` where the registry could not be read at that revision or carries no row for the
    /// branch: a commit the clone does not have, a tip naming no hash, or a seat registered
    /// after the resolution it took part in. All three are *the registry does not say*, which
    /// is one answer, and [`Ground`] is what tells them apart in the message.
    pub row: Option<ElectorRow>,
}

/// One resolution's stated value, and the seats it read.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct IndependenceAudit {
    /// Repo-relative path of the resolution record.
    pub file: String,
    /// `independence:` verbatim; empty where the record carries none.
    pub stated: String,
    /// The seats the record's `tips:` name, deduplicated, in the order the record names them.
    pub seats: Vec<SeatAtTip>,
}

/// Why a derivation came out the way it did, in the words a finding quotes.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Derivation {
    pub value: Independence,
    /// A clause completing *"at the tips it names, …"*.
    pub because: String,
}

/// Whether a registry row describes an agent seat well enough to compare.
///
/// A seat is read as an agent unless its `Kind` says `human`, which is the conservative
/// direction: a registry that does not say what a seat is has not established that its blank
/// model columns are the expected kind of blank.
fn is_human(row: &ElectorRow) -> bool {
    row.kind.eq_ignore_ascii_case("human")
}

/// The columns `PROTOCOL.md` names, in its order, that this row leaves blank.
fn blank_columns(row: &ElectorRow) -> Vec<&'static str> {
    [
        ("Model", &row.model),
        ("Version", &row.version),
        ("Config", &row.config),
    ]
    .into_iter()
    .filter(|(_, v)| v.trim().is_empty())
    .map(|(name, _)| name)
    .collect()
}

/// `["a", "b", "c"]` → ``"`a`, `b` and `c`"``.
fn listed(items: &[String]) -> String {
    let quoted: Vec<String> = items.iter().map(|i| format!("`{i}`")).collect();
    match quoted.split_last() {
        None => String::new(),
        Some((last, [])) => last.clone(),
        Some((last, rest)) => format!("{} and {last}", rest.join(", ")),
    }
}

/// Derive `independence:` from the seats a resolution read.
///
/// Pure, and the only place the vocabulary is decided. The order of the arms is
/// `PROTOCOL.md`'s and is argued in the module docs: a demonstrated collision beats a gap,
/// because *"so the question cannot be answered from the registry at all"* stops being true
/// the moment one pair has been shown to be a pair.
pub(crate) fn derive(seats: &[SeatAtTip]) -> Derivation {
    if seats.is_empty() {
        return Derivation {
            value: Independence::Unrecorded,
            because: "the record names no `tips:`, so it read no seat the registry could \
                      describe"
                .to_string(),
        };
    }

    // Agent seats the registry describes fully, grouped by the triple that distinguishes
    // them. `BTreeMap` so a collision is reported the same way on every run — a message that
    // depends on hash order is a golden that fails at random.
    let mut described: BTreeMap<(&str, &str, &str), Vec<String>> = BTreeMap::new();
    let mut humans: Vec<String> = Vec::new();
    let mut blank: Vec<String> = Vec::new();
    let mut unread: Vec<String> = Vec::new();

    for seat in seats {
        let Some(row) = &seat.row else {
            unread.push(seat.tip.clone());
            continue;
        };
        if is_human(row) {
            humans.push(seat.branch.clone());
            continue;
        }
        let missing = blank_columns(row);
        if !missing.is_empty() {
            blank.push(format!(
                "`{}` leaves {} blank",
                seat.branch,
                listed(&missing.iter().map(|m| (*m).to_string()).collect::<Vec<_>>())
            ));
            continue;
        }
        described
            .entry((&row.model, &row.version, &row.config))
            .or_default()
            .push(seat.branch.clone());
    }

    // 1 — a collision is a fact the registry recorded, so it decides.
    if let Some(((model, version, config), branches)) =
        described.iter().find(|(_, branches)| branches.len() > 1)
    {
        // All three are non-empty by construction: a row with any of them blank went to
        // `blank` above and never reached this map.
        return Derivation {
            value: Independence::SharedConfiguration,
            because: format!(
                "{} carry the same `Model`, `Version` and `Config` — `{model}` `{version}` at \
                 config `{config}`",
                listed(branches)
            ),
        };
    }

    // 2 — otherwise a gap decides, and the two kinds of gap say which they are.
    if !unread.is_empty() || !blank.is_empty() {
        let mut why: Vec<String> = Vec::new();
        if !unread.is_empty() {
            why.push(format!(
                "the registry could not be read at {}, which {} no commit in this clone",
                listed(&unread),
                if unread.len() == 1 { "names" } else { "name" }
            ));
        }
        why.extend(blank.iter().cloned());
        return Derivation {
            value: Independence::Unrecorded,
            because: why.join("; "),
        };
    }

    // 3 — every seat described and no two alike. Vacuously true of a single-seat record,
    // which is the honest answer: there is nothing for that seat to be indistinguishable from.
    let described_branches: Vec<String> = described.values().flatten().cloned().collect();
    let described_count = described_branches.len();
    let because = match (humans.len(), described_count) {
        (0, 1) => format!(
            "it names one seat, {}, and the registry describes it — nothing to collide with",
            listed(&described_branches)
        ),
        (h, 0) => format!(
            "{} {} human {}, and human seats differ by being different people",
            listed(&humans),
            if h == 1 { "is a" } else { "are" },
            if h == 1 { "seat" } else { "seats" }
        ),
        (0, n) => format!(
            "all {n} agent seats — {} — carry a different `Model`, `Version` or `Config`",
            listed(&described_branches)
        ),
        (h, n) => format!(
            "{h} human seat(s) differ by being different people, and the {n} agent seats — {} \
             — carry a different `Model`, `Version` or `Config`",
            listed(&described_branches)
        ),
    };
    Derivation {
        value: Independence::DistinctSeats,
        because,
    }
}

// ── the check ─────────────────────────────────────────────────────────────────

/// Why a resolution's `independence:` is being reported, in the words the finding opens with.
///
/// Three reasons, and they are not one finding said three ways: a record stating nothing and a
/// record stating something false are different acts, and a reader told only *"does not
/// match"* has to re-derive the value to know which one they have. The fourth population —
/// nothing stated over a derivation of `unrecorded` — is silent, and has no arm here.
enum Ground {
    /// The record carries no `independence:`, and the seats derive something to write down.
    Unstated(Independence),
    /// The record states a value the closed vocabulary does not contain.
    OutsideVocabulary(Independence),
    /// The record states a vocabulary member, and it is not the one the seats derive.
    Disagrees(Independence, Independence),
}

/// A resolution whose `independence:` is not the one its seats derive.
///
/// The severity and the silences are argued in the module docs. Four populations reach this
/// check and one deliberately does not: a record stating nothing whose seats derive
/// `unrecorded` has nothing to record and is passed over.
pub(crate) fn independence_mismatch(audits: &[IndependenceAudit]) -> Check {
    let mut violations = Vec::new();
    for a in audits {
        let derived = derive(&a.seats);
        let stated = a.stated.trim();
        let ground = if stated.is_empty() {
            // Nothing stated and nothing derivable is not a finding — see the module docs.
            if derived.value == Independence::Unrecorded {
                continue;
            }
            Ground::Unstated(derived.value)
        } else {
            match Independence::parse(stated) {
                None => Ground::OutsideVocabulary(derived.value),
                Some(v) if v == derived.value => continue,
                Some(v) => Ground::Disagrees(v, derived.value),
            }
        };
        let opening = match ground {
            Ground::Unstated(d) => {
                format!("no `independence:` — its seats derive `{}`", d.as_str())
            }
            Ground::OutsideVocabulary(d) => format!(
                "states `{stated}`, which is not one of `distinct-seats`, \
                 `shared-configuration` or `unrecorded`; its seats derive `{}`",
                d.as_str()
            ),
            Ground::Disagrees(s, d) => {
                format!("states `{}`; its seats derive `{}`", s.as_str(), d.as_str())
            }
        };
        violations.push(Violation::new(
            a.file.clone(),
            format!("{opening} — at the tips it names, {}", derived.because),
        ));
    }
    Check::new(
        "resolution-independence-mismatch",
        "Resolution's stated independence is not the one its seats derive",
        Severity::Info,
        "`independence:` states what `electors.md` distinguishes among the seats a resolution \
         read, and it is derivable rather than judged: the record names its tips, each tip \
         names a seat, and the registry carries `Kind`, `Model`, `Version` and `Config`. That \
         makes a stated value that disagrees with the registry the one part of the record \
         which is checkable and was, until this check, invisible. It reports rather than \
         gates because every record written before the field existed carries no value at all, \
         so a gate would be red on the whole corpus the day it armed — and it carries the \
         derived answer rather than only the complaint, so the finding is also the repair. \
         The registry is read at each seat's own tip, never at HEAD: a seat's row is mutable \
         and a model upgrade is material, so reading HEAD would turn somebody else's upgrade \
         into a finding against a record that was correct when it was written.",
        violations,
    )
}

// ── reading the repository ────────────────────────────────────────────────────

/// `ma/auditor@0b12292` → `("ma/auditor", Some("0b12292"))`.
///
/// A tip with no `@` names a branch and no revision. That is a malformed record and not this
/// check's business; here it is simply a seat whose registry row cannot be located, which is
/// `unrecorded` — the same answer a tip naming a commit the clone lacks gets, and for the same
/// reason.
fn split_tip(tip: &str) -> (&str, Option<&str>) {
    match tip.split_once('@') {
        Some((branch, hash)) if !hash.is_empty() => (branch, Some(hash)),
        _ => (tip, None),
    }
}

/// Each resolution's stated value and the registry rows at the tips it names.
///
/// One `git cat-file --batch` for every revision any record names, which is the same reason
/// [`read_blobs`] exists: a subprocess per tip is the difference between milliseconds and
/// seconds on a corpus with 70 of them. Each blob is parsed once and shared, because several
/// records naming one tip is the common case, not the exception.
pub(crate) fn audit(root: &Path, records: &[Resolution]) -> Vec<IndependenceAudit> {
    // Every distinct `<hash>:.yidam/sangha/electors.md` any record asks for.
    let revspecs: BTreeSet<String> = records
        .iter()
        .flat_map(|r| r.tips.iter())
        .filter_map(|t| split_tip(t).1)
        .map(|hash| format!("{hash}:{REGISTRY}"))
        .collect();
    let revspecs: Vec<String> = revspecs.into_iter().collect();
    let blobs = read_blobs(root, &revspecs);

    // Parsed once per revision, not once per seat.
    let registries: BTreeMap<String, Vec<ElectorRow>> = blobs
        .iter()
        .map(|(spec, text)| (spec.clone(), parse_electors(text)))
        .collect();

    records
        .iter()
        .map(|r| {
            let mut seen: BTreeSet<&str> = BTreeSet::new();
            let mut seats = Vec::new();
            for tip in &r.tips {
                let (branch, hash) = split_tip(tip);
                // The same seat named twice is one seat. Its first tip wins, which is the
                // only choice that does not make the answer depend on `tips:` order.
                if branch.is_empty() || !seen.insert(branch) {
                    continue;
                }
                let row = hash
                    .and_then(|h| registries.get(&format!("{h}:{REGISTRY}")))
                    .and_then(|rows| rows.iter().find(|row| row.branch == branch))
                    .cloned();
                seats.push(SeatAtTip {
                    branch: branch.to_string(),
                    tip: tip.clone(),
                    row,
                });
            }
            IndependenceAudit {
                file: r.file.clone(),
                stated: r.independence.clone(),
                seats,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn agent(branch: &str, model: &str, version: &str, config: &str) -> SeatAtTip {
        SeatAtTip {
            branch: branch.to_string(),
            tip: format!("{branch}@aaaaaaa"),
            row: Some(ElectorRow {
                name: branch.trim_start_matches("ma/").to_string(),
                branch: branch.to_string(),
                role: "A seat.".to_string(),
                kind: "agent".to_string(),
                model: model.to_string(),
                version: version.to_string(),
                config: config.to_string(),
                key: String::new(),
            }),
        }
    }

    fn human(branch: &str) -> SeatAtTip {
        let mut s = agent(branch, "", "", "");
        if let Some(row) = s.row.as_mut() {
            row.kind = "human".to_string();
        }
        s
    }

    /// A tip whose commit the clone does not have, or a seat the registry does not carry.
    fn unread(branch: &str) -> SeatAtTip {
        SeatAtTip {
            branch: branch.to_string(),
            tip: format!("{branch}@ddddddd"),
            row: None,
        }
    }

    fn record(file: &str, stated: &str, seats: Vec<SeatAtTip>) -> IndependenceAudit {
        IndependenceAudit {
            file: file.to_string(),
            stated: stated.to_string(),
            seats,
        }
    }

    // ── the derivation reaches every arm, from a real cause ───────────────────
    //
    // #773's lesson, and the issue restates it: a vocabulary arm proved only by a hand-built
    // value proves the type and not the derivation. Every arm below is reached by describing
    // seats, never by naming the arm.

    #[test]
    fn two_agent_seats_the_registry_tells_apart_derive_distinct_seats() {
        let d = derive(&[
            agent("ma/auditor", "claude-opus-4-8", "1", "c0ffee"),
            agent("ma/advocate", "claude-opus-4-8", "1", "decafbad"),
        ]);
        assert_eq!(d.value, Independence::DistinctSeats, "{d:?}");
        assert!(d.because.contains("ma/advocate"), "{d:?}");
    }

    #[test]
    fn two_agent_seats_sharing_model_version_and_config_derive_shared_configuration() {
        let d = derive(&[
            agent("ma/auditor", "claude-opus-4-8", "1", "c0ffee"),
            agent("ma/advocate", "claude-opus-4-8", "1", "c0ffee"),
        ]);
        assert_eq!(d.value, Independence::SharedConfiguration, "{d:?}");
        assert!(
            d.because.contains("ma/advocate") && d.because.contains("ma/auditor"),
            "the finding must name the pair: {d:?}"
        );
    }

    /// The issue's own example of the `unrecorded` cause: one blank column, nothing else
    /// wrong. It must not read as either other value — the amendment says so in a MUST NOT.
    #[test]
    fn a_seat_with_a_blank_config_derives_unrecorded_and_neither_other_value() {
        let d = derive(&[
            agent("ma/auditor", "claude-opus-4-8", "1", "c0ffee"),
            agent("ma/advocate", "claude-opus-4-8", "1", ""),
        ]);
        assert_eq!(d.value, Independence::Unrecorded, "{d:?}");
        assert_ne!(d.value, Independence::DistinctSeats);
        assert_ne!(d.value, Independence::SharedConfiguration);
        assert!(d.because.contains("`Config`"), "{d:?}");
    }

    /// Two blank seats are two seats the registry does not describe — not two seats it says
    /// are the same. `electors.md`: *"neither an accusation nor a clearance."*
    #[test]
    fn two_equally_blank_seats_are_unrecorded_rather_than_a_collision() {
        let d = derive(&[
            agent("ma/auditor", "", "", ""),
            agent("ma/advocate", "", "", ""),
        ]);
        assert_eq!(d.value, Independence::Unrecorded, "{d:?}");
    }

    /// The ordering argued in the module docs, and the one place the two gap arms compete.
    /// `PROTOCOL.md` conditions `unrecorded` on the question being unanswerable, and a
    /// demonstrated pair answers it.
    #[test]
    fn a_demonstrated_collision_beats_a_blank_column_elsewhere() {
        let d = derive(&[
            agent("ma/auditor", "claude-opus-4-8", "1", "c0ffee"),
            agent("ma/advocate", "claude-opus-4-8", "1", "c0ffee"),
            agent("ma/archivist", "claude-opus-4-8", "1", ""),
        ]);
        assert_eq!(d.value, Independence::SharedConfiguration, "{d:?}");
    }

    /// A human's agent columns are `—` by `electors.md`'s own template. Reading that as a gap
    /// would report every mixed sangha as `unrecorded` forever.
    #[test]
    fn human_seats_are_distinct_without_model_columns() {
        let d = derive(&[human("ma/goedelsoup"), human("ma/reviewer")]);
        assert_eq!(d.value, Independence::DistinctSeats, "{d:?}");

        let mixed = derive(&[
            human("ma/goedelsoup"),
            agent("ma/auditor", "claude-opus-4-8", "1", "c0ffee"),
        ]);
        assert_eq!(mixed.value, Independence::DistinctSeats, "{mixed:?}");
    }

    /// A blank `Kind` does not buy the human exemption. An empty column means the registry
    /// does not say, which is not the same fact as *there is nothing to say* — and every
    /// registry written before RFC-0012's columns existed is in that state.
    #[test]
    fn a_seat_that_does_not_say_what_it_is_gets_no_exemption() {
        let mut a = agent("ma/auditor", "", "", "");
        if let Some(row) = a.row.as_mut() {
            row.kind = String::new();
        }
        let d = derive(&[a, human("ma/goedelsoup")]);
        assert_eq!(d.value, Independence::Unrecorded, "{d:?}");
    }

    /// Q1's answer, and the reason there is no HEAD fallback: a tip the clone does not carry
    /// is the registry declining to say, which already has a word.
    #[test]
    fn a_tip_the_clone_cannot_read_derives_unrecorded() {
        let d = derive(&[
            agent("ma/auditor", "claude-opus-4-8", "1", "c0ffee"),
            unread("ma/advocate"),
        ]);
        assert_eq!(d.value, Independence::Unrecorded, "{d:?}");
        assert!(d.because.contains("no commit in this clone"), "{d:?}");
    }

    /// A record naming no tips read nothing, and reporting that as `distinct-seats` on the
    /// vacuous reading would blame the synthesis for a malformed record — [`super::scope`]
    /// declines the same shape for the same reason.
    #[test]
    fn a_record_naming_no_tips_derives_unrecorded_rather_than_vacuous_distinctness() {
        let d = derive(&[]);
        assert_eq!(d.value, Independence::Unrecorded, "{d:?}");
    }

    /// One seat has nothing to be indistinguishable from, and saying so is not the same as
    /// declining to answer.
    #[test]
    fn a_single_described_seat_is_vacuously_distinct() {
        let d = derive(&[agent("ma/auditor", "claude-opus-4-8", "1", "c0ffee")]);
        assert_eq!(d.value, Independence::DistinctSeats, "{d:?}");
        assert!(d.because.contains("nothing to collide with"), "{d:?}");
    }

    // ── the check ─────────────────────────────────────────────────────────────

    /// The issue's mutation, both directions. Seats that share a configuration while the
    /// record states `distinct-seats` go red; the same fixture with the seats differing
    /// stays green.
    #[test]
    fn a_record_claiming_distinct_seats_over_a_shared_configuration_is_reported() {
        let shared = [record(
            "resolutions/e.md",
            "distinct-seats",
            vec![
                agent("ma/auditor", "claude-opus-4-8", "1", "c0ffee"),
                agent("ma/advocate", "claude-opus-4-8", "1", "c0ffee"),
            ],
        )];
        let c = independence_mismatch(&shared);
        assert!(!c.passed(), "{c:#?}");
        assert_eq!(c.severity, Severity::Info);
        let detail = &c.violations[0].detail;
        assert!(detail.contains("states `distinct-seats`"), "{detail}");
        assert!(detail.contains("derive `shared-configuration`"), "{detail}");

        let differing = [record(
            "resolutions/e.md",
            "distinct-seats",
            vec![
                agent("ma/auditor", "claude-opus-4-8", "1", "c0ffee"),
                agent("ma/advocate", "claude-opus-4-8", "1", "decafbad"),
            ],
        )];
        let c = independence_mismatch(&differing);
        assert!(c.passed(), "{c:#?}");
    }

    /// A blank column must not be reported as either other value — the check's half of the
    /// amendment's MUST NOT.
    #[test]
    fn a_blank_column_is_reported_as_unrecorded_and_not_as_either_other_value() {
        let a = [record(
            "resolutions/e.md",
            "distinct-seats",
            vec![
                agent("ma/auditor", "claude-opus-4-8", "1", "c0ffee"),
                agent("ma/advocate", "claude-opus-4-8", "1", ""),
            ],
        )];
        let c = independence_mismatch(&a);
        assert!(!c.passed(), "{c:#?}");
        let detail = &c.violations[0].detail;
        assert!(detail.contains("derive `unrecorded`"), "{detail}");
        assert!(
            !detail.contains("derive `shared-configuration`")
                && !detail.contains("derive `distinct-seats`"),
            "{detail}"
        );
    }

    /// The silence that keeps this check off the whole measured corpus: nothing stated, and
    /// nothing the registry can say. There is no repair to recommend.
    #[test]
    fn a_record_stating_nothing_whose_registry_says_nothing_is_not_reported() {
        let a = [record(
            "resolutions/e.md",
            "",
            vec![
                agent("ma/auditor", "", "", ""),
                agent("ma/advocate", "", "", ""),
            ],
        )];
        let c = independence_mismatch(&a);
        assert!(c.passed(), "{c:#?}");
    }

    /// …but a record stating nothing whose registry *can* answer is told the answer. This is
    /// the arm that arms itself as registries fill in.
    #[test]
    fn a_record_stating_nothing_over_a_registry_that_answers_is_told_the_answer() {
        let a = [record(
            "resolutions/e.md",
            "",
            vec![
                agent("ma/auditor", "claude-opus-4-8", "1", "c0ffee"),
                agent("ma/advocate", "claude-opus-4-8", "1", "c0ffee"),
            ],
        )];
        let c = independence_mismatch(&a);
        assert!(!c.passed(), "{c:#?}");
        let detail = &c.violations[0].detail;
        assert!(detail.contains("no `independence:`"), "{detail}");
        assert!(detail.contains("derive `shared-configuration`"), "{detail}");
    }

    /// A record under-claiming is still a record disagreeing with its own ancestry. Stale
    /// rather than false, and reported the same way because the repair is the same.
    #[test]
    fn stating_unrecorded_over_a_registry_that_distinguishes_the_seats_is_reported() {
        let a = [record(
            "resolutions/e.md",
            "unrecorded",
            vec![
                agent("ma/auditor", "claude-opus-4-8", "1", "c0ffee"),
                agent("ma/advocate", "claude-opus-4-8", "1", "decafbad"),
            ],
        )];
        let c = independence_mismatch(&a);
        assert!(!c.passed(), "{c:#?}");
        assert!(
            c.violations[0].detail.contains("derive `distinct-seats`"),
            "{:#?}",
            c.violations[0]
        );
    }

    /// The vocabulary is closed, so a value outside it is a finding of its own rather than
    /// something to normalize into the nearest member.
    #[test]
    fn a_value_outside_the_vocabulary_is_reported_as_one() {
        let a = [record(
            "resolutions/e.md",
            "distinct_seats",
            vec![agent("ma/auditor", "claude-opus-4-8", "1", "c0ffee")],
        )];
        let c = independence_mismatch(&a);
        assert!(!c.passed(), "{c:#?}");
        assert!(
            c.violations[0].detail.contains("is not one of"),
            "{:#?}",
            c.violations[0]
        );
    }

    /// Every arm of the vocabulary is reachable as a *stated* value that agrees, so the check
    /// is silent on a record that got it right — including the one that says *we cannot tell*.
    #[test]
    fn a_record_that_states_the_derived_value_passes_on_every_arm() {
        let cases = [
            (
                "distinct-seats",
                vec![
                    agent("ma/auditor", "claude-opus-4-8", "1", "c0ffee"),
                    agent("ma/advocate", "claude-opus-4-8", "1", "decafbad"),
                ],
            ),
            (
                "shared-configuration",
                vec![
                    agent("ma/auditor", "claude-opus-4-8", "1", "c0ffee"),
                    agent("ma/advocate", "claude-opus-4-8", "1", "c0ffee"),
                ],
            ),
            (
                "unrecorded",
                vec![
                    agent("ma/auditor", "claude-opus-4-8", "1", "c0ffee"),
                    agent("ma/advocate", "claude-opus-4-8", "1", ""),
                ],
            ),
        ];
        for (stated, seats) in cases {
            let c = independence_mismatch(&[record("resolutions/e.md", stated, seats)]);
            assert!(c.passed(), "stating `{stated}` should agree: {c:#?}");
        }
    }

    // ── against a repository ──────────────────────────────────────────────────
    //
    // Everything above is data. These are the half that decides Q1, and the first one is a
    // test HEAD cannot pass: the registry says one thing at the tips and another at HEAD, and
    // only one of those answers is the record's own ancestry.

    use crate::git::fixture::{commit, git, git_out, write};

    fn head(dir: &std::path::Path) -> String {
        git_out(dir, &["rev-parse", "--short", "HEAD"])
    }

    /// A registry with the two seats at the configs given.
    fn registry(auditor: &str, advocate: &str) -> String {
        format!(
            "# Electors\n\n\
             | Name | Branch | Role | Kind | Model | Version | Config |\n\
             |------|--------|------|------|-------|---------|--------|\n\
             | `auditor` | `ma/auditor` | Audits. | agent | `claude-opus-4-8` | `1` | `{auditor}` |\n\
             | `advocate` | `ma/advocate` | Advocates. | agent | `claude-opus-4-8` | `1` | `{advocate}` |\n"
        )
    }

    fn repo() -> tempfile::TempDir {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path();
        git(root, &["init", "-q", "-b", "main"]);
        git(root, &["config", "user.email", "t@t.com"]);
        git(root, &["config", "user.name", "T"]);
        git(root, &["config", "commit.gpgsign", "false"]);
        tmp
    }

    fn resolution(file: &str, stated: &str, tips: &[String]) -> Resolution {
        Resolution {
            file: file.to_string(),
            evolution: "e".to_string(),
            date: "2026-01-01".to_string(),
            tips: tips.to_vec(),
            synthesized_by: vec!["ma/auditor".to_string()],
            independence: stated.to_string(),
            branch_present: true,
        }
    }

    /// **Q1, decided and proved.** The two seats shared a configuration when the resolution
    /// read them and differ at HEAD, because somebody bumped one. Reading HEAD would call the
    /// record correct; reading the tips reports what was true when it was written. Nothing but
    /// a registry that moved can tell the two implementations apart, which is why this test
    /// moves it.
    #[test]
    fn the_registry_is_read_at_the_tips_and_not_at_head() {
        let tmp = repo();
        let root = tmp.path();
        write(root, REGISTRY, &registry("c0ffee", "c0ffee"));
        commit(root, "register: two seats on one configuration");
        let tip = head(root);

        write(root, REGISTRY, &registry("c0ffee", "decafbad"));
        commit(root, "update: the advocate's configuration is material");

        let records = [resolution(
            "resolutions/e.md",
            "distinct-seats",
            &[format!("ma/auditor@{tip}"), format!("ma/advocate@{tip}")],
        )];
        let audits = audit(root, &records);
        let d = derive(&audits[0].seats);
        assert_eq!(
            d.value,
            Independence::SharedConfiguration,
            "HEAD says distinct; the tips say shared: {d:?}"
        );
        let c = independence_mismatch(&audits);
        assert!(!c.passed(), "{c:#?}");
    }

    /// Each seat at *its own* tip, which is what *"the state of the agent that held it"*
    /// means. An implementation that read the whole registry at the first tip would report
    /// these two as sharing a configuration, because at that revision they did.
    #[test]
    fn each_seat_is_read_at_the_tip_that_names_it() {
        let tmp = repo();
        let root = tmp.path();
        write(root, REGISTRY, &registry("c0ffee", "c0ffee"));
        commit(root, "register: two seats on one configuration");
        let early = head(root);

        write(root, REGISTRY, &registry("c0ffee", "decafbad"));
        commit(root, "update: the advocate moved");
        let late = head(root);

        let records = [resolution(
            "resolutions/e.md",
            "",
            &[format!("ma/auditor@{early}"), format!("ma/advocate@{late}")],
        )];
        let audits = audit(root, &records);
        let d = derive(&audits[0].seats);
        assert_eq!(
            d.value,
            Independence::DistinctSeats,
            "the advocate held `decafbad` at the tip this record names: {d:?}"
        );
    }

    /// The same seat named at two tips is one seat. Left undeduplicated it would collide with
    /// itself, and every record that names a seat twice would report `shared-configuration`
    /// against a sangha with nothing wrong with it.
    #[test]
    fn one_seat_named_at_two_tips_is_one_seat() {
        let tmp = repo();
        let root = tmp.path();
        write(root, REGISTRY, &registry("c0ffee", "decafbad"));
        commit(root, "register: two seats");
        let first = head(root);
        write(root, "note.md", "a later commit\n");
        commit(root, "note: time passes");
        let second = head(root);

        let records = [resolution(
            "resolutions/e.md",
            "",
            &[
                format!("ma/auditor@{first}"),
                format!("ma/auditor@{second}"),
            ],
        )];
        let audits = audit(root, &records);
        assert_eq!(audits[0].seats.len(), 1, "{:#?}", audits[0].seats);
        assert_eq!(derive(&audits[0].seats).value, Independence::DistinctSeats);
    }

    /// Q1's other half: a tip this clone does not carry is `unrecorded`, and there is no
    /// fallback to HEAD — which is present here, and describes both seats.
    #[test]
    fn a_tip_absent_from_the_clone_does_not_fall_back_to_head() {
        let tmp = repo();
        let root = tmp.path();
        write(root, REGISTRY, &registry("c0ffee", "decafbad"));
        commit(root, "register: two seats");

        let records = [resolution(
            "resolutions/e.md",
            "distinct-seats",
            &[
                "ma/auditor@dedbeef".to_string(),
                "ma/advocate@dedbeef".to_string(),
            ],
        )];
        let audits = audit(root, &records);
        assert!(audits[0].seats.iter().all(|s| s.row.is_none()));
        let d = derive(&audits[0].seats);
        assert_eq!(d.value, Independence::Unrecorded, "{d:?}");
        assert!(d.because.contains("no commit in this clone"), "{d:?}");
    }

    /// A seat registered after the resolution it took part in has no row at that tip. The
    /// registry genuinely does not say, and `resolution-elector-unregistered` owns the
    /// separate question of whether the seat is registered at all.
    #[test]
    fn a_seat_with_no_row_at_its_tip_is_unrecorded() {
        let tmp = repo();
        let root = tmp.path();
        write(
            root,
            REGISTRY,
            "# Electors\n\n\
             | Name | Branch | Role | Kind | Model | Version | Config |\n\
             |------|--------|------|------|-------|---------|--------|\n\
             | `auditor` | `ma/auditor` | Audits. | agent | `claude-opus-4-8` | `1` | `c0ffee` |\n",
        );
        commit(root, "register: one seat");
        let tip = head(root);

        let records = [resolution(
            "resolutions/e.md",
            "distinct-seats",
            &[format!("ma/auditor@{tip}"), format!("ma/advocate@{tip}")],
        )];
        let audits = audit(root, &records);
        assert_eq!(derive(&audits[0].seats).value, Independence::Unrecorded);
    }

    /// A repository that runs no sangha spawns no subprocess and reports nothing — the state
    /// nearly every derived repository is in.
    #[test]
    fn a_corpus_with_no_resolutions_audits_nothing() {
        let tmp = repo();
        assert!(audit(tmp.path(), &[]).is_empty());
        assert!(independence_mismatch(&[]).passed());
    }

    #[test]
    fn a_tip_with_no_hash_names_a_branch_and_no_revision() {
        assert_eq!(
            split_tip("ma/auditor@0b12292"),
            ("ma/auditor", Some("0b12292"))
        );
        assert_eq!(split_tip("ma/auditor"), ("ma/auditor", None));
        assert_eq!(split_tip("ma/auditor@"), ("ma/auditor@", None));
    }
}
