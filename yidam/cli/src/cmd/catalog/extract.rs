//! `yidam catalog-extract` — take a text reading of each PDF artifact the catalog records (#1172).
//!
//! A quotation of a PDF is compared with a reading of it, never with the PDF, and the reading
//! is taken here, once, rather than by lint on every run: [`crate::reading`] says why. This
//! command reads the PDF's bytes from the vault cache, extracts the text with the extractor
//! [`crate::reading::EXTRACTOR`] names, files the text in the cache under its own digest, and
//! records that digest and extractor under the PDF's record as `text:`. Then it commits
//! `extract:`, an operational verb: the pipeline advanced, and no understanding changed.
//!
//! # What it takes a reading of
//!
//! A record whose `media_type` is `application/pdf` and which records no `text:` yet. A
//! reading already taken is never replaced — see [`super::record::set_text_reading`] — so a
//! re-run over a read catalog changes nothing and commits nothing. A record that states no
//! media type is passed over: which bytes are a PDF is the record's to say, not a guess made
//! from the first bytes of a file.
//!
//! # What it refuses
//!
//! Bytes this machine does not hold, bytes that no longer hash to their name, a PDF the
//! extractor cannot read, and one it reads as no text at all. Each is reported, the rest of
//! the catalog is still read, and nothing is recorded for it.

use std::path::Path;

use anyhow::{Context, Result};

use super::fetch::{select, staging};
use super::record;
use crate::cmd::operational::{Commit, Writer};
use crate::parse::{parse_frontmatter, CatalogArtifact, TextReading};
use crate::paths::{repo_root, yidam_catalog_dir};
use crate::reading::{self, EXTRACTOR};
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
    /// The PDF's digest.
    artifact: String,
    /// The reading's digest. Absent on a dry run, which reads nothing.
    sha256: Option<String>,
    extractor: String,
    bytes: Option<u64>,
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

/// The records of an entry a reading is still to be taken of, with their digests.
fn unread(artifacts: &[CatalogArtifact]) -> Vec<ContentHash> {
    artifacts
        .iter()
        .filter(|a| a.text.is_none())
        .filter(|a| a.media_type.as_deref().is_some_and(reading::is_pdf))
        .filter_map(|a| ContentHash::parse(a.sha256.as_deref()?).ok())
        .collect()
}

/// Read one PDF out of the cache and file its reading there, or say why not.
fn take(cache: &Cache, pdf: &ContentHash, n: usize) -> Result<std::result::Result<Read, String>> {
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
    let text = match reading::extract(&raw) {
        Ok(text) => text,
        Err(why) => return Ok(Err(why)),
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
        extractor: EXTRACTOR.to_string(),
        bytes: Some(text.len() as u64),
    }))
}

/// The subject line, and a body naming every digest and the extractor that read it.
fn message(entry: &str, read: &[Read]) -> (String, String) {
    let subject = match read {
        [one] => format!("extract: {entry} text of sha256:{}", &one.artifact[..12]),
        many => format!("extract: {entry} text of {} artifacts", many.len()),
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
        let pending = unread(&parse_frontmatter(&text).artifacts.unwrap_or_default());
        if pending.is_empty() {
            continue;
        }
        let Some(cache) = &cache else {
            out.push(EntryOutcome {
                entry: name,
                read: pending
                    .iter()
                    .map(|h| Read {
                        artifact: h.as_str().to_string(),
                        sha256: None,
                        extractor: EXTRACTOR.to_string(),
                        bytes: None,
                    })
                    .collect(),
                skipped: vec![],
                commit: None,
            });
            continue;
        };

        // Checked before anything is read, so the refusal names a person's uncommitted work
        // rather than the edit this command is about to make.
        writer.require_clean(&root, std::slice::from_ref(&rel))?;

        let (mut read, mut skipped) = (Vec::new(), Vec::new());
        let mut updated = text.clone();
        for pdf in &pending {
            n += 1;
            match take(cache, pdf, n)? {
                Ok(r) => {
                    let reading = TextReading {
                        sha256: r.sha256.clone(),
                        extractor: Some(r.extractor.clone()),
                    };
                    if let Some(next) = record::set_text_reading(&updated, pdf.as_str(), &reading)
                        .with_context(|| format!("recording a reading in {rel}"))?
                    {
                        updated = next;
                        read.push(r);
                    }
                }
                Err(why) => skipped.push(Skipped {
                    artifact: pdf.as_str().to_string(),
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
        return "No catalog entry records a PDF without a text reading.\n".to_string();
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

    /// Only a PDF, only one not yet read, and only one whose record says it is a PDF.
    #[test]
    fn a_reading_is_taken_of_each_pdf_not_yet_read() {
        let a = ContentHash::of_bytes(b"a");
        let b = ContentHash::of_bytes(b"b");
        let c = ContentHash::of_bytes(b"c");
        let d = ContentHash::of_bytes(b"d");
        let mut html = pdf(c.as_str(), None);
        html.media_type = Some("text/html".into());
        let mut unstated = pdf(d.as_str(), None);
        unstated.media_type = None;
        let held = [
            pdf(a.as_str(), None),
            pdf(b.as_str(), Some(a.as_str())),
            html,
            unstated,
        ];
        assert_eq!(unread(&held), vec![a]);
    }

    #[test]
    fn the_commit_names_every_digest_and_the_extractor() {
        let r = Read {
            artifact: "a".repeat(64),
            sha256: Some("b".repeat(64)),
            extractor: EXTRACTOR.to_string(),
            bytes: Some(9),
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
        assert_eq!(subject, "extract: veto text of 2 artifacts");
    }

    /// Absent bytes and corrupt bytes are both reported, not read.
    #[test]
    fn bytes_not_held_or_not_what_they_claim_are_not_read() {
        let tmp = tempfile::tempdir().unwrap();
        let cache = Cache::at(tmp.path());
        let pdf = ContentHash::of_bytes(b"%PDF");
        let why = take(&cache, &pdf, 1).unwrap().unwrap_err();
        assert!(why.contains("yidam vault pull"), "{why}");

        let path = cache.path_of(&pdf);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, b"not those bytes").unwrap();
        let why = take(&cache, &pdf, 1).unwrap().unwrap_err();
        assert!(why.contains("corrupt"), "{why}");
    }
}
