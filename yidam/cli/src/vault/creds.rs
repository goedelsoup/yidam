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
//!
//! # Roles, under the same rule (#1234)
//!
//! A pod on EKS is given a role rather than a key: `AWS_ROLE_ARN` and a token file the platform
//! projects, exchanged in [`super::web_identity`]. Workload identity is ambient by nature, since
//! every pod under a service account gets it, so it follows the asymmetry above exactly:
//! `AWS_ROLE_ARN` stands in for `default` only, and a second vault assumes a role only when its
//! own `YIDAM_VAULT_<NAME>_ROLE_ARN` and `…_WEB_IDENTITY_TOKEN_FILE` say which. RFC-0023's
//! amendment of 2026-09-30 records why a token file named by the environment counts as the
//! environment.

use std::path::PathBuf;
use std::sync::Mutex;

use anyhow::{bail, Result};

use super::sigv4::Credentials;
use super::web_identity::{Lease, WebIdentity};

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

/// One place credentials may be looked for, and what may stand in for them.
///
/// A type rather than two copies of the same environment walk. The S3 Vectors transport signs
/// the same way against a different service and needs the same lookup — but under its own
/// prefix and with its own answer to the ambient question — and the second copy is the one
/// that would have drifted.
pub struct Scope {
    /// Environment prefix, e.g. `YIDAM_VAULT_SOURCES_` or `YIDAM_INDEX_`.
    pub prefix: String,
    /// How to name this scope in a message: *vault `sources`*, *the remote index*.
    pub label: String,
    /// Whether `AWS_*` may stand in when this scope's own variables are unset.
    pub ambient: bool,
    /// Why not, when it may not. Spliced into the refusal so the rule explains itself where it
    /// bites rather than only in this file's header.
    pub isolation_note: Option<String>,
}

/// The scope for one vault. `AWS_*` is honoured only for [`AMBIENT_VAULT`] — see the header.
pub fn vault_scope(vault: &str) -> Scope {
    Scope {
        prefix: env_prefix(vault),
        label: format!("vault `{vault}`"),
        ambient: vault == AMBIENT_VAULT,
        isolation_note: (vault != AMBIENT_VAULT).then(|| {
            format!(
                "`AWS_*`, `AWS_ROLE_ARN` included, is honoured only for the vault named \
                 `{AMBIENT_VAULT}`: a second vault \
                 exists because its readership differs, and inheriting whatever credentials \
                 happen to be in the environment is the failure that boundary was drawn to \
                 prevent."
            )
        }),
    }
}

/// The scope for the remote vector index declared in `[index.remote]`.
///
/// # Why the ambient fallback is licensed here
///
/// It reads as an inconsistency with the vault rule above, so it is worth stating. That rule
/// withholds `AWS_*` from a *second* vault, and the reason is specific: a second vault exists
/// **because its readership differs**, so silently inheriting the shell's identity is the
/// boundary failing at the moment it was meant to hold.
///
/// A corpus declares at most one remote index. There is no second one for it to be confused
/// with, so there is no boundary for inheritance to cross — and an AWS environment already
/// configured for the account a corpus publishes to is exactly the common case. `YIDAM_INDEX_*`
/// still wins where it is set, which is how a corpus whose vectors belong to a narrower
/// audience than its shell says so.
pub fn index_scope() -> Scope {
    Scope {
        prefix: "YIDAM_INDEX_".to_string(),
        label: "the remote index".to_string(),
        ambient: true,
        isolation_note: None,
    }
}

/// Resolve the credentials for one vault.
///
/// `lookup` is passed rather than read so this is testable without setting process-wide
/// variables, which parallel tests cannot do independently. Production hands it
/// [`std::env::var`].
pub fn resolve(vault: &str, lookup: impl Fn(&str) -> Option<String>) -> Result<Provider> {
    resolve_scope(&vault_scope(vault), lookup)
}

/// Where one scope's credentials come from: keys in the environment, or a role to assume with
/// a token the platform projected (#1234).
pub enum Source {
    Keys {
        creds: Credentials,
        /// The variable prefix they were read under: `YIDAM_VAULT_SOURCES_`, or `AWS_`.
        prefix: String,
    },
    WebIdentity(WebIdentity),
}

