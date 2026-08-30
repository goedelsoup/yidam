# Configuration

A derived repository configures yidam through four files, and the split between them is not
arbitrary: each answers a different question, and putting an answer in the wrong file is how
one repository's judgement ends up imposed on another.

| File | Answers | Written by |
|---|---|---|
| `.yidam.toml` | *Which yidam governs this corpus?* | `clone` / `overlay`, then `mise run yidam-vendor-update` |
| `.yidam/config.toml` | *What has this corpus decided about itself?* | You |
| `.yidam/lint-baseline.yml` | *What debt did this corpus start with?* | `yidam lint --bless` / `--init-baseline` |
| `.yidam/tonpa.toml` | *Which other corpora does this one draw on?* | `yidam tonpa add` |

Everything is optional. A repository with none of them still lints, still gates, and still
serves — it simply has no ratchet, no dependencies, and no opinion about how its sources age.

---

## `.yidam.toml`

The template provenance pin, at the repository root. Generated; you do not normally edit it.

```toml
[yidam]
origin    = "git@github.com:goedelsoup/yidam.git"
commit    = "88edd17f4c2a1b09e3d5f7a8c6b4e2d1a9f0c3b5"
template  = "v0.1.0"
committed = "2026-08-27"
```

| Field | Meaning |
|---|---|
| `origin` | Where the template came from |
| `commit` | **The resolvable pin** — what the re-vendor procedure and CI check out |
| `template` | The template-layer release tag at that commit, or `"untagged"` |
| `committed` | That commit's author date — i.e. how old the vendored prelude is |

`commit` is the field that makes the pin mean anything; the other three are for reading.
`template` names the **template** layer specifically — bare `v<semver>` tags — because every
other layer prefixes its tag (`cli/v*`, `sdk/rust/v*`, `bootstrap/v*`, `editor/v*`) and a field
that answered with whichever tag happened to sit on the commit would answer for the wrong layer.

`committed` is the *pinned commit's* date, not the date this repository ran the vendor step.
That is what makes `yidam doctor`'s prelude-age check meaningful.

This file is also how the editor surface decides which binary may speak for a corpus: it
records which yidam governs the repository, and only it gets to say.

## `.yidam/config.toml`

What this corpus has decided about itself. Every section and every key is optional, and
**absent means the behaviour is off** — not a compiled-in default quietly applying.

```toml
[lint]
escalate_after = 100

[propose]
withdraw_uncited_after = 400

[catalog]
ttl_days = 180

[index]
model = "BAAI/bge-small-en-v1.5"
```

### `[lint] escalate_after`

Corpus-touching commits a dated finding may hold before it escalates from a warning to an
error.

Absent means **no finding ever escalates**, and that is the design rather than timidity. The
number is a judgement about how fast *this* corpus is meant to consume what it collects — a
breadth sweep landing twelve nodes it will link over the next eighty commits is healthy in one
repository and over-collection in another. A value compiled into the binary would be one
corpus's answer arriving as a build failure in a repository that never agreed to it.

### `[propose] withdraw_uncited_after`

