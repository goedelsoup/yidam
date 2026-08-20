use anyhow::Result;
use std::path::Path;

use crate::git::{active_phase_count, genesis_date};
use crate::parse::CorpusInstance;
use crate::paths::{repo_root, yidam_catalog_dir, yidam_corpus_dir, yidam_index_dir};
use crate::regen::update_file_regen;
use crate::walk::{walk_corpus_instances, walk_md_files};

use super::has_open_claim;

#[derive(serde::Serialize)]
struct StatusReport {
    nodes: usize,
    open_questions: usize,
    catalog_entries: usize,
    claims_verified: usize,
    claims_inference: usize,
    claims_open: usize,
    index_present: bool,
    active_phases: usize,
    genesis: String,
}

pub fn status(format: crate::report::Format) -> Result<()> {
    let root = repo_root()?;
    let corpus = yidam_corpus_dir(&root);
    let catalog = yidam_catalog_dir(&root);

    let instances = walk_corpus_instances(&corpus);
    let node_count = instances.len();

    let open_count = instances
        .iter()
        .filter(|p| {
            let text = std::fs::read_to_string(p).unwrap_or_default();
            let inst: CorpusInstance = serde_yaml::from_str(&text).unwrap_or_default();
            let label = inst.label.unwrap_or_default();
            label.starts_with('?') || has_open_claim(&text)
        })
        .count();

    // How much of the corpus is measured against how much is supposed. This is the
    // template's most-adopted convention by a wide margin and nothing reported on it.
    let mut claims = crate::claims::ClaimCounts::default();
    for p in &instances {
        claims.add(crate::claims::count_in_source(
            &std::fs::read_to_string(p).unwrap_or_default(),
        ));
    }

    let catalog_entries = walk_md_files(&catalog).len();

    let index_path = root.join(".yidam").join("index");
    let index_freshness = if index_path.exists() {
        "present"
    } else {
        "not initialized"
    };

    let phases = active_phase_count(&root);
    let genesis = genesis_date(&root);

    let content = format!(
        "**{node_count} nodes** · {open_count} open · {catalog_entries} sources · \
         claims {} · index {index_freshness} · {phases} active phase(s) · genesis {genesis}",
        claims.cell()
    );

    if format.is_json() {
        return crate::report::emit(
            &root,
            StatusReport {
                nodes: node_count,
                open_questions: open_count,
                catalog_entries,
                claims_verified: claims.verified,
                claims_inference: claims.inference,
                claims_open: claims.open,
                index_present: index_path.exists(),
                active_phases: phases,
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
    let content = render_index_status(&data, now);

    crate::regen::emit(&content);
    let corpus = yidam_corpus_dir(&root);
    update_file_regen(&corpus.join("README.md"), "yidam index-status", &content)?;
    update_file_regen(
        &root.join("crates").join("README.md"),
        "yidam index-status",
        &content,
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

// Civil (Gregorian) date string from a Unix timestamp — Hinnant's algorithm.
fn unix_to_date_str(ts: u64) -> String {
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
}
