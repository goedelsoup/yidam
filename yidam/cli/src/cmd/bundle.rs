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
/// ```
///
/// ## `manifest.yml` fields
///
/// | Field                | Type    | Description                                   |
/// |----------------------|---------|-----------------------------------------------|
/// | `bundle_version`     | string  | Format version ("1"). Bumped on breaking change|
/// | `commit`             | string  | Short SHA of HEAD at bundle time               |
/// | `genesis`            | string  | ISO date of the first commit                  |
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
pub(crate) fn render_bundle(model: &DomainModel) -> Result<Vec<u8>> {
    let p = &model.provenance;
    let index_line = match &model.index {
        Some(idx) => {
            let name = idx.meta["model_name"].as_str().unwrap_or("unknown");
            format!("vector_index_model: \"{name}\"\n")
        }
        None => "vector_index_model: null\n".to_string(),
    };

    let manifest = format!(
        "bundle_version: \"1\"\n\
         commit: \"{}\"\n\
         genesis: \"{}\"\n\
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
