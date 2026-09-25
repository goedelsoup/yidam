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

/// A Pages deployment must be identified by more than the commit it was built from.
///
/// `actions/deploy-pages` sends `pages_build_version: process.env.GITHUB_SHA` and nothing
/// else. Pages treats that string as the deployment's id, and re-deploying an id already
/// live is accepted, acknowledged, and serves nothing new.
///
/// A release is precisely that case. The tag is cut on the current `main`, so the tag and
/// the push that became it carry the same sha — and main's build ran first, before the tag
/// existed, with the *previous* release resolved as latest. At `cli/v0.14.0` the tag's build
/// was correct (`v0.14 (latest)`, `./v0.14/` in the uploaded artifact), the deploy reported
/// success, and the site kept serving the earlier tree with `/yidam/v0.14/` a 404. The #535
/// trigger fired; its deploy was a no-op. `workflow_dispatch` — this file's documented way
/// to redeploy without a commit — has the same defect for the same reason.
///
/// The assembled tree is a function of the commit *and the tag set*, so the run is the
/// smallest thing that identifies it.
#[test]
fn the_pages_deployment_is_identified_by_more_than_the_commit() {
    let workflow: serde_yaml::Value =
        serde_yaml::from_str(&read(".github/workflows/docs.yml")).expect("docs.yml parses");
    let steps = workflow["jobs"]["deploy"]["steps"]
        .as_sequence()
        .expect("docs.yml has a deploy job with steps");
    let deploy = steps
        .iter()
        .find(|s| {
            s["uses"]
                .as_str()
                .is_some_and(|u| u.starts_with("actions/deploy-pages"))
        })
        .expect("docs.yml deploys with actions/deploy-pages");

    let version = deploy["env"]["GITHUB_SHA"].as_str().unwrap_or_else(|| {
        panic!(
            "the deploy step sends the bare commit as its build version. The action reads \
             GITHUB_SHA and exposes no input, so a tag that shares main's sha deploys an id \
             Pages already serves — accepted, and a no-op."
        )
    });
    assert!(
        version.contains("github.run_id") || version.contains("github.run_number"),
        "the deploy step's build version is `{version}`. It has to differ between two \
         deploys of the same commit — a release tag and the push it was cut from, or a \
         `workflow_dispatch` redeploy — and only the run distinguishes those."
    );
}

/// What the deploy claims and what the site serves are checked against each other.
///
/// The defect above was silent for exactly one reason: "Reported success!" describes the
/// API's answer to a request, not the bytes a reader receives. The assembly writes a stamp
/// into the tree it describes and a step after the deploy fetches it back, so the two are
/// separate claims that have to agree.
#[test]
fn the_deploy_is_checked_against_the_site_it_claims_to_have_published() {
    let text = read(".github/workflows/docs.yml");
    let workflow: serde_yaml::Value = serde_yaml::from_str(&text).expect("docs.yml parses");

    let scripts = |job: &str| -> Vec<String> {
        workflow["jobs"][job]["steps"]
            .as_sequence()
            .unwrap_or_else(|| panic!("docs.yml has a {job} job with steps"))
            .iter()
            .filter_map(|s| s["run"].as_str())
            .flat_map(|s| s.lines())
            .map(str::trim)
            .filter(|l| !l.starts_with('#'))
            .map(str::to_string)
            .collect()
    };

    let written = scripts("assemble");
    let stamp = written
        .iter()
        .find_map(|l| l.split_once("> site/").map(|(_, f)| f.trim().to_string()))
        .expect(
            "the assemble job writes no stamp into `site/`. Without one there is nothing a \
             reader can fetch that says which build the site is, and a deploy that changed \
             nothing looks exactly like one that worked",
        );

    // The *same step* that fetches the stamp has to be the one that fails, which is why
    // this reads whole scripts rather than the job's lines: an `exit 1` somewhere else in
    // the job would satisfy a flatter assertion while the stamp went unchecked.
    let checker = workflow["jobs"]["deploy"]["steps"]
        .as_sequence()
        .expect("docs.yml has a deploy job with steps")
        .iter()
        .filter_map(|s| s["run"].as_str())
        .find(|s| {
            s.lines()
                .map(str::trim)
                .any(|l| !l.starts_with('#') && l.contains(&stamp))
        })
        .unwrap_or_else(|| {
            panic!(
                "the assembly writes `site/{stamp}` and the deploy job never fetches it. A \
                 stamp nothing reads is a file, not a check"
            )
        });
    assert!(
        checker.contains("GITHUB_RUN_ID") && checker.contains("exit 1"),
        "the deploy job fetches `{stamp}` and does not fail when it names another run. \
         Serving the previous build is the failure; reporting it as success is what made it \
         cost a release"
    );
}

// ── the README and the site say the shared part once ──────────────────────────

