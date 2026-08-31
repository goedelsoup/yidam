//! Where a vault's credentials come from, and the one place ambient ones are allowed.
//!
//! # The environment, and nothing else
//!
//! `.yidam/config.toml` is committed. It carries the store — url, region, endpoint, audience
//! — and never a secret. This repository has already found an untracked `.env` that any of
//! its own prescribed `git add -A` steps would have staged; a vault must not add a second
//! route by which a key gets committed.
//!
//! # Why `AWS_*` is honoured for `default` and no other vault
//!
//! The asymmetry is the point, and it is worth stating because it reads as an inconsistency.
//!
//! An ordinary AWS environment is plausibly already configured for the store a repository
//! publishes its own output to, so making `default` work with the credentials already in a
//! shell is a convenience with no boundary crossed. A **second** vault exists precisely
//! because its readership differs — that is the only reason to declare one — so letting it
//! silently inherit whatever `AWS_ACCESS_KEY_ID` happens to be set is the failure the
//! boundary was drawn to prevent, arriving as a success.
//!
//! So a vault that wants isolation has to say which keys it uses. `doctor` additionally warns
//! when two vaults resolve to the same credentials: legal, and also exactly what a
//! half-finished isolation setup looks like.

use anyhow::{bail, Result};

use super::sigv4::Credentials;

/// The vault whose name licenses the `AWS_*` fallback.
const AMBIENT_VAULT: &str = super::config::DEFAULT_VAULT;

/// The environment variable prefix for a vault's own credentials.
///
/// `default` → `YIDAM_VAULT_DEFAULT_`. Hyphens become underscores, because a shell cannot
/// export a variable with a hyphen in it and a vault named `licensed-sources` is otherwise
/// unconfigurable.
pub fn env_prefix(vault: &str) -> String {
    format!(
        "YIDAM_VAULT_{}_",
        vault.to_ascii_uppercase().replace('-', "_")
    )
}

