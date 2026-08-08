use anyhow::Result;
use std::path::Path;

use crate::git::{active_phase_count, genesis_date};
use crate::parse::CorpusInstance;
use crate::paths::{repo_root, yidam_catalog_dir, yidam_corpus_dir, yidam_index_dir};
use crate::regen::update_file_regen;
use crate::walk::{walk_corpus_instances, walk_md_files};

use super::has_open_claim;

pub fn status() -> Result<()> {
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

    println!("{content}");
    update_file_regen(&root.join("README.md"), "yidam status", &content)
}

pub fn index_status() -> Result<()> {
    let root = repo_root()?;
    let index_dir = yidam_index_dir(&root);

    let content = if !index_dir.exists() {
        "_Index not initialized. Run `yidam index-build` to build._".to_string()
    } else {
        let meta_path = index_dir.join("meta.json");
        match std::fs::read_to_string(&meta_path) {
            Err(_) => "Index present — no metadata file found.".to_string(),
            Ok(meta_str) => {
                let meta: serde_json::Value =
                    serde_json::from_str(&meta_str).unwrap_or(serde_json::Value::Null);

                let generated_at = meta["generated_at"].as_u64().unwrap_or(0);
                let model_name = meta["model_name"].as_str().unwrap_or("unknown");
                let node_count = meta["node_count"].as_u64().unwrap_or(0);
                let dim = meta["embedding_dim"].as_u64().unwrap_or(0);

                let corpus = yidam_corpus_dir(&root);
                let stale = count_stale_corpus_files(&corpus, generated_at);

                let now_ts = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(generated_at);
                let age_secs = now_ts.saturating_sub(generated_at);
                let date_str = unix_to_date_str(generated_at);
                let age_str = humanize_age(age_secs);

                if stale > 0 {
                    format!(
                        "Vector index: built {date_str} ({age_str})\n\
                         Stale nodes:  {stale} corpus instance(s) added or modified since last build\n\
                         Action:       run `yidam index-build` to refresh"
                    )
                } else {
                    format!(
                        "Vector index: built {date_str} ({age_str}), up-to-date\n\
                         Model:        {model_name} ({dim} dims)\n\
                         Nodes:        {node_count}"
                    )
                }
            }
        }
    };

    println!("{content}");
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
