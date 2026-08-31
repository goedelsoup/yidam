//! What a repository declares about its vaults, and the one it is allowed to declare today.
//!
//! # The table is plural before it needs to be
//!
//! This ships accepting exactly one vault, and it ships as `[vault.<name>]` rather than
//! `[vault]`. That is not indecision. The two are different config shapes, `.yidam/config.toml`
//! is committed, and derived repositories adopt configuration quickly — so a corpus that wrote
//! the singular form is a corpus whose config a later release breaks. The plural table costs
//! nothing today and cannot be retrofitted quietly.
//!
//! What this does *not* do is honour a second entry. A config declaring two vaults is refused,
//! by name, saying what is missing. The alternative — accepting the declaration and routing
//! everything to the first — is the isolation failure the whole feature exists to prevent,
//! arriving as a success.
//!
//! # `audience` is required and nothing can check it
//!
//! Every vault says who can read it, in prose. That is `.yidam/publishable`'s argument applied
//! to a store, and `docs/sharing-derivations.md` states it in the form this inherits:
//!
//! > The file is not a security control — anyone who can push a tag can add a file. It is a
//! > statement of intent that lives in the repository and outlasts the person who made it.
//!
//! So the field is unvalidatable by construction and required anyway. What is enforced is that
//! somebody wrote one, which is the only part a program can be responsible for.

use std::collections::BTreeMap;

use anyhow::{bail, Result};
use serde::Deserialize;

/// The one vault name this release accepts.
///
/// Also the name the `AWS_*` credential fallback will be scoped to when the S3 transport
/// lands, which is the second reason the single vault is not allowed an arbitrary name: a
/// corpus that called its only store `archive` would silently lose that fallback later.
pub const ONLY_VAULT: &str = "default";

/// One store a repository can put bytes in.
///
/// Deserialized permissively at the leaves — `region`, `endpoint` and `path_style` mean
/// nothing to the `file://` backend and are carried so a config written for S3 is not
/// rejected by a build that cannot yet reach one. An unknown *key*, though, is refused: a
/// misspelled `endpiont` that parses silently is a vault pointing somewhere nobody intended.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct VaultConfig {
    /// Where the store is. `file:///…` today; `s3://bucket/prefix` when the transport lands.
    pub url: String,
    /// Who can read this store, in prose. Required; see the module header.
    #[serde(default)]
    pub audience: Option<String>,
    /// SigV4 signing scope. Unused by `file://`, carried for the S3 backend.
    #[serde(default)]
    pub region: Option<String>,
    /// A non-AWS endpoint — MinIO, Ceph, R2. Absent means AWS.
    #[serde(default)]
    pub endpoint: Option<String>,
    /// Path-style addressing. Defaults to true when `endpoint` is set.
    #[serde(default)]
    pub path_style: Option<bool>,
}

impl VaultConfig {
    /// The audience, having established there is one.
    pub fn audience(&self) -> &str {
        self.audience.as_deref().unwrap_or("")
    }
}

/// The vault a repository has declared, if it has declared one.
///
/// `Ok(None)` is the common case and is not a degraded one: a corpus with no vault is a
/// corpus that keeps its artifacts locally, which is every corpus until somebody configures a
/// store.
pub fn resolve(vaults: &BTreeMap<String, VaultConfig>) -> Result<Option<(&str, &VaultConfig)>> {
    let names: Vec<&str> = vaults.keys().map(String::as_str).collect();
    match names.as_slice() {
        [] => Ok(None),
        [one] => {
            let cfg = &vaults[*one];
            if *one != ONLY_VAULT {
                bail!(
                    "`.yidam/config.toml` declares a vault named `{one}`, and yidam {} \
                     supports one vault named `{ONLY_VAULT}`.\n  \
                     Rename the section to `[vault.{ONLY_VAULT}]`. Named vaults — with \
                     per-vault credentials and routing — are specified in RFC-0023 and not \
                     yet built.",
                    env!("CARGO_PKG_VERSION")
                );
            }
            check_audience(one, cfg)?;
            Ok(Some((*one, cfg)))
        }
        many => bail!(
            "`.yidam/config.toml` declares {} vaults ({}), and yidam {} supports one, named \
             `{ONLY_VAULT}`.\n  \
             This is refused rather than resolved to the first, because routing every \
             artifact into one of several declared stores is the failure separate stores \
             exist to prevent. Keep `[vault.{ONLY_VAULT}]` and remove the rest until named \
             vaults land; they are specified in RFC-0023.",
            many.len(),
            many.iter()
                .map(|n| format!("`{n}`"))
                .collect::<Vec<_>>()
                .join(", "),
            env!("CARGO_PKG_VERSION")
        ),
    }
}

