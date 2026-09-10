use anyhow::Result;
use flate2::write::GzEncoder;
use flate2::Compression;
use std::io::Write;
use tar::Builder;

use crate::model::{load_domain_model, DomainModel};
use crate::paths::repo_root;

fn add_bytes<W: Write>(tar: &mut Builder<W>, path: &str, data: &[u8]) -> Result<()> {
    let mut header = tar::Header::new_gnu();
    header.set_size(data.len() as u64);
    header.set_mode(0o644);
    header.set_cksum();
    tar.append_data(&mut header, path, data)?;
    Ok(())
}

/// Serialize `model` into the `.yiz` bundle format and return the raw bytes.
///
/// This function is **pure**: it reads no files and has no side effects.
/// All disk I/O happens in [`load_domain_model`] before this is called.
///
/// # Bundle layout (version 1)
///
/// The output is a gzip-compressed tar archive:
///
/// ```text
/// manifest.yml          provenance + counts (see fields below)
/// corpus/<name>.ont.yml class schema files
/// corpus/<class>/<name>.yml instance files
/// skills/<name>.md      skill markdown files
/// decisions/<name>.yml  decision record files
/// index/corpus.md       rendered instance table
/// index/graph.md        graph integrity check report
/// index/decisions.md    rendered decisions table
/// index/skills.md       rendered skills table
/// index/corpus.arrow    Arrow IPC for the WASM web shell (if index present)
/// index/meta.json       vector index metadata (if index present)
/// index/embed.config.json embedding reproducibility contract (if index present)
/// ```
///
/// ## `manifest.yml` fields
///
/// | Field                | Type    | Description                                   |
/// |----------------------|---------|-----------------------------------------------|
/// | `bundle_version`     | string  | Format version ("1"). Bumped on breaking change|
/// | `commit`             | string  | Short SHA of HEAD at bundle time               |
/// | `genesis`            | string  | ISO date of the first commit                  |
/// | `genesis_hash`       | string? | Full SHA of the first commit, or `null`       |
/// | `generated_at`       | integer | Unix timestamp (seconds) of bundle creation   |
/// | `domain`             | string  | Domain name extracted from the genesis commit |
/// | `classes`            | integer | Count of `.ont.yml` schema files              |
/// | `instances`          | integer | Count of corpus instance files                |
/// | `skills`             | integer | Count of skill files                          |
/// | `decisions`          | integer | Count of decision files                       |
/// | `vector_index_model` | string? | Embedding model name, or `null` if no index   |
///
/// ## Versioning policy
///
/// `bundle_version` is incremented **only on breaking changes**:
/// removing a field, renaming a field, changing a field's type, or removing a
/// file from the archive layout. Adding new fields or new archive entries is
/// non-breaking. Consumers MUST ignore unknown fields and unknown archive paths.
///
/// `genesis_hash` is the first field added under that policy, and it is why the policy
/// needs a second half: a *new consumer* reading it meets bundles produced before it
/// existed, and those carry no such field. **Absence means unidentified — never zero, and
/// never a shared constant.** A constant would be the same string for every bundle that
/// predates the field, which is the conflation the value exists to prevent (RFC-0032 §2).
/// So the version stays `"1"` — nothing that could read a bundle before can fail to read
/// one now — and the burden is on the reader to have an answer for `None`.
pub(crate) fn render_bundle(model: &DomainModel) -> Result<Vec<u8>> {
    let p = &model.provenance;
    let index_line = match &model.index {
        Some(idx) => {
            let name = idx.meta["model_name"].as_str().unwrap_or("unknown");
            format!("vector_index_model: \"{name}\"\n")
        }
        None => "vector_index_model: null\n".to_string(),
    };

    // `genesis` is a date, and two corpora created on one day share it — so it can order
    // bundles and cannot identify them. This is the field that can: the first commit's full
    // hash, fixed at genesis and unrenameable. `null` rather than a placeholder for a tree
    // with no root commit; the versioning note above says why absence must stay absent.
    let genesis_hash_line = match &p.genesis_hash {
        Some(hash) => format!("genesis_hash: \"{hash}\"\n"),
        None => "genesis_hash: null\n".to_string(),
    };

    let manifest = format!(
        "bundle_version: \"1\"\n\
         commit: \"{}\"\n\
         genesis: \"{}\"\n\
         {genesis_hash_line}\
         generated_at: {}\n\
         domain: \"{}\"\n\
         classes: {}\n\
         instances: {}\n\
         skills: {}\n\
         decisions: {}\n\
         {index_line}",
        p.commit,
        p.genesis,
        p.generated_at,
        p.domain,
        model.classes.len(),
        model.instances.len(),
        model.skills.len(),
        model.decisions.len(),
    );

    let gz = GzEncoder::new(Vec::new(), Compression::default());
    let mut tar = Builder::new(gz);

    add_bytes(&mut tar, "manifest.yml", manifest.as_bytes())?;

    for cls in &model.classes {
        add_bytes(&mut tar, &format!("corpus/{}", cls.filename), &cls.content)?;
    }
    for inst in &model.instances {
        add_bytes(
            &mut tar,
            &format!("corpus/{}/{}", inst.class, inst.filename),
            &inst.content,
        )?;
    }
    for skill in &model.skills {
        add_bytes(
            &mut tar,
            &format!("skills/{}", skill.filename),
            &skill.content,
        )?;
    }
    for decision in &model.decisions {
        add_bytes(
            &mut tar,
            &format!("decisions/{}", decision.filename),
            &decision.content,
        )?;
    }

    add_bytes(
        &mut tar,
        "index/corpus.md",
        model.rendered.corpus_index.as_bytes(),
    )?;
    add_bytes(
        &mut tar,
        "index/graph.md",
        model.rendered.graph_check.as_bytes(),
    )?;
    add_bytes(
        &mut tar,
        "index/decisions.md",
        model.rendered.decisions_log.as_bytes(),
    )?;
    add_bytes(
        &mut tar,
        "index/skills.md",
        model.rendered.skills_index.as_bytes(),
    )?;

    if let Some(idx) = &model.index {
        add_bytes(&mut tar, "index/corpus.arrow", &idx.arrow_ipc)?;
        add_bytes(&mut tar, "index/meta.json", &idx.meta_raw)?;
        if let Some(cfg) = &idx.embed_config {
            add_bytes(
                &mut tar,
                "index/embed.config.json",
                &serde_json::to_vec_pretty(cfg)?,
            )?;
        }
    }

    let gz = tar.into_inner()?;
    Ok(gz.finish()?)
}

