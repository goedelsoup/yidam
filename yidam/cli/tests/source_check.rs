//! `yidam source check`, end to end (RFC-0048 §3, #1315).
//!
//! The check is the only gate an authored pack has, so these assert on the **exit code** first
//! and the message second: a check that names the error and exits 0 lets the pack through CI.

mod common;

use std::collections::BTreeSet;
use std::path::Path;
use std::process::Command;

const SHA: &str = "0123456789abcdef0123456789abcdef01234567";

fn check(root: &Path) -> (i32, String, serde_json::Value) {
    let text = Command::new(env!("CARGO_BIN_EXE_yidam"))
        .args(["source", "--root"])
        .arg(root)
        .arg("check")
        .output()
        .expect("running yidam");
    let json = Command::new(env!("CARGO_BIN_EXE_yidam"))
        .args(["source", "--root"])
        .arg(root)
        .args(["check", "--format", "json"])
        .output()
        .expect("running yidam");
    assert_eq!(
        text.status.code(),
        json.status.code(),
        "the two formats disagree"
    );
    let said = format!(
        "{}{}",
        String::from_utf8_lossy(&text.stdout),
        String::from_utf8_lossy(&text.stderr)
    );
    let report = serde_json::from_slice(&json.stdout)
        .unwrap_or_else(|e| panic!("not JSON ({e}): {}", String::from_utf8_lossy(&json.stdout)));
    (text.status.code().unwrap_or(-1), said, report)
}

fn corpus() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".yidam/decisions")).unwrap();
    dir
}

const GOOD: &str = r#"[pack]
name    = "scholarly"
version = "0.1.2"

[scheme.doi]
pattern  = '^10\.(?P<registrant>\d{4,9})/\S+$'
type     = "paper"
resolve  = { template = "https://api.crossref.org/works/{id}?r={registrant}", media = "application/json" }
describe = "transforms/crossref.glu"
then     = [{ scheme = "pmc", from = "describe.pmcid" }]

[scheme.pmc]
pattern = '^PMC\d+$'
type    = "paper"
resolve = { template = "https://www.ebi.ac.uk/europepmc/webservices/rest/{id}/fullTextXML" }

[transport]
contact      = "required"
min_interval = "100ms"
auth         = [{ env = "CROSSREF_TOKEN", header = "Crossref-Plus-API-Token", prefix = "Bearer " }]

[fixtures]
"doi:10.1167/tvst.8.5.14" = "crossref-tvst.json"
"#;

/// Write a pack directory holding `manifest`, an entry, the transform GOOD names, and the
/// fixture GOOD names.
fn pack(dir: &Path, manifest: &str) {
    std::fs::create_dir_all(dir.join("transforms")).unwrap();
    std::fs::create_dir_all(dir.join("fixtures")).unwrap();
    std::fs::write(dir.join("pack.toml"), manifest).unwrap();
    std::fs::write(dir.join("entry.md"), "# scholarly\n").unwrap();
    std::fs::write(dir.join("transforms/crossref.glu"), CROSSREF).unwrap();
    std::fs::write(dir.join("fixtures/crossref-tvst.json"), TVST).unwrap();
}

/// A describe transform over a Crossref work: its first title, and its PMCID when it has one.
const CROSSREF: &str = r#"\p ->
    let same : String -> String -> Bool = \a b -> a == b
    let pick name acc f =
        match acc with
        | Some found -> Some found
        | None -> if same f.path name then Some f.value else None
    let text : String -> Option String = \name ->
        match array.foldable.foldl (pick name) None p.fields with
        | Some (Text s) -> Some s
        | _ -> None
    let ids =
        match text "message/pmcid" with
        | Some d -> [{ key = "pmcid", value = d }]
        | None -> []
    { name = text "message/title/0", kind = None, date = None, description = None, identifiers = ids }
"#;

const TVST: &str = r#"{"message": {"title": ["Retinal Imaging"], "pmcid": "PMC6762077"}}"#;

/// Each error as `<path>: <message>`, the way the text report prints it.
fn errors(report: &serde_json::Value) -> Vec<String> {
    report["pack_findings"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|f| f["severity"] == "error")
        .map(|f| {
            format!(
                "{}: {}",
                f["path"].as_str().unwrap(),
                f["message"].as_str().unwrap()
            )
        })
        .collect()
}

