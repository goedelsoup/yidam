//! `yidam source list`, `search` and `add`, end to end (RFC-0048 §4, #1316).
//!
//! Every run here is `--offline` or is refused before it asks, so the suite makes no request.
//! The environment a pack's transport reads is cleared on every run and set only where a test
//! means it: a developer with `YIDAM_CONTACT` exported would otherwise see a different report.

mod common;

use std::path::Path;
use std::process::Command;

const ENV: &[&str] = &["YIDAM_CONTACT", "CROSSREF_TOKEN"];

struct Run {
    code: i32,
    said: String,
}

fn yidam(root: &Path, args: &[&str], env: &[(&str, &str)]) -> Run {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_yidam"));
    cmd.args(["source", "--root"]).arg(root).args(args);
    for k in ENV {
        cmd.env_remove(k);
    }
    for (k, v) in env {
        cmd.env(k, v);
    }
    let o = cmd.output().expect("running yidam");
    Run {
        code: o.status.code().unwrap_or(-1),
        said: format!(
            "{}{}",
            String::from_utf8_lossy(&o.stdout),
            String::from_utf8_lossy(&o.stderr)
        ),
    }
}

fn json(root: &Path, args: &[&str], env: &[(&str, &str)]) -> serde_json::Value {
    let mut all = args.to_vec();
    all.extend(["--format", "json"]);
    let r = yidam(root, &all, env);
    assert_eq!(r.code, 0, "{}", r.said);
    serde_json::from_str(&r.said).unwrap_or_else(|e| panic!("not JSON ({e}): {}", r.said))
}

fn git(dir: &Path, args: &[&str]) -> String {
    common::git::out_at(dir, args, common::git::FIXTURE_DATE)
}

/// A committed corpus holding one pack, `scholarly`, with `manifest`.
fn corpus(manifest: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    std::fs::create_dir_all(root.join(".yidam/decisions")).unwrap();
    std::fs::create_dir_all(root.join(".yidam/catalog")).unwrap();
    std::fs::write(root.join(".yidam/decisions/.keep"), "").unwrap();
    let pack = root.join(".yidam/sources/scholarly");
    std::fs::create_dir_all(pack.join("transforms")).unwrap();
    std::fs::create_dir_all(pack.join("fixtures")).unwrap();
    std::fs::write(pack.join("pack.toml"), manifest).unwrap();
    std::fs::write(pack.join("entry.md"), ENTRY).unwrap();
    std::fs::write(pack.join("transforms/crossref.glu"), CROSSREF).unwrap();
    std::fs::write(pack.join("fixtures/crossref-tvst.json"), TVST).unwrap();
    std::fs::write(pack.join("fixtures/crossref-search.json"), SEARCH).unwrap();
    git(root, &["init", "-q", "-b", "main"]);
    git(root, &["config", "user.email", "runner@test"]);
    git(root, &["config", "user.name", "Runner"]);
    git(root, &["add", "."]);
    git(root, &["commit", "-q", "-m", "init"]);
    dir
}

const GOOD: &str = r#"[pack]
name        = "scholarly"
version     = "0.1.2"
description = "Papers, by DOI and PMCID."

[scheme.doi]
pattern  = '^10\.(?P<registrant>\d{4,9})/\S+$'
type     = "paper"
resolve  = { template = "https://api.crossref.org/works/{id}", media = "application/json" }
describe = "transforms/crossref.glu"
then     = [{ scheme = "pmc", from = "describe.pmcid" }]

[scheme.pmc]
pattern = '^PMC\d+$'
type    = "paper"
resolve = { template = "https://www.ebi.ac.uk/europepmc/webservices/rest/{id}/fullTextXML" }

[transport]
contact      = "required"
min_interval = "100ms"
auth         = ["CROSSREF_TOKEN"]

[defaults]
ttl_days = 365

[search]
scheme   = "doi"
template = "https://api.crossref.org/works?query={query}&rows={limit}"
media    = "application/json"
items    = "message/items/*"
id       = "DOI"
title    = "title/0"
fixtures = { "retinal imaging" = "crossref-search.json" }

[fixtures]
"doi:10.1167/tvst.8.5.14" = "crossref-tvst.json"
"#;

const ENTRY: &str = "# scholarly\n\n## What it is\n\n<!-- prompt -->\n\n## What was read\n\n\
                     <!-- prompt -->\n";

/// A describe over a Crossref work: its first title, and its PMCID when it has one.
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

