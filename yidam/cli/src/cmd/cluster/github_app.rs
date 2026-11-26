//! The lander's push credential as a GitHub App installation token (#1233).
//!
//! A deploy key in `yidam-<corpus>-git-write` has write access and no expiry. The invariant
//! keeps it out of every pod but `land`, but a key that leaks from the secret store, a backup
//! or a node's disk works until someone notices and rotates it. With `--git-auth github-app`
//! the secret holds a GitHub App's private key instead. The lander signs a JWT with it, asks
//! GitHub for an installation token scoped to this one repository with `contents: write`,
//! and pushes over HTTPS with that. The token expires an hour after it is minted.
//!
//! # Minted in the lander and nowhere else
//!
//! A separate pod that minted the token and handed it to `land` would pass it as an Argo
//! parameter. Argo records parameters in the Workflow object, so the token would be readable
//! by anything that can read Workflows, for as long as the object is kept. Here the token
//! exists in the lander process and in the git processes it spawns, and nowhere else.
//!
//! # Where the token goes
//!
//! Into the environment of each git process, as `http.<url>.extraHeader` through git's
//! `GIT_CONFIG_COUNT` variables. Each other place git could find it was ruled out:
//!
//! - **argv** (`git -c …`): any process in the pod can read `/proc/<pid>/cmdline`.
//! - **the clone's config**: a file on the scratch volume.
//! - **the remote url** (`https://x-access-token:…@`): git prints remote urls, and the lander
//!   passes the remote's stderr through as its own error.
//!
//! The header is keyed to the repository's own url, so git sends it to that repository and to
//! no other url, including one a redirect points at.
//!
//! The App's private key is still a long-lived secret, mounted by `land` alone. That is no
//! worse than a deploy key, and what a push carries now expires.

use std::path::PathBuf;

use anyhow::{bail, Result};
use clap::Args;

use crate::config::GitAuth;
use crate::git::Git;

/// Where GitHub's REST API is, for github.com. GitHub Enterprise Server's is
/// `https://<host>/api/v3`.
pub const DEFAULT_API: &str = "https://api.github.com";

/// Where the workflow mounts the App's private key, in the `git-write` secret's volume.
pub const KEY_FILE: &str = "private-key.pem";

/// How the lander authenticates its push: a deploy key over SSH, or an App token over HTTPS.
#[derive(Args, Debug, Clone)]
pub struct GitAuthArgs {
    /// The push credential: `deploy-key` uses the SSH key `GIT_SSH_COMMAND` names, and
    /// `github-app` mints an hour-long installation token from an App's private key
    #[arg(long = "git-auth", value_enum, default_value_t = GitAuth::DeployKey)]
    pub git_auth: GitAuth,
    /// The GitHub App's id, with `--git-auth github-app`
    #[arg(long = "github-app-id", value_name = "ID")]
    pub app_id: Option<u64>,
    /// The GitHub App's private key, PEM, with `--git-auth github-app`
    #[arg(long = "github-app-key", value_name = "FILE")]
    pub app_key: Option<PathBuf>,
    /// GitHub's REST API, with `--git-auth github-app`. GitHub Enterprise Server's is
    /// `https://<host>/api/v3`
    #[arg(long = "github-api", value_name = "URL")]
    pub api: Option<String>,
}

impl GitAuthArgs {
    /// The url the lander clones and pushes, and what each git process it spawns is given.
    ///
    /// For a deploy key that is the remote as given and nothing. For an App it is the
    /// repository's HTTPS url and a token minted for it now.
    pub fn resolve(&self, remote: &str) -> Result<(String, PushAuth)> {
        match self.git_auth {
            GitAuth::DeployKey => {
                for (flag, set) in [
                    ("--github-app-id", self.app_id.is_some()),
                    ("--github-app-key", self.app_key.is_some()),
                    ("--github-api", self.api.is_some()),
                ] {
                    if set {
                        bail!("{flag} is read only with `--git-auth github-app`");
                    }
                }
                Ok((remote.to_string(), PushAuth::none()))
            }
            GitAuth::GithubApp => {
                let (Some(id), Some(key)) = (self.app_id, &self.app_key) else {
                    bail!("`--git-auth github-app` needs `--github-app-id` and `--github-app-key`");
                };
                let api = self.api.as_deref().unwrap_or(DEFAULT_API);
                check_api(api)?;
                let repo = Repo::of_remote(remote)?;
                let app = App {
                    id,
                    key: key.clone(),
                    api: api.trim_end_matches('/').to_string(),
                };
                let token = mint(&app, &repo)?;
                Ok((repo.https_url(), PushAuth::bearer(&token, &repo)))
            }
        }
    }
}

