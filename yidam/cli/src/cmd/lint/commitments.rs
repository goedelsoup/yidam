//! What a seat holds, and whether it is still holding it.
//!
//! An elector is a seat, and an *agent* elector is a seat whose occupant arrives with no memory of
//! having sat there before. `CONSTITUTION.md` Article VI licenses a `ma/*` branch to diverge
//! freely from `rigpa/*` and calls divergence normal and expected — which it is, and which
//! presumes the thing doing the diverging carries a position between acts. A branch maintained by
//! a succession of cold instances does not. It is a random walk that looks like deliberation, and
//! it satisfies every constitutional check this repository has: Article V decides what a
//! *resolution* may synthesize from the tips in front of it, and nothing reads one seat across
//! time.
//!
//! # The norm existed and the artifact did not
//!
//! `sangha/positions/README.md` already says it: *"An elector who changes their mind writes a new
//! position for the new question and says in it which earlier ground of theirs did not survive."*
//! A norm whose alternative leaves no trace is hard to tell from an absent one. Measured on
//! 2026-09-22 in the one repository that has run this protocol — 71 positions, 30 resolutions,
//! three seats — **17 positions link another elector's and 4 link one of the author's own**. The
//! loop is built to make electors read each other and it works. Reading themselves is the
//! direction nothing supported.
//!
//! # What is decided here, and what is not
//!
//! A seat's commitments file is an **index**, not an argument, and that is what makes any of this
//! checkable. Article V's commentary draws the line this module is built on: a node and an edge
//! carry an identity of their own so membership can be decided, and a claim does not, so whether
//! two sentences assert the same thing stays with the elector. Applied one layer out, the item's
//! identity is **the position it links** — never its prose. So wording may be revised at any time,
//! and a position that was named under `holds` may not quietly stop being named at all.
//!
//! What is deliberately not checked is whether a withdrawal *engaged* the argument it reverses.
//! That is the same judgement under a different name, and [#294](https://github.com/goedelsoup/yidam/issues/294)
//! asked for it. A seat can withdraw a ground in one contentless line and nothing here will say
//! so. What the checks guarantee is the narrower part that can be guaranteed: the reversal is on
//! the record, in the seat's own hand, next to the ground it replaced.

use std::collections::{BTreeSet, HashMap};
use std::path::Path;

use super::lineage::git;
use super::model::{Check, Severity, Violation};
use crate::git::RefKind;

const SANGHA: &str = ".yidam/sangha";
const HOLDS: &str = "## What this seat holds";
const WITHDRAWN: &str = "## What this seat has withdrawn";

/// One seat's commitments file, as its own branch carries it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Commitments {
    /// `ma/<elector>`.
    pub branch: String,
    /// Repo-relative path the file would live at, whether or not it does.
    pub file: String,
    /// Whether the branch tip carries it.
    pub present: bool,
    /// How many positions this seat has filed.
    pub filed: usize,
    /// Required headings the file does not carry. Empty when absent or well-formed.
    pub missing_sections: Vec<&'static str>,
    /// Positions this seat filed that neither section names, as repo-relative paths.
    pub unindexed: Vec<String>,
    /// `(position path, short sha)` — a ground that left `holds` and entered neither section.
    pub vanished: Vec<(String, String)>,
}

// ── the checks ────────────────────────────────────────────────────────────────

pub(crate) fn absent(seats: &[Commitments]) -> Check {
    let violations = seats
        .iter()
        .filter(|c| !c.present && c.filed > 0)
        .map(|c| {
            Violation::new(
                c.branch.clone(),
                format!(
                    "has filed {} position(s) and its branch tip carries no `{}`",
                    c.filed, c.file
                ),
            )
        })
        .collect();
    Check::new(
        "elector-commitments-absent",
        "Elector branch carries no commitments file",
        Severity::Info,
        "A seat that has argued something is standing on it, and between two acts of the same \
         seat nothing in this repository said what. Info rather than Error because adopting the \
         file is a decision a sangha makes once and an unadopted convention is not a defect — a \
         corpus mid-adoption would otherwise be told its every seat is in violation. It fires \
         only for a seat that has actually filed a position: a branch opened this morning has \
         nothing to index yet, and reporting it would train people to ignore the check before it \
         ever had a subject.",
        violations,
    )
}

