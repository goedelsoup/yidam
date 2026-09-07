//! Turning a declared address into something that can be followed — or saying why not.
//!
//! `CATALOG_LOCATION_KINDS` has been a closed set of four since it was written, and
//! `catalog-location-malformed` has enforced that a `url_template` carries a `{…}` slot.
//! Both facts describe an address as *dereferenceable*, and until now nothing dereferenced
//! one: `url_template` occurred four times in `src/`, twice in the type and twice in the
//! check.
//!
//! # Why this module holds no transport
//!
//! Everything here is pure and ungated, and the bytes are fetched elsewhere. That split is
//! not tidiness — it is the mitigation #460 names for the failure mode it calls *the
//! feature-gate blind spot*:
//!
//! > Gated code never compiles in PR CI and breaks at release. […] Anything gated takes
//! > gated facts as arguments.
//!
//! So the decision of *what would be fetched* — which is where every interesting error
//! lives: an unbound slot, a path escaping the repository, a kind nothing can follow — is
//! made in code every pull request compiles and every test run exercises. What remains
//! behind the feature is a function from a URL to bytes, which is the one part a gate cannot
//! make wrong in an interesting way.

use std::path::{Component, Path, PathBuf};

use crate::parse::CatalogLocation;

/// What following one location would actually do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Plan {
    /// Read a file inside the repository. No network, and available in every build.
    ///
    /// The path is already resolved against the root and already checked not to leave it.
    File { path: PathBuf, declared: String },
    /// `GET` an absolute URL.
    ///
    /// Produced by `kind: url` verbatim and by `kind: url_template` once every slot is
    /// bound. The two collapse here on purpose: a template with its slots filled *is* a URL,
    /// and keeping them distinct past this point would mean the transport had to know about
    /// templating, which is exactly the knowledge that belongs on the ungated side.
    Url { url: String, declared: String },
}

/// Why an address cannot be followed.
///
/// Each variant is a different repair, and they are kept apart rather than collapsed into
/// one string because the caller renders them differently: two of them are a corpus author's
/// problem, one is a caller's problem, and one is not a problem at all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unfollowable {
    /// `kind: address` — a reading room, a records office, a person.
    ///
    /// **Not an error.** The kind is in the closed set because it is a real way to hold a
    /// source, and four of the seven locations across this repository's own examples are
    /// one. An entry whose only address is a physical one is fully valid and is simply not
    /// something a fetch has anything to do; the caller skips it rather than failing.
    NotAnEndpoint { declared: String },
    /// A `url_template` whose slots nothing bound.
    ///
    /// The slots are named so the message can say `--bind site=…` rather than "some
    /// substitution is missing", which is the difference between a message that repairs
    /// itself and one that sends a reader back to the file.
    Unbound {
        slots: Vec<String>,
        declared: String,
    },
    /// A `kind: file` path that leaves the repository.
    ///
    /// Refused rather than resolved. A catalog entry is a committed file that anyone may
    /// send a pull request against, so a value of `../../../etc/passwd` is untrusted input
    /// by construction — and a fetch that followed it would read a file outside the
    /// repository and then commit its digest as something this corpus obtained.
    Escapes { declared: String },
    /// A kind outside the closed set, or a location with no value at all.
    ///
    /// `lint` reports both — `catalog-location-malformed` covers the kind and the empty
    /// value. Repeated here rather than assumed, because a fetch is reachable on a corpus
    /// whose gate has not been run and must not act on a location it cannot read.
    Unreadable { why: String },
}

impl Unfollowable {
    /// Whether this is a location a fetch should pass over in silence rather than report.
    ///
    /// Only [`Self::NotAnEndpoint`]. An entry listing a records office and a URL should
    /// fetch the URL and say nothing about the office; an entry listing only the office
    /// should say it has nothing to fetch, which the caller decides from the *set* of
    /// outcomes rather than from any one of them.
    pub fn is_benign(&self) -> bool {
        matches!(self, Self::NotAnEndpoint { .. })
    }

