//! The docs site's URL is the one it is built for, and something publishes it.
//!
//! The site at `yidam/web/docs/` built for as long as it existed and was served nowhere.
//! Fixing that means two facts that live in different files and must agree: the URL the
//! README advertises, and the `site` + `base` Astro bakes into every generated link. Astro
//! does not know where GitHub Pages puts a repository, and GitHub Pages does not read
//! `astro.config.mjs`, so nothing but this file joins them.
//!
//! The failure it exists for is quiet in exactly the way this repository keeps finding: the
//! site would still build, still deploy, and serve a sidebar of links prefixed with the
//! wrong path — a 404 on every page from a green pipeline.

use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn read(rel: &str) -> String {
    let p = repo_root().join(rel);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{} is unreadable ({e})", p.display()))
}

/// Value of a `const NAME = '…';` declaration in the Astro config.
fn astro_const(name: &str) -> String {
    let config = read("yidam/web/docs/astro.config.mjs");
    let needle = format!("const {name} = '");
    let rest = config
        .split_once(&needle)
        .unwrap_or_else(|| panic!("astro.config.mjs declares no `const {name}`"))
        .1;
    rest.split_once('\'')
        .unwrap_or_else(|| panic!("`const {name}` in astro.config.mjs is unterminated"))
        .0
        .to_string()
}

/// The URL a reader is sent to. Trailing slash included: it is how the README writes it and
/// how Pages serves a project site.
fn published_url() -> String {
    format!(
        "{}{}/",
        astro_const("SITE"),
        astro_const("BASE").trim_end_matches('/')
    )
}

/// The README advertises the address the site is actually built for.
///
/// `base` is not cosmetic. Every route Starlight emits and every repository link the loader
/// rewrites is prefixed with it, so a `base` of `/yidam` and a README promising
/// `goedelsoup.github.io/docs` are not two spellings of one address — they are a working
/// site and a dead link, and only the second is visible in a diff.
#[test]
fn the_readme_advertises_the_url_the_site_is_built_for() {
    let url = published_url();
    let readme = read("README.md");

    assert!(
        readme.contains(&url),
        "README.md does not mention {url}, which is where astro.config.mjs's `site` + `base` \
         put this site. One of the two moved."
    );
}

/// A workflow deploys it, and deploys the assembled tree rather than one version of it.
///
/// The `path:` is asserted rather than merely the action, because `upload-pages-artifact`
/// succeeds on any directory it is pointed at. Aiming it one level up publishes the Astro
/// project — `package.json`, `src/`, `node_modules/` — as a static site, with no failure
/// anywhere in the run.
///
/// It used to be `yidam/web/docs/dist`, one build of `main`. Since #466 that directory is
/// one *version's* output and publishing it would put a single version at the site root with
/// every other version's path 404ing — while every page's switcher went on offering them.
#[test]
fn a_workflow_publishes_the_assembled_site() {
    let workflow = read(".github/workflows/docs.yml");

    assert!(
        workflow.contains("actions/deploy-pages@"),
        "docs.yml must deploy to Pages; without it the site builds in CI and reaches nobody, \
         which is the state this workflow was written to end"
    );
    assert!(
        workflow.contains("scripts/assemble.mjs"),
        "docs.yml no longer assembles the per-version builds into one tree. Whatever it \
         uploads is at most one version of a site whose every page links to the others."
    );
    assert!(
        workflow.contains("path: site"),
        "docs.yml must upload the directory `assemble.mjs` writes, not one version's `dist/`"
    );
    assert!(
        workflow.contains("name: site-${{ matrix.slot }}"),
        "each version's build must be kept under its own artifact name. Two builds sharing \
         one name is the root alias overwriting the version it copies, and the site root \
         then serving whichever job finished last."
    );
}

/// Value of an `export const NAME = '…';` in a JavaScript module under the docs site.
fn js_const(file: &str, name: &str) -> String {
    let text = read(&format!("yidam/web/docs/{file}"));
    let needle = format!("export const {name} = '");
    let rest = text
        .split_once(&needle)
        .unwrap_or_else(|| panic!("{file} declares no `export const {name}`"))
        .1;
    rest.split_once('\'')
        .unwrap_or_else(|| panic!("`{name}` in {file} is unterminated"))
        .0
        .to_string()
}

/// The subpath this site is served from is spelled once.
///
/// Two files know it since #466: `astro.config.mjs`, whose `base` is what a build with no
/// `--base` uses and therefore what a developer sees locally, and `src/versions.mjs`, whose
/// `ROOT` every published base is derived from — including the root alias the README's links
/// resolve to.
///
/// Letting them drift is the same defect this file has always been about, one level up: the
/// versions would build under one prefix and the site would be served from another, every
/// page would render, and every link between versions would 404. That is a working site and
/// a dead link, and only the second is visible in a diff.
#[test]
fn the_site_root_is_spelled_once() {
    let base = astro_const("BASE");
    let root = js_const("src/versions.mjs", "ROOT");
    assert_eq!(
        base, root,
        "astro.config.mjs builds for `{base}` and versions.mjs publishes under `{root}`. \
         Every version would be served from a prefix the site was not built for."
    );
}

