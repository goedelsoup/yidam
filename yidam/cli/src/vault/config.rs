//! What a repository declares about its vaults, and how an artifact finds one.
//!
//! # A vault declares what it holds
//!
//! The table has been plural since #413, which allowed exactly one entry. This lifts that
//! limit, and the shape it lifts it into is `holds` on the store rather than a central
//! routing table:
//!
//! ```toml
//! [vault.default]
//! url      = "s3://corpus-artifacts/yidam"
//! audience = "Anyone who can read this corpus. Derived output only."
//! holds    = ["index", "embeddings", "bundle"]
//!
//! [vault.sources]
//! url      = "s3://licensed-sources/yidam"
//! audience = "The sangha. Documents obtained under a licence to read, not to host."
//! holds    = ["catalog"]
//! ```
//!
//! RFC-0023 argues the shape at length; the decisive part is that `[vault.default]` has
//! already shipped, so the alternative — `[vault.stores.default]` beside `[vault.routes]` —
//! would move a section derived repositories may already have written. `holds` is additive.
//! The second reason is worth restating here because this file is where it bites: **the claim
//! sits beside the audience it has to be consistent with.** *"This store is for the sangha,
//! and it holds the catalog"* is one block a reader checks at once.
//!
//! # Where the totality rule went
//!
//! RFC-0023's table says a kind claimed by no vault is *refused*. It does not say when, and
//! the two readings are not equivalent:
//!
//! - **At resolve** — every kind in [`ARTIFACT_KINDS`] must be claimed, or the config is bad.
//! - **At the artifact** — a kind is refused when something of that kind needs a route.
//!
//! This implements the second, and the reason is that the first would make the kind
//! vocabulary a compatibility surface. Adding `embeddings` to the list in a later release
//! would turn every multi-vault config in the wild red, for a kind those corpora have none of.
//! That is the failure this repository has already recorded once, in the `edge_policy`
//! episode: a list of permitted values never says *and no others* unless somebody checks what
//! the others are.
//!
//! So an unclaimed kind is refused at the moment it has bytes to route, which is still before
//! any of them move. What [`ARTIFACT_KINDS`] is for is the *typo*: `holds = ["catalouge"]` is
//! a vault claiming nothing, and it would otherwise be indistinguishable from a vault that
//! meant to. That is the same argument `deny_unknown_fields` makes about `endpiont`, and it
//! costs nothing, because it only ever looks at kinds somebody actually wrote.
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

/// The vault name the ambient `AWS_*` credential fallback is scoped to.
///
/// Not a required name any more — a corpus may declare a single vault called `archive`. What
/// it loses by doing so is that fallback, which [`super::creds`] explains and which its error
/// message names when the credentials turn out to be missing.
pub const DEFAULT_VAULT: &str = "default";

/// The artifact kinds a vault can claim.
///
/// A closed set so that a misspelling in `holds` is caught rather than silently claiming
/// nothing. **Not** a set every config must cover — see the module header for why totality
/// would make this list a compatibility surface.
///
/// `catalog` is the only kind anything produces today. The other three are named here because
/// #417 and #418 produce them and a corpus should be able to declare their routes before they
/// arrive, rather than reorganising its storage on the release that starts writing them.
pub const ARTIFACT_KINDS: &[&str] = &["catalog", "index", "embeddings", "bundle"];

/// The kind a catalog record's artifact is.
pub const CATALOG_KIND: &str = "catalog";

/// The route `vault: none` names: this machine's cache and nowhere else.
pub const LOCAL_ONLY: &str = "none";

