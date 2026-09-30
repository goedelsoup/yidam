//! What a dispatched elector's receipt says ran, against what its seat's row says (#477).
//!
//! `yidam dispatch` commits a receipt beside every position it proposes, at
//! `.yidam/runs/elector/<name>.yml`, recording the model, version and configuration digest the
//! run was declared under. The registry row says the same three things about the seat. The two
//! are written by different hands — the row by whoever registered the seat, the receipt by the
//! run — and a seat whose row and receipt disagree is a seat whose `independence:` reading is
//! about an occupant that is not the one that answered.
//!
//! # Registry first, receipt corroborates
//!
//! **Nothing here feeds [`super::independence::derive`].** The derivation reads the registry
//! alone, at each seat's tip, and a receipt can neither settle a question the registry leaves
//! open nor overturn one it answers. What a receipt can do is disagree, and this check is where
//! that disagreement is said out loud. So a receipt that fills a blank column is a finding and
//! not a repair: the registry has not recorded what the run did, and it is the registry that is
//! read.
//!
//! # Where it reads
//!
//! **At a tip, both files from the same one.** A receipt from one revision against a row from
//! another would compare two moments, and a model bump between them would read as a
//! disagreement nobody made. The tips are each seat's current `ma/*` ref and every tip a
//! resolution names, so a settled record's seats are checked as they stood when they were read.
//!
//! A tip with no receipt is silent: a position a person wrote has none, and that is the ordinary
//! case rather than a gap. A receipt field that is absent is silent for the same reason.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use super::history::read_blobs;
use super::model::{Check, Severity, Violation};
use crate::cmd::sangha::{parse_electors, ElectorRow, Resolution};

/// The registry, at whatever revision it is being read from.
const REGISTRY: &str = ".yidam/sangha/electors.md";

/// Where a dispatched elector's receipt lives, by seat name.
pub(crate) fn receipt_path(name: &str) -> String {
    crate::cmd::run::receipt::Receipt::path(&format!("elector/{name}"))
}

/// The three fields of a receipt this check compares. A reader of a format a producer owns, so
/// it takes only what it asks about — [`crate::cmd::run::receipt::Receipt::landed`]'s argument.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize)]
pub(crate) struct Occupant {
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub config: Option<String>,
}

impl Occupant {
    /// `None` where the text is not YAML, which is silence rather than a finding: this check is
    /// about disagreement, and an unreadable receipt states nothing to disagree with.
    pub(crate) fn parse(text: &str) -> Option<Self> {
        serde_yaml::from_str(text).ok()
    }
}

/// Whether a registry `Config` cell names this digest.
///
/// The whole digest, or a prefix of it at least seven characters long — the width git
/// abbreviates a hash to, and the form a person writing a table cell will reach for. Shorter
/// than that is not an abbreviation anyone could check, so it does not match.
///
/// Shared with `yidam dispatch`, which refuses a seat whose row this does not hold for: the
/// dispatcher and the lint must agree on what *the same configuration* means, or a run the
/// dispatcher accepted would be one the lint reports.
pub(crate) fn config_matches(cell: &str, digest: &str) -> bool {
    let cell = cell.trim();
    cell == digest || (cell.len() >= 7 && digest.starts_with(cell))
}

/// One receipt at one tip, and the row the registry at that tip carries for its seat.
pub(crate) struct ReceiptAudit {
    /// `ma/<name>@<hash>`.
    pub at: String,
    /// The seat's branch, `ma/<name>`.
    pub branch: String,
    pub occupant: Occupant,
    pub row: Option<ElectorRow>,
}

