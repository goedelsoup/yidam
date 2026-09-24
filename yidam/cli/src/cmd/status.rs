use anyhow::Result;
use std::path::Path;

use crate::git::{genesis_date, phase_tally};
use crate::paths::{repo_root, yidam_catalog_dir, yidam_corpus_dir, yidam_index_dir};
use crate::regen::update_file_regen;
use crate::walk::{walk_corpus_instances, walk_md_files};

#[derive(serde::Serialize)]
struct StatusReport {
    nodes: usize,
    open_questions: usize,
    catalog_entries: usize,
    /// Source records past the TTL the corpus declared. Zero when none is declared.
    catalog_expired: usize,
    claims_verified: usize,
    claims_inference: usize,
    claims_open: usize,
    /// The same three over the corpus's **edges** — every link's `claim_tag` (#857).
    ///
    /// Beside the node figures and never folded into them. `ClaimCounts` is *measured against
    /// supposed* over one node's text; an edge is in no node's text and belongs to two nodes
    /// at once, so one merged number would answer about a denominator nobody chose — and
    /// would move every derived repository's headline figures on upgrade. A consumer that
    /// wants the corpus-wide total adds the two.
    edge_claims_verified: usize,
    edge_claims_inference: usize,
    edge_claims_open: usize,
    index_present: bool,
    /// Bounded work not yet on the baseline. Before `phase_tally` this counted every
    /// `ma/*` and `rigpa/*` ref and no `phase/*` ref at all — see [`crate::git::RefKind`].
    ///
    /// **This and the three counts below are reported here and nowhere else.** They used to
    /// render into the README block as well, and could not: see [`status`].
    active_phases: usize,
    /// Merged `phase/*` and `rigpa/*` refs still present. PHASES.md prescribes deleting them.
    settled_phases: usize,
    /// Refs whose work reached the baseline by a merge that rewrote its commits, so they are not
    /// ancestors of it and never will be. A new field rather than a fold into `settled_phases`:
    /// these were counted as `active_phases` until #773, and a consumer that saw that number
    /// fall needs to be able to see where it went.
    rewritten_phases: usize,
    /// Standing `ma/*` elector positions, which are neither active work nor drift.
    positions: usize,
    genesis: String,
}