fn assert_refused(root: &Path, needle: &str) {
    let (code, said, report) = check(root);
    assert_ne!(code, 0, "refused nothing:\n{said}");
    assert_eq!(report["passed"], false);
    assert!(
        errors(&report).iter().any(|m| m.contains(needle)),
        "no error mentions {needle:?}:\n{said}"
    );
}

#[test]
fn a_corpus_with_no_packs_passes_and_says_so() {
    let dir = corpus();
    let (code, said, report) = check(dir.path());
    assert_eq!(code, 0, "{said}");
    assert!(said.contains("No source packs"), "{said}");
    assert_eq!(report["packs"].as_array().unwrap().len(), 0);
}

#[test]
fn a_well_formed_authored_pack_checks() {
    let dir = corpus();
    pack(&dir.path().join(".yidam/sources/scholarly"), GOOD);
    let (code, said, report) = check(dir.path());
    assert_eq!(code, 0, "{said}");
    assert!(said.contains("Every pack checks."), "{said}");
    let p = &report["packs"][0];
    assert_eq!(p["name"], "scholarly");
    assert_eq!(p["origin"], "authored");
    assert_eq!(p["fixtures"], 1);
}

#[test]
fn a_misspelt_key_fails_the_manifest() {
    let dir = corpus();
    pack(
        &dir.path().join(".yidam/sources/scholarly"),
        &GOOD.replace("min_interval", "min_intreval"),
    );
    assert_refused(dir.path(), "min_intreval");
}

#[test]
fn each_scheme_rule_is_held() {
    for (from, to, needle) in [
        ("'^PMC\\d+$'", "'PMC\\d+'", "anchored"),
        ("'^PMC\\d+$'", "'^PMC(\\d+$'", "compile"),
        ("type    = \"paper\"", "type    = \"papyrus\"", "papyrus"),
        ("rest/{id}/fullTextXML", "rest/{nope}/fullTextXML", "nope"),
        (
            "\"transforms/crossref.glu\"",
            "\"transforms/missing.glu\"",
            "missing.glu",
        ),
        (
            "scheme = \"pmc\", from",
            "scheme = \"arxiv\", from",
            "arxiv",
        ),
        ("\"100ms\"", "\"1m\"", "1m"),
        ("env = \"CROSSREF_TOKEN\"", "env = \"sk-live-123\"", "sk-live-123"),
        (
            "[{ env = \"CROSSREF_TOKEN\", header = \"Crossref-Plus-API-Token\", prefix = \"Bearer \" }]",
            "[\"CROSSREF_TOKEN\"]",
            "names a variable and not how it is sent",
        ),
        ("version = \"0.1.2\"", "version = \"0.1\"", "0.1"),
        ("name    = \"scholarly\"", "name    = \"other\"", "other"),
    ] {
        let dir = corpus();
        assert!(GOOD.contains(from), "{from}");
        pack(
            &dir.path().join(".yidam/sources/scholarly"),
            &GOOD.replace(from, to),
        );
        assert_refused(dir.path(), needle);
    }
}

/// The `[search]` table `source search` asks (#1316), and the fixture its `--offline` answers
/// from.
const SEARCH: &str = r#"
[search]
scheme   = "doi"
template = "https://api.crossref.org/works?query={query}&rows={limit}"
media    = "application/json"
items    = "message/items/*"
id       = "DOI"
title    = "title/0"
fixtures = { "retinal imaging" = "crossref-search.json" }
"#;

const SEARCHED: &str = r#"{"message": {"items": [{"DOI": "10.1167/tvst.8.5.14"}]}}"#;