pub(crate) fn malformed(seats: &[Commitments]) -> Check {
    let violations = seats
        .iter()
        .filter(|c| !c.missing_sections.is_empty())
        .map(|c| {
            Violation::new(
                c.file.clone(),
                format!(
                    "is missing {}",
                    c.missing_sections
                        .iter()
                        .map(|s| format!("`{s}`"))
                        .collect::<Vec<_>>()
                        .join(" and ")
                ),
            )
        })
        .collect();
    Check::new(
        "elector-commitments-malformed",
        "Commitments file does not carry both required sections",
        Severity::Error,
        "This gates because otherwise the file disarms the rest. A commitments file with no \
         headings parses to two empty sections, empty sections lose nothing, and every vanished \
         ground would read as clean — a seat could turn `elector-commitment-vanished` off by \
         writing a worse file. The two headings are the whole of the required shape, and nothing \
         below them is prescribed: order, grouping, and how much of the argument is restated are \
         the elector's, because an index of positions is structural where a position is argument.",
        violations,
    )
}

pub(crate) fn position_unindexed(seats: &[Commitments]) -> Check {
    let violations = seats
        .iter()
        .flat_map(|c| {
            c.unindexed.iter().map(|p| {
                Violation::new(
                    p.clone(),
                    format!(
                        "is a position `{}` filed, and its commitments file names it under \
                         neither section",
                        c.branch
                    ),
                )
            })
        })
        .collect();
    Check::new(
        "elector-position-unindexed",
        "A seat's own position appears in neither section of its commitments",
        Severity::Info,
        "The earliest visible symptom of a seat losing its thread, and the cheapest to answer: \
         either the position argued a ground the seat still holds, or it argued one the seat has \
         since retired, and saying which is one line. Info because a third answer is legitimate \
         and common — the position argued something about procedure, or about another seat, and \
         is no ground of this seat's at all. A check that cannot tell those apart must not gate, \
         and this one cannot: which of a seat's positions carry commitments is exactly the \
         judgement Article V's commentary leaves with the elector.",
        violations,
    )
}

pub(crate) fn commitment_vanished(seats: &[Commitments]) -> Check {
    let violations = seats
        .iter()
        .flat_map(|c| {
            c.vanished.iter().map(|(position, sha)| {
                Violation::new(
                    position.clone(),
                    format!(
                        "`{}` held this ground and stopped naming it at `{sha}` — it is under \
                         neither `{HOLDS}` nor `{WITHDRAWN}`",
                        c.branch
                    ),
                )
            })
        })
        .collect();
    Check::new(
        "elector-commitment-vanished",
        "A ground left a seat's commitments without being withdrawn",
        Severity::Error,
        "Article III forbids a resolution silently discarding a tension it could not resolve. \
         This is the same rule read across one elector instead of across several: a ground may be \
         held, and it may be withdrawn — which is the interesting case and the one the loop was \
         built to produce — and it may not stop existing. Reversing a commitment is not the \
         violation and is never reported; leaving no trace of the reversal is. Deleting the file \
         is reported here rather than as the file going absent, because a deletion that takes \
         held grounds with it is precisely the silent discard, and routing it to the Info check \
         would make removing the file the cheapest way past the Error one.",
        violations,
    )
}

pub(crate) fn checks(seats: &[Commitments]) -> [Check; 4] {
    [
        absent(seats),
        malformed(seats),
        position_unindexed(seats),
        commitment_vanished(seats),
    ]
}

// ── reading a file ────────────────────────────────────────────────────────────

/// What one revision of a commitments file says, or that it cannot be read as one.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Indexed {
    pub holds: BTreeSet<String>,
    pub withdrawn: BTreeSet<String>,
    /// Required headings this revision does not carry.
    pub missing: Vec<&'static str>,
}