/// A GitHub App, as the lander is told about it.
#[cfg_attr(not(feature = "github-app"), allow(dead_code))]
pub struct App {
    pub id: u64,
    pub key: PathBuf,
    pub api: String,
}

/// A repository on a GitHub host, read off the corpus's remote.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Repo {
    pub host: String,
    pub owner: String,
    pub name: String,
}

impl Repo {
    /// The repository a GitHub remote names, in any of the forms `git clone` takes for one:
    /// `git@host:owner/name.git`, `ssh://git@host[:port]/owner/name.git` or
    /// `https://host/owner/name.git`.
    ///
    /// An SSH remote is accepted because the same remote is what `pin` and `admit` read with
    /// the read key, and one `[cluster] remote` serves every pod. An SSH port is dropped,
    /// since it is not the HTTPS port.
    pub fn of_remote(remote: &str) -> Result<Self> {
        let refuse = |why: &str| {
            anyhow::anyhow!(
                "`--git-auth github-app` pushes to a GitHub repository over HTTPS, and the \
                 remote `{remote}` {why}"
            )
        };
        let (host, path) = if let Some(rest) = remote.strip_prefix("https://") {
            let (authority, path) = rest
                .split_once('/')
                .ok_or_else(|| refuse("names no repository"))?;
            if authority.contains('@') {
                return Err(refuse(
                    "carries a credential, and git prints remote urls in its errors",
                ));
            }
            (authority.to_string(), path)
        } else if let Some(rest) = remote.strip_prefix("ssh://") {
            let (authority, path) = rest
                .split_once('/')
                .ok_or_else(|| refuse("names no repository"))?;
            let host = authority.rsplit('@').next().unwrap_or(authority);
            let host = host.split(':').next().unwrap_or(host);
            (host.to_string(), path)
        } else if remote.contains("://") {
            return Err(refuse("is neither an SSH nor an HTTPS url"));
        } else {
            // scp-like: `[user@]host:path`. A local path has no `:` before its first `/`.
            match remote.split_once(':') {
                Some((authority, path)) if !authority.contains('/') && !authority.is_empty() => {
                    let host = authority.rsplit('@').next().unwrap_or(authority);
                    (host.to_string(), path)
                }
                _ => return Err(refuse("is a path, not a url")),
            }
        };
        let path = path.trim_end_matches('/');
        let path = path.strip_suffix(".git").unwrap_or(path);
        let mut parts = path.split('/');
        let (Some(owner), Some(name), None) = (parts.next(), parts.next(), parts.next()) else {
            return Err(refuse("is not `<host>/<owner>/<repository>`"));
        };
        let ok = |s: &str| {
            !s.is_empty()
                && s.chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
        };
        if host.is_empty() || !ok(owner) || !ok(name) {
            return Err(refuse("is not `<host>/<owner>/<repository>`"));
        }
        Ok(Self {
            host,
            owner: owner.to_string(),
            name: name.to_string(),
        })
    }

    pub fn https_url(&self) -> String {
        format!("https://{}/{}/{}.git", self.host, self.owner, self.name)
    }
}

/// The API url is HTTPS, or HTTP to this machine.
///
/// The JWT is a credential for the App, valid for ten minutes on every repository the App is
/// installed on. Sent in the clear it is readable by anything on the path. Loopback is
/// allowed for a test double, since it never leaves the host.
pub fn check_api(url: &str) -> Result<()> {
    if url.starts_with("https://") {
        return Ok(());
    }
    if let Some(rest) = url.strip_prefix("http://") {
        let authority = rest.split('/').next().unwrap_or(rest);
        let host = match authority.strip_prefix('[') {
            Some(v6) => v6.split(']').next().unwrap_or(v6),
            None => authority.split(':').next().unwrap_or(authority),
        };
        if matches!(host, "127.0.0.1" | "localhost" | "::1") {
            return Ok(());
        }
    }
    bail!(
        "the GitHub API url `{url}` is not https, and the App's JWT would cross the network in \
         the clear"
    )
}

/// An installation token. It has no `Display` and no `Serialize`, so it can reach a record or
/// stdout only by someone writing `.0`.
pub struct Token(String);

impl std::fmt::Debug for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Token(<redacted>)")
    }
}

