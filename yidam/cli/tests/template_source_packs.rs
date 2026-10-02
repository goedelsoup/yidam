//! The source packs the template ships in `yidam/sources/`, as a corpus gets them (#1320).
//!
//! `source_check.rs` and `source_add.rs` hold the machinery to account against packs they
//! write themselves. This holds the shipped packs to account against the machinery: each one
//! checks once step 8 has vendored and pinned it, and `scholarly` does what RFC-0048 §7 says
//! it is for — one `source add --fetch doi:…` reaches the full text 140 entries were given by
//! hand, and `catalog-fetch` records it.
//!
//! Nothing here touches the network. The acceptance run serves each pack's own recorded
//! fixtures from loopback, so the bytes it records are the ones `source check` reads.

mod common;

use std::path::Path;
use std::process::Command;

const SHA: &str = "0123456789abcdef0123456789abcdef01234567";

/// Every shipped pack, by name, with its tracked files relative to the pack's directory.
fn shipped() -> Vec<(String, Vec<String>)> {
    let mut packs: std::collections::BTreeMap<String, Vec<String>> = Default::default();
    for path in common::tracked_under(&common::repo_root(), "yidam/sources/") {
        let rest = path.strip_prefix("yidam/sources/").unwrap();
        let (pack, file) = rest
            .split_once('/')
            .expect("a file inside a pack directory");
        packs
            .entry(pack.to_string())
            .or_default()
            .push(file.to_string());
    }
    packs.into_iter().collect()
}

/// Copy a shipped pack's tracked files into `to`.
fn install(pack: &str, files: &[String], to: &Path) {
    let from = common::repo_root().join("yidam/sources").join(pack);
    for file in files {
        let dest = to.join(file);
        std::fs::create_dir_all(dest.parent().unwrap()).unwrap();
        std::fs::copy(from.join(file), &dest).unwrap();
    }
}

/// `0.1.0` → `^0.1`: the range step 8 pins a pack at.
fn range(manifest: &str) -> String {
    let table: toml::Value = toml::from_str(manifest).unwrap();
    let version = table["pack"]["version"].as_str().unwrap();
    let mut parts = version.split('.');
    format!("^{}.{}", parts.next().unwrap(), parts.next().unwrap())
}

#[test]
fn the_template_ships_scholarly_and_archive() {
    let names: Vec<String> = shipped().into_iter().map(|(n, _)| n).collect();
    for want in ["archive", "scholarly"] {
        assert!(names.iter().any(|n| n == want), "{names:?}");
    }
}

/// Each shipped pack, copied where step 8 copies it with the `[vendored]` table step 8
/// appends, and pinned in `prelude_sources`, passes `source check`. In a build with
/// `source-transforms` that runs every describe and extract over the pack's own fixtures.
#[test]
fn every_shipped_pack_checks_once_vendored_and_pinned() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let mut pins = Vec::new();
    let packs = shipped();
    for (pack, files) in &packs {
        let at = root.join(".yidam/.vendor/sources").join(pack);
        install(pack, files, &at);
        let manifest = std::fs::read_to_string(at.join("pack.toml")).unwrap();
        let pin = format!("{pack}@{}", range(&manifest));
        std::fs::write(
            at.join("pack.toml"),
            format!(
                "{manifest}\n[vendored]\npin = \"{pin}\"\n\
                 from = \"https://github.com/goedelsoup/yidam\"\ncommit = \"{SHA}\"\n"
            ),
        )
        .unwrap();
        pins.push(pin);
    }
    std::fs::create_dir_all(root.join(".yidam/decisions")).unwrap();
    std::fs::write(
        root.join(".yidam/decisions/proposals.yml"),
        format!("prelude_sources: [{}]\n", pins.join(", ")),
    )
    .unwrap();

    let out = Command::new(env!("CARGO_BIN_EXE_yidam"))
        .args(["source", "--root"])
        .arg(root)
        .args(["check", "--format", "json"])
        .output()
        .unwrap();
    let said = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success(), "{said}");
    let report: serde_json::Value = serde_json::from_str(&said).unwrap();
    let checked = report["packs"].as_array().unwrap();
    assert_eq!(checked.len(), packs.len(), "{said}");
    for p in checked {
        assert_eq!(p["origin"], "vendored", "{said}");
    }
}