/// The one passage the README and the site are meant to share, taken from the site's copy.
///
/// Extracted rather than written here, so this file is not a third place the text lives.
fn shared_passage() -> String {
    let page = read("docs/what-yidam-is.md");
    let start = page
        .find("Most knowledge systems keep the graph")
        .expect("docs/what-yidam-is.md no longer opens its argument with the database paragraph");
    let mut lines: Vec<&str> = Vec::new();
    let mut seen_table = false;
    for line in page[start..].lines() {
        // The passage is the paragraph plus the table that follows it, and it ends where the
        // table does.
        if line.trim_start().starts_with('|') {
            seen_table = true;
        } else if seen_table {
            break;
        }
        lines.push(line);
    }
    let passage = lines.join("\n");
    // Vacuity guard: this is the text every assertion below is written in terms of, and an
    // extractor that returned the paragraph alone would compare almost nothing.
    let rows = passage
        .lines()
        .filter(|l| l.trim_start().starts_with('|'))
        .count();
    assert!(
        rows >= 6 && passage.contains("| Git | Graph |"),
        "the extractor found {rows} table row(s) and no Git/Graph header; it is reading the \
         wrong part of the page and every comparison built on it is vacuous:\n{passage}"
    );
    passage
}

/// Every `.md` under `docs/`, by repo-relative path.
fn docs_pages() -> Vec<String> {
    fn walk(dir: &PathBuf, out: &mut Vec<PathBuf>) {
        for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(&p, out);
            } else if p.extension().is_some_and(|x| x == "md") {
                out.push(p);
            }
        }
    }
    let docs = repo_root().join("docs");
    let mut found = Vec::new();
    walk(&docs, &mut found);
    let root = repo_root().canonicalize().unwrap_or_else(|_| repo_root());
    let mut pages: Vec<String> = found
        .iter()
        .map(|p| {
            let c = p.canonicalize().unwrap_or_else(|_| p.clone());
            c.strip_prefix(&root)
                .unwrap_or(&c)
                .to_string_lossy()
                .to_string()
        })
        .collect();
    pages.sort();
    // Discovered, not listed — a page added under `docs/` is covered the day it lands, which a
    // roster cannot manage. Loud if the walk finds nothing, because an empty population is the
    // way this stops checking.
    assert!(
        pages.len() > 40,
        "found only {} page(s) under docs/; the walk is broken, not the directory",
        pages.len()
    );
    pages
}

/// The README repeats one passage from the site, character for character, and no other.
///
/// #947: the README reproduced the Git/Graph table and the paragraph introducing it from
/// `what-yidam-is.md`, and three further stretches — the install channels, the editor
/// walkthrough, the cargo feature table — restated `installation.md` and `editor-setup.md` in
/// their own words. Nothing held any of it, and the feature table is what that cost: it had
/// lost `vector-read` entirely and credited that feature's capability to `index`, while the
/// published table beside it was right.
///
/// So the pitch stays in both places — a README with no pitch is worse than a duplicated one —
/// and it is held identical to the site's copy, where the prose checks can see it. Everything
/// else is a link. The second half of this test is the part that keeps working after today: it
/// does not know which passages are duplicated, it finds them, so the next restatement to be
/// pasted in goes red without anyone remembering to add it here.
#[test]
fn the_readme_shares_one_passage_with_the_site_and_repeats_nothing_else() {
    let readme = read("README.md");
    let passage = shared_passage();

    assert!(
        readme.contains(&passage),
        "the README's copy of the Git/Graph passage has drifted from \
         `docs/what-yidam-is.md`.\n\nThe site's copy reads:\n\n{passage}\n\nEdit both or neither. \
         The site's copy is the one under the prose checks, so it is the one to change first."
    );

    // Every other run of four or more substantive lines the README shares with a published
    // page, found rather than listed.
    let rl: Vec<&str> = readme.lines().collect();
    let allowed: Vec<&str> = passage.lines().collect();
    let mut repeats: Vec<String> = Vec::new();

    for page in docs_pages() {
        let text = read(&page);
        let wl: Vec<&str> = text.lines().collect();
        for i in 0..rl.len() {
            for j in 0..wl.len() {
                if rl[i] != wl[j] {
                    continue;
                }
                // Only maximal runs: a run that could start one line earlier is reported there.
                if i > 0 && j > 0 && rl[i - 1] == wl[j - 1] {
                    continue;
                }
                let mut n = 0;
                while i + n < rl.len() && j + n < wl.len() && rl[i + n] == wl[j + n] {
                    n += 1;
                }
                let run = &rl[i..i + n];
                // Blank lines and a lone `|---|---|` are shape, not text; four lines of prose
                // is the threshold at which a restatement is a restatement.
                if run.iter().filter(|l| l.trim().len() > 12).count() < 4 {
                    continue;
                }
                if run.iter().all(|l| allowed.contains(l)) {
                    continue;
                }
                repeats.push(format!(
                    "  README:{} and {page}:{} share {n} lines, beginning: {}",
                    i + 1,
                    j + 1,
                    run.iter().find(|l| !l.trim().is_empty()).unwrap_or(&"")
                ));
            }
        }
    }

    assert!(
        repeats.is_empty(),
        "{} passage(s) in the README repeat a published page:\n{}\n\nThe README is not a copy \
         of the site. Where a subject has a page, say the one thing specific to the repository \
         and link out; the Git/Graph pitch is the single exception, and it is held identical \
         rather than merely similar.",
        repeats.len(),
        repeats.join("\n")
    );
}
