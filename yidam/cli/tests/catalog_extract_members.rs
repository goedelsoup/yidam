//! A member of a fetched zip, read out of it as a reading, through the binary (#1351).
//!
//! ```text
//!   an entry whose file location is a zip, naming one member under `members:`
//!     → catalog-fetch → the zip in the cache, a record `from: 0`
//!     → catalog-extract → the member in the cache under its own digest, a reading with
//!       `member:` and `by: unzip`, an `extract:` commit
//!     → catalog-extract again → nothing, because the record already has that reading
//! ```
//!
//! The zip is written by [`zip`] rather than kept as a binary fixture, for the reason
//! `catalog_extract.rs` gives for its PDF: the member's text is stated in this file. It is not
//! gated on `pdf-text`, because unpacking a member needs no extractor.

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use yidam::vault::{Cache, ContentHash};

mod common;

fn git(dir: &Path, args: &[&str]) -> String {
    common::git::out_at(dir, args, "@1700000000 +0000")
}

/// A zip holding `members`, each deflated, with no extra fields and no comment.
fn zip(members: &[(&str, &[u8])]) -> Vec<u8> {
    let (mut out, mut central) = (Vec::new(), Vec::new());
    for (name, bytes) in members {
        let mut crc = flate2::Crc::new();
        crc.update(bytes);
        let mut e = flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::best());
        e.write_all(bytes).unwrap();
        let body = e.finish().unwrap();
        let sizes = |h: &mut Vec<u8>| {
            h.extend(8u16.to_le_bytes()); // deflate
            h.extend([0u8; 4]); // time, date
            h.extend(crc.sum().to_le_bytes());
            h.extend((body.len() as u32).to_le_bytes());
            h.extend((bytes.len() as u32).to_le_bytes());
            h.extend((name.len() as u16).to_le_bytes());
            h.extend(0u16.to_le_bytes()); // extra
        };
        let offset = out.len() as u32;
        out.extend(0x0403_4b50u32.to_le_bytes());
        out.extend(20u16.to_le_bytes()); // version needed
        out.extend(0u16.to_le_bytes()); // flags
        sizes(&mut out);
        out.extend(name.as_bytes());
        out.extend(&body);

        central.extend(0x0201_4b50u32.to_le_bytes());
        central.extend(20u16.to_le_bytes()); // version made by
        central.extend(20u16.to_le_bytes()); // version needed
        central.extend(0u16.to_le_bytes()); // flags
        sizes(&mut central);
        central.extend([0u8; 10]); // comment, disk, internal and external attributes
        central.extend(offset.to_le_bytes());
        central.extend(name.as_bytes());
    }
    let at = out.len() as u32;
    out.extend(&central);
    out.extend(0x0605_4b50u32.to_le_bytes());
    out.extend([0u8; 4]); // this disk, the directory's disk
    out.extend((members.len() as u16).to_le_bytes());
    out.extend((members.len() as u16).to_le_bytes());
    out.extend((central.len() as u32).to_le_bytes());
    out.extend(at.to_le_bytes());
    out.extend(0u16.to_le_bytes()); // comment
    out
}

const PACKING_LIST: &str = "STATE=OH\nFILE=oh2010.sf1.prd\nRECORDS=365\nSEGMENTS=48\n";
const GEO: &str = "OHGEO2010 SF1 geographic header, not named by the entry\n";

struct Staged {
    dir: tempfile::TempDir,
    root: PathBuf,
}

impl Staged {
    fn cache(&self) -> PathBuf {
        self.dir.path().join("cache")
    }

    fn entry(&self) -> PathBuf {
        self.root.join(".yidam/catalog/census-2010-sf1.md")
    }

    fn run(&self, args: &[&str]) -> (bool, String, String) {
        let out = Command::new(env!("CARGO_BIN_EXE_yidam"))
            .current_dir(&self.root)
            .args(args)
            .env("YIDAM_VAULT_CACHE", self.cache())
            .env("GIT_AUTHOR_DATE", "@1700000000 +0000")
            .env("GIT_COMMITTER_DATE", "@1700000000 +0000")
            .output()
            .unwrap();
        (
            out.status.success(),
            String::from_utf8_lossy(&out.stdout).to_string(),
            String::from_utf8_lossy(&out.stderr).to_string(),
        )
    }

    fn ok(&self, args: &[&str]) -> String {
        let (ok, stdout, stderr) = self.run(args);
        assert!(ok, "{args:?} failed:\n{stdout}\n{stderr}");
        stdout
    }

    fn commit_count(&self) -> usize {
        git(&self.root, &["rev-list", "--count", "HEAD"])
            .parse()
            .unwrap()
    }
}

