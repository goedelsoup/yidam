//! `yidam catalog-extract` — take the readings of each artifact the catalog records (#1172, #1318).
//!
//! A quotation of a PDF is compared with a reading of it, never with the PDF, and the reading
//! is taken here, once, rather than by lint on every run: [`crate::reading`] says why. This
//! command reads an artifact's bytes from the vault cache, takes a reading, files it in the
//! cache under its own digest, and records that digest, its media type and what took it under
//! the artifact's record in `readings:`. Then it commits `extract:`, an operational verb: the
//! pipeline advanced, and no understanding changed.
//!
//! # What it takes a reading of
//!
//! Two kinds of record, and a record can be both:
//!
//! - one whose `media_type` is `application/pdf` and which has no text reading yet — under
//!   `readings:`, or under the `text:` key entries wrote before #1318. The reading is
//!   [`crate::reading::EXTRACTOR`]'s.
//! - one fetched `from:` an `identifier` location whose scheme's source pack declares an
//!   `extract` transform (RFC-0048 §6), and which has no reading by that transform's exact
//!   bytes yet. The reading is the transform's, and its `by` names the pack, its version, the
//!   script and the script's digest. A build without `source-transforms` reports these as
//!   skipped, with that reason, rather than as nothing to do.
//!
//! A reading already taken is never replaced — see [`super::record::add_reading`] — so a
//! re-run over a read catalog changes nothing and commits nothing. A record that states no
//! media type is passed over by the PDF extractor: which bytes are a PDF is the record's to
//! say, not a guess made from the first bytes of a file.
//!
//! # What it refuses
//!
//! Bytes this machine does not hold, bytes that no longer hash to their name, an artifact the
//! extractor or transform cannot read, and one it reads as no text at all. Each is reported,
//! the rest of the catalog is still read, and nothing is recorded for it.

use std::path::Path;

use anyhow::{Context, Result};

use super::fetch::{select, staging};
use super::record;
use crate::cmd::operational::{Commit, Writer};
use crate::parse::{
    parse_frontmatter, ArtifactOrigin, ArtifactReading, CatalogArtifact, CatalogLocation,
    TEXT_READING,
};
use crate::paths::{repo_root, yidam_catalog_dir};
use crate::reading::{self, EXTRACTOR};
use crate::sources::{self, transform::Transform};
use crate::vault::{Cache, ContentHash};

pub struct ExtractOptions {
    /// Restrict to one entry, by file stem or `name:`. Absent means every entry.
    pub entry: Option<String>,
    /// Report what would be read; read nothing, write nothing, commit nothing.
    pub dry_run: bool,
    pub format: crate::report::Format,
}