#[test]
fn each_search_rule_is_held() {
    let good = format!("{GOOD}{SEARCH}");
    let searched = |manifest: &str, fixture: &str| {
        let dir = corpus();
        let at = dir.path().join(".yidam/sources/scholarly");
        pack(&at, manifest);
        std::fs::write(at.join("fixtures/crossref-search.json"), fixture).unwrap();
        dir
    };
    let dir = searched(&good, SEARCHED);
    let (code, said, _) = check(dir.path());
    assert_eq!(code, 0, "{said}");

    for (from, to, needle) in [
        ("scheme   = \"doi\"", "scheme   = \"isbn\"", "isbn"),
        (
            "\"https://api.crossref.org/works?",
            "\"ftp://x/?",
            "http(s)",
        ),
        ("query={query}", "query=fixed", "{query}"),
        ("rows={limit}", "rows={rows}", "{rows}"),
        (
            "items    = \"message/items/*\"",
            "items    = \"/\"",
            "items",
        ),
        (
            "= \"crossref-search.json\"",
            "= \"../pack.toml\"",
            "inside fixtures/",
        ),
        ("= \"crossref-search.json\"", "= \"gone.json\"", "missing"),
        ("id       = \"DOI\"", "id       = \"ISSN\"", "no candidate"),
    ] {
        assert!(good.contains(from), "{from}");
        let dir = searched(&good.replace(from, to), SEARCHED);
        assert_refused(dir.path(), needle);
    }
    let dir = searched(&good, r#"{"message": {"items": [{"DOI": "not-a-doi"}]}}"#);
    assert_refused(dir.path(), "does not admit");
    let dir = searched(&good, "<xml/>");
    assert_refused(dir.path(), "not JSON");
}

#[test]
fn every_fixture_is_claimed_by_a_scheme_and_binds() {
    // A file nothing names.
    let dir = corpus();
    let root = dir.path().join(".yidam/sources/scholarly");
    pack(&root, GOOD);
    std::fs::write(root.join("fixtures/stray.json"), "{}").unwrap();
    assert_refused(dir.path(), "stray.json");

    // A file claimed through a `./` segment, which resolves but is not the name the walk
    // of fixtures/ yields: refused as a path, not reported as both claimed and unclaimed.
    let dir = corpus();
    pack(
        &dir.path().join(".yidam/sources/scholarly"),
        &GOOD.replace("= \"crossref-tvst.json\"", "= \"./crossref-tvst.json\""),
    );
    let (code, said, report) = check(dir.path());
    assert_ne!(code, 0, "refused nothing:\n{said}");
    let errors = errors(&report);
    assert!(
        errors
            .iter()
            .any(|m| m.contains("`./crossref-tvst.json`, which is not a path inside fixtures/")),
        "{said}"
    );
    assert!(
        !errors.iter().any(|m| m.contains("no scheme claims it")),
        "the refused path still left its file unclaimed:\n{said}"
    );

    // An identifier whose local id the scheme's pattern refuses.
    let dir = corpus();
    pack(
        &dir.path().join(".yidam/sources/scholarly"),
        &GOOD.replace("doi:10.1167/tvst.8.5.14", "doi:not-a-doi"),
    );
    assert_refused(dir.path(), "not-a-doi");

    // A scheme the pack does not declare.
    let dir = corpus();
    pack(
        &dir.path().join(".yidam/sources/scholarly"),
        &GOOD.replace("doi:10.1167/tvst.8.5.14", "isbn:10.1167/tvst.8.5.14"),
    );
    assert_refused(dir.path(), "isbn");
}

#[test]
fn a_vendored_pack_answers_to_its_pin() {
    let stamp = |pin: &str| {
        format!("{GOOD}\n[vendored]\npin = \"{pin}\"\nfrom = \"https://github.com/goedelsoup/yidam\"\ncommit = \"{SHA}\"\n")
    };
    let write = |root: &Path, pins: &str, manifest: &str| {
        std::fs::write(root.join(".yidam/decisions/proposals.yml"), pins).unwrap();
        let vendored = root.join(".yidam/.vendor/sources/scholarly");
        let _ = std::fs::remove_dir_all(&vendored);
        pack(&vendored, manifest);
    };

    let dir = corpus();
    write(
        dir.path(),
        "prelude_sources: [scholarly@^0.1]\n",
        &stamp("scholarly@^0.1"),
    );
    let (code, said, report) = check(dir.path());
    assert_eq!(code, 0, "{said}");
    assert_eq!(report["packs"][0]["origin"], "vendored");

    // The pin moved and nobody re-vendored.
    write(
        dir.path(),
        "prelude_sources: [scholarly@^0.2]\n",
        &stamp("scholarly@^0.1"),
    );
    assert_refused(dir.path(), "^0.2");

    // A pin with nothing vendored for it.
    let dir = corpus();
    std::fs::write(
        dir.path().join(".yidam/decisions/proposals.yml"),
        "prelude_sources: [scholarly@^0.1]\n",
    )
    .unwrap();
    assert_refused(dir.path(), "yidam-vendor-update");

    // A vendored pack nothing pins.
    let dir = corpus();
    write(
        dir.path(),
        "prelude_sources: []\n",
        &stamp("scholarly@^0.1"),
    );
    assert_refused(dir.path(), "scholarly");

    // A copy with no provenance, and an authored pack that claims some.
    let dir = corpus();
    write(dir.path(), "prelude_sources: [scholarly@^0.1]\n", GOOD);
    assert_refused(dir.path(), "vendored");
    let dir = corpus();
    pack(
        &dir.path().join(".yidam/sources/scholarly"),
        &stamp("scholarly@^0.1"),
    );
    assert_refused(dir.path(), "vendored");

    // A pin that does not parse.
    let dir = corpus();
    std::fs::write(
        dir.path().join(".yidam/decisions/proposals.yml"),
        "prelude_sources: [scholarly@>=0.1]\n",
    )
    .unwrap();
    assert_refused(dir.path(), ">=0.1");
}

/// The corpus's own pack is read before the vendored one of the same name, and the two do not
/// count as two packs claiming one scheme.
#[test]
fn an_authored_pack_shadows_the_vendored_one() {
    let dir = corpus();
    std::fs::write(
        dir.path().join(".yidam/decisions/proposals.yml"),
        "prelude_sources: [scholarly@^0.1]\n",
    )
    .unwrap();
    pack(
        &dir.path().join(".yidam/.vendor/sources/scholarly"),
        &format!(
            "{GOOD}\n[vendored]\npin = \"scholarly@^0.1\"\nfrom = \"o\"\ncommit = \"{SHA}\"\n"
        ),
    );
    pack(&dir.path().join(".yidam/sources/scholarly"), GOOD);
    let (code, said, report) = check(dir.path());
    assert_eq!(code, 0, "{said}");
    assert!(said.contains("shadowed"), "{said}");
    let packs = report["packs"].as_array().unwrap();
    assert_eq!(packs[0]["origin"], "authored");
    assert_eq!(packs[1]["shadowed"], true);

    // Two different packs claiming one scheme is an error.
    let other = dir.path().join(".yidam/sources/papers");
    pack(
        &other,
        &GOOD.replace("name    = \"scholarly\"", "name    = \"papers\""),
    );
    assert_refused(dir.path(), "doi");
}

/// Every path the populated report emits is declared in the committed schema, and every path
/// the schema declares for it is emitted. The report goldens can only reach the empty arm.
#[test]
fn every_emitted_field_is_declared_in_the_schema() {
    let schema: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(
            common::repo_root().join("yidam/sdks/parity/fixtures/reports/report.schema.json"),
        )
        .unwrap(),
    )
    .unwrap();

    // Both origins, a shadowed copy, and both severities.
    let dir = corpus();
    std::fs::write(
        dir.path().join(".yidam/decisions/proposals.yml"),
        "prelude_sources: [scholarly@^0.1]\n",
    )
    .unwrap();
    pack(
        &dir.path().join(".yidam/.vendor/sources/scholarly"),
        &format!(
            "{GOOD}\n[vendored]\npin = \"scholarly@^0.1\"\nfrom = \"o\"\ncommit = \"{SHA}\"\n"
        ),
    );
    let authored = dir.path().join(".yidam/sources/scholarly");
    pack(&authored, GOOD);
    std::fs::write(authored.join("fixtures/stray.json"), "{}").unwrap();
    let (_, _, report) = check(dir.path());
    let severities: BTreeSet<&str> = report["pack_findings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["severity"].as_str().unwrap())
        .collect();
    assert_eq!(severities, BTreeSet::from(["error", "info"]), "{report}");

    let mut emitted = BTreeSet::new();
    common::paths_of(&report, "", &mut emitted);
    for path in &emitted {
        assert!(
            common::declares(&schema, path),
            "`source check` emits `{path}`, which report.schema.json does not declare"
        );
    }
    for key in ["packs", "pack_findings"] {
        let items = &schema["properties"][key]["items"]["properties"];
        for field in items.as_object().unwrap().keys() {
            let path = format!("{key}[].{field}");
            assert!(
                emitted.contains(&path),
                "the schema declares `{path}` and nothing emits it"
            );
        }
    }
}

/// A build without the engine checks everything else and says, once, what it did not check.
#[cfg(not(feature = "source-transforms"))]
#[test]
fn a_build_without_transforms_says_it_did_not_check_them() {
    let dir = corpus();
    pack(&dir.path().join(".yidam/sources/scholarly"), GOOD);
    let (code, said, report) = check(dir.path());
    assert_eq!(code, 0, "{said}");
    let infos: Vec<&str> = report["pack_findings"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|f| f["severity"] == "info")
        .map(|f| f["message"].as_str().unwrap())
        .collect();
    assert_eq!(infos.len(), 1, "{report}");
    assert!(infos[0].contains("source-transforms"), "{}", infos[0]);
}

/// A transform the closed prelude does not admit fails the check at the script.
#[cfg(feature = "source-transforms")]
#[test]
fn a_transform_that_is_not_a_describe_is_refused() {
    let dir = corpus();
    let at = dir.path().join(".yidam/sources/scholarly");
    pack(&at, GOOD);
    std::fs::write(at.join("transforms/crossref.glu"), "\\p -> 1\n").unwrap();
    assert_refused(dir.path(), "is not a describe transform");
    assert_refused(dir.path(), "transforms/crossref.glu: ");
}

/// A `then` reads an identifier describe never yields for any fixture: it resolves nothing.
#[cfg(feature = "source-transforms")]
#[test]
fn a_then_no_fixture_yields_is_refused() {
    let dir = corpus();
    pack(
        &dir.path().join(".yidam/sources/scholarly"),
        &GOOD.replace("describe.pmcid", "describe.pmid"),
    );
    assert_refused(dir.path(), "yields no identifier `pmid` for any fixture");
}

/// A fixture the transform cannot read — here, not JSON — fails the check at the fixture.
#[cfg(feature = "source-transforms")]
#[test]
fn a_fixture_the_transform_cannot_read_is_refused() {
    let dir = corpus();
    let at = dir.path().join(".yidam/sources/scholarly");
    pack(&at, GOOD);
    std::fs::write(at.join("fixtures/crossref-tvst.json"), "<xml/>").unwrap();
    assert_refused(
        dir.path(),
        "fixtures/crossref-tvst.json: `doi:10.1167/tvst.8.5.14`",
    );
    assert_refused(dir.path(), "is not JSON");
}

/// An extract transform runs over each fixture of its scheme too.
#[cfg(feature = "source-transforms")]
#[test]
fn an_extract_transform_runs_over_its_fixtures() {
    let dir = corpus();
    let at = dir.path().join(".yidam/sources/scholarly");
    let manifest = GOOD.replace(
        "describe = \"transforms/crossref.glu\"",
        "describe = \"transforms/crossref.glu\"\nextract  = \"transforms/body.glu\"",
    );
    pack(&at, &manifest);
    std::fs::write(
        at.join("transforms/body.glu"),
        "\\p ->\n    { media_type = \"text/plain\", text = p.media_type }\n",
    )
    .unwrap();
    let (code, said, _) = check(dir.path());
    assert_eq!(code, 0, "{said}");

    std::fs::write(
        at.join("transforms/body.glu"),
        "\\p ->\n    { media_type = \"text/plain\", text = \"\" }\n",
    )
    .unwrap();
    assert_refused(dir.path(), "returned an empty reading");
}

// ── a listing or a catalog, read for a pin (#1342) ─────────────────────────────

const LOOKUP: &str = r#"[pack]
name    = "federal"
version = "0.1.0"

[scheme.climdiv]
pattern = '^[a-z]+$'
type    = "dataset"
resolve = { listing = "https://www.ncei.noaa.gov/pub/data/cirs/climdiv/", match = '^climdiv-{id}-v1\.0\.0-\d{8}$', pick = "latest" }

[scheme.cms]
pattern = '^[0-9a-f-]{36}$'
type    = "dataset"
resolve = { dcat = "data.cms.gov", media = "text/csv" }

[fixtures]
"climdiv:pcpndv" = "climdiv-listing.html"
"climdiv:pcpndv@climdiv-pcpndv-v1.0.0-20261001" = "climdiv-pcpndv.txt"
"cms:6a3aa708-3c9f-4c1a-8b8d-9d0c1d1e6b1a" = "cms-data.json"
"#;

const CLIMDIV_LISTING: &str = r#"<a href="../">up</a>
<a href="climdiv-pcpndv-v1.0.0-20260901">a</a> <a href="climdiv-pcpndv-v1.0.0-20261001">b</a>"#;

const CMS_CATALOG: &str = r#"{"dataset": [{
  "identifier": "https://data.cms.gov/data-api/v1/dataset/6a3aa708-3c9f-4c1a-8b8d-9d0c1d1e6b1a/data-viewer",
  "distribution": [{"mediaType": "text/csv", "downloadURL": "https://data.cms.gov/files/2026-09/a.csv"}]
}]}"#;

