//! `yidam catalog reconcile` — bring a `used-by` list back into agreement with the corpus.
//!
//! #460's table records `reconcile:` as *"catalog and corpus brought back into agreement"*,
//! performed by nothing: `catalog-audit` reports the disagreement and reconciles none of it.
//! The disagreement it reports is already computed, already shared with the gate, and already
//! has an authoritative side.
//!
//! # Why this can be mechanical when so little else here is
//!
//! Because one of the two lists cannot be wrong. `catalog-used-by-drift` states it:
//!
//! > The citations are authoritative — they cannot drift from the corpus, and a
//! > hand-maintained list can.
//!
//! A citation is an edge a node actually carries; `used-by` is a convenience copy of the same
//! fact, and a copy that disagrees with its source is stale by definition. So reconciling
//! needs no judgement about which side is right, which is exactly what makes it operational
//! — *"the pipeline advanced; no understanding changed"* — and therefore something a run may
//! author without a person in the loop.
//!
//! **It is a substitution, not a merge.** Names the list claims that no node carries are
//! removed rather than kept, because keeping them would be keeping the drift.
//!
//! # What it will not touch
//!
//! An entry declaring no `used-by` at all. Absence is not drift — `used_by_drift` returns
//! `None` for it and the gate stays silent — so writing one would be this command deciding an
//! entry ought to make a claim it never made. The list is optional, and an entry that declares
//! one is asserting it is current; that is the assertion this repairs, and the only one.

use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::{Context, Result};

use super::audit::draws_on;
use super::commit::{self, Commit};
use super::record;
use crate::cmd::lint::checks::{normalize, used_by_drift, UsedByDrift};
use crate::parse::parse_frontmatter;
use crate::paths::{repo_root, yidam_catalog_dir, yidam_corpus_dir};
use crate::walk::walk_md_files;

pub struct ReconcileOptions {
    /// Restrict to one entry, by file stem or `name:`.
    pub entry: Option<String>,
    /// Report the substitution and write nothing.
    pub dry_run: bool,
    pub format: crate::report::Format,
}

#[derive(Debug, Clone, serde::Serialize)]
struct Reconciled {
    entry: String,
    /// The drift as the gate and the audit report it, before the repair.
    drift: UsedByDrift,
    /// The list as it now stands.
    used_by: Vec<String>,
    commit: Option<Commit>,
}

/// `reconciled`, not `entries` — see [`super::fetch`]'s note on the same rename.
#[derive(serde::Serialize)]
struct ReconcileReport {
    reconciled: Vec<Reconciled>,
}

/// How a `used-by` entry is written: relative to the catalog directory the entry sits in.
///
/// The citations arrive repo-relative (`.yidam/corpus/gage/x.yml`) because that is what the
/// walk produces, and every hand-written list in this repository is entry-relative
/// (`../corpus/gage/x.yml`). Writing the walk's form would be a reconcile that "fixed" every
/// path in the file — a diff nobody could review for the one line that mattered.
///
/// `used_by_drift` compares by [`basename`](crate::cmd::lint::checks), so the two forms are
/// already equivalent to the gate. This is about what a person reads.
fn as_declared(repo_relative: &str) -> String {
    let tail = repo_relative
        .strip_prefix(".yidam/")
        .unwrap_or(repo_relative);
    format!("../{tail}")
}

pub fn reconcile(opts: &ReconcileOptions) -> Result<()> {
    let root = repo_root()?;
    let catalog = yidam_catalog_dir(&root);
    let corpus = yidam_corpus_dir(&root);
    let draws = if corpus.exists() {
        draws_on(&root, &corpus)
    } else {
        HashMap::new()
    };

    let entries: Vec<PathBuf> = walk_md_files(&catalog)
        .into_iter()
        .filter(|p| p.file_name().is_some_and(|n| n != "README.md"))
        .filter(|p| match opts.entry.as_deref() {
            None => true,
            Some(want) => {
                let want = want.trim().trim_end_matches(".md");
                let stem = p.file_stem().unwrap_or_default().to_string_lossy();
                if stem == want {
                    return true;
                }
                let text = std::fs::read_to_string(p).unwrap_or_default();
                parse_frontmatter(&text).name.as_deref() == Some(want)
            }
        })
        .collect();
    if entries.is_empty() {
        if let Some(want) = &opts.entry {
            anyhow::bail!(
                "no catalog entry named `{want}`. `yidam catalog-audit` lists what this \
                 corpus holds."
            );
        }
    }

    let mut out: Vec<Reconciled> = Vec::new();
    for path in &entries {
        let rel = path
            .strip_prefix(&root)
            .unwrap_or(path)
            .to_string_lossy()
            .to_string();
        let name = path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let text =
            std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        let declared = parse_frontmatter(&text).used_by.unwrap_or_default();

        // The citing instances, from the same walk `catalog-audit` reports and the gate
        // counts. `nodes` and not `total()`: `catalog-used-by-drift` compares against the
        // citations the gate reads, and a repair disagreeing with the gate about what drifted
        // would leave the check red after a successful reconcile.
        let citing = draws
            .get(&normalize(path))
            .cloned()
            .unwrap_or_default()
            .nodes;

        let Some(drift) = used_by_drift(&declared, &citing) else {
            continue;
        };
        if drift.claimed_not_citing.is_empty() && drift.citing_not_claimed.is_empty() {
            continue;
        }

        let want: Vec<String> = citing.iter().map(|c| as_declared(c)).collect();
        let Some(updated) = record::set_used_by(&text, &want)? else {
            continue;
        };

        let mut written = None;
        if !opts.dry_run {
            commit::require_clean(&root, &[rel.clone()])?;
            std::fs::write(path, &updated)
                .with_context(|| format!("writing {}", path.display()))?;
            let (subject, body) = message(&name, &drift);
            written = commit::author(&root, &subject, &body, &[rel.clone()])?;
        }

        out.push(Reconciled {
            entry: name,
            drift,
            used_by: want,
            commit: written,
        });
    }

    if opts.format.is_json() {
        return crate::report::emit(&root, ReconcileReport { reconciled: out });
    }
    print!("{}", render(&out, opts.dry_run));
    Ok(())
}