/// The positions named under each required section.
///
/// Targets are matched by basename against `<elector>-*.md`, which is the naming
/// `positions/README.md` prescribes. A link to *another* seat's position is context rather than a
/// commitment and is ignored — a seat indexes its own grounds, and the arguments it is answering
/// are already on the baseline where everyone can read them.
///
/// Fenced code is skipped for the reason the prose-link scanner skips it: a document explaining
/// the shape of a commitments file would otherwise be read as one.
pub(crate) fn index(elector: &str, text: &str) -> Indexed {
    let prefix = format!("{elector}-");
    let mut out = Indexed::default();
    let (mut saw_holds, mut saw_withdrawn) = (false, false);
    let mut section: Option<bool> = None; // Some(true) = holds, Some(false) = withdrawn
    let mut fence: Option<String> = None;

    for line in text.lines() {
        let trimmed = line.trim();
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
        // Only `#` and `##` close a section: a seat is free to group its grounds under `###`
        // headings, and taking those as the end of the section would silently drop everything
        // an organised file holds.
        if trimmed.starts_with("# ") || trimmed.starts_with("## ") {
            section = match trimmed {
                h if h == HOLDS => {
                    saw_holds = true;
                    Some(true)
                }
                w if w == WITHDRAWN => {
                    saw_withdrawn = true;
                    Some(false)
                }
                _ => None,
            };
            continue;
        }
        let Some(holds) = section else { continue };
        for target in link_targets(line) {
            let base = target.rsplit('/').next().unwrap_or(&target);
            if base.starts_with(&prefix) && base.ends_with(".md") {
                let name = base.to_string();
                if holds {
                    out.holds.insert(name);
                } else {
                    out.withdrawn.insert(name);
                }
            }
        }
    }

    if !saw_holds {
        out.missing.push(HOLDS);
    }
    if !saw_withdrawn {
        out.missing.push(WITHDRAWN);
    }
    out
}

/// Markdown link targets on one line, with any `#fragment` and title dropped.
fn link_targets(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let bytes = line.as_bytes();
    let mut i = 0;
    while let Some(at) = line[i..].find("](") {
        let start = i + at + 2;
        let Some(len) = line[start..].find(')') else {
            break;
        };
        let target = line[start..start + len].trim();
        let target = target.split_whitespace().next().unwrap_or(target);
        let target = target.split('#').next().unwrap_or(target);
        if !target.is_empty() {
            out.push(target.to_string());
        }
        i = start + len + 1;
        if i >= bytes.len() {
            break;
        }
    }
    out
}

// ── reading the repository ────────────────────────────────────────────────────

/// Every ground a seat stopped naming, and the commit where it happened.
///
/// Walks the file's own history on the branch. Two revisions are compared only when both can be
/// read: a revision missing a required heading is skipped rather than believed, because
/// [`malformed`] already gates on it and treating its empty sections as fact would report the
/// whole index as vanishing. A revision where the file is *absent* is not skipped — that is the
/// deletion case, and everything the previous revision held vanished with it.
fn vanished(root: &Path, git_ref: &str, path: &str, elector: &str) -> Vec<(String, String)> {
    let shas: Vec<String> = git(
        root,
        &["log", "--reverse", "--format=%H", git_ref, "--", path],
    )
    .lines()
    .map(str::to_string)
    .collect();
    if shas.len() < 2 {
        return Vec::new();
    }
    let revs: Vec<String> = shas.iter().map(|s| format!("{s}:{path}")).collect();
    let blobs = super::history::read_blobs(root, &revs);

    let mut out = Vec::new();
    let mut held: Option<BTreeSet<String>> = None;
    for (sha, rev) in shas.iter().zip(&revs) {
        let next = match blobs.get(rev) {
            None => None, // deleted at this commit
            Some(text) => {
                let idx = index(elector, text);
                if idx.missing.is_empty() {
                    Some(idx)
                } else {
                    held = None;
                    continue;
                }
            }
        };
        if let Some(previous) = held.take() {
            let still: BTreeSet<&String> = match &next {
                Some(idx) => idx.holds.iter().chain(idx.withdrawn.iter()).collect(),
                None => BTreeSet::new(),
            };
            for gone in previous.iter().filter(|p| !still.contains(*p)) {
                out.push((
                    format!("{SANGHA}/positions/{gone}"),
                    sha.chars().take(8).collect(),
                ));
            }
        }
        held = next.map(|idx| idx.holds);
    }
    out
}

