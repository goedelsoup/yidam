//! An identifier, resolved through its pack to the draft entry `source add` writes (#1316).
//!
//! Resolving is three steps, and only the second can touch a network:
//!
//! 1. **Match.** [`super::resolve::Enabled`] binds the identifier to an address, exactly as
//!    `catalog fetch` does for a `kind: identifier` location: one pack declares the scheme, the
//!    local id matches its pattern, the template binds, and the publisher is not blocked. An
//!    identifier it refuses is refused here in its words. A scheme that reads its address off
//!    a listing or a catalog (#1342) has the page asked here, and the identifier is written
//!    with the name it picked as a pin, `scheme:local-id@<pin>`, which binds offline after.
//! 2. **Describe.** Where the scheme declares a `describe` and the build runs transforms, the
//!    address is asked and the answer handed to the transform. How it is asked is the caller's
//!    [`Asker`]: the network, or the pack's own recorded fixtures.
//! 3. **Chain.** Each `then` whose `describe.<key>` the answer named becomes a further
//!    identifier in the same pack, and is resolved the same way. A chain is followed until it
//!    names nothing new, so a cycle ends where it repeats.
//!
//! A describe that cannot run is not a failure to resolve. Its draft is filled from the pack's
//! templates and says why, and the entry is still a source somebody is adding. What *is* a
//! failure is an identifier no enabled pack can resolve, because the entry would then carry a
//! location `catalog-fetch` can never follow.
//!
//! Nothing here writes. The command writes the entry; the MCP `resolve_source` (#1319) returns
//! the same [`Resolved`] without writing it, and `search_sources` runs [`describe_only`] on
//! each candidate.

use std::collections::BTreeSet;
use std::path::Path;

use serde::Serialize;

use super::manifest::{Contact, Manifest, Scheme};
use super::resolve::{Enabled, Refusal};
use super::transform::{self, Draft};
use super::Pack;
use crate::cmd::catalog::transport::CONTACT_VAR as CONTACT;

/// The body a draft starts from when its pack's `entry.md` is blank.
///
/// The headings corpora converged on (RFC-0048 §4). Each is a prompt, in a comment so it does
/// not render, and none is answered: an entry nobody has read says nothing about what it holds.
pub const ENTRY_TEMPLATE: &str = "\
## What it is

<!-- Who published it, in what form, and when. -->

## What was read

<!-- Which parts, of which version. Nothing is read until someone reads it. -->

## What it establishes

<!-- The claims it is the authority on, in the publisher's own terms. -->

## What it does not establish

<!-- What a reader might take it to say, and it does not. -->

## What else it holds, unread

<!-- Sections, tables or appendices nobody has read yet. -->

## Defects

<!-- Errata, retractions, known errors, gaps in coverage. -->

## Access constraints

<!-- Licence, paywall, rate limits, anything that stops a re-fetch. -->

## Currency

<!-- How often it changes, and what would make this entry stale. -->
";

/// What a pack's transport asks of the environment, and whether the environment has it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Needs {
    /// `required` or `optional`.
    pub contact: &'static str,
    /// Whether `YIDAM_CONTACT` is set and not blank.
    pub contact_set: bool,
    /// Each `[transport] auth` variable, and whether it is set. Never its value.
    pub auth: Vec<AuthVar>,
    /// Why the publisher cannot be fetched from at all, as the pack says.
    pub blocked: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AuthVar {
    pub var: String,
    pub set: bool,
}

impl Needs {
    /// Read the environment through `env`, so a test can hand it one.
    pub fn of(m: &Manifest, env: &dyn Fn(&str) -> Option<String>) -> Self {
        let set = |k: &str| env(k).is_some_and(|v| !v.trim().is_empty());
        Self {
            contact: match m.transport.contact {
                Contact::Required => "required",
                Contact::Optional => "optional",
            },
            contact_set: set(CONTACT),
            auth: m
                .transport
                .auth
                .iter()
                .map(|a| AuthVar {
                    var: a.env().to_string(),
                    set: set(a.env()),
                })
                .collect(),
            blocked: m.transport.blocked.clone(),
        }
    }

    /// Whether every need is met.
    pub fn satisfied(&self) -> bool {
        self.unmet().is_none()
    }

