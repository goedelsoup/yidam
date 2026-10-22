//! RFC-0049 §4: what an export may overwrite.
//!
//! The folders this writes are committed to repositories that sync both ways, so a re-export
//! must not overwrite a change someone made at the other end. An export owns a file when its
//! manifest records it or when the file carries the marker. It overwrites an owned file only
//! when the file still hashes to what the manifest recorded, and it never touches a file it
//! does not own. Every refusal is found before anything is written.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{bail, Context, Result};
use sha2::{Digest, Sha256};

/// The manifest's file name, in the export's output directory.
pub(crate) const MANIFEST: &str = ".yidam-export.json";

/// How far into a file the marker is looked for. Every renderer writes it on the first line.
const MARKER_WINDOW: usize = 512;

#[derive(serde::Serialize, serde::Deserialize)]
struct Manifest {
    #[serde(rename = "$comment", default)]
    comment: String,
    format_version: u32,
    generator: String,
    /// Output-relative path → `sha256:<hex>` of the bytes written.
    files: BTreeMap<String, String>,
}

/// What [`write_owned`] did, path by path.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct Outcome {
    pub written: Vec<String>,
    pub unchanged: Vec<String>,
    /// Present, unmarked and unrecorded: not this export's file, so not touched.
    pub left_alone: Vec<String>,
    /// Recorded by the last export, produced by none this time, and unedited.
    pub removed: Vec<String>,
    /// Recorded, not produced this time, and under the `history` prefix: kept as written.
    pub kept: Vec<String>,
}

fn digest(bytes: &[u8]) -> String {
    format!("sha256:{}", hex::encode(Sha256::digest(bytes)))
}

fn carries_marker(bytes: &[u8], marker_prefix: &str) -> bool {
    let window = &bytes[..bytes.len().min(MARKER_WINDOW)];
    String::from_utf8_lossy(window).contains(marker_prefix)
}

