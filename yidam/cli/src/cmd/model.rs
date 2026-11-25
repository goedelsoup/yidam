//! `yidam model` — put the embedding model where this corpus reads it (#1516).
//!
//! # Why a command at all
//!
//! fastembed fetches its model from Hugging Face the first time anything embeds, and caches it.
//! On a machine that cannot reach Hugging Face — no network, a proxy, a TLS-inspecting
//! gateway whose root rustls does not trust — that first fetch is the only way the model ever
//! arrives, and `retrieve` answers by keyword for as long as it is missing. The cache is plain
//! files, so a copy made on a machine that could download is enough. What was missing was a way
//! to put one where the corpus reads it without knowing the Hugging Face cache layout by heart.
//!
//! `fetch` with `--from` copies; without it, downloads. Both end by loading the model from the
//! cache they wrote, so success means a search would find it, not only that files were copied.
//!
//! # The layout it writes
//!
//! The Hugging Face hub's: `models--<org>--<name>/refs/main` holding a revision, and every file
//! under `snapshots/<revision>/`. The hub reads its cache before it asks the network, so a
//! seeded cache makes no request. Files are copied, never linked: a source that is a cache
//! itself keeps its files as symlinks into `blobs/`, and a link into a directory that is later
//! removed is a cache that stops working.

use std::path::{Path, PathBuf};

use anyhow::Result;
use clap::Subcommand;

#[derive(Debug, Subcommand)]
pub enum ModelCommand {
    /// Put the embedding model in this corpus's model cache, from a directory or by download
    ///
    /// The cache is `[index] cache_dir`, else `$FASTEMBED_CACHE_DIR`, else `.fastembed_cache`
    /// under the corpus root. `$HF_HOME`, when set, is read before all three, so it is written
    /// to instead.
    Fetch {
        /// Copy the model from this directory instead of downloading it
        ///
        /// A Hugging Face cache (holding `models--<org>--<name>/`), one model's folder within
        /// one, or a directory holding the model's files themselves.
        #[arg(long)]
        from: Option<PathBuf>,
        /// The model to fetch (overrides [index] model and the default)
        #[arg(long)]
        model: Option<String>,
    },
}

/// The revision a seeded cache records when its source named none.
///
/// The hub reads `refs/main` as a directory name and nothing more, so any spelling works; this
/// one says where the files came from to whoever lists the cache.
#[cfg_attr(not(feature = "vector-read"), allow(dead_code))]
const SEEDED_REVISION: &str = "yidam-seeded";

pub fn run(root: Option<&Path>, sub: ModelCommand) -> Result<()> {
    match sub {
        ModelCommand::Fetch { from, model } => fetch(root, from.as_deref(), model),
    }
}

#[cfg(not(feature = "vector-read"))]
fn fetch(_root: Option<&Path>, _from: Option<&Path>, _model: Option<String>) -> Result<()> {
    anyhow::bail!(
        "`model fetch` needs the `vector-read` feature — this build embeds nothing, so it reads \
         no model.\n  Reinstall from any release channel, or `cargo install yidam --locked`."
    )
}

#[cfg(feature = "vector-read")]
fn fetch(root: Option<&Path>, from: Option<&Path>, model: Option<String>) -> Result<()> {
    use crate::embedding::{self, Runtime, DEFAULT_MODEL};

    let root = crate::paths::resolve_root(root)?;
    let cfg = crate::config::load_yidam_config(&root)?;
    let name = model
        .or_else(|| cfg.index.model.clone())
        .unwrap_or_else(|| DEFAULT_MODEL.to_string());
    let (embedding_model, _, _) = embedding::resolve_model(&name)?;
    let runtime = Runtime::for_query(&root, &cfg.index);
    let cache = effective_cache(&runtime, std::env::var_os("HF_HOME"));

    match from {
        Some(source) => {
            let info = fastembed::TextEmbedding::get_model_info(&embedding_model)?;
            let files = required_files(&info.model_file, &info.additional_files);
            let seeded = seed(source, &cache, &name, &files)?;
            println!(
                "Copied {} file(s) of {name} into {}",
                files.len(),
                seeded.display()
            );
        }
        None => println!("Downloading {name} into {}", cache.display()),
    }

    // Loading is the check. A cache that has every file and will not load is a cache a search
    // would also fail on, and the error is the one it would report.
    embedding::load(embedding_model, &runtime).map_err(|e| {
        anyhow::anyhow!(
            "{name} did not load from {}: {e:#}\n  \
             Without --from this downloads from Hugging Face, which needs network access and a \
             certificate chain rustls trusts. On a machine without either, copy a cache from \
             one that has it and pass --from.",
            cache.display()
        )
    })?;
    println!("{name} loads from {}", cache.display());
    Ok(())
}