/// The subject and body of the `reconcile:` commit.
///
/// The body names what moved in each direction, because the diff alone does not say *why* a
/// line was removed — a path dropped from `used-by` looks identical whether the node stopped
/// citing the entry or the list was wrong all along, and only the first is a fact about the
/// corpus.
fn message(entry: &str, drift: &UsedByDrift) -> (String, String) {
    let added = drift.citing_not_claimed.len();
    let removed = drift.claimed_not_citing.len();
    let subject = format!("reconcile: {entry} used-by against the citations");
    let mut body = String::new();
    use std::fmt::Write;
    if added > 0 {
        let _ = writeln!(
            body,
            "added {added} that cite it and the list omitted: {}",
            drift.citing_not_claimed.join(", ")
        );
    }
    if removed > 0 {
        let _ = writeln!(
            body,
            "removed {removed} the list claimed that no node carries: {}",
            drift.claimed_not_citing.join(", ")
        );
    }
    let _ = writeln!(
        body,
        "\nThe citations are authoritative; a hand-maintained list can drift from them."
    );
    (subject, body)
}

fn render(entries: &[Reconciled], dry_run: bool) -> String {
    use std::fmt::Write;
    if entries.is_empty() {
        return "Every declared `used-by` agrees with the citations.\n".to_string();
    }
    let mut s = String::new();
    for e in entries {
        let _ = writeln!(s, "{}", e.entry);
        for a in &e.drift.citing_not_claimed {
            let _ = writeln!(s, "  + {a}");
        }
        for r in &e.drift.claimed_not_citing {
            let _ = writeln!(s, "  - {r}");
        }
        match &e.commit {
            Some(c) => {
                let _ = writeln!(s, "  {} {}", c.sha, c.subject);
            }
            None if dry_run => {
                let _ = writeln!(s, "  would rewrite {} entries in used-by", e.used_by.len());
            }
            None => {}
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_citation_is_rewritten_in_the_form_entries_are_written_in() {
        assert_eq!(
            as_declared(".yidam/corpus/gage/canyon-outlet.yml"),
            "../corpus/gage/canyon-outlet.yml"
        );
        // Already relative, or from somewhere unexpected: left recognisable rather than
        // mangled into a path that resolves nowhere.
        assert_eq!(as_declared("corpus/gage/x.yml"), "../corpus/gage/x.yml");
    }

    #[test]
    fn the_subject_is_operational_and_the_body_names_both_directions() {
        let drift = UsedByDrift {
            claimed_not_citing: vec!["gone.yml".into()],
            citing_not_claimed: vec!["new.yml".into()],
        };
        let (subject, body) = message("usgs-nwis", &drift);
        assert_eq!(
            yidam_core::git::classify_commit("", &subject).kind,
            yidam_core::git::CommitKind::Operational,
            "{subject}"
        );
        assert!(body.contains("added 1"), "{body}");
        assert!(body.contains("new.yml"), "{body}");
        assert!(body.contains("removed 1"), "{body}");
        assert!(body.contains("gone.yml"), "{body}");
    }

    /// The repair has to leave the gate green, which means it has to agree with the gate
    /// about what drifted. This is that agreement, asserted end to end on the two functions
    /// that would otherwise be free to diverge.
    #[test]
    fn reconciling_a_drifted_list_leaves_no_drift_behind() {
        let entry = "---\nname: s\nused-by:\n  - ../corpus/gage/gone.yml\n---\n\n# S\n";
        let citing = vec![".yidam/corpus/gage/canyon-outlet.yml".to_string()];
        let declared = parse_frontmatter(entry).used_by.unwrap();

        let before = used_by_drift(&declared, &citing).unwrap();
        assert_eq!(before.claimed_not_citing, vec!["gone.yml"]);
        assert_eq!(before.citing_not_claimed, vec!["canyon-outlet.yml"]);

        let want: Vec<String> = citing.iter().map(|c| as_declared(c)).collect();
        let updated = record::set_used_by(entry, &want).unwrap().unwrap();
        let after = parse_frontmatter(&updated).used_by.unwrap();
        let drift = used_by_drift(&after, &citing).unwrap();
        assert!(drift.claimed_not_citing.is_empty(), "{drift:?}");
        assert!(drift.citing_not_claimed.is_empty(), "{drift:?}");
    }
}
