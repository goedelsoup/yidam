//! `yidam catalog-extract` and the lint that reads what it records, through the binary (#1172).
//!
//! ```text
//!   a quotation of a PDF
//!     → lint → quotation-unchecked, naming `catalog-extract`
//!     → catalog-extract → a reading in the cache, `text:` on the record, an `extract:` commit
//!     → lint → nothing for the span the page holds, drift for the one it does not
//!     → catalog-extract again → nothing, because the record already has its reading
//! ```
//!
//! The PDF is written by [`pdf`] rather than kept as a binary fixture: a page whose text is
//! stated in this file is one a reader can check the assertions against, and it carries a
//! line-end hyphen so the whole path, extractor included, is held to the hyphen rule.
//!
//! The cache is redirected into each test's temp directory, for the reason `catalog_fetch.rs`
//! gives.

#![cfg(feature = "pdf-text")]

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use yidam::vault::ContentHash;

mod common;

fn git(dir: &Path, args: &[&str]) -> String {
    common::git::out_at(dir, args, "@1700000000 +0000")
}

/// A one-page PDF showing `lines`, one per line, in a standard font.
///
/// The cross-reference offsets are computed, so the file is one a strict reader accepts.
fn pdf(lines: &[&str]) -> Vec<u8> {
    let mut content = String::from("BT /F1 12 Tf 72 720 Td 14 TL\n");
    for l in lines {
        writeln!(content, "({l}) Tj T*").unwrap();
    }
    content.push_str("ET\n");
    let objects = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_string(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 4 0 R \
         /Resources << /Font << /F1 5 0 R >> >> >>"
            .to_string(),
        format!(
            "<< /Length {} >>\nstream\n{content}endstream",
            content.len()
        ),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>"
            .to_string(),
    ];
    let mut out = b"%PDF-1.4\n".to_vec();
    let mut offsets = Vec::new();
    for (i, o) in objects.iter().enumerate() {
        offsets.push(out.len());
        out.extend(format!("{} 0 obj\n{o}\nendobj\n", i + 1).bytes());
    }
    let xref = out.len();
    out.extend(format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).bytes());
    for at in offsets {
        out.extend(format!("{at:010} 00000 n \n").bytes());
    }
    out.extend(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
            objects.len() + 1
        )
        .bytes(),
    );
    out
}

const PAGE: &[&str] = &[
    "Therefore, a veto of this item is in the public inter-",
    "est, and I return it without my signature.",
];

struct Staged {
    dir: tempfile::TempDir,
    root: PathBuf,
    pdf: ContentHash,
}

impl Staged {
    fn cache(&self) -> PathBuf {
        self.dir.path().join("cache")
    }

    fn entry(&self) -> PathBuf {
        self.root.join(".yidam/catalog/veto-message.md")
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

    /// Every quotation finding lint reports, as `(check, detail)`.
    fn quotation_findings(&self) -> Vec<(String, String)> {
        let (_, stdout, stderr) = self.run(&["lint", "--format", "json"]);
        let report: serde_json::Value = serde_json::from_str(&stdout)
            .unwrap_or_else(|e| panic!("lint printed no report ({e}):\n{stdout}\n{stderr}"));
        report["checks"]
            .as_array()
            .expect("a checks array")
            .iter()
            .filter(|c| {
                c["id"]
                    .as_str()
                    .unwrap_or_default()
                    .starts_with("quotation-")
            })
            .flat_map(|c| {
                c["violations"]
                    .as_array()
                    .cloned()
                    .unwrap_or_default()
                    .into_iter()
                    .map(|v| {
                        (
                            c["id"].as_str().unwrap().to_string(),
                            v["detail"].as_str().unwrap_or_default().to_string(),
                        )
                    })
                    .collect::<Vec<_>>()
            })
            .collect()
    }

    fn commit_count(&self) -> usize {
        git(&self.root, &["rev-list", "--count", "HEAD"])
            .parse()
            .unwrap()
    }
}

/// A repository with one node quoting `spans` from a catalogued PDF, and the PDF in the cache.
fn stage(spans: &[&str]) -> Staged {
    let dir = tempfile::TempDir::new().unwrap();
    let root = dir.path().join("repo");
    let bytes = pdf(PAGE);
    let hash = ContentHash::of_bytes(&bytes);

    let write = |rel: &str, text: &str| {
        let p = root.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, text).unwrap();
    };
    write(
        ".yidam/corpus/provision.ont.yml",
        "class: provision\nlabel: Provision\ndescription: An item of the bill.\nproperties:\n  \
         - name: anchors\n    type: quotation\n    description: Words of the veto message.\n",
    );
    let anchors: String = spans
        .iter()
        .map(|s| format!("    - of: veto-message\n      span: \"{s}\"\n"))
        .collect();
    write(
        ".yidam/corpus/provision/item-12.yml",
        &format!(
            "class: provision\nlabel: Item 12\ndescription: The item vetoed.\nproperties:\n  \
             anchors:\n{anchors}links:\n  - target: ../provision.ont.yml\n    relationship: \
             instance-of\n"
        ),
    );
    write(
        ".yidam/catalog/veto-message.md",
        &format!(
            "---\nname: veto-message\ndescription: The veto message.\ntype: document\n\
             obtained: true\nartifacts:\n  - sha256: {}\n    bytes: {}\n    \
             media_type: application/pdf\n---\n\n# The veto message\n",
            hash.as_str(),
            bytes.len()
        ),
    );
    let cached = yidam::vault::Cache::at(dir.path().join("cache")).path_of(&hash);
    std::fs::create_dir_all(cached.parent().unwrap()).unwrap();
    std::fs::write(&cached, &bytes).unwrap();