/// Bundle ontology, corpus, skills, decisions, and vector index into `.yidam/bundle.yiz`.
///
/// Backwards-compatible entry point; `yidam export --format bundle` is the canonical form.
pub fn bundle() -> Result<()> {
    let root = repo_root()?;
    let model = load_domain_model(&root)?;
    let bytes = render_bundle(&model)?;

    let output = root.join(".yidam").join("bundle.yiz");
    std::fs::write(&output, &bytes)?;

    let index_note = if model.index.is_some() {
        " + vector index"
    } else {
        " (no vector index — run `yidam index-build` first)"
    };
    println!(
        "Bundle written: {} classes, {} instances, {} skills, {} decisions{index_note}, \
         {} bytes → {}",
        model.classes.len(),
        model.instances.len(),
        model.skills.len(),
        model.decisions.len(),
        bytes.len(),
        output.display(),
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::embed_config::EmbedConfig;
    use crate::model::{IndexData, RenderedViews};
    use flate2::read::GzDecoder;
    use std::io::Read;

    fn minimal_model(index: Option<IndexData>) -> DomainModel {
        DomainModel {
            classes: vec![],
            instances: vec![],
            skills: vec![],
            decisions: vec![],
            index,
            provenance: crate::model::test_provenance(),
            rendered: RenderedViews {
                corpus_index: String::new(),
                graph_check: String::new(),
                decisions_log: String::new(),
                skills_index: String::new(),
            },
        }
    }

    fn archive_entries(bytes: &[u8]) -> Vec<(String, Vec<u8>)> {
        let mut archive = tar::Archive::new(GzDecoder::new(bytes));
        archive
            .entries()
            .unwrap()
            .map(|e| {
                let mut e = e.unwrap();
                let path = e.path().unwrap().to_string_lossy().into_owned();
                let mut data = Vec::new();
                e.read_to_end(&mut data).unwrap();
                (path, data)
            })
            .collect()
    }

    #[test]
    fn bundle_includes_embed_config_when_present() {
        let cfg = EmbedConfig::for_fastembed_model(
            "Xenova/all-MiniLM-L6-v2",
            384,
            "onnx/model_quantized.onnx",
            "AllMiniLML6V2Q",
        );
        let index = IndexData {
            arrow_ipc: b"stub".to_vec(),
            meta_raw: b"{}".to_vec(),
            meta: serde_json::json!({}),
            embed_config: Some(cfg.clone()),
        };
        let bytes = render_bundle(&minimal_model(Some(index))).unwrap();

        let entries = archive_entries(&bytes);
        let (_, data) = entries
            .iter()
            .find(|(p, _)| p == "index/embed.config.json")
            .expect("bundle should contain index/embed.config.json");
        let parsed: EmbedConfig = serde_json::from_slice(data).unwrap();
        assert_eq!(parsed, cfg);
    }

    #[test]
    fn bundle_omits_embed_config_when_absent() {
        let index = IndexData {
            arrow_ipc: b"stub".to_vec(),
            meta_raw: b"{}".to_vec(),
            meta: serde_json::json!({}),
            embed_config: None,
        };
        let bytes = render_bundle(&minimal_model(Some(index))).unwrap();

        let entries = archive_entries(&bytes);
        assert!(entries.iter().any(|(p, _)| p == "index/corpus.arrow"));
        assert!(!entries.iter().any(|(p, _)| p == "index/embed.config.json"));
    }

    fn manifest_of(model: &DomainModel) -> String {
        let bytes = render_bundle(model).unwrap();
        let (_, data) = archive_entries(&bytes)
            .into_iter()
            .find(|(p, _)| p == "manifest.yml")
            .expect("every bundle carries a manifest");
        String::from_utf8(data).unwrap()
    }

    /// RFC-0032 §4.5 and the addressing plan's decision C both say a consumer can tell two
    /// corpora apart "using the genesis digest each manifest already carries". Until #781 no
    /// manifest carried one — `genesis` is an ISO *date*, which two corpora created on one day
    /// share. Asserted through the decoder rather than by substring, so the field a reader
    /// gets is the field the writer wrote.
    #[test]
    fn the_manifest_carries_the_genesis_hash_a_reader_can_decode() {
        let model = DomainModel {
            provenance: Provenance {
                genesis_hash: Some("da4eeb36530f1111222233334444555566667777".into()),
                ..crate::model::test_provenance()
            },
            ..minimal_model(None)
        };
        let manifest = manifest_of(&model);
        assert_eq!(
            crate::deps::parse_manifest(&manifest)
                .genesis_hash
                .as_deref(),
            Some("da4eeb36530f1111222233334444555566667777")
        );
        assert!(
            manifest.contains("genesis: \"2026-01-01\""),
            "the date is still its own field, not replaced:\n{manifest}"
        );
    }

    /// A tree with no root commit has no identity, and the manifest says so rather than
    /// standing in a placeholder. A constant here would be the same string for every such
    /// bundle, which is the conflation RFC-0032 §2 is about — so absence has to survive the
    /// round trip as absence.
    #[test]
    fn a_corpus_with_no_root_commit_writes_a_null_that_reads_back_absent() {
        let manifest = manifest_of(&minimal_model(None));
        assert!(
            manifest.contains("genesis_hash: null"),
            "expected an explicit null:\n{manifest}"
        );
        assert_eq!(crate::deps::parse_manifest(&manifest).genesis_hash, None);
    }

    /// The manifest is hand-written YAML, so "it parses" is not a given for any field added
    /// to it. Both spellings of the new field, held to the whole document decoding.
    #[test]
    fn the_manifest_is_wellformed_yaml_either_way() {
        for model in [
            minimal_model(None),
            DomainModel {
                provenance: Provenance {
                    genesis_hash: Some("da4eeb36530f".into()),
                    ..crate::model::test_provenance()
                },
                ..minimal_model(None)
            },
        ] {
            let manifest = manifest_of(&model);
            let value: serde_yaml::Value = serde_yaml::from_str(&manifest)
                .unwrap_or_else(|e| panic!("manifest is not YAML: {e}\n{manifest}"));
            assert_eq!(value["bundle_version"].as_str(), Some("1"));
            assert_eq!(value["domain"].as_str(), Some("test-domain"));
        }
    }
}