/// Resolve the credentials for one vault.
///
/// `lookup` is passed rather than read so this is testable without setting process-wide
/// variables, which parallel tests cannot do independently. Production hands it
/// [`std::env::var`].
pub fn resolve(vault: &str, lookup: impl Fn(&str) -> Option<String>) -> Result<Credentials> {
    let get = |k: &str| lookup(k).filter(|v| !v.trim().is_empty());
    let prefix = env_prefix(vault);

    let own_id = get(&format!("{prefix}ACCESS_KEY_ID"));
    let own_secret = get(&format!("{prefix}SECRET_ACCESS_KEY"));

    // A half-set pair is worth its own message. Falling back to `AWS_*` because only the
    // secret was exported would use credentials the operator did not choose, and the
    // resulting `403` says nothing about which key was tried.
    if own_id.is_some() != own_secret.is_some() {
        bail!(
            "vault `{vault}` has only half its credentials in the environment.\n  \
             Set both {prefix}ACCESS_KEY_ID and {prefix}SECRET_ACCESS_KEY, or neither."
        );
    }

    if let (Some(id), Some(secret)) = (own_id, own_secret) {
        return Ok(Credentials {
            access_key_id: id,
            secret_access_key: secret,
            session_token: get(&format!("{prefix}SESSION_TOKEN")),
        });
    }

    if vault == AMBIENT_VAULT {
        if let (Some(id), Some(secret)) = (get("AWS_ACCESS_KEY_ID"), get("AWS_SECRET_ACCESS_KEY")) {
            return Ok(Credentials {
                access_key_id: id,
                secret_access_key: secret,
                session_token: get("AWS_SESSION_TOKEN"),
            });
        }
        bail!(
            "no credentials for vault `{vault}`.\n  \
             Set {prefix}ACCESS_KEY_ID and {prefix}SECRET_ACCESS_KEY, or AWS_ACCESS_KEY_ID \
             and AWS_SECRET_ACCESS_KEY.\n  \
             Credentials come from the environment only — `.yidam/config.toml` is committed \
             and must never carry one."
        );
    }

    bail!(
        "no credentials for vault `{vault}`.\n  \
         Set {prefix}ACCESS_KEY_ID and {prefix}SECRET_ACCESS_KEY.\n  \
         `AWS_*` is honoured only for the vault named `{AMBIENT_VAULT}`: a second vault \
         exists because its readership differs, and inheriting whatever credentials happen \
         to be in the environment is the failure that boundary was drawn to prevent."
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn env(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let m: HashMap<String, String> = pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        move |k: &str| m.get(k).cloned()
    }

    #[test]
    fn a_vaults_own_variables_are_used() {
        let c = resolve(
            "sources",
            env(&[
                ("YIDAM_VAULT_SOURCES_ACCESS_KEY_ID", "AKIA_OWN"),
                ("YIDAM_VAULT_SOURCES_SECRET_ACCESS_KEY", "s3cret"),
                ("YIDAM_VAULT_SOURCES_SESSION_TOKEN", "tok"),
            ]),
        )
        .unwrap();
        assert_eq!(c.access_key_id, "AKIA_OWN");
        assert_eq!(c.session_token.as_deref(), Some("tok"));
    }

    #[test]
    fn a_hyphenated_vault_name_becomes_an_exportable_prefix() {
        assert_eq!(
            env_prefix("licensed-sources"),
            "YIDAM_VAULT_LICENSED_SOURCES_"
        );
        let c = resolve(
            "licensed-sources",
            env(&[
                ("YIDAM_VAULT_LICENSED_SOURCES_ACCESS_KEY_ID", "A"),
                ("YIDAM_VAULT_LICENSED_SOURCES_SECRET_ACCESS_KEY", "B"),
            ]),
        )
        .unwrap();
        assert_eq!(c.access_key_id, "A");
    }

    #[test]
    fn default_falls_back_to_the_ambient_aws_variables() {
        let c = resolve(
            "default",
            env(&[
                ("AWS_ACCESS_KEY_ID", "AKIA_AMBIENT"),
                ("AWS_SECRET_ACCESS_KEY", "s3cret"),
            ]),
        )
        .unwrap();
        assert_eq!(c.access_key_id, "AKIA_AMBIENT");
    }

    /// A vault's own variables beat the ambient ones even for `default` — otherwise there
    /// would be no way to point `default` at something other than the shell's AWS identity.
    #[test]
    fn a_vaults_own_variables_outrank_the_ambient_ones() {
        let c = resolve(
            "default",
            env(&[
                ("YIDAM_VAULT_DEFAULT_ACCESS_KEY_ID", "AKIA_OWN"),
                ("YIDAM_VAULT_DEFAULT_SECRET_ACCESS_KEY", "s"),
                ("AWS_ACCESS_KEY_ID", "AKIA_AMBIENT"),
                ("AWS_SECRET_ACCESS_KEY", "s"),
            ]),
        )
        .unwrap();
        assert_eq!(c.access_key_id, "AKIA_OWN");
    }

    /// **The isolation rule.** A second vault must not pick up whatever is in the shell.
    #[test]
    fn a_non_default_vault_does_not_inherit_the_ambient_variables() {
        let err = resolve(
            "sources",
            env(&[
                ("AWS_ACCESS_KEY_ID", "AKIA_AMBIENT"),
                ("AWS_SECRET_ACCESS_KEY", "s3cret"),
            ]),
        )
        .map(|_| ())
        .unwrap_err()
        .to_string();
        assert!(err.contains("YIDAM_VAULT_SOURCES_ACCESS_KEY_ID"), "{err}");
        assert!(
            err.contains("only for the vault named `default`"),
            "says why: {err}"
        );
    }

    /// Half a pair is its own diagnosis. Falling through to `AWS_*` here would use
    /// credentials nobody chose and report a 403 that names nothing.
    #[test]
    fn half_a_credential_pair_is_reported_rather_than_fallen_through() {
        let err = resolve(
            "default",
            env(&[
                ("YIDAM_VAULT_DEFAULT_ACCESS_KEY_ID", "AKIA_OWN"),
                ("AWS_ACCESS_KEY_ID", "AKIA_AMBIENT"),
                ("AWS_SECRET_ACCESS_KEY", "s"),
            ]),
        )
        .map(|_| ())
        .unwrap_err()
        .to_string();
        assert!(err.contains("half its credentials"), "{err}");
    }

    /// An exported-but-empty variable is not a credential. Treating `AWS_ACCESS_KEY_ID=""` as
    /// one produces a signature with an empty key id and a server error about nothing.
    #[test]
    fn empty_variables_are_not_credentials() {
        let err = resolve(
            "default",
            env(&[("AWS_ACCESS_KEY_ID", "  "), ("AWS_SECRET_ACCESS_KEY", "")]),
        )
        .map(|_| ())
        .unwrap_err()
        .to_string();
        assert!(err.contains("no credentials"), "{err}");
    }

    #[test]
    fn with_nothing_set_the_message_names_both_variables_it_wants() {
        let err = resolve("default", env(&[]))
            .map(|_| ())
            .unwrap_err()
            .to_string();
        assert!(err.contains("YIDAM_VAULT_DEFAULT_ACCESS_KEY_ID"), "{err}");
        assert!(err.contains("AWS_ACCESS_KEY_ID"), "{err}");
        assert!(
            err.contains("committed"),
            "says why not a config file: {err}"
        );
    }
}