    pub fn message(&self) -> String {
        match self {
            Self::NotAnEndpoint { declared } => format!(
                "`{declared}` is a place rather than an endpoint — `kind: address` records \
                 where a source is held, and nothing fetches one"
            ),
            Self::Unbound { slots, declared } => format!(
                "`{declared}` has {} nothing bound: {}. Supply {} — for example \
                 `--bind {}=…`",
                if slots.len() == 1 { "a slot" } else { "slots" },
                slots
                    .iter()
                    .map(|s| format!("`{{{s}}}`"))
                    .collect::<Vec<_>>()
                    .join(", "),
                if slots.len() == 1 { "it" } else { "each" },
                slots[0]
            ),
            Self::Escapes { declared } => format!(
                "`{declared}` leaves the repository. A `kind: file` location names a path \
                 inside the corpus, and one that climbs out of it is refused rather than \
                 followed"
            ),
            Self::Unreadable { why } => why.clone(),
        }
    }
}

/// The slot names in a template, in the order they appear, without duplicates.
///
/// Order is the template's rather than sorted, because the message that names them reads
/// alongside the value it came from and a reader is matching them off left to right.
pub fn slots(template: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut rest = template;
    while let Some(open) = rest.find('{') {
        let after = &rest[open + 1..];
        let Some(close) = after.find('}') else {
            break;
        };
        let name = &after[..close];
        // `{}` is not a slot and neither is `{ }`. A template carrying one is malformed
        // rather than parameterised, and inventing an empty-named binding for it would make
        // the refusal message unreadable.
        if !name.trim().is_empty() && !out.iter().any(|s| s == name) {
            out.push(name.to_string());
        }
        rest = &after[close + 1..];
    }
    out
}

/// Substitute every `{slot}` a binding names, and report what is left.
fn bind(template: &str, bindings: &[(String, String)]) -> Result<String, Vec<String>> {
    let mut filled = template.to_string();
    for (k, v) in bindings {
        filled = filled.replace(&format!("{{{k}}}"), v);
    }
    let remaining = slots(&filled);
    if remaining.is_empty() {
        Ok(filled)
    } else {
        Err(remaining)
    }
}

/// Resolve a `kind: file` value against the repository root, refusing anything that leaves.
///
/// Lexical, and deliberately so: this runs before the file is opened, so it must not depend
/// on what exists. `canonicalize` would answer differently for a path that is absent, and
/// "absent" is a perfectly ordinary state for a location a corpus declares and has not
/// obtained yet — refusing it as an escape would be wrong, and following it because
/// canonicalisation failed would be worse.
///
/// A symlink inside the corpus pointing out of it is not covered here and is not this
/// module's to cover; the digest lands in the commit either way, which is RFC-0023's answer
/// to bytes whose provenance is in doubt.
fn under_root(root: &Path, declared: &str) -> Option<PathBuf> {
    let rel = Path::new(declared);
    if rel.is_absolute() {
        return None;
    }
    let mut depth: i32 = 0;
    for c in rel.components() {
        match c {
            Component::ParentDir => {
                depth -= 1;
                if depth < 0 {
                    return None;
                }
            }
            Component::Normal(_) => depth += 1,
            Component::CurDir => {}
            // A prefix or a root component on a relative path is not something to reason
            // about; refuse rather than guess.
            Component::Prefix(_) | Component::RootDir => return None,
        }
    }
    Some(root.join(rel))
}

