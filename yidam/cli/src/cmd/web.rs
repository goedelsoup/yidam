use anyhow::Result;

use crate::paths::repo_root;
use crate::regen::update_file_regen;

pub fn bundle_status() -> Result<()> {
    let root = repo_root()?;
    let bundle_path = root.join(".yidam").join("bundle.yiz");

    let content = if bundle_path.exists() {
        let size = std::fs::metadata(&bundle_path)?.len();
        let meta_path = root.join(".yidam").join("index").join("meta.json");
        let index_line = if let Ok(meta_text) = std::fs::read_to_string(&meta_path) {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&meta_text) {
                let model = v["model_name"].as_str().unwrap_or("unknown");
                let nodes = v["node_count"].as_u64().unwrap_or(0);
                let indexed_at = v["indexed_commit"].as_str().unwrap_or("?");
                format!(" · {nodes} nodes indexed with `{model}` @ {indexed_at}")
            } else {
                String::new()
            }
        } else {
            " · no vector index (run `yidam index-build`)".to_string()
        };
        format!("bundle.yiz: {size} bytes{index_line}")
    } else {
        "_No bundle. Run `yidam bundle` to produce one._".to_string()
    };

    crate::regen::emit(&content);
    let web_dir = root.join("web");
    update_file_regen(&web_dir.join("README.md"), "yidam bundle-status", &content)
}
