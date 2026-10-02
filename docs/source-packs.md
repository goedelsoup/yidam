# Source packs

*Fetch a catalog source by its identifier, through a pack that says how its publisher answers.*

A catalog entry can name a source by identifier, such as `doi:10.1167/tvst.8.5.14`.
A **source pack** says how one family of identifiers resolves to an address.
It also says how to ask that publisher politely, and how to read its answers.
One pack serves every corpus that pins it, so nobody writes a connector twice.
[RFC-0048](rfcs/0048-source-packs.md) gives the reasoning.

The template ships two packs in `yidam/sources/`. A corpus can also write its own, or pin one another corpus wrote.

| Pack | Schemes | Reads |
|---|---|---|
| `scholarly` | `doi`, `europepmc`, `pmc`, `arxiv` | Crossref metadata, Europe PMC full text, the arXiv API |
| `archive` | `wayback`, `ia`, `ia-file` | Wayback Machine snapshots, Internet Archive items |

One `doi:` reaches the full text when Europe PMC holds it open access.
The DOI's describe names a `europepmc:` location for the same DOI.
That one asks Europe PMC for the record, and names a `pmc:` location when the full text is open.
A DOI Europe PMC does not index still gets the `europepmc:` location, which leads nowhere.
`scholarly` requires `YIDAM_CONTACT`, and waits three seconds between requests to one host, as arXiv asks.

An `ia:` item names its OCR text as an `ia-file:` location, when it has one.
`archive` is also what `catalog-fetch --archive` needs: it declares `wayback`.

## Name a source by identifier

Write the location as `kind: identifier`, with a `scheme:local-id` value:

```yaml
location:
  - kind: identifier
    value: doi:10.1167/tvst.8.5.14
  - kind: identifier
    value: pmc:PMC6753881
```

The scheme is the text before the first colon. `catalog-fetch` resolves it through the enabled pack that declares it.
An identifier in a scheme no pack declares is passed over, and nothing fails.

An entry written with `kind: doi` or `kind: pmc` is reported by `catalog-location-malformed`. Rewrite them:

```sh
yidam migrate --dry-run locations
yidam migrate locations
```

A value that is a URL is reported and left alone. Rewrite it by hand.

## Find and add a source

`yidam source list` prints each enabled pack and its schemes.
It also says which of the transport's variables are set.

Find an identifier with the pack's search endpoint, then draft its entry:

```sh
yidam source search scholarly "visual acuity reading speed"
yidam source add doi:10.1167/tvst.8.5.14 --fetch
```

`source add` commits a draft `.yidam/catalog/<slug>.md` as `catalog:`.
The draft is `obtained: false`, with the pack's `entry.md` as its body.
`--fetch` then runs `catalog-fetch` on it, as a second commit.
Neither command writes a node.

## Use a pack another corpus wrote

Pin each pack under `prelude_sources` in `.yidam/decisions/proposals.yml`:

```yaml
prelude_sources:
  - scholarly@^0.1
  - us-oh-legislature@^0.3 from github.com/<owner>/<corpus>@<40-hex commit>
```

1. Write `<pack>@<range>` for a pack the template ships.
2. Write `<pack>@<range> from <repo>@<commit>` for a pack another corpus wrote.
3. Run `mise run yidam-vendor-update`.
4. Run `yidam source check` to confirm each copy matches its pin.
5. Commit the copies under `.yidam/.vendor/sources/`.

A range is `^X.Y`, `~X.Y.Z`, `=X.Y.Z` or a bare `X.Y`. The bounds are cargo's.
A bare range means `^`, as in `Cargo.toml`.
The commit is the full 40-character SHA. A short one can come to name a different commit.

A commit holds one version of a pack. So a `from` range is checked against it, never resolved.
The update writes a `[vendored]` table onto each copy. It records the pin, the origin and the commit.

### What the update refuses

The update checks every pin before it touches anything. One refusal leaves the repository unchanged.

It refuses a pin when:

- the pin is neither of the two forms above
- the pin names no range
- the same pack is pinned twice
- a `from` pin does not end in a 40-character commit
- the commit cannot be checked out
- that repository has no pack of that name
- the pack's version does not satisfy the range
- the named corpus only vendored the pack; pin the corpus that wrote it