/// What each git process the lander spawns is given beyond its arguments: nothing for a
/// deploy key, and the token's header, keyed to the repository's url, for an App.
#[derive(Clone, Default)]
pub struct PushAuth {
    env: Vec<(String, String)>,
}

impl std::fmt::Debug for PushAuth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let keys: Vec<&str> = self.env.iter().map(|(k, _)| k.as_str()).collect();
        f.debug_struct("PushAuth").field("env", &keys).finish()
    }
}

impl PushAuth {
    pub fn none() -> Self {
        Self::default()
    }

    /// The header GitHub reads an installation token from, for `repo`'s url alone.
    ///
    /// `x-access-token` is the user name GitHub documents for an installation token over
    /// HTTPS. `Basic` is git's own scheme for a user and password, so the header is the one
    /// a credential helper would have produced.
    fn bearer(token: &Token, repo: &Repo) -> Self {
        let basic = base64_encode(format!("x-access-token:{}", token.0).as_bytes());
        Self {
            env: vec![
                ("GIT_CONFIG_COUNT".into(), "1".into()),
                (
                    "GIT_CONFIG_KEY_0".into(),
                    format!("http.{}.extraHeader", repo.https_url()),
                ),
                (
                    "GIT_CONFIG_VALUE_0".into(),
                    format!("Authorization: Basic {basic}"),
                ),
            ],
        }
    }

    /// A git invocation in `root` that carries this credential.
    pub fn git(&self, root: impl AsRef<std::path::Path>) -> Git {
        self.env
            .iter()
            .fold(Git::new(root), |g, (k, v)| g.env(k, v))
    }
}

/// GitHub's `message`, from an error response, or a placeholder when it gave none.
///
/// Read only from a response that is not a success. A token response that failed to parse is
/// never echoed, since its body holds the token.
fn github_message(body: &[u8]) -> String {
    serde_json::from_slice::<serde_json::Value>(body)
        .ok()
        .and_then(|v| v.get("message")?.as_str().map(str::to_string))
        .unwrap_or_else(|| "(GitHub gave no message)".to_string())
}

/// The installation id in `GET /repos/{owner}/{repo}/installation`'s answer.
#[cfg_attr(not(feature = "github-app"), allow(dead_code))]
fn installation_id(body: &[u8]) -> Result<u64> {
    serde_json::from_slice::<serde_json::Value>(body)
        .ok()
        .and_then(|v| v.get("id")?.as_u64())
        .ok_or_else(|| anyhow::anyhow!("GitHub's installation response carries no numeric `id`"))
}

/// The token in `POST /app/installations/{id}/access_tokens`'s answer.
#[cfg_attr(not(feature = "github-app"), allow(dead_code))]
fn token_of(body: &[u8]) -> Result<Token> {
    serde_json::from_slice::<serde_json::Value>(body)
        .ok()
        .and_then(|v| v.get("token")?.as_str().map(str::to_string))
        .filter(|t| !t.is_empty())
        .map(Token)
        // The body is not quoted: if it is malformed, it may still hold the token.
        .ok_or_else(|| anyhow::anyhow!("GitHub's token response carries no `token`"))
}

/// The token request's body: this repository alone, and the one permission a push needs.
#[cfg_attr(not(feature = "github-app"), allow(dead_code))]
fn token_request(repo: &Repo) -> serde_json::Value {
    serde_json::json!({
        "repositories": [repo.name],
        "permissions": { "contents": "write" },
    })
}

/// The refusal for an answer GitHub gave with a failing status, with GitHub's own words.
#[cfg_attr(not(feature = "github-app"), allow(dead_code))]
fn refused(what: &str, repo: &Repo, status: u16, body: &[u8]) -> anyhow::Error {
    let hint = match status {
        401 => {
            "GitHub did not accept the App's JWT. The app id or the private key is wrong, or \
             this pod's clock is more than a minute off."
        }
        404 => "The App is not installed on this repository, or the app id is wrong.",
        422 => "The App's installation lacks `contents: write` on this repository.",
        _ => "",
    };
    anyhow::anyhow!(
        "{what} for {}/{} failed: HTTP {status}: {}.\n  {hint}",
        repo.owner,
        repo.name,
        github_message(body)
    )
}

