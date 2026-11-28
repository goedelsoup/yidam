use super::*;

/// Are the vaults configured, and is everything they need in place — **without asking them**.
///
/// `doctor` is documented read-only and offline, and this does not change that. Whether a
/// store is *reachable* is `yidam vault status --remote`'s question, and answering it here
/// would put a network call in the one command a person runs when the network is what they
/// suspect.
///
/// So what is checked is everything that can be settled locally: that the declared vaults
/// resolve, that each says who can read it, that each has its credentials in the environment,
/// that no artifact the corpus names has been left without a route, and that the artifacts are
/// cached. Each of those fails in a way that produces an unhelpful error much later — a `403`
/// names no variable, and a missing artifact surfaces as a broken citation.
///
/// # The one warning that is not about a failure
///
/// Two vaults resolving to the same credentials is **legal** — a corpus may genuinely use one
/// account and two buckets, and this repository is not in a position to say otherwise. It is
/// also exactly what a half-finished isolation setup looks like: somebody declared `sources`,
/// exported nothing for it, and it is quietly running on the account meant for public output.
/// The two are indistinguishable from here, so this reports the shape and lets the reader
/// decide, which is the only honest thing available.
pub(super) fn check_vault(root: &Path) -> Answer {
    let config = crate::config::load_yidam_config(root).unwrap_or_default();
    let vaults =
        match crate::vault::resolve(&config.vault) {
            Err(e) => return Answer::fail(
                first_line(&e.to_string()),
                Some(
                    "Fix `[vault.…]` in `.yidam/config.toml`; `yidam vault list` shows the shape.",
                ),
            ),
            Ok(v) => v,
        };

    let named = crate::vault::named_artifacts(root);
    if vaults.is_empty() {
        // No vault is the common case and is not a defect — unless the corpus has already
        // started recording artifacts, which means it is relying on somewhere to keep them.
        return if named.is_empty() {
            Answer::ok("none declared")
        } else {
            Answer::warn(
                format!("{} artifact(s) recorded and no vault declared", named.len()),
                Some(
                    "Declare `[vault.default]` in `.yidam/config.toml`, or the bytes live \
                      only in whichever caches happen to hold them.",
                ),
            )
        };
    }

    // A record the config cannot place. Reported here rather than by `lint`, because the
    // defect is in `.yidam/config.toml` and lint's subject is the corpus — blaming a catalog
    // entry for a store nobody declared would point at the wrong file.
    let stranded: Vec<&crate::vault::Named> = named
        .iter()
        .filter(|a| {
            matches!(
                vaults.route(&a.kind, a.vault.as_deref()),
                crate::vault::Route::Unroutable(_)
            )
        })
        .collect();
    if let Some(first) = stranded.first() {
        let why = match vaults.route(&first.kind, first.vault.as_deref()) {
            crate::vault::Route::Unroutable(w) => first_line(&w),
            _ => unreachable!("filtered to unroutable"),
        };
        return Answer::warn(
            format!("{} artifact(s) have no route — {why}", stranded.len()),
            Some(
                "Add the kind to a vault's `holds` in `.yidam/config.toml`, or route the \
                  record itself with `vault:`.",
            ),
        );
    }

    // Credentials, for the stores that need them. A `file://` vault needs none, and demanding
    // them would report a healthy setup as broken.
    let mut principals: BTreeMap<String, Vec<&str>> = BTreeMap::new();
    // Where each vault's credentials came from (#1234). With static keys, a role and an ambient
    // fallback all possible, "has credentials" no longer says which ones a vault will sign with.
    let mut origins: Vec<String> = Vec::new();
    for (name, cfg) in vaults.iter() {
        if !cfg.url.trim().starts_with("s3://") {
            continue;
        }
        match crate::vault::credentials_available(name) {
            Err(e) => {
                return Answer::warn(
                    first_line(&e.to_string()),
                    Some(
                        "Credentials come from the environment only — `.yidam/config.toml` is \
                          committed and must never carry one.",
                    ),
                );
            }
            Ok(origin) if !origin.is_empty() => origins.push(format!("`{name}` {origin}")),
            Ok(_) => {}
        }
        if let Some(id) = crate::vault::credential_principal(name) {
            principals.entry(id).or_default().push(name);
        }
    }
    // The access key id is compared and never printed: it identifies the account, and naming
    // it in a report that gets pasted into an issue helps nobody.
    if let Some((_, shared)) = principals.iter().find(|(_, v)| v.len() > 1) {
        return Answer::warn(
            format!(
                "{} resolve to the same credentials",
                shared
                    .iter()
                    .map(|n| format!("`{n}`"))
                    .collect::<Vec<_>>()
                    .join(" and ")
            ),
            Some(
                "Legal — one account can own two buckets. It is also what an unfinished \
                  isolation setup looks like; export YIDAM_VAULT_<NAME>_ACCESS_KEY_ID, or \
                  YIDAM_VAULT_<NAME>_ROLE_ARN, per vault if they were meant to differ.",
            ),
        );
    }

    let cache = match crate::vault::Cache::resolve(|k| std::env::var(k).ok()) {
        Ok(c) => c,
        Err(e) => {
            return Answer::warn(
                first_line(&e.to_string()),
                Some("Set YIDAM_VAULT_CACHE to say where artifacts should live."),
            )
        }
    };
    let configured = format!(
        "{} vault{} configured{}",
        vaults.len(),
        if vaults.len() == 1 { "" } else { "s" },
        match origins.is_empty() {
            true => String::new(),
            false => format!(" ({})", origins.join(", ")),
        }
    );
    let uncached = named.iter().filter(|a| !cache.contains(&a.hash)).count();
    if uncached > 0 {
        return Answer::warn(
            format!(
                "{configured}; {uncached} of {} recorded artifact(s) not cached",
                named.len()
            ),
            Some(
                "`yidam vault pull` fetches them. Reachability is `yidam vault status \
                  --remote`; this check makes no network call.",
            ),
        );
    }
    Answer::ok(format!("{configured}; {} artifact(s) cached", named.len()))
}

/// The first line of an error, for a one-line verdict.
pub(super) fn first_line(s: &str) -> String {
    s.lines().next().unwrap_or_default().trim().to_string()
}
