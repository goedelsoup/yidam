use anyhow::{Context, Result};
use arrow_array::{
    FixedSizeListArray, Float32Array, RecordBatch, RecordBatchIterator, StringArray,
};
use arrow_ipc::writer::FileWriter;
use arrow_schema::{DataType, Field, Schema};
use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};
use futures::TryStreamExt;
use lancedb::connect;
use lancedb::query::ExecutableQuery;
use std::sync::Arc;

use crate::config::load_yidam_config;
use crate::embed_config::{EmbedConfig, EMBED_CONFIG_FILENAME};
use crate::git::head_commit_short;
use crate::paths::{repo_root, yidam_embeddings_dir, yidam_index_dir};

const DEFAULT_MODEL: &str = "Xenova/all-MiniLM-L6-v2";
const TABLE_NAME: &str = "corpus";

#[derive(serde::Deserialize)]
struct EmbedRecord {
    path: String,
    class: String,
    label: String,
    text: String,
}

pub(crate) fn resolve_model(name: &str) -> Result<(EmbeddingModel, i32, String)> {
    TextEmbedding::list_supported_models()
        .into_iter()
        .find(|m| m.model_code == name)
        .map(|m| (m.model, m.dim as i32, m.model_file))
        .ok_or_else(|| {
            let list = TextEmbedding::list_supported_models()
                .into_iter()
                .map(|m| format!("  {}", m.model_code))
                .collect::<Vec<_>>()
                .join("\n");
            anyhow::anyhow!("unknown model {name:?}\n\nSupported models:\n{list}")
        })
}

pub async fn index_build(model_arg: Option<String>) -> Result<()> {
    let root = repo_root()?;
    let embeddings_dir = yidam_embeddings_dir(&root);
    let index_dir = yidam_index_dir(&root);

    if !embeddings_dir.exists() {
        anyhow::bail!(
            "No embeddings found at {}. Run `yidam embed` first.",
            embeddings_dir.display()
        );
    }

    // Model resolution: CLI arg → .yidam/config.toml → default
    let model_name = if let Some(m) = model_arg {
        m
    } else {
        let cfg = load_yidam_config(&root)?;
        cfg.index.model.unwrap_or_else(|| DEFAULT_MODEL.to_string())
    };

    let (embedding_model, embedding_dim, model_file) = resolve_model(&model_name)?;

    let mut records: Vec<EmbedRecord> = Vec::new();
    for entry in walkdir::WalkDir::new(&embeddings_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|x| x == "json"))
    {
        let text = std::fs::read_to_string(entry.path())?;
        let r: EmbedRecord = serde_json::from_str(&text)
            .with_context(|| format!("parsing {}", entry.path().display()))?;
        records.push(r);
    }

    if records.is_empty() {
        println!("No embedding records found.");
        return Ok(());
    }

    println!("Loaded {} embedding record(s).", records.len());
    println!("Initializing model ({model_name})…");
    println!("  (first run downloads model weights)");

    let model = TextEmbedding::try_new(InitOptions::new(embedding_model.clone()))?;

    let texts: Vec<String> = records.iter().map(|r| r.text.clone()).collect();
    println!("Embedding {} texts…", texts.len());
    let embeddings = model.embed(texts, None)?;

    let schema = Arc::new(Schema::new(vec![
        Field::new("path", DataType::Utf8, false),
        Field::new("class", DataType::Utf8, false),
        Field::new("label", DataType::Utf8, false),
        Field::new("text", DataType::Utf8, false),
        Field::new(
            "vector",
            DataType::FixedSizeList(
                Arc::new(Field::new("item", DataType::Float32, true)),
                embedding_dim,
            ),
            false,
        ),
    ]));

    let paths: StringArray = records.iter().map(|r| Some(r.path.as_str())).collect();
    let classes: StringArray = records.iter().map(|r| Some(r.class.as_str())).collect();
    let labels: StringArray = records.iter().map(|r| Some(r.label.as_str())).collect();
    let texts_arr: StringArray = records.iter().map(|r| Some(r.text.as_str())).collect();

    let flat_floats: Float32Array = embeddings
        .iter()
        .flat_map(|v| v.iter().copied())
        .collect::<Vec<f32>>()
        .into();

    let item_field = Arc::new(Field::new("item", DataType::Float32, true));
    let vector_array =
        FixedSizeListArray::new(item_field, embedding_dim, Arc::new(flat_floats), None);

    let batch = RecordBatch::try_new(
        schema.clone(),
        vec![
            Arc::new(paths),
            Arc::new(classes),
            Arc::new(labels),
            Arc::new(texts_arr),
            Arc::new(vector_array),
        ],
    )?;

    if index_dir.exists() {
        std::fs::remove_dir_all(&index_dir)?;
    }
    std::fs::create_dir_all(&index_dir)?;

    let db_path = index_dir
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("index path is not valid UTF-8: {}", index_dir.display()))?
        .to_string();
    let db = connect(&db_path).execute().await?;
    db.create_table(
        TABLE_NAME,
        RecordBatchIterator::new(vec![Ok(batch)], schema),
    )
    .execute()
    .await?;

    // Export Arrow IPC for the web shell (reads from the table to guarantee consistency)
    let table = db.open_table(TABLE_NAME).execute().await?;
    let stream = table.query().execute().await?;
    let batches: Vec<RecordBatch> = stream.try_collect().await?;

    if !batches.is_empty() {
        let arrow_schema = batches[0].schema();
        let mut ipc_bytes: Vec<u8> = Vec::new();
        {
            let mut writer = FileWriter::try_new(&mut ipc_bytes, &arrow_schema)?;
            for b in &batches {
                writer.write(b)?;
            }
            writer.finish()?;
        }
        std::fs::write(index_dir.join("corpus.arrow"), &ipc_bytes)?;
        println!("  Arrow IPC exported: {} bytes", ipc_bytes.len());
    }

    let commit = head_commit_short(&root);
    let node_count = records.len();
    let generated_at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let meta = serde_json::json!({
        "model_name": model_name,
        "embedding_dim": embedding_dim,
        "node_count": node_count,
        "indexed_commit": commit,
        "table": TABLE_NAME,
        "generated_at": generated_at,
    });
    std::fs::write(
        index_dir.join("meta.json"),
        serde_json::to_string_pretty(&meta)?,
    )?;

    // The witness, embedded with the model that just built the index — so what travels with
    // the index is a number produced by the same weights rather than a number copied from a
    // fixture that might have moved.
    let probe = model.embed(
        vec![crate::embed_config::VERIFICATION_PROBE.to_string()],
        None,
    )?;
    let embed_config = EmbedConfig::for_fastembed_model(
        &model_name,
        embedding_dim,
        &model_file,
        &format!("{embedding_model:?}"),
    )
    .with_verification(probe.first().map(Vec::as_slice).unwrap_or(&[]));
    std::fs::write(
        index_dir.join(EMBED_CONFIG_FILENAME),
        serde_json::to_string_pretty(&embed_config)?,
    )?;

    println!(
        "Index built: {node_count} node(s) → {} (model: {model_name})",
        index_dir.display(),
    );
    Ok(())
}