No `prelude_sources`, or no proposals file, vendors no packs.

## Write your own pack

Put a pack your corpus writes in `.yidam/sources/<pack>/`:

```
.yidam/sources/scholarly/
  pack.toml          # schemes, resolution, transport, defaults, fixtures
  entry.md           # the body a new catalog entry starts from
  transforms/*.glu   # optional describe and extract scripts
  fixtures/          # recorded responses, checked offline
```

An authored pack is read before a vendored pack of the same name.
The vendored copy stays on disk, and `source check` reports it as shadowed.

A pack name is lowercase letters, digits and `-`, starting with a letter.
The directory name and `[pack] name` must match, because a pin names the directory.

A complete manifest:

```toml
[pack]
name        = "scholarly"
version     = "0.1.0"
description = "Papers by DOI and PMCID"

[scheme.doi]
pattern  = '^10\.(?P<registrant>\d{4,9})/\S+$'
type     = "paper"
resolve  = { template = "https://api.crossref.org/works/{id}", media = "application/json" }
describe = "transforms/crossref.glu"
then     = [{ scheme = "pmc", from = "describe.pmcid" }]

[scheme.pmc]
pattern = '^PMC\d+$'
type    = "paper"
resolve = { template = "https://www.ebi.ac.uk/europepmc/webservices/rest/{id}/fullTextXML", media = "application/xml" }
extract = "transforms/europepmc-body.glu"

[transport]
contact      = "required"
min_interval = "100ms"
auth         = [{ env = "CROSSREF_TOKEN", header = "Crossref-Plus-API-Token", prefix = "Bearer " }]
archive      = "wayback"

[defaults]
ttl_days = 365

[fixtures]
"doi:10.1167/tvst.8.5.14" = "crossref-tvst.json"
"pmc:PMC6753881"          = "europepmc-pmc6753881.xml"
```

## Manifest keys

Every table refuses a key it does not know. A misspelt key is an error, not a silent default.

### `[pack]`

| Key | Required | Value |
|---|---|---|
| `name` | yes | The pack name. Must equal the directory name. |
| `version` | yes | The pack's own semver, `MAJOR.MINOR.PATCH`. No pre-release or build suffix. |
| `description` | no | One line saying what the pack resolves. |

### `[scheme.<name>]`

One table per scheme. A scheme name is lowercase letters, digits and `-`, starting with a letter. `http` and `https` are refused.

| Key | Required | Value |
|---|---|---|
| `pattern` | yes | A regular expression the local id must match, anchored `^…$`. Its named groups are template slots. |
| `type` | yes | The catalog `type` an entry from this scheme takes. One of the catalog types below. |
| `resolve.template` | yes | An `http(s)` address. Slots are `{id}` and each named group of `pattern`. At least one slot. |
| `resolve.media` | no | The media type the publisher answers in. Required when the scheme has a transform and fixtures. |
| `describe` | no | `transforms/<name>.glu`: publisher metadata to a draft entry. |
| `extract` | no | `transforms/<name>.glu`: a fetched artifact to a reading. |
| `then` | no | A list of `{ scheme, from }`. Each names a scheme in this pack, and `from = "describe.<key>"`. |

A `then` names the next identifier a source leads to, such as a DOI's PMCID.
Its `from` reads an identifier the scheme's `describe` returns.
`source check` holds each `then` to that. `source add` follows each `then`, and the identifiers it names become the entry's further locations.

The catalog types are `paper`, `dataset`, `api`, `database`, `statute`, `report`, `standard`, `document` and `other`.

### `[transport]`

| Key | Default | Value |
|---|---|---|
| `contact` | `"optional"` | `"required"` skips every fetch through this pack unless `YIDAM_CONTACT` is set. |
| `min_interval` | none | The least time between two requests to one host in a run: `<n>ms` or `<n>s`. |
| `auth` | `[]` | Credentials, each named by its environment variable. See below. |
| `archive` | none | `"wayback"` lets `catalog-fetch --archive` pin a Wayback capture. The only value. It also needs an enabled pack declaring the `wayback` scheme. |
| `blocked` | none | Why this publisher cannot be fetched at all. Its identifiers are refused with this reason. |