const SEARCH: &str = r#"{"message": {"items": [
    {"DOI": "10.1167/tvst.8.5.14", "title": ["Retinal Imaging"]},
    {"title": ["Listed without an identifier"]},
    {"DOI": "10.1000/xyz", "title": ["Another"]},
    {"DOI": "10.1000/abc"}
]}}"#;

const DOI: &str = "doi:10.1167/tvst.8.5.14";

// ── list ─────────────────────────────────────────────────────────────────────────────────

#[test]
fn list_names_each_need_and_whether_it_is_met_and_never_a_value() {
    let dir = corpus(GOOD);
    let bare = json(dir.path(), &["list"], &[]);
    let p = &bare["source_packs"][0];
    assert_eq!(p["name"], "scholarly");
    assert_eq!(p["search"], "doi");
    assert_eq!(p["needs"]["contact"], "required");
    assert_eq!(p["needs"]["contact_set"], false);
    assert_eq!(p["needs"]["auth"][0]["var"], "CROSSREF_TOKEN");
    assert_eq!(p["needs"]["auth"][0]["set"], false);
    assert_eq!(p["ready"], false);
    let schemes: Vec<&str> = p["schemes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["scheme"].as_str().unwrap())
        .collect();
    assert_eq!(schemes, ["doi", "pmc"]);
    assert_eq!(p["schemes"][0]["then"][0], "pmc");

    let env = [
        ("YIDAM_CONTACT", "me@example.org"),
        ("CROSSREF_TOKEN", "s3cret-token"),
    ];
    let set = json(dir.path(), &["list"], &env);
    assert_eq!(set["source_packs"][0]["ready"], true);
    let text = yidam(dir.path(), &["list"], &env);
    assert_eq!(text.code, 0, "{}", text.said);
    for (_, v) in env {
        assert!(
            !text.said.contains(v),
            "a value was printed:\n{}",
            text.said
        );
        assert!(!set.to_string().contains(v), "a value was reported: {set}");
    }
    assert!(text.said.contains("CROSSREF_TOKEN is set"), "{}", text.said);
}

#[test]
fn list_with_no_packs_says_so() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".yidam/decisions")).unwrap();
    let r = yidam(dir.path(), &["list"], &[]);
    assert_eq!(r.code, 0, "{}", r.said);
    assert!(r.said.contains("No enabled source packs"), "{}", r.said);
}

// ── search ───────────────────────────────────────────────────────────────────────────────

#[test]
fn search_offline_prints_the_identifiers_its_fixture_answers_and_writes_nothing() {
    let dir = corpus(GOOD);
    let before = git(dir.path(), &["status", "--porcelain"]);
    let r = json(
        dir.path(),
        &["search", "scholarly", "retinal imaging", "--offline"],
        &[],
    );
    let s = &r["search"];
    assert_eq!(s["answered"], "fixture");
    assert_eq!(
        s["url"],
        "https://api.crossref.org/works?query=retinal%20imaging&rows=10"
    );
    let ids: Vec<&str> = s["candidates"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["identifier"].as_str().unwrap())
        .collect();
    assert_eq!(ids, [DOI, "doi:10.1000/xyz", "doi:10.1000/abc"]);
    assert_eq!(s["candidates"][0]["title"], "Retinal Imaging");

    let two = json(
        dir.path(),
        &[
            "search",
            "scholarly",
            "retinal imaging",
            "--offline",
            "--limit",
            "2",
        ],
        &[],
    );
    assert_eq!(two["search"]["candidates"].as_array().unwrap().len(), 2);
    assert_eq!(git(dir.path(), &["status", "--porcelain"]), before);
}

#[test]
fn search_refuses_to_ask_a_publisher_whose_needs_are_unmet() {
    let dir = corpus(GOOD);
    let r = yidam(dir.path(), &["search", "scholarly", "anything"], &[]);
    assert_ne!(r.code, 0, "{}", r.said);
    assert!(r.said.contains("YIDAM_CONTACT"), "{}", r.said);
    assert!(r.said.contains("was not asked"), "{}", r.said);
}

#[test]
fn search_offline_names_the_queries_a_pack_recorded() {
    let dir = corpus(GOOD);
    let r = yidam(
        dir.path(),
        &["search", "scholarly", "glaucoma", "--offline"],
        &[],
    );
    assert_ne!(r.code, 0, "{}", r.said);
    assert!(r.said.contains("`retinal imaging`"), "{}", r.said);
}

