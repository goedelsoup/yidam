# RFC-0056 — A calculator reads the graph yidam wrote, and writes a file yidam has already read

- **Status:** Implemented
- **Track:** I39
- **Relates to:**
  - #1501 (five derived repos wrote their own `$YIDAM_GRAPH` reader or output writer; the evidence this answers), parent epic #1513
  - RFC-0026 (the capability manifest and `yidam run`, whose §4.2 signal table and §4.3 graph this keeps)
  - RFC-0042 (the typed gluon arm, which already hands a calculator a parsed corpus and needs nothing here)
  - RFC-0052 §2.6 (the doctor check on an output that stamps its own input commit)
- **Versioning layers touched:** CLI and prelude. `yidam run` parses each output before it builds the commit, and refuses one that does not read. The prelude gains `kit/`, a graph reader and an output writer in Rust and in Python. Both are standard library only, and both are vendored with the prelude. No change to the `$YIDAM_GRAPH` format. No change to the signal table format. No report or MCP contract change. No SDK change: §2.4 says why.
- **Downstream reference cases:** measured read-only on 2026-11-22 at `origin/main` of every derived repository on the measuring machine (§1). These are ohio-campaign-finance-2025-2026 (e91e01c), traffic-design (8b1529df), travel-demand (2699f7a), and the six repositories that carry `engine-pack.py`. allen-county-ohio-democratic-party and cellular-automata are not checked out there, so the issue's figures for them are cited, not re-measured.

## Summary

`yidam run` hands a step its corpus as `$YIDAM_GRAPH`, so a calculator has no parser to write.
Every calculator that reads it wrote a parser anyway, because yidam ships nothing that reads the
format. Every calculator writes its YAML by hand, because yidam ships nothing that writes it.
Nothing parses that YAML before it is committed. `doctor` already fails a computed file that
does not parse, but it runs after the commit, and nine invalid blobs reached a default branch.
One of them is on `travel-demand`'s `main` today.

This RFC does two things. `yidam run` reads every output the way a later reader will, and
refuses to commit one that does not read. The prelude ships `kit/`: one file per language that
reads `$YIDAM_GRAPH` and writes YAML that parses by construction. A calculator that uses the
kit lists it in `reads`, so a kit update makes the step stale, which a package dependency could
not do. That is also the answer to the issue's third question. The SDKs stay what they are:
libraries for a consumer reading the corpus. A calculator depends on nothing outside its
`reads`.

## Problem

### 1.1 The readers

`$YIDAM_GRAPH` is TSV ([`resolved.rs`](../../yidam/cli/src/cmd/run/resolved.rs)). It has
`format_version` and `corpus_dir` rows, then `class`, `class_prop`, `node`, `malformed`, `prop`
and `link` records, with `\\ \t \n \r` escapes. It has been in a release since cli/v0.16.0. The
per-field rows for a list of records (`<key>[<i>].<field>`) arrived in cli/v0.25.0, and no
corpus with a built calculator pins that release yet.

The measured repositories hold two independent parsers of it, and one of them has six copies:

| Reader | Lines | What it reads |
|---|---:|---|
| ohio-campaign-finance `crates/sos-calculators/src/graph.rs` | 216 | `node` id, class, label; `prop` id, key, value (kind ignored); `link` id, relationship, resolved target |
| `engine-pack.py` `read_graph()`, byte-identical in civic-infrastructure, land-use-activity, traffic-design, travel-demand, urban-environment-health, urban-land-markets | — | `node` class, label; `prop` cast to int or float by kind; `link` relationship, written and resolved target, `exists` |

Neither reads `format_version`, so neither would notice a format change. Neither reads
`malformed`, `class` or `class_prop`. Both turn a node id into a reference
(`.yidam/corpus/gage/x.yml` into `gage/x`) themselves. The issue reports a third reader,
allen-county's `crates/corpus-graph` (543 lines), and cellular-automata's `resolved_corpus.py`.
traffic-design's nine other calculators read the graph only to test that one node is present.

### 1.2 The writers

Every YAML writer measured is hand-rolled. None uses a YAML library to serialize.

| Writer | How | Blobs | Do not parse |
|---|---|---:|---:|
| ohio-campaign-finance `src/yaml.rs` | Rust emitter, every string through `quote()` | 50 | 0 |
| city-engine `engine-state` | `format!` / `writeln!` | 316 | 0 |
| land-use-activity atus calculators | `writeln!` | 78 | 0 |
| traffic-design, ten calculators | Python f-strings, a local `fmt()` | 85 | 7 |
| travel-demand `percentile-sd-ratio.py` | Python f-strings | 2 | 2 |

The nine are:

- traffic-design `idm-ring.yml` at 6983a4f7, fixed by 57d5ef0f *"so the computed file parses as
  YAML"*.
