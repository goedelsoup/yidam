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

use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::Result;

use super::lookup::{self, Base, Lookup, Page};
use super::manifest::{Archive, Form, Manifest, Scheme, Transport};
use super::pinning::Pinning;
use super::{Origin, Pack};

/// One scheme an enabled pack declares, with what resolving it needs.
#[derive(Debug, Clone)]
struct Declared {
    /// `<pack>@<version>`, the form a record's `by` takes (§6).
    pack: String,
    scheme: Scheme,
    transport: Transport,
    pattern: regex::Regex,
    /// The pin read's `mutable`, compiled. `None` with no pin read, or one that does not
    /// compile, which `source check` reports.
    mutable: Option<regex::Regex>,
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
    /// The bound resolve template, or the pin joined to the page it was read off.
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
    /// The scheme reads its address off a page, and the identifier names no `@<pin>` saying
    /// which file was read. A fetch never reads the page itself (#1342).
    Unpinned { pack: String, page: String },
    /// The pin names something other than one matching file on the page's host.
    Pin {
        pack: String,
        pin: String,
        why: String,
    },
    /// The scheme's resolve cannot be used as written. `source check` reports the pack.
    Unusable { pack: String, why: String },
    /// The local id is the mutable form the scheme's pin read reads, which names whatever the
    /// publisher holds today. A fetch never makes the read (#1343).
    Mutable { pack: String, read: String },
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
            Self::Unpinned { pack, page } => format!(
                "`{identifier}`: {pack} reads its address off `{page}`, and the identifier \
                 pins no `@<name>` it read there. A fetch follows only a pinned identifier; \
                 `yidam source add --dry-run {identifier}` reads the page and prints one"
            ),
            Self::Mutable { pack, read } => format!(
                "`{identifier}` names whatever {pack}'s publisher holds today, and a location \
                 names one version. `yidam source add --dry-run {identifier}` asks `{read}` \
                 and prints the identifier pinned to it"
            ),
            Self::Unusable { pack, why } => format!(
                "`{identifier}`: {pack}'s resolve {why}. `yidam source check` reports the pack"
            ),
            Self::Pin { pack, pin, why } => {
                format!("`{identifier}`: the pin `{pin}` {why}, so {pack} resolves it to nothing")
            }
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

