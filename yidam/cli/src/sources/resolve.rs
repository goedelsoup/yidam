//! An identifier, resolved through the enabled packs to the address a fetch follows (RFC-0048 §5).
//!
//! Pure and offline. It reads the packs once and answers from them, so a test builds
//! [`Enabled`] from manifests and never touches a directory or the network. What the transport
//! needs from the environment (a contact, a credential) is not read here: that is
//! [`crate::cmd::catalog::location`]'s, which takes the environment as an argument.
//!
//! # Which packs are enabled
//!
//! The ones [`super::load`] reads, less two kinds. A vendored pack that an authored pack of the
//! same name shadows is not enabled. A pack whose manifest does not parse resolves nothing, and
//! `source check` reports why. A scheme declared by two enabled packs is held as ambiguous, and
//! an identifier in it is refused rather than resolved by whichever pack sorts first.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::Result;

use super::manifest::{Archive, Manifest, Scheme, Transport};
use super::{Origin, Pack};

/// One scheme an enabled pack declares, with what resolving it needs.
#[derive(Debug, Clone)]
struct Declared {
    /// `<pack>@<version>`, the form a record's `by` takes (§6).
    pack: String,
    scheme: Scheme,
    transport: Transport,
    pattern: regex::Regex,
}

/// The schemes the enabled packs declare.
#[derive(Debug, Clone, Default)]
pub struct Enabled {
    schemes: BTreeMap<String, Declared>,
    /// A scheme two enabled packs declare, and the packs.
    ambiguous: BTreeMap<String, Vec<String>>,
}

/// An identifier, resolved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolved {
    /// `<pack>@<version>`.
    pub pack: String,
    pub scheme: String,
    /// The bound resolve template.
    pub url: String,
    /// The pack's transport, which the caller turns into a request policy.
    pub transport: Transport,
}

/// Why an identifier resolves to nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// No `:`, so there is no scheme to look up.
    NoScheme,
    /// No enabled pack declares the scheme. The one benign refusal: a corpus that pins no
    /// pack wrote `doi:` locations before packs existed, and a fetch passes over them as it
    /// did before.
    NoPack { scheme: String },
    /// Two enabled packs declare it.
    Ambiguous { scheme: String, packs: Vec<String> },
    /// The local id does not match the scheme's pattern.
    Malformed {
        scheme: String,
        local: String,
        pattern: String,
    },
    /// The template keeps a slot nothing bound. `source check` refuses such a pack, so this is
    /// a pack that was not checked.
    Unbound { pack: String, slots: Vec<String> },
    /// The bound template is not an `http(s)` address.
    NotHttp { pack: String, url: String },
    /// The pack says its publisher cannot be fetched from.
    Blocked { pack: String, why: String },
}

