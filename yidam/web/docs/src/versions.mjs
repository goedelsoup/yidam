// Which versions of this site exist, and which tag each one is built from.
//
// Four layers release independently (VERSIONING.md) and the site documents Layer 4, the
// tooling, because that is what a reader has installed. So the version list is derived from
// `cli/v*` tags and from nothing else.
//
// # The trap this module exists to not step in
//
// "Which tag?" has been asked by `install.sh`, `[yidam-build]`, the tap and
// `install-channels.yml`, and it broke `curl | sh` once already (#397) when `releases/latest`
// answered for the wrong layer. It is answered here by *filtering*, never by asking a service
// for "the newest release": `releases/latest` currently resolves to `cli/v0.7.0` and that is
// luck — `editor/v0.2.0` is the second-newest release, and one editor release would make the
// newest tag belong to a layer this site does not document.
//
// # Everything is an argument
//
// `resolve` takes tags and yank state rather than fetching them. The fetching lives in
// `collect`, which the workflow calls; the deciding lives here, where a test can reach it
// without a network or a git checkout.

/** How many minor series keep a published copy of the docs. */
export const KEEP = 3;

/**
 * The Pages subpath this repository's site is served from.
 *
 * Every base is derived from it, so the one place that knows the deployment lives here
 * rather than being spelled separately in the workflow, the config and the tests — which is
 * the shape of the three-copies-of-the-palette problem #465 spent a phase undoing.
 */
export const ROOT = '/yidam';

/**
 * `cli/v<major>.<minor>.<patch>` only, newest first.
 *
 * Everything else in the tag namespace is another layer — `sdk/rust/v*`, `editor/v*`,
 * `bootstrap/*` — plus the three unprefixed `v0.1.0`-era template tags that predate the
 * namespacing. A prefix match on `v` would take all of them.
 */
export function cliTags(tags) {
  const parsed = [];
  for (const tag of tags) {
    const m = /^cli\/v(\d+)\.(\d+)\.(\d+)$/.exec(tag.trim());
    if (!m) continue;
    parsed.push({
      tag: tag.trim(),
      major: Number(m[1]),
      minor: Number(m[2]),
      patch: Number(m[3]),
      version: `${m[1]}.${m[2]}.${m[3]}`,
    });
  }
  parsed.sort((a, b) => b.major - a.major || b.minor - a.minor || b.patch - a.patch);
  return parsed;
}

/**
 * The site's versions: `main`, then a series per published minor, newest first.
 *
 * # Why series rather than tags
 *
 * #466 says "the last 3 released `cli/v*` tags" and writes the URL as `/yidam/v0.2/`. Those
 * two disagree the moment a patch ships: `cli/v0.7.1` and `cli/v0.7.0` are two tags and one
 * `/yidam/v0.7/`, and a matrix built from tags would have two jobs writing the same path.
 *
 * Series wins because the URL is the thing a reader keeps. `/yidam/v0.7/` means "the 0.7
 * docs" for as long as 0.7 gets patches, instead of dying at each one. `KEEP` therefore
 * counts series, and each is built from its newest patch.
 *
 * # Yanked versions are dropped
 *
 * Decided for #466: a yanked release is a retracted release, documentation included, so its
 * URLs 404. Two consequences worth stating where they are implemented.
 *
 * A yank removes live URLs without a release having happened, so the set of published paths
 * is not monotonic — a link that worked yesterday can 404 today. That is inherent in the
 * decision, not a defect in this code.
 *
 * And `yanked` is a set of versions *known* to be yanked. A lookup that failed must arrive
 * here as an empty set, never as "everything is fine to drop": treating an unreachable
 * crates.io as evidence would 404 every version of the docs during someone else's outage.
 * `collect` is where that is enforced; this function only promises to drop what it is told.
 */
export function resolve({ tags, yanked = new Set(), keep = KEEP }) {
  const live = cliTags(tags).filter((t) => !yanked.has(t.version));

  const bySeries = new Map();
  for (const t of live) {
    const series = `${t.major}.${t.minor}`;
    // `live` is newest-first, so the first patch seen in a series is its newest.
    if (!bySeries.has(series)) bySeries.set(series, t);
  }

  const kept = [...bySeries.values()].slice(0, keep);
  return [
    {
      id: 'main',
      label: 'main',
      ref: 'main',
      base: `${ROOT}/main`,
      latest: false,
      development: true,
    },
    ...kept.map((t, i) => ({
      id: `v${t.major}.${t.minor}`,
      label: t.version,
      ref: t.tag,
      base: `${ROOT}/v${t.major}.${t.minor}`,
      latest: i === 0,
      development: false,
    })),
  ];
}

/**
 * What the workflow builds: every version at its own path, and the latest release again at
 * the site root.
 *
 * # Why the root is a release and not `main`
 *
 * It was `main` until #466, and a reader arriving at the URL the README advertises got
 * documentation for tooling nobody could install — `cli-reference` most of all, which
 * `cli_reference.rs` keeps faithful to `main`'s binary and to no other. The bare URL now
 * serves what a reader can install, and `main` moves to a path that says which it is.
 *
 * # Why the latest is built twice rather than copied
 *
 * `base` is baked into every URL Starlight generates and every link the loader rewrites, so
 * a copy of `/yidam/v0.7/` served at `/yidam/` would have every internal link pointing back
 * into `/yidam/v0.7/`. Two builds of one ref is the cheaper half of that trade.
 *
 * The alias carries the latest's own `id`, so its banner and switcher behave exactly as the
 * versioned copy's do: no "you are reading an old release" notice, and the switcher showing
 * the release it is.
 */
export const ROOT_SLOT = '__root__';

export function buildTargets(versions) {
  const latest = versions.find((v) => v.latest);
  return [
    // `slot` is the artifact name and the staging directory: a build target needs an
    // identifier the *alias* does not share with the version it copies, or the two would
    // overwrite each other on the way back from CI and the site root would be whatever
    // finished last.
    ...versions.map((v) => ({ ...v, alias: false, slot: v.id })),
    ...(latest ? [{ ...latest, base: ROOT, alias: true, slot: ROOT_SLOT }] : []),
  ];
}

/**
 * Versions of a crate that crates.io reports as yanked.
 *
 * Takes the parsed response rather than fetching, for the reason the module header gives.
 * A shape this does not recognise yields an empty set — see [`resolve`] on why the safe
 * failure is "nothing is known to be yanked".
 */
export function yankedVersions(cratesIoResponse) {
  const versions = cratesIoResponse?.versions;
  if (!Array.isArray(versions)) return new Set();
  return new Set(versions.filter((v) => v?.yanked === true).map((v) => String(v?.num)));
}
