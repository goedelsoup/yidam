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

/// A workflow deploys it, and deploys the directory the build writes.
///
/// The `path:` is asserted rather than merely the action, because
/// `upload-pages-artifact` succeeds on any directory it is pointed at. Aiming it one level
/// up publishes the Astro project — `package.json`, `src/`, `node_modules/` — as a static
/// site, with no failure anywhere in the run.
#[test]
fn a_workflow_publishes_the_built_site() {
    let workflow = read(".github/workflows/docs.yml");

    assert!(
        workflow.contains("actions/deploy-pages@"),
        "docs.yml must deploy to Pages; without it the site builds in CI and reaches nobody, \
         which is the state this workflow was written to end"
    );
    assert!(
        workflow.contains("path: yidam/web/docs/dist"),
        "docs.yml must upload yidam/web/docs/dist — the directory `astro build` writes"
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
