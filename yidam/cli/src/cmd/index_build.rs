use crate::embedding::{resolve_model, DEFAULT_MODEL};
use anyhow::{Context, Result};
use arrow_array::{FixedSizeListArray, Float32Array, RecordBatch, StringArray};
use arrow_ipc::writer::FileWriter;
use arrow_schema::{DataType, Field, Schema};
use fastembed::{TextEmbedding, TextInitOptions};
use std::sync::Arc;

use crate::config::load_yidam_config;
use crate::embed_config::{EmbedConfig, EMBED_CONFIG_FILENAME};
use crate::git::head_commit_short;
use crate::paths::{repo_root, yidam_embeddings_dir, yidam_index_dir};

/// The name `meta.json` records under `table`. There is no table any more — #1287 removed the
/// LanceDB one nothing read — but the key is part of the contract a pushed index carries, and
/// `corpus` is still the name of the one file that holds the rows: `corpus.arrow`.
const TABLE_NAME: &str = "corpus";

/// The columns every index built by this command carries, in the order the schema declares
/// them. Written to `meta.json` as `columns` so a reader with no Arrow decoder — a person, a
/// web shell deciding what it can filter on — can see what the index holds without opening
/// it. `properties` is the one a `--where` needs, and the one an older index lacks.
pub const COLUMNS: &[&str] = &["path", "class", "label", "text", "properties", "vector"];

#[derive(serde::Deserialize)]
struct EmbedRecord {
    path: String,
    class: String,
    label: String,
    text: String,
    /// What `embed` carried for the node's ordered properties (#1029), absent where it
    /// carried none. Written to the index as one JSON column rather than a column per
    /// property: the set of declared names is the corpus's, not the schema's, and a table
    /// whose columns follow an ontology has to be rebuilt when the ontology gains a field.
    /// One nullable text column holds whatever the corpus declared, and the evaluator reads
    /// the declaration at query time — the same split `query` makes between the check, which
    /// reads the ontology, and the executor, which reads the value.
    #[serde(default)]
    properties: Option<std::collections::BTreeMap<String, serde_json::Value>>,
}

impl EmbedRecord {
    /// The `properties` cell: the object serialised, or null where the record carried none.
    ///
    /// Null and not `{}`, so a row that carried nothing and a row from an index built before
    /// the column existed read the same to a decoder — both are `None`, and the one thing
    /// that tells them apart is whether the column is there at all.
    fn properties_cell(&self) -> Option<String> {
        self.properties
            .as_ref()
            .filter(|p| !p.is_empty())
            .and_then(|p| serde_json::to_string(p).ok())
    }
}