/// What every seat's branch says it holds.
///
/// Positions are counted from the working tree, which is where [`super::scope`] reads them too:
/// step 2 transports every position onto the baseline, so the baseline is the one place the whole
/// set exists. The commitments file is read from the branch, which is the only place it exists —
/// it is never transported, deliberately, and [`crate::cmd::sangha`]'s protocol says why.
pub(crate) fn read(root: &Path) -> Vec<Commitments> {
    let seats: Vec<crate::git::PhaseRef> = crate::git::phase_refs(root)
        .into_iter()
        .filter(|r| r.kind == RefKind::Position)
        .collect();
    if seats.is_empty() {
        return Vec::new();
    }

    let mut filed: HashMap<String, Vec<String>> = HashMap::new();
    if let Ok(entries) = std::fs::read_dir(root.join(SANGHA).join("positions")) {
        for name in entries.flatten().filter_map(|e| {
            let n = e.file_name().to_string_lossy().to_string();
            (n.ends_with(".md") && n != "README.md").then_some(n)
        }) {
            if let Some((elector, _)) = name.trim_end_matches(".md").split_once('-') {
                filed.entry(elector.to_string()).or_default().push(name);
            }
        }
    }

    seats
        .iter()
        .map(|seat| {
            let elector = seat.name.strip_prefix("ma/").unwrap_or(&seat.name);
            let file = format!("{SANGHA}/commitments/{elector}.md");
            let mine = filed.get(elector).cloned().unwrap_or_default();
            let text = blob(root, &seat.git_ref, &file);
            let idx = text.as_deref().map(|t| index(elector, t));
            let unindexed = idx
                .as_ref()
                .map(|i| {
                    let mut out: Vec<String> = mine
                        .iter()
                        .filter(|p| !i.holds.contains(*p) && !i.withdrawn.contains(*p))
                        .map(|p| format!("{SANGHA}/positions/{p}"))
                        .collect();
                    out.sort();
                    out
                })
                .unwrap_or_default();
            Commitments {
                branch: seat.name.clone(),
                present: text.is_some(),
                filed: mine.len(),
                missing_sections: idx.map(|i| i.missing).unwrap_or_default(),
                unindexed,
                vanished: vanished(root, &seat.git_ref, &file, elector),
                file,
            }
        })
        .collect()
}