/// The corpus in one line, into the README block — and the same numbers plus the phase tally
/// as JSON.
///
/// **The two renderings do not carry the same fields, on purpose.** `--format json` is a report:
/// it answers about the repository as it stands, and a phase count is one of the more useful
/// things it can say. The markdown block is *committed*, and `yidam regen --check` gates it, so
/// it is held to a stricter rule:
///
/// > A REGEN block held by `--check` must be a function of what every checkout of that commit
/// > has in common.
///
/// A phase count is not. It reads remote-tracking refs, and which of those a checkout holds is a
/// property of how it was fetched, not of the commit — so for one commit there was no value that
/// was green both in a clone that had fetched and in a default CI checkout, and `--check` was
/// unwinnable rather than wrong (#647). Worse, it moved with no commit at all: pushing a phase
/// branch reddened the gate on every open pull request, on a file none of them touched.
///
/// `yidam phases` is where the live view belongs — being current is the point there rather than a
/// hazard — and `status --format json` keeps all four counts for anything reading them.
///
/// **Index freshness broke the same rule and was not noticed for two releases (#895).** The cell
/// read `index present` or `index not initialized` from a stat of `.yidam/index/`, which is a
/// build artifact of `--features index` and is in no commit. So the gate's verdict was a fact
/// about the machine: measured on one derived corpus, same tree and same commit, moving the
/// index directory aside took `regen --check` from exit 1 to exit 0. CI never has an index and
/// so never saw the failure, and the contributors who did were exactly the ones who had adopted
/// the feature the repository invited them to adopt.
///
/// Both horns of the repair are closed rather than one. Git-ignoring the index — what the
/// reporting corpus did by hand, and what the scaffolded `.gitignore` now does — makes *absent*
/// the only committed answer, but a block asserting absence is still wrong on the machine
/// holding one. So the assertion leaves the block: `index_present` stays in `--format json`,
/// [`crate::cmd::index_status`] writes a block that does not depend on the tree, and
/// `yidam doctor` and `yidam due` answer freshness where being current is the point.
///
/// The other half of that contract is depth, and it is not answered here: `genesis` reads the
/// repository's first commit, which a shallow clone does not have. [`crate::git::is_shallow`] is
/// why that now reports `unknown` instead of a date it invented, and
/// [`crate::cmd::regen`] is where the gate refuses rather than gating a value it cannot compute.
pub fn status(format: crate::report::Format) -> Result<()> {
    let root = repo_root()?;
    let corpus = yidam_corpus_dir(&root);
    let catalog = yidam_catalog_dir(&root);

    let instances = walk_corpus_instances(&corpus);
    let node_count = instances.len();
    let fields = crate::claims::ClaimFields::load(&corpus);

    let open_count = instances
        .iter()
        .filter(|p| {
            let text = std::fs::read_to_string(p).unwrap_or_default();
            let inst = crate::parse::parse_instance(&text);
            let label = inst.label.unwrap_or_default();
            let class = inst.class.unwrap_or_default();
            crate::claims::has_open_claim(&label, &text, fields.for_class(&class))
        })
        .count();

    // How much of the corpus is measured against how much is supposed. This is the
    // template's most-adopted convention by a wide margin and nothing reported on it.
    let mut claims = crate::claims::ClaimCounts::default();
    let mut edge_claims = crate::claims::ClaimCounts::default();
    for p in &instances {
        let text = std::fs::read_to_string(p).unwrap_or_default();
        let inst = crate::parse::parse_instance(&text);
        claims.add(crate::claims::count_in_node(
            &text,
            fields.for_class(inst.class.as_deref().unwrap_or_default()),
        ));
        edge_claims.add(crate::claims::count_in_edges(&text));
    }

    let catalog_paths = walk_md_files(&catalog);
    let catalog_entries = catalog_paths.len();
    // Expired source records, shown only when there are any. A corpus that declared no TTL
    // is asking nothing here, and a permanent `· 0 expired` is a number nobody can act on.
    let catalog_expired = crate::cmd::lint::ttl::ages(
        &crate::corpus::load_sources(&root, &catalog_paths, &Default::default()),
        &crate::cmd::lint::ttl::committed_dates(&root, &catalog),
        crate::config::load_yidam_config(&root)
            .map(|c| c.catalog.ttl_days)
            .unwrap_or_default(),
        crate::cmd::export::unix_to_iso(crate::dates::today_days() as u64 * 86_400)
            .split('T')
            .next()
            .unwrap_or_default(),
    )
    .iter()
    .filter(|a| a.overdue_days().is_some())
    .count();

    let genesis = genesis_date(&root);

    let sources_cell = match catalog_expired {
        0 => format!("{catalog_entries} sources"),
        n => format!("{catalog_entries} sources · {n} expired"),
    };
    // Rendered only where the corpus tags edges. This line is committed into the README and
    // `yidam regen --check` gates it, so a permanent `edges —` would redden that gate in every
    // derived repository on upgrade over a distinction none of them had made. Where the corpus
    // *has* made it, the figure is the point.
    let edges_cell = match edge_claims.total() {
        0 => String::new(),
        _ => format!("edges {} · ", edge_claims.cell()),
    };
    let content = format!(
        "**{node_count} nodes** · {open_count} open · {sources_cell} · \
         claims {} · {edges_cell}genesis {genesis}",
        claims.cell()
    );

    if format.is_json() {
        // Read only here. The refs are not merely unrendered above — they are not *looked at*,
        // which is the property [`the_gate_survives_a_phase_ref_appearing`] asserts. A tally
        // computed and discarded would still make the text path's behaviour depend on which
        // refs a checkout happens to hold, the moment anyone renders it again.
        let phases = phase_tally(&root);
        // Read only here, for the same reason as `phases` above: a stat of `.yidam/index/`
        // answers about one machine, and the block is committed. See [`status`].
        let index_present = yidam_index_dir(&root).exists();
        return crate::report::emit(
            &root,
            StatusReport {
                nodes: node_count,
                open_questions: open_count,
                catalog_entries,
                catalog_expired,
                claims_verified: claims.verified,
                claims_inference: claims.inference,
                claims_open: claims.open,
                edge_claims_verified: edge_claims.verified,
                edge_claims_inference: edge_claims.inference,
                edge_claims_open: edge_claims.open,
                index_present,
                active_phases: phases.active,
                settled_phases: phases.settled,
                rewritten_phases: phases.rewritten,
                positions: phases.positions,
                genesis: genesis.clone(),
            },
        );
    }

    crate::regen::emit(&content);
    update_file_regen(&root.join("README.md"), "yidam status", &content)
}

