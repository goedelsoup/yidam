use anyhow::{bail, Context, Result};
use flate2::read::GzDecoder;
use sha2::{Digest, Sha256};
use std::io::{Cursor, Read};
use std::path::Path;
use tar::Archive;

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

pub fn sha256_hex(data: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(data);
    hex::encode(h.finalize())
}

// ── extract ───────────────────────────────────────────────────────────────────

/// Parsed subset of `manifest.yml` from a `.yiz` bundle archive.
///
/// Only the fields that `tonpa install` needs are decoded here. The full
/// field list is documented on [`render_bundle`] in `cmd/bundle.rs`.
/// Unknown fields in the YAML are silently ignored — consumers MUST treat
/// unrecognised fields as non-breaking additions per the versioning policy.
#[derive(Debug, Default, serde::Deserialize)]
pub struct BundleManifest {
    /// Short SHA of the HEAD commit when the bundle was produced.
    pub commit: Option<String>,
    /// ISO date (YYYY-MM-DD) of the genesis (first) commit.
    pub genesis: Option<String>,
    /// Name of the fastembed model used to build the vector index, if present.
    pub vector_index_model: Option<String>,
    /// Number of corpus instance files included in the bundle.
    pub instances: Option<u64>,
}

pub fn extract_bundle(data: &[u8], dest: &Path) -> Result<BundleManifest> {
    std::fs::create_dir_all(dest)
        .with_context(|| format!("creating {}", dest.display()))?;

    // Cache the raw archive for future verify / reinstall without re-fetch
    std::fs::write(dest.join("bundle.yiz"), data)?;

    let gz = GzDecoder::new(Cursor::new(data));
    let mut archive = Archive::new(gz);
    let mut manifest_yaml = String::new();

    for entry in archive.entries()? {
        let mut entry = entry?;
        let rel = entry.path()?.to_path_buf();
        let dest_path = dest.join(&rel);

        if let Some(parent) = dest_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let mut buf = Vec::new();
        entry.read_to_end(&mut buf)?;

        if rel.to_string_lossy() == "manifest.yml" {
            manifest_yaml = String::from_utf8_lossy(&buf).to_string();
        }

        // Don't overwrite the bundle.yiz sentinel we wrote above
        if rel.to_string_lossy() != "bundle.yiz" {
            std::fs::write(&dest_path, &buf)
                .with_context(|| format!("writing {}", dest_path.display()))?;
        }
    }

    let manifest: BundleManifest =
        serde_yaml::from_str(&manifest_yaml).unwrap_or_default();
    Ok(manifest)
}

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
            bail!(
                "hash mismatch for {name}\n  expected: {expected}\n  got:      {sha256}"
            );
        }
    }

    let dest = tonpa_dir.join(name);
    println!("  extracting {name} ({} bytes) …", data.len());
    let manifest = extract_bundle(&data, &dest)?;

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

pub fn verify_installed(
    name: &str,
    tonpa_dir: &Path,
    locked: &LockedPackage,
) -> Result<bool> {
    let bundle_path = tonpa_dir.join(name).join("bundle.yiz");
    if !bundle_path.exists() {
        return Ok(false);
    }
    let data = std::fs::read(&bundle_path)?;
    Ok(sha256_hex(&data) == locked.sha256)
}

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