/// Every way this receipt and this row disagree, in the registry's column order.
pub(crate) fn disagreements(
    occupant: &Occupant,
    row: Option<&ElectorRow>,
    branch: &str,
) -> Vec<String> {
    let Some(row) = row else {
        return vec![format!(
            "the registry at this tip carries no row for `{branch}`, so nothing says what the \
             seat's occupant is"
        )];
    };
    if row.kind.eq_ignore_ascii_case("human") {
        return vec![format!(
            "the row says `{branch}` is a human seat, and a receipt records a dispatched run"
        )];
    }
    let mut out = Vec::new();
    for (column, cell, recorded) in [
        ("Model", &row.model, &occupant.model),
        ("Version", &row.version, &occupant.version),
        ("Config", &row.config, &occupant.config),
    ] {
        let Some(recorded) = recorded.as_deref() else {
            continue;
        };
        let cell = cell.trim();
        let agrees = if column == "Config" {
            config_matches(cell, recorded)
        } else {
            cell == recorded
        };
        if agrees {
            continue;
        }
        out.push(if cell.is_empty() {
            format!("`{column}` does not record `{recorded}`, which the run did")
        } else {
            format!("`{column}` is `{cell}` and the run recorded `{recorded}`")
        });
    }
    out
}

pub(crate) fn elector_receipt_disagrees(audits: &[ReceiptAudit]) -> Check {
    let violations = audits
        .iter()
        .filter_map(|a| {
            let found = disagreements(&a.occupant, a.row.as_ref(), &a.branch);
            (!found.is_empty()).then(|| {
                let name = a.branch.trim_start_matches("ma/");
                Violation::new(
                    format!("{}:{}", a.at, receipt_path(name)),
                    format!("at `{}`, {}", a.at, found.join("; ")),
                )
            })
        })
        .collect();
    Check::new(
        "elector-receipt-disagrees",
        "Elector receipt disagrees with the seat's registry row",
        Severity::Warn,
        "`yidam dispatch` records the model, version and configuration digest an elector ran \
         under in a receipt beside the position, and `electors.md` records the same three about \
         the seat. The registry is what a resolution's `independence:` is derived from; the \
         receipt only corroborates it. A receipt that disagrees with the row at the same tip — \
         or records a value the row leaves blank — means the derivation is describing an \
         occupant that is not the one that answered, and nothing else would say so. Both files \
         are read at the same tip, so a model bump between two revisions is never reported as \
         a disagreement. Warn rather than a gate: the repair is a registry update or a new \
         dispatch, and either is a person's call.",
        violations,
    )
}

// ── reading the repository ────────────────────────────────────────────────────