    /// Why a request to this pack's publisher would not be made, if it would not.
    pub fn unmet(&self) -> Option<String> {
        if let Some(why) = &self.blocked {
            return Some(format!("the pack says its publisher is blocked: {why}"));
        }
        if self.contact == "required" && !self.contact_set {
            return Some(format!(
                "the pack requires a contact, and `{CONTACT}` is not set; set it to an address \
                 the publisher can reach you at"
            ));
        }
        let missing: Vec<&str> = self
            .auth
            .iter()
            .filter(|a| !a.set)
            .map(|a| a.var.as_str())
            .collect();
        if !missing.is_empty() {
            return Some(format!(
                "the pack authenticates with {}, which {} not set",
                missing
                    .iter()
                    .map(|v| format!("`{v}`"))
                    .collect::<Vec<_>>()
                    .join(", "),
                if missing.len() == 1 { "is" } else { "are" }
            ));
        }
        None
    }
}

/// One identifier this resolution reached.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Located {
    /// `scheme:local-id`, as the entry's `kind: identifier` location records it.
    pub identifier: String,
    /// The address the scheme's template binds it to.
    pub url: String,
    /// How it was reached: `None` for the identifier asked for, otherwise the identifier
    /// whose describe named it and the key it was named under.
    pub via: Option<String>,
    /// The page the pin was read off, for a scheme that resolves through one.
    #[serde(skip)]
    pub read_off: Option<String>,
}

impl Located {
    /// The location's `description:`. Every location says how it got there, because the lint
    /// asks that an entry's several locations be told apart and the reason is the difference.
    pub fn description(&self) -> String {
        let how = match &self.via {
            None => "the identifier this entry was added by".to_string(),
            Some(via) => format!("named by {via}"),
        };
        match &self.read_off {
            None => how,
            Some(page) => format!("{how}, pinned to the file {page} listed"),
        }
    }
}

/// Where the answers a describe ran over came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Answered {
    /// The publisher, over the network.
    Network,
    /// The pack's recorded `[fixtures]`. Nothing was asked.
    Fixture,
}

/// An identifier, resolved.
#[derive(Debug, Clone, Serialize)]
pub struct Resolved {
    /// The identifier asked for, normalised: surrounding space trimmed.
    pub identifier: String,
    /// The pack that resolved it.
    pub pack: String,
    /// The identifier first, then each one `then` reached, in the order reached.
    pub locations: Vec<Located>,
    /// What the identifier's describe made of its answer, or the template draft and why.
    pub draft: Draft,
    /// `[defaults] ttl_days`, which the entry is written with.
    pub ttl_days: Option<u32>,
    /// Where each answer came from: a describe's, or a page read for a pin. `None` when
    /// nothing was asked.
    pub answered: Option<Answered>,
    /// Why a `then` was not followed, one line each.
    pub unfollowed: Vec<String>,
    /// The pack's `entry.md`, or [`ENTRY_TEMPLATE`] when it is blank.
    #[serde(skip)]
    pub body: String,
}

/// How a describe's answer is obtained.
pub trait Asker {
    /// Which of [`Answered`] this asker gives.
    fn answered(&self) -> Answered;
    /// The bytes `url` answers with for `identifier` in `pack`, or why there are none.
    fn ask(
        &mut self,
        pack: &Pack,
        manifest: &Manifest,
        identifier: &str,
        url: &str,
    ) -> Result<Vec<u8>, String>;
}

/// Answers from the pack's `[fixtures]`, and asks nothing.
pub struct Fixtures<'a> {
    pub root: &'a Path,
}

impl Asker for Fixtures<'_> {
    fn answered(&self) -> Answered {
        Answered::Fixture
    }
    fn ask(
        &mut self,
        pack: &Pack,
        m: &Manifest,
        identifier: &str,
        _: &str,
    ) -> Result<Vec<u8>, String> {
        let Some(file) = m.fixtures.get(identifier) else {
            return Err(format!(
                "offline, and {} records no fixture for `{identifier}`",
                pack.name
            ));
        };
        let path = self.root.join(&pack.dir).join("fixtures").join(file);
        std::fs::read(&path).map_err(|e| format!("reading {}: {e}", path.display()))
    }
}