/// A scope's credentials as of the moment they are signed with.
///
/// Keys are what they are. A web identity's are a lease that ends, so a store holds this and
/// asks it per request rather than holding [`Credentials`]: a push or a server can outlive the
/// hour a lease lasts. The lease is behind a lock so concurrent requests near expiry exchange
/// once rather than once each.
pub struct Provider {
    source: Source,
    lease: Mutex<Option<Lease>>,
    exchange: fn(&WebIdentity) -> Result<Lease>,
    now: fn() -> u64,
}

impl Provider {
    fn new(source: Source) -> Self {
        Self {
            source,
            lease: Mutex::new(None),
            exchange: super::web_identity::exchange,
            now: unix_now,
        }
    }

    #[cfg(test)]
    pub fn source(&self) -> &Source {
        &self.source
    }

    /// The credentials to sign with now. For a web identity, this exchanges the token when
    /// there is no lease or the one held is inside [`REFRESH_BEFORE`] of its end.
    ///
    /// [`REFRESH_BEFORE`]: super::web_identity::REFRESH_BEFORE
    pub fn credentials(&self) -> Result<Credentials> {
        let w = match &self.source {
            Source::Keys { creds, .. } => return Ok(creds.clone()),
            Source::WebIdentity(w) => w,
        };
        // A poisoned lock means another request panicked mid-exchange. The lease it guards is
        // either absent or whole, so it is still safe to read.
        let mut held = self.lease.lock().unwrap_or_else(|p| p.into_inner());
        let now = (self.now)();
        if let Some(lease) = held.as_ref().filter(|l| l.fresh(now)) {
            return Ok(lease.creds.clone());
        }
        let lease = (self.exchange)(w)?;
        let creds = lease.creds.clone();
        *held = Some(lease);
        Ok(creds)
    }

    /// Where the credentials come from, in words for `doctor`. Names variables, never values.
    pub fn origin(&self) -> String {
        match &self.source {
            Source::Keys { prefix, .. } => format!("{prefix}ACCESS_KEY_ID"),
            Source::WebIdentity(w) => format!("web identity via {}", w.role_var),
        }
    }

    /// Which principal these credentials name, for comparing two scopes without printing a
    /// secret: the access key id, which travels in every request in plaintext, or the role.
    /// A web identity's lease keys change every hour, so the role is the stable answer.
    pub fn principal(&self) -> String {
        match &self.source {
            Source::Keys { creds, .. } => creds.access_key_id.clone(),
            Source::WebIdentity(w) => w.role_arn.clone(),
        }
    }
}