/// One file at one ref, or `None` when that ref does not carry it.
fn blob(root: &Path, git_ref: &str, path: &str) -> Option<String> {
    let rev = format!("{git_ref}:{path}");
    super::history::read_blobs(root, std::slice::from_ref(&rev)).remove(&rev)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(holds: &[&str], withdrawn: &[&str]) -> String {
        let items = |ps: &[&str], what: &str| {
            ps.iter()
                .map(|p| format!("- {what} — [x](../positions/{p})\n"))
                .collect::<String>()
        };
        format!(
            "# Commitments: ma/advocate\n\n{HOLDS}\n\n{}\n{WITHDRAWN}\n\n{}",
            items(holds, "a ground"),
            items(withdrawn, "retired"),
        )
    }

    #[test]
    fn both_sections_are_read_and_neither_is_missing() {
        let idx = index(
            "advocate",
            &file(&["advocate-a.md", "advocate-b.md"], &["advocate-c.md"]),
        );
        assert!(idx.missing.is_empty());
        assert_eq!(
            idx.holds,
            ["advocate-a.md".to_string(), "advocate-b.md".to_string()].into()
        );
        assert_eq!(idx.withdrawn, ["advocate-c.md".to_string()].into());
    }

    #[test]
    fn a_missing_heading_is_named_rather_than_read_as_empty() {
        let idx = index(
            "advocate",
            "# Commitments\n\n- a ground — [x](../positions/advocate-a.md)\n",
        );
        assert_eq!(idx.missing, vec![HOLDS, WITHDRAWN]);
        assert!(idx.holds.is_empty(), "nothing outside a section is indexed");
    }

    #[test]
    fn another_seats_position_is_context_and_not_a_commitment() {
        let text = format!(
            "{HOLDS}\n\n- answering [the auditor](../positions/auditor-x.md) — \
             [mine](../positions/advocate-a.md)\n\n{WITHDRAWN}\n"
        );
        let idx = index("advocate", &text);
        assert_eq!(idx.holds, ["advocate-a.md".to_string()].into());
    }

    #[test]
    fn a_fenced_example_is_not_an_index() {
        let text = format!(
            "{HOLDS}\n\n```markdown\n- [x](../positions/advocate-a.md)\n```\n\n{WITHDRAWN}\n"
        );
        assert!(index("advocate", &text).holds.is_empty());
    }

    #[test]
    fn a_subheading_does_not_end_a_section() {
        let text = format!(
            "{HOLDS}\n\n### On the frame\n\n- [x](../positions/advocate-a.md)\n\n{WITHDRAWN}\n"
        );
        assert_eq!(
            index("advocate", &text).holds,
            ["advocate-a.md".to_string()].into()
        );
    }

    #[test]
    fn an_unrelated_second_level_heading_does_end_one() {
        let text =
            format!("{HOLDS}\n\n## Notes\n\n- [x](../positions/advocate-a.md)\n\n{WITHDRAWN}\n");
        assert!(index("advocate", &text).holds.is_empty());
    }

    #[test]
    fn a_fragment_and_a_title_are_stripped_from_a_target() {
        assert_eq!(
            link_targets("see [a](../positions/advocate-a.md#why \"t\") and [b](b.md)"),
            vec!["../positions/advocate-a.md".to_string(), "b.md".to_string()]
        );
    }

    fn seat(
        missing: Vec<&'static str>,
        unindexed: &[&str],
        vanished: &[(&str, &str)],
    ) -> Commitments {
        Commitments {
            branch: "ma/advocate".to_string(),
            file: ".yidam/sangha/commitments/advocate.md".to_string(),
            present: true,
            filed: 3,
            missing_sections: missing,
            unindexed: unindexed.iter().map(|s| s.to_string()).collect(),
            vanished: vanished
                .iter()
                .map(|(p, s)| (p.to_string(), s.to_string()))
                .collect(),
        }
    }

    #[test]
    fn a_seat_that_has_filed_nothing_is_not_asked_for_an_index() {
        let never = Commitments {
            present: false,
            filed: 0,
            ..seat(Vec::new(), &[], &[])
        };
        assert!(absent(&[never]).passed());
    }

    #[test]
    fn a_seat_that_has_filed_and_kept_no_index_is_reported() {
        let c = Commitments {
            present: false,
            ..seat(Vec::new(), &[], &[])
        };
        let check = absent(&[c]);
        assert_eq!(check.violations.len(), 1);
        assert_eq!(check.severity, Severity::Info);
    }

    #[test]
    fn a_malformed_file_gates() {
        let check = malformed(&[seat(vec![WITHDRAWN], &[], &[])]);
        assert_eq!(check.severity, Severity::Error);
        assert_eq!(check.violations.len(), 1);
        assert!(check.violations[0].detail.contains(WITHDRAWN));
    }

    #[test]
    fn each_unindexed_position_is_its_own_finding() {
        let check = position_unindexed(&[seat(
            Vec::new(),
            &[
                ".yidam/sangha/positions/advocate-a.md",
                ".yidam/sangha/positions/advocate-b.md",
            ],
            &[],
        )]);
        assert_eq!(check.violations.len(), 2);
        assert_eq!(
            check.violations[0].node,
            ".yidam/sangha/positions/advocate-a.md"
        );
    }

    #[test]
    fn a_vanished_ground_gates_and_names_the_commit() {
        let check = commitment_vanished(&[seat(
            Vec::new(),
            &[],
            &[(".yidam/sangha/positions/advocate-a.md", "deadbeef")],
        )]);
        assert_eq!(check.severity, Severity::Error);
        assert_eq!(check.violations.len(), 1);
        assert!(check.violations[0].detail.contains("deadbeef"));
    }

    #[test]
    fn nothing_fires_on_a_repository_with_no_seats() {
        assert!(checks(&[]).iter().all(Check::passed));
    }
}