Every request sends `User-Agent: yidam-catalog/<version>`.
Set `YIDAM_CONTACT` to an email address or URL, and it is appended as `(+<contact>)`.
A pack cannot set the User-Agent, and no credential may be sent in it.

Each `auth` entry takes one of two forms:

| Form | Sends the credential |
|---|---|
| `{ env = "VAR", header = "Name" }` | In that request header. |
| `{ env = "VAR", header = "Name", prefix = "Token " }` | In that header, after the prefix. |
| `{ env = "VAR", query = "param" }` | As that query parameter. Takes no `prefix`. |

`env` is an uppercase variable name, never the value.
A bare name such as `auth = ["VAR"]` is refused, because it does not say where the credential goes.
No credential reaches the report, the entry or the commit.

Use `blocked` for a publisher that refuses automated clients.
Fetch it by hand, and record the export as a `kind: file` location.

### `[defaults]`

| Key | Value |
|---|---|
| `ttl_days` | The `ttl_days` a new entry from this pack starts with. |

`source add` writes `ttl_days` into the draft entry it commits.

A pack cannot set `redistributable`, and a pack that tries fails to load.
Whether bytes may leave this machine is a licence for one source.
Write `redistributable: true` on that artifact's record, in the entry.

### `[fixtures]`

Map each identifier to a recorded response under `fixtures/`:

```toml
[fixtures]
"doi:10.1167/tvst.8.5.14" = "crossref-tvst.json"
```

The identifier's scheme must be one this pack declares. Its local id must match that scheme's pattern.
Every file under `fixtures/` must be named here. An unnamed file belongs to no scheme and is an error.
Write each path relative to `fixtures/`, as `crossref.json` and not `./crossref.json`.

### `[vendored]`

Never write this table by hand. `yidam-vendor-update` writes it on each copy it makes.
So does the bootstrap's vendor step, for each pack the bootstrap pinned.

| Key | Value |
|---|---|
| `pin` | The `prelude_sources` entry this copy satisfied, verbatim. |
| `from` | The repository it was copied from. |
| `commit` | The commit it was copied at. |

An authored pack that carries `[vendored]` is an error. So is a vendored copy without one.

## Write a transform

A transform is a pure gluon script under `transforms/`. A scheme names at most one of each kind.

| Transform | Entry point | Used by |
|---|---|---|
| `describe` | `Parsed -> EntryDraft` | `source add`, and `source check` over the fixtures. |
| `extract` | `Parsed -> Reading` | `catalog-extract`, on an artifact fetched through this scheme. |

The script is a single function value. The host parses the response, and the script never sees bytes.

### What a transform receives

`resolve.media` chooses the parser. JSON, XML and CSV are read, including `+json` and `+xml` types.
Any other media type is refused by name.

The host hands the script a flat list in document order:

```
type Parsed = { media_type : String, fields : Array Field }
type Field  = { parent : Int, name : String, path : String, value : Value }
type Value  = | Text String | Int Int | Number Float | Flag Bool | Empty | Unrepresentable
```

| Format | How it becomes fields |
|---|---|
| JSON | Each object member and array item is a field. An array item's name is its index. The document itself is not a field. |
| XML | Each element is a field. An attribute is a child named `@name`. Text is a child named `#text`. |
| CSV | The first row is the header. Each row is a field named by its index, with a child per column. |

`parent` is the index of the parent field, or `-1` at the root.
`path` joins every name from the root with `/`, such as `message/title/0`.
A container's value is `Empty`. Its children follow it in the list.
A JSON `null` and an empty CSV cell are also `Empty`.
Whitespace between XML elements is dropped. Adjacent XML text joins into one `#text` field.

Select a field by its `path`. JSON object order is not something a transform can rely on.

### What a transform returns

A `describe` returns an `EntryDraft`:

```
{ name : Option String, kind : Option String, date : Option String,
  description : Option String, identifiers : Array { key : String, value : String } }
```

