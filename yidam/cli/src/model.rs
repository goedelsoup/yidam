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
            "[warn] genesis commit {first_line:?} does not match expected format \
             \"chore: genesis \u{2014} <name>\" — using directory name {fallback:?} as domain"
        );
        fallback
    };
    let generated_at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    // Prefix "../corpus/": this rendering goes into the export bundle at `index/corpus.md`,
    // where the instances sit at `corpus/<class>/<file>` (see `cmd::bundle`). One directory
    // up and over. It was root-relative here too, so the bundle's index was dead the same
    // way the README's was, one layer further from anyone who would notice.
    let corpus_index = render_corpus_index("../corpus/", &corpus_dir);
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

/// One row of the vector index, decoded from `index/corpus.arrow`.
#[cfg(any(feature = "index", feature = "export-sqlite"))]
pub struct VectorRow {
    pub path: String,
    pub class: String,
    pub label: String,
    pub text: String,
    pub vector: Vec<f32>,
}

/// Decode the Arrow IPC index into rows — shared by the MCP server's
/// `retrieve` and the sqlite export, so vectors are read one way everywhere.
#[cfg(any(feature = "index", feature = "export-sqlite"))]
pub fn index_rows(index: &IndexData) -> Result<Vec<VectorRow>> {
    use anyhow::Context;
    use arrow_array::cast::AsArray;

    let reader =
        arrow_ipc::reader::FileReader::try_new(std::io::Cursor::new(&index.arrow_ipc), None)
            .context("reading index/corpus.arrow")?;
    let mut rows = Vec::new();
    for batch in reader {
        let batch = batch?;
        let col = |name: &str| -> Result<&arrow_array::StringArray> {
            batch
                .column_by_name(name)
                .and_then(|c| c.as_any().downcast_ref())
                .with_context(|| format!("index column {name:?} missing or not a string"))
        };
        let paths = col("path")?;
        let classes = col("class")?;
        let labels = col("label")?;
        let texts = col("text")?;
        let vectors = batch
            .column_by_name("vector")
            .map(|c| c.as_fixed_size_list())
            .context("index column \"vector\" missing")?;

        for i in 0..batch.num_rows() {
            let values = vectors.value(i);
            let floats = values.as_primitive::<arrow_array::types::Float32Type>();
            rows.push(VectorRow {
                path: paths.value(i).to_string(),
                class: classes.value(i).to_string(),
                label: labels.value(i).to_string(),
                text: texts.value(i).to_string(),
                vector: floats.values().to_vec(),
            });
        }
    }
    Ok(rows)
}

/// One corpus instance parsed into graph form — the shared view consumed by
/// the MCP server and the graph-shaped exporters (GraphML, RDF).
#[derive(Debug)]
pub struct NodeView {
    /// `<class>/<name>` — the stable node id.
    pub id: String,
    pub class: String,
    pub label: String,
    pub description: String,
    /// Raw YAML source of the instance file.
    pub content: String,
    /// Outgoing edges: (target node id, relationship).
    pub links: Vec<(String, String)>,
    /// Which corpus this node came from: `None` for this repository, `Some(pkg)` for an
    /// installed dependency.
    ///
    /// Kept beside the id rather than folded into it, so every existing consumer that
    /// parses `<class>/<name>` keeps working and a caller must *choose* to look at foreign
    /// nodes. [`NodeView::qualified_id`] is the disambiguated form for display.
    pub origin: Option<String>,
}

impl NodeView {
    /// The id to show a reader: `pkg::class/name` for a dependency, `class/name` for a
    /// local node.
    ///
    /// Two corpora may legitimately hold the same `class/name`; without this a retrieval
    /// result naming one of them is not an answer to "which node is this".
    pub fn qualified_id(&self) -> String {
        match &self.origin {
            Some(pkg) => format!("{pkg}::{}", self.id),
            None => self.id.clone(),
        }
    }

    /// Whether this node belongs to this repository rather than to a dependency.
    pub fn is_local(&self) -> bool {
        self.origin.is_none()
    }
}

/// Resolve a link target (a path relative to the source instance's class
/// directory, e.g. `../other-class/thing.yml`) to a node id
/// (`other-class/thing`). Targets that escape the corpus directory are
/// returned verbatim.
pub(crate) fn resolve_link_target(source_class: &str, target: &str) -> String {
    let mut parts: Vec<&str> = vec![source_class];
    for comp in target.split('/') {
        match comp {
            "." | "" => {}
            ".." => {
                if parts.pop().is_none() {
                    return target.to_string();
                }
            }
            other => parts.push(other),
        }
    }
    let joined = parts.join("/");
    joined
        .strip_suffix(".yml")
        .map(str::to_string)
        .unwrap_or(joined)
}

pub(crate) fn file_stem(filename: &str) -> String {
    Path::new(filename)
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| filename.to_string())
}

