//! Bytes from an address — the one part of a fetch that needs a network.
//!
//! # How small this is, on purpose
//!
//! Everything interesting about following a catalog address happens in
//! [`super::location`], which is ungated: deciding what would be fetched, refusing a path
//! that leaves the repository, refusing a template nothing bound. What is left here is a
//! function from a resolved URL to a file on disk.
//!
//! That split is #460's stated mitigation for the failure mode it calls the *feature-gate
//! blind spot* — "anything gated takes gated facts as arguments". The gated half takes a
//! `String` and a `Path` and returns bytes, which is about as little judgement as a function
//! can carry, so the code a pull request does not compile is also the code a pull request has
//! least reason to.
//!
//! # Why it is a feature at all, and why that feature is in `default`
//!
//! `vault-s3` settled both halves of this and the reasoning transfers unchanged. It is a
//! feature because `--no-default-features --features reports` must keep meaning something and
//! a reports-only binary has no business making outbound requests. It is in the **default
//! set** because anything outside `default` ships code no pull request has built — which is
//! precisely how a `forbid(unsafe_code)` change went green through review and red on a job
//! nobody was watching.
//!
//! `reqwest` and `tokio` are already in the default build through `tonpa` and `vault-s3`, so
//! the marginal cost of this feature on the released binary is zero crates.

use std::path::Path;

use anyhow::Result;

/// What a transport learned about the bytes beyond the bytes themselves.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Fetched {
    /// The server's `Content-Type`, with any `; charset=…` parameter dropped.
    ///
    /// Recorded so a reader knows what an artifact is without fetching it, which is the
    /// reason `CatalogArtifact::media_type` exists. `None` where the server said nothing —
    /// guessing from the URL's extension would put a claim in a committed record that no
    /// server ever made.
    pub media_type: Option<String>,
}

/// `GET` a URL into `dest`.
///
/// Writes `dest` only on success, so a caller finding a file there can rely on it being
/// complete — the same contract `vault::Store::get` states, for the same reason.
#[cfg(feature = "catalog-fetch")]
pub fn get(url: &str, dest: &Path) -> Result<Fetched> {
    use anyhow::{bail, Context};
    use std::io::Write;

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("starting the async runtime for a catalog fetch")?;

    runtime.block_on(async {
        let client = reqwest::Client::builder()
            // Named, and named as this command rather than as the binary. A source that
            // rate-limits or blocks automated retrieval is entitled to see what is asking,
            // and several of the archives a corpus catalogues say so in their terms.
            .user_agent(concat!("yidam-catalog/", env!("CARGO_PKG_VERSION")))
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
        let media_type = resp
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .map(|v| v.split(';').next().unwrap_or(v).trim().to_string())
            .filter(|v| !v.is_empty());

        let body = resp
            .bytes()
            .await
            .with_context(|| format!("reading the response body from {url}"))?;

        // Through a temporary in the destination's own directory, then renamed. The reasoning
        // is `vault::Cache::put_file`'s and is not repeated: `rename` is atomic only within a
        // filesystem, and a half-written file under a name a digest asserts is the one
        // failure content addressing exists to make impossible.
        let dir = dest.parent().unwrap_or(Path::new("."));
        std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
        let tmp = dir.join(format!(".catalog-fetch.{}.part", std::process::id()));
        {
            let mut f = std::fs::File::create(&tmp)
                .with_context(|| format!("creating {}", tmp.display()))?;
            f.write_all(&body)
                .with_context(|| format!("writing {}", tmp.display()))?;
        }
        std::fs::rename(&tmp, dest).with_context(|| {
            format!("moving {} into place at {}", tmp.display(), dest.display())
        })?;
        Ok(Fetched { media_type })
    })
}

/// The refusal a build without the feature gives.
///
/// It names the feature and the command that installs it, because a message saying only "not
/// supported" sends a reader to the issue tracker for something a flag fixes. `cmd/export.rs`
/// words its three the same way.
#[cfg(not(feature = "catalog-fetch"))]
pub fn get(url: &str, _dest: &Path) -> Result<Fetched> {
    anyhow::bail!(
        "this build cannot fetch {url} — `catalog-fetch` is not compiled in.\n  \
         It is in the default feature set, so this is a `--no-default-features` build: \
         reinstall with `cargo install yidam --features catalog-fetch`.\n  \
         `kind: file` locations need no network and work in every build."
    )
}

/// Copy a file the repository already holds into `dest`.
///
/// Ungated, and the whole of what a `kind: file` location needs. It is separate from a plain
/// `fs::copy` at the call site so that both arms of a fetch — the local one and the network
/// one — land bytes the same way and report a missing source in the same words.
pub fn read_local(src: &Path, dest: &Path) -> Result<Fetched> {
    use anyhow::Context;
    if !src.is_file() {
        anyhow::bail!(
            "{} does not exist — the entry declares it as a `kind: file` location, and \
             nothing has put it there",
            src.display()
        );
    }
    let dir = dest.parent().unwrap_or(Path::new("."));
    std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    std::fs::copy(src, dest)
        .with_context(|| format!("copying {} to {}", src.display(), dest.display()))?;
    Ok(Fetched { media_type: None })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_local_file_is_copied_and_claims_no_media_type() {
        let tmp = tempfile::tempdir().unwrap();
        let src = tmp.path().join("registry.csv");
        std::fs::write(&src, b"a,b\n1,2\n").unwrap();
        let dest = tmp.path().join("out/staged");

        assert_eq!(
            read_local(&src, &dest).unwrap(),
            Fetched { media_type: None }
        );
        assert_eq!(std::fs::read(&dest).unwrap(), b"a,b\n1,2\n");
    }

    /// A declared location that is not there is an ordinary state — a corpus can catalogue a
    /// source before obtaining it — so it must report the path rather than panic.
    #[test]
    fn a_missing_local_file_names_the_path_it_looked_for() {
        let tmp = tempfile::tempdir().unwrap();
        let err = read_local(&tmp.path().join("absent.csv"), &tmp.path().join("out")).unwrap_err();
        assert!(err.to_string().contains("absent.csv"), "{err}");
        assert!(err.to_string().contains("does not exist"), "{err}");
    }

    /// The refusal a `--no-default-features` build gives has to name the flag that fixes it.
    #[cfg(not(feature = "catalog-fetch"))]
    #[test]
    fn a_build_without_the_feature_names_it() {
        let err = get("https://x/y", Path::new("/tmp/x"))
            .unwrap_err()
            .to_string();
        assert!(err.contains("catalog-fetch"), "{err}");
        assert!(err.contains("kind: file"), "{err}");
    }
}