#[test]
fn search_names_the_packs_there_are_and_a_pack_with_no_search() {
    let dir = corpus(GOOD);
    let r = yidam(dir.path(), &["search", "scolarly", "x", "--offline"], &[]);
    assert_ne!(r.code, 0, "{}", r.said);
    assert!(r.said.contains("scholarly"), "{}", r.said);

    let start = GOOD.find("[search]").unwrap();
    let end = GOOD.find("[fixtures]").unwrap();
    let without = format!("{}{}", &GOOD[..start], &GOOD[end..]);
    let dir = corpus(&without);
    let r = yidam(dir.path(), &["search", "scholarly", "x", "--offline"], &[]);
    assert_ne!(r.code, 0, "{}", r.said);
    assert!(r.said.contains("declares no [search]"), "{}", r.said);
}

// ── add ──────────────────────────────────────────────────────────────────────────────────

fn entry_path(report: &serde_json::Value) -> String {
    report["drafts"][0]["path"].as_str().unwrap().to_string()
}

#[test]
fn add_commits_one_draft_entry_obtained_false_and_nothing_else() {
    let dir = corpus(GOOD);
    let root = dir.path();
    let r = json(root, &["add", DOI, "--offline"], &[]);
    let rel = entry_path(&r);
    assert!(rel.starts_with(".yidam/catalog/"), "{rel}");
    assert_eq!(r["drafts"][0]["written"], true);

    assert_eq!(
        git(root, &["log", "-1", "--format=%s"]),
        format!(
            "catalog: {} from {DOI}",
            r["drafts"][0]["entry"].as_str().unwrap()
        )
    );
    assert_eq!(
        git(root, &["log", "-1", "--format=%an <%ae>"]),
        "yidam catalog <catalog@yidam>"
    );
    assert_eq!(
        git(root, &["show", "--name-only", "--format=", "HEAD"]),
        rel,
        "the commit touched something besides the entry"
    );
    assert_eq!(git(root, &["status", "--porcelain"]), "");

    let text = std::fs::read_to_string(root.join(&rel)).unwrap();
    assert!(text.contains("obtained: false\n"), "{text}");
    assert!(!text.contains("obtained: true"), "{text}");
    assert!(text.contains("type: paper\n"), "{text}");
    assert!(text.contains("ttl_days: 365\n"), "{text}");
    assert!(text.contains("  - kind: identifier\n"), "{text}");
    assert!(text.contains(&format!("value: {DOI}")), "{text}");
    assert!(text.contains("## What was read"), "{text}");
    assert!(
        !text.contains("# scholarly"),
        "the pack's own title was kept: {text}"
    );

    // The entry is one `catalog lint` reads as a catalog entry.
    let fm = text.split("---").nth(1).unwrap();
    let parsed: serde_yaml::Value = serde_yaml::from_str(fm).unwrap();
    assert_eq!(parsed["obtained"], serde_yaml::Value::Bool(false));
}

#[test]
fn add_refuses_an_identifier_already_catalogued() {
    let dir = corpus(GOOD);
    json(dir.path(), &["add", DOI, "--offline"], &[]);
    let head = git(dir.path(), &["rev-parse", "HEAD"]);
    let r = yidam(dir.path(), &["add", DOI, "--offline"], &[]);
    assert_ne!(r.code, 0, "{}", r.said);
    assert!(r.said.contains("already catalogued"), "{}", r.said);
    assert_eq!(git(dir.path(), &["rev-parse", "HEAD"]), head);
}

#[test]
fn add_writes_nothing_when_any_identifier_is_refused() {
    let dir = corpus(GOOD);
    let head = git(dir.path(), &["rev-parse", "HEAD"]);
    for bad in ["isbn:123", "doi:not-a-doi", "no-scheme"] {
        let r = yidam(dir.path(), &["add", DOI, bad, "--offline"], &[]);
        assert_ne!(r.code, 0, "{bad}: {}", r.said);
        assert!(r.said.contains("nothing was written"), "{bad}: {}", r.said);
        assert_eq!(git(dir.path(), &["rev-parse", "HEAD"]), head, "{bad}");
        assert_eq!(git(dir.path(), &["status", "--porcelain"]), "", "{bad}");
    }
}

#[test]
fn add_dry_run_reports_the_draft_and_writes_nothing() {
    let dir = corpus(GOOD);
    let head = git(dir.path(), &["rev-parse", "HEAD"]);
    let r = json(dir.path(), &["add", DOI, "--offline", "--dry-run"], &[]);
    assert_eq!(r["drafts"][0]["written"], false);
    assert!(r["drafts"][0]["commit"].is_null());
    assert_eq!(git(dir.path(), &["rev-parse", "HEAD"]), head);
    assert_eq!(git(dir.path(), &["status", "--porcelain"]), "");

    let both = yidam(dir.path(), &["add", DOI, "--fetch", "--dry-run"], &[]);
    assert_ne!(both.code, 0, "{}", both.said);
}

