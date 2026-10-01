//! `yidam serve --mcp --http --bundle` — serve an exported `.yiz`, not a checkout (#1238).
//!
//! A pod that serves a corpus has no business holding its git remote: it would need a
//! credential to clone, and everything it then served would be whatever the branch said at
//! the moment the pod started. A bundle is the corpus at one commit, addressed by its
//! digest, and the vault already holds it — the cluster run put it there. So the pod is
//! pinned by digest and fetches nothing else.
//!
//! # Read-only by construction
//!
//! The bundle is unpacked into scratch, which is not a git repository and holds no
//! `.yidam/config.toml`. So `[serve] act` cannot be declared, no write tool is listed, and a
//! write would have no branch to land on and nowhere it would last past the pod. That is
//! not a setting a deployment could get wrong; there is nothing to set.
//!
//! # What the bundle says about itself wins
//!
//! The commit and the domain come from `manifest.yml`. Read from the scratch directory they
//! would be wrong twice: there is no git there, so the commit is unknown, and the domain would
//! be read from a genesis commit that does not exist.

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};

use crate::cmd::cluster::VaultArgs;
use crate::cmd::run::exec::Scratch;
use crate::vault::ContentHash;

/// A bundle unpacked into scratch, and what its manifest says it is.
///
/// The directory lives as long as this value, so the caller holds it for the server's life:
/// a vector index is read lazily, on the first query, from under `root`.
pub(crate) struct Unpacked {
    pub(crate) root: PathBuf,
    pub(crate) commit: Option<String>,
    pub(crate) domain: Option<String>,
    _scratch: Scratch,
}

/// What `--bundle` names: a digest in the vault, or a file on disk.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Source {
    Digest(ContentHash),
    Path(PathBuf),
}

/// Read `--bundle`. A digest is 64 hex digits, optionally after `sha256:`; anything else is a
/// path. A digest never names a file, so the two cannot be confused.
pub(crate) fn source(arg: &str) -> Source {
    let bare = arg.strip_prefix("sha256:").unwrap_or(arg);
    match ContentHash::parse(bare) {
        Ok(hash) => Source::Digest(hash),
        Err(_) => Source::Path(PathBuf::from(arg)),
    }
}

/// Fetch, verify and unpack the bundle `--bundle` names.
///
/// A digest is fetched from the vault and its bytes hashed before anything is unpacked: the
/// pin is the digest, and a vault that answered with other bytes would otherwise be served
/// as the pinned corpus. A path is read as it is — the operator named the file — and a vault
/// beside it is refused, since nothing would be read from it.
pub(crate) fn unpack(arg: &str, vault: Option<&VaultArgs>) -> Result<Unpacked> {
    let bytes = match (source(arg), vault) {
        (Source::Digest(hash), Some(vault)) => fetch(&hash, vault)?,
        (Source::Digest(hash), None) => bail!(
            "`--bundle {hash}` is a digest, and a digest is fetched from a vault. Pass \
             `--vault-url`, or name the `.yiz` file itself."
        ),
        (Source::Path(path), None) => std::fs::read(&path)
            .with_context(|| format!("cannot read `--bundle {}`", path.display()))?,
        (Source::Path(path), Some(_)) => bail!(
            "`--bundle {}` is a path, so `--vault-url` would be read for nothing. Pass a \
             digest to fetch from the vault, or drop `--vault-url`.",
            path.display()
        ),
    };
    let scratch = Scratch::new("serve")?;
    let unpacked = scratch.path().join("bundle");
    let (manifest, unreadable) = crate::deps::extract_bundle(&bytes, &unpacked.join(".yidam"))
        .context("unpacking the bundle")?;
    if let Some(reason) = unreadable {
        eprintln!("[warn] the bundle's manifest.yml did not decode ({reason}); serving it without a commit or a domain");
    }
    // Named for its domain, which is what a corpus with no genesis commit is called; a
    // domain that is not one plain path component keeps the neutral name.
    let root = match manifest.domain.as_deref().filter(|d| is_component(d)) {
        Some(domain) => {
            let named = scratch.path().join(domain);
            std::fs::rename(&unpacked, &named)
                .with_context(|| format!("naming the bundle {}", named.display()))?;
            named
        }
        None => unpacked,
    };
    Ok(Unpacked {
        root,
        commit: manifest.commit,
        domain: manifest.domain,
        _scratch: scratch,
    })
}

fn is_component(s: &str) -> bool {
    let mut parts = Path::new(s).components();
    matches!(
        (parts.next(), parts.next()),
        (Some(std::path::Component::Normal(_)), None)
    )
}

fn fetch(hash: &ContentHash, vault: &VaultArgs) -> Result<Vec<u8>> {
    let store = vault.open()?;
    if !store
        .has(hash)
        .with_context(|| format!("asking {} for {hash}", store.describe()))?
    {
        bail!("{} holds no bundle {hash}", store.describe());
    }
    let scratch = Scratch::new("fetch")?;
    let file = scratch.path().join("bundle.yiz");
    store
        .get(hash, &file)
        .with_context(|| format!("fetching {hash} from {}", store.describe()))?;
    let bytes = std::fs::read(&file).with_context(|| format!("reading {}", file.display()))?;
    let got = crate::deps::sha256_hex(&bytes);
    if got != hash.to_string() {
        bail!(
            "{} answered {hash} with bytes that hash to {got}; refusing to serve them as the \
             pinned bundle",
            store.describe()
        );
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIGEST: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    #[test]
    fn a_digest_is_a_digest_and_anything_else_is_a_path() {
        assert!(matches!(source(DIGEST), Source::Digest(_)));
        assert!(matches!(
            source(&format!("sha256:{DIGEST}")),
            Source::Digest(_)
        ));
        for path in ["streamflow.yiz", "./0123", "/var/yidam/bundle.yiz"] {
            assert_eq!(source(path), Source::Path(PathBuf::from(path)), "{path}");
        }
    }

    #[test]
    fn a_digest_without_a_vault_is_refused_by_name() {
        let err = unpack(DIGEST, None).err().unwrap().to_string();
        assert!(err.contains("--vault-url"), "{err}");
    }

    #[test]
    fn a_domain_names_the_directory_only_when_it_is_one_component() {
        assert!(is_component("streamflow"));
        for d in ["", "..", "a/b", "/abs"] {
            assert!(!is_component(d), "{d}");
        }
    }
}