impl Refusal {
    pub fn message(&self, identifier: &str) -> String {
        match self {
            Self::NoScheme => {
                format!("`{identifier}` is not `scheme:local-id`, so no pack can resolve it")
            }
            Self::NoPack { scheme } => format!(
                "`{identifier}` is an identifier rather than an endpoint, and no enabled source \
                 pack declares `{scheme}`. Pin one in prelude_sources, or write one in {}",
                super::AUTHORED
            ),
            Self::Ambiguous { scheme, packs } => format!(
                "`{identifier}`: scheme `{scheme}` is declared by {}, so nothing says which \
                 resolves it. `yidam source check` reports the same",
                packs.join(" and ")
            ),
            Self::Malformed {
                scheme,
                local,
                pattern,
            } => format!(
                "`{identifier}`: `{local}` does not match [scheme.{scheme}] pattern `{pattern}`"
            ),
            Self::Unbound { pack, slots } => format!(
                "`{identifier}`: {pack} leaves {} unbound in its resolve template. \
                 `yidam source check` reports the pack",
                slots
                    .iter()
                    .map(|s| format!("`{{{s}}}`"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Self::NotHttp { pack, url } => format!(
                "`{identifier}` resolves through {pack} to `{url}`, which is not an http(s) address"
            ),
            Self::Blocked { pack, why } => format!(
                "`{identifier}`: {pack} declares its publisher blocked — {why}. Nothing works \
                 around a block; record a manual export as a `kind: file` location"
            ),
        }
    }

    /// Only [`Self::NoPack`]: see its note.
    pub fn is_benign(&self) -> bool {
        matches!(self, Self::NoPack { .. })
    }
}

impl Enabled {
    /// The enabled packs in the repository at `root`.
    pub fn load(root: &Path) -> Result<Self> {
        Ok(Self::from_packs(&super::load(root)?))
    }

    pub fn from_packs(packs: &[Pack]) -> Self {
        let authored: BTreeSet<&str> = packs
            .iter()
            .filter(|p| p.origin == Origin::Authored)
            .map(|p| p.name.as_str())
            .collect();
        let mut by_scheme: BTreeMap<String, Vec<Declared>> = BTreeMap::new();
        for pack in packs {
            if pack.origin == Origin::Vendored && authored.contains(pack.name.as_str()) {
                continue;
            }
            let Ok(m) = &pack.manifest else { continue };
            for (name, d) in declared(m) {
                by_scheme.entry(name).or_default().push(d);
            }
        }
        let mut out = Self::default();
        for (scheme, mut ds) in by_scheme {
            if ds.len() == 1 {
                if let Some(d) = ds.pop() {
                    out.schemes.insert(scheme, d);
                }
            } else {
                out.ambiguous
                    .insert(scheme, ds.into_iter().map(|d| d.pack).collect());
            }
        }
        out
    }

    /// Whether no enabled pack declares any scheme: the corpus lists no packs, or none it
    /// lists parses.
    pub fn is_empty(&self) -> bool {
        self.schemes.is_empty() && self.ambiguous.is_empty()
    }

    /// Whether `scheme:local-id` names something an enabled pack declares, as
    /// `catalog-location-malformed` asks it (RFC-0048 §2): the scheme is declared, and the
    /// local id matches its pattern. Only [`Refusal::NoPack`] and [`Refusal::Malformed`] come
    /// back. An ambiguous or blocked scheme, or a template that binds badly, is the pack's
    /// defect and `source check`'s finding, not the location's.
    pub fn admits(&self, identifier: &str) -> Result<(), Refusal> {
        let Some((scheme, local)) = identifier.split_once(':') else {
            return Ok(());
        };
        if self.ambiguous.contains_key(scheme) {
            return Ok(());
        }
        let Some(d) = self.schemes.get(scheme) else {
            return Err(Refusal::NoPack {
                scheme: scheme.to_string(),
            });
        };
        if d.pattern.is_match(local) {
            Ok(())
        } else {
            Err(Refusal::Malformed {
                scheme: scheme.to_string(),
                local: local.to_string(),
                pattern: d.scheme.pattern.clone(),
            })
        }
    }

    /// Whether an enabled pack declares `scheme`, unambiguously.
    pub fn declares(&self, scheme: &str) -> bool {
        self.schemes.contains_key(scheme)
    }

    /// The transport of the pack that declares `scheme`.
    pub fn transport_of(&self, scheme: &str) -> Option<(&str, &Transport)> {
        self.schemes
            .get(scheme)
            .map(|d| (d.pack.as_str(), &d.transport))
    }

    /// Resolve `scheme:local-id` to the address a fetch follows.
    pub fn resolve(&self, identifier: &str) -> Result<Resolved, Refusal> {
        let Some((scheme, local)) = identifier.split_once(':') else {
            return Err(Refusal::NoScheme);
        };
        if let Some(packs) = self.ambiguous.get(scheme) {
            return Err(Refusal::Ambiguous {
                scheme: scheme.to_string(),
                packs: packs.clone(),
            });
        }
        let Some(d) = self.schemes.get(scheme) else {
            return Err(Refusal::NoPack {
                scheme: scheme.to_string(),
            });
        };
        if let Some(why) = &d.transport.blocked {
            return Err(Refusal::Blocked {
                pack: d.pack.clone(),
                why: why.clone(),
            });
        }
        let Some(caps) = d.pattern.captures(local) else {
            return Err(Refusal::Malformed {
                scheme: scheme.to_string(),
                local: local.to_string(),
                pattern: d.scheme.pattern.clone(),
            });
        };
        // The same bindings `source check` binds a fixture with: `{id}`, then each named group.
        let mut bindings = vec![("id".to_string(), local.to_string())];
        for group in d.pattern.capture_names().flatten() {
            if let Some(v) = caps.name(group) {
                bindings.push((group.to_string(), v.as_str().to_string()));
            }
        }
        let url = crate::cmd::catalog::location::bind(&d.scheme.resolve.template, &bindings)
            .map_err(|slots| Refusal::Unbound {
                pack: d.pack.clone(),
                slots,
            })?;
        if !(url.starts_with("https://") || url.starts_with("http://")) {
            return Err(Refusal::NotHttp {
                pack: d.pack.clone(),
                url,
            });
        }
        Ok(Resolved {
            pack: d.pack.clone(),
            scheme: scheme.to_string(),
            url,
            transport: d.transport.clone(),
        })
    }
}

/// Each scheme a pack the template ships in `yidam/sources/` declares, and that pack.
///
/// A corpus lints with a binary, not a template checkout, so this is how a finding about an
/// undeclared scheme names the pack to pin. `template_source_packs.rs` holds it to the
/// manifests in both directions.
pub const TEMPLATE_SCHEMES: &[(&str, &str)] = &[
    ("arxiv", "scholarly"),
    ("doi", "scholarly"),
    ("europepmc", "scholarly"),
    ("ia", "archive"),
    ("ia-file", "archive"),
    ("pmc", "scholarly"),
    ("wayback", "archive"),
];

/// The template pack that declares `scheme`, if one does.
pub fn template_pack(scheme: &str) -> Option<&'static str> {
    TEMPLATE_SCHEMES
        .iter()
        .find(|(s, _)| *s == scheme)
        .map(|(_, p)| *p)
}

/// Whether a pack with this transport offers `--archive`.
pub fn archives(t: &Transport) -> bool {
    t.archive == Some(Archive::Wayback)
}

/// The scheme `--archive` writes, which some enabled pack has to declare.
pub const WAYBACK: &str = "wayback";

/// A manifest's schemes whose patterns compile. One that does not is `source check`'s finding,
/// and resolves nothing.
fn declared(m: &Manifest) -> Vec<(String, Declared)> {
    let pack = format!("{}@{}", m.pack.name, m.pack.version);
    m.scheme
        .iter()
        .filter_map(|(name, s)| {
            let pattern = regex::Regex::new(&s.pattern).ok()?;
            Some((
                name.clone(),
                Declared {
                    pack: pack.clone(),
                    scheme: s.clone(),
                    transport: m.transport.clone(),
                    pattern,
                },
            ))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn pack(name: &str, origin: Origin, toml_src: &str) -> Pack {
        Pack {
            name: name.into(),
            dir: PathBuf::from(format!(".yidam/sources/{name}")),
            origin,
            manifest: toml::from_str(toml_src).map_err(|e| e.to_string()),
        }
    }

    const SCHOLARLY: &str = r#"
[pack]
name = "scholarly"
version = "0.1.0"

[scheme.doi]
pattern = '^10\.\d{4,9}/\S+$'
type = "paper"
resolve = { template = "https://api.crossref.org/works/{id}" }

[transport]
contact = "required"
archive = "wayback"
"#;

    const ARCHIVE: &str = r#"
[pack]
name = "archive"
version = "0.1.0"

[scheme.wayback]
pattern = '^(?P<ts>\d{14})/(?P<url>https?://\S+)$'
type = "document"
resolve = { template = "https://web.archive.org/web/{ts}id_/{url}" }
"#;

    #[test]
    fn a_doi_resolves_through_its_template() {
        let e = Enabled::from_packs(&[pack("scholarly", Origin::Authored, SCHOLARLY)]);
        let r = e.resolve("doi:10.1167/tvst.8.5.14").unwrap();
        assert_eq!(r.url, "https://api.crossref.org/works/10.1167/tvst.8.5.14");
        assert_eq!(r.pack, "scholarly@0.1.0");
        assert!(archives(&r.transport));
    }

    /// The Wayback location 165 measured entries already use is the `id_` form: the publisher's
    /// bytes, without the archive's toolbar spliced into them.
    #[test]
    fn a_wayback_identifier_resolves_to_the_id_form() {
        let e = Enabled::from_packs(&[pack("archive", Origin::Vendored, ARCHIVE)]);
        let r = e
            .resolve("wayback:20240102030405/https://www.bls.gov/cpi/data.htm?x=1")
            .unwrap();
        assert_eq!(
            r.url,
            "https://web.archive.org/web/20240102030405id_/https://www.bls.gov/cpi/data.htm?x=1"
        );
    }

    #[test]
    fn admits_answers_only_for_the_scheme_and_the_pattern() {
        let e = Enabled::from_packs(&[pack("scholarly", Origin::Authored, SCHOLARLY)]);
        assert!(!e.is_empty());
        assert_eq!(e.admits("doi:10.1167/tvst.8.5.14"), Ok(()));
        assert!(matches!(
            e.admits("doi:not-a-doi"),
            Err(Refusal::Malformed { .. })
        ));
        assert!(matches!(
            e.admits("isbn:9780262033848"),
            Err(Refusal::NoPack { .. })
        ));
        // Two packs declaring `doi` is the packs' defect, which `source check` reports.
        let twice = Enabled::from_packs(&[
            pack("scholarly", Origin::Authored, SCHOLARLY),
            pack(
                "other",
                Origin::Authored,
                &SCHOLARLY.replace("scholarly", "other"),
            ),
        ]);
        assert_eq!(twice.admits("doi:not-a-doi"), Ok(()));
        assert!(Enabled::from_packs(&[]).is_empty());
    }

    #[test]
    fn a_local_id_the_pattern_refuses_is_malformed() {
        let e = Enabled::from_packs(&[pack("scholarly", Origin::Authored, SCHOLARLY)]);
        let err = e.resolve("doi:not-a-doi").unwrap_err();
        assert!(matches!(err, Refusal::Malformed { .. }), "{err:?}");
        assert!(!err.is_benign());
    }

    /// A corpus pinning no pack keeps today's behaviour: its identifiers are passed over.
    #[test]
    fn an_undeclared_scheme_is_the_one_benign_refusal() {
        let e = Enabled::default();
        let err = e.resolve("doi:10.1/a").unwrap_err();
        assert_eq!(
            err,
            Refusal::NoPack {
                scheme: "doi".into()
            }
        );
        assert!(err.is_benign());
        assert!(err.message("doi:10.1/a").contains("prelude_sources"));
        assert!(!Enabled::default()
            .resolve("nocolon")
            .unwrap_err()
            .is_benign());
    }

    #[test]
    fn an_authored_pack_shadows_the_vendored_one_it_names() {
        let authored = SCHOLARLY.replace("api.crossref.org", "mirror.example");
        let e = Enabled::from_packs(&[
            pack("scholarly", Origin::Authored, &authored),
            pack("scholarly", Origin::Vendored, SCHOLARLY),
        ]);
        assert_eq!(
            e.resolve("doi:10.1167/a").unwrap().url,
            "https://mirror.example/works/10.1167/a"
        );
    }

    #[test]
    fn a_scheme_two_packs_declare_resolves_through_neither() {
        let other = SCHOLARLY.replace("name = \"scholarly\"", "name = \"other\"");
        let e = Enabled::from_packs(&[
            pack("other", Origin::Authored, &other),
            pack("scholarly", Origin::Authored, SCHOLARLY),
        ]);
        let err = e.resolve("doi:10.1/a").unwrap_err();
        assert!(matches!(err, Refusal::Ambiguous { .. }), "{err:?}");
        assert!(!e.declares("doi"));
    }

    #[test]
    fn a_blocked_publisher_is_refused_with_the_packs_reason() {
        let blocked = SCHOLARLY.replace(
            "[transport]",
            "[transport]\nblocked = \"403 to every automated client\"",
        );
        let e = Enabled::from_packs(&[pack("scholarly", Origin::Authored, &blocked)]);
        let err = e.resolve("doi:10.1/a").unwrap_err();
        let msg = err.message("doi:10.1/a");
        assert!(msg.contains("403 to every automated client"), "{msg}");
        assert!(msg.contains("kind: file"), "{msg}");
        assert!(!err.is_benign());
    }

    #[test]
    fn a_pack_that_does_not_parse_resolves_nothing() {
        let e = Enabled::from_packs(&[pack("broken", Origin::Authored, "[pack]\nname = 1")]);
        assert!(!e.declares("doi"));
    }
}