/// Keys handed over directly: a store built in a test, or the S3 Vectors live suite, which
/// reads its own variables.
impl From<Credentials> for Provider {
    fn from(creds: Credentials) -> Self {
        Self::new(Source::Keys {
            creds,
            prefix: String::new(),
        })
    }
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Resolve the credentials for any [`Scope`].
///
/// # Order
///
/// The scope's own variables before the ambient ones, so an operator who exported a vault's
/// keys is never overruled by whatever the platform injected:
///
/// 1. `<prefix>ROLE_ARN` with `<prefix>WEB_IDENTITY_TOKEN_FILE`;
/// 2. `<prefix>ACCESS_KEY_ID` with `<prefix>SECRET_ACCESS_KEY`;
/// 3. for an ambient scope only, `AWS_ACCESS_KEY_ID` with `AWS_SECRET_ACCESS_KEY`;
/// 4. for an ambient scope only, `AWS_ROLE_ARN` with `AWS_WEB_IDENTITY_TOKEN_FILE`.
///
/// 3 before 4 is the AWS SDKs' own order. A scope's own role and own keys together are
/// refused: the two name different principals, and picking one silently is the guess the
/// half-pair rule below already declines to make.
pub fn resolve_scope(scope: &Scope, lookup: impl Fn(&str) -> Option<String>) -> Result<Provider> {
    let get = |k: &str| lookup(k).filter(|v| !v.trim().is_empty());
    let prefix = &scope.prefix;
    let label = &scope.label;

    let own_id = get(&format!("{prefix}ACCESS_KEY_ID"));
    let own_secret = get(&format!("{prefix}SECRET_ACCESS_KEY"));

    // A half-set pair is worth its own message. Falling back to `AWS_*` because only the
    // secret was exported would use credentials the operator did not choose, and the
    // resulting `403` says nothing about which key was tried.
    if own_id.is_some() != own_secret.is_some() {
        bail!(
            "{label} has only half its credentials in the environment.\n  \
             Set both {prefix}ACCESS_KEY_ID and {prefix}SECRET_ACCESS_KEY, or neither."
        );
    }

    let own_role = role(scope, prefix, &get)?;
    match (own_role, own_id, own_secret) {
        (Some(_), Some(_), _) => bail!(
            "{label} has both a role and keys in the environment.\n  \
             {prefix}ROLE_ARN and {prefix}ACCESS_KEY_ID name different principals; unset one."
        ),
        (Some(w), None, _) => return Ok(Provider::new(Source::WebIdentity(w))),
        (None, Some(id), Some(secret)) => {
            return Ok(Provider::new(Source::Keys {
                creds: Credentials {
                    access_key_id: id,
                    secret_access_key: secret,
                    session_token: get(&format!("{prefix}SESSION_TOKEN")),
                },
                prefix: prefix.clone(),
            }))
        }
        _ => {}
    }

    if scope.ambient {
        if let (Some(id), Some(secret)) = (get("AWS_ACCESS_KEY_ID"), get("AWS_SECRET_ACCESS_KEY")) {
            return Ok(Provider::new(Source::Keys {
                creds: Credentials {
                    access_key_id: id,
                    secret_access_key: secret,
                    session_token: get("AWS_SESSION_TOKEN"),
                },
                prefix: "AWS_".into(),
            }));
        }
        if let Some(w) = role(scope, "AWS_", &get)? {
            return Ok(Provider::new(Source::WebIdentity(w)));
        }
        bail!(
            "no credentials for {label}.\n  \
             Set {prefix}ACCESS_KEY_ID and {prefix}SECRET_ACCESS_KEY, or {prefix}ROLE_ARN and \
             {prefix}WEB_IDENTITY_TOKEN_FILE; or AWS_ACCESS_KEY_ID and AWS_SECRET_ACCESS_KEY, \
             or AWS_ROLE_ARN and AWS_WEB_IDENTITY_TOKEN_FILE.\n  \
             Credentials come from the environment only — `.yidam/config.toml` is committed \
             and must never carry one."
        );
    }

    bail!(
        "no credentials for {label}.\n  \
         Set {prefix}ACCESS_KEY_ID and {prefix}SECRET_ACCESS_KEY, or {prefix}ROLE_ARN and \
         {prefix}WEB_IDENTITY_TOKEN_FILE.\n  \
         {}",
        scope.isolation_note.as_deref().unwrap_or_default()
    )
}

/// A role under `prefix`, its token file checked to exist, or `None` where neither is set.
///
/// The file is checked here and read only at exchange time. Here, so `doctor` can say a
/// token is missing without a network call; not read, because the kubelet rotates it.
fn role(
    scope: &Scope,
    prefix: &str,
    get: &dyn Fn(&str) -> Option<String>,
) -> Result<Option<WebIdentity>> {
    let role_var = format!("{prefix}ROLE_ARN");
    let file_var = format!("{prefix}WEB_IDENTITY_TOKEN_FILE");
    let (role_arn, file) = match (get(&role_var), get(&file_var)) {
        (None, None) => return Ok(None),
        (Some(r), Some(f)) => (r, f),
        _ => bail!(
            "{} has only half its web identity in the environment.\n  \
             Set both {role_var} and {file_var}, or neither.",
            scope.label
        ),
    };
    let token_file = PathBuf::from(file.trim());
    if !token_file.is_file() {
        bail!(
            "{} names a web identity token at {}, and there is no file there.\n  \
             {file_var} is the path the platform projects the token to; on EKS, check the \
             service account's role annotation and that the pod was created after it.",
            scope.label,
            token_file.display()
        );
    }
    let session_name = get(&format!("{prefix}ROLE_SESSION_NAME"))
        .unwrap_or_else(|| format!("yidam-{}", unix_now()));
    Ok(Some(WebIdentity {
        role_arn: role_arn.trim().to_string(),
        token_file,
        session_name,
        sts_endpoint: super::web_identity::sts_endpoint(get),
        role_var,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vault::web_identity::REFRESH_BEFORE;
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
        .unwrap()
        .credentials()
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
        .unwrap()
        .credentials()
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
        .unwrap()
        .credentials()
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
        .unwrap()
        .credentials()
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
    fn the_remote_index_uses_its_own_variables_first() {
        let c = resolve_scope(
            &index_scope(),
            env(&[
                ("YIDAM_INDEX_ACCESS_KEY_ID", "AKIA_INDEX"),
                ("YIDAM_INDEX_SECRET_ACCESS_KEY", "s"),
                ("YIDAM_INDEX_SESSION_TOKEN", "tok"),
                ("AWS_ACCESS_KEY_ID", "AKIA_AMBIENT"),
                ("AWS_SECRET_ACCESS_KEY", "s"),
            ]),
        )
        .unwrap()
        .credentials()
        .unwrap();
        assert_eq!(c.access_key_id, "AKIA_INDEX");
        assert_eq!(c.session_token.as_deref(), Some("tok"));
    }

    /// The asymmetry with a second vault, asserted rather than only argued: there is at most
    /// one remote index, so inheritance crosses no boundary.
    #[test]
    fn the_remote_index_may_inherit_the_ambient_variables() {
        let c = resolve_scope(
            &index_scope(),
            env(&[
                ("AWS_ACCESS_KEY_ID", "AKIA_AMBIENT"),
                ("AWS_SECRET_ACCESS_KEY", "s"),
            ]),
        )
        .unwrap()
        .credentials()
        .unwrap();
        assert_eq!(c.access_key_id, "AKIA_AMBIENT");

        // And a second vault still may not, from the same environment — the two rules are
        // different on purpose and this is where that is visible.
        assert!(resolve(
            "sources",
            env(&[
                ("AWS_ACCESS_KEY_ID", "AKIA_AMBIENT"),
                ("AWS_SECRET_ACCESS_KEY", "s"),
            ]),
        )
        .is_err());
    }

    #[test]
    fn the_remote_index_names_its_own_variables_when_it_has_none() {
        let err = resolve_scope(&index_scope(), env(&[]))
            .map(|_| ())
            .unwrap_err()
            .to_string();
        assert!(err.contains("the remote index"), "{err}");
        assert!(err.contains("YIDAM_INDEX_ACCESS_KEY_ID"), "{err}");
        assert!(err.contains("committed"), "{err}");
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

    // ── Web identity (#1234) ─────────────────────────────────────────────────────────────

    /// A token file that exists, which resolution checks for. The directory is returned so it
    /// lives as long as the test.
    fn token() -> (tempfile::TempDir, String) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("token");
        std::fs::write(&path, "eyJ.token").unwrap();
        let path = path.to_string_lossy().to_string();
        (dir, path)
    }

    /// The role a provider will assume, or a panic naming what it resolved to instead.
    fn role_of(p: &Provider) -> (&str, &str) {
        match p.source() {
            Source::WebIdentity(w) => (w.role_arn.as_str(), w.role_var.as_str()),
            Source::Keys { prefix, .. } => panic!("resolved to keys under {prefix}"),
        }
    }

    fn keys_prefix(p: &Provider) -> &str {
        match p.source() {
            Source::Keys { prefix, .. } => prefix,
            Source::WebIdentity(w) => panic!("resolved to the role in {}", w.role_var),
        }
    }

    #[test]
    fn a_vault_assumes_its_own_role() {
        let (_d, file) = token();
        let p = resolve(
            "sources",
            env(&[
                (
                    "YIDAM_VAULT_SOURCES_ROLE_ARN",
                    "arn:aws:iam::1:role/sources",
                ),
                ("YIDAM_VAULT_SOURCES_WEB_IDENTITY_TOKEN_FILE", &file),
                ("YIDAM_VAULT_SOURCES_ROLE_SESSION_NAME", "run-7"),
            ]),
        )
        .unwrap();
        assert_eq!(
            role_of(&p),
            (
                "arn:aws:iam::1:role/sources",
                "YIDAM_VAULT_SOURCES_ROLE_ARN"
            )
        );
        match p.source() {
            Source::WebIdentity(w) => assert_eq!(w.session_name, "run-7"),
            Source::Keys { .. } => unreachable!(),
        }
        assert_eq!(p.origin(), "web identity via YIDAM_VAULT_SOURCES_ROLE_ARN");
        assert_eq!(p.principal(), "arn:aws:iam::1:role/sources");
    }

    /// **The isolation rule, for roles.** Every pod under a service account gets
    /// `AWS_ROLE_ARN`, and a second vault must not quietly become that role.
    #[test]
    fn a_non_default_vault_does_not_inherit_the_ambient_role() {
        let (_d, file) = token();
        let err = resolve(
            "sources",
            env(&[
                ("AWS_ROLE_ARN", "arn:aws:iam::1:role/pod"),
                ("AWS_WEB_IDENTITY_TOKEN_FILE", &file),
            ]),
        )
        .map(|_| ())
        .unwrap_err()
        .to_string();
        assert!(err.contains("YIDAM_VAULT_SOURCES_ROLE_ARN"), "{err}");
        assert!(err.contains("`AWS_ROLE_ARN` included"), "{err}");
    }

    #[test]
    fn default_assumes_the_ambient_role() {
        let (_d, file) = token();
        let p = resolve(
            "default",
            env(&[
                ("AWS_ROLE_ARN", "arn:aws:iam::1:role/pod"),
                ("AWS_WEB_IDENTITY_TOKEN_FILE", &file),
            ]),
        )
        .unwrap();
        assert_eq!(role_of(&p), ("arn:aws:iam::1:role/pod", "AWS_ROLE_ARN"));
    }

    /// The whole order, one rung at a time: each environment holds everything below the rung
    /// it expects, so a resolver that skipped a rung picks the wrong one.
    #[test]
    fn own_role_then_own_keys_then_ambient_keys_then_ambient_role() {
        let (_d, file) = token();
        let ambient_role = [
            ("AWS_ROLE_ARN", "arn:aws:iam::1:role/pod"),
            ("AWS_WEB_IDENTITY_TOKEN_FILE", file.as_str()),
        ];
        let ambient_keys = [
            ("AWS_ACCESS_KEY_ID", "AKIA_AMBIENT"),
            ("AWS_SECRET_ACCESS_KEY", "s"),
        ];
        let own_keys = [
            ("YIDAM_VAULT_DEFAULT_ACCESS_KEY_ID", "AKIA_OWN"),
            ("YIDAM_VAULT_DEFAULT_SECRET_ACCESS_KEY", "s"),
        ];
        let own_role = [
            ("YIDAM_VAULT_DEFAULT_ROLE_ARN", "arn:aws:iam::1:role/own"),
            ("YIDAM_VAULT_DEFAULT_WEB_IDENTITY_TOKEN_FILE", file.as_str()),
        ];
        let with = |sets: &[&[(&str, &str)]]| {
            let all: Vec<(&str, &str)> = sets.iter().flat_map(|s| s.iter().copied()).collect();
            resolve("default", env(&all)).unwrap()
        };

        let p = with(&[&own_role, &ambient_keys, &ambient_role]);
        assert_eq!(role_of(&p).1, "YIDAM_VAULT_DEFAULT_ROLE_ARN");
        let p = with(&[&own_keys, &ambient_keys, &ambient_role]);
        assert_eq!(keys_prefix(&p), "YIDAM_VAULT_DEFAULT_");
        let p = with(&[&ambient_keys, &ambient_role]);
        assert_eq!(keys_prefix(&p), "AWS_");
        let p = with(&[&ambient_role]);
        assert_eq!(role_of(&p).1, "AWS_ROLE_ARN");
    }

    #[test]
    fn a_vaults_own_role_and_own_keys_together_are_refused() {
        let (_d, file) = token();
        let err = resolve(
            "sources",
            env(&[
                ("YIDAM_VAULT_SOURCES_ROLE_ARN", "arn:aws:iam::1:role/r"),
                ("YIDAM_VAULT_SOURCES_WEB_IDENTITY_TOKEN_FILE", &file),
                ("YIDAM_VAULT_SOURCES_ACCESS_KEY_ID", "A"),
                ("YIDAM_VAULT_SOURCES_SECRET_ACCESS_KEY", "S"),
            ]),
        )
        .map(|_| ())
        .unwrap_err()
        .to_string();
        assert!(err.contains("both a role and keys"), "{err}");
    }

    /// Half a role is its own diagnosis, like half a key pair, and is not fallen through:
    /// `default` would otherwise go on to whatever `AWS_*` holds.
    #[test]
    fn half_a_role_is_reported_rather_than_fallen_through() {
        let err = resolve(
            "default",
            env(&[
                ("YIDAM_VAULT_DEFAULT_ROLE_ARN", "arn:aws:iam::1:role/r"),
                ("AWS_ACCESS_KEY_ID", "AKIA_AMBIENT"),
                ("AWS_SECRET_ACCESS_KEY", "s"),
            ]),
        )
        .map(|_| ())
        .unwrap_err()
        .to_string();
        assert!(err.contains("half its web identity"), "{err}");
        assert!(
            err.contains("YIDAM_VAULT_DEFAULT_WEB_IDENTITY_TOKEN_FILE"),
            "{err}"
        );
    }

    /// Checked at resolution, so `doctor` reports it offline, rather than at the first
    /// request, where it would arrive as a failure to push.
    #[test]
    fn a_token_file_that_is_not_there_is_reported_at_resolution() {
        let err = resolve(
            "default",
            env(&[
                ("AWS_ROLE_ARN", "arn:aws:iam::1:role/r"),
                ("AWS_WEB_IDENTITY_TOKEN_FILE", "/nonexistent/yidam/token"),
            ]),
        )
        .map(|_| ())
        .unwrap_err()
        .to_string();
        assert!(err.contains("/nonexistent/yidam/token"), "{err}");
        assert!(err.contains("no file there"), "{err}");
    }

    #[test]
    fn the_remote_index_assumes_its_own_role_then_the_ambient_one() {
        let (_d, file) = token();
        let p = resolve_scope(
            &index_scope(),
            env(&[
                ("YIDAM_INDEX_ROLE_ARN", "arn:aws:iam::1:role/index"),
                ("YIDAM_INDEX_WEB_IDENTITY_TOKEN_FILE", &file),
                ("AWS_ROLE_ARN", "arn:aws:iam::1:role/pod"),
                ("AWS_WEB_IDENTITY_TOKEN_FILE", &file),
            ]),
        )
        .unwrap();
        assert_eq!(role_of(&p).1, "YIDAM_INDEX_ROLE_ARN");
        let p = resolve_scope(
            &index_scope(),
            env(&[
                ("AWS_ROLE_ARN", "arn:aws:iam::1:role/pod"),
                ("AWS_WEB_IDENTITY_TOKEN_FILE", &file),
            ]),
        )
        .unwrap();
        assert_eq!(role_of(&p).1, "AWS_ROLE_ARN");
    }

    /// A lease is reused while it is fresh and exchanged again inside the margin, which is
    /// what lets a push or a server outlive the hour. The clock and the exchange are
    /// injected; both are statics because the provider holds plain `fn`s.
    #[test]
    fn a_lease_is_reused_until_the_margin_and_then_exchanged_again() {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NOW: AtomicU64 = AtomicU64::new(1_000);
        static EXCHANGES: AtomicU64 = AtomicU64::new(0);
        fn exchange(_: &WebIdentity) -> Result<Lease> {
            let n = EXCHANGES.fetch_add(1, Ordering::SeqCst) + 1;
            Ok(Lease {
                creds: Credentials {
                    access_key_id: format!("ASIA{n}"),
                    secret_access_key: "s".into(),
                    session_token: Some("t".into()),
                },
                expires: NOW.load(Ordering::SeqCst) + 3600,
            })
        }

        let (_d, file) = token();
        let mut p = resolve(
            "default",
            env(&[
                ("AWS_ROLE_ARN", "arn:aws:iam::1:role/pod"),
                ("AWS_WEB_IDENTITY_TOKEN_FILE", &file),
            ]),
        )
        .unwrap();
        p.exchange = exchange;
        p.now = || NOW.load(Ordering::SeqCst);

        assert_eq!(p.credentials().unwrap().access_key_id, "ASIA1");
        NOW.store(1_000 + 3600 - REFRESH_BEFORE - 1, Ordering::SeqCst);
        assert_eq!(
            p.credentials().unwrap().access_key_id,
            "ASIA1",
            "still fresh"
        );
        NOW.store(1_000 + 3600 - REFRESH_BEFORE, Ordering::SeqCst);
        assert_eq!(
            p.credentials().unwrap().access_key_id,
            "ASIA2",
            "inside the margin"
        );
        assert_eq!(EXCHANGES.load(Ordering::SeqCst), 2);
    }
}
