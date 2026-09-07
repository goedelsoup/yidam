# The catalog-fetch fixture corpus

A corpus whose catalog entry can actually be followed, so that `catalog-fetch` and
`catalog-reconcile` have something to run against end to end.

## Why it is a directory here rather than heredocs in the test

Because what it asserts is a property of a *corpus*, and a corpus written as string literals
inside one test file is one nobody can read, review or run a second command against. That is
the same reasoning `prelude/sdks/parity/mcp/corpus/` records for its own move out of
`mcp_serve.rs`: the counts a suite asserts are about a corpus, and the only way to learn which
corpus was to read Rust.

## Why it is not one of the `examples/`

None of the four could carry it, and each declines for a reason written in its own files.

- **streamflow** states, in the entry itself, that *"No observation from NWIS is reproduced in
  this corpus"* — its gage nodes carry NWIS's conventions and none of its data. A fetched
  artifact would contradict the corpus's own text.
- **journalism** has the receipt shape and two named vaults already, and says of them:
  *"Neither store exists. […] nothing in this example ever reaches a network."*
- **incidents** and **property** declare only `kind: address` locations — a records office and
  a county recorder — which is a real way to hold a source and not a thing a fetch can follow.

Measured rather than assumed: across the four examples there are seven locations, and they are
`address` (4), `url` (1) and `url_template` (2). **Not one `kind: file`.** So the path that
needs no network had no fixture anywhere in the repository, which is most of why this exists.

## What it holds, and which case each piece is

| Piece | The case it carries |
|---|---|
| `sources/registry-2026.csv` | Bytes a `kind: file` location can reach, in-tree and git-tracked |
| `location[0]` — `file` | The whole path with no network: address → bytes → cache → record → `refresh:` |
| `location[1]` — `url_template` | A slot nothing binds. Refused by name, and the refusal says `--bind year=` |
| `location[2]` — `address` | Followable by nothing. Passed over in silence beside a location that is |
| `used-by` | Drifted **in both directions** — see below |
| `.yidam/config.toml` | One vault claiming `catalog`, so the push route is a real answer |

The `used-by` list claims `withdrawn-survey.yml`, which no longer exists, and omits
`hydrant-count.yml`, which cites the entry. Both directions, because `UsedByDrift` reports
them separately and a fixture with only one would leave half of `reconcile` unexercised.

The vault URL is a `file://` path that does not exist, and that is deliberate: a fetch lands
bytes in the local **cache** and never in a store — uploading is `yidam vault push` — so a
store that cannot be reached is exactly the right shape for testing that the fetch does not
try. What the config buys is a route to *report*.