/// Write `files` under `out`, keeping to the ownership rules, and record them in the manifest.
///
/// - A recorded file whose bytes no longer hash to the record was edited, and is a refusal.
/// - A marked file with no record cannot be shown unedited, and is a refusal.
/// - An unmarked file with no record is someone else's. It is left alone and reported.
/// - A recorded file this export no longer produces is removed if unedited.
///
/// - A recorded file under `history` that this export does not produce is kept, not removed:
///   it is a migration somebody may already have run (§6). An edit to one is still a refusal.
///
/// `force` turns each refusal into an overwrite (or a removal, or, for a kept file, a record
/// of its edited bytes). It never touches an unowned file, because the export has no claim
/// on one.
pub(crate) fn write_owned(
    out: &Path,
    generator: &str,
    marker_prefix: &str,
    files: &[(String, Vec<u8>)],
    history: Option<&str>,
    force: bool,
) -> Result<Outcome> {
    let manifest_path = out.join(MANIFEST);
    let recorded: BTreeMap<String, String> = match std::fs::read(&manifest_path) {
        Ok(bytes) => {
            serde_json::from_slice::<Manifest>(&bytes)
                .with_context(|| format!("reading {}", manifest_path.display()))?
                .files
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => BTreeMap::new(),
        Err(e) => return Err(e).with_context(|| format!("reading {}", manifest_path.display())),
    };

    let mut outcome = Outcome::default();
    let mut refusals = Vec::new();
    let mut to_write: Vec<&(String, Vec<u8>)> = Vec::new();
    let mut next: BTreeMap<String, String> = BTreeMap::new();

    for file in files {
        let (rel, bytes) = file;
        let path = out.join(rel);
        let on_disk = match std::fs::read(&path) {
            Ok(b) => Some(b),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => return Err(e).with_context(|| format!("reading {}", path.display())),
        };
        let Some(on_disk) = on_disk else {
            to_write.push(file);
            next.insert(rel.clone(), digest(bytes));
            continue;
        };
        if on_disk == *bytes {
            outcome.unchanged.push(rel.clone());
            next.insert(rel.clone(), digest(bytes));
            continue;
        }
        match recorded.get(rel) {
            Some(hash) if *hash == digest(&on_disk) => to_write.push(file),
            Some(_) => {
                refusals.push(format!("{rel} was edited since the last export"));
                to_write.push(file);
            }
            None if carries_marker(&on_disk, marker_prefix) => {
                refusals.push(format!(
                    "{rel} carries the export marker but the manifest has no record of it, so \
                     it cannot be shown unedited"
                ));
                to_write.push(file);
            }
            None => {
                outcome.left_alone.push(rel.clone());
                continue;
            }
        }
        next.insert(rel.clone(), digest(bytes));
    }

    let mut to_remove = Vec::new();
    for (rel, hash) in &recorded {
        if next.contains_key(rel) || outcome.left_alone.contains(rel) {
            continue;
        }
        let path = out.join(rel);
        let kept = history.is_some_and(|h| rel.starts_with(h));
        match std::fs::read(&path) {
            Ok(b) if kept => {
                if digest(&b) != *hash {
                    refusals.push(format!(
                        "{rel} is a migration an earlier export wrote, and was edited since"
                    ));
                }
                outcome.kept.push(rel.clone());
                next.insert(rel.clone(), digest(&b));
            }
            Ok(b) if digest(&b) == *hash => to_remove.push(rel.clone()),
            Ok(_) => {
                refusals.push(format!(
                    "{rel} is no longer generated, and was edited since the last export"
                ));
                to_remove.push(rel.clone());
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e).with_context(|| format!("reading {}", path.display())),
        }
    }

    if !refusals.is_empty() && !force {
        bail!(
            "Nothing was written. These files belong to the export but changed at the other \
             end, and overwriting them would lose that change:\n  - {}\n\
             Carry the change into the corpus and export again, or pass --force to overwrite.",
            refusals.join("\n  - ")
        );
    }

    for (rel, bytes) in to_write {
        let path = out.join(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, bytes).with_context(|| format!("writing {}", path.display()))?;
        outcome.written.push(rel.clone());
    }
    for rel in to_remove {
        let path = out.join(&rel);
        std::fs::remove_file(&path)?;
        // A directory the removal emptied goes too: one export's whole feed is a folder, and
        // dropping it should not leave the folder behind. `remove_dir` refuses a directory
        // anything else is in, so a file someone else keeps there keeps its folder.
        for dir in path.ancestors().skip(1) {
            if dir == out || std::fs::remove_dir(dir).is_err() {
                break;
            }
        }
        outcome.removed.push(rel);
    }

    let manifest = Manifest {
        comment: format!(
            "Written by `{generator}`: the hash of each file it wrote, which is how a re-export \
             tells an edited file from its own output. Do not edit."
        ),
        format_version: 1,
        generator: generator.to_string(),
        files: next,
    };
    std::fs::create_dir_all(out)?;
    let mut body = serde_json::to_string_pretty(&manifest)?;
    body.push('\n');
    std::fs::write(&manifest_path, body)?;
    Ok(outcome)
}

#[cfg(test)]
mod tests {
    use super::*;

    const MARK: &str = "-- Generated by test";

    fn files(entries: &[(&str, &str)]) -> Vec<(String, Vec<u8>)> {
        entries
            .iter()
            .map(|(r, b)| ((*r).to_string(), format!("{MARK}\n{b}").into_bytes()))
            .collect()
    }

    fn write(dir: &Path, entries: &[(&str, &str)], force: bool) -> Result<Outcome> {
        write_owned(dir, "test", MARK, &files(entries), None, force)
    }

    fn read(dir: &Path, rel: &str) -> String {
        std::fs::read_to_string(dir.join(rel)).unwrap()
    }

    #[test]
    fn a_first_export_writes_every_file_and_records_it() {
        let dir = tempfile::tempdir().unwrap();
        let o = write(dir.path(), &[("a.sql", "1"), ("d/b.sql", "2")], false).unwrap();
        assert_eq!(o.written, ["a.sql", "d/b.sql"]);
        let m: Manifest = serde_json::from_str(&read(dir.path(), MANIFEST)).unwrap();
        assert_eq!(m.files.keys().collect::<Vec<_>>(), ["a.sql", "d/b.sql"]);
        assert_eq!(m.files["a.sql"], digest(format!("{MARK}\n1").as_bytes()));
    }

    #[test]
    fn a_folder_its_removals_empty_goes_and_one_holding_another_file_stays() {
        let dir = tempfile::tempdir().unwrap();
        write(
            dir.path(),
            &[("a.sql", "1"), ("f/x/b.sql", "2"), ("g/c.sql", "3")],
            false,
        )
        .unwrap();
        std::fs::write(dir.path().join("g/notes.txt"), "kept").unwrap();
        let o = write(dir.path(), &[("a.sql", "1")], false).unwrap();
        assert_eq!(o.removed, ["f/x/b.sql", "g/c.sql"]);
        assert!(!dir.path().join("f").exists());
        assert_eq!(read(dir.path(), "g/notes.txt"), "kept");
    }

    #[test]
    fn an_unedited_file_is_overwritten_with_the_new_output() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), &[("a.sql", "1")], false).unwrap();
        let o = write(dir.path(), &[("a.sql", "2")], false).unwrap();
        assert_eq!(o.written, ["a.sql"]);
        assert!(read(dir.path(), "a.sql").ends_with('2'));
        let o = write(dir.path(), &[("a.sql", "2")], false).unwrap();
        assert_eq!(o.unchanged, ["a.sql"]);
    }

    #[test]
    fn an_edited_file_is_refused_and_nothing_is_written() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), &[("a.sql", "1"), ("b.sql", "1")], false).unwrap();
        std::fs::write(dir.path().join("a.sql"), format!("{MARK}\nedited")).unwrap();
        let err = write(dir.path(), &[("a.sql", "2"), ("b.sql", "2")], false).unwrap_err();
        assert!(err.to_string().contains("a.sql was edited"), "{err}");
        assert!(read(dir.path(), "a.sql").ends_with("edited"));
        assert!(
            read(dir.path(), "b.sql").ends_with('1'),
            "a refusal must leave every file as it was"
        );
    }

    #[test]
    fn force_overwrites_an_edited_file() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), &[("a.sql", "1")], false).unwrap();
        std::fs::write(dir.path().join("a.sql"), format!("{MARK}\nedited")).unwrap();
        write(dir.path(), &[("a.sql", "2")], true).unwrap();
        assert!(read(dir.path(), "a.sql").ends_with('2'));
    }

    #[test]
    fn an_unmarked_unrecorded_file_survives_even_under_force() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.sql"), "-- mine\n").unwrap();
        let o = write(dir.path(), &[("a.sql", "1")], true).unwrap();
        assert_eq!(o.left_alone, ["a.sql"]);
        assert_eq!(read(dir.path(), "a.sql"), "-- mine\n");
        let m: Manifest = serde_json::from_str(&read(dir.path(), MANIFEST)).unwrap();
        assert!(
            m.files.is_empty(),
            "an unowned file is not recorded as ours"
        );
    }

    #[test]
    fn a_marked_file_with_no_record_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.sql"), format!("{MARK}\nold")).unwrap();
        let err = write(dir.path(), &[("a.sql", "1")], false).unwrap_err();
        assert!(err.to_string().contains("no record"), "{err}");
    }

    #[test]
    fn a_file_no_longer_generated_is_removed_only_when_unedited() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), &[("a.sql", "1"), ("b.sql", "1")], false).unwrap();
        let o = write(dir.path(), &[("a.sql", "1")], false).unwrap();
        assert_eq!(o.removed, ["b.sql"]);
        assert!(!dir.path().join("b.sql").exists());

        write(dir.path(), &[("a.sql", "1"), ("c.sql", "1")], false).unwrap();
        std::fs::write(dir.path().join("c.sql"), "edited").unwrap();
        let err = write(dir.path(), &[("a.sql", "1")], false).unwrap_err();
        assert!(
            err.to_string().contains("c.sql is no longer generated"),
            "{err}"
        );
        assert!(dir.path().join("c.sql").exists());
    }

    #[test]
    fn a_migration_no_longer_produced_is_kept_and_an_edit_to_it_refused() {
        let dir = tempfile::tempdir().unwrap();
        let hist = |entries: &[(&str, &str)], force| {
            write_owned(
                dir.path(),
                "test",
                MARK,
                &files(entries),
                Some("ddl/"),
                force,
            )
        };
        hist(&[("c.json", "1"), ("ddl/0001.sql", "1")], false).unwrap();
        let o = hist(&[("c.json", "2"), ("ddl/0002.sql", "2")], false).unwrap();
        assert_eq!(o.kept, ["ddl/0001.sql"]);
        assert!(o.removed.is_empty());
        let m: Manifest = serde_json::from_str(&read(dir.path(), MANIFEST)).unwrap();
        assert!(
            m.files.contains_key("ddl/0001.sql"),
            "a kept file stays recorded"
        );

        std::fs::write(dir.path().join("ddl/0001.sql"), "edited").unwrap();
        let err = hist(&[("c.json", "3")], false).unwrap_err();
        assert!(
            err.to_string().contains("ddl/0001.sql is a migration"),
            "{err}"
        );
        assert!(read(dir.path(), "c.json").ends_with('2'));
        hist(&[("c.json", "3")], true).unwrap();
        assert_eq!(
            read(dir.path(), "ddl/0001.sql"),
            "edited",
            "force records, never rewrites"
        );
        hist(&[("c.json", "3")], false).unwrap();
    }
}
