//! The quality pages exist, are reachable, and read the contract they were built for.
//!
//! `/quality/` is a segment of the docs deployment that Starlight does not know about. Every
//! guard the site already has is a Starlight guard: `docs-source.ts` enumerates the content
//! collection, `astro.config.mjs` refuses a page the sidebar does not name, and
//! `rewrite-repo-links.ts` fails the build on a dead relative link. None of them look at
//! `src/pages/`, so a route added there is exactly the thing this repository keeps finding —
//! rendered, routed, and reachable by nobody who did not already know the URL.
//!
//! Both sides are discovered. The routes come from the filesystem, the navigation from the
//! layout, and the version this site reads from the constant it declares. A roster here would
//! stop covering the next page without ever going red.

use std::collections::BTreeSet;
use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn read(rel: &str) -> String {
    let p = repo_root().join(rel);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{} is unreadable ({e})", p.display()))
}

const SITE: &str = "yidam/web/docs";
const PAGES: &str = "yidam/web/docs/src/pages/quality";

/// Text with `//` comments removed, so prose about the code cannot answer for the code.
///
/// The mistake this file would otherwise repeat is #461's: a guard scanning for a step stayed
/// green after the step was deleted, because the comment above it said the words. Every
/// assertion below reads this rather than the file.
fn code_only(text: &str) -> String {
    text.lines()
        .map(|line| match line.split_once("//") {
            Some((before, _)) if !before.ends_with(':') => before,
            _ => line,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// The routes the surface actually builds, from the filesystem.
///
/// `index.astro` is `/quality/`; `tests.astro` is `/quality/tests/`. Astro's own mapping,
/// read rather than restated.
fn routes() -> BTreeSet<String> {
    let dir = repo_root().join(PAGES);
    let entries =
        std::fs::read_dir(&dir).unwrap_or_else(|e| panic!("{} is unreadable ({e})", dir.display()));
    let out: BTreeSet<String> = entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "astro"))
        .map(|p| {
            let stem = p.file_stem().unwrap().to_string_lossy().to_string();
            if stem == "index" {
                "/quality/".to_string()
            } else {
                format!("/quality/{stem}/")
            }
        })
        .collect();
    assert!(
        out.len() >= 2,
        "only {out:?} under {PAGES}; the walk is looking at the wrong tree and every \
         assertion below would be vacuous"
    );
    out
}

/// The routes the layout's own navigation offers, as `${base}`-relative paths.
fn navigation() -> BTreeSet<String> {
    let layout = code_only(&read(&format!("{SITE}/src/layouts/Quality.astro")));
    let nav = layout
        .split("const NAV = [")
        .nth(1)
        .expect("the quality layout declares no NAV")
        .split(']')
        .next()
        .expect("NAV is unterminated");
    nav.split("${base}")
        .skip(1)
        .filter_map(|rest| rest.split('`').next())
        .map(str::to_string)
        .collect()
}

/// Every page is in the navigation, and every navigation entry is a page.
///
/// It runs in both directions for the reason the sidebar gate does: an unlisted page is not
/// published, and an entry naming a page that does not exist is a dead link whose error
/// arrives later and says less. This surface gets neither error from Astro — `src/pages/`
/// takes whatever is in it.
#[test]
fn every_quality_route_is_reachable_from_the_surfaces_own_navigation() {
    let routes = routes();
    let nav = navigation();

    let unlisted: Vec<&String> = routes.difference(&nav).collect();
    assert!(
        unlisted.is_empty(),
        "these routes build and nothing links to them: {unlisted:?}. Starlight's sidebar gate \
         does not see `src/pages/`, so an unreachable page here fails no build."
    );

    let dead: Vec<&String> = nav.difference(&routes).collect();
    assert!(
        dead.is_empty(),
        "the navigation offers {dead:?}, which no page under {PAGES} builds — a 404 from \
         inside the surface's own header"
    );
}

/// The docs sidebar reaches the surface, and reaches it through `BASE`.
///
/// A literal `/yidam/quality/` beside a `base` that moved is a working site and a dead link,
/// and only the second is visible in a diff — `docs_site.rs`'s own words, applied to the one
/// link on this site that Starlight does not generate.
#[test]
fn the_docs_sidebar_links_to_the_quality_surface() {
    let config = code_only(&read(&format!("{SITE}/astro.config.mjs")));
    let quality_group = config
        .split("label: 'Quality'")
        .nth(1)
        .expect("the sidebar has no Quality group")
        .split("},\n  {")
        .next()
        .unwrap_or_default();
    assert!(
        quality_group.contains("${BASE}/quality/"),
        "the Quality sidebar group does not link to the measurements, or links to them with \
         a literal path rather than through BASE:\n{quality_group}"
    );
}

/// The site reads the contract version the CLI writes.
///
/// The two are declared in different languages in different workspaces, and a mismatch has
/// one visible consequence: `loadReport` refuses the document and every page renders "not
/// measured" — a silence that looks exactly like a CI run that did not publish.
#[test]
fn the_site_and_the_cli_agree_about_the_contract_version() {
    let declared = |text: &str, needle: &str| -> String {
        text.split(needle)
            .nth(1)
            .unwrap_or_else(|| panic!("no {needle} found"))
            .split(['\'', '"'])
            .nth(1)
            .expect("unterminated")
            .to_string()
    };
    let site = declared(
        &code_only(&read(&format!("{SITE}/src/quality/report.ts"))),
        "KNOWN_FORMAT_VERSION =",
    );
    let cli = declared(&read("yidam/cli/src/report.rs"), "FORMAT_VERSION: &str =");
    assert_eq!(
        site, cli,
        "the docs site reads format_version {site} and the CLI writes {cli}. Every quality \
         page would render as though no report had been published."
    );
}

/// The report the site is built against carries the envelope the contract declares.
///
/// Read out of `report.schema.json` rather than restated: the envelope is defined once, and
/// #467's instruction was "not a new envelope — **this** one". A comment saying so would not
/// notice a field being added to the schema and not to the reporter.
#[test]
fn the_quality_report_carries_the_repositorys_envelope() {
    let schema: serde_json::Value = serde_json::from_str(&read(
        "yidam/prelude/sdks/parity/fixtures/reports/report.schema.json",
    ))
    .expect("report.schema.json parses");
    let golden: serde_json::Value = serde_json::from_str(&read(
        "yidam/tests/harness/ci-report/tests/goldens/quality-report.json",
    ))
    .expect("the quality golden parses");

    // The envelope half of the contract, extracted rather than retyped.
    let required: Vec<&str> = schema["required"]
        .as_array()
        .expect("the schema declares no required fields")
        .iter()
        .filter_map(|v| v.as_str())
        .collect();
    assert!(
        required.contains(&"format_version") && required.contains(&"yidam"),
        "report.schema.json no longer requires the envelope: {required:?}"
    );

    let envelope = serde_json::json!({
        "type": "object",
        "required": required,
        "properties": {
            "format_version": schema["properties"]["format_version"],
            "yidam": schema["properties"]["yidam"],
            "root": schema["properties"]["root"],
        },
    });
    let validator = jsonschema::validator_for(&envelope).expect("the envelope sub-schema compiles");
    let errors: Vec<String> = validator
        .iter_errors(&golden)
        .map(|e| e.to_string())
        .collect();
    assert!(
        errors.is_empty(),
        "the quality report does not satisfy the envelope every other report here uses. A \
         consumer that reads a yidam report would have to learn a second shape:\n  {}",
        errors.join("\n  ")
    );
}

/// Something builds the pages and reads them back, and it is reachable both ways.
///
/// Three names for one check — an npm script, a mise task, and a workflow step — because
/// docs.yml carries no mise and a contributor's terminal has no workflow. Any one of them
/// going missing leaves the render assertions runnable and unrun, which is
/// `design_lint.rs`'s finding in #465: a config nothing invoked.
#[test]
fn the_render_assertions_are_invoked_from_a_task_and_from_a_workflow() {
    let script = "test/quality-render.mjs";
    let package: serde_json::Value =
        serde_json::from_str(&read(&format!("{SITE}/package.json"))).expect("package.json parses");
    let npm_test = package["scripts"]["test"].as_str().unwrap_or_default();
    assert!(
        npm_test.contains(script),
        "package.json's `test` script does not run {script} (it runs {npm_test:?})"
    );

    // Comments stripped from the TOML too: the task's own note explains what it does, and a
    // check satisfied by that note is the mistake #461 found and #465 found again.
    let mise: String = read("mise.toml")
        .lines()
        .map(|l| l.split('#').next().unwrap_or(""))
        .collect::<Vec<_>>()
        .join("\n");
    let task = mise
        .split("[tasks.docs-test]")
        .nth(1)
        .expect("mise.toml declares no `docs-test` task")
        .split("\n[tasks.")
        .next()
        .unwrap_or_default();
    assert!(
        task.contains("npm run") && task.contains("test"),
        "the `docs-test` task no longer runs the site's own test script:\n{task}"
    );

    let docs_yml: String = read(".github/workflows/docs.yml")
        .lines()
        .map(|l| l.split('#').next().unwrap_or(""))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        docs_yml.contains("npm test"),
        "no step in docs.yml runs the render assertions, so a template that draws a \
         fully-skipped suite green would deploy"
    );
    assert!(
        docs_yml.contains("YIDAM_QUALITY_REPORT"),
        "docs.yml never passes a report to the build, so every published quality page would \
         render the `not measured` state"
    );
}

/// The pages refuse to render a report they cannot vouch for, rather than drawing zeroes.
///
/// The assertion is about `report.ts` having a refusal at all — the render assertions in
/// `test/quality-render.mjs` are what prove the pages then say so. Here because the two live
/// in different workspaces and only one of them is in this gate.
#[test]
fn an_unreadable_report_is_refused_rather_than_partly_rendered() {
    let reader = code_only(&read(&format!("{SITE}/src/quality/report.ts")));
    for needle in ["format_version !==", "gates?.length", "problem"] {
        assert!(
            reader.contains(needle),
            "report.ts no longer refuses a document it cannot read ({needle} is gone). A page \
             built from a half-understood report draws bars out of fields whose meaning may \
             have changed."
        );
    }
}