/// A repository whose one entry locates a zip kept in-tree, naming `members` inside it.
fn stage(members: &[&str]) -> Staged {
    let dir = tempfile::TempDir::new().unwrap();
    let root = dir.path().join("repo");
    let write = |rel: &str, bytes: &[u8]| {
        let p = root.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, bytes).unwrap();
    };
    write(
        "sources/oh2010.sf1.zip",
        &zip(&[
            ("oh2010.sf1.prd.packinglist.txt", PACKING_LIST.as_bytes()),
            ("ohgeo2010.sf1", GEO.as_bytes()),
        ]),
    );
    let named: String = members.iter().map(|m| format!("      - {m}\n")).collect();
    write(
        ".yidam/catalog/census-2010-sf1.md",
        format!(
            "---\nname: census-2010-sf1\ndescription: The 2010 Census Summary File 1 for Ohio.\n\
             type: dataset\nobtained: false\nlocation:\n  - kind: file\n    \
             value: sources/oh2010.sf1.zip\n    members:\n{named}    \
             description: The state's whole release, one zip.\n---\n\n# Summary File 1\n"
        )
        .as_bytes(),
    );

    git(&root, &["init", "-q", "-b", "main"]);
    git(&root, &["config", "user.email", "runner@test"]);
    git(&root, &["config", "user.name", "Runner"]);
    git(&root, &["add", "."]);
    git(
        &root,
        &["commit", "-q", "-m", "scaffold: zip member fixture"],
    );
    Staged { dir, root }
}

/// The issue's whole loop: fetch a zip, read one member out of it, record it, and stop.
#[test]
fn a_member_of_a_fetched_zip_is_read_out_as_a_reading() {
    let s = stage(&["oh2010.sf1.prd.packinglist.txt"]);
    s.ok(&["catalog-fetch", "census-2010-sf1", "--location", "0"]);
    let zip = ContentHash::of_file(&s.root.join("sources/oh2010.sf1.zip")).unwrap();
    assert!(
        std::fs::read_to_string(s.entry())
            .unwrap()
            .contains(zip.as_str()),
        "the fetch recorded the zip"
    );

    let dry = s.ok(&["catalog-extract", "--dry-run"]);
    assert!(
        dry.contains("member oh2010.sf1.prd.packinglist.txt"),
        "{dry}"
    );

    let commits = s.commit_count();
    let out = s.ok(&["catalog-extract"]);
    assert_eq!(s.commit_count(), commits + 1, "{out}");
    assert_eq!(
        git(&s.root, &["log", "-1", "--format=%s"]),
        format!(
            "extract: census-2010-sf1 oh2010.sf1.prd.packinglist.txt out of sha256:{}",
            &zip.as_str()[..12]
        )
    );

    // The reading is the member's own bytes, under their own digest, recorded under the zip.
    let digest = ContentHash::of_bytes(PACKING_LIST.as_bytes());
    let held =
        std::fs::read(Cache::at(s.cache()).path_of(&digest)).expect("the member is in the cache");
    assert_eq!(held, PACKING_LIST.as_bytes());
    let entry = std::fs::read_to_string(s.entry()).unwrap();
    let reading = format!(
        "    readings:\n      - sha256: {}\n        media_type: text/plain\n        \
         member: oh2010.sf1.prd.packinglist.txt\n        by: unzip\n",
        digest.as_str()
    );
    assert!(entry.contains(&reading), "{entry}");
    // Only the member the location names is read; the other one stays in the zip.
    assert!(!Cache::at(s.cache())
        .path_of(&ContentHash::of_bytes(GEO.as_bytes()))
        .exists());

    // Run again: the record has the reading, so there is nothing to do and nothing to commit.
    let commits = s.commit_count();
    s.ok(&["catalog-extract"]);
    assert_eq!(s.commit_count(), commits);
}

/// A member the zip does not hold is a skip that says so, and nothing is recorded or committed.
#[test]
fn a_member_the_zip_does_not_hold_is_reported_not_recorded() {
    let s = stage(&["oh2010.sf1.prd.missing.txt"]);
    s.ok(&["catalog-fetch", "census-2010-sf1", "--location", "0"]);

    let commits = s.commit_count();
    let (_, stdout, stderr) = s.run(&["catalog-extract"]);
    assert!(
        format!("{stdout}{stderr}").contains("holds no member `oh2010.sf1.prd.missing.txt`"),
        "{stdout}\n{stderr}"
    );
    assert_eq!(s.commit_count(), commits);
    assert!(!std::fs::read_to_string(s.entry())
        .unwrap()
        .contains("readings:"));
}

/// A member path that climbs out of the archive is a lint finding before anything unpacks it.
#[test]
fn a_member_path_that_leaves_the_archive_is_a_lint_finding() {
    let s = stage(&["../../etc/passwd"]);
    let (_, stdout, _) = s.run(&["lint", "--format", "json"]);
    let report: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let details: Vec<String> = report["checks"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|c| c["id"] == "catalog-location-malformed")
        .flat_map(|c| c["violations"].as_array().cloned().unwrap_or_default())
        .map(|v| v["detail"].as_str().unwrap_or_default().to_string())
        .collect();
    assert!(
        details.iter().any(|d| d.contains("`../../etc/passwd`")),
        "{details:?}"
    );
}