/// What `index-status` found, before it is turned into prose.
///
/// Deliberately carries `built_at` as the raw timestamp and no age string: an age is a
/// function of when you ask, and a report field that changes every minute cannot be a
/// golden. Humanizing it is the client's job — an affordance, not a verdict.
#[derive(Debug, serde::Serialize)]
pub struct IndexStatusReport {
    /// Whether `.yidam/index/` exists at all.
    pub index_present: bool,
    /// Whether it carries a readable `meta.json`. An index without one is present and
    /// undescribable, which is a different state from absent.
    pub meta_present: bool,
    pub built_at: Option<u64>,
    /// Civil date of `built_at`, so a reader does not have to convert one.
    pub built: Option<String>,
    pub model: Option<String>,
    pub embedding_dim: Option<u64>,
    pub node_count: Option<u64>,
    /// Corpus files modified since the build. This is the freshness verdict.
    pub stale_nodes: usize,
}

pub(crate) fn index_status_data(root: &Path) -> IndexStatusReport {
    let index_dir = yidam_index_dir(root);
    let absent = IndexStatusReport {
        index_present: index_dir.exists(),
        meta_present: false,
        built_at: None,
        built: None,
        model: None,
        embedding_dim: None,
        node_count: None,
        stale_nodes: 0,
    };
    if !index_dir.exists() {
        return absent;
    }
    let Ok(meta_str) = std::fs::read_to_string(index_dir.join("meta.json")) else {
        return absent;
    };
    let meta: serde_json::Value =
        serde_json::from_str(&meta_str).unwrap_or(serde_json::Value::Null);
    let generated_at = meta["generated_at"].as_u64().unwrap_or(0);
    IndexStatusReport {
        index_present: true,
        meta_present: true,
        built_at: Some(generated_at),
        built: Some(unix_to_date_str(generated_at)),
        model: Some(meta["model_name"].as_str().unwrap_or("unknown").to_string()),
        embedding_dim: Some(meta["embedding_dim"].as_u64().unwrap_or(0)),
        node_count: Some(meta["node_count"].as_u64().unwrap_or(0)),
        stale_nodes: count_stale_corpus_files(&yidam_corpus_dir(root), generated_at),
    }
}

/// The prose, rendered *from* the report, so the two cannot say different things.
pub(crate) fn render_index_status(r: &IndexStatusReport, now: u64) -> String {
    if !r.index_present {
        return "_Index not initialized. Run `yidam index-build` to build._".to_string();
    }
    if !r.meta_present {
        return "Index present — no metadata file found.".to_string();
    }
    let generated_at = r.built_at.unwrap_or(0);
    let date_str = r.built.clone().unwrap_or_default();
    let age_str = humanize_age(now.saturating_sub(generated_at));
    if r.stale_nodes > 0 {
        let stale = r.stale_nodes;
        format!(
            "Vector index: built {date_str} ({age_str})\n\
             Stale nodes:  {stale} corpus instance(s) added or modified since last build\n\
             Action:       run `yidam index-build` to refresh"
        )
    } else {
        format!(
            "Vector index: built {date_str} ({age_str}), up-to-date\n\
             Model:        {} ({} dims)\n\
             Nodes:        {}",
            r.model.clone().unwrap_or_default(),
            r.embedding_dim.unwrap_or(0),
            r.node_count.unwrap_or(0)
        )
    }
}

/// What the committed block holds — and it is not [`render_index_status`].
///
/// Every field that render carries is a fact about one working tree: whether the directory
/// exists, when the binary that built it ran, how many corpus files have been touched since.
/// None of it is in any commit, and `yidam regen --check` gates this block. Writing the live
/// verdict here made the gate's exit code a property of the machine running it (#895) — green
/// in CI, which never has an index, and red on a contributor who had built one.
///
/// So the block says the one thing about the index that *is* true of every checkout: that this
/// is not where the answer lives. A constant rather than a removal, because a removal leaves a
/// block behind in every corpus that already has one — refreshed by nothing and checked by
/// nothing, which is the state #831 exists to have ended. `yidam regen` rewrites this one in
/// place, so a corpus upgrades by running the command the gate already prescribes.
///
/// The live verdict is not lost. It goes to stdout below, to `--format json`, and to the two
/// commands whose whole business is the current machine — `yidam doctor`'s `index` check and
/// `yidam due`'s index row, both of which call [`index_status_data`] rather than re-deriving it.
const COMMITTED_INDEX_BLOCK: &str =
    "_The vector index is a build artifact and is in no commit — whether one exists, and how \
     stale it is, is a fact about a working tree rather than about this revision. Run \
     `yidam index-status` for this machine's answer, or `yidam doctor` for it alongside the \
     rest._";