    /// One pack's schemes, as if it were the only pack enabled: what `source check` resolves
    /// the pack's own fixtures through.
    pub fn from_manifest(m: &Manifest) -> Self {
        Self {
            schemes: declared(m).into_iter().collect(),
            ambiguous: BTreeMap::new(),
        }
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
        // A lookup scheme's pin is not part of the local id its pattern reads.
        let local = match local.split_once('@') {
            Some((l, _)) if d.scheme.resolve.form().is_ok_and(|f| f.looks_up()) => l,
            _ => local,
        };
        // A mutable local id is well formed: it is refused at resolve, with how to pin it.
        let mutable = d.mutable.as_ref().is_some_and(|m| m.is_match(local));
        if d.pattern.is_match(local) || mutable {
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

    /// The pack that declares `identifier`'s scheme, and the identifier as its pattern reads it.
    fn split<'a>(&'a self, identifier: &'a str) -> Result<Split<'a>, Refusal> {
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
        // `declared` holds only schemes whose resolve is one form.
        let form = d.scheme.resolve.form().map_err(|why| Refusal::Unusable {
            pack: d.pack.clone(),
            why,
        })?;
        // Only a lookup scheme's identifier carries a pin: a DOI may contain an `@`.
        let (local, pin) = match local.split_once('@') {
            Some((l, p)) if form.looks_up() => (l, Some(p)),
            _ => (local, None),
        };
        let Some(caps) = d.pattern.captures(local) else {
            if let Some(p) = self.pinning_of(d, scheme, local) {
                return Err(match p {
                    Ok(p) => Refusal::Mutable {
                        pack: d.pack.clone(),
                        read: p.url,
                    },
                    Err(r) => r,
                });
            }
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
        // A pin read's value is the scheme's `pin` group.
        let pinned = d
            .scheme
            .pin
            .as_ref()
            .and_then(|_| caps.name("pin"))
            .map(|m| m.range());
        Ok(Split {
            d,
            form,
            scheme,
            local,
            pin,
            pinned,
            bindings,
        })
    }

    /// The read that pins `identifier`, when it is the mutable form a scheme's pin read reads
    /// (#1343). `None` for any other identifier, which [`Self::resolve`] answers as it is.
    pub fn pinning(&self, identifier: &str) -> Result<Option<Pinning>, Refusal> {
        match self.split(identifier) {
            Ok(_) => Ok(None),
            Err(Refusal::Mutable { .. }) => {
                let Some((scheme, local)) = identifier.split_once(':') else {
                    return Ok(None);
                };
                let Some(d) = self.schemes.get(scheme) else {
                    return Ok(None);
                };
                self.pinning_of(d, scheme, local).transpose()
            }
            Err(r) => Err(r),
        }
    }

    fn pinning_of(
        &self,
        d: &Declared,
        scheme: &str,
        local: &str,
    ) -> Option<Result<Pinning, Refusal>> {
        let (pin, mutable) = (d.scheme.pin.as_ref()?, d.mutable.as_ref()?);
        let p = Pinning::of(&d.pack, scheme, local, pin, mutable, &d.pattern)?;
        Some(
            p.map_err(|slots| Refusal::Unbound {
                pack: d.pack.clone(),
                slots,
            })
            .and_then(|p| {
                if p.url.starts_with("https://") || p.url.starts_with("http://") {
                    Ok(p)
                } else {
                    Err(Refusal::NotHttp {
                        pack: d.pack.clone(),
                        url: p.url,
                    })
                }
            }),
        )
    }

    /// Resolve `scheme:local-id` to the address a fetch follows.
    ///
    /// Offline for every form. A lookup scheme's identifier resolves through its pin alone:
    /// the page it was read off is not asked again.
    pub fn resolve(&self, identifier: &str) -> Result<Resolved, Refusal> {
        let sp = self.split(identifier)?;
        let d = sp.d;
        let url = match sp.form {
            Form::Template(template) => crate::cmd::catalog::location::bind(template, &sp.bindings)
                .map_err(|slots| Refusal::Unbound {
                    pack: d.pack.clone(),
                    slots,
                })?,
            _ => {
                let l = self.lookup_of(&sp)?;
                let Some(pin) = sp.pin else {
                    return Err(Refusal::Unpinned {
                        pack: d.pack.clone(),
                        page: l.url,
                    });
                };
                let refuse = |why: String| Refusal::Pin {
                    pack: d.pack.clone(),
                    pin: pin.to_string(),
                    why,
                };
                if let Some(why) = lookup::unsafe_pin(pin) {
                    return Err(refuse(why.to_string()));
                }
                if let Some(re) = &l.matches {
                    if !re.is_match(pin) {
                        return Err(refuse(format!("does not match `{re}`")));
                    }
                }
                l.base.join(pin)
            }
        };
        if !(url.starts_with("https://") || url.starts_with("http://")) {
            return Err(Refusal::NotHttp {
                pack: d.pack.clone(),
                url,
            });
        }
        Ok(Resolved {
            pack: d.pack.clone(),
            scheme: sp.scheme.to_string(),
            url,
            transport: d.transport.clone(),
        })
    }

    /// The page a lookup scheme reads for `identifier`, and how it picks; `None` for a scheme
    /// a template resolves. A pin the identifier carries is set aside: the page is read anew.
    pub fn lookup(&self, identifier: &str) -> Result<Option<Lookup>, Refusal> {
        let sp = self.split(identifier)?;
        if !sp.form.looks_up() {
            return Ok(None);
        }
        self.lookup_of(&sp).map(Some)
    }

    fn lookup_of(&self, sp: &Split<'_>) -> Result<Lookup, Refusal> {
        let d = sp.d;
        let unusable = |why: String| Refusal::Unusable {
            pack: d.pack.clone(),
            why,
        };
        let (page, url, matches, pick, media) = match sp.form {
            Form::Listing {
                listing,
                matches,
                pick,
            } => {
                let url = crate::cmd::catalog::location::bind(listing, &sp.bindings).map_err(
                    |slots| Refusal::Unbound {
                        pack: d.pack.clone(),
                        slots,
                    },
                )?;
                (Page::Listing, url, Some(matches), pick, None)
            }
            Form::Dcat {
                host,
                matches,
                pick,
                media,
            } => (
                Page::Dcat {
                    local: sp.local.to_string(),
                },
                format!("https://{host}/data.json"),
                matches,
                pick,
                media.map(str::to_string),
            ),
            Form::Template(_) => return Err(unusable("is a template, not a lookup".into())),
        };
        let Some(base) = Base::of(&url) else {
            return Err(Refusal::NotHttp {
                pack: d.pack.clone(),
                url,
            });
        };
        let matches = matches
            .map(|m| regex::Regex::new(&lookup::bind_match(m, &sp.bindings)))
            .transpose()
            .map_err(|e| unusable(format!("leaves its scheme's `match` uncompilable: {e}")))?;
        Ok(Lookup {
            pack: d.pack.clone(),
            scheme: sp.scheme.to_string(),
            identifier: format!("{}:{}", sp.scheme, sp.local),
            url,
            transport: d.transport.clone(),
            page,
            base,
            matches,
            pick,
            media,
        })
    }

    /// What two identifiers share when they name one source: `scheme:local-id`, less the pin a
    /// lookup scheme's carries, or the value a pin read wrote. A re-read listing pins a newer
    /// file of the same source, and a re-read branch a newer commit of the same file.
    pub fn source_of<'a>(&self, identifier: &'a str) -> Cow<'a, str> {
        match self.split(identifier) {
            // A scheme name has no `@`, so the first is the one `split` cut at.
            Ok(sp) if sp.pin.is_some() => {
                Cow::Borrowed(identifier.split_once('@').map_or(identifier, |(s, _)| s))
            }
            Ok(sp) => match sp.pinned {
                Some(range) => {
                    let at = sp.scheme.len() + 1;
                    Cow::Owned(format!(
                        "{}{}",
                        &identifier[..at + range.start],
                        &identifier[at + range.end..]
                    ))
                }
                None => Cow::Borrowed(identifier),
            },
            _ => Cow::Borrowed(identifier),
        }
    }

    /// Whether `identifier` is the mutable form a scheme's pin read reads.
    pub fn is_mutable(&self, identifier: &str) -> bool {
        matches!(self.split(identifier), Err(Refusal::Mutable { .. }))
    }
}

/// An identifier, split against the pack that declares its scheme.
struct Split<'a> {
    d: &'a Declared,
    form: Form<'a>,
    scheme: &'a str,
    local: &'a str,
    pin: Option<&'a str>,
    /// Where in `local` a pin read's value sits.
    pinned: Option<std::ops::Range<usize>>,
    bindings: Vec<(String, String)>,
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
    ("github", "archive"),
    ("ia", "archive"),
    ("ia-file", "archive"),
    ("pmc", "scholarly"),
    ("wayback", "archive"),
    ("wikipedia", "archive"),
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
            s.resolve.form().ok()?;
            Some((
                name.clone(),
                Declared {
                    pack: pack.clone(),
                    scheme: s.clone(),
                    transport: m.transport.clone(),
                    pattern,
                    mutable: s
                        .pin
                        .as_ref()
                        .and_then(|p| regex::Regex::new(&p.mutable).ok()),
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

    const GITHUB: &str = r#"
[pack]
name = "archive"
version = "0.2.0"

[scheme.github]
pattern = '^(?P<owner>[A-Za-z0-9-]+)/(?P<repo>[A-Za-z0-9._-]+)@(?P<pin>[0-9a-f]{40})/(?P<path>[^\s?#]+)$'
type = "document"
resolve = { template = "https://raw.githubusercontent.com/{owner}/{repo}/{pin}/{path}" }

[scheme.github.pin]
mutable = '^(?P<owner>[A-Za-z0-9-]+)/(?P<repo>[A-Za-z0-9._-]+)@(?P<ref>[^\s/@?#]+)/(?P<path>[^\s?#]+)$'
read = "https://api.github.com/repos/{owner}/{repo}/commits/{ref}"
media = "application/json"
value = "sha"
pinned = "{owner}/{repo}@{pin}/{path}"
"#;

    const SHA: &str = "051478957371ee0084a7c0913941d2a8c4757bb9";

    /// A branch names whatever it points at today, so a fetch refuses it and says how to pin
    /// it. A commit is what the pattern admits, and resolves offline.
    #[test]
    fn a_mutable_identifier_is_refused_with_its_pin_read_and_a_pinned_one_resolves() {
        let e = Enabled::from_packs(&[pack("archive", Origin::Vendored, GITHUB)]);
        let branch = "github:rust-lang/rust@main/README.md";
        let refusal = e.resolve(branch).unwrap_err();
        assert_eq!(
            refusal,
            Refusal::Mutable {
                pack: "archive@0.2.0".into(),
                read: "https://api.github.com/repos/rust-lang/rust/commits/main".into(),
            }
        );
        assert!(!refusal.is_benign());
        assert!(
            refusal
                .message(branch)
                .contains(&format!("yidam source add --dry-run {branch}")),
            "{}",
            refusal.message(branch)
        );
        assert_eq!(e.admits(branch), Ok(()), "well formed, only not fixed");
        assert!(e.is_mutable(branch));

        let p = e.pinning(branch).unwrap().unwrap();
        let answer = format!(r#"{{"sha": "{SHA}"}}"#);
        let (_, pinned) = p.pin(answer.as_bytes()).unwrap();
        assert_eq!(pinned, format!("github:rust-lang/rust@{SHA}/README.md"));
        assert!(e.pinning(&pinned).unwrap().is_none());
        assert_eq!(
            e.resolve(&pinned).unwrap().url,
            format!("https://raw.githubusercontent.com/rust-lang/rust/{SHA}/README.md")
        );
        assert!(matches!(
            e.resolve("github:rust-lang/rust/README.md"),
            Err(Refusal::Malformed { .. })
        ));
    }

    /// Two commits of one file are one source, so `source add` finds the entry that pinned the
    /// older one.
    #[test]
    fn a_pinned_identifier_is_one_source_whatever_its_pin() {
        let e = Enabled::from_packs(&[pack("archive", Origin::Vendored, GITHUB)]);
        let at = |sha: &str| format!("github:o/r@{sha}/a/b.csv");
        let other = "0".repeat(40);
        assert_eq!(e.source_of(&at(SHA)), e.source_of(&at(&other)));
        assert_eq!(e.source_of(&at(SHA)), "github:o/r@/a/b.csv");
        assert_ne!(
            e.source_of(&at(SHA)),
            e.source_of(&format!("github:o/r@{SHA}/a/c.csv"))
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

    const CLIMDIV: &str = r#"
[pack]
name = "noaa"
version = "0.1.0"

[scheme.climdiv]
pattern = '^[a-z]+$'
type = "dataset"
resolve = { listing = "https://www.ncei.noaa.gov/pub/data/cirs/climdiv/", match = '^climdiv-{id}-v1\.0\.0-\d{8}$', pick = "latest" }

[scheme.cms]
pattern = '^[0-9a-f-]{36}$'
type = "dataset"
resolve = { dcat = "data.cms.gov", media = "text/csv" }
"#;

    const LISTING: &str = r#"<html><body><a href="../">Parent</a>
<a href="climdiv-pcpndv-v1.0.0-20260901">climdiv-pcpndv-v1.0.0-20260901</a>
<a href="climdiv-pcpndv-v1.0.0-20261001">climdiv-pcpndv-v1.0.0-20261001</a>
<a href="climdiv-tmpcdv-v1.0.0-20261001">climdiv-tmpcdv-v1.0.0-20261001</a>
<a href="https://elsewhere.org/climdiv-pcpndv-v1.0.0-20991231">mirror</a>
</body></html>"#;

    fn noaa() -> Enabled {
        Enabled::from_packs(&[pack("noaa", Origin::Authored, CLIMDIV)])
    }

    #[test]
    fn a_listing_picks_the_latest_match_on_its_own_host() {
        let e = noaa();
        let l = e.lookup("climdiv:pcpndv").unwrap().unwrap();
        assert_eq!(l.url, "https://www.ncei.noaa.gov/pub/data/cirs/climdiv/");
        let pin = l.pick(LISTING.as_bytes()).unwrap();
        assert_eq!(pin, "climdiv-pcpndv-v1.0.0-20261001");
        let pinned = l.pinned(&pin);
        assert_eq!(pinned, "climdiv:pcpndv@climdiv-pcpndv-v1.0.0-20261001");
        assert_eq!(
            e.resolve(&pinned).unwrap().url,
            "https://www.ncei.noaa.gov/pub/data/cirs/climdiv/climdiv-pcpndv-v1.0.0-20261001"
        );
        assert_eq!(e.source_of(&pinned), "climdiv:pcpndv");
        assert!(e.lookup("doi:10.1/a").is_err());
    }

    /// A fetch reads what the entry pins. Reading the listing again would make the entry
    /// whichever file was released last.
    #[test]
    fn an_unpinned_lookup_identifier_is_refused_and_says_how_to_pin_it() {
        let err = noaa().resolve("climdiv:pcpndv").unwrap_err();
        assert!(matches!(err, Refusal::Unpinned { .. }), "{err:?}");
        assert!(!err.is_benign());
        let msg = err.message("climdiv:pcpndv");
        assert!(msg.contains("source add --dry-run climdiv:pcpndv"), "{msg}");
    }

    /// `catalog-location-malformed` reads the local id before the pin, as `resolve` does.
    #[test]
    fn admits_reads_a_lookup_identifier_without_its_pin() {
        let e = noaa();
        assert_eq!(
            e.admits("climdiv:pcpndv@climdiv-pcpndv-v1.0.0-20261001"),
            Ok(())
        );
        assert_eq!(e.admits("climdiv:pcpndv"), Ok(()));
        assert!(matches!(
            e.admits("climdiv:PCP@climdiv-pcpndv-v1.0.0-20261001"),
            Err(Refusal::Malformed { local, .. }) if local == "PCP"
        ));
    }

    #[test]
    fn a_pin_the_match_refuses_or_that_leaves_the_host_is_refused() {
        let e = noaa();
        for id in [
            "climdiv:pcpndv@climdiv-tmpcdv-v1.0.0-20261001",
            "climdiv:pcpndv@../../secret",
            "climdiv:pcpndv@//evil.org/climdiv-pcpndv-v1.0.0-20261001",
            "cms:00000000-0000-0000-0000-000000000000@https://evil.org/a.csv",
            "cms:00000000-0000-0000-0000-000000000000@/a.zip#packinglist.txt",
        ] {
            let err = e.resolve(id).unwrap_err();
            assert!(matches!(err, Refusal::Pin { .. }), "{id}: {err:?}");
        }
    }

    #[test]
    fn a_dcat_catalog_picks_the_distribution_of_the_named_dataset() {
        let id = "cms:6a3aa708-3c9f-4c1a-8b8d-9d0c1d1e6b1a";
        let catalog = format!(
            r#"{{"dataset": [
              {{"identifier": "https://data.cms.gov/data-api/v1/dataset/{}/data-viewer",
                "distribution": [
                  {{"mediaType": "text/csv", "downloadURL": "https://data.cms.gov/sites/default/files/2026-09-30/abc/data.csv"}},
                  {{"mediaType": "application/json", "downloadURL": "https://data.cms.gov/data-api/v1/dataset/x/data"}},
                  {{"mediaType": "text/csv", "downloadURL": "https://mirror.example/data.csv"}}
                ]}},
              {{"identifier": "other", "distribution": []}}
            ]}}"#,
            &id[4..]
        );
        let e = noaa();
        let l = e.lookup(id).unwrap().unwrap();
        assert_eq!(l.url, "https://data.cms.gov/data.json");
        let pin = l.pick(catalog.as_bytes()).unwrap();
        assert_eq!(pin, "sites/default/files/2026-09-30/abc/data.csv");
        assert_eq!(
            e.resolve(&l.pinned(&pin)).unwrap().url,
            "https://data.cms.gov/sites/default/files/2026-09-30/abc/data.csv"
        );
    }

    #[test]
    fn several_matches_under_only_are_refused_by_name() {
        let only = CLIMDIV.replace(", pick = \"latest\"", "");
        let e = Enabled::from_packs(&[pack("noaa", Origin::Authored, &only)]);
        let l = e.lookup("climdiv:pcpndv").unwrap().unwrap();
        let err = l.pick(LISTING.as_bytes()).unwrap_err();
        let msg = err.message(&l);
        assert!(
            msg.contains("20260901") && msg.contains("20261001"),
            "{msg}"
        );
        let none = l.pick(b"<a href=\"x\">x</a>").unwrap_err();
        assert!(
            none.message(&l).contains("none matches"),
            "{}",
            none.message(&l)
        );
    }

    /// An `@` is a DOI's to carry: only a lookup scheme reads one as a pin.
    #[test]
    fn a_template_scheme_keeps_its_at_signs() {
        let e = Enabled::from_packs(&[pack("scholarly", Origin::Authored, SCHOLARLY)]);
        let r = e.resolve("doi:10.1167/a@b").unwrap();
        assert_eq!(r.url, "https://api.crossref.org/works/10.1167/a@b");
        assert_eq!(e.source_of("doi:10.1167/a@b"), "doi:10.1167/a@b");
        assert!(e.lookup("doi:10.1167/a").unwrap().is_none());
    }

    #[test]
    fn a_pack_that_does_not_parse_resolves_nothing() {
        let e = Enabled::from_packs(&[pack("broken", Origin::Authored, "[pack]\nname = 1")]);
        assert!(!e.declares("doi"));
    }
}