#[cfg(feature = "github-app")]
fn mint(app: &App, repo: &Repo) -> Result<Token> {
    use anyhow::Context;

    let pem = std::fs::read_to_string(&app.key)
        .with_context(|| format!("reading the GitHub App's key at {}", app.key.display()))?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .context("the clock is before 1970")?
        .as_secs();
    let jwt = sign::jwt(app.id, now, &pem)?;
    crate::runtime::block_on(async {
        let client = reqwest::Client::builder()
            .user_agent(concat!("yidam-cluster/", env!("CARGO_PKG_VERSION")))
            .build()
            .context("building HTTP client")?;
        let send = |req: reqwest::RequestBuilder| {
            req.bearer_auth(&jwt)
                .header("Accept", "application/vnd.github+json")
                .header("X-GitHub-Api-Version", "2022-11-28")
                .send()
        };
        let url = format!(
            "{}/repos/{}/{}/installation",
            app.api, repo.owner, repo.name
        );
        let resp = send(client.get(&url))
            .await
            .with_context(|| format!("GET {url}"))?;
        let status = resp.status();
        let body = resp.bytes().await.context("reading GitHub's response")?;
        if !status.is_success() {
            return Err(refused(
                "looking up the App's installation",
                repo,
                status.as_u16(),
                &body,
            ));
        }
        let id = installation_id(&body)?;
        let url = format!("{}/app/installations/{id}/access_tokens", app.api);
        let resp = send(client.post(&url).json(&token_request(repo)))
            .await
            .with_context(|| format!("POST {url}"))?;
        let status = resp.status();
        let body = resp.bytes().await.context("reading GitHub's response")?;
        if !status.is_success() {
            return Err(refused(
                "minting an installation token",
                repo,
                status.as_u16(),
                &body,
            ));
        }
        token_of(&body)
    })
}

/// The refusal a build without the feature gives, before anything is cloned.
#[cfg(not(feature = "github-app"))]
fn mint(_app: &App, _repo: &Repo) -> Result<Token> {
    bail!(
        "this build cannot mint a GitHub App token — `github-app` is not compiled in.\n  \
         It is in the default feature set, so this is a `--no-default-features` build: \
         reinstall with `cargo install yidam --features github-app`, or land with a deploy key."
    )
}

