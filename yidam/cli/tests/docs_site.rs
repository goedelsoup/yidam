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

/// A step's `run:` script with its comment lines removed.
///
/// `docs.yml` argues about `pages_build_version` at length in prose, so a check that
/// grepped the file would be answered by the argument rather than by the request.
fn script_of(step: &serde_yaml::Value) -> String {
    step["run"]
        .as_str()
        .unwrap_or_else(|| {
            panic!(
                "the step named `{}` runs nothing",
                step["name"].as_str().unwrap_or("?")
            )
        })
        .lines()
        .filter(|l| !l.trim_start().starts_with('#'))
        .collect::<Vec<_>>()
        .join("\n")
}

/// The step that publishes — the one `environment.url` reads the site's URL from.
///
/// Found by that id rather than by what it runs, so the assertions below can ask what it
/// does without having already assumed the answer.
fn deploy_step() -> serde_yaml::Value {
    let workflow: serde_yaml::Value =
        serde_yaml::from_str(&read(".github/workflows/docs.yml")).expect("docs.yml parses");
    workflow["jobs"]["deploy"]["steps"]
        .as_sequence()
        .expect("docs.yml has a deploy job with steps")
        .iter()
        .find(|s| s["id"].as_str() == Some("deployment"))
        .expect(
            "docs.yml's deploy job has no step with `id: deployment`. That id is what the \
             job's `environment.url` and the check after it read the page URL from.",
        )
        .clone()
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
        script_of(&deploy_step())
            .lines()
            .any(|l| l.contains("POST") && l.contains("pages/deployments")),
        "docs.yml's deploy step creates no Pages deployment — nothing in it POSTs to \
         `pages/deployments`. Without that request the site builds in CI and reaches \
         nobody, which is the state this workflow was written to end"
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

/// A Pages deployment must be identified by a commit Pages has never deployed.
///
/// A deployment is identified by `pages_build_version` and nothing else. Pages treats that
/// string as the deployment's id, and re-deploying an id already live is accepted,
/// acknowledged, and serves nothing new.
///
/// A release is precisely that case. The tag is cut on the current `main`, so the tag and
/// the push that became it carry the same sha — and main's build ran first, before the tag
/// existed, with the *previous* release resolved as latest. At `cli/v0.14.0` the tag's build
/// was correct (`v0.14 (latest)`, `./v0.14/` in the uploaded artifact), the deploy reported
/// success, and the site kept serving the earlier tree with `/yidam/v0.14/` a 404. The #535
/// trigger fired; its deploy was a no-op. `workflow_dispatch` — `docs.yml`'s documented way
/// to redeploy without a commit — has the same defect for the same reason.
///
/// Two routes to a run-scoped id have failed. An override of `GITHUB_SHA` **never reached
/// the action** (#992): the runner overwrites every name the `github` context carries before
/// a step runs, and the log echoes the declaration rather than the process environment. Then
/// `<sha>-<run id>` sent as an argument reached Pages and was refused: at `cli/v0.16.0` Pages
/// 404'd it and accepted the bare commit with the same artifact id. **A build version must be
/// a real commit.** So the step writes one per run — this run's tree, this run's commit as
/// parent, a message naming the run — and sends its sha.
///
/// This test is written against the request for that reason, and it pins the dead route
/// shut at the end.
#[test]
fn the_pages_deployment_is_identified_by_a_commit_written_for_the_run() {
    let script = script_of(&deploy_step());

    assert!(
        script.contains("pages_build_version: $pages_build_version"),
        "the deploy step's payload does not fill `pages_build_version` from the argument \
         resolved below, so what this test grades is not what Pages is sent"
    );

    // Resolved rather than assumed, and resolved through two indirections: the payload builder
    // fills the jq argument from its own parameter, and `deploy` passes the payload builder its
    // own. Which value Pages is sent is decided by the calls to `deploy` and not by the line
    // that names the field. A check that found `github.run_id` somewhere in the step would
    // pass on a run id the request never carries — which is how the `GITHUB_SHA` override
    // passed for six weeks.
    assert!(
        script.contains(r#"--arg pages_build_version "$1""#),
        "the deploy step's payload no longer takes its build version as its first argument, \
         so the attempts read below are not what fills `pages_build_version`."
    );
    assert!(
        script.contains(r#"payload "$1" |"#),
        "`deploy` no longer hands the payload builder its own first argument, so the calls to \
         `deploy` read below are not what fills `pages_build_version`."
    );
    let attempts: Vec<String> = script
        .lines()
        .filter_map(|l| l.split_once("deploy \""))
        .map(|(_, rest)| {
            rest.split('"')
                .next()
                .expect("split always yields a first field")
                .trim()
                .trim_start_matches('$')
                .trim_matches(|c| c == '{' || c == '}')
                .to_string()
        })
        .collect();
    let (first, fallbacks) = attempts.split_first().unwrap_or_else(|| {
        panic!(
            "the deploy step never calls `deploy`. It has to send a build version: the \
             default is the bare commit, and a tag shares its commit with the push it was \
             cut from."
        )
    });

    // The first attempt's value is a commit this step writes, and nothing else. Found by its
    // assignment, and the assignment graded on the request it reads: a POST to the Git Data
    // API's commits endpoint, parented on this run's commit, whose `.sha` is the value.
    let assignment = script
        .lines()
        .map(str::trim)
        .find(|l| l.starts_with(&format!("{first}=")))
        .unwrap_or_else(|| {
            panic!(
                "the deploy step's first attempt sends `${first}`, which the step never \
                 assigns. Whatever the request carries, it is not this."
            )
        });
    let written = script
        .split_once(assignment)
        .expect("the assignment was found in this script")
        .1
        .split("\n\n")
        .next()
        .unwrap_or_default();
    let written = format!("{assignment}\n{written}");
    for (needle, why) in [
        (
            "/git/commits\"",
            "is not a commit this step writes. Pages refuses a build version that is not a \
             real commit (it 404'd `<sha>-<run id>` at `cli/v0.16.0`), and a real commit that \
             is not new is the #992 no-op.",
        ),
        (
            "-X POST",
            "reads a commit rather than writing one. Every commit that already exists is one \
             Pages may already have deployed.",
        ),
        (
            "parents: [$parent]",
            "writes a commit with no parent, so nothing ties the deployment's id to the \
             commit it was built from.",
        ),
        (
            "--jq '.sha'",
            "does not take the written commit's sha, so what Pages is sent is not the commit \
             this step wrote.",
        ),
    ] {
        assert!(
            written.contains(needle),
            "the deploy step's first build version `${first}` {why} Assigned by:\n{written}"
        );
    }
    assert!(
        script.contains(r#"--arg parent "$GITHUB_SHA""#),
        "the commit the deploy step writes is not parented on `$GITHUB_SHA`, the commit this \
         run built."
    );
    // The message line itself, not the script: the artifact listing names the run too, so a
    // check over the whole step is satisfied by a request that has nothing to do with this.
    let message = written
        .lines()
        .map(str::trim)
        .find(|l| l.starts_with("--arg message "))
        .unwrap_or_else(|| {
            panic!("the commit the deploy step writes takes no `--arg message`:\n{written}")
        });
    assert!(
        message.contains("${GITHUB_RUN_ID}"),
        "the commit the deploy step writes does not name the run ({message}). Two runs on one \
         commit — a release tag and the push it was cut from — must write two different \
         commits."
    );

    // Writing a commit object takes `contents: write`, and the job is the scope for it: a
    // job-level block replaces the workflow's, so the others must be restated too, or the
    // deploy loses the OIDC token and the Pages write it was already making.
    let workflow: serde_yaml::Value =
        serde_yaml::from_str(&read(".github/workflows/docs.yml")).expect("docs.yml parses");
    let permissions = &workflow["jobs"]["deploy"]["permissions"];
    for (scope, level) in [
        ("contents", "write"),
        ("pages", "write"),
        ("id-token", "write"),
        ("actions", "read"),
    ] {
        assert_eq!(
            permissions[scope].as_str(),
            Some(level),
            "the deploy job does not grant `{scope}: {level}`. It writes a commit object \
             (contents), mints an OIDC token (id-token), lists this run's artifact (actions) \
             and creates the deployment (pages); a job-level block replaces the workflow's, \
             so each must be stated here."
        );
    }
    assert_eq!(
        workflow["permissions"]["contents"].as_str(),
        Some("read"),
        "docs.yml grants `contents` beyond `read` at the workflow level. Only the deploy job \
         writes, and only a commit object; the build jobs run the site's npm dependencies."
    );

    // A second attempt is allowed, and only a second: the bare commit, announced. It is what
    // a push to main needs if the written commit is refused — main's commit is always new —
    // and on a tag it is the #992 no-op, which the check after this step reports.
    assert!(
        fallbacks.len() <= 1,
        "the deploy step attempts the deployment {} times: {attempts:?}. One fallback is a \
         safety net; more is a step guessing.",
        attempts.len()
    );
    if let Some(fallback) = fallbacks.first() {
        assert_eq!(
            fallback, "GITHUB_SHA",
            "the deploy step falls back to `${fallback}`. The only other value Pages is known \
             to accept is the bare commit."
        );
        // Graded on what the warning says, phrase by phrase, rather than on there being one.
        // The step prints several `::warning::` lines and any single one of them satisfies a
        // check for the marker, so deleting the line that says what went wrong would pass a
        // step that had stopped reporting it.
        let warned = script
            .lines()
            .filter(|l| l.contains("::warning::"))
            .collect::<Vec<_>>()
            .join(" ");
        for phrase in ["did not take", "publishes nothing"] {
            assert!(
                warned.contains(phrase),
                "the deploy step falls back to the bare commit and its warning never says \
                 `{phrase}`. The fallback reintroduces #992 — two deploys of one commit, the \
                 second accepted and publishing nothing — so the warning has to say both what \
                 Pages would not take and what accepting the commit costs. Warned: {warned:?}"
            );
        }
    }

    // The route that looked like it worked, held shut. It is the obvious thing to reach for
    // again, the runner accepts it without complaint, and the log prints the declaration
    // back as though it had taken effect.
    let workflow: serde_yaml::Value =
        serde_yaml::from_str(&read(".github/workflows/docs.yml")).expect("docs.yml parses");
    for (job, body) in workflow["jobs"].as_mapping().expect("docs.yml has jobs") {
        let job = job.as_str().unwrap_or("?");
        for step in body["steps"].as_sequence().into_iter().flatten() {
            assert!(
                step["env"]["GITHUB_SHA"].is_null(),
                "a step in the `{job}` job overrides GITHUB_SHA. The runner replaces every \
                 `GITHUB_*` name from the github context before running a step, so the \
                 override reaches nothing and the `env:` line the log echoes back is the \
                 declaration rather than the process environment (#992)."
            );
        }
    }
}

/// A request that fails anonymously cannot be diagnosed from the log it failed in.
///
/// The deploy step makes three requests and, before #1085, printed nothing before the first
/// of them — so twenty-five consecutive failures carried one line, `gh: Not Found (HTTP
/// 404)`, belonging to an unidentified one of the three while the site went a day without
/// publishing. What this grades is not that the step logs *something*. It is that no request
/// can fail without naming itself, which holds only while every call goes through one
/// labelled call site.
///
/// `script_of` strips the comments first, so the argument this workflow makes about its own
/// requests cannot answer for the requests.
#[test]
fn no_request_the_deploy_makes_can_fail_anonymously() {
    let script = script_of(&deploy_step());

    // One call site, and it is the helper's. A second bare `gh api` is a second anonymous
    // 404: `gh` writes its diagnosis to stderr and exits 1, and `set -e` then ends the step
    // with that message detached from the call that produced it.
    let calls: Vec<&str> = script
        .lines()
        .map(str::trim)
        .filter(|l| l.contains("gh api"))
        .collect();
    assert_eq!(
        calls,
        vec![r#"if ! body=$(gh api "$@" 2>&1); then"#],
        "the deploy step's `gh api` calls are not the single labelled one. Every extra call \
         site is a request that can fail with a message the log cannot attribute to it."
    );

    // Captured, and then printed where a reader is. `2>&1` above puts the API's own answer
    // in `body` — which is where it says *which* resource it could not find; `>&2` puts it
    // in the log rather than in the caller's `$(…)`, which would capture it as data.
    for fragment in [
        r#"echo "::error::${label} failed""#,
        r#"echo "${body}""#,
        "} >&2",
        "return 1",
    ] {
        assert!(
            script.contains(fragment),
            "the deploy step's request helper is missing `{fragment}`. Without it a failed \
             request says nothing, says it where nobody reads, or is swallowed and the step \
             continues with an empty body."
        );
    }

    // Every request, named. A count rather than a floor: the defect is a *new* request added
    // without a label, and a floor of three passes that.
    let labels: Vec<&str> = script
        .lines()
        .filter_map(|l| l.split_once(r#"request ""#))
        .filter_map(|(_, rest)| rest.split_once('"'))
        .map(|(label, _)| label)
        .collect();
    assert_eq!(
        labels.len(),
        5,
        "the deploy step makes {} labelled requests, not the five it is written around (the \
         artifact, this run's commit, the commit written as the build version, the \
         deployment — one call site for both attempts — and the poll). A request added \
         without a label fails the way #1085 did; one removed leaves this count lying. \
         Found: {labels:?}",
        labels.len()
    );
    for label in &labels {
        assert!(
            label.split_whitespace().count() > 2,
            "`{label}` does not name a request well enough to find it in a step that makes \
             five. The label is the whole of what the log will carry."
        );
    }

    // The one request that is not `gh api`. Its two variables exist only in a job granted
    // `id-token: write`; read blind under `set -u`, a job without it ends on a variable
    // name instead of on the reason, and that reason is a one-line fix in this file.
    for fragment in [
        "${ACTIONS_ID_TOKEN_REQUEST_URL:-}",
        "${ACTIONS_ID_TOKEN_REQUEST_TOKEN:-}",
    ] {
        assert!(
            script.contains(fragment),
            "the deploy step reads `{fragment}` without checking it. The OIDC request is \
             the one call here that is not `gh api`, and it is the one that stops existing \
             when a permission is dropped."
        );
    }
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
        .find(|s| {
            s["run"].as_str().is_some_and(|r| {
                r.lines()
                    .map(str::trim)
                    .any(|l| !l.starts_with('#') && l.contains(&stamp))
            })
        })
        .unwrap_or_else(|| {
            panic!(
                "the assembly writes `site/{stamp}` and the deploy job never fetches it. A \
                 stamp nothing reads is a file, not a check"
            )
        });
    let script = checker["run"].as_str().unwrap_or_default();
    assert!(
        script.contains("GITHUB_RUN_ID") && script.contains("exit 1"),
        "the deploy job fetches `{stamp}` and does not fail when it names another run. \
         Serving the previous build is the failure; reporting it as success is what made it \
         cost a release"
    );

    // Which events it runs on is the other half. A push to main always carries a sha Pages
    // has not seen, so it cannot collide and the check is not worth its thirteen minutes
    // there; the two events that *can* collide are a release tag and a `workflow_dispatch`,
    // which redeploys a commit that is live by definition.
    //
    // The dispatch is also the only rehearsal there is. A tag cannot be cut to test a
    // deploy, and a green run on main proves nothing about the tag path — so a change to
    // the deploy step is verifiable before a release only if this check is armed on the
    // event a maintainer can fire on demand.
    let when = checker["if"].as_str().unwrap_or_default();
    for event in ["tag", "workflow_dispatch"] {
        assert!(
            when.contains(event),
            "the check that the site serves this build runs `if: {when}`, which does not \
             cover `{event}`. Both events deploy a commit Pages may already be serving, and \
             the dispatch is the only one that can be fired without cutting a release."
        );
    }
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