/// Write the `federal` pack: `manifest`, an entry, and the three fixtures LOOKUP names.
fn lookup_pack(root: &Path, manifest: &str) {
    let dir = root.join(".yidam/sources/federal");
    std::fs::create_dir_all(dir.join("fixtures")).unwrap();
    std::fs::write(dir.join("pack.toml"), manifest).unwrap();
    std::fs::write(dir.join("entry.md"), "# federal\n").unwrap();
    std::fs::write(dir.join("fixtures/climdiv-listing.html"), CLIMDIV_LISTING).unwrap();
    std::fs::write(dir.join("fixtures/climdiv-pcpndv.txt"), "0101 1.0\n").unwrap();
    std::fs::write(dir.join("fixtures/cms-data.json"), CMS_CATALOG).unwrap();
}

#[test]
fn a_listing_and_a_catalog_pack_checks() {
    let dir = corpus();
    lookup_pack(dir.path(), LOOKUP);
    let (code, said, report) = check(dir.path());
    assert_eq!(code, 0, "{said}");
    assert_eq!(report["passed"], true, "{said}");
}

#[test]
fn each_lookup_rule_is_held() {
    let listing = r#"resolve = { listing = "https://www.ncei.noaa.gov/pub/data/cirs/climdiv/", match = '^climdiv-{id}-v1\.0\.0-\d{8}$', pick = "latest" }"#;
    let dcat = r#"resolve = { dcat = "data.cms.gov", media = "text/csv" }"#;
    for (from, to, needle) in [
        (
            listing,
            r#"resolve = { listing = "https://www.ncei.noaa.gov/pub/data/cirs/climdiv/" }"#,
            "needs `match`",
        ),
        (
            dcat,
            r#"resolve = { dcat = "data.cms.gov", template = "https://data.cms.gov/{id}" }"#,
            "a scheme resolves one way",
        ),
        (
            dcat,
            r#"resolve = { dcat = "https://data.cms.gov" }"#,
            "not a bare host",
        ),
        (listing, &listing.replace("{id}", "pcpndv"), "binds no slot"),
        (
            listing,
            &listing.replace("^climdiv-", "climdiv-"),
            "not anchored",
        ),
        (
            listing,
            &listing.replace("climdiv/\"", "climdiv/#x\""),
            "carries a `#`",
        ),
        (
            listing,
            &format!("{listing}\ndescribe = \"transforms/x.glu\""),
            "read off a page",
        ),
        // A pinned fixture whose pin the `match` refuses.
        (
            "@climdiv-pcpndv-v1.0.0-20261001\"",
            "@climdiv-tmpcdv-v1.0.0-20261001\"",
            "does not match",
        ),
        // A pin naming a member inside the file it fetches.
        (
            "@climdiv-pcpndv-v1.0.0-20261001\"",
            "@climdiv-pcpndv-v1.0.0-20261001#member\"",
            "never a member",
        ),
        // A recorded listing that lists nothing the scheme would pick.
        ("climdiv:pcpndv\" =", "climdiv:tmpcdv\" =", "none matches"),
        // A recorded catalog with no dataset named by the identifier.
        ("cms:6a3aa708", "cms:0a3aa708", "no dataset"),
    ] {
        let manifest = LOOKUP.replace(from, to);
        assert_ne!(manifest, LOOKUP, "{from} is not in LOOKUP");
        let dir = corpus();
        lookup_pack(dir.path(), &manifest);
        assert_refused(dir.path(), needle);
    }
}

