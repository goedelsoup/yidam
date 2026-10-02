# RFC-0048 — A source is named by what it is, and fetched by something the corpus did not write (source packs)

- **Status:** Draft
- **Commands:** `source`
- **Track:** I35
- **Relates to:**
  - RFC-0023 (the `artifacts:` record and `catalog-fetch`, which this extends rather than replaces, and whose `redistributable` default every pack inherits)
  - RFC-0026 (the invariant that a fetch writes bytes and never a node, restated here as the boundary a pack may not cross, and #460 decision 9, which this leaves standing)
  - RFC-0042 (the closed-prelude gluon arm, which this proposes as the only place a pack may carry logic)
  - RFC-0024 (the bar an embedded engine is measured against, which the gluon half of this proposal inherits rather than re-argues)
  - RFC-0045 (the precedent for measuring what derived repositories built for themselves and lifting the shape they agree on)
  - RFC-0005 (the MCP tool contract the two read-only tools of §8 are added to)
- **Versioning layers touched:** template (`guidelines/directories.md` gains the `identifier` location kind and the pack layout; the bootstrap skill's first-catalog-entry step offers packs) / tooling (one `source` command, an extended `catalog-fetch`, one migration, a vendored `yidam/sources/` tree) / bootstrap protocol (`prelude_sources` beside `prelude_domains`, pinned per pack) / **MCP contract (minor bump)**: two read-only tools, `search_sources` and `resolve_source` (§8). The catalog `type` set grows by four (§2.1).
- **Downstream reference case:** ten derived repositories on one machine — 1,069 catalog entries and every connector crate beside them — measured read-only on 2026-10-01.

## Summary

A catalog entry is written by hand, and the only thing that can obtain the bytes it names is a
single plain `GET` (`cmd/catalog/transport.rs:47-101`). So derived repositories obtain their
sources somewhere else. Three of ten wrote working connectors, and they agree on five patterns
while sharing no code for any of them. Five more declared connector crates that are still READMEs.
One added about 330 scholarly entries one commit at a time, resolving each DOI against Crossref
and Europe PMC by hand. Of 1,014 entries with frontmatter, 220 record any `artifacts:`.

This RFC proposes a **source pack**: a directory the template ships, and a derived repository may
also write, that names a family of sources by identifier. It covers four things:

1. how an identifier **resolves** to a fetchable address
2. how the publisher is to be **asked** for it (contact, key, spacing, archive)
3. what a new catalog entry for it **looks like** before anyone has read it
4. optionally, a **pure transform** from what was fetched to a derived reading of it

A pack performs no effects. The host does every fetch, through `catalog-fetch`, into `artifacts:`.
Logic a pack needs beyond a template is gluon under RFC-0042's closed prelude, so a pack cannot
reach the network, the filesystem or a node.

The aim is to make the connector the default instead of the exception: discovery and download
patterns a corpus inherits on day one, and a pack format a corpus can write for its own
publishers without writing a crate.

## Problem

### 1.1 What the catalog asks for, and what corpora wrote

| Measure (10 repositories) | Count |
|---|---:|
| catalog entries with frontmatter | 1,014 |
| entries recording `artifacts:` | 220 (4 repositories) |
| entries with a `url_template` location | 86 |
| entries with a Wayback location | 84 (6 repositories) |
| locations of kind `doi` or `pmc` — refused by `catalog-location-malformed` today | 30 |
| entries in a repository with no frontmatter at all | 46 |

The shape corpora reach for is an identifier, not a URL. A transport-engineering corpus wrote
`kind: doi, value: 10.1167/tvst.8.5.14` beside `kind: pmc, value: PMC6753881`, and the
location kind set (`parse.rs:163`) refuses both. The same corpus has 140 entries with an
identical signature: type `paper`, a `doi.org` URL, and a Europe PMC `fullTextXML` URL. Its
history shows 267 one-entry commits. That is a resolver, run by hand, 267 times.

Government sources carry identifiers just as clearly. They are written as URLs that embed them:

| Family | Entries / repos | Identifier inside the URL |
|---|---|---|
| State legislature (bill text, status) | 122 / 5 | general assembly + bill + version code |
| State revised code | 55 / 6 | section number |
| Courts (state supreme court, CourtListener, case.law) | 51 / 5 | citation, docket, reporter volume/page |
| Census (TIGER, gazetteer, API) | 54 / 4 | vintage + FIPS + table |
| Secretary of state (campaign finance) | 34 / 5 | report id |
| FEC | 18 / 3 | committee id, filing id |
| eCFR | 15 / 2 | date + title + section |
| govinfo / US Code | 15 / 3 | package id + granule |
| Socrata portals (CMS, CDC, HRSA) | 19 / 3 | dataset id + SoQL |
| BLS | 7 / 2 | series id |

The scholarly families — DOI/Crossref (179), Europe PMC/PMC (122) and arXiv (30) — appear in one
repository and are the largest by volume. The government families are smaller, and appear in
four to six repositories each.

### 1.2 What connectors corpora wrote, and the five things they each wrote again

Three repositories have connectors that make requests: a school-funding corpus (one `connect`
crate, 21 connectors, 207 declared sources, ~5.7k lines), a legislator dossier (six crates behind
a `live` cargo feature, ~22k lines with committed data), and a housing corpus (one REST client,
1.5k lines). Eighteen more connector crates across five repositories are a README and nothing else.
They are stubs waiting for someone to write the fetch.

All three working designs agree on five things, and implement each one separately:

1. **Network behind a switch.** CI is hermetic and fixtures are committed. A cargo feature, a
   separate CLI with a `NotCached` error, and out-of-band retrieval plus an attestation file are
   three spellings of one rule.
2. **A digest manifest of the publisher's exact bytes.** `source-digests.txt`, `manifest.jsonl`
   and `attestation.json` are three schemas for what `artifacts:` already records. Two of them
   hand-roll SHA-256.
3. **A polite User-Agent with a contact.** BLS returns 403 without one. Each crate defines its
   own `AGENT` constant, and none spoofs a browser.
4. **Atomic writes, and a non-200 kept as data.** `.partial` then rename; a refusal is recorded,
   not retried into silence.
5. **A registry of sources with a status.** `Wired`, `Parsed`, `Retrievable` and
   `Declared { blocked_on }` in one corpus is what the README stubs in six others are trying to
   say.

**None of them writes `artifacts:`.** Every connector bypasses `catalog-fetch`. In the corpora
that use `catalog-fetch` (152 of 172 entries in one, 32 of 118 in another), the decision records
say why the rest went around it. One reports a certificate chain `reqwest` refuses
(`UnknownIssuer` on dol.gov). Another reports that agents fetched into scratch directories and
wrote 45 entries by hand, so a 56 MB digest exists only in a commit message.

### 1.3 Why corpora do not write connectors

The stub crates say it themselves: a connector costs a crate, a dependency argument (`curl`,
`ureq` and `reqwest`+`tokio` were each chosen separately), a cache, a manifest and a fixture
discipline before it fetches one source. The template offers a single `GET` and no place to put
anything else. So a corpus either pays all of it, or fetches by hand and writes the entry
afterwards. Usually it fetches by hand.

## Proposal

### 2. The `identifier` location kind

`CATALOG_LOCATION_KINDS` gains one kind:

```yaml
location:
  - kind: identifier
    value: doi:10.1167/tvst.8.5.14
  - kind: identifier
    value: pmc:PMC6753881
    description: full text, read via Europe PMC fullTextXML
```

The value is `scheme:local-id`. The scheme names a pack's identifier, not a host. One kind keeps
the set closed. A kind per scheme would grow the set with every pack, and the 30 locations
refused today show where that ends. `catalog-location-malformed` checks that the scheme is one an
enabled pack declares, and that the local id matches the scheme's declared pattern.

A migration rewrites `kind: doi, value: X` to `kind: identifier, value: doi:X`, and `pmc`
likewise.

### 2.1 The catalog `type` set grows by four

Corpora wrote `statute` (30), `report` (59), `standard` (27) and `document` (62) because the
schema's five — `paper`, `dataset`, `api`, `database`, `other` — had no word for them. Those
four join the closed set, which stays closed: a pack's scheme declares its type from the set,
and `cmd/schema.rs` and the lint read one list.

`primary` (107 entries, one corpus) is not added. It says what standing a source has, not what
form it takes, and a primary source can be a statute, a dataset or a report. The migration
does not guess. An entry typed `primary` is reported by the schema check with the four new words
offered, and a person picks one. The long tail (`publication`, `reference`, `thesis`, `book`
and others, 25 entries) is handled the same way.

### 3. A pack

```
yidam/sources/<pack>/
  pack.toml          # schemes, resolution, transport, entry template
  entry.md           # the body a new entry starts from
  transforms/*.glu   # optional; pure, closed prelude (§6)
  fixtures/          # recorded responses; tests never touch the network
```

```toml
[pack]
name    = "scholarly"
version = "0.1.0"   # the pack's own semver; see below

[scheme.doi]
pattern  = '^10\.\d{4,9}/\S+$'
type     = "paper"
resolve  = { template = "https://api.crossref.org/works/{id}", media = "application/json" }
describe = "transforms/crossref.glu"         # metadata -> entry fields (§6)
then     = [{ scheme = "pmc", from = "describe.pmcid" }]

[scheme.pmc]
pattern = '^PMC\d+$'
resolve = { template = "https://www.ebi.ac.uk/europepmc/webservices/rest/{id}/fullTextXML", media = "application/xml" }

[transport]
contact       = "required"   # YIDAM_CONTACT goes into the User-Agent; never a browser string
min_interval  = "100ms"
auth          = []           # e.g. ["COURTLISTENER_TOKEN"] — env var names, never values
archive       = "wayback"    # may add a pinned `id_` snapshot location (§5)

[defaults]
ttl_days        = 365
redistributable = false
```

Resolution is a template wherever a template suffices. The 86 measured `url_template` values
are that shape already: an address with an identifier's parts left as placeholders. `then` chains one identifier to another. It is
how 140 entries got their second location by hand.

**A pack carries its own semver.** A pack changes on the publisher's schedule, not on
yidam's. Crossref adds a field, or a legislature moves its API to `v3`. So `version` in
`pack.toml` is the pack's own, and a corpus pins it:

```yaml
# .yidam/decisions/proposals.yml
prelude_sources:
  - scholarly@^0.1
  - archive@^0.1
  - us-oh-legislature@^0.3 from github.com/<owner>/<corpus>@<commit>
```

- `yidam-vendor-update` resolves each pin.
- It copies the pack wholesale into `.yidam/.vendor/sources/<pack>/`, the way it copies domains.
- It refuses a pin that the available version does not satisfy, rather than vendoring the nearest version.
- The vendored `pack.toml` records the exact version.
- Every reading's `by` (§6) names `<pack>@<version>` beside the transform's hash.

A pack's major version bumps when a scheme's pattern or resolution changes, since an entry's
identifier then resolves differently.

**Where packs come from.** There are three places:

- **Template packs.** The template ships generic packs in `yidam/sources/`. Each pack is
  versioned on its own, and the template carries the latest version of each.
- **A corpus's own packs.** A corpus writes packs for its own publishers in
  `.yidam/sources/<pack>/`. They are tracked and linted, and they are read before vendored ones.
  That is how a corpus carries a pack for a county auditor that the template will never ship.
- **Another corpus's pack.** A corpus can vendor a pack that a different corpus wrote. The pin
  names the repository and commit (the `from` form above), and the vendored copy records both.
  It is the same pinned-peer model `tonpa` uses for gathers.

Nothing is fetched from a pack's source at `catalog-fetch` time. Vendoring is the only step that
reaches another repository, and it is a reviewed commit.

### 4. The `source` command

One top-level command with subcommands, so the roster grows by one:

- **`yidam source list`** — enabled packs, their schemes, and each scheme's transport needs
  (`contact`, `auth`), with which of those the environment satisfies.
- **`yidam source search <pack> <query>`** — runs a pack's declared search endpoint, where it
  has one (Crossref `query`, CourtListener v4 search, the eCFR search API), and prints candidate
  identifiers. It is read-only, and it writes nothing at all.
- **`yidam source add <scheme:id>…`** — resolves each identifier, runs `describe` if the scheme
  has one, and writes a **draft** `.yidam/catalog/<slug>.md`. The draft has the identifier
  locations, the type, `ttl_days` and the pack's `entry.md` body. It is written `obtained:
  false`, since nothing has been read yet. Corpora's bodies converge on the same sections: what it
  is, what was read, what it establishes, what it does not, what else it holds unread, defects,
  access constraints and currency. So that is the template, with every section a prompt and none
  pre-answered. With `--fetch` it then runs `catalog-fetch` on the new entry, as a second commit.
- **`yidam source check`** — verifies every pack's fixtures against its transforms, offline. It
  is the gate a corpus-authored pack is held to in CI.

`source add` writes a catalog entry and nothing else. It never writes a node, and it never sets
`obtained: true`. Saying a source was read stays a person's commit, or a `catalog-fetch` that
recorded the bytes.

### 5. `catalog-fetch` learns identifiers

`location.rs` gains a `Plan` arm for `identifier`. It resolves through the pack to a URL plus the
pack's transport, still without network, still refusing what it cannot follow. `transport.rs`
gains exactly the five patterns of §1.2, once, for every pack:

| Pattern | In the host |
|---|---|
| network behind a switch | `catalog-fetch` already is; `source check` and every test run offline against `fixtures/` |
| digest manifest | `artifacts:`, unchanged — the only manifest |
| contact in the User-Agent | `yidam-catalog/<ver> (+$YIDAM_CONTACT)`; a pack declaring `contact = "required"` refuses to fetch without it |
| atomic write, non-200 as data | `.partial` + rename into the cache; a 403 is recorded on the entry as a finding, not retried |
| per-host spacing and auth | `min_interval`; `auth` env vars sent as the pack declares, never written to the repository |

**Archive.** Wayback is a cross-cutting pack rather than a per-pack feature. `wayback:<timestamp>/<url>`
resolves to the `id_` form, which 165 of the measured Wayback locations already use. A pack with
`archive = "wayback"` lets `catalog-fetch --archive` look up the nearest capture through the
availability API, which is a read. It then adds that capture as a pinned location with its date
in the description. It never asks the archive to capture anything: that is a write to a service
the corpus does not own, and stays a person's act.

**What the host refuses.** It sends no browser User-Agent, rotates no client, and takes no step
around a 403. One state's secretary of state refuses all automated clients, and from 2024 on the
Wayback captures of that site are captures of the refusal page. The pack for such a publisher
says `blocked = "..."` and `source list` reports it. The answer to a block is a manual export
recorded as a `file` location, which is what that corpus did.

### 6. Pure gluon transforms

A pack may carry logic in exactly two places, both pure:

```
describe : Parsed -> EntryDraft   -- publisher metadata -> name, type, date, second identifiers
extract  : Parsed -> Reading      -- fetched artifact   -> a derived reading (text, table)
```

These run under `gluon_arm`'s closed prelude: an allowlist of pure `std` modules, no macro
invocation, and the entry point's type checked. That prelude has no `regex` or `serde`, so
**the host parses** (JSON, XML, CSV) and marshals a typed `Parsed` value in. The transform only
maps.

**Host-side parsing.**

- JSON and CSV use parsers already in the closure.
- XML is needed because Europe PMC full text, arXiv's Atom feed and eCFR all answer in it. It
  takes `quick-xml`, behind the same feature as the transforms.
- `quick-xml` is measured against RFC-0024's bar (marginal packages, binary delta) in the PR that
  adds it, and the numbers go in that PR.
- The default build gains no XML parser. A scheme whose `resolve` names an XML media type still
  fetches in the default build. Only its `describe` and `extract` wait for the feature. A transform cannot fetch, read a file or write a node, because the prelude does not
contain those things to refuse — the argument RFC-0042 makes for calculators, unchanged.

An `extract` output is recorded on the artifact as a derived reading, generalising the `text:`
reading `catalog-extract` already writes for PDFs:

```yaml
artifacts:
  - sha256: 3f1c…
    media_type: application/xml
    readings:
      - sha256: 9ab0…
        media_type: text/plain
        by: scholarly@0.1.0/transforms/epmc-body.glu@sha256:77e2…
```

`by` names the transform by content hash. So a reading is reproducible from the artifact, and a
changed transform is a changed reading, not a silent one.

**Why gluon and not code a pack runs.** #460 decision 9 refused connectors that `run` a corpus's
own program, and this RFC leaves that standing. A transform is not a connector. It has no
effects, so the reasons decision 9 gave do not reach it. And it is what lets a corpus write a
pack for its own county auditor without writing a crate, which §1.3 says is the thing that stops
them.

**Why gluon is optional.** Templates resolve most measured identifiers (§3). A pack with no
`.glu` works in the default build. Transforms need `calculators-gluon`, which stays outside the
default set for RFC-0024's reasons: +71 packages and +6.8 MB, measured 2026-09-26. A pack whose
scheme declares `describe` degrades without the feature: `source add` writes the draft from the
template alone and says what it could not fill.

### 7. The first packs

Chosen by the measurement in §1.1, generic families first:

| Pack | Schemes | Why first |
|---|---|---|
| `scholarly` | `doi` (Crossref), `pmc` (Europe PMC), `arxiv` | ~330 entries; the 267-commit hand pipeline is this pack |
| `archive` | `wayback` | 84 entries in 6 repositories; every other pack leans on it |
| `us-federal` | `ecfr`, `govinfo`, `uscode`, `fec`, `bls`, `census` | 2–4 repositories each; stable public APIs |
| `courts` | `courtlistener`, `caselaw` | 5 repositories; v4 API needs a token, which exercises `auth` |
| `socrata` | `socrata:<host>/<dataset>` | one scheme covers CMS, CDC, HRSA and city portals |

The state-legislature, revised-code and state-court families have the widest spread (five to six
repositories), but every one of them is one state's. **The template ships no state pack.** These
become the first corpus-authored pack: written in one derived repository's `.yidam/sources/`,
and vendored by the others through a `from <repo>@<commit>` pin (§3). That is the acceptance
test for two claims: a corpus can carry a pack the template does not ship, and a pack written
once is used five times.

### 8. Two read-only MCP tools

The measured workflow is an agent adding sources: 267 one-entry commits, each co-authored by
one. So the search half of §4 is offered to agents directly. The contract (`mcp-contract.json`)
gains two read-only tools and a minor version:

- **`search_sources { pack, query, limit? }`** — the same as `source search`. Returns candidate
  identifiers with the pack's `describe` summary where the build has transforms, and the raw
  title otherwise.
- **`resolve_source { identifier }`** — the draft entry `source add` would write: frontmatter,
  locations after `then` chaining, and the body template. Nothing is written. It reports which
  transport needs (`contact`, `auth`) the server's environment does not satisfy.

No tool writes. Adding an entry is a commit, and stays `source add` on the CLI. An agent that
has `resolve_source`'s draft in hand runs `source add` exactly as a person would. A write-tier
`add_source` under RFC-0029 is left for a later RFC, once the read tools have been used.

All three servers implement the two tools under RFC-0005's parity rule. A server whose
environment has no network returns the pack's fixtures marked as such, not an error, so the
tools are testable offline like `source check`.

## What this does not touch

- **Nodes.** No pack, transform or subcommand writes one. RFC-0026's invariant is the reason the
  transform is pure, not a rule a pack is trusted to keep.
- **#460 decision 9.** A `kind = "connector"` capability is still refused by `run`. A pack
  is not a capability.
- **An MCP write tool.** §8 adds two read tools and no write. `add_source` waits for its own RFC.
- **Moving bytes between cluster pods** (#1224). `catalog-fetch` on a cluster is unchanged.
- **Getting past a block.** The host takes no step around a publisher's refusal (§5).

## Migration & compatibility

- **Template:** `directories.md` documents `identifier`, packs and `prelude_sources`. The
  bootstrap skill's first-catalog-entry step offers `source search` / `source add` for enabled
  packs.
- **Tooling:** one new command, an extended `catalog-fetch`, an extended
  `catalog-location-malformed`, the `doi`/`pmc` → `identifier` migration, and four more catalog
  types. An entry with no identifier location behaves exactly as today.
- **MCP contract:** a minor bump for `search_sources` and `resolve_source`. It is taken when the
  implementing PR lands, not here, because two branches can each claim the same contract number.
- **Adoption:** a corpus opts in by listing packs. One that lists none sees no change except
  that 30 locations it wrote as `doi`/`pmc` now migrate instead of warning.
- **Existing connector crates** keep working. A corpus that moves one onto a pack drops its own
  manifest for `artifacts:`, which the corpora that use `catalog-fetch` already did.

## Alternatives considered

- **A location kind per scheme** (`kind: doi`). This is what corpora wrote. It grows a closed set
  with every pack, and a lint over the set cannot tell an unknown scheme from a typo.
- **Packs as Rust, compiled in behind features.** Every pack would be a release of yidam, and a
  corpus could not write one. That is §1.3's barrier, moved upstream.
- **Packs as programs a corpus runs** (a `connector` capability, made runnable). This reverses
  #460 decision 9, and puts an effectful program on the path between a publisher and the record
  of what it served.
- **Lift one corpus's `connect` crate.** It is the most complete of the three. But it keeps a
  manifest beside `artifacts:` rather than in it, and its registry is Rust, which brings back
  the second alternative.

## Decisions

Settled in review on 2026-10-01. These were this RFC's open questions.

1. **Catalog `type`:** the closed set gains `statute`, `report`, `standard` and `document`.
   `primary` is a standing, not a form, and is resolved by hand (§2.1).
2. **XML parsing:** `quick-xml` on the host, behind the transform feature, measured against
   RFC-0024 when it lands (§6).
3. **MCP:** in this RFC, as two read-only tools, `search_sources` and `resolve_source`. No write
   tool (§8).
4. **Pack versioning:** each pack carries its own semver, and a corpus pins it in
   `prelude_sources` (§3).
5. **Per-state packs:** written by a corpus, and shared between corpora by a pinned
   repository and commit. The template stays state-neutral (§7).

## Open questions

1. **Resolving a version range across sources.** A pin like `^0.1` against the template has one
   candidate. The same range against another corpus's repository has its git history. Should
   `from` pins accept a range at all, or only an exact version at a commit?
2. **Fixtures in an offline MCP server.** §8 returns fixtures when there is no network. Should
   that response carry a marker in the contract, or is it enough that the identifiers are the
   fixtures' own?
