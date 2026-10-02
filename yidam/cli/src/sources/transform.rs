//! A pack's `describe` and `extract`, as the rest of the CLI calls them (RFC-0048 §6, #1318).
//!
//! This module is compiled in every build. The engine is not: the transforms run in
//! [`crate::gluon_arm`]'s closed prelude, which is behind `source-transforms`. A build without
//! it still finds a scheme's transform, names it and hashes it. What it cannot do is run it,
//! and it says so in the result rather than in an error:
//!
//! - [`describe`] returns a draft filled from the pack's templates alone, with the fields it
//!   could not fill and why, so `source add` (#1316) writes an entry a person completes
//!   rather than none at all.
//! - [`extract`] returns the reason it took no reading, which `catalog-extract` reports as a
//!   skip beside the readings it did take.
//!
//! # What a reading records about the transform that took it
//!
//! `by: <pack>@<version>/<transform>@sha256:<hash>` — [`Transform::by`]. The version says which
//! pack, and the hash says which bytes of the script, because a pack's version is a promise its
//! author makes and the hash is the one this binary can check. A reading whose `by` names a
//! script that has since changed is a reading of an older transform, and a reader can tell.

use std::path::Path;

use anyhow::{Context, Result};

use super::manifest::{Manifest, Scheme};
use super::{Origin, Pack};
use crate::vault::ContentHash;

/// Whether this build runs transforms.
pub const AVAILABLE: bool = cfg!(feature = "source-transforms");

/// Why a transform did not run in this build, as every caller says it.
pub const UNAVAILABLE: &str =
    "this build was compiled without `source-transforms`, so a pack's transforms do not run";

/// The draft fields a describe transform fills, in the order a person reads them.
pub const DRAFT_FIELDS: &[&str] = &["name", "type", "date", "description"];

/// One transform script, read from its pack.
#[derive(Debug, Clone)]
pub struct Transform {
    pub pack: String,
    pub version: String,
    /// `transforms/<name>.glu`, relative to the pack.
    pub path: String,
    pub script: String,
    pub sha256: String,
}

impl Transform {
    /// What a reading or a draft records about the script that produced it.
    pub fn by(&self) -> String {
        format!(
            "{}@{}/{}@sha256:{}",
            self.pack, self.version, self.path, self.sha256
        )
    }

    /// The name a refusal calls it by: the file a pack author opens.
    pub fn name(&self) -> String {
        format!("{}/{}", self.pack, self.path)
    }

    /// Read `path` out of `pack`.
    pub fn read(root: &Path, pack: &Pack, manifest: &Manifest, path: &str) -> Result<Self> {
        let file = root.join(&pack.dir).join(path);
        let bytes = std::fs::read(&file).with_context(|| format!("reading {}", file.display()))?;
        let script =
            String::from_utf8(bytes).with_context(|| format!("{} is not UTF-8", file.display()))?;
        Ok(Self {
            pack: pack.name.clone(),
            version: manifest.pack.version.clone(),
            path: path.to_string(),
            sha256: ContentHash::of_bytes(script.as_bytes())
                .as_str()
                .to_string(),
            script,
        })
    }
}

/// The packs a lookup may answer from: every authored pack, and every vendored pack no authored
/// pack shadows — the set `source check` calls enabled. A pack whose manifest does not parse
/// declares nothing.
pub fn enabled(packs: &[Pack]) -> Vec<(&Pack, &Manifest)> {
    let authored: Vec<&str> = packs
        .iter()
        .filter(|p| p.origin == Origin::Authored)
        .map(|p| p.name.as_str())
        .collect();
    packs
        .iter()
        .filter(|p| p.origin == Origin::Authored || !authored.contains(&p.name.as_str()))
        .filter_map(|p| Some((p, p.manifest.as_ref().ok()?)))
        .collect()
}

