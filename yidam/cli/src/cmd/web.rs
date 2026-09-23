use anyhow::Result;

use crate::paths::repo_root;
use crate::regen::update_file_regen;

/// What the committed block holds — and it is not the measurement below.
///
/// `.yidam/bundle.yiz` is built by `yidam export --format bundle` and attached to a release;
/// no commit carries it, and the scaffolded `.gitignore` says so. Its size and the index
/// metadata beside it are facts about a working tree, and `yidam regen --check` gates this
/// block — so writing them here made the gate's exit code a property of the machine running
/// it. Found by asking #895's question of the other generators: building a bundle and
/// changing nothing else took `regen --check` from exit 0 to exit 1.
///
/// A constant rather than a removal, for the reason given on
/// [`crate::cmd::status::COMMITTED_INDEX_BLOCK`]: a removed generator leaves the block behind
/// in every corpus that has one, refreshed by nothing and checked by nothing. The measurement
/// still goes to stdout, where being current is the point.
const COMMITTED_BUNDLE_BLOCK: &str =
    "_The bundle is a build artifact and is in no commit — whether one exists, and what it \
     holds, is a fact about a working tree rather than about this revision. Run \
     `yidam bundle-status` for this machine's answer._";

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

    // Live to the terminal. `emit` is suppressed under `--check`, so the measured content
    // reaches no gated artifact.
    crate::regen::emit(&content);
    let web_dir = root.join("web");
    update_file_regen(
        &web_dir.join("README.md"),
        "yidam bundle-status",
        COMMITTED_BUNDLE_BLOCK,
    )
}