fn check_audience(name: &str, cfg: &VaultConfig) -> Result<()> {
    if cfg.audience().trim().is_empty() {
        bail!(
            "vault `{name}` does not say who can read it.\n  \
             Add an `audience` to `[vault.{name}]` in `.yidam/config.toml` — one sentence \
             naming who has access to this store and why that is acceptable. Nothing checks \
             what it says; the point is that somebody wrote it down before artifacts started \
             going there."
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(toml_src: &str) -> Result<BTreeMap<String, VaultConfig>> {
        #[derive(Deserialize)]
        struct Wrapper {
            #[serde(default)]
            vault: BTreeMap<String, VaultConfig>,
        }
        Ok(toml::from_str::<Wrapper>(toml_src)?.vault)
    }

    const ONE: &str = r#"
[vault.default]
url = "file:///mnt/archive/yidam"
audience = "Anyone who can read this corpus."
"#;

    #[test]
    fn a_repository_with_no_vault_has_no_vault_and_that_is_not_an_error() {
        let vaults = parse("").unwrap();
        assert!(resolve(&vaults).unwrap().is_none());
    }

    #[test]
    fn the_one_supported_vault_resolves_with_its_audience() {
        let vaults = parse(ONE).unwrap();
        let (name, cfg) = resolve(&vaults).unwrap().expect("one vault resolves");
        assert_eq!(name, "default");
        assert_eq!(cfg.url, "file:///mnt/archive/yidam");
        assert_eq!(cfg.audience(), "Anyone who can read this corpus.");
    }

    /// The load-bearing refusal. Two vaults must not resolve to the first one silently.
    #[test]
    fn two_vaults_are_refused_by_name_rather_than_resolved_to_the_first() {
        let vaults = parse(
            r#"
[vault.default]
url = "s3://a/yidam"
audience = "everyone"
[vault.sources]
url = "s3://b/yidam"
audience = "the sangha"
"#,
        )
        .unwrap();
        let err = resolve(&vaults)
            .expect_err("two vaults must not resolve")
            .to_string();
        assert!(err.contains("`default`"), "names the first: {err}");
        assert!(err.contains("`sources`"), "names the second: {err}");
        assert!(
            err.contains("RFC-0023"),
            "says where the design lives: {err}"
        );
    }

    #[test]
    fn a_single_vault_under_another_name_is_refused_with_the_rename() {
        let vaults = parse(
            r#"
[vault.archive]
url = "file:///mnt/archive"
audience = "me"
"#,
        )
        .unwrap();
        let err = resolve(&vaults).unwrap_err().to_string();
        assert!(err.contains("`archive`"), "{err}");
        assert!(err.contains("[vault.default]"), "carries the fix: {err}");
    }

    #[test]
    fn a_vault_that_does_not_say_who_can_read_it_is_refused() {
        for src in [
            "[vault.default]\nurl = \"file:///a\"\n",
            "[vault.default]\nurl = \"file:///a\"\naudience = \"   \"\n",
        ] {
            let vaults = parse(src).unwrap();
            let err = resolve(&vaults).unwrap_err().to_string();
            assert!(err.contains("audience"), "{err}");
        }
    }

    /// A misspelled key that parses is a vault pointing somewhere nobody intended.
    #[test]
    fn an_unknown_key_is_refused_rather_than_ignored() {
        let err = parse(
            r#"
[vault.default]
url = "file:///a"
audience = "me"
endpiont = "https://typo"
"#,
        )
        .unwrap_err()
        .to_string();
        assert!(err.contains("endpiont"), "{err}");
    }

    /// A config written for S3 must survive a build that cannot reach one, so the fields the
    /// `file://` backend ignores still have to parse.
    #[test]
    fn the_s3_fields_parse_in_a_build_that_cannot_use_them() {
        let vaults = parse(
            r#"
[vault.default]
url = "s3://corpus-artifacts/yidam"
region = "us-east-1"
endpoint = "https://s3.example.net"
path_style = true
audience = "the sangha"
"#,
        )
        .unwrap();
        let (_, cfg) = resolve(&vaults).unwrap().unwrap();
        assert_eq!(cfg.region.as_deref(), Some("us-east-1"));
        assert_eq!(cfg.path_style, Some(true));
    }
}