/// The version list is filtered, never asked for.
///
/// "Which tag?" is the question that broke `curl | sh` in #397, when `releases/latest`
/// answered for a layer the caller did not mean. This repository has four release
/// namespaces and `releases/latest` currently resolves to a `cli/v*` tag by luck:
/// `editor/v0.2.0` is the second-newest release, and one editor release would make the
/// newest tag belong to a layer this site does not document.
///
/// Asserted as an absence, because that is the shape the mistake takes: a call that asks a
/// service which release is newest. What the filter *does* is graded by
/// `yidam/web/docs/test/versions.mjs`, which runs it against this repository's real tag list.
#[test]
fn the_version_list_is_filtered_rather_than_asked_for() {
    for file in ["src/versions.mjs", "scripts/versions.mjs"] {
        let text = read(&format!("yidam/web/docs/{file}"));
        // Comments discuss the trap by name; the code must not perform it.
        let code = text
            .lines()
            .filter(|l| !l.trim_start().starts_with("//") && !l.trim_start().starts_with('*'))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            !code.contains("releases/latest"),
            "{file} resolves a version by asking for the newest release. This repository \
             releases four layers into one tag namespace, and the newest release is \
             regularly not the CLI's."
        );
    }

    let source = read("yidam/web/docs/src/versions.mjs");
    assert!(
        source.contains("cli\\/v"),
        "src/versions.mjs no longer filters tags on the `cli/v` prefix, so `editor/v*`, \
         `sdk/rust/v*` and the unprefixed template tags are candidates to be published as \
         versions of these docs"
    );
}

/// The docs job's Node and the toolchain's Node are the same Node.
///
/// docs.yml is the one workflow here that does not start with `jdx/mise-action`, because
/// provisioning a Rust toolchain, protoc, Python and uv to run `astro build` is four
/// toolchains of waste. The cost of that choice is a second declaration of the Node version,
/// and this is the check that keeps the two from drifting into a site built against a
/// runtime nobody develops on.
#[test]
fn the_docs_workflow_and_the_toolchain_pin_the_same_node() {
    let mise = read("mise.toml");
    let workflow = read(".github/workflows/docs.yml");

    let pinned = mise
        .lines()
        .find_map(|l| l.trim().strip_prefix("node = "))
        .map(|v| v.trim().trim_matches('"').to_string())
        .expect("mise.toml declares no `node =` under [tools]");

    // mise spells the moving target `lts`; setup-node spells it `lts/*`. Any other value on
    // either side is a real pin and must be matched literally.
    let expected = if pinned == "lts" {
        "lts/*".to_string()
    } else {
        pinned.clone()
    };

    assert!(
        workflow.contains(&format!("node-version: {expected}")),
        "docs.yml requests a different Node than mise.toml pins ({pinned}); it must request \
         `{expected}`"
    );
}

/// The workflow that derives the version list runs when the tags it derives from change.
///
/// `scripts/versions.mjs` reads `git tag -l` and `src/versions.mjs` keeps the last three
/// minor series, so a `cli/v*` tag push is the only event that changes the published set. It
/// was also the only event `docs.yml` did not fire on, and nothing went red: the site served
/// a coherent *previous* release until an unrelated push to main happened along (#535).
///
/// **Both sides are discovered.** The glob comes out of the workflow's `tags:` line and the
/// prefix out of `src/versions.mjs`'s own filter — the one the test above already requires to
/// exist. Hardcoding `cli/v` here would let the two drift: rename the tag namespace and the
/// version list would follow it while the trigger kept watching the old one, which is this
/// bug again with the halves swapped.
#[test]
fn the_docs_workflow_fires_on_the_tags_its_version_list_is_built_from() {
    let workflow = read(".github/workflows/docs.yml");

    // The `tags:` entry under `on: push:`, not one in a job's script or a comment.
    let globs = workflow
        .lines()
        .map(str::trim_start)
        .filter(|l| !l.starts_with('#'))
        .find_map(|l| l.strip_prefix("tags:"))
        .map(|v| {
            v.trim()
                .trim_start_matches('[')
                .trim_end_matches(']')
                .split(',')
                .map(|g| g.trim().trim_matches(['\'', '"']).to_string())
                .collect::<Vec<_>>()
        })
        .unwrap_or_else(|| {
            panic!(
                "docs.yml has no `tags:` filter, so pushing a release tag does not rebuild \
                 the site. The version list is read from `git tag -l`; the workflow that \
                 reads it must run when that answer changes."
            )
        });

    // The prefix the version list itself filters on, spelled as the regex escapes it.
    let source = read("yidam/web/docs/src/versions.mjs");
    let prefix = source
        .split_once("^cli\\/v")
        .map(|_| "cli/v")
        .expect("src/versions.mjs no longer filters tags with `^cli\\/v`");

    assert!(
        globs.iter().any(|g| g.starts_with(prefix)),
        "docs.yml fires on {globs:?}, but the version list is built from tags matching \
         `{prefix}*`. A trigger that does not cover the prefix the list filters on leaves \
         the published set stale from the tag until an unrelated push to main."
    );
    assert!(
        !globs.iter().any(|g| g == "v*" || g == "*"),
        "docs.yml fires on {globs:?}. Four layers release into one tag namespace and only \
         `{prefix}*` changes this list, so a broader glob is a deploy per template, editor \
         and SDK tag that rewrites the site with identical content."
    );
}