    git(&root, &["init", "-q", "-b", "main"]);
    git(&root, &["config", "user.email", "runner@test"]);
    git(&root, &["config", "user.name", "Runner"]);
    git(&root, &["add", "."]);
    git(
        &root,
        &["commit", "-q", "-m", "scaffold: catalog-extract fixture"],
    );
    Staged {
        dir,
        root,
        pdf: hash,
    }
}

/// The whole loop: unchecked, read, recorded, committed, and then checked against the reading.
#[test]
fn a_pdf_quotation_is_checked_once_the_pdf_has_been_read() {
    let s = stage(&[
        "a veto of this item is in the public interest",
        "a veto of this item is in the public good",
    ]);

    let before = s.quotation_findings();
    assert_eq!(before.len(), 2, "{before:?}");
    for (check, detail) in &before {
        assert_eq!(check, "quotation-unchecked", "{detail}");
        assert!(detail.contains("yidam catalog-extract"), "{detail}");
    }

    let dry = s.ok(&["catalog-extract", "--dry-run"]);
    assert!(
        dry.contains(&format!("would read sha256:{}", s.pdf.as_str())),
        "{dry}"
    );
    let commits = s.commit_count();
    let out = s.ok(&["catalog-extract"]);
    assert_eq!(s.commit_count(), commits + 1, "{out}");
    let subject = git(&s.root, &["log", "-1", "--format=%s"]);
    assert_eq!(
        subject,
        format!(
            "extract: veto-message text of sha256:{}",
            &s.pdf.as_str()[..12]
        )
    );

    // The record names the reading, the reading is in the cache, and it holds the page.
    let entry = std::fs::read_to_string(s.entry()).unwrap();
    let (_, nested) = entry
        .split_once("    media_type: application/pdf\n    text:\n      sha256: ")
        .unwrap_or_else(|| panic!("a reading is recorded under the PDF:\n{entry}"));
    let (digest, rest) = nested.split_once('\n').unwrap();
    assert!(
        rest.starts_with("      extractor: pdf-extract 0."),
        "{entry}"
    );
    let digest = ContentHash::parse(digest).unwrap();
    let text = std::fs::read_to_string(yidam::vault::Cache::at(s.cache()).path_of(&digest))
        .expect("the reading is in the cache");
    assert!(text.contains("public inter"), "{text:?}");
    assert!(git(&s.root, &["log", "-1", "--format=%b"]).contains(digest.as_str()));

    // Only the span the page does not hold is reported, and the hyphen the line broke at
    // does not count against the other one.
    let after = s.quotation_findings();
    assert_eq!(after.len(), 1, "{after:?}");
    assert_eq!(after[0].0, "quotation-span-drift");
    assert!(after[0].1.contains("public good"), "{}", after[0].1);

    // A second run finds the reading already taken, and commits nothing.
    let again = s.ok(&["catalog-extract"]);
    assert_eq!(s.commit_count(), commits + 1, "{again}");
    assert!(again.contains("No catalog entry records a PDF"), "{again}");
}

/// Bytes this machine does not hold are reported and not read, and nothing is committed.
#[test]
fn a_pdf_not_in_the_cache_is_reported_and_nothing_is_recorded() {
    let s = stage(&["a veto of this item"]);
    std::fs::remove_dir_all(s.cache()).unwrap();
    let commits = s.commit_count();
    let out = s.ok(&["catalog-extract", "--format", "json"]);
    let report: serde_json::Value = serde_json::from_str(&out).unwrap();
    let skipped = &report["extracted"][0]["skipped"][0];
    assert_eq!(skipped["artifact"].as_str(), Some(s.pdf.as_str()));
    assert!(
        skipped["why"]
            .as_str()
            .unwrap()
            .contains("yidam vault pull"),
        "{out}"
    );
    assert_eq!(s.commit_count(), commits);
    assert!(!std::fs::read_to_string(s.entry())
        .unwrap()
        .contains("text:"));
}