pub fn index_status(format: crate::report::Format) -> Result<()> {
    let root = repo_root()?;
    let data = index_status_data(&root);
    if format.is_json() {
        return crate::report::emit(&root, data);
    }

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_else(|_| data.built_at.unwrap_or(0));
    // Live to the terminal, where being current is the point. `emit` is suppressed under
    // `--check`, so the measured verdict reaches no gated artifact.
    crate::regen::emit(&render_index_status(&data, now));

    let corpus = yidam_corpus_dir(&root);
    update_file_regen(
        &corpus.join("README.md"),
        "yidam index-status",
        COMMITTED_INDEX_BLOCK,
    )?;
    update_file_regen(
        &root.join("crates").join("README.md"),
        "yidam index-status",
        COMMITTED_INDEX_BLOCK,
    )
}

fn count_stale_corpus_files(corpus: &Path, generated_at: u64) -> usize {
    if !corpus.exists() {
        return 0;
    }
    walkdir::WalkDir::new(corpus)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| {
            e.metadata()
                .ok()
                .and_then(|m| m.modified().ok())
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs() > generated_at)
                .unwrap_or(false)
        })
        .count()
}

// Civil (Gregorian) date string from a Unix timestamp, via `crate::dates::civil_from_days`.
pub(crate) fn unix_to_date_str(ts: u64) -> String {
    let (y, m, d) = crate::dates::civil_from_days(ts as i64 / 86400);
    format!("{y:04}-{m:02}-{d:02}")
}

fn humanize_age(secs: u64) -> String {
    if secs < 3600 {
        format!("{} minute(s) ago", secs / 60)
    } else if secs < 86400 {
        format!("{} hour(s) ago", secs / 3600)
    } else {
        format!("{} day(s) ago", secs / 86400)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unix_epoch_is_1970_01_01() {
        assert_eq!(unix_to_date_str(0), "1970-01-01");
    }

    #[test]
    fn known_date_2026_07_01() {
        // 2026-07-01 00:00:00 UTC
        assert_eq!(unix_to_date_str(1782864000), "2026-07-01");
    }

    /// What this function computed while Hinnant's algorithm was inlined here, kept verbatim
    /// so the fold onto [`crate::dates::civil_from_days`] has something to be differential
    /// against. Note the `u64` intermediates and the `if z >= 0` era: this copy and the one
    /// in `cmd::export` were arranged differently, which is exactly the shape that is right
    /// in one copy and wrong in the other.
    fn unix_to_date_str_before_the_fold(ts: u64) -> String {
        let z = ts as i64 / 86400 + 719468;
        let era = if z >= 0 { z } else { z - 146096 } / 146097;
        let doe = (z - era * 146097) as u64;
        let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
        let y = yoe as i64 + era * 400;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let d = doy - (153 * mp + 2) / 5 + 1;
        let m = if mp < 10 { mp + 3 } else { mp - 9 };
        let y = if m <= 2 { y + 1 } else { y };
        format!("{y:04}-{m:02}-{d:02}")
    }

    /// 1970 → 2100 at 90,061-second steps: 45,525 timestamps, ~1.042 days apart. The stride
    /// is deliberately not a whole number of days, so it drifts through every month length,
    /// every leap year and the century rule rather than sitting on one phase of them.
    #[test]
    fn unix_to_date_str_is_unchanged_by_the_fold() {
        const STEP: u64 = 90_061;
        const SAMPLES: u64 = 45_525;
        for i in 0..SAMPLES {
            let ts = i * STEP;
            assert_eq!(
                unix_to_date_str(ts),
                unix_to_date_str_before_the_fold(ts),
                "diverged at unix {ts}"
            );
        }
    }

    /// The two call sites reached the day count by different routes — `(secs / 86400) as i64`
    /// in `cmd::export`, `ts as i64 / 86400` here — and the fold passes each through
    /// unchanged. Both parameters are `u64`, so the routes agree; this asserts that rather
    /// than leaving it on trust.
    #[test]
    fn the_two_day_count_routes_agree() {
        const STEP: u64 = 90_061;
        const SAMPLES: u64 = 45_525;
        for i in 0..SAMPLES {
            let ts = i * STEP;
            assert_eq!(
                (ts / 86400) as i64,
                ts as i64 / 86400,
                "diverged at unix {ts}"
            );
        }
    }
}