/// Resolve `identifier` through the enabled packs in `packs`.
///
/// `Err` when [`Enabled::resolve`] refuses it — a scheme nothing declares, a local id the
/// pattern refuses, a template that does not bind, a blocked publisher. Every other shortfall
/// is in the [`Resolved`].
pub fn resolve(
    root: &Path,
    packs: &[Pack],
    identifier: &str,
    asker: &mut dyn Asker,
) -> Result<Resolved, String> {
    let asked = identifier.trim();
    let enabled = Enabled::from_packs(packs);
    let first = locate(packs, &enabled, asked, None, asker)?;
    let (pack, manifest, _) = transform::scheme(packs, &first.scheme)?;
    let url = first.located.url.clone();
    let scheme_name = first.scheme;
    let identifier = first.located.identifier.as_str();

    let mut out = Resolved {
        identifier: asked.to_string(),
        pack: pack.name.clone(),
        locations: vec![first.located.clone()],
        draft: Draft::default(),
        ttl_days: manifest.defaults.ttl_days,
        answered: first.answered,
        unfollowed: Vec::new(),
        body: body(root, pack),
    };

    // Breadth first, so the locations read in the order a person would follow them.
    let mut seen: BTreeSet<String> = BTreeSet::from([asked.to_string(), identifier.to_string()]);
    let mut queue: Vec<(String, String, String)> = vec![(scheme_name, identifier.to_string(), url)];
    let mut first = true;
    while !queue.is_empty() {
        let mut next = Vec::new();
        for (name, id, url) in queue {
            let s = &manifest.scheme[&name];
            let draft = describe(root, pack, manifest, s, &id, &url, asker, &mut out.answered);
            for then in &s.then {
                let Some(key) = then.from.strip_prefix("describe.") else {
                    continue;
                };
                let Some(value) = draft.identifier(key) else {
                    if draft.by.is_some() {
                        // The describe ran and named nothing under the key: a fact about this
                        // source (a paper with no PMCID), not a shortfall to report.
                        continue;
                    }
                    out.unfollowed.push(format!(
                        "`{id}` then `{}` from `{}`: {}",
                        then.scheme,
                        then.from,
                        draft.why.as_deref().unwrap_or("its describe did not run")
                    ));
                    continue;
                };
                let chained = format!("{}:{}", then.scheme, value.trim());
                if !seen.insert(chained.clone()) {
                    continue;
                }
                if !manifest.scheme.contains_key(&then.scheme) {
                    continue;
                }
                let via = format!("{id}'s {}", then.from);
                match locate(packs, &enabled, &chained, Some(via), asker) {
                    Ok(l) => {
                        out.answered = out.answered.or(l.answered);
                        let (chained, u) = (l.located.identifier.clone(), l.located.url.clone());
                        out.locations.push(l.located);
                        next.push((then.scheme.clone(), chained, u));
                    }
                    Err(why) => out
                        .unfollowed
                        .push(format!("`{id}` names `{chained}`, and {why}")),
                }
            }
            if first {
                out.draft = draft;
                first = false;
            }
        }
        queue = next;
    }
    Ok(out)
}

/// What `identifier`'s own describe makes of it, with no `then` followed.
///
/// The summary the MCP `search_sources` gives each candidate (#1319). A search answers with
/// several identifiers, and following each one's chain would ask the publisher for sources
/// nobody has chosen yet. The [`Answered`] is `None` when nothing was asked.
pub fn describe_only(
    root: &Path,
    packs: &[Pack],
    identifier: &str,
    asker: &mut dyn Asker,
) -> Result<(Draft, Option<Answered>), String> {
    let identifier = identifier.trim();
    let asked = Enabled::from_packs(packs)
        .resolve(identifier)
        .map_err(|refusal| refusal.message(identifier))?;
    // A lookup scheme declares no describe (`source check`), so an unpinned candidate is
    // refused above rather than having its page read for a summary nobody chose.
    let (pack, manifest, s) = transform::scheme(packs, &asked.scheme)?;
    let mut answered = None;
    let draft = describe(
        root,
        pack,
        manifest,
        s,
        identifier,
        &asked.url,
        asker,
        &mut answered,
    );
    Ok((draft, answered))
}

/// An identifier, located: its scheme, and where it resolves.
struct Locating {
    scheme: String,
    located: Located,
    /// Where the page read for its pin came from, when one was read.
    answered: Option<Answered>,
}