/// What following this location would do, or why nothing can.
pub fn resolve(
    location: &CatalogLocation,
    root: &Path,
    bindings: &[(String, String)],
) -> Result<Plan, Unfollowable> {
    let kind = location.kind.as_deref().unwrap_or("").trim();
    let value = location.value.as_deref().unwrap_or("").trim();
    if value.is_empty() {
        return Err(Unfollowable::Unreadable {
            why: format!("a `kind: {kind}` location declares no value"),
        });
    }
    let declared = value.to_string();
    match kind {
        "address" => Err(Unfollowable::NotAnEndpoint { declared }),
        "url" => Ok(Plan::Url {
            url: declared.clone(),
            declared,
        }),
        "url_template" => match bind(value, bindings) {
            Ok(url) => Ok(Plan::Url { url, declared }),
            Err(slots) => Err(Unfollowable::Unbound { slots, declared }),
        },
        "file" => match under_root(root, value) {
            Some(path) => Ok(Plan::File { path, declared }),
            None => Err(Unfollowable::Escapes { declared }),
        },
        other => Err(Unfollowable::Unreadable {
            why: format!(
                "`{other}` is not a location kind — the set is {}",
                crate::parse::CATALOG_LOCATION_KINDS.join(", ")
            ),
        }),
    }
}