// ── the acceptance run ─────────────────────────────────────────────────────────────────

/// What a request for `path` is answered with: the first route whose key it contains.
type Route = (&'static str, &'static str, &'static str);

/// Which recorded fixture answers which publisher's address. Each key is a piece of the
/// shipped template, so a template that changed shape would be answered with a 404.
const ROUTES: &[Route] = &[
    (
        "/api.crossref.org/works/10.1167/tvst.8.5.14",
        "crossref-tvst.json",
        "application/json",
    ),
    (
        "/www.ebi.ac.uk/europepmc/webservices/rest/search?query=DOI:%2210.1167/tvst.8.5.14%22",
        "europepmc-tvst.json",
        "application/json",
    ),
    (
        "/www.ebi.ac.uk/europepmc/webservices/rest/PMC6753881/fullTextXML",
        "europepmc-pmc6753881.xml",
        "application/xml",
    ),
];

/// Serve `ROUTES` from `fixtures` on a loopback port until the process exits, one request per
/// connection, and keep every request line it was asked.
fn serve(fixtures: std::path::PathBuf) -> (u16, std::sync::Arc<std::sync::Mutex<Vec<String>>>) {
    use std::io::{Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let log = seen.clone();
    std::thread::spawn(move || {
        for sock in listener.incoming() {
            let Ok(mut sock) = sock else { continue };
            let mut got = Vec::new();
            let mut buf = [0u8; 4096];
            while !String::from_utf8_lossy(&got).contains("\r\n\r\n") {
                match sock.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => got.extend_from_slice(&buf[..n]),
                }
            }
            let request = String::from_utf8_lossy(&got).to_string();
            let path = request.split(' ').nth(1).unwrap_or_default().to_string();
            let answer = match ROUTES.iter().find(|(k, _, _)| path.starts_with(k)) {
                Some((_, file, media)) => {
                    let body = std::fs::read(fixtures.join(file)).unwrap();
                    let mut a = format!(
                        "HTTP/1.1 200 OK\r\ncontent-type: {media}\r\ncontent-length: {}\r\n\
                         connection: close\r\n\r\n",
                        body.len()
                    )
                    .into_bytes();
                    a.extend(body);
                    a
                }
                None => b"HTTP/1.1 404 Not Found\r\ncontent-length: 0\r\nconnection: close\r\n\r\n"
                    .to_vec(),
            };
            let _ = sock.write_all(&answer);
            log.lock().unwrap().push(request);
        }
    });
    (port, seen)
}

fn git(dir: &Path, args: &[&str]) -> String {
    common::git::out_at(dir, args, "@1700000000 +0000")
}

/// RFC-0048 §7's acceptance: the 140-entry signature — a DOI, and Europe PMC's full text of
/// it — from one `source add --fetch doi:…`, ending in the `artifacts:` `catalog-fetch` writes,
/// and then the full text read by the pack's own extract.
///
/// The shipped pack is installed as the corpus's own with only its hosts moved to loopback
/// and arXiv's three-second interval shortened; every scheme, pattern, transform and template
/// path is the one the template ships.
#[cfg(feature = "source-transforms")]
#[test]
fn one_doi_reaches_the_full_text_and_catalog_fetch_records_it() {
    let fixtures = common::repo_root().join("yidam/sources/scholarly/fixtures");
    let (port, seen) = serve(fixtures.clone());

    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("repo");
    let cache = dir.path().join("cache");
    let catalog_fixture =
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/catalog-fetch");
    for path in common::tracked_under(
        &common::repo_root(),
        "yidam/cli/tests/fixtures/catalog-fetch/",
    ) {
        let rel = path
            .strip_prefix("yidam/cli/tests/fixtures/catalog-fetch/")
            .unwrap();
        let dest = root.join(rel);
        std::fs::create_dir_all(dest.parent().unwrap()).unwrap();
        std::fs::copy(catalog_fixture.join(rel), &dest).unwrap();
    }
    let (_, files) = shipped()
        .into_iter()
        .find(|(n, _)| n == "scholarly")
        .unwrap();
    let pack = root.join(".yidam/sources/scholarly");
    install("scholarly", &files, &pack);
    let manifest = std::fs::read_to_string(pack.join("pack.toml")).unwrap();
    assert!(manifest.contains("min_interval = \"3s\""), "{manifest}");
    std::fs::write(
        pack.join("pack.toml"),
        manifest
            .replace("https://", &format!("http://127.0.0.1:{port}/"))
            .replace("min_interval = \"3s\"", "min_interval = \"10ms\""),
    )
    .unwrap();
    git(&root, &["init", "-q", "-b", "main"]);
    git(&root, &["config", "user.email", "runner@test"]);
    git(&root, &["config", "user.name", "Runner"]);
    git(&root, &["add", "."]);
    git(
        &root,
        &["commit", "-q", "-m", "scaffold: the scholarly pack"],
    );

    let run = |args: &[&str]| {
        let out = Command::new(env!("CARGO_BIN_EXE_yidam"))
            .current_dir(&root)
            .args(args)
            .env("YIDAM_CONTACT", "ops@example.org")
            .env("NO_PROXY", "127.0.0.1")
            .env("YIDAM_VAULT_CACHE", &cache)
            .env("GIT_AUTHOR_DATE", "@1700000000 +0000")
            .env("GIT_COMMITTER_DATE", "@1700000000 +0000")
            .output()
            .unwrap();
        let said = format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        assert!(out.status.success(), "{args:?}:\n{said}");
        said
    };
    let said = run(&["source", "add", "--fetch", "doi:10.1167/tvst.8.5.14"]);

    let slug = "the-effects-of-age-and-central-field-loss-on-head-scanning";
    let entry = std::fs::read_to_string(root.join(format!(".yidam/catalog/{slug}.md"))).unwrap();
    for want in [
        "type: paper",
        "date: 2019-09-16",
        "    value: doi:10.1167/tvst.8.5.14",
        "    value: europepmc:10.1167/tvst.8.5.14",
        "    value: pmc:PMC6753881",
        "# The Effects of Age and Central Field Loss on Head Scanning and Detection at \
         Intersections",
        "artifacts:",
    ] {
        assert!(entry.contains(want), "no `{want}` in:\n{entry}\n{said}");
    }
    // One record per location, and the full text's is the recorded bytes, by their address.
    for from in 0..3 {
        assert!(entry.contains(&format!("    from: {from}\n")), "{entry}");
    }
    let full_text =
        yidam::vault::ContentHash::of_file(&fixtures.join("europepmc-pmc6753881.xml")).unwrap();
    assert!(entry.contains(full_text.as_str()), "{entry}");

    let log = git(&root, &["log", "--format=%s", "-2"]);
    let subjects: Vec<&str> = log.lines().collect();
    assert!(
        subjects[0].starts_with(&format!("refresh: {slug}")),
        "{log}"
    );
    assert!(
        subjects[1].starts_with(&format!("catalog: {slug}")),
        "{log}"
    );

    let asked = seen.lock().unwrap().clone();
    assert!(
        asked.iter().all(|r| r.contains("(+ops@example.org)")),
        "every request names the contact the pack requires:\n{asked:#?}"
    );
    assert!(
        asked.iter().any(|r| r.contains("/PMC6753881/fullTextXML")),
        "{asked:#?}"
    );

    // The full text, read by the pack's extract: the article and not its reference list.
    run(&["catalog-extract", slug]);
    let entry = std::fs::read_to_string(root.join(format!(".yidam/catalog/{slug}.md"))).unwrap();
    assert!(
        entry.contains("scholarly@0.1.0/transforms/europepmc-body.glu@sha256:"),
        "{entry}"
    );
    let reading = entry
        .split("readings:\n      - sha256: ")
        .nth(1)
        .and_then(|r| r.lines().next())
        .unwrap_or_else(|| panic!("no reading in:\n{entry}"));
    let text =
        std::fs::read_to_string(cache.join("sha256").join(&reading[..2]).join(reading)).unwrap();
    assert!(
        text.starts_with("The Effects of Age and Central Field Loss"),
        "{text}"
    );
    assert!(text.contains("\n\nTranslational Relevance\n\n"), "{text}");
    assert!(
        !text.contains("Optom Vis Sci"),
        "the reference list is not the article"
    );
}
