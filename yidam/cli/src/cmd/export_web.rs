use anyhow::Result;
use serde_json::json;

use super::bundle::render_bundle;
use crate::model::DomainModel;

/// Default WebLLM model: broad WebGPU support at a tolerable download size.
const DEFAULT_WEBLLM_MODEL: &str = "Llama-3.2-1B-Instruct-q4f16_1-MLC";
/// Fallback for weak GPUs, attempted automatically when the default fails.
const FALLBACK_WEBLLM_MODEL: &str = "Qwen2.5-0.5B-Instruct-q4f16_1-MLC";

const INDEX_HTML: &str = include_str!("../../assets/web/index.html");
const MAIN_JS: &str = include_str!("../../assets/web/main.js");
const MAIN_CSS: &str = include_str!("../../assets/web/main.css");

/// Render the static web agent site: relative paths and their contents.
///
/// Pure `DomainModel → files` transform; the caller writes them under the
/// output directory. Layout:
///
/// ```text
/// index.html            entry point (web.config.json inlined for file:// use)
/// web.config.json       { bundle_path, domain, embed_config, webllm, cdn }
/// bundle.yiz            the same bytes `export --format bundle` produces
/// assets/main.js        retrieval + UI logic (vanilla ES module)
/// assets/main.css       design-system token subset
/// ```
///
/// transformers.js, apache-arrow, and WebLLM are loaded from the CDN URLs
/// pinned in the config — the exporter ships no model weights. Model IDs are
/// configuration, never hardcoded in the JS.
pub(crate) fn render_web(
    model: &DomainModel,
    webllm_model: Option<&str>,
) -> Result<Vec<(String, Vec<u8>)>> {
    let embed_config = model
        .index
        .as_ref()
        .and_then(|idx| idx.embed_config.as_ref())
        .map(serde_json::to_value)
        .transpose()?
        .unwrap_or(serde_json::Value::Null);

    let config = json!({
        "format_version": "1",
        "bundle_path": "bundle.yiz",
        "domain": model.provenance.domain,
        "commit": model.provenance.commit,
        "embed_config": embed_config,
        "webllm": {
            "model": webllm_model.unwrap_or(DEFAULT_WEBLLM_MODEL),
            "fallback_model": FALLBACK_WEBLLM_MODEL,
        },
        "cdn": {
            "transformers": "https://cdn.jsdelivr.net/npm/@huggingface/transformers@3.4.2/+esm",
            "arrow": "https://cdn.jsdelivr.net/npm/apache-arrow@17.0.0/+esm",
            "webllm": "https://cdn.jsdelivr.net/npm/@mlc-ai/web-llm@0.2.79/+esm",
        },
    });
    let config_json = serde_json::to_string_pretty(&config)?;

    let index_html = INDEX_HTML.replace("__WEB_CONFIG_JSON__", &config_json);
    let bundle = render_bundle(model)?;

    Ok(vec![
        ("index.html".to_string(), index_html.into_bytes()),
        ("web.config.json".to_string(), config_json.into_bytes()),
        ("bundle.yiz".to_string(), bundle),
        ("assets/main.js".to_string(), MAIN_JS.as_bytes().to_vec()),
        ("assets/main.css".to_string(), MAIN_CSS.as_bytes().to_vec()),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::embed_config::EmbedConfig;
    use crate::model::{IndexData, Provenance, RenderedViews};

    fn model(index: Option<IndexData>) -> DomainModel {
        DomainModel {
            classes: vec![],
            instances: vec![],
            skills: vec![],
            decisions: vec![],
            index,
            provenance: Provenance {
                commit: "abc1234".into(),
                genesis: "2026-01-01".into(),
                domain: "test-domain".into(),
                generated_at: 0,
            },
            rendered: RenderedViews {
                corpus_index: String::new(),
                graph_check: String::new(),
                decisions_log: String::new(),
                skills_index: String::new(),
            },
        }
    }

    fn indexed_model() -> DomainModel {
        model(Some(IndexData {
            arrow_ipc: b"stub".to_vec(),
            meta_raw: b"{}".to_vec(),
            meta: serde_json::json!({}),
            embed_config: Some(EmbedConfig::for_fastembed_model(
                "Xenova/all-MiniLM-L6-v2",
                384,
                "onnx/model_quantized.onnx",
                "AllMiniLML6V2Q",
            )),
        }))
    }

    fn file<'a>(files: &'a [(String, Vec<u8>)], name: &str) -> &'a [u8] {
        &files
            .iter()
            .find(|(n, _)| n == name)
            .unwrap_or_else(|| panic!("missing {name}"))
            .1
    }

    #[test]
    fn renders_all_five_files() {
        let files = render_web(&indexed_model(), None).unwrap();
        let names: Vec<&str> = files.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(
            names,
            vec![
                "index.html",
                "web.config.json",
                "bundle.yiz",
                "assets/main.js",
                "assets/main.css"
            ]
        );
    }

    #[test]
    fn config_carries_embed_contract_and_models() {
        let files = render_web(&indexed_model(), None).unwrap();
        let config: serde_json::Value =
            serde_json::from_slice(file(&files, "web.config.json")).unwrap();
        assert_eq!(config["bundle_path"], "bundle.yiz");
        assert_eq!(config["domain"], "test-domain");
        assert_eq!(
            config["embed_config"]["model_id"],
            "Xenova/all-MiniLM-L6-v2"
        );
        assert_eq!(
            config["embed_config"]["model_file"],
            "onnx/model_quantized.onnx"
        );
        assert_eq!(config["webllm"]["model"], DEFAULT_WEBLLM_MODEL);
        assert_eq!(config["webllm"]["fallback_model"], FALLBACK_WEBLLM_MODEL);
        assert!(config["cdn"]["transformers"]
            .as_str()
            .unwrap()
            .starts_with("https://"));
    }

    #[test]
    fn webllm_model_is_configurable() {
        let files =
            render_web(&indexed_model(), Some("Phi-3.5-mini-instruct-q4f16_1-MLC")).unwrap();
        let config: serde_json::Value =
            serde_json::from_slice(file(&files, "web.config.json")).unwrap();
        assert_eq!(
            config["webllm"]["model"],
            "Phi-3.5-mini-instruct-q4f16_1-MLC"
        );
    }

    #[test]
    fn index_html_inlines_the_config() {
        let files = render_web(&indexed_model(), None).unwrap();
        let html = std::str::from_utf8(file(&files, "index.html")).unwrap();
        assert!(
            !html.contains("__WEB_CONFIG_JSON__"),
            "placeholder replaced"
        );
        assert!(html.contains("\"model_id\": \"Xenova/all-MiniLM-L6-v2\""));
        assert!(html.contains("assets/main.js"));
    }

    #[test]
    fn embed_config_is_null_without_index() {
        let files = render_web(&model(None), None).unwrap();
        let config: serde_json::Value =
            serde_json::from_slice(file(&files, "web.config.json")).unwrap();
        assert!(config["embed_config"].is_null());
    }

    #[test]
    fn bundle_matches_render_bundle_output() {
        let m = indexed_model();
        let files = render_web(&m, None).unwrap();
        assert_eq!(
            file(&files, "bundle.yiz"),
            render_bundle(&m).unwrap().as_slice()
        );
    }
}