Corpus-touching commits an uncited node may hold before [`yidam propose`](cli-reference.md#propose-is-deliberately-small)
drafts its withdrawal.

Absent means no withdrawal is ever drafted, which is every corpus until someone turns it on.

**This is not `escalate_after` under another name.** `escalate_after` declares when a finding
becomes a build failure — a statement about the gate. This declares when an uncited node stops
being a sweep in progress and becomes over-collection — a statement about the corpus. A
repository may reasonably hold the first and not the second, and most will: failing the build
asks a person to look, while drafting a deletion decides what they would have concluded.

### `[catalog] ttl_days`

Days a catalog entry may stand before it is worth looking at again, **when the entry does not
declare its own**. The per-entry `ttl_days:` is the primary form, because a gauge record and a
statute do not age at the same rate; this key exists for the common case of a corpus whose
sources mostly do age alike, so adopting a TTL is one line rather than one line per entry.

Absent means no entry expires unless it says so itself.

### `[index] model`

The embedding model `index-build` uses. Read only by `index-build`, which the light `reports`
binary does not carry — but the key is still *parsed* there, so a config naming a model is not
rejected by a binary that simply cannot act on it. `yidam index-build --model` overrides it.

## `.yidam/lint-baseline.yml`

The ratchet. It records the findings a corpus already had when the baseline was written, so
`yidam lint` can answer *did this change make the corpus less clean?* rather than *is the
corpus clean?* — which is the only question a corpus with inherited debt can usefully be asked.

Generated, sorted for a stable diff, and identified by node path plus message. You do not hand-edit it.

```sh
yidam lint --init-baseline   # write it only if absent — the adoption path, safe to re-run
yidam lint --bless           # rewrite it from this run, accepting the current findings
```

Two consequences worth knowing:

- **A baselined finding that is later repaired fails the build.** The baseline says "this was
  broken"; a fix means the entry no longer describes reality, and a stale ratchet is a ratchet
  that has stopped ratcheting. Re-bless after a cleanup pass.
- **Baseline membership outranks check severity in the editor.** Inherited debt renders as a
  Hint however severe the check is — see [Editor setup](editor-setup.md).

Commit-vocabulary findings are baselined by **commit**, not by file: history is immutable, so a
baselined commit stays baselined.

## `.yidam/tonpa.toml`

Bundle dependencies on other derived corpora. Written by `yidam tonpa add`; the lock file
beside it records the hashes `tonpa verify` checks.

```toml
[package]
name = "my-domain"

[dependencies.upstream-hydrology]
github = "someone/hydrology-corpus"
tag    = "v1.2.0"

[index]
merge_imported_index = true
```

A dependency is named by exactly one source — `github`, `url`, or `path`. `merge_imported_index`
defaults to **true**: an imported corpus's vectors join the local index so retrieval spans both.
A repository with no dependencies and one that never declared any are the same answer to every
caller, and neither is an error.

[Sharing a derivation](sharing-derivations.md) covers what a cross-corpus citation is and is not.

## The ontology, and what it licenses

Class definitions live at `.yidam/corpus/<class>.ont.yml` and are configuration in the sense
that matters most: they decide what the gate will and will not accept.
[Information architecture](information-architecture.md#ontology-class-definitions) has the full
shape. Three fields are worth calling out here because their defaults are load-bearing.

**`required:` on a property defaults to false.** It is what lets the `missing-property` check
gate at all — without it the check cannot distinguish *every instance of this class has this*
from *an instance may have this*, and gating on the second reading asserts a contract the
ontology never wrote. Defaulting it to true would demand a declaration nobody made, in every
derived repository at once.

**`implemented_by:` is absent by default, and nothing is checked without it.** A class may
name the `struct` or `enum` under `crates/` that implements it, and `unimplemented-class` then
gates when the tree defines no type of that name — the class stated a fact about the code, and
a missing type contradicts it. Reading the *absence* of a type as a finding was measured and
rejected: across twelve derived corpora 129 of 157 declared classes have no type bearing their
name, and matching traits, aliases and every language in the tree raises it to 165 of 186. An
ontology models a domain while `crates/` models the pipeline that gathers evidence about it, so
a class with no type is the ordinary case rather than debt. Name the type as Rust spells it —
`HTTPServer` and `HttpServer` are two types and one kebab-case name, so nothing is derived.

**A property `type:` the corpus coins is carried through unconstrained.** `string`, `text`,
`date`, `ref` and `claim` are the types the tooling understands; anything else is accepted and
left alone rather than rejected.

## Environment variables

| Variable | Read by | Effect |
|---|---|---|
| `YIDAM_BIN_DIR` | `install.sh` | Install target, default `~/.local/bin` |
| `YIDAM_VERSION` | `install.sh` | Install a specific tag rather than the latest |
| `YIDAM_REPO` | `install.sh` | Resolve releases from a fork |
| `YIDAM_REF` | `yidam-vendor-update` | Re-vendor from a tag or branch instead of the pinned commit |
| `YIDAM_CODE` | `ext-dev` | The editor CLI, when `code` is not on `PATH` |
| `YIDAM_REQUIRE_CONTRACT` | extension tests | Turn a missing or stale binary from a skip into a failure; CI sets it |
| `YIDAM_BUILD_COMMIT` | build script | Stamps the commit `yidam --version` reports |

## Editor settings

The VS Code extension contributes five settings; [Editor setup](editor-setup.md) explains when
each matters.

| Setting | Default | Effect |
|---|---|---|
| `yidam.path` | `""` | An explicit binary path, outranking every other resolution step |
| `yidam.lint.showBaselined` | `true` | Show findings the baseline already records, as faded Hints |
| `yidam.diagnostics.debounceMs` | `400` | Wait after a save before re-running the reports — they walk the whole corpus per run |
| `yidam.claims.decorate` | `true` | Tint `[verified]` / `[inference]` / `[open]` inline |
| `yidam.vendor.protect` | `true` | Guard `.yidam/.vendor/` against edits the next re-vendor would discard |

`yidam.claims.decorate` is **always off in high-contrast themes** regardless of this setting: a
high-contrast theme is a stated accessibility choice, and tinting text against it would override
a decision the reader made deliberately.

Turning `showBaselined` off loses nothing a reader needs — the debt is still in
`.yidam/lint-baseline.yml` and still in the gate.