pub fn index_build(model_arg: Option<String>) -> Result<()> {
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

    // Whether this index may vouch for its own class metadata (RFC-0033 §4.5).
    //
    // Checked here because here is where every row that will exist is in hand at once. An
    // anchored query narrows a remote search on the recorded `class` and resolves the node
    // from `path`; the two are the same derivation applied at different times, and a reader
    // has no way to establish that for itself without fetching the whole index back. So it
    // is established once, over all of them, and written into the contract that travels.
    //
    // A disagreement is not a build failure. The index is correct and every reader that
    // narrows locally is unaffected — what it loses is the push-down, which is a round trip
    // and never an answer.
    let misfiled = records
        .iter()
        .find(|r| !crate::paths::class_matches_path(&r.path, &r.class));
    if let Some(r) = misfiled {
        println!(
            "  note: {} is recorded as class `{}`, which is not the directory it sits in.\n  \
             This index will not declare `{}`, so a remote query narrows after fetching \
             rather than during the search. Re-run `yidam embed` with this yidam to fix it.",
            r.path,
            r.class,
            crate::embed_config::CLASS_SOURCE_PARENT_DIRECTORY
        );
    }

    println!("Initializing model ({model_name})…");
    println!("  (first run downloads model weights)");

    let mut model = TextEmbedding::try_new(TextInitOptions::new(embedding_model.clone()))?;

    let texts: Vec<String> = records.iter().map(|r| r.text.clone()).collect();
    println!("Embedding {} texts…", texts.len());
    let embeddings = model.embed(texts, None)?;

    // Encoded before the old index is removed, so a failure here leaves that one standing.
    let ipc_bytes = encode_corpus_arrow(&records, &embeddings, embedding_dim)?;

    if index_dir.exists() {
        std::fs::remove_dir_all(&index_dir)?;
    }
    std::fs::create_dir_all(&index_dir)?;

    std::fs::write(index_dir.join("corpus.arrow"), &ipc_bytes)?;
    println!("  Arrow IPC exported: {} bytes", ipc_bytes.len());

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
        // What the table holds, for a reader that will not open it. An index without this key
        // was built before `properties` existed, and lacks that column.
        "columns": COLUMNS,
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
    .with_verification(probe.first().map(Vec::as_slice).unwrap_or(&[]))
    .with_class_source(misfiled.is_none());
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

/// The rows as `corpus.arrow`: Arrow IPC, one batch, in the schema [`COLUMNS`] names.
///
/// The batch is what every reader decodes, so it is written as it stands. This used to go
/// through a LanceDB table first and read the file back out of it "to guarantee consistency".
/// Nothing ever opened the table, and the round trip was the only thing that could make the
/// file disagree with the batch: lancedb's `query()` capped its answer at ten rows, so for a
/// while the file held the first ten of every corpus while `meta.json` said otherwise. Writing
/// the batch removes that class of bug, and with it lancedb and the protoc it needed (#1287) —
/// which is why `index-build` is in `vector-read`, the build that reads an index.
///
/// Separate from [`index_build`] so the writer can be tested without a model: everything here
/// is arithmetic on rows already embedded.
fn encode_corpus_arrow(
    records: &[EmbedRecord],
    embeddings: &[Vec<f32>],
    dim: i32,
) -> Result<Vec<u8>> {
    let schema = Arc::new(Schema::new(vec![
        Field::new("path", DataType::Utf8, false),
        Field::new("class", DataType::Utf8, false),
        Field::new("label", DataType::Utf8, false),
        Field::new("text", DataType::Utf8, false),
        // Nullable: most rows in most corpora carry no ordered property, and the absence is
        // the value. See `EmbedRecord::properties_cell`.
        Field::new("properties", DataType::Utf8, true),
        Field::new(
            "vector",
            DataType::FixedSizeList(Arc::new(Field::new("item", DataType::Float32, true)), dim),
            false,
        ),
    ]));

    let paths: StringArray = records.iter().map(|r| Some(r.path.as_str())).collect();
    let classes: StringArray = records.iter().map(|r| Some(r.class.as_str())).collect();
    let labels: StringArray = records.iter().map(|r| Some(r.label.as_str())).collect();
    let texts_arr: StringArray = records.iter().map(|r| Some(r.text.as_str())).collect();
    let properties_arr: StringArray = records.iter().map(EmbedRecord::properties_cell).collect();

    let flat_floats: Float32Array = embeddings
        .iter()
        .flat_map(|v| v.iter().copied())
        .collect::<Vec<f32>>()
        .into();

    let item_field = Arc::new(Field::new("item", DataType::Float32, true));
    let vector_array = FixedSizeListArray::new(item_field, dim, Arc::new(flat_floats), None);

    let batch = RecordBatch::try_new(
        schema.clone(),
        vec![
            Arc::new(paths),
            Arc::new(classes),
            Arc::new(labels),
            Arc::new(texts_arr),
            Arc::new(properties_arr),
            Arc::new(vector_array),
        ],
    )?;

    let mut ipc_bytes: Vec<u8> = Vec::new();
    {
        let mut writer = FileWriter::try_new(&mut ipc_bytes, &schema)?;
        writer.write(&batch)?;
        writer.finish()?;
    }
    Ok(ipc_bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{index_rows, IndexData};

    fn record(i: usize, properties: Option<&str>) -> EmbedRecord {
        EmbedRecord {
            path: format!("corpus/gauge/g{i:02}.yml"),
            class: "gauge".to_string(),
            label: format!("Gauge {i}"),
            text: format!("gauge {i} on a river"),
            properties: properties.map(|p| serde_json::from_str(p).unwrap()),
        }
    }

    fn decode(ipc: Vec<u8>) -> IndexData {
        IndexData {
            arrow_ipc: ipc,
            meta_raw: b"{}".to_vec(),
            meta: serde_json::json!({}),
            embed_config: None,
        }
    }

    /// What `index-build` writes is what every reader decodes, row for row.
    ///
    /// Twelve rows, because ten is the number the LanceDB round trip used to stop at: a plain
    /// `query()` capped its answer there, and the file held the first ten of every corpus. The
    /// writer no longer has a round trip to get wrong, and this is the test that says so.
    #[test]
    fn every_row_written_is_a_row_read_back() {
        let records: Vec<EmbedRecord> = (0..12)
            .map(|i| record(i, (i == 3).then_some(r#"{"observed":"2026-01-02"}"#)))
            .collect();
        let embeddings: Vec<Vec<f32>> = (0..12).map(|i| vec![i as f32, 1.0, -1.0]).collect();

        let index = decode(encode_corpus_arrow(&records, &embeddings, 3).unwrap());
        let rows = index_rows(&index).expect("the reader decodes what the writer wrote");

        assert_eq!(rows.len(), 12, "every record is a row");
        for (i, row) in rows.iter().enumerate() {
            assert_eq!(row.path, records[i].path);
            assert_eq!(row.class, "gauge");
            assert_eq!(row.label, records[i].label);
            assert_eq!(row.text, records[i].text);
            assert_eq!(row.vector, embeddings[i]);
        }
        assert_eq!(
            rows[3].properties.as_deref(),
            Some(r#"{"observed":"2026-01-02"}"#)
        );
        assert!(rows
            .iter()
            .enumerate()
            .all(|(i, r)| i == 3 || r.properties.is_none()));
        assert!(index.has_properties_column());
    }

    /// The schema written is the one `meta.json` advertises, in that order. [`COLUMNS`] is
    /// what a reader without an Arrow decoder is told the file holds.
    #[test]
    fn the_written_schema_is_the_advertised_one() {
        let ipc = encode_corpus_arrow(&[record(0, None)], &[vec![0.5, 0.5]], 2).unwrap();
        let reader = arrow_ipc::reader::FileReader::try_new(std::io::Cursor::new(ipc), None)
            .expect("an Arrow IPC file");
        let schema = reader.schema();
        let names: Vec<&str> = schema.fields().iter().map(|f| f.name().as_str()).collect();
        assert_eq!(names, COLUMNS);
        assert!(schema.field_with_name("properties").unwrap().is_nullable());
        assert_eq!(
            schema.field_with_name("vector").unwrap().data_type(),
            &DataType::FixedSizeList(Arc::new(Field::new("item", DataType::Float32, true)), 2)
        );
    }
}