#[cfg(feature = "github-app")]
fn base64_encode(bytes: &[u8]) -> String {
    use base64::Engine as _;
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

/// Unreachable without the feature: [`mint`] refuses first, so no token exists to encode.
#[cfg(not(feature = "github-app"))]
fn base64_encode(_bytes: &[u8]) -> String {
    unreachable!("no token is minted without `github-app`")
}

/// The App's JWT: RS256 over the App's id, signed with its private key.
#[cfg(feature = "github-app")]
mod sign {
    use anyhow::{bail, Context, Result};
    use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
    use base64::Engine as _;
    use ring::rand::SystemRandom;
    use ring::signature::{RsaKeyPair, RSA_PKCS1_SHA256};

    /// `iat` a minute back and `exp` nine minutes on, as GitHub documents: the backdating
    /// allows for clock drift, and GitHub refuses an `exp` more than ten minutes out.
    pub(super) fn jwt(app_id: u64, now: u64, pem: &str) -> Result<String> {
        let header = URL_SAFE_NO_PAD.encode(br#"{"alg":"RS256","typ":"JWT"}"#);
        let claims = serde_json::json!({
            "iat": now.saturating_sub(60),
            "exp": now + 540,
            "iss": app_id.to_string(),
        });
        let claims = URL_SAFE_NO_PAD.encode(claims.to_string());
        let input = format!("{header}.{claims}");
        let key = key_pair(pem)?;
        let mut sig = vec![0; key.public().modulus_len()];
        key.sign(
            &RSA_PKCS1_SHA256,
            &SystemRandom::new(),
            input.as_bytes(),
            &mut sig,
        )
        .map_err(|_| anyhow::anyhow!("signing the App's JWT failed"))?;
        Ok(format!("{input}.{}", URL_SAFE_NO_PAD.encode(sig)))
    }

    /// The RSA key in a PEM file: PKCS#1, the form GitHub generates, or PKCS#8.
    pub(super) fn key_pair(pem: &str) -> Result<RsaKeyPair> {
        let (label, der) = pem_block(pem)?;
        let parsed = match label.as_str() {
            "RSA PRIVATE KEY" => RsaKeyPair::from_der(&der),
            "PRIVATE KEY" => RsaKeyPair::from_pkcs8(&der),
            other => bail!(
                "the GitHub App's key is a PEM `{other}`; an App's key is an `RSA PRIVATE KEY`"
            ),
        };
        parsed.map_err(|e| anyhow::anyhow!("the GitHub App's key is not a usable RSA key: {e}"))
    }

    /// The first PEM block's label and bytes.
    fn pem_block(pem: &str) -> Result<(String, Vec<u8>)> {
        let mut lines = pem.lines().map(str::trim);
        let label = lines
            .by_ref()
            .find_map(|l| l.strip_prefix("-----BEGIN ")?.strip_suffix("-----"))
            .context("the GitHub App's key holds no PEM block")?
            .to_string();
        let end = format!("-----END {label}-----");
        let body: String = lines.take_while(|l| *l != end).collect();
        let der = STANDARD
            .decode(body)
            .context("the GitHub App's key is not valid PEM")?;
        Ok((label, der))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo() -> Repo {
        Repo {
            host: "github.com".into(),
            owner: "acme".into(),
            name: "corpus".into(),
        }
    }

    #[test]
    fn every_form_of_a_github_remote_names_one_repository() {
        for remote in [
            "git@github.com:acme/corpus.git",
            "git@github.com:acme/corpus",
            "ssh://git@github.com/acme/corpus.git",
            "ssh://git@github.com:22/acme/corpus.git",
            "https://github.com/acme/corpus.git",
            "https://github.com/acme/corpus/",
        ] {
            assert_eq!(Repo::of_remote(remote).unwrap(), repo(), "{remote}");
        }
        assert_eq!(repo().https_url(), "https://github.com/acme/corpus.git");
        let ghes = Repo::of_remote("git@ghe.example.com:team/notes.git").unwrap();
        assert_eq!(ghes.https_url(), "https://ghe.example.com/team/notes.git");
    }

    #[test]
    fn a_remote_that_names_no_github_repository_is_refused() {
        for (remote, why) in [
            ("/srv/git/corpus.git", "is a path"),
            ("../corpus.git", "is a path"),
            ("file:///srv/corpus.git", "neither an SSH nor an HTTPS"),
            (
                "http://github.com/acme/corpus.git",
                "neither an SSH nor an HTTPS",
            ),
            (
                "https://github.com/acme",
                "is not `<host>/<owner>/<repository>`",
            ),
            ("https://github.com/acme/corpus/tree", "is not"),
            (
                "https://x:ghp_secret@github.com/acme/corpus.git",
                "carries a credential",
            ),
        ] {
            let e = Repo::of_remote(remote).unwrap_err().to_string();
            assert!(e.contains(why), "{remote}: {e}");
        }
    }

    #[test]
    fn the_api_is_https_or_this_machine() {
        for ok in [
            "https://api.github.com",
            "https://ghe.example.com/api/v3",
            "http://127.0.0.1:8080",
            "http://localhost:1/x",
            "http://[::1]:9",
        ] {
            check_api(ok).unwrap_or_else(|e| panic!("{ok}: {e}"));
        }
        for bad in [
            "http://api.github.com",
            "http://127.0.0.1.evil.example",
            "ftp://x",
        ] {
            assert!(check_api(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn deploy_key_mode_refuses_an_app_flag_it_would_ignore() {
        let args = GitAuthArgs {
            git_auth: GitAuth::DeployKey,
            app_id: Some(1),
            app_key: None,
            api: None,
        };
        let e = args.resolve("git@github.com:acme/corpus.git").unwrap_err();
        assert!(e.to_string().contains("--github-app-id"), "{e}");
        let plain = GitAuthArgs {
            app_id: None,
            ..args
        };
        let (url, auth) = plain.resolve("/srv/corpus.git").unwrap();
        assert_eq!(url, "/srv/corpus.git");
        assert!(auth.env.is_empty());
    }

    #[test]
    fn app_mode_needs_an_id_and_a_key_before_it_reaches_the_network() {
        let args = GitAuthArgs {
            git_auth: GitAuth::GithubApp,
            app_id: Some(1),
            app_key: None,
            api: None,
        };
        let e = args.resolve("git@github.com:acme/corpus.git").unwrap_err();
        assert!(e.to_string().contains("--github-app-key"), "{e}");
    }

    /// A token response is parsed, and when it cannot be, the error does not quote it.
    #[test]
    fn a_token_is_never_printed() {
        let t = token_of(br#"{"token":"ghs_SENTINEL","expires_at":"x"}"#).unwrap();
        assert_eq!(format!("{t:?}"), "Token(<redacted>)");
        let e = token_of(br#"{"tok":"ghs_SENTINEL"}"#).unwrap_err();
        assert!(!format!("{e:#}").contains("SENTINEL"), "{e:#}");
        let e = token_of(b"ghs_SENTINEL not json").unwrap_err();
        assert!(!format!("{e:#}").contains("SENTINEL"), "{e:#}");
    }

    #[test]
    fn the_token_request_asks_for_one_repository_and_contents_write() {
        assert_eq!(
            token_request(&repo()),
            serde_json::json!({
                "repositories": ["corpus"],
                "permissions": { "contents": "write" },
            })
        );
        assert_eq!(installation_id(br#"{"id":42,"app_id":7}"#).unwrap(), 42);
        assert!(installation_id(br#"{"id":"42"}"#).is_err());
    }

    #[test]
    fn a_refusal_carries_githubs_words_and_a_reason() {
        let e = refused(
            "minting an installation token",
            &repo(),
            422,
            br#"{"message":"Validation Failed"}"#,
        )
        .to_string();
        assert!(e.contains("HTTP 422: Validation Failed"), "{e}");
        assert!(e.contains("contents: write"), "{e}");
        assert!(github_message(b"<html>").contains("no message"));
    }

    #[cfg(feature = "github-app")]
    mod signed {
        use super::super::*;
        use super::repo;
        use base64::engine::general_purpose::URL_SAFE_NO_PAD;
        use base64::Engine as _;

        fn fixture(name: &str) -> String {
            std::fs::read_to_string(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("tests/fixtures/github-app")
                    .join(name),
            )
            .unwrap()
        }

        /// The JWT verifies against the key's public half, in both PEM forms, and says what
        /// GitHub reads: RS256, the app id as `iss`, and a window GitHub accepts.
        #[test]
        fn the_jwt_is_signed_by_the_apps_key() {
            for name in ["private-key.pem", "private-key.pkcs8.pem"] {
                let pem = fixture(name);
                let jwt = sign::jwt(123456, 1_800_000_000, &pem).unwrap();
                let parts: Vec<&str> = jwt.split('.').collect();
                assert_eq!(parts.len(), 3, "{name}");
                let json = |s: &str| -> serde_json::Value {
                    serde_json::from_slice(&URL_SAFE_NO_PAD.decode(s).unwrap()).unwrap()
                };
                assert_eq!(json(parts[0])["alg"], "RS256");
                let claims = json(parts[1]);
                assert_eq!(claims["iss"], "123456");
                assert_eq!(claims["iat"], 1_800_000_000u64 - 60);
                assert_eq!(claims["exp"], 1_800_000_000u64 + 540);
                let public = sign::key_pair(&pem).unwrap();
                let public = ring::signature::UnparsedPublicKey::new(
                    &ring::signature::RSA_PKCS1_2048_8192_SHA256,
                    ring::signature::KeyPair::public_key(&public)
                        .as_ref()
                        .to_vec(),
                );
                public
                    .verify(
                        format!("{}.{}", parts[0], parts[1]).as_bytes(),
                        &URL_SAFE_NO_PAD.decode(parts[2]).unwrap(),
                    )
                    .unwrap_or_else(|_| panic!("{name}: the signature does not verify"));
            }
        }

        #[test]
        fn a_key_that_is_not_an_rsa_private_key_is_refused() {
            let e = sign::jwt(
                1,
                0,
                "-----BEGIN CERTIFICATE-----\nAAAA\n-----END CERTIFICATE-----",
            )
            .unwrap_err();
            assert!(e.to_string().contains("`CERTIFICATE`"), "{e}");
            assert!(sign::jwt(1, 0, "not a key").is_err());
        }

        /// What git makes of the environment: the header for the repository's url, and
        /// nothing for any other — a redirect to another host, or another repository on this
        /// one.
        #[test]
        fn git_sends_the_header_to_the_repository_alone() {
            let dir = tempfile::tempdir().unwrap();
            let auth = PushAuth::bearer(&Token("ghs_SENTINEL".into()), &repo());
            let ask = |url: &str| {
                auth.git(dir.path())
                    .args(["config", "--get-urlmatch", "http.extraHeader", url])
                    .try_run()
            };
            let header = ask("https://github.com/acme/corpus.git").unwrap();
            let encoded = header.strip_prefix("Authorization: Basic ").unwrap();
            assert_eq!(
                base64::engine::general_purpose::STANDARD
                    .decode(encoded)
                    .unwrap(),
                b"x-access-token:ghs_SENTINEL"
            );
            for other in [
                "https://github.com/acme/other.git",
                "https://evil.example/acme/corpus.git",
            ] {
                assert_eq!(ask(other).filter(|s| !s.is_empty()), None, "{other}");
            }
            assert!(!format!("{auth:?}").contains("Basic"), "{auth:?}");
        }
    }
}