/// Resolve `identifier`, asking a lookup scheme's page for the pin it lacks.
///
/// A pinned identifier, or one a template binds, is resolved offline. An unpinned one in a
/// lookup scheme has its page asked through `asker`, and comes back pinned to what it picked.
fn locate(
    packs: &[Pack],
    enabled: &Enabled,
    identifier: &str,
    via: Option<String>,
    asker: &mut dyn Asker,
) -> Result<Locating, String> {
    let mut answered = None;
    let (identifier, read_off) = match enabled.resolve(identifier) {
        Err(Refusal::Unpinned { .. }) => {
            let lookup = match enabled.lookup(identifier) {
                Ok(Some(l)) => l,
                Ok(None) => return Err(format!("`{identifier}` names no page to read")),
                Err(refusal) => return Err(refusal.message(identifier)),
            };
            let (pack, manifest, _) = transform::scheme(packs, &lookup.scheme)?;
            let page = asker.ask(pack, manifest, &lookup.identifier, &lookup.url)?;
            answered = Some(asker.answered());
            let pin = lookup
                .pick(&page)
                .map_err(|u| format!("`{identifier}`: {}", u.message(&lookup)))?;
            (lookup.pinned(&pin), Some(lookup.url))
        }
        _ => (identifier.to_string(), None),
    };
    let r = enabled
        .resolve(&identifier)
        .map_err(|refusal| refusal.message(&identifier))?;
    Ok(Locating {
        scheme: r.scheme,
        located: Located {
            identifier,
            url: r.url,
            via,
            read_off,
        },
        answered,
    })
}

#[allow(clippy::too_many_arguments)]
fn describe(
    root: &Path,
    pack: &Pack,
    manifest: &Manifest,
    s: &Scheme,
    identifier: &str,
    url: &str,
    asker: &mut dyn Asker,
    answered: &mut Option<Answered>,
) -> Draft {
    if s.describe.is_none() {
        return Draft::from_templates(s, "the scheme declares no describe transform");
    }
    // Checked before asking, so a build that cannot run the answer does not ask for it.
    if !transform::AVAILABLE {
        return Draft::from_templates(s, transform::UNAVAILABLE);
    }
    let Some(media) = &s.resolve.media else {
        return Draft::from_templates(
            s,
            "the scheme's resolve declares no media, so its answer cannot be parsed",
        );
    };
    let bytes = match asker.ask(pack, manifest, identifier, url) {
        Ok(b) => b,
        Err(why) => return Draft::from_templates(s, why),
    };
    *answered = Some(asker.answered());
    match transform::describe(root, pack, manifest, s, media, &bytes) {
        Ok(d) => d,
        Err(e) => Draft::from_templates(s, format!("{e:#}")),
    }
}

/// The pack's `entry.md`, or the template when it is blank or unreadable.
fn body(root: &Path, pack: &Pack) -> String {
    std::fs::read_to_string(root.join(&pack.dir).join("entry.md"))
        .ok()
        .filter(|b| !b.trim().is_empty())
        .unwrap_or_else(|| ENTRY_TEMPLATE.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest(transport: &str) -> Manifest {
        toml::from_str(&format!(
            "[pack]\nname = \"p\"\nversion = \"0.1.0\"\n[transport]\n{transport}\n"
        ))
        .unwrap()
    }

    fn env(vars: &'static [(&'static str, &'static str)]) -> impl Fn(&str) -> Option<String> {
        move |k| {
            vars.iter()
                .find(|(n, _)| *n == k)
                .map(|(_, v)| v.to_string())
        }
    }

    #[test]
    fn a_required_contact_is_unmet_until_it_is_set() {
        let m = manifest("contact = \"required\"");
        let none = Needs::of(&m, &env(&[]));
        assert!(none.unmet().unwrap().contains(CONTACT));
        let blank = Needs::of(&m, &env(&[(CONTACT, "  ")]));
        assert!(!blank.satisfied(), "a blank contact reaches nobody");
        assert!(Needs::of(&m, &env(&[(CONTACT, "a@b.org")])).satisfied());
    }

    #[test]
    fn auth_names_each_missing_variable_and_never_a_value() {
        let m = manifest(
            "auth = [{ env = \"A_TOKEN\", query = \"a\" }, { env = \"B_TOKEN\", query = \"b\" }]",
        );
        let n = Needs::of(&m, &env(&[("A_TOKEN", "secret-value")]));
        let why = n.unmet().unwrap();
        assert!(
            why.contains("`B_TOKEN`") && !why.contains("A_TOKEN"),
            "{why}"
        );
        assert!(!format!("{n:?}").contains("secret-value"));
    }

    #[test]
    fn a_blocked_publisher_is_unmet_whatever_else_is_set() {
        let m = manifest("blocked = \"refuses automated clients\"");
        let why = Needs::of(&m, &env(&[(CONTACT, "a@b.org")]))
            .unmet()
            .unwrap();
        assert!(why.contains("refuses automated clients"), "{why}");
    }
}