/// Parse a `--bind name=value` argument.
///
/// Split on the *first* `=` so a value may contain one. Query-string values routinely do,
/// and a binding that refused them would be unusable against exactly the templates this
/// exists for.
pub fn parse_binding(raw: &str) -> anyhow::Result<(String, String)> {
    let Some((k, v)) = raw.split_once('=') else {
        anyhow::bail!("`{raw}` is not a binding — write `--bind name=value`");
    };
    let k = k.trim();
    if k.is_empty() {
        anyhow::bail!("`{raw}` binds nothing — the name before `=` is empty");
    }
    Ok((k.to_string(), v.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn loc(kind: &str, value: &str) -> CatalogLocation {
        CatalogLocation {
            kind: Some(kind.to_string()),
            value: Some(value.to_string()),
            description: None,
        }
    }

    fn binds(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect()
    }

    #[test]
    fn slots_are_named_in_order_and_deduplicated() {
        assert_eq!(
            slots("https://x/?sites={site}&p={param}&also={site}"),
            vec!["site", "param"]
        );
        assert_eq!(slots("https://x/no-slots"), Vec::<String>::new());
    }

    /// The lint check only asserts a `{` is present. `{}` satisfies that and binds nothing,
    /// so the planner must not treat it as a slot it could ask a caller to fill.
    #[test]
    fn an_empty_brace_is_not_a_slot() {
        assert_eq!(slots("https://x/{}"), Vec::<String>::new());
        assert_eq!(slots("https://x/{  }"), Vec::<String>::new());
    }

    #[test]
    fn an_unclosed_brace_ends_the_scan_rather_than_looping() {
        assert_eq!(slots("https://x/{site"), Vec::<String>::new());
        assert_eq!(slots("https://x/{a}/{b"), vec!["a"]);
    }

    #[test]
    fn a_url_is_followed_verbatim() {
        let root = Path::new("/repo");
        assert_eq!(
            resolve(&loc("url", "https://waterdata.usgs.gov/nwis"), root, &[]),
            Ok(Plan::Url {
                url: "https://waterdata.usgs.gov/nwis".into(),
                declared: "https://waterdata.usgs.gov/nwis".into(),
            })
        );
    }

    /// The streamflow entry's own template, which is the case this whole issue is named
    /// after: the linter validates it and nothing has ever followed it.
    #[test]
    fn a_bound_template_becomes_a_url() {
        let root = Path::new("/repo");
        let t =
            "https://waterservices.usgs.gov/nwis/iv/?sites={site}&parameterCd=00060&format=json";
        let plan = resolve(
            &loc("url_template", t),
            root,
            &binds(&[("site", "09380000")]),
        );
        assert_eq!(
            plan,
            Ok(Plan::Url {
                url: "https://waterservices.usgs.gov/nwis/iv/?sites=09380000&parameterCd=00060&format=json".into(),
                declared: t.into(),
            })
        );
    }

    /// Every slot must be bound. A template half-filled would fetch a URL containing a
    /// literal `{cik}`, which a server answers with something — and that something would be
    /// committed as an artifact this corpus obtained.
    #[test]
    fn a_template_with_an_unbound_slot_is_refused_and_names_it() {
        let root = Path::new("/repo");
        let err = resolve(
            &loc("url_template", "https://x/?a={one}&b={two}"),
            root,
            &binds(&[("one", "1")]),
        )
        .unwrap_err();
        assert_eq!(
            err,
            Unfollowable::Unbound {
                slots: vec!["two".into()],
                declared: "https://x/?a={one}&b={two}".into(),
            }
        );
        assert!(err.message().contains("--bind two="), "{}", err.message());
        assert!(!err.is_benign());
    }

    /// A binding whose value itself contains braces must not be re-scanned as a slot —
    /// otherwise a legitimate value makes the template permanently unfillable.
    #[test]
    fn a_binding_value_containing_braces_does_not_reopen_a_slot() {
        let root = Path::new("/repo");
        let plan = resolve(
            &loc("url_template", "https://x/?q={q}"),
            root,
            &binds(&[("q", "{literal}")]),
        );
        assert!(
            matches!(plan, Err(Unfollowable::Unbound { .. })),
            "a value that reintroduces a slot is refused rather than fetched: {plan:?}"
        );
    }

    #[test]
    fn an_address_is_benign_rather_than_an_error() {
        let root = Path::new("/repo");
        let err = resolve(
            &loc("address", "Vantry County Recorder, 14 Court St"),
            root,
            &[],
        )
        .unwrap_err();
        assert!(err.is_benign());
        assert!(err.message().contains("place rather than an endpoint"));
    }

    #[test]
    fn a_file_resolves_under_the_root() {
        let root = Path::new("/repo");
        assert_eq!(
            resolve(&loc("file", "sources/registry.csv"), root, &[]),
            Ok(Plan::File {
                path: PathBuf::from("/repo/sources/registry.csv"),
                declared: "sources/registry.csv".into(),
            })
        );
    }

    /// A catalog entry is a committed file anyone may send a pull request against, so its
    /// `value` is untrusted input. This is the case that makes it matter.
    #[test]
    fn a_file_climbing_out_of_the_repository_is_refused() {
        let root = Path::new("/repo");
        for escape in ["../secrets", "a/../../secrets", "/etc/passwd"] {
            let err = resolve(&loc("file", escape), root, &[]).unwrap_err();
            assert_eq!(
                err,
                Unfollowable::Escapes {
                    declared: escape.into()
                },
                "{escape} must not resolve"
            );
            assert!(!err.is_benign());
        }
    }

    /// Climbing and coming back is not an escape. `a/../b` never leaves, and refusing it
    /// would be a guard that fires on a shape rather than on the thing it guards against.
    #[test]
    fn a_path_that_climbs_within_the_root_still_resolves() {
        let root = Path::new("/repo");
        assert_eq!(
            resolve(&loc("file", "a/../b/data.json"), root, &[]),
            Ok(Plan::File {
                path: PathBuf::from("/repo/a/../b/data.json"),
                declared: "a/../b/data.json".into(),
            })
        );
    }

    #[test]
    fn an_unknown_kind_and_an_empty_value_are_both_unreadable() {
        let root = Path::new("/repo");
        assert!(matches!(
            resolve(&loc("ftp", "ftp://x/f"), root, &[]),
            Err(Unfollowable::Unreadable { .. })
        ));
        assert!(matches!(
            resolve(&loc("url", "   "), root, &[]),
            Err(Unfollowable::Unreadable { .. })
        ));
    }

    #[test]
    fn a_binding_splits_on_the_first_equals_so_values_may_contain_one() {
        assert_eq!(
            parse_binding("q=a=b&c=d").unwrap(),
            ("q".to_string(), "a=b&c=d".to_string())
        );
        assert!(parse_binding("nope").is_err());
        assert!(parse_binding("=v").is_err());
    }
}