// ── pin reads (#1343) ────────────────────────────────────────────────────────────────────

const PIN: &str = r#"[pack]
name    = "revisions"
version = "0.1.0"

[scheme.wikipedia]
pattern = '^(?P<lang>[a-z][a-z-]*)/(?P<title>[^\s@#]+)@(?P<pin>\d+)$'
type    = "document"
resolve = { template = "https://{lang}.wikipedia.org/w/index.php?oldid={pin}&action=raw" }

[scheme.wikipedia.pin]
mutable = '^(?P<lang>[a-z][a-z-]*)/(?P<title>[^\s@#]+)$'
read    = "https://{lang}.wikipedia.org/w/api.php?action=query&prop=revisions&rvprop=ids&format=json&formatversion=2&titles={title}"
media   = "application/json"
value   = "query/pages/0/revisions/0/revid"
pinned  = "{lang}/{title}@{pin}"

[fixtures]
"wikipedia:en/Allen_County,_Ohio" = "revid.json"
"#;

const REVID: &str = r#"{"query": {"pages": [{"title": "Allen County, Ohio", "revisions": [{"revid": 1375157827}]}]}}"#;

fn pin_pack(root: &Path, manifest: &str) {
    let dir = root.join(".yidam/sources/revisions");
    std::fs::create_dir_all(dir.join("fixtures")).unwrap();
    std::fs::write(dir.join("pack.toml"), manifest).unwrap();
    std::fs::write(dir.join("entry.md"), "# revisions\n").unwrap();
    std::fs::write(dir.join("fixtures/revid.json"), REVID).unwrap();
}

