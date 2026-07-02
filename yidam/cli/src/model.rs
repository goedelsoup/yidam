use anyhow::Result;
use std::path::Path;

use crate::cmd::corpus::{render_corpus_index, render_graph_check};
use crate::cmd::decisions::render_decisions_log;
use crate::cmd::registry::render_skills_index;
use crate::embed_config::{EmbedConfig, EMBED_CONFIG_FILENAME};
use crate::git::{genesis_date, genesis_message, head_commit_short};
use crate::paths::{yidam_corpus_dir, yidam_decisions_dir, yidam_index_dir, yidam_skills_dir};
use crate::walk::{walk_corpus_instances, walk_decision_files, walk_md_files, walk_ont_files};

/// A single `.ont.yml` class schema file from the corpus directory.
pub struct OntClass {
    pub filename: String,
    pub content: Vec<u8>,
}

/// A single corpus instance `.yml` file, including its parent class name.
pub struct InstanceFile {
    pub class: String,
    pub filename: String,
    pub content: Vec<u8>,
}

/// A single skill `.md` file from the skills directory.
pub struct SkillFile {
    pub filename: String,
    pub content: Vec<u8>,
}

/// A single decision `.yml` file from the decisions directory.
pub struct DecisionFile {
    pub filename: String,
    pub content: Vec<u8>,
}

/// Vector index artefacts, present only when `yidam index-build` has run.
pub struct IndexData {
    /// Raw Arrow IPC bytes — written verbatim to `index/corpus.arrow` in the bundle.
    pub arrow_ipc: Vec<u8>,
    /// Raw `meta.json` bytes — written verbatim to `index/meta.json` in the bundle.
    pub meta_raw: Vec<u8>,
    /// Parsed metadata — used to extract field values (e.g. `model_name`).
    pub meta: serde_json::Value,
    /// Embedding reproducibility contract — written to `index/embed.config.json`
    /// in the bundle. `None` for indexes built before the contract existed;
    /// rebuild with `yidam index-build` to emit it.
    pub embed_config: Option<EmbedConfig>,
}

/// Immutable provenance fields stamped at load time.
pub struct Provenance {
    pub commit: String,
    pub genesis: String,
    pub domain: String,
    pub generated_at: u64,
}

/// Pre-rendered Markdown views, computed once by [`load_domain_model`] so that
/// export renderers can be pure functions with no disk reads.
pub struct RenderedViews {
    pub corpus_index: String,
    pub graph_check: String,
    pub decisions_log: String,
    pub skills_index: String,
}

/// All domain data assembled from disk in one pass.
///
/// Every export renderer receives a `&DomainModel` and returns bytes — no
/// further disk I/O is required.
pub struct DomainModel {
    pub classes: Vec<OntClass>,
    pub instances: Vec<InstanceFile>,
    pub skills: Vec<SkillFile>,
    pub decisions: Vec<DecisionFile>,
    pub index: Option<IndexData>,
    pub provenance: Provenance,
    pub rendered: RenderedViews,
}