#[test]
fn add_gives_two_identifiers_two_entries_and_two_commits() {
    let dir = corpus(GOOD);
    let r = json(
        dir.path(),
        &["add", DOI, "doi:10.1000/xyz", "--offline"],
        &[],
    );
    let drafts = r["drafts"].as_array().unwrap();
    assert_eq!(drafts.len(), 2);
    assert_ne!(drafts[0]["path"], drafts[1]["path"]);
    assert_eq!(git(dir.path(), &["rev-list", "--count", "HEAD"]), "3");
}

/// Without transforms, nothing reads the answer, so the draft is the pack's templates and
/// the `then` it could not follow is named rather than dropped.
#[cfg(not(feature = "source-transforms"))]
#[test]
fn add_without_transforms_drafts_from_templates_and_names_what_it_did_not_follow() {
    let dir = corpus(GOOD);
    let r = json(dir.path(), &["add", DOI, "--offline"], &[]);
    let d = &r["drafts"][0];
    assert!(d["answered"].is_null(), "{d}");
    assert_eq!(d["locations"].as_array().unwrap().len(), 1, "{d}");
    let unfollowed = d["unfollowed"].to_string();
    assert!(unfollowed.contains("pmc"), "{unfollowed}");
}

/// With transforms, the describe names the title and the PMCID, and the PMCID becomes the
/// entry's second location.
#[cfg(feature = "source-transforms")]
#[test]
fn add_follows_then_and_titles_the_entry_from_the_describe() {
    let dir = corpus(GOOD);
    let r = json(dir.path(), &["add", DOI, "--offline"], &[]);
    let d = &r["drafts"][0];
    assert_eq!(d["answered"], "fixture");
    assert_eq!(d["entry"], "retinal-imaging");
    let ids: Vec<&str> = d["locations"]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| l["identifier"].as_str().unwrap())
        .collect();
    assert_eq!(ids, [DOI, "pmc:PMC6762077"]);
    let text = std::fs::read_to_string(dir.path().join(entry_path(&r))).unwrap();
    assert!(text.contains("# Retinal Imaging\n"), "{text}");
    assert!(text.contains("named by"), "{text}");
}

// ── the report contract ──────────────────────────────────────────────────────────────────

/// `report_goldens` reaches none of these three: its fixture holds no pack, and `add` commits.
/// So the paths each emits over a staged pack are held to report.schema.json here, both ways.
#[test]
fn every_emitted_field_is_declared_in_the_schema() {
    let schema: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(
            common::repo_root()
                .join("yidam/prelude/sdks/parity/fixtures/reports/report.schema.json"),
        )
        .unwrap(),
    )
    .unwrap();
    let dir = corpus(&GOOD.replace(
        "auth         = [\"CROSSREF_TOKEN\"]",
        "auth         = [\"CROSSREF_TOKEN\"]\nblocked      = \"terms forbid automated access\"",
    ));
    let reports = [
        ("source list", json(dir.path(), &["list"], &[])),
        (
            "source search",
            json(
                dir.path(),
                &["search", "scholarly", "retinal imaging", "--offline"],
                &[],
            ),
        ),
        (
            "source add",
            json(
                dir.path(),
                &["add", DOI, "doi:10.1000/xyz", "--offline"],
                &[],
            ),
        ),
    ];
    let mut emitted = std::collections::BTreeSet::new();
    for (command, report) in &reports {
        let mut mine = std::collections::BTreeSet::new();
        common::paths_of(report, "", &mut mine);
        for path in &mine {
            assert!(
                common::declares(&schema, path),
                "`{command}` emits `{path}`, which report.schema.json does not declare"
            );
        }
        emitted.extend(mine);
    }
    for key in ["source_packs", "drafts"] {
        let items = &schema["properties"][key]["items"]["properties"];
        for field in items.as_object().unwrap().keys() {
            let path = format!("{key}[].{field}");
            assert!(
                emitted.contains(&path),
                "the schema declares `{path}` and nothing emits it"
            );
        }
    }
    for field in schema["properties"]["search"]["properties"]
        .as_object()
        .unwrap()
        .keys()
    {
        let path = format!("search.{field}");
        assert!(
            emitted.contains(&path),
            "the schema declares `{path}` and nothing emits it"
        );
    }
}