Leave a field `None` when the response does not say. A `None` kind takes the scheme's `type`.
Each `identifiers` key is what a `then` reads as `describe.<key>`.

An `extract` returns a `Reading`:

```
{ media_type : String, text : String }
```

An empty `media_type` or an empty `text` is refused, and no reading is recorded.

This `describe` takes a Crossref title and PMCID:

```
\p ->
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
```

### The closed prelude

A transform runs in the calculator arm's closed prelude, as a calculator does.
Its modules are already bound by name, so write no `import!`:

| Bound modules |
|---|
| `array`, `bool`, `byte`, `char`, `cmp`, `float`, `foldable`, `function` |
| `int`, `list`, `map`, `option`, `result`, `show`, `string` |

No file, process, environment, network or `debug` module is reachable.
A script that invokes any macro is refused before it runs.
A call budget stops a script that loops.

### Build with transforms

Transforms need the `source-transforms` feature. The released binary does not include it.

```sh
cargo install --git https://github.com/goedelsoup/yidam --locked --features source-transforms yidam
```

`--features full` includes it too. A build without it still checks the rest of each pack.
It reports one info finding per pack with transforms, saying they were not checked.
`catalog-extract` reports an artifact that needs a transform as skipped.

## Check a pack

Run the check after every change to a pack or a pin:

```sh
yidam source check
yidam source check --format json
```

`--format` is `text` (the default) or `json`. It reads only the repository, never the network.

It verifies that:

- each `pack.toml` parses, with no unknown key
- the name matches its directory, and the version is `MAJOR.MINOR.PATCH`
- `entry.md` exists
- each pattern is anchored and compiles
- each template is `http(s)` and every slot is bound
- each `type` is a catalog type
- each transform path is `transforms/<name>.glu` and exists
- each `then` names a declared scheme and reads `describe.<key>`
- `min_interval` and every `auth` entry are well formed
- every fixture is claimed by a scheme, matches its pattern, and exists
- no two enabled packs declare one scheme
- every vendored pack has its pin, and every pin has its satisfying copy

With `source-transforms`, it also admits each transform and runs it over its scheme's fixtures.
Each `then` key must be yielded for at least one fixture.

The JSON report has `passed`, `packs` and `pack_findings`. Read `passed` rather than counting errors.
Each finding has `severity`, `pack`, `path` and `message`. Any `error` fails the check.

A corpus with no packs and no pins passes. It says so.

### Run it in CI

The scaffolded workflow does not run it. Add a step after the binary is installed:

```yaml
- name: Check source packs
  run: yidam source check
```

It exits nonzero on any error. Build with `source-transforms` there if your packs carry transforms.

## Readings on an artifact

`catalog-extract` records each reading under the artifact it was taken from:

```yaml
artifacts:
  - sha256: 9f2c8e…
    media_type: application/xml
    from: 0
    readings:
      - sha256: 41d0a7…
        media_type: text/plain
        by: scholarly@0.1.0/transforms/europepmc-body.glu@sha256:7be1…
```

| Field | Value |
|---|---|
| `sha256` | The reading's own digest, in the same vault cache as the artifact. |
| `media_type` | What the reading is. A quotation is compared with the first `text/plain` reading. |
| `by` | What took it. A transform is `<pack>@<version>/<path>@sha256:<script hash>`. A PDF's is `<crate> <version>`. |

A changed transform script is a new `by`, so `catalog-extract` takes a new reading beside the old.
A reading already taken is never replaced.

An entry written before `readings:` holds its PDF reading under `text:`. It is still read.

Run `yidam vault push` afterwards to share readings with other machines.
See [Reading a PDF](cli-reference.md#reading-a-pdf) for the PDF steps.

## See also

- [Fetching an identifier](cli-reference.md#fetching-an-identifier): what `catalog-fetch` does per request.
- [Source packs](cli-reference.md#source-packs): the reference summary.
- [Artifact vaults](artifact-vaults.md): where fetched bytes and readings are kept.
- [RFC-0048](rfcs/0048-source-packs.md): why packs exist, and what is still to come.