/// One store a repository can put bytes in.
///
/// Deserialized permissively at the leaves — `region`, `endpoint` and `path_style` mean
/// nothing to the `file://` backend and are carried so a config written for S3 is not
/// rejected by a build that cannot reach one. An unknown *key*, though, is refused: a
/// misspelled `endpiont` that parses silently is a vault pointing somewhere nobody intended.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct VaultConfig {
    /// Where the store is — `file:///…` or `s3://bucket/prefix`.
    pub url: String,
    /// Who can read this store, in prose. Required; see the module header.
    #[serde(default)]
    pub audience: Option<String>,
    /// The artifact kinds this store takes. Optional for a lone vault, required once there
    /// are two — a vault among several that claims nothing is the silent catch-all an
    /// isolation boundary exists to prevent.
    #[serde(default)]
    pub holds: Option<Vec<String>>,
    /// SigV4 signing scope. Unused by `file://`.
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

    /// What this vault claims, or the empty slice where it claims nothing explicitly.
    pub fn holds(&self) -> &[String] {
        self.holds.as_deref().unwrap_or(&[])
    }

    /// How to describe what it holds, on one line.
    pub fn holds_display(&self) -> String {
        match &self.holds {
            None => "everything".to_string(),
            Some(k) => k.join(", "),
        }
    }
}

/// Where an artifact goes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Route<'a> {
    /// The local cache and nowhere else. A decision, not an absence.
    Local,
    /// This vault takes it.
    To(&'a str, &'a VaultConfig),
    /// Nothing takes it. The reason is written for someone reading a terminal.
    Unroutable(String),
}

/// The vaults a repository declares, having been checked for coherence.
///
/// Empty is the common case and is not a degraded one: a corpus with no vault is a corpus
/// that keeps its artifacts in the local cache, which is every corpus until somebody
/// configures a store.
#[derive(Debug, Clone, Default)]
pub struct Vaults(BTreeMap<String, VaultConfig>);

impl Vaults {
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &VaultConfig)> {
        self.0.iter().map(|(k, v)| (k.as_str(), v))
    }

    pub fn get(&self, name: &str) -> Option<&VaultConfig> {
        self.0.get(name)
    }

    pub fn names(&self) -> Vec<&str> {
        self.0.keys().map(String::as_str).collect()
    }

    /// The one vault, where there is exactly one. Used only to phrase a message.
    pub fn only(&self) -> Option<(&str, &VaultConfig)> {
        match self.0.len() {
            1 => self.iter().next(),
            _ => None,
        }
    }

    /// Where an artifact of `kind` goes, given whatever its own record said.
    ///
    /// **A record's own `vault:` wins.** Routing by kind is the default a config states once;
    /// a record naming a store is a decision somebody made about those particular bytes, and
    /// the specific assertion outranks the general one. `vault: none` is such a decision.
    ///
    /// This never consults credentials, the network, or the local cache. It is a function of
    /// two committed files, so it answers identically in every clone.
    pub fn route(&self, kind: &str, record: Option<&str>) -> Route<'_> {
        if let Some(named) = record.map(str::trim).filter(|v| !v.is_empty()) {
            if named == LOCAL_ONLY {
                return Route::Local;
            }
            return match self.0.get_key_value(named) {
                Some((n, cfg)) => Route::To(n.as_str(), cfg),
                None => Route::Unroutable(format!(
                    "its record routes it to `{named}`, which `.yidam/config.toml` does not \
                     declare ({})",
                    self.declared_list()
                )),
            };
        }

        // A lone vault that claims nothing holds everything. This is #413's behaviour and a
        // corpus that upgrades into this release keeps it.
        if let Some((n, cfg)) = self.only() {
            if cfg.holds.is_none() {
                return Route::To(n, cfg);
            }
        }

        let claiming: Vec<(&str, &VaultConfig)> = self
            .iter()
            .filter(|(_, c)| c.holds().iter().any(|h| h == kind))
            .collect();
        match claiming.as_slice() {
            [(n, cfg)] => Route::To(n, cfg),
            // Two claimants cannot happen: `resolve` refuses that config. Reported rather
            // than asserted, because a panic here would be a crash over a config typo.
            [] => Route::Unroutable(format!(
                "no vault holds `{kind}` artifacts ({}).\n  \
                 Add `{kind}` to a vault's `holds` in `.yidam/config.toml`, or route the \
                 record itself with `vault:`",
                self.holdings()
            )),
            many => Route::Unroutable(format!(
                "`{kind}` is claimed by {} vaults ({})",
                many.len(),
                many.iter()
                    .map(|(n, _)| format!("`{n}`"))
                    .collect::<Vec<_>>()
                    .join(", ")
            )),
        }
    }

    fn declared_list(&self) -> String {
        if self.0.is_empty() {
            return "it declares none".to_string();
        }
        format!(
            "declared: {}",
            self.names()
                .iter()
                .map(|n| format!("`{n}`"))
                .collect::<Vec<_>>()
                .join(", ")
        )
    }

    fn holdings(&self) -> String {
        if self.0.is_empty() {
            return "this repository declares no vault".to_string();
        }
        format!(
            "declared: {}",
            self.iter()
                .map(|(n, c)| format!("`{n}` holds {}", c.holds_display()))
                .collect::<Vec<_>>()
                .join("; ")
        )
    }
}