#[derive(Debug, Clone, serde::Serialize)]
pub(crate) struct Read {
    /// The artifact's digest.
    artifact: String,
    /// The reading's digest. Absent on a dry run, which reads nothing.
    sha256: Option<String>,
    /// What took the reading, as its record's `by:` says it.
    extractor: String,
    bytes: Option<u64>,
    /// The reading's media type. Recorded, and not reported: the report's shape predates it.
    #[serde(skip)]
    media_type: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub(crate) struct Skipped {
    artifact: String,
    why: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub(crate) struct EntryOutcome {
    pub(crate) entry: String,
    read: Vec<Read>,
    skipped: Vec<Skipped>,
    /// Absent when nothing was recorded.
    pub(crate) commit: Option<Commit>,
}

/// `extracted`, a key of its own for the reason `FetchReport` gives for `fetched`.
#[derive(serde::Serialize)]
struct ExtractReport {
    extracted: Vec<EntryOutcome>,
}

/// One reading still to be taken: of which bytes, and by what.
#[derive(Debug, Clone)]
enum Job {
    Pdf(ContentHash),
    Pack {
        artifact: ContentHash,
        transform: Transform,
        /// The artifact's media type, or the one its scheme resolves to.
        media_type: String,
    },
}

impl Job {
    fn artifact(&self) -> &ContentHash {
        match self {
            Self::Pdf(h) | Self::Pack { artifact: h, .. } => h,
        }
    }
    fn by(&self) -> String {
        match self {
            Self::Pdf(_) => EXTRACTOR.to_string(),
            Self::Pack { transform, .. } => transform.by(),
        }
    }
}

/// The readings of an entry still to be taken, and the ones that cannot be, with why.
fn unread(
    root: &Path,
    packs: &[sources::Pack],
    artifacts: &[CatalogArtifact],
    locations: &[CatalogLocation],
) -> (Vec<Job>, Vec<Skipped>) {
    let (mut jobs, mut skipped) = (Vec::new(), Vec::new());
    for a in artifacts {
        let Some(hash) = a.sha256.as_deref().and_then(|s| ContentHash::parse(s).ok()) else {
            continue;
        };
        if a.text_reading().is_none() && a.media_type.as_deref().is_some_and(reading::is_pdf) {
            jobs.push(Job::Pdf(hash.clone()));
        }
        match pack_job(root, packs, &hash, a, locations) {
            None => {}
            Some(Ok(job)) => jobs.push(job),
            Some(Err(why)) => skipped.push(Skipped {
                artifact: hash.as_str().to_string(),
                why,
            }),
        }
    }
    (jobs, skipped)
}

/// The pack transform a record's identifier asks for, if it asks for one and has no reading
/// by it yet.
fn pack_job(
    root: &Path,
    packs: &[sources::Pack],
    hash: &ContentHash,
    a: &CatalogArtifact,
    locations: &[CatalogLocation],
) -> Option<std::result::Result<Job, String>> {
    let Some(ArtifactOrigin::Location(i)) = &a.from else {
        return None;
    };
    let location = locations.get(*i)?;
    if location.kind.as_deref() != Some("identifier") {
        return None;
    }
    let (scheme_name, _) = location.value.as_deref()?.split_once(':')?;
    // An identifier no enabled pack declares is `catalog fetch`'s to report, not this one's.
    let (pack, manifest, scheme) = sources::transform::scheme(packs, scheme_name).ok()?;
    let path = scheme.extract.as_deref()?;
    let transform = match Transform::read(root, pack, manifest, path) {
        Ok(t) => t,
        Err(e) => return Some(Err(format!("{e:#}"))),
    };
    if a.all_readings()
        .iter()
        .any(|r| r.by.as_deref() == Some(&transform.by()))
    {
        return None;
    }
    if !sources::transform::AVAILABLE {
        return Some(Err(format!(
            "not read by {}: {}",
            transform.name(),
            sources::transform::UNAVAILABLE
        )));
    }
    let Some(media_type) = a
        .media_type
        .clone()
        .or_else(|| scheme.resolve.media.clone())
    else {
        return Some(Err(format!(
            "not read by {}: the record states no media type and [scheme.{scheme_name}] resolves to none, so nothing says how to parse it",
            transform.name()
        )));
    };
    Some(Ok(Job::Pack {
        artifact: hash.clone(),
        transform,
        media_type,
    }))
}

/// Read one artifact out of the cache, take its reading and file it there, or say why not.
fn take(cache: &Cache, job: &Job, n: usize) -> Result<std::result::Result<Read, String>> {
    let pdf = job.artifact();
    let Ok(raw) = std::fs::read(cache.path_of(pdf)) else {
        return Ok(Err(
            "is not in this machine's vault cache — `yidam vault pull` fetches it".to_string(),
        ));
    };
    let found = ContentHash::of_bytes(&raw);
    if &found != pdf {
        return Ok(Err(format!(
            "is corrupt in this machine's vault cache: the file hashes to {}",
            found.as_str()
        )));
    }
    let (media_type, text, by) = match job {
        Job::Pdf(_) => match reading::extract(&raw) {
            Ok(text) => (TEXT_READING.to_string(), text, EXTRACTOR.to_string()),
            Err(why) => return Ok(Err(why)),
        },
        Job::Pack {
            transform,
            media_type,
            ..
        } => match sources::transform::extract(transform, media_type, &raw) {
            Ok(t) => (t.media_type, t.text, t.by),
            Err(why) => return Ok(Err(why)),
        },
    };
    let hash = ContentHash::of_bytes(text.as_bytes());
    let staged = staging(cache, n);
    if let Some(dir) = staged.parent() {
        std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    }
    std::fs::write(&staged, &text).with_context(|| format!("writing {}", staged.display()))?;
    cache.put_file(&staged, &hash)?;
    let _ = std::fs::remove_file(&staged);
    Ok(Ok(Read {
        artifact: pdf.as_str().to_string(),
        sha256: Some(hash.as_str().to_string()),
        extractor: by,
        bytes: Some(text.len() as u64),
        media_type,
    }))
}

/// The subject line, and a body naming every digest and the extractor that read it.
fn message(entry: &str, read: &[Read]) -> (String, String) {
    let subject = match read {
        [one] => format!("extract: {entry} text of sha256:{}", &one.artifact[..12]),
        many => format!("extract: {entry} {} readings", many.len()),
    };
    let body = read
        .iter()
        .map(|r| {
            format!(
                "sha256:{} read by {} as sha256:{} ({} bytes)\n",
                r.artifact,
                r.extractor,
                r.sha256.as_deref().unwrap_or_default(),
                r.bytes.unwrap_or_default()
            )
        })
        .collect();
    (subject, body)
}

pub fn extract(opts: &ExtractOptions) -> Result<()> {
    let root = repo_root()?;
    let out = extract_in(&root, opts, &mut Writer::WorkingTree)?;
    crate::report::finish(&root, opts.format, ExtractReport { extracted: out }, |r| {
        print!("{}", render(&r.extracted, opts.dry_run))
    })
}

/// [`extract`] against `root`, committing through `writer`.
pub(crate) fn extract_in(
    root: &Path,
    opts: &ExtractOptions,
    writer: &mut Writer,
) -> Result<Vec<EntryOutcome>> {
    let root = root.to_path_buf();
    let catalog = yidam_catalog_dir(&root);
    let entries = select(&catalog, opts.entry.as_deref())?;
    let cache = if opts.dry_run {
        None
    } else {
        Some(Cache::resolve(|k| std::env::var(k).ok())?)
    };

    let packs = sources::load(&root)?;
    let mut out = Vec::new();
    let mut n = 0usize;
    for path in &entries {
        let rel = rel_to(&root, path);
        let name = path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let text =
            std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        let fm = parse_frontmatter(&text);
        let (pending, mut skipped) = unread(
            &root,
            &packs,
            &fm.artifacts.unwrap_or_default(),
            &fm.location.unwrap_or_default(),
        );
        if pending.is_empty() && skipped.is_empty() {
            continue;
        }
        let Some(cache) = &cache else {
            out.push(EntryOutcome {
                entry: name,
                read: pending
                    .iter()
                    .map(|j| Read {
                        artifact: j.artifact().as_str().to_string(),
                        sha256: None,
                        extractor: j.by(),
                        bytes: None,
                        media_type: String::new(),
                    })
                    .collect(),
                skipped,
                commit: None,
            });
            continue;
        };

        // Checked before anything is read, so the refusal names a person's uncommitted work
        // rather than the edit this command is about to make.
        if !pending.is_empty() {
            writer.require_clean(&root, std::slice::from_ref(&rel))?;
        }

        let mut read = Vec::new();
        let mut updated = text.clone();
        for job in &pending {
            n += 1;
            let artifact = job.artifact().as_str();
            match take(cache, job, n)? {
                Ok(r) => {
                    let reading = ArtifactReading {
                        sha256: r.sha256.clone(),
                        media_type: Some(r.media_type.clone()),
                        by: Some(r.extractor.clone()),
                    };
                    if let Some(next) = record::add_reading(&updated, artifact, &reading)
                        .with_context(|| format!("recording a reading in {rel}"))?
                    {
                        updated = next;
                        read.push(r);
                    }
                }
                Err(why) => skipped.push(Skipped {
                    artifact: artifact.to_string(),
                    why,
                }),
            }
        }

        let mut written = None;
        if updated != text {
            let (subject, body) = message(&name, &read);
            written = writer.commit(&root, super::WHO, &subject, &body, &[(rel, updated)])?;
        }
        out.push(EntryOutcome {
            entry: name,
            read,
            skipped,
            commit: written,
        });
    }
    Ok(out)
}

fn rel_to(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .to_string()
}

fn render(entries: &[EntryOutcome], dry_run: bool) -> String {
    use std::fmt::Write;
    if entries.is_empty() {
        return "No catalog entry records an artifact with a reading still to take.\n".to_string();
    }
    let mut s = String::new();
    for e in entries {
        let _ = writeln!(s, "{}", e.entry);
        for r in &e.read {
            match &r.sha256 {
                None => {
                    let _ = writeln!(s, "  would read sha256:{} with {}", r.artifact, r.extractor);
                }
                Some(h) => {
                    let _ = writeln!(
                        s,
                        "  sha256:{}\n    read as sha256:{h} ({} bytes) by {}",
                        r.artifact,
                        r.bytes.unwrap_or_default(),
                        r.extractor
                    );
                }
            }
        }
        for k in &e.skipped {
            let _ = writeln!(s, "  sha256:{} not read — {}", k.artifact, k.why);
        }
        match &e.commit {
            Some(c) => {
                let _ = writeln!(s, "  {} {}", c.sha, c.subject);
            }
            None if !dry_run && e.skipped.is_empty() => {
                let _ = writeln!(s, "  nothing new to record");
            }
            None => {}
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cmd::operational as commit;
    use crate::parse::TextReading;

    fn pdf(sha: &str, text: Option<&str>) -> CatalogArtifact {
        CatalogArtifact {
            sha256: Some(sha.to_string()),
            media_type: Some("application/pdf".to_string()),
            text: text.map(|t| TextReading {
                sha256: Some(t.to_string()),
                extractor: Some(EXTRACTOR.to_string()),
            }),
            ..Default::default()
        }
    }

    fn pdfs(jobs: &[Job]) -> Vec<ContentHash> {
        jobs.iter()
            .filter_map(|j| match j {
                Job::Pdf(h) => Some(h.clone()),
                Job::Pack { .. } => None,
            })
            .collect()
    }

    /// Only a PDF, only one not yet read under either key, and only one whose record says it
    /// is a PDF.
    #[test]
    fn a_reading_is_taken_of_each_pdf_not_yet_read() {
        let a = ContentHash::of_bytes(b"a");
        let b = ContentHash::of_bytes(b"b");
        let c = ContentHash::of_bytes(b"c");
        let d = ContentHash::of_bytes(b"d");
        let e = ContentHash::of_bytes(b"e");
        let mut html = pdf(c.as_str(), None);
        html.media_type = Some("text/html".into());
        let mut unstated = pdf(d.as_str(), None);
        unstated.media_type = None;
        let mut read = pdf(e.as_str(), None);
        read.readings = Some(vec![ArtifactReading {
            sha256: Some(a.as_str().into()),
            media_type: Some("text/plain; charset=utf-8".into()),
            by: Some(EXTRACTOR.into()),
        }]);
        let held = [
            pdf(a.as_str(), None),
            pdf(b.as_str(), Some(a.as_str())),
            html,
            unstated,
            read,
        ];
        let tmp = tempfile::tempdir().unwrap();
        let (jobs, skipped) = unread(tmp.path(), &[], &held, &[]);
        assert_eq!(pdfs(&jobs), vec![a]);
        assert!(skipped.is_empty());
    }

    const PACK: &str = r#"[pack]
name = "europe"
version = "0.2.0"

[scheme.pmc]
pattern = '^PMC\d+$'
type = "paper"
resolve = { template = "https://example.org/{id}", media = "application/xml" }
extract = "transforms/body.glu"
"#;

    /// A root holding one authored pack whose `pmc` scheme declares an extract transform.
    fn with_pack() -> (tempfile::TempDir, Vec<sources::Pack>) {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join(sources::AUTHORED).join("europe");
        std::fs::create_dir_all(dir.join("transforms")).unwrap();
        std::fs::write(dir.join("pack.toml"), PACK).unwrap();
        std::fs::write(
            dir.join("transforms/body.glu"),
            "\\p ->\n    { media_type = \"text/plain\", text = p.media_type }\n",
        )
        .unwrap();
        let packs = sources::load(tmp.path()).unwrap();
        (tmp, packs)
    }

    fn identifier(value: &str) -> CatalogLocation {
        CatalogLocation {
            kind: Some("identifier".into()),
            value: Some(value.into()),
            description: None,
        }
    }

    /// An artifact fetched from an identifier whose scheme declares `extract` is a job for that
    /// transform — once. A reading by the same bytes of the script is not taken again, and one
    /// by an older script is.
    #[test]
    fn an_identifier_whose_pack_extracts_is_read_by_that_transform() {
        let (tmp, packs) = with_pack();
        let h = ContentHash::of_bytes(b"<article/>");
        let a = CatalogArtifact {
            sha256: Some(h.as_str().into()),
            from: Some(ArtifactOrigin::Location(1)),
            ..Default::default()
        };
        let locations = [
            CatalogLocation {
                kind: Some("url".into()),
                value: Some("https://example.org".into()),
                description: None,
            },
            identifier("pmc:PMC1"),
        ];
        let (jobs, skipped) = unread(tmp.path(), &packs, std::slice::from_ref(&a), &locations);
        if !sources::transform::AVAILABLE {
            assert!(jobs.is_empty());
            assert_eq!(skipped.len(), 1);
            assert!(
                skipped[0].why.contains("source-transforms"),
                "{}",
                skipped[0].why
            );
            assert!(
                skipped[0].why.contains("europe/transforms/body.glu"),
                "{}",
                skipped[0].why
            );
            return;
        }
        assert!(skipped.is_empty(), "{skipped:?}");
        let [Job::Pack {
            transform,
            media_type,
            artifact,
        }] = jobs.as_slice()
        else {
            panic!("expected one pack job, got {jobs:?}")
        };
        assert_eq!(artifact, &h);
        assert_eq!(
            media_type, "application/xml",
            "the scheme's media, as the record states none"
        );
        assert!(transform
            .by()
            .starts_with("europe@0.2.0/transforms/body.glu@sha256:"));

        let mut done = a.clone();
        done.readings = Some(vec![ArtifactReading {
            sha256: Some(h.as_str().into()),
            media_type: Some("text/plain".into()),
            by: Some(transform.by()),
        }]);
        let (jobs, _) = unread(tmp.path(), &packs, std::slice::from_ref(&done), &locations);
        assert!(jobs.is_empty(), "{jobs:?}");

        let mut older = a;
        older.readings = Some(vec![ArtifactReading {
            by: Some(format!(
                "europe@0.1.0/transforms/body.glu@sha256:{}",
                "0".repeat(64)
            )),
            ..Default::default()
        }]);
        let (jobs, _) = unread(tmp.path(), &packs, &[older], &locations);
        assert_eq!(jobs.len(), 1);
    }

    /// A location that is not an identifier, or names a scheme no pack declares, asks for no
    /// transform — and says nothing, because that is `catalog fetch`'s to report.
    #[test]
    fn a_location_no_pack_extracts_asks_for_nothing() {
        let (tmp, packs) = with_pack();
        let h = ContentHash::of_bytes(b"x");
        let a = CatalogArtifact {
            sha256: Some(h.as_str().into()),
            from: Some(ArtifactOrigin::Location(0)),
            ..Default::default()
        };
        for loc in [identifier("doi:10.1/x"), identifier("no-colon")] {
            let (jobs, skipped) = unread(tmp.path(), &packs, std::slice::from_ref(&a), &[loc]);
            assert!(
                jobs.is_empty() && skipped.is_empty(),
                "{jobs:?} {skipped:?}"
            );
        }
    }

    #[test]
    fn the_commit_names_every_digest_and_the_extractor() {
        let r = Read {
            artifact: "a".repeat(64),
            sha256: Some("b".repeat(64)),
            extractor: EXTRACTOR.to_string(),
            bytes: Some(9),
            media_type: TEXT_READING.to_string(),
        };
        let (subject, body) = message("veto", std::slice::from_ref(&r));
        assert_eq!(
            subject,
            format!("extract: veto text of sha256:{}", "a".repeat(12))
        );
        assert!(commit::is_operational(&subject));
        assert!(body.contains(&"b".repeat(64)), "{body}");
        assert!(body.contains(EXTRACTOR), "{body}");
        let (subject, _) = message("veto", &[r.clone(), r]);
        assert_eq!(subject, "extract: veto 2 readings");
    }

    /// Absent bytes and corrupt bytes are both reported, not read.
    #[test]
    fn bytes_not_held_or_not_what_they_claim_are_not_read() {
        let tmp = tempfile::tempdir().unwrap();
        let cache = Cache::at(tmp.path());
        let pdf = ContentHash::of_bytes(b"%PDF");
        let job = Job::Pdf(pdf.clone());
        let why = take(&cache, &job, 1).unwrap().unwrap_err();
        assert!(why.contains("yidam vault pull"), "{why}");

        let path = cache.path_of(&pdf);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, b"not those bytes").unwrap();
        let why = take(&cache, &job, 1).unwrap().unwrap_err();
        assert!(why.contains("corrupt"), "{why}");
    }
}