/// The enabled pack that declares `scheme`, or why there is none to ask.
///
/// Two packs declaring one scheme is a `source check` error, and here it is a refusal rather
/// than a choice: picking one would resolve an identifier by whichever pack sorted first.
pub fn scheme<'a>(
    packs: &'a [Pack],
    scheme: &str,
) -> std::result::Result<(&'a Pack, &'a Manifest, &'a Scheme), String> {
    let owners: Vec<_> = enabled(packs)
        .into_iter()
        .filter_map(|(p, m)| Some((p, m, m.scheme.get(scheme)?)))
        .collect();
    match owners.as_slice() {
        [one] => Ok(*one),
        [] => Err(format!("no enabled source pack declares scheme `{scheme}`")),
        many => Err(format!(
            "scheme `{scheme}` is declared by {}; `yidam source check` says which to remove",
            many.iter()
                .map(|(p, _, _)| p.name.as_str())
                .collect::<Vec<_>>()
                .join(" and ")
        )),
    }
}

/// What `describe` made of a response, and what it could not.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize)]
pub struct Draft {
    pub name: Option<String>,
    /// The catalog `type:`. The scheme's own when the transform names none.
    pub kind: Option<String>,
    pub date: Option<String>,
    pub description: Option<String>,
    /// Further identifiers the response names, by the key a `then.from` reads.
    pub identifiers: Vec<(String, String)>,
    /// [`DRAFT_FIELDS`] this draft leaves empty, in that order.
    pub unfilled: Vec<&'static str>,
    /// Why no transform filled them, when none ran.
    pub why: Option<String>,
    /// The transform that filled it, as [`Transform::by`] spells it.
    pub by: Option<String>,
}

impl Draft {
    /// The draft the pack's templates alone give: the scheme's type, and nothing a response says.
    pub fn from_templates(scheme: &Scheme, why: impl Into<String>) -> Self {
        Self {
            kind: Some(scheme.kind.clone()),
            why: Some(why.into()),
            ..Self::default()
        }
        .settled()
    }

    /// The identifier a `then.from` of `describe.<key>` reads.
    pub fn identifier(&self, key: &str) -> Option<&str> {
        self.identifiers
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    fn settled(mut self) -> Self {
        self.unfilled = DRAFT_FIELDS
            .iter()
            .zip([&self.name, &self.kind, &self.date, &self.description])
            .filter(|(_, v)| v.as_deref().is_none_or(str::is_empty))
            .map(|(f, _)| *f)
            .collect();
        self
    }
}

/// Fill a draft entry for `scheme` from a fetched `response` in `media_type`.
///
/// `Err` is a failure to read the pack. A transform that is refused, fails, or cannot run in
/// this build is a draft from the templates whose `why` says so — a refused describe is a
/// pack's problem, and the entry it would have described is still a source somebody is adding.
pub fn describe(
    root: &Path,
    pack: &Pack,
    manifest: &Manifest,
    scheme: &Scheme,
    media_type: &str,
    response: &[u8],
) -> Result<Draft> {
    let Some(path) = &scheme.describe else {
        return Ok(Draft::from_templates(
            scheme,
            "the scheme declares no describe transform",
        ));
    };
    let t = Transform::read(root, pack, manifest, path)?;
    Ok(match run_describe(&t, media_type, response) {
        Ok(mut draft) => {
            if draft.kind.as_deref().is_none_or(str::is_empty) {
                draft.kind = Some(scheme.kind.clone());
            }
            draft.by = Some(t.by());
            draft.settled()
        }
        Err(why) => Draft::from_templates(scheme, format!("{}: {why}", t.name())),
    })
}

/// A reading a pack's `extract` took.
#[derive(Debug, Clone, PartialEq)]
pub struct Taken {
    pub media_type: String,
    pub text: String,
    pub by: String,
}

/// Run `t` as an extract transform over `bytes` in `media_type`, or say why it took no reading.
pub fn extract(
    t: &Transform,
    media_type: &str,
    bytes: &[u8],
) -> std::result::Result<Taken, String> {
    let (media_type, text) = run_extract(t, media_type, bytes)?;
    if media_type.trim().is_empty() {
        return Err(format!(
            "{} returned a reading with no media type",
            t.name()
        ));
    }
    if text.trim().is_empty() {
        return Err(format!("{} returned an empty reading", t.name()));
    }
    Ok(Taken {
        media_type,
        text,
        by: t.by(),
    })
}

/// Which of a pack's two entry points a script is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Describe,
    Extract,
}