#[test]
fn a_pin_read_pack_checks() {
    let dir = corpus();
    pin_pack(dir.path(), PIN);
    let (code, said, report) = check(dir.path());
    assert_eq!(code, 0, "{said}");
    assert_eq!(report["passed"], true, "{said}");
}

#[test]
fn each_pin_rule_is_held() {
    for (from, to, needle) in [
        (
            "@(?P<pin>\\d+)$'",
            "@(?P<rev>\\d+)$'",
            "has no group `(?P<pin>…)`",
        ),
        (
            "mutable = '^(?P<lang>",
            "mutable = '(?P<lang>",
            "pin.mutable is not anchored",
        ),
        (
            "mutable = '^(?P<lang>",
            "mutable = '^(?P<lang",
            "pin.mutable does not compile",
        ),
        (
            "titles={title}\"",
            "titles={page}\"",
            "pin.read slot `{page}` is bound by nothing",
        ),
        (
            "read    = \"https://",
            "read    = \"ftp://",
            "is not an http(s) address",
        ),
        (
            "\"{lang}/{title}@{pin}\"",
            "\"{lang}/{title}\"",
            "has no `{pin}` slot",
        ),
        (
            "\"{lang}/{title}@{pin}\"",
            "\"{lang}/{page}@{pin}\"",
            "pin.pinned slot `{page}`",
        ),
        (
            "value   = \"query/pages/0/revisions/0/revid\"",
            "value   = \"/\"",
            "pin.value is an empty path",
        ),
        // The recorded answer has nothing at the path.
        (
            "revisions/0/revid\"",
            "revisions/0/oldid\"",
            "answered with nothing at",
        ),
        // The value read pins an identifier the pattern does not admit.
        ("revisions/0/revid\"", "title\"", "does not admit"),
    ] {
        let manifest = PIN.replacen(from, to, 1);
        assert_ne!(manifest, PIN, "{from} is not in PIN");
        let dir = corpus();
        pin_pack(dir.path(), &manifest);
        assert_refused(dir.path(), needle);
    }
}
