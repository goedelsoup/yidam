//! Rust reference runner for the embedding reproducibility contract
//! (`prelude/sdks/parity/fixtures/embed_config/`).
//!
//! Downloads model weights on first run, so it only executes when
//! `YIDAM_EMBED_PARITY=1` is set (see the `embed-parity` mise task).
//! TypeScript and Python runners for the same fixtures live in
//! `prelude/sdks/{typescript,python}/tests/`.

// The fixture-agreement test below needs no model weights and no vector feature; only the
// reference runner does — and what it needs is `fastembed`, which is `vector-read`. It was
// gated on `index` until #442 split the two, which meant the parity runner sat behind protoc
// for no reason: it embeds text, it never builds an index.
#![cfg_attr(not(feature = "vector-read"), allow(unused_imports))]

#[cfg(feature = "vector-read")]
use fastembed::{InitOptions, TextEmbedding};
use std::path::PathBuf;

fn fixture_dir() -> PathBuf {
    // CARGO_MANIFEST_DIR = yidam/cli/
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../prelude/sdks/parity/fixtures/embed_config")
}

#[cfg(feature = "vector-read")]
#[test]
fn embed_config_parity() {
    if std::env::var("YIDAM_EMBED_PARITY").as_deref() != Ok("1") {
        ci_report::skipped("set YIDAM_EMBED_PARITY=1 to run the embed_config parity check");
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

/// The witness written into an index and the witness CI compares across three runtimes must
/// be the same witness.
///
/// `index-build` embeds [`VERIFICATION_PROBE`] and writes the first eight normalized
/// dimensions into `embed.config.json`; this fixture embeds a probe of its own and compares
/// across fastembed, transformers.js and sentence-transformers. If those two probes ever
/// stopped being the same sentence, the number travelling with every index would be checked
/// against nothing — and the number CI proves agreement on would be reaching no consumer.
///
/// Runs without model weights, so it is not gated behind `YIDAM_EMBED_PARITY`. That matters:
/// the gated runner downloads hundreds of megabytes and is therefore off in every run that
/// is not deliberately about embeddings, which is nearly all of them.
#[test]
fn the_shipped_witness_and_the_parity_fixture_agree() {
    let raw = std::fs::read_to_string(fixture_dir().join("sentence-a.toml")).unwrap();
    let fx: toml::Value = toml::from_str(&raw).unwrap();

    assert_eq!(
        fx["input"]["text"].as_str().unwrap(),
        yidam::embed_config::VERIFICATION_PROBE,
        "the probe shipped in every index is not the probe parity proves agreement on"
    );
    assert_eq!(
        fx["expected"]["prefix"].as_array().unwrap().len(),
        yidam::embed_config::VERIFICATION_PREFIX_DIMS,
        "the fixture and the shipped block carry different numbers of dimensions"
    );
    assert_eq!(
        fx["expected"]["tolerance"].as_float().unwrap() as f32,
        yidam::embed_config::VERIFICATION_TOLERANCE,
    );

    // Every runtime that declares a drift in the fixture must have that drift carried into
    // the index. A consumer holding only an index directory has no other way to learn it.
    let declared = fx["known_delta"].as_table().unwrap();
    for (runtime, table) in declared {
        let shipped = yidam::embed_config::KNOWN_DELTAS
            .iter()
            .find(|(name, _)| name == runtime)
            .unwrap_or_else(|| {
                panic!("the fixture declares a known delta for `{runtime}` and no index carries it")
            });
        assert_eq!(
            table["tolerance"].as_float().unwrap() as f32,
            shipped.1,
            "`{runtime}` drift bound differs between the fixture and what ships"
        );
    }
    assert_eq!(
        declared.len(),
        yidam::embed_config::KNOWN_DELTAS.len(),
        "an index ships a known delta the parity fixture never measured"
    );
}