/// Whether `t` is a transform of `kind` the closed prelude admits, decided without running it:
/// the same admission [`describe`] and [`extract`] open with. `Ok` in a build without
/// `source-transforms`, which cannot say — [`AVAILABLE`] is how a caller tells.
pub fn admit(t: &Transform, kind: Kind) -> std::result::Result<(), String> {
    admit_in(t, kind)
}

#[cfg(feature = "source-transforms")]
fn admit_in(t: &Transform, kind: Kind) -> std::result::Result<(), String> {
    use crate::gluon_arm::transform;
    let admitted = match kind {
        Kind::Describe => transform::admit_describe(&t.name(), &t.script),
        Kind::Extract => transform::admit_extract(&t.name(), &t.script),
    };
    admitted.map(drop).map_err(|e| e.to_string())
}

#[cfg(not(feature = "source-transforms"))]
fn admit_in(_: &Transform, _: Kind) -> std::result::Result<(), String> {
    Ok(())
}

#[cfg(feature = "source-transforms")]
fn run_describe(
    t: &Transform,
    media_type: &str,
    bytes: &[u8],
) -> std::result::Result<Draft, String> {
    use crate::gluon_arm::{budget::DEFAULT_CALLS, transform};
    let parsed =
        super::parsed::parse(media_type, bytes).map_err(|e| format!("the response {e}"))?;
    let d = transform::describe(&t.name(), &t.script, parsed, DEFAULT_CALLS)
        .map_err(|e| format!("{e:#}"))?;
    Ok(Draft {
        name: d.name,
        kind: d.kind,
        date: d.date,
        description: d.description,
        identifiers: d
            .identifiers
            .into_iter()
            .map(|n| (n.key, n.value))
            .collect(),
        ..Draft::default()
    })
}

#[cfg(not(feature = "source-transforms"))]
fn run_describe(_: &Transform, _: &str, _: &[u8]) -> std::result::Result<Draft, String> {
    Err(UNAVAILABLE.to_string())
}

#[cfg(feature = "source-transforms")]
fn run_extract(
    t: &Transform,
    media_type: &str,
    bytes: &[u8],
) -> std::result::Result<(String, String), String> {
    use crate::gluon_arm::{budget::DEFAULT_CALLS, transform};
    let parsed =
        super::parsed::parse(media_type, bytes).map_err(|e| format!("the artifact {e}"))?;
    let r = transform::extract(&t.name(), &t.script, parsed, DEFAULT_CALLS)
        .map_err(|e| format!("{e:#}"))?;
    Ok((r.media_type, r.text))
}

#[cfg(not(feature = "source-transforms"))]
fn run_extract(_: &Transform, _: &str, _: &[u8]) -> std::result::Result<(String, String), String> {
    Err(UNAVAILABLE.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scheme(kind: &str) -> Scheme {
        toml::from_str(&format!(
            "pattern = '^x$'\ntype = \"{kind}\"\nresolve = {{ template = \"https://e.org/{{id}}\" }}\n"
        ))
        .unwrap()
    }

    #[test]
    fn a_draft_from_templates_names_every_field_it_could_not_fill() {
        let d = Draft::from_templates(&scheme("paper"), UNAVAILABLE);
        assert_eq!(d.kind.as_deref(), Some("paper"));
        assert_eq!(d.unfilled, vec!["name", "date", "description"]);
        assert_eq!(d.why.as_deref(), Some(UNAVAILABLE));
        assert_eq!(d.by, None);
    }

    #[test]
    fn by_names_the_pack_its_version_the_script_and_its_bytes() {
        let t = Transform {
            pack: "scholarly".into(),
            version: "0.1.0".into(),
            path: "transforms/crossref.glu".into(),
            script: String::new(),
            sha256: "ab".repeat(32),
        };
        assert_eq!(
            t.by(),
            format!(
                "scholarly@0.1.0/transforms/crossref.glu@sha256:{}",
                "ab".repeat(32)
            )
        );
    }
}