/// Parse every corpus instance into a [`NodeView`], resolving link targets
/// to node ids. Instances that fail YAML parsing degrade to empty fields
/// rather than being dropped — the node exists even if malformed.
pub fn corpus_nodes(model: &DomainModel) -> Vec<NodeView> {
    model
        .instances
        .iter()
        .map(|inst| {
            let content = String::from_utf8_lossy(&inst.content).into_owned();
            let parsed: crate::parse::CorpusInstance =
                serde_yaml::from_str(&content).unwrap_or_default();
            let name = file_stem(&inst.filename);
            let links = parsed
                .links
                .unwrap_or_default()
                .iter()
                .filter_map(|l| {
                    l.target.as_ref().map(|t| {
                        (
                            resolve_link_target(&inst.class, t),
                            l.relationship.clone().unwrap_or_else(|| "link".to_string()),
                        )
                    })
                })
                .collect();
            NodeView {
                id: format!("{}/{}", inst.class, name),
                class: inst.class.clone(),
                label: parsed.label.unwrap_or_default(),
                description: parsed.description.unwrap_or_default(),
                content,
                links,
                origin: None,
            }
        })
        .collect()
}

// ── installed dependencies ────────────────────────────────────────────────────
//
// `yidam tonpa` fetched a bundle, hashed it, locked it, and unpacked a whole corpus into
// `.yidam/tonpa/<pkg>/` — and until this existed nothing in the CLI ever read it. The
// package manager was complete and its product had no consumer, which meant a repository
// could declare a dependency, verify it, commit the lock, and observe no difference in
// anything the tool could tell it.
//
// What a dependency is allowed to be, here, is deliberately narrow: **readable and
// searchable, never an edge target.** An edge in this model is a claim, and the
// constitution governs who may assert one; a citation into a corpus with a different
// ontology, its own electors, and its own revision history is a different object from a
// local edge, and it wants its own argument rather than arriving as a side effect of a
// package manager. So foreign nodes carry their own links for display, no local edge ever
// resolves to one, and traversal does not cross the boundary.

/// Names of the dependencies installed under `.yidam/tonpa/`, sorted.
///
/// The lock file is not consulted: this answers "what is on disk", which is what a reader
/// can actually be shown. A declared-but-uninstalled dependency is `tonpa status`'s
/// question, not this one.
pub fn dependency_names(root: &Path) -> Vec<String> {
    crate::deps::resolved(root)
        .into_iter()
        .map(|d| d.name)
        .collect()
}

/// Every corpus instance in one installed dependency, as [`NodeView`]s tagged with its name.
///
/// The layout mirrors this repository's own — `corpus/<class>/<name>.yml` — because a bundle
/// is a corpus. That is why the parsing below is the same parsing, and why a change to how
/// instances are read cannot apply to one and not the other.
pub fn dependency_nodes(root: &Path, package: &str) -> Vec<NodeView> {
    let Some(dep) = crate::deps::resolved(root)
        .into_iter()
        .find(|d| d.name == package)
    else {
        return Vec::new();
    };
    let Ok(classes) = std::fs::read_dir(&dep.corpus_dir) else {
        return Vec::new();
    };

    let mut out = Vec::new();
    let mut class_dirs: Vec<_> = classes.filter_map(|e| e.ok()).collect();
    class_dirs.sort_by_key(|e| e.file_name());

    for class_entry in class_dirs {
        if !class_entry.path().is_dir() {
            continue; // `<name>.ont.yml` schema files sit beside the class directories
        }
        let class = class_entry.file_name().to_string_lossy().into_owned();
        let Ok(files) = std::fs::read_dir(class_entry.path()) else {
            continue;
        };
        let mut files: Vec<_> = files.filter_map(|e| e.ok()).collect();
        files.sort_by_key(|e| e.file_name());

        for f in files {
            let path = f.path();
            if path.extension().and_then(|e| e.to_str()) != Some("yml") {
                continue;
            }
            let Ok(content) = std::fs::read_to_string(&path) else {
                continue;
            };
            let parsed: crate::parse::CorpusInstance =
                serde_yaml::from_str(&content).unwrap_or_default();
            let name = file_stem(&f.file_name().to_string_lossy());
            let links = parsed
                .links
                .unwrap_or_default()
                .iter()
                .filter_map(|l| {
                    l.target.as_ref().map(|t| {
                        (
                            resolve_link_target(&class, t),
                            l.relationship.clone().unwrap_or_else(|| "link".to_string()),
                        )
                    })
                })
                .collect();
            out.push(NodeView {
                id: format!("{class}/{name}"),
                class: class.clone(),
                label: parsed.label.unwrap_or_default(),
                description: parsed.description.unwrap_or_default(),
                content,
                links,
                origin: Some(package.to_string()),
            });
        }
    }
    out
}

/// Every node from every installed dependency, in dependency-name order.
pub fn all_dependency_nodes(root: &Path) -> Vec<NodeView> {
    dependency_names(root)
        .iter()
        .flat_map(|pkg| dependency_nodes(root, pkg))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn resolve_link_targets_relative_paths() {
        assert_eq!(resolve_link_target("reach", "alpha.yml"), "reach/alpha");
        assert_eq!(
            resolve_link_target("reach", "../concept/formation.yml"),
            "concept/formation"
        );
        assert_eq!(resolve_link_target("reach", "./beta.yml"), "reach/beta");
        // escapes the corpus dir — returned verbatim
        assert_eq!(
            resolve_link_target("reach", "../../skills/x.md"),
            "../../skills/x.md"
        );
    }

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