/// Every receipt at every tip this repository can name, beside the row it is compared with.
///
/// The tips are each seat's current `ma/*` ref and every `ma/<name>@<hash>` a resolution names.
/// Two names for one commit are one tip: refs are resolved to their abbreviated hash first, so
/// a seat whose current tip a resolution also names is reported once.
pub(crate) fn audit(root: &Path, records: &[Resolution]) -> Vec<ReceiptAudit> {
    let mut tips: BTreeSet<(String, String)> = BTreeSet::new();
    for r in crate::git::phase_refs(root)
        .into_iter()
        .filter(|r| r.kind == crate::git::RefKind::Position)
    {
        if let Some(hash) = crate::git::Git::new(root)
            .args(["rev-parse", "--short", "--verify", "--quiet"])
            .rev(format!("{}^{{commit}}", r.git_ref))
            .try_run()
        {
            tips.insert((r.name, hash));
        }
    }
    for tip in records.iter().flat_map(|r| r.tips.iter()) {
        if let Some((branch, hash)) = tip.split_once('@') {
            if branch.starts_with("ma/") && !hash.is_empty() {
                // A resolution may abbreviate differently from this clone; resolve it so the
                // same commit named twice is not audited twice.
                let hash = crate::git::Git::new(root)
                    .args(["rev-parse", "--short", "--verify", "--quiet"])
                    .rev(format!("{hash}^{{commit}}"))
                    .try_run()
                    .unwrap_or_else(|| hash.to_string());
                tips.insert((branch.to_string(), hash));
            }
        }
    }

    let specs: Vec<String> = tips
        .iter()
        .flat_map(|(branch, hash)| {
            let name = branch.trim_start_matches("ma/");
            [
                format!("{hash}:{}", receipt_path(name)),
                format!("{hash}:{REGISTRY}"),
            ]
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let blobs = read_blobs(root, &specs);
    let registries: BTreeMap<&str, Vec<ElectorRow>> = tips
        .iter()
        .filter_map(|(_, hash)| {
            blobs
                .get(&format!("{hash}:{REGISTRY}"))
                .map(|t| (hash.as_str(), parse_electors(t)))
        })
        .collect();

    tips.iter()
        .filter_map(|(branch, hash)| {
            let name = branch.trim_start_matches("ma/");
            let occupant = Occupant::parse(blobs.get(&format!("{hash}:{}", receipt_path(name)))?)?;
            let row = registries
                .get(hash.as_str())
                .and_then(|rows| rows.iter().find(|row| &row.branch == branch))
                .cloned();
            Some(ReceiptAudit {
                at: format!("{branch}@{hash}"),
                branch: branch.clone(),
                occupant,
                row,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::fixture::{commit, git, git_out, write};

    fn row(model: &str, version: &str, config: &str) -> ElectorRow {
        ElectorRow {
            name: "auditor".into(),
            branch: "ma/auditor".into(),
            role: "Audits.".into(),
            kind: "agent".into(),
            model: model.into(),
            version: version.into(),
            config: config.into(),
            key: String::new(),
        }
    }

    fn ran(model: &str, version: &str, config: &str) -> Occupant {
        Occupant {
            model: Some(model.into()),
            version: Some(version.into()),
            config: Some(config.into()),
        }
    }

    const DIGEST: &str = "c0ffee00c0ffee00c0ffee00c0ffee00c0ffee00c0ffee00c0ffee00c0ffee00";

    #[test]
    fn a_receipt_that_matches_its_row_is_silent() {
        let found = disagreements(
            &ran("claude-opus-4-8", "1", DIGEST),
            Some(&row("claude-opus-4-8", "1", DIGEST)),
            "ma/auditor",
        );
        assert!(found.is_empty(), "{found:?}");
    }

    #[test]
    fn a_config_cell_may_abbreviate_the_digest_to_seven_and_no_fewer() {
        assert!(config_matches("c0ffee0", DIGEST));
        assert!(config_matches(DIGEST, DIGEST));
        assert!(
            !config_matches("c0ffee", DIGEST),
            "six characters is not a hash anyone checks"
        );
        assert!(!config_matches("decafba", DIGEST));
        assert!(!config_matches("", DIGEST));
    }

    #[test]
    fn a_model_the_row_does_not_name_is_reported_with_both_values() {
        let found = disagreements(
            &ran("claude-opus-5-5", "1", DIGEST),
            Some(&row("claude-opus-4-8", "1", DIGEST)),
            "ma/auditor",
        );
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(
            found[0].contains("`claude-opus-4-8`") && found[0].contains("`claude-opus-5-5`"),
            "{found:?}"
        );
    }

    /// Registry first: a receipt filling a blank column is not a repair of the row, it is a
    /// row that has not recorded what ran.
    #[test]
    fn a_blank_column_the_receipt_fills_is_a_finding_and_not_a_repair() {
        let found = disagreements(
            &ran("claude-opus-4-8", "1", DIGEST),
            Some(&row("claude-opus-4-8", "1", "")),
            "ma/auditor",
        );
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(found[0].contains("does not record"), "{found:?}");
    }

    #[test]
    fn a_field_the_receipt_does_not_carry_is_not_compared() {
        let found = disagreements(
            &Occupant {
                model: Some("claude-opus-4-8".into()),
                ..Occupant::default()
            },
            Some(&row("claude-opus-4-8", "2", "anything")),
            "ma/auditor",
        );
        assert!(found.is_empty(), "{found:?}");
    }

    #[test]
    fn no_row_and_a_human_row_are_each_their_own_finding() {
        let occupant = ran("m", "1", DIGEST);
        let none = disagreements(&occupant, None, "ma/auditor");
        assert!(none[0].contains("no row"), "{none:?}");
        let mut human = row("", "", "");
        human.kind = "human".into();
        let found = disagreements(&occupant, Some(&human), "ma/auditor");
        assert!(found[0].contains("human seat"), "{found:?}");
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

    fn registry(model: &str) -> String {
        format!(
            "# Electors\n\n\
             | Name | Branch | Role | Kind | Model | Version | Config |\n\
             |------|--------|------|------|-------|---------|--------|\n\
             | `auditor` | `ma/auditor` | Audits. | agent | `{model}` | `1` | `c0ffee0` |\n"
        )
    }

    fn receipt(model: &str) -> String {
        format!(
            "format_version: 2\nstep: elector/auditor\nkind: elector\nmodel: {model}\nversion: \
             '1'\nconfig: {DIGEST}\n"
        )
    }

    /// The issue's fixture: a receipt and a row that differ, at the same tip, read through git.
    #[test]
    fn a_seat_whose_receipt_and_row_differ_at_its_tip_is_reported() {
        let tmp = repo();
        let root = tmp.path();
        write(root, REGISTRY, &registry("claude-opus-4-8"));
        commit(root, "genesis: registry");
        git(root, &["switch", "-q", "-c", "ma/auditor"]);
        write(root, &receipt_path("auditor"), &receipt("claude-opus-5-5"));
        commit(root, "open: auditor on q");
        let tip = git_out(root, &["rev-parse", "--short", "HEAD"]);
        git(root, &["switch", "-q", "main"]);

        let audits = audit(root, &[]);
        assert_eq!(audits.len(), 1);
        let check = elector_receipt_disagrees(&audits);
        assert_eq!(check.violations.len(), 1, "{:?}", check.violations);
        let v = &check.violations[0];
        assert!(v.detail.contains(&format!("ma/auditor@{tip}")), "{v:?}");
        assert!(v.detail.contains("`claude-opus-5-5`"), "{v:?}");
    }

    #[test]
    fn a_seat_whose_receipt_and_row_agree_at_its_tip_is_silent() {
        let tmp = repo();
        let root = tmp.path();
        write(root, REGISTRY, &registry("claude-opus-4-8"));
        commit(root, "genesis: registry");
        git(root, &["switch", "-q", "-c", "ma/auditor"]);
        write(root, &receipt_path("auditor"), &receipt("claude-opus-4-8"));
        commit(root, "open: auditor on q");
        git(root, &["switch", "-q", "main"]);

        let audits = audit(root, &[]);
        assert_eq!(audits.len(), 1);
        assert!(elector_receipt_disagrees(&audits).violations.is_empty());
    }

    /// Both files from one revision: the row bumped on `main` does not reach a seat whose tip
    /// still carries the old row, and a resolution naming that tip is the same tip, audited once.
    #[test]
    fn the_row_is_read_at_the_receipts_own_tip_and_a_tip_named_twice_is_audited_once() {
        let tmp = repo();
        let root = tmp.path();
        write(root, REGISTRY, &registry("claude-opus-4-8"));
        commit(root, "genesis: registry");
        git(root, &["switch", "-q", "-c", "ma/auditor"]);
        write(root, &receipt_path("auditor"), &receipt("claude-opus-4-8"));
        commit(root, "open: auditor on q");
        let tip = git_out(root, &["rev-parse", "HEAD"]);
        git(root, &["switch", "-q", "main"]);
        write(root, REGISTRY, &registry("claude-opus-5-5"));
        commit(root, "update: auditor moves to 5.5");

        let record = Resolution {
            file: ".yidam/sangha/resolutions/q.md".into(),
            evolution: "e".into(),
            date: "2026-01-01".into(),
            tips: vec![format!("ma/auditor@{}", &tip[..10])],
            synthesized_by: vec![],
            independence: String::new(),
            rounds: String::new(),
            positions: vec![],
            branch_present: true,
        };
        let audits = audit(root, &[record]);
        assert_eq!(
            audits.len(),
            1,
            "one commit, named by a ref and a resolution"
        );
        assert!(elector_receipt_disagrees(&audits).violations.is_empty());
    }
}