/// Where fastembed will actually read: `$HF_HOME` beats any directory it is handed.
#[cfg(feature = "vector-read")]
fn effective_cache(
    runtime: &crate::embedding::Runtime,
    hf_home: Option<std::ffi::OsString>,
) -> PathBuf {
    match hf_home {
        Some(home) => PathBuf::from(home),
        None => runtime
            .cache_dir
            .clone()
            .unwrap_or_else(|| PathBuf::from(crate::embedding::DEFAULT_CACHE_DIR)),
    }
}

/// Every file fastembed reads for one model: its weights, the files beside them, and the four
/// tokenizer files `load_tokenizer_hf_hub` asks for by name.
#[cfg_attr(not(feature = "vector-read"), allow(dead_code))]
fn required_files(model_file: &str, additional: &[String]) -> Vec<String> {
    let mut files = vec![model_file.to_string()];
    files.extend(additional.iter().cloned());
    for f in [
        "tokenizer.json",
        "config.json",
        "special_tokens_map.json",
        "tokenizer_config.json",
    ] {
        files.push(f.to_string());
    }
    files.dedup();
    files
}

/// The hub's folder for one model: `Xenova/all-MiniLM-L6-v2` is `models--Xenova--all-MiniLM-L6-v2`.
#[cfg_attr(not(feature = "vector-read"), allow(dead_code))]
fn folder_name(model: &str) -> String {
    format!("models--{}", model.replace('/', "--"))
}

/// The directory a source keeps one model's files in, and the revision it records for them.
///
/// Three shapes are accepted, tried in this order: a cache holding the model's folder, the
/// model's folder itself, and a directory holding the files with no layout at all.
#[cfg_attr(not(feature = "vector-read"), allow(dead_code))]
fn locate(source: &Path, model: &str) -> Result<(PathBuf, String)> {
    if !source.is_dir() {
        anyhow::bail!("--from {} is not a directory", source.display());
    }
    let nested = source.join(folder_name(model));
    let repo = if nested.is_dir() {
        Some(nested)
    } else if source.join("snapshots").is_dir() {
        Some(source.to_path_buf())
    } else {
        None
    };
    let Some(repo) = repo else {
        return Ok((source.to_path_buf(), SEEDED_REVISION.to_string()));
    };

    let snapshots = repo.join("snapshots");
    let named = std::fs::read_to_string(repo.join("refs/main"))
        .ok()
        .map(|r| r.trim().to_string())
        .filter(|r| !r.is_empty());
    let revision = match named {
        Some(r) => r,
        // No `refs/main`: a cache copied without it. One snapshot is unambiguous; several are
        // not, and picking one would seed whichever sorted first.
        None => {
            let mut found: Vec<String> = std::fs::read_dir(&snapshots)
                .map_err(|e| anyhow::anyhow!("reading {}: {e}", snapshots.display()))?
                .filter_map(|e| e.ok())
                .filter(|e| e.path().is_dir())
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect();
            found.sort();
            match found.as_slice() {
                [one] => one.clone(),
                [] => anyhow::bail!("{} holds no snapshot", snapshots.display()),
                many => anyhow::bail!(
                    "{} has no refs/main and {} snapshots ({}); copy the one you mean as a \
                     plain directory and pass that",
                    repo.display(),
                    many.len(),
                    many.join(", ")
                ),
            }
        }
    };
    Ok((snapshots.join(&revision), revision))
}

/// Copy `files` of `model` from `source` into `cache`, in the hub's layout. Returns the model's
/// folder in the cache.
///
/// Every file is checked before any is written, so a source missing one leaves the cache as it
/// was and the error names everything absent at once.
#[cfg_attr(not(feature = "vector-read"), allow(dead_code))]
fn seed(source: &Path, cache: &Path, model: &str, files: &[String]) -> Result<PathBuf> {
    let (dir, revision) = locate(source, model)?;
    let missing: Vec<&str> = files
        .iter()
        .filter(|f| !dir.join(f).is_file())
        .map(String::as_str)
        .collect();
    if !missing.is_empty() {
        anyhow::bail!(
            "{} is missing {} of {model}'s files: {}",
            dir.display(),
            missing.len(),
            missing.join(", ")
        );
    }

    let repo = cache.join(folder_name(model));
    let snapshot = repo.join("snapshots").join(&revision);
    for f in files {
        let to = snapshot.join(f);
        if let Some(parent) = to.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| anyhow::anyhow!("creating {}: {e}", parent.display()))?;
        }
        // `fs::copy` follows a symlink and copies what it points at, which is the point.
        std::fs::copy(dir.join(f), &to).map_err(|e| {
            anyhow::anyhow!("copying {} to {}: {e}", dir.join(f).display(), to.display())
        })?;
    }
    std::fs::create_dir_all(repo.join("refs"))?;
    std::fs::write(repo.join("refs/main"), &revision)?;
    Ok(repo)
}