/// Load all domain data from `root` into a [`DomainModel`].
///
/// This is the single disk-access point for the export pipeline. Everything
/// that can be derived from the loaded data (rendered views, archive bytes)
/// is computed downstream without touching the filesystem.
pub fn load_domain_model(root: &Path) -> Result<DomainModel> {
    let corpus_dir = yidam_corpus_dir(root);
    let decisions_dir = yidam_decisions_dir(root);
    let skills_dir = yidam_skills_dir(root);
    let index_dir = yidam_index_dir(root);

    let mut classes = Vec::new();
    for path in walk_ont_files(&corpus_dir) {
        let filename = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();
        classes.push(OntClass {
            filename,
            content: std::fs::read(&path)?,
        });
    }

    let mut instances = Vec::new();
    for path in walk_corpus_instances(&corpus_dir) {
        let class = path
            .parent()
            .and_then(|d| d.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();
        let filename = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();
        instances.push(InstanceFile {
            class,
            filename,
            content: std::fs::read(&path)?,
        });
    }

    let mut skills = Vec::new();
    for path in walk_md_files(&skills_dir) {
        let filename = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();
        skills.push(SkillFile {
            filename,
            content: std::fs::read(&path)?,
        });
    }

    let mut decisions = Vec::new();
    for path in walk_decision_files(&decisions_dir) {
        let filename = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();
        decisions.push(DecisionFile {
            filename,
            content: std::fs::read(&path)?,
        });
    }

    let arrow_path = index_dir.join("corpus.arrow");
    let meta_path = index_dir.join("meta.json");
    let index = if arrow_path.exists() && meta_path.exists() {
        let arrow_ipc = std::fs::read(&arrow_path)?;
        let meta_raw = std::fs::read(&meta_path)?;
        let meta = serde_json::from_slice(&meta_raw).unwrap_or(serde_json::Value::Null);
        let embed_config_path = index_dir.join(EMBED_CONFIG_FILENAME);
        let embed_config = if embed_config_path.exists() {
            match serde_json::from_slice(&std::fs::read(&embed_config_path)?) {
                Ok(cfg) => Some(cfg),
                Err(e) => {
                    eprintln!(
                        "[warn] {} is not a valid embed config ({e}) — \
                         rebuild with `yidam index-build`",
                        embed_config_path.display()
                    );
                    None
                }
            }
        } else {
            eprintln!(
                "[warn] index has no {EMBED_CONFIG_FILENAME} — \
                 rebuild with `yidam index-build` to pin the embedding contract"
            );
            None
        };
        Some(IndexData {
            arrow_ipc,
            meta_raw,
            meta,
            embed_config,
        })
    } else {
        None
    };

    let commit = head_commit_short(root);
    let genesis = genesis_date(root);
    let genesis_msg = genesis_message(root);
    let first_line = genesis_msg.lines().next().unwrap_or("").trim();
    let domain = if let Some(name) = first_line
        .strip_prefix("chore: genesis \u{2014} ")
        .filter(|s| !s.is_empty())
    {
        name.to_string()
    } else {
        let fallback = root
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();
        eprintln!(
            "[warn] genesis commit {:?} does not match expected format \
             \"chore: genesis \u{2014} <name>\" — using directory name {:?} as domain",
            first_line, fallback
        );
        fallback
    };
    let generated_at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let corpus_index = render_corpus_index(root, &corpus_dir);
    let (graph_check, _) = render_graph_check(root, &corpus_dir);
    let decisions_log = render_decisions_log(&decisions_dir);
    let skills_index = render_skills_index(&skills_dir);

    Ok(DomainModel {
        classes,
        instances,
        skills,
        decisions,
        index,
        provenance: Provenance {
            commit,
            genesis,
            domain,
            generated_at,
        },
        rendered: RenderedViews {
            corpus_index,
            graph_check,
            decisions_log,
            skills_index,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn init_git_repo(dir: &Path) {
        for args in [
            vec!["init", "-q"],
            vec!["config", "user.email", "test@test.com"],
            vec!["config", "user.name", "Test"],
        ] {
            std::process::Command::new("git")
                .args(&args)
                .current_dir(dir)
                .status()
                .unwrap();
        }
    }

    #[test]
    fn load_domain_model_round_trip() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        init_git_repo(root);

        let corpus = root.join(".yidam").join("corpus");
        let class_dir = corpus.join("reach");
        fs::create_dir_all(&class_dir).unwrap();
        fs::write(corpus.join("reach.ont.yml"), "class: reach\n").unwrap();
        fs::write(class_dir.join("alpha.yml"), "class: reach\nlabel: Alpha\n").unwrap();

        let skills_dir = root.join(".yidam").join("skills");
        fs::create_dir_all(&skills_dir).unwrap();
        fs::write(skills_dir.join("my-skill.md"), "---\nname: my-skill\n---\n").unwrap();

        let model = load_domain_model(root).unwrap();

        assert_eq!(model.classes.len(), 1);
        assert_eq!(model.classes[0].filename, "reach.ont.yml");
        assert_eq!(model.instances.len(), 1);
        assert_eq!(model.instances[0].class, "reach");
        assert_eq!(model.instances[0].filename, "alpha.yml");
        assert_eq!(model.skills.len(), 1);
        assert_eq!(model.skills[0].filename, "my-skill.md");
        assert_eq!(model.decisions.len(), 0);
        assert!(model.index.is_none());
        assert!(model.rendered.corpus_index.contains("alpha.yml"));
    }

    #[test]
    fn load_domain_model_reads_embed_config() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        init_git_repo(root);

        let index_dir = root.join(".yidam").join("index");
        fs::create_dir_all(&index_dir).unwrap();
        fs::write(index_dir.join("corpus.arrow"), b"stub").unwrap();
        fs::write(index_dir.join("meta.json"), "{}").unwrap();
        let cfg = EmbedConfig::for_fastembed_model(
            "Xenova/all-MiniLM-L6-v2",
            384,
            "onnx/model_quantized.onnx",
            "AllMiniLML6V2Q",
        );
        fs::write(
            index_dir.join(EMBED_CONFIG_FILENAME),
            serde_json::to_string_pretty(&cfg).unwrap(),
        )
        .unwrap();

        let model = load_domain_model(root).unwrap();
        let index = model.index.expect("index should load");
        assert_eq!(index.embed_config, Some(cfg));
    }

    #[test]
    fn load_domain_model_tolerates_missing_embed_config() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        init_git_repo(root);

        let index_dir = root.join(".yidam").join("index");
        fs::create_dir_all(&index_dir).unwrap();
        fs::write(index_dir.join("corpus.arrow"), b"stub").unwrap();
        fs::write(index_dir.join("meta.json"), "{}").unwrap();

        let model = load_domain_model(root).unwrap();
        let index = model.index.expect("index should load");
        assert_eq!(index.embed_config, None);
    }
}
