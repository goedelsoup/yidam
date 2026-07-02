use anyhow::{Context, Result};
use rusqlite::Connection;
use std::path::Path;

use crate::model::{index_rows, DomainModel};

/// Register the sqlite-vec extension for every connection this process opens.
/// Safe to call repeatedly; `sqlite3_auto_extension` deduplicates.
fn register_sqlite_vec() {
    unsafe {
        type InitFn = unsafe extern "C" fn(
            *mut rusqlite::ffi::sqlite3,
            *mut *mut std::os::raw::c_char,
            *const rusqlite::ffi::sqlite3_api_routines,
        ) -> std::os::raw::c_int;
        let init: InitFn =
            std::mem::transmute(sqlite_vec::sqlite3_vec_init as *const std::os::raw::c_void);
        rusqlite::ffi::sqlite3_auto_extension(Some(init));
    }
}

fn f32s_to_blob(vector: &[f32]) -> Vec<u8> {
    vector.iter().flat_map(|f| f.to_le_bytes()).collect()
}

/// Write the vector index as a single-file SQLite database using the
/// `sqlite-vec` extension: a `corpus_vec` vec0 virtual table with the
/// embeddings plus metadata columns, and a `corpus_meta` table carrying the
/// embedding contract so consumers embed queries with matching settings.
///
/// Vectors come from the already-built Arrow index — nothing is re-embedded.
/// The embedding dimension is read from `embed.config.json` (falling back to
/// `meta.json` for pre-contract indexes), never hardcoded.
pub(crate) fn render_sqlite(model: &DomainModel, out: &Path) -> Result<()> {
    let index = model.index.as_ref().context(
        "no vector index — run `yidam embed && yidam index-build` before exporting sqlite",
    )?;
    let rows = index_rows(index)?;

    let (model_id, dim, pooling, normalize) = match &index.embed_config {
        Some(ec) => (
            ec.model_id.clone(),
            ec.embedding_dim as i64,
            Some(ec.pooling.clone()),
            Some(ec.normalize),
        ),
        // Pre-contract index: meta.json knows the model and dim, but the
        // pooling/normalize settings were never pinned — stored as NULL
        // rather than guessed.
        None => (
            index.meta["model_name"]
                .as_str()
                .unwrap_or("unknown")
                .to_string(),
            index.meta["embedding_dim"].as_i64().unwrap_or(0),
            None,
            None,
        ),
    };
    anyhow::ensure!(
        dim > 0,
        "embedding dimension unknown — rebuild with `yidam index-build`"
    );

    register_sqlite_vec();
    if out.exists() {
        std::fs::remove_file(out)
            .with_context(|| format!("removing existing {}", out.display()))?;
    }
    let conn = Connection::open(out)?;

    conn.execute_batch(&format!(
        "CREATE VIRTUAL TABLE corpus_vec USING vec0(
           path TEXT,
           class TEXT,
           label TEXT,
           +text TEXT,
           embedding FLOAT[{dim}]
         );
         CREATE TABLE corpus_meta (
           model_id TEXT,
           embedding_dim INTEGER,
           pooling TEXT,
           normalize INTEGER,
           indexed_commit TEXT,
           generated_at INTEGER
         );"
    ))?;

    conn.execute(
        "INSERT INTO corpus_meta (model_id, embedding_dim, pooling, normalize, indexed_commit, generated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        rusqlite::params![
            model_id,
            dim,
            pooling,
            normalize,
            index.meta["indexed_commit"].as_str(),
            model.provenance.generated_at as i64,
        ],
    )?;

    let mut insert = conn.prepare(
        "INSERT INTO corpus_vec (path, class, label, text, embedding)
         VALUES (?1, ?2, ?3, ?4, ?5)",
    )?;
    for row in &rows {
        anyhow::ensure!(
            row.vector.len() as i64 == dim,
            "vector for {} has {} dims, expected {dim}",
            row.path,
            row.vector.len()
        );
        insert.execute(rusqlite::params![
            row.path,
            row.class,
            row.label,
            row.text,
            f32s_to_blob(&row.vector),
        ])?;
    }
    drop(insert);
    conn.close().map_err(|(_, e)| e)?;

    println!(
        "SQLite vector DB written: {} row(s), {dim} dims, model {model_id} → {}",
        rows.len(),
        out.display()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::embed_config::EmbedConfig;
    use crate::model::{IndexData, Provenance, RenderedViews};
    use arrow_array::{FixedSizeListArray, Float32Array, RecordBatch, StringArray};
    use arrow_schema::{DataType, Field, Schema};
    use std::sync::Arc;

    /// Build Arrow IPC bytes the way `index_build` does.
    fn arrow_ipc(rows: &[(&str, &str, &str, &str, Vec<f32>)], dim: i32) -> Vec<u8> {
        let schema = Arc::new(Schema::new(vec![
            Field::new("path", DataType::Utf8, false),
            Field::new("class", DataType::Utf8, false),
            Field::new("label", DataType::Utf8, false),
            Field::new("text", DataType::Utf8, false),
            Field::new(
                "vector",
                DataType::FixedSizeList(Arc::new(Field::new("item", DataType::Float32, true)), dim),
                false,
            ),
        ]));
        let flat: Float32Array = rows
            .iter()
            .flat_map(|r| r.4.iter().copied())
            .collect::<Vec<f32>>()
            .into();
        let vectors = FixedSizeListArray::new(
            Arc::new(Field::new("item", DataType::Float32, true)),
            dim,
            Arc::new(flat),
            None,
        );
        let batch = RecordBatch::try_new(
            schema.clone(),
            vec![
                Arc::new(rows.iter().map(|r| Some(r.0)).collect::<StringArray>()),
                Arc::new(rows.iter().map(|r| Some(r.1)).collect::<StringArray>()),
                Arc::new(rows.iter().map(|r| Some(r.2)).collect::<StringArray>()),
                Arc::new(rows.iter().map(|r| Some(r.3)).collect::<StringArray>()),
                Arc::new(vectors),
            ],
        )
        .unwrap();
        let mut bytes = Vec::new();
        {
            let mut writer = arrow_ipc::writer::FileWriter::try_new(&mut bytes, &schema).unwrap();
            writer.write(&batch).unwrap();
            writer.finish().unwrap();
        }
        bytes
    }

    fn test_model(dim: i32) -> DomainModel {
        // Unit vectors along different axes: cosine top-1 is unambiguous.
        let mut alpha = vec![0.0f32; dim as usize];
        alpha[0] = 1.0;
        let mut gamma = vec![0.0f32; dim as usize];
        gamma[1] = 1.0;
        let ipc = arrow_ipc(
            &[
                (
                    ".yidam/corpus/concept/alpha.yml",
                    "concept",
                    "Alpha",
                    "alpha text",
                    alpha,
                ),
                (
                    ".yidam/corpus/concept/gamma.yml",
                    "concept",
                    "Gamma",
                    "gamma text",
                    gamma,
                ),
            ],
            dim,
        );
        DomainModel {
            classes: vec![],
            instances: vec![],
            skills: vec![],
            decisions: vec![],
            index: Some(IndexData {
                arrow_ipc: ipc,
                meta_raw: b"{}".to_vec(),
                meta: serde_json::json!({"indexed_commit": "abc1234"}),
                embed_config: Some(EmbedConfig::for_fastembed_model(
                    "Xenova/all-MiniLM-L6-v2",
                    dim,
                    "onnx/model_quantized.onnx",
                    "AllMiniLML6V2Q",
                )),
            }),
            provenance: Provenance {
                commit: "abc1234".into(),
                genesis: "2026-01-01".into(),
                domain: "test".into(),
                generated_at: 1_780_000_000,
            },
            rendered: RenderedViews {
                corpus_index: String::new(),
                graph_check: String::new(),
                decisions_log: String::new(),
                skills_index: String::new(),
            },
        }
    }

    #[test]
    fn meta_table_matches_embed_config() {
        let tmp = tempfile::TempDir::new().unwrap();
        let db = tmp.path().join("corpus.db");
        render_sqlite(&test_model(4), &db).unwrap();

        let conn = Connection::open(&db).unwrap();
        let (model_id, dim, pooling, normalize): (String, i64, String, i64) = conn
            .query_row(
                "SELECT model_id, embedding_dim, pooling, normalize FROM corpus_meta",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .unwrap();
        assert_eq!(model_id, "Xenova/all-MiniLM-L6-v2");
        assert_eq!(dim, 4);
        assert_eq!(pooling, "mean");
        assert_eq!(normalize, 1);
    }

    #[test]
    fn vec0_ann_query_returns_correct_top1() {
        let tmp = tempfile::TempDir::new().unwrap();
        let db = tmp.path().join("corpus.db");
        render_sqlite(&test_model(4), &db).unwrap();

        let conn = Connection::open(&db).unwrap();
        // Query near the gamma axis: gamma must rank first.
        let query = f32s_to_blob(&[0.1, 0.99, 0.0, 0.0]);
        let (label, path): (String, String) = conn
            .query_row(
                "SELECT label, path FROM corpus_vec WHERE embedding MATCH ?1 \
                 ORDER BY distance LIMIT 1",
                [query],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(label, "Gamma");
        assert_eq!(path, ".yidam/corpus/concept/gamma.yml");
    }

    #[test]
    fn export_without_index_is_a_clear_error() {
        let mut m = test_model(4);
        m.index = None;
        let tmp = tempfile::TempDir::new().unwrap();
        let err = render_sqlite(&m, &tmp.path().join("corpus.db")).unwrap_err();
        assert!(err.to_string().contains("index-build"));
    }

    #[test]
    fn re_export_overwrites_cleanly() {
        let tmp = tempfile::TempDir::new().unwrap();
        let db = tmp.path().join("corpus.db");
        render_sqlite(&test_model(4), &db).unwrap();
        render_sqlite(&test_model(4), &db).unwrap();

        let conn = Connection::open(&db).unwrap();
        let count: i64 = conn
            .query_row("SELECT count(*) FROM corpus_vec", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 2);
    }
}