/// Check what a repository declares, and refuse a configuration that cannot route.
///
/// Three things are settled here rather than at the artifact, because each is a defect in the
/// config whatever the corpus happens to contain:
///
/// - a vault that does not say who can read it,
/// - a `holds` entry that is not an artifact kind — a typo claims nothing,
/// - a kind claimed by two vaults, which is the ambiguity separate stores exist to remove,
/// - and, once there are two vaults, a vault that claims nothing at all.
///
/// The last one is the rule worth stating plainly: **with two or more vaults, every vault
/// declares `holds`.** The tempting alternative is to let an unclaimed vault be the catch-all
/// for whatever the others did not take, and that is precisely the silent routing an isolation
/// boundary exists to prevent — it would put a licensed PDF in the public bucket by default.
pub fn resolve(vaults: &BTreeMap<String, VaultConfig>) -> Result<Vaults> {
    for (name, cfg) in vaults {
        if cfg.audience().trim().is_empty() {
            bail!(
                "vault `{name}` does not say who can read it.\n  \
                 Add an `audience` to `[vault.{name}]` in `.yidam/config.toml` — one sentence \
                 naming who has access to this store and why that is acceptable. Nothing \
                 checks what it says; the point is that somebody wrote it down before \
                 artifacts started going there."
            );
        }
        for kind in cfg.holds() {
            if !ARTIFACT_KINDS.contains(&kind.as_str()) {
                bail!(
                    "vault `{name}` holds `{kind}`, which is not an artifact kind.\n  \
                     Known kinds: {}.\n  \
                     A kind that is not one of these claims nothing, and an artifact that \
                     should have gone here would be reported as having no route at all.",
                    ARTIFACT_KINDS.join(", ")
                );
            }
        }
    }

    if vaults.len() > 1 {
        let silent: Vec<&str> = vaults
            .iter()
            .filter(|(_, c)| c.holds().is_empty())
            .map(|(n, _)| n.as_str())
            .collect();
        if !silent.is_empty() {
            let (who, verb) = match silent.len() {
                1 => (format!("`{}`", silent[0]), "does not say what it holds"),
                _ => (
                    silent
                        .iter()
                        .map(|n| format!("`{n}`"))
                        .collect::<Vec<_>>()
                        .join(" and "),
                    "do not say what they hold",
                ),
            };
            bail!(
                "`.yidam/config.toml` declares {} vaults, and {who} {verb}.\n  \
                 With more than one vault every one of them declares `holds` — for example \
                 `holds = [\"catalog\"]`. A vault that claims nothing would have to be a \
                 catch-all for whatever the others did not take, and routing by default is \
                 how a licensed document ends up in the store meant for public output.",
                vaults.len()
            );
        }

        let mut claimed: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
        for (name, cfg) in vaults {
            for kind in cfg.holds() {
                claimed.entry(kind.as_str()).or_default().push(name);
            }
        }
        for (kind, by) in &claimed {
            if by.len() > 1 {
                bail!(
                    "`{kind}` artifacts are claimed by {} vaults ({}).\n  \
                     A kind goes to one store. Two stores for one kind is an ambiguity \
                     nothing can resolve — and resolving it silently, to whichever came \
                     first alphabetically, is the isolation failure separate stores exist to \
                     prevent.",
                    by.len(),
                    by.iter()
                        .map(|n| format!("`{n}`"))
                        .collect::<Vec<_>>()
                        .join(", ")
                );
            }
        }
    }

    Ok(Vaults(vaults.clone()))
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

    fn ok(toml_src: &str) -> Vaults {
        resolve(&parse(toml_src).unwrap()).expect("this config should resolve")
    }

    fn err(toml_src: &str) -> String {
        resolve(&parse(toml_src).unwrap())
            .map(|_| ())
            .expect_err("this config should be refused")
            .to_string()
    }

    const TWO: &str = r#"
[vault.default]
url      = "s3://corpus-artifacts/yidam"
audience = "Anyone who can read this corpus. Derived output only."
holds    = ["index", "embeddings", "bundle"]

[vault.sources]
url      = "s3://licensed-sources/yidam"
audience = "The sangha. A licence to read, not to host."
holds    = ["catalog"]
"#;

    #[test]
    fn a_repository_with_no_vault_has_no_vault_and_that_is_not_an_error() {
        assert!(ok("").is_empty());
    }

    #[test]
    fn a_lone_vault_that_claims_nothing_holds_everything() {
        let v = ok(r#"
[vault.default]
url = "file:///mnt/archive/yidam"
audience = "Anyone who can read this corpus."
"#);
        for kind in ARTIFACT_KINDS {
            assert!(
                matches!(v.route(kind, None), Route::To("default", _)),
                "{kind} must route to the only vault"
            );
        }
    }

    /// The one-vault rule is gone: a corpus may call its only store whatever it likes. What
    /// it gives up is the ambient `AWS_*` fallback, which `creds` reports when it matters.
    #[test]
    fn a_lone_vault_may_now_be_called_something_other_than_default() {
        let v = ok(r#"
[vault.archive]
url = "file:///mnt/archive"
audience = "me"
"#);
        assert!(matches!(v.route("catalog", None), Route::To("archive", _)));
    }

    /// **The routing this phase exists for.** Two audiences, and each kind reaches exactly
    /// one of them.
    #[test]
    fn each_kind_routes_to_the_vault_that_claims_it() {
        let v = ok(TWO);
        assert!(matches!(v.route("catalog", None), Route::To("sources", _)));
        assert!(matches!(v.route("index", None), Route::To("default", _)));
        assert!(matches!(v.route("bundle", None), Route::To("default", _)));
    }

    /// A record's own `vault:` outranks the config's default route. Specific beats general.
    #[test]
    fn a_records_own_vault_overrides_the_route_its_kind_would_take() {
        let v = ok(TWO);
        assert!(matches!(
            v.route("catalog", Some("default")),
            Route::To("default", _)
        ));
    }

    #[test]
    fn vault_none_is_a_route_to_the_local_cache_and_not_an_absence() {
        assert_eq!(ok(TWO).route("catalog", Some("none")), Route::Local);
        // Even for a corpus with no vault at all: the decision stands on its own.
        assert_eq!(ok("").route("catalog", Some("none")), Route::Local);
    }

    #[test]
    fn a_record_naming_an_undeclared_vault_is_unroutable_and_says_what_is_declared() {
        match ok(TWO).route("catalog", Some("archive")) {
            Route::Unroutable(why) => {
                assert!(why.contains("`archive`"), "{why}");
                assert!(why.contains("`sources`"), "names what does exist: {why}");
            }
            r => panic!("expected unroutable, got {r:?}"),
        }
    }

    /// A kind nobody claims is refused **here**, at the artifact, rather than at resolve —
    /// see the module header. The message has to carry the remedy, because the config it
    /// blames is not the file the person is looking at.
    #[test]
    fn a_kind_no_vault_claims_is_refused_when_something_of_it_needs_a_route() {
        let v = ok(r#"
[vault.default]
url = "s3://a/yidam"
audience = "everyone"
holds = ["index"]

[vault.sources]
url = "s3://b/yidam"
audience = "the sangha"
holds = ["bundle"]
"#);
        match v.route("catalog", None) {
            Route::Unroutable(why) => {
                assert!(why.contains("no vault holds `catalog`"), "{why}");
                assert!(why.contains("holds"), "carries the fix: {why}");
                assert!(why.contains("`default` holds index"), "{why}");
            }
            r => panic!("expected unroutable, got {r:?}"),
        }
    }

    /// **The load-bearing refusal.** One kind, two stores, and nothing that could pick.
    #[test]
    fn a_kind_claimed_by_two_vaults_is_refused_naming_both() {
        let e = err(r#"
[vault.default]
url = "s3://a/yidam"
audience = "everyone"
holds = ["catalog", "index"]

[vault.sources]
url = "s3://b/yidam"
audience = "the sangha"
holds = ["catalog"]
"#);
        assert!(e.contains("`catalog`"), "{e}");
        assert!(e.contains("`default`") && e.contains("`sources`"), "{e}");
        assert!(e.contains("alphabetically"), "says why not resolved: {e}");
    }

    /// A vault among several that claims nothing would be a catch-all, and a catch-all is
    /// how a licensed document reaches the public store without anybody deciding it should.
    #[test]
    fn a_second_vault_that_claims_nothing_is_refused_rather_than_made_a_catch_all() {
        let e = err(r#"
[vault.default]
url = "s3://a/yidam"
audience = "everyone"

[vault.sources]
url = "s3://b/yidam"
audience = "the sangha"
holds = ["catalog"]
"#);
        assert!(e.contains("`default`"), "names the silent one: {e}");
        assert!(
            !e.contains("`sources` and"),
            "does not blame the one that did: {e}"
        );
        assert!(e.contains("catch-all"), "{e}");
    }

    /// A misspelled kind claims nothing, and is indistinguishable from a vault that meant to
    /// claim nothing unless something says so. This is `endpiont`'s argument for `holds`.
    #[test]
    fn a_misspelled_kind_is_refused_rather_than_claiming_nothing() {
        let e = err(r#"
[vault.default]
url = "s3://a/yidam"
audience = "everyone"
holds = ["catalouge"]
"#);
        assert!(e.contains("catalouge"), "{e}");
        assert!(e.contains("catalog"), "lists the real ones: {e}");
    }

    #[test]
    fn a_vault_that_does_not_say_who_can_read_it_is_refused() {
        for src in [
            "[vault.default]\nurl = \"file:///a\"\n",
            "[vault.default]\nurl = \"file:///a\"\naudience = \"   \"\n",
        ] {
            assert!(err(src).contains("audience"), "{src}");
        }
    }

    /// A misspelled key that parses is a vault pointing somewhere nobody intended.
    #[test]
    fn an_unknown_key_is_refused_rather_than_ignored() {
        let e = parse(
            r#"
[vault.default]
url = "file:///a"
audience = "me"
endpiont = "https://typo"
"#,
        )
        .unwrap_err()
        .to_string();
        assert!(e.contains("endpiont"), "{e}");
        assert!(e.contains("holds"), "the new key is offered too: {e}");
    }

    /// A config written for S3 must survive a build that cannot reach one, so the fields the
    /// `file://` backend ignores still have to parse.
    #[test]
    fn the_s3_fields_parse_in_a_build_that_cannot_use_them() {
        let v = ok(r#"
[vault.default]
url = "s3://corpus-artifacts/yidam"
region = "us-east-1"
endpoint = "https://s3.example.net"
path_style = true
audience = "the sangha"
"#);
        let cfg = v.get("default").unwrap();
        assert_eq!(cfg.region.as_deref(), Some("us-east-1"));
        assert_eq!(cfg.path_style, Some(true));
        assert_eq!(cfg.holds_display(), "everything");
    }
}