#[cfg(test)]
mod tests {
    use super::*;

    const MODEL: &str = "Xenova/all-MiniLM-L6-v2";

    fn files() -> Vec<String> {
        required_files("onnx/model.onnx", &[])
    }

    fn write_files(dir: &Path) {
        for f in files() {
            let p = dir.join(&f);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, f.as_bytes()).unwrap();
        }
    }

    fn seeded_file(cache: &Path, revision: &str, file: &str) -> String {
        let repo = cache.join(folder_name(MODEL));
        assert_eq!(
            std::fs::read_to_string(repo.join("refs/main")).unwrap(),
            revision
        );
        std::fs::read_to_string(repo.join("snapshots").join(revision).join(file)).unwrap()
    }

    /// A plain directory of files is seeded under a revision yidam names.
    #[test]
    fn a_flat_directory_seeds_the_hub_layout() {
        let src = tempfile::tempdir().unwrap();
        let cache = tempfile::tempdir().unwrap();
        write_files(src.path());
        seed(src.path(), cache.path(), MODEL, &files()).unwrap();
        assert_eq!(
            seeded_file(cache.path(), SEEDED_REVISION, "onnx/model.onnx"),
            "onnx/model.onnx"
        );
    }

    /// A cache copied from another machine keeps its revision, whether it is passed whole or as
    /// the model's own folder, and its symlinks are copied through.
    #[test]
    fn a_copied_cache_keeps_its_revision() {
        let src = tempfile::tempdir().unwrap();
        let repo = src.path().join(folder_name(MODEL));
        let blobs = repo.join("blobs");
        let snap = repo.join("snapshots/abc123");
        std::fs::create_dir_all(&blobs).unwrap();
        write_files(&blobs);
        for f in files() {
            let link = snap.join(&f);
            std::fs::create_dir_all(link.parent().unwrap()).unwrap();
            #[cfg(unix)]
            std::os::unix::fs::symlink(blobs.join(&f), &link).unwrap();
            #[cfg(not(unix))]
            std::fs::copy(blobs.join(&f), &link).unwrap();
        }
        std::fs::create_dir_all(repo.join("refs")).unwrap();
        std::fs::write(repo.join("refs/main"), "abc123\n").unwrap();

        for source in [src.path().to_path_buf(), repo.clone()] {
            let cache = tempfile::tempdir().unwrap();
            seed(&source, cache.path(), MODEL, &files()).unwrap();
            let copied = cache
                .path()
                .join(folder_name(MODEL))
                .join("snapshots/abc123/tokenizer.json");
            assert!(
                !copied.symlink_metadata().unwrap().file_type().is_symlink(),
                "a link into the source would break when the source is removed"
            );
            assert_eq!(
                seeded_file(cache.path(), "abc123", "tokenizer.json"),
                "tokenizer.json"
            );
        }
    }

    /// A source missing a file writes nothing, and names every file it lacks.
    #[test]
    fn a_missing_file_writes_nothing() {
        let src = tempfile::tempdir().unwrap();
        let cache = tempfile::tempdir().unwrap();
        write_files(src.path());
        std::fs::remove_file(src.path().join("config.json")).unwrap();
        std::fs::remove_file(src.path().join("onnx/model.onnx")).unwrap();
        let err = seed(src.path(), cache.path(), MODEL, &files())
            .unwrap_err()
            .to_string();
        assert!(err.contains("missing 2"), "{err}");
        assert!(
            err.contains("onnx/model.onnx") && err.contains("config.json"),
            "{err}"
        );
        assert!(!cache.path().join(folder_name(MODEL)).exists());
    }

    /// Several snapshots and no `refs/main` is a choice this command will not make.
    #[test]
    fn an_ambiguous_cache_is_refused() {
        let src = tempfile::tempdir().unwrap();
        let repo = src.path().join(folder_name(MODEL));
        std::fs::create_dir_all(repo.join("snapshots/aaa")).unwrap();
        std::fs::create_dir_all(repo.join("snapshots/bbb")).unwrap();
        let err = locate(src.path(), MODEL).unwrap_err().to_string();
        assert!(err.contains("2 snapshots"), "{err}");
    }

    /// `$HF_HOME` is where fastembed reads, so it is where a fetch writes.
    #[cfg(feature = "vector-read")]
    #[test]
    fn hf_home_wins_over_the_configured_cache() {
        let runtime = crate::embedding::Runtime {
            cache_dir: Some(PathBuf::from("/corpus/.fastembed_cache")),
            ..Default::default()
        };
        assert_eq!(
            effective_cache(&runtime, Some("/hf".into())),
            PathBuf::from("/hf")
        );
        assert_eq!(
            effective_cache(&runtime, None),
            PathBuf::from("/corpus/.fastembed_cache")
        );
    }
}
