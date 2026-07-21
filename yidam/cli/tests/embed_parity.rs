//! Rust reference runner for the embedding reproducibility contract
//! (`prelude/sdks/parity/fixtures/embed_config/`).
//!
//! Downloads model weights on first run, so it only executes when
//! `YIDAM_EMBED_PARITY=1` is set (see the `embed-parity` mise task).
//! TypeScript and Python runners for the same fixtures live in
//! `prelude/sdks/{typescript,python}/tests/`.

#![cfg(feature = "index")]

use fastembed::{InitOptions, TextEmbedding};
use std::path::PathBuf;

fn fixture_dir() -> PathBuf {
    // CARGO_MANIFEST_DIR = yidam/cli/
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../prelude/sdks/parity/fixtures/embed_config")
}

#[test]
fn embed_config_parity() {
    if std::env::var("YIDAM_EMBED_PARITY").as_deref() != Ok("1") {
        eprintln!("skipping embed_config parity (set YIDAM_EMBED_PARITY=1 to run)");
        return;
    }

    let dir = fixture_dir();
    let mut entries: Vec<_> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("reading {}: {e}", dir.display()))
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|x| x == "toml"))
        .collect();
    entries.sort_by_key(|e| e.path());
    assert!(!entries.is_empty(), "no embed_config fixtures found");

    for entry in entries {
        let raw = std::fs::read_to_string(entry.path()).unwrap();
        let fx: toml::Value = toml::from_str(&raw).unwrap();
        let input = &fx["input"];
        let expected = &fx["expected"];

        let model_id = input["model_id"].as_str().unwrap();
        let text = input["text"].as_str().unwrap();
        let exp_dim = expected["embedding_dim"].as_integer().unwrap() as usize;
        let tolerance = expected["tolerance"].as_float().unwrap();
        let exp_prefix: Vec<f64> = expected["prefix"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_float().unwrap())
            .collect();

        let model_enum = TextEmbedding::list_supported_models()
            .into_iter()
            .find(|m| m.model_code == model_id)
            .unwrap_or_else(|| panic!("model {model_id} not supported by fastembed"))
            .model;
        let model = TextEmbedding::try_new(
            InitOptions::new(model_enum)
                .with_cache_dir(std::env::temp_dir().join("yidam-fastembed-cache")),
        )
        .unwrap();

        let vector = model.embed(vec![text.to_string()], None).unwrap().remove(0);
        assert_eq!(vector.len(), exp_dim, "embedding_dim");

        let norm: f32 = vector.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!(
            (norm - 1.0).abs() < 1e-3,
            "vector should be L2-normalized, norm = {norm}"
        );

        let actual_prefix: Vec<f32> = vector[..exp_prefix.len()].to_vec();
        for (i, (a, e)) in actual_prefix.iter().zip(exp_prefix.iter()).enumerate() {
            assert!(
                (*a as f64 - e).abs() <= tolerance,
                "{}: prefix[{i}] = {a}, expected {e} (tolerance {tolerance})\n\
                 full actual prefix: {actual_prefix:?}",
                entry.path().display(),
            );
        }
    }
}