- traffic-design `ngsim-arterial.yml` at six `compute:` commits on 2026-10-19, fixed by 67a0c3a6
  *"parses as YAML again"*. A flow mapping was misindented.
- travel-demand `percentile-sd-ratio.yml` at acd422c and d3b693e. Line 105 is
  `- [00:00, 184, …]`, an unquoted time in a flow sequence. **It is the file on `main` now.**

The issue adds cellular-automata: 120 f-string calculators run under `python3 -I`, and ten
`fix: … writes YAML` commits on 2026-11-05.

So the failures come from one kind of writer: a string format that has to get YAML's scalar
rules right at every call site. The writers that got it right each wrote a quoting function.
They wrote three.

Not all nine fail the same reader. `yidam` reads YAML 1.2 with `serde_yaml`, which refuses seven:
idm-ring's and the six ngsim-arterial blobs. travel-demand's two are valid YAML 1.2, where
`00:00` is a string. PyYAML reads YAML 1.1, where it is a sexagesimal int, and refuses the flow
sequence. So a run check with `yidam`'s reader catches seven of the nine. The other two are a
writer's failure that only a writer can prevent, and the kit's writer quotes every string.

### 1.3 What the template checks, and when

`yidam run` checks two things about an output before it commits it.
[`check_declared`](../../yidam/cli/src/cmd/run/exec.rs#L532) checks that `writes` covers it.
`build_commit` checks that it is UTF-8. A `.yml` that does not parse is committed.

The rules a later reader applies already exist, in
[`computed::Signals`](../../yidam/cli/src/computed.rs). It requires that a file parses as YAML.
If it has `signals:`, it also requires a `format_version` this binary reads, a `node:` on every
row in RFC-0032's grammar naming a node the corpus holds, scalar values, no reserved names,
and no name declared by two files. `doctor`'s `computed` check fails on any of these, and so do
`embed` and `status`. Every one of them reads the committed tree. The step that wrote the file
has already landed, its receipt says its outputs stand, and a re-run with unchanged inputs
writes nothing.

## Proposal

### 2.1 `yidam run` reads each output before it commits it

After `check_declared` and before `build_commit`, `yidam run` reads every output the step
produced. It reads each one by its extension, the way the next reader will:

| Output | Must |
|---|---|
| `*.yml`, `*.yaml`, anywhere | parse as YAML |
| `*.json`, anywhere | parse as JSON |
| `.yidam/computed/*.yml` | also pass every `Signals` rule, read over the tree the commit would have |
| `.yidam/computed/*.yaml` | not be written |
| anything else | be UTF-8, as today |

The last row is unchanged, so a report artifact keeps every form it has today. `graph.tsv` and
`probe.txt` stay legal. So does a `-report.yml` with no `signals:`, which `Signals` already
reads as a report.

`Signals` lists `.yidam/computed/` for `.yml` only. A `.yaml` there is read by no later reader,
so a signal table under that name would be lost without a word. The refusal names the `.yml`
spelling.

The signal-table rules are `Signals::read` applied to a tree, not a second implementation.
`Signals::read` reads a directory on disk. So `run` checks out `HEAD`'s `.yidam/` and
`.yidam/computed/` into a scratch directory, writes the step's outputs over it, and reads that.
The corpus nodes come from the same tree. A collision with a file the step did not write is the
step's problem, because the step is what introduced it. A problem that names only files the
step did not write is not the step's, and is left to `doctor`. A step is not refused for a
neighbour already being broken.

To tell the two apart, `Signals` reports each problem with the paths it names, beside its
message. Today a problem is a string, and a collision names two files in its text. The run
refuses a problem whose paths include one the step wrote. It does not parse the message.

A refusal reads like `check_declared`'s: *nothing was committed*. It names the file, the
parser's line and column, and that line of the output. Nothing is written to the checkout and no
receipt lands. There is no flag to commit anyway. A step that means to write something that is
not YAML names it something other than `.yml`.

### 2.2 The kit

`yidam/prelude/kit/` holds two files. Each is standard library only:

- `yidam_kit.py`, for Python 3.9 and later. It imports nothing outside the standard library, so
  it runs under `python3 -I`.
- `yidam_kit.rs`, a module a crate includes with `#[path]`. It uses `std` only, so a
  calculator crate with no `serde` dependency gains none.

The prelude is already vendored, so the kit arrives at `.yidam/.vendor/prelude/kit/` with no new
vendor block, bootstrap line or update step. A step that uses it lists
`.yidam/.vendor/prelude/kit/**` in its `reads`, and finds it under `$YIDAM_IN`.

**The reader** parses `$YIDAM_GRAPH` into classes, nodes, properties and links. It refuses a
`format_version` it does not know, which neither existing reader does. It decodes the escapes. It
keeps a property's kind, so an int is an int. It gives each node its RFC-0032 reference beside
its id. It keeps `malformed` rows rather than dropping them. It skips record kinds it does not
know, as the format requires. A list of records comes back as a list of mappings when the graph
carries the `<key>[<i>].<field>` rows, and as the flat rows when it does not.

**The writer** emits block YAML from a value tree of mappings, lists and scalars. Every string is
written double-quoted. So `00:00`, `yes`, `a: b`, `#x`, a leading space and a newline each come
back as the string they were. The writer escapes `"`, `\`, every C0 control, DEL and every C1
control, using `\xNN`. It escapes U+2028, U+2029, U+FFFE and U+FFFF as `\uXXXX`: no reader
accepts the last two raw, and YAML 1.1 folds the first two to a space. Every other character is
written as itself, including a character outside the Basic Multilingual Plane. That is not JSON's rule. JSON writes such a character as a
surrogate pair, which a YAML reader refuses. JSON also leaves DEL and C1 controls raw, and YAML
does not allow them raw. A lone surrogate is refused, because it is not text.

A float always carries a `.`, and an exponent when it has one, as in `12.0` and `1.0e+16`. The
digits are the shortest that round-trip. Without the `.`, Rust writes `12.0` as `12`, which reads
back as an int, and PyYAML reads `1e+16` as a string. A non-finite float is refused. YAML can
spell one, as `.inf` or `.nan`, but JSON cannot, and a reader that loads a signal into a JSON
value would lose it. A calculator that means an unbounded value writes a string for it. An int outside the 64-bit
range `serde_yaml` reads, `i64::MIN` to `u64::MAX`, is refused too.

A key is written bare when it matches `[A-Za-z_][A-Za-z0-9_-]*` and is not a word any YAML schema
reads as something else: `true`, `false`, `null`, `yes`, `no`, `on`, `off`, `y`, `n`, in any
case. Every other key is written double-quoted, like a string.

A signal table is one call:
`signals(rows, method=…)`, which adds `format_version: 1` and requires `node:` on every row.

The kit does not copy `Signals`' rules. It does not know the reserved names, and it does not know
which nodes exist. `yidam run` checks those against the corpus, and a second list in the kit would
be a second list to keep in step (§4.3).

### 2.3 The template's own calculators

`examples/streamflow`'s `travel-tier.sh` is sh and awk over `$YIDAM_GRAPH`, and stays so. It is
the example of a step that needs no kit. A Python calculator, `travel-support.py`, is added beside
it and uses the kit for both directions. For each node, it counts the corpus nodes its links rest
on, and the links that reach no node.

The example vendors no prelude, so its copy of the kit sits beside the script, under
`.yidam/capabilities/`, which the step already reads. A partial `.yidam/.vendor/prelude/` would
change what `doctor` says about the prelude pin. A test holds the copy byte-equal to the kit.

### 2.4 What the SDKs are for

The SDKs are corpus-type and parity libraries for a consumer that reads the corpus: a site
build, an agent, a migration. They stay that. A calculator does not depend on one, for a reason
the run itself states. A step's input state is the digest of what its `reads` admit. A
dependency installed from a registry is outside it. A new version would change what the step
computes, and the receipt would still say its outputs stand. A vendored file in `reads` is
inside it. A kit update makes every step that uses it stale, and `doctor` says so.

That is also why the kit is not a module of `yidam-core` on crates.io, and why there is no
TypeScript kit. No measured calculator is TypeScript. The TypeScript readers in matt-huffman and
ohio-education-funding read the corpus files for a site build, and need the description and body,
which `$YIDAM_GRAPH` does not carry. That is the SDKs' job, and the open questions leave it open.

### 2.5 Tests

- **The run refuses each failure shape it can see.** A step writing each of these is refused,
  and nothing is committed: a misindented flow mapping; `signals:` with
  no `format_version`; a row keyed on a node the corpus does not hold; a name another committed
  file declares; invalid JSON; a `.yidam/computed/*.yaml`. A step writing a TSV, a `.txt`, or a report `.yml` with no
  `signals:` commits as before.
  travel-demand's line 105 parses for `yidam`, and a unit test says so (§1.2).
- **A neighbour's problem is not the step's.** A corpus whose committed computed file collides
  with itself still lets an unrelated step land.
- **The reader agrees with the writer of the format.** The template's test renders the graph of
  `examples/streamflow` and of a fixture holding every property kind, every escape, a malformed
  node, a list of records and an unknown record kind. It runs both kits over it and compares
  their parse with the corpus yidam parsed.
- **The writer's output reads.** Hostile strings, every scalar type and a nested report are
  written by both kits and read back by `Signals::read`, by `serde_yaml` and by PyYAML. The
  results must equal the input, type for type, and report no problems. The hostile set includes
  the keys `true`, `null`, `on` and `N`; a string with an emoji, DEL and U+0085; and the floats
  `12.0`, `1e16` and `-0.0`.
- **The kit is standard library only.** The Python kit runs under `python3 -I` with an empty
  `PYTHONPATH`. The Rust kit compiles in a crate with no dependencies.

The run's refusals are suite tests in `yidam/cli/tests/suite/run_outputs.rs`. The kit's are unit
tests in `yidam/cli/src/cmd/run/kit.rs`, because the graph renderer and the corpus reader they
compare against are private to the crate. The hostile set is
`yidam/cli/tests/fixtures/calculator-kit/hostile.json`. The PyYAML reading is
`yidam/sdks/python/tests/test_calculator_kit.py`, where PyYAML is already a dependency.

## What this does not touch

- The `$YIDAM_GRAPH` format and the signal table format. Neither gains a field.
- The gluon arm (RFC-0042), whose script receives a typed value and returns a typed `Computed`.
  Its outputs pass through §2.1 like any other, and parse by construction.
- History. Committed files that do not parse are left as they are. `doctor` already reports
  them.
- A step's own choice of writer. A hand-rolled writer that produces YAML that reads keeps
  working. None of the 522 valid blobs in §1.2 would be refused.

## Migration & compatibility

A step whose output does not parse is refused on its next run. That is the intent, and it is the
one break. travel-demand's `percentile-sd-ratio` is the measured case. Its next run is refused
until its writer quotes the time, or uses the kit. The upgrade note says so and names the
refusal's text.

Nothing else changes for a corpus that does not adopt the kit. The kit arrives with the next
`yidam vendor update`, and nothing reads it until a step lists it.

A corpus that adopts it replaces its reader and writer with the kit and adds the `reads` line.
The first run after that is a recompute, because the step's code changed. It should land outputs
whose values are unchanged. Where they differ, the old writer was the one that was wrong, or the
old reader was. The six engine-pack copies can take the kit in one change each, because their
`read_graph()` is identical.

## Alternatives considered

### 4.1 Validate in `doctor` or in CI only

`doctor` already does. Nine invalid blobs landed anyway, because the step that wrote them
reported success and committed. A check that runs after the commit finds the file on the branch.
The run is the one place that holds both the bytes and the chance not to land them.

### 4.2 Hand the step a JSON graph, and take JSON outputs

JSON would end the quoting problem for writers, since every language has a JSON serializer in
its standard library. But the signal table is YAML in every corpus that has one, `embed` and
`status` read it as YAML, and a JSON graph would be a second format beside a released one. The
kit gets the same property from YAML: every string it writes is a JSON string, which YAML reads.

### 4.3 Put the full `Signals` rules in the kit

Then a calculator could check itself before it exits. But the reserved names come from the
embed record and the vector metadata, and node existence comes from the corpus. The kit would
have to carry a copy of both and track every change to them. `yidam run` already holds both,
and refuses before anything lands, so the copy would buy an earlier message, not a stronger
guarantee.

### 4.4 Publish the kit as packages

Rejected in §2.4. A registry dependency is outside the step's input state, and the run could not
say whether an answer still stands.

## Why the run refuses rather than warns

A warning on a run that then commits is the state §1.2 measured. The step lands, its receipt
says its outputs stand, and a re-run with unchanged inputs writes nothing. So the next chance to
fix the file is a person reading `doctor`. In traffic-design that took one day for one file. The
broken travel-demand file has stood for a month. A refusal costs a step nothing it should have
kept. It keeps the previous commit's outputs, and those parsed.

## Expected effect, and the falsifier

Expected: no `.yml` or `.json` output lands from `yidam run` that does not parse, and no signal
table lands that `doctor` would fail. Corpora that adopt the kit delete their own reader and
writer: about 216 lines in ohio-campaign-finance, and a `read_graph()` and a `yaml_str` in each
of the six engine-pack copies.

Falsified if, within two releases of this landing, a corpus pinned at or after it commits a
`compute:` output that `doctor` fails as unparseable. It is also falsified if a corpus that
adopts the kit re-introduces its own graph parser or YAML emitter beside it. That would mean the
kit's surface is not the one calculators need.

## Open questions

- **Should the refusal keep the step's output?** Today a refused step's scratch directory is
  dropped, and the error shows one line. A person debugging a writer may want the file. A
  `--keep` that leaves the scratch directory and prints its path is the smallest answer.
- **Does `$YIDAM_GRAPH` need the node's description and body?** Every direct-file reader in §1
  reads them: claim-tag scans, citation scans, span checks. The graph omits them deliberately,
  because a calculator over prose is a different thing to make stale. engine-pack opens the node
  files to get them. If more calculators do the same, the omission is costing more than it saves.
- **Should the reader resolve the six independent link resolvers too?** Six readers that are
  not calculators each resolve `../` targets themselves. That is an SDK question and is out of
  scope here.
