use anyhow::{bail, Context, Result};
use std::path::Path;

use super::config::LockedPackage;

// ── fetch ─────────────────────────────────────────────────────────────────────

pub async fn fetch_bytes(url: &str) -> Result<Vec<u8>> {
    let client = reqwest::Client::builder()
        .user_agent("yidam-tonpa/0.1")
        .build()
        .context("building HTTP client")?;
    let resp = client
        .get(url)
        .send()
        .await
        .with_context(|| format!("GET {url}"))?;
    if !resp.status().is_success() {
        bail!("HTTP {} fetching {url}", resp.status());
    }
    let bytes = resp
        .bytes()
        .await
        .with_context(|| format!("reading response body from {url}"))?;
    Ok(bytes.to_vec())
}

// ── hash ──────────────────────────────────────────────────────────────────────

// In `crate::deps`, which is not behind the `tonpa` feature, so `doctor` can ask whether an
// installed bundle matches the lock in a build that cannot fetch one.
pub use crate::deps::sha256_hex;

// ── extract ───────────────────────────────────────────────────────────────────

// In `crate::deps`, beside `verify_installed` that hashes what it writes: `cluster ask` unpacks
// a peer it took from the vault, in a build that may have no `tonpa` feature to fetch with.
pub use crate::deps::extract_bundle;

// ── install ───────────────────────────────────────────────────────────────────

pub async fn install_package(
    name: &str,
    url: &str,
    tonpa_dir: &Path,
    expected_sha256: Option<&str>,
) -> Result<LockedPackage> {
    println!("  fetching {url} …");
    let data = fetch_bytes(url).await?;
    let sha256 = sha256_hex(&data);

    if let Some(expected) = expected_sha256 {
        if sha256 != expected {
            bail!("hash mismatch for {name}\n  expected: {expected}\n  got:      {sha256}");
        }
    }

    let dest = tonpa_dir.join(name);
    println!("  extracting {name} ({} bytes) …", data.len());
    let (manifest, malformed) = extract_bundle(&data, &dest)?;

    // Said, not swallowed. The install still proceeds — the corpus files are on disk and
    // readable, which is what a dependency is for — but the three lock fields below are about
    // to be written as `None`, and `None` is indistinguishable from a bundle built before the
    // fields existed. `doctor`'s `corpora` line reports the same state on a later run.
    if let Some(why) = malformed {
        eprintln!(
            "  warning: {name}'s manifest.yml does not parse ({why}) — pinning it \
             without a commit, genesis or index model"
        );
    }

    let nodes = count_corpus_nodes(&dest);
    let dims = read_index_dims(&dest);

    println!(
        "  installed {name} — {nodes} nodes, model: {}",
        manifest.vector_index_model.as_deref().unwrap_or("none")
    );

    Ok(LockedPackage {
        name: name.to_string(),
        url: url.to_string(),
        sha256,
        commit: manifest.commit,
        genesis: manifest.genesis,
        model: manifest.vector_index_model,
        dims,
        nodes: Some(nodes),
    })
}

/// Re-extract a package from its cached `bundle.yiz` without re-fetching.
/// Not wired to a CLI subcommand yet.
#[allow(dead_code)]
pub fn reinstall_from_cache(name: &str, tonpa_dir: &Path) -> Result<()> {
    let bundle_path = tonpa_dir.join(name).join("bundle.yiz");
    if !bundle_path.exists() {
        bail!("no cached bundle for '{name}' — run `yidam tonpa install` to fetch");
    }
    let data = std::fs::read(&bundle_path)?;
    let dest = tonpa_dir.join(name);
    extract_bundle(&data, &dest)?;
    println!("  reinstalled {name} from cache");
    Ok(())
}

// ── verify ────────────────────────────────────────────────────────────────────

pub use crate::deps::verify_installed;

// ── private helpers ───────────────────────────────────────────────────────────

fn count_corpus_nodes(dest: &Path) -> u64 {
    let corpus = dest.join("corpus");
    if !corpus.exists() {
        return 0;
    }
    walkdir::WalkDir::new(&corpus)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .count() as u64
}

fn read_index_dims(dest: &Path) -> Option<u32> {
    let text = std::fs::read_to_string(dest.join("index").join("meta.json")).ok()?;
    let v: serde_json::Value = serde_json::from_str(&text).ok()?;
    v["dim"].as_u64().map(|d| d as u32)
}
