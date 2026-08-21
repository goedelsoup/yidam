# yidam for VS Code

The editor surface for a yidam-derived knowledge repository — RFC-0016.

It lives here, beside `yidam/cli`, and not in a repository of its own. The extension and the
JSON report contract it consumes must version together; separating them re-creates, between
the extension and the CLI, exactly the drift the RFC set exists to close. The vendor step
copies only `yidam/prelude/`, so derived repositories carry none of this.

## The rule

> **TypeScript computes affordances. The CLI computes verdicts.**

An affordance is a navigation or authoring convenience whose failure mode is *not helping* —
go-to-definition on an edge, completion in a commit message. A verdict is a statement about
whether the corpus is sound. Verdicts cross the process boundary as JSON from the binary the
repository pins, and this extension renders them. It never derives them.

A TypeScript re-implementation of the checks is the failure this whole set of RFCs exists to
close. One downstream project already wrote ~1,600 lines of it, in Python, whose docstrings
claim faithfulness to Rust symbols it has since drifted from.

## Running it

```
mise run ext-dev            # compile, stage a fixture, open an Extension Development Host
```

or **F5** from the repository root, which runs the same two steps as its `preLaunchTask`.
`YIDAM_CODE` names the editor CLI if `code` is not on your `PATH`.

**The workspace it opens is a staged copy of the reports fixture.** The extension activates
on `workspaceContains:.yidam.toml` or `.yidam/**`, and this repository is not a derived
repository — it has neither, so launching against the repo root activates nothing. `mise run
ext-fixture` builds one at `.local/ext-fixture` through the same `stage.toml` the goldens and
these tests read, so what you see by hand and what CI checks are one repository rather than
two that drift. Re-run it after editing the fixture; it rebuilds from scratch.

Four nodes across two classes, an open question of each arm, a claim tag of each kind, a
deliberate broken edge, two phase branches and a sangha — small, and chosen so that every
view has something to show. `basic/README.md` says what each property is there to reach.

The tests are the other half, and they need no editor:

```
cd yidam/editors/vscode && npm run test:unit
```

`YIDAM_REQUIRE_CONTRACT=1` turns a missing or stale binary from a skip into a failure, which
is what CI sets.

## Which binary

Resolved once at activation and re-checked when `.yidam.toml` or the setting changes:

1. the `yidam.path` setting
2. `.yidam/bin/yidam` — this repository's own build
3. `yidam` on `PATH`
4. a mise shim resolved from the workspace
5. not found

**Not found is a first-class state**, not an error path: verdict features are disabled, the
status bar says so, and the single offered action runs `mise run yidam-build` in a terminal
where you can watch it.

The extension never bundles, downloads, or builds a binary. `.yidam.toml` records which
yidam commit governs a corpus, and only it gets to say — an editor that quietly installed a
different one would make its verdicts disagree with CI's.

Which is why the repository's own build outranks `PATH`. `mise run yidam-build` installs to
`.yidam/bin/`, beside the pin it was built from; `~/.cargo/bin/yidam` is one location per
*machine* while the pin is one per *repository*, so on a machine with two yidam repositories
it is whichever built last. Preferring `PATH` would let one repository's pinned binary answer
for another's corpus — the disagreement above, arriving by a route nobody chose. An explicit
`yidam.path` still wins: that is somebody's decision, and this is a default.

## Diagnostics

`lint --format json` and `graph-check --format json`, on save, on ref change, and on demand
— debounced, cached per git OID plus a generation counter that a save bumps (the reports
read the working tree, not the commit).

**The severity mapping is the whole feature.** `yidam lint` does not ask *is the corpus
clean?* It asks *did this change make it less clean?* An extension that rendered every
finding as an Error would fill the Problems panel with inherited debt no commit caused, and
the panel would stop being read — the same failure the baseline exists to prevent, one layer
up.

| | |
|---|---|
| `error`, not baselined | **Error** — this is what fails CI |
| `warn` / `info`, not baselined | Warning / Information |
| any severity, **baselined** | Hint, faded, source `yidam (baseline)` |
| stale baseline entry | **not a diagnostic** — a repository condition with a Bless action |

The last row matters: a baseline entry that no longer occurs also fails the gate, but its
problem is that the file *no longer has* a problem. A squiggle would point at a line that is
now correct.

Every diagnostic carries its check id as `code` and its rationale as hover, so `--explain`
is available without a second command.

### Why lint owns the diagnostics

`graph-check`'s node checks are a subset of lint's, and it carries neither a baseline nor
spans. Measured: on a real 90-node corpus it finds nothing where lint finds 92; on the
reports fixture it reports the *same* broken edge as `dangling-edge` — but with no span, so
rendering both puts one accurate mark on the offending line and a redundant one at line 1.

So lint owns the marks and `graph-check` fills only the gap: any node it objects to that
lint did not mention, plus its own pass/fail as a repository condition. It is a gate CI runs
and its verdict is not dropped.

## The views

Five trees in one activity-bar container, all fed by `--format json`.

| View | Report | |
|---|---|---|
| **Corpus** | `corpus-index` + `open-questions` | classes, then instances. Open questions marked. |
| **Open questions** | `open-questions` | flat. Previously answerable only by reading a REGEN table in a README — which is to say, correct as of the last time somebody ran the generator. |
| **Phases** | `phases` | `ma/*` and `rigpa/*`, grouped. The branch model is the most distinctive thing about these repositories and was invisible in every editor. |
| **Health** | `lint` + `graph-check` + `index-status` + `regen --check` | four gates and one act — see below. |
| **Sangha** | `sangha` | electors, their positions, the settled record. Read-only. |

The Sangha view is hidden unless `sangha --format json` reports `collective: true`, which is
keyed on registered electors rather than on the directory: the template ships `sangha/` with
a placeholder table, so a directory test would show the view in every derived repository
from its genesis commit.

**Read-only is constitutional, not a scoping decision.** Article V confines synthesis to
resolution events, so a surface that wrote a position or drafted a resolution would be
performing one outside the protocol that routes them. RFCs 0009 / 0011 / 0012 have to settle
first.

### Four gates and one act

`graph-check`, `lint`, `index-status` and `regen --check` each answer a verdict, and the
Health view renders it.

REGEN freshness was an act until `yidam regen --check` existed. It could not be a gate while
the only way to answer the question was to rewrite the blocks and see what moved — an
extension is not going to edit your files to render a tick.

**Vendored-prelude drift is still an act**, for a reason that will not go away: it needs the
network. A row claiming the prelude is current without asking the origin would be this
extension asserting something no command answered.

A report that fails to arrive renders as *unavailable* — its own state, not a failure.
Folding it into red would show an X about the corpus because a subprocess died.

Blessing the baseline is offered as the Lint row's action only when the debt is **stale and
nothing is new**. One click that turns fresh violations into inherited debt would make
laundering a regression the easiest thing on the screen.

### Why two cached groups

Measured against a real 105-node corpus with 23 settled resolutions: most reports finish in
under 200 ms, and `phases` takes **1.26 s** — it spawns three git
processes per ref, and a sangha has dozens. So what a *save* can change is one cache, and
what a *ref* can change is another. A sangha edit bumps the ref generation, because that is
the view it moves.

## The commit box

VS Code's commit input is a real text document with language id `scminput`, so language
features register against it.

- **Completion** on the verb, epistemic before operational, each carrying its **When** cell
  as detail text. Offered only while the cursor is before the first `: ` — past it you are
  writing prose, and thirty verbs there is noise.
- **A squiggle** for a verb outside the vocabulary, or one carrying a conventional-commits
  `(scope)` suffix.

Both rules already existed and were already checked — by `yidam lint --commits`, *after* the
commit. GRAPH.md says why moving them matters: the check is Warn severity and correctly so,
since history cannot be rewritten to fix a verb — *"that also means it reports drift only
after the drift is permanent."*

**`(scope)` earns the squiggle on its own.** Everything before the first `: ` is the verb,
so `vendor(yidam):` is read as the verb `vendor(yidam)`. That costs twice: it is outside the
vocabulary, **and** classification falls through to Epistemic, silently filing an operational
commit as a change in understanding. The bootstrap skill prescribed exactly that form, and
every derived repository's first three commits were reported by its own lint.

### Nothing here decides what is legal

The list and the verdict both come from `yidam vocabulary --format json`. Membership is
`is_recognized_verb` and the kind is `classify_commit` — the parity-certified pair, proven
total against the Dafny spec. A hardcoded list in TypeScript would be a second source of
truth for a rule the prelude owns and re-vendoring may change: `resolve`, `scope` and
`adopt` all arrived that way, and the vendored `GRAPH.md` is watched for exactly that.

The severity is transcribed too, read from the `unrecognized-verb` check rather than
chosen. A commit box that squiggled harder than the gate would be asserting a verdict
nobody agreed to — and this is precisely where escalating feels helpful.

### And the "kept in sync" comment became checkable

`git.rs`, `git.py` and `git.ts` each say their constants are *"kept in sync with the commit
vocabulary in `prelude/GRAPH.md`"*. Three comments, three languages, nothing verifying it.
`vocabulary` parses the tables to fetch their **When** prose, so comparing them costs
nothing — `drift` is a field on the report rather than a hope.

## Link navigation

A corpus edge is a filesystem-relative path inside YAML. To VS Code that is an opaque
scalar — no ctrl-click, no completion, no hover.

- **Definition** on `target:` — ctrl-click through an edge.
- **References** — inbound edges to the open node, or to the target under the cursor.
  Nothing surfaced reverse traversal for corpus nodes: `used-by` covers catalog entries only,
  and `orphan-in` reports the *absence* of inbound edges without ever naming the present ones.
- **Hover** — the target's label, class, degree in both directions, and description.
- **Completion** on `relationship:` and on `target:`.
- **New node** — class, label, filename, description, then **its first edge**. Cancelling at
  that step writes nothing: a node with no outgoing edge is a lint error the moment it
  exists, so a command that scaffolded one would be offering to break the gate.

### The CLI resolves the edges

`yidam graph --format json` reports every edge already resolved, with `exists` answered by
the same two lines `dangling_edge` uses. Resolution is `normalize(dir.join(target))` — the
rule that makes the graph a graph — and a client re-deriving it would disagree with the gate
about which edges are broken, silently, in the direction of "looks fine here".

`resolveFrom` exists on this side anyway, because a buffer can be edited after the report
was taken and navigation has to work in it. A contract test runs it against **every edge the
binary resolved** so the two cannot come apart. Definition offers its location without an
existence check of its own: when the path is wrong VS Code says the file cannot be opened,
which beats a second answer to a question `lint` already gates on.

### The ontology is a guide, not a closed list

Measured on a live derived repository at 90 nodes and 299 edges: **17 (class, relationship)
pairs in use are not declared as `out` edges**, and one of them — `instance-of`, the edge
every node carries to its own `.ont.yml` — is used by every class and declared by none.
Nothing lints relationships against the ontology, so a list restricted to declared edges
would be stricter than any rule in the system and would omit the corpus's most-used
relationship.

So: declared first, in-use beside them, and the reason each is offered in its detail text.
The opposite of the commit vocabulary, which *is* closed and *is* gated — and that difference
is why one gets a squiggle and this does not.

**One relationship may be declared against several classes.** Three of that repository's
classes do it (`maneuver -[operates-on]->` legislation, ballot-measure, election). Reading
only the first declaration offered a third of the legal targets and hid the rest, which
reads to a user as "those nodes do not exist".

## The neighbourhood panel

The open node at depth 1–2 — outgoing and incoming edges alike, grouped by hop then
direction then relationship. Click a neighbour to open it.

`export --format web` already serves reading and retrieval, and serves them better. What it
structurally cannot do is show the neighbourhood of a node *in flight*: it is a built
artifact of a committed corpus, and the node under the cursor is neither.

**The traversal is not in TypeScript.** `yidam neighbors --format json` performs it, and it
is the same function `serve --mcp`'s `neighbors` tool calls — that traversal moved out of
`serve/tools.rs` and into the light build for this, so the two surfaces answer identically
rather than similarly. A light binary could not have answered the question at all before.

Direction is part of the grouping key rather than a column: at hops > 1 it is relative to
the node the edge was reached *from*, so mixing the two under one arrow would make the arrow
a lie. A reached target that is not a corpus node — an `.ont.yml`, or a broken edge — is
shown unlinked rather than hidden; hiding it would make the panel disagree with the graph.

### Offline, and what that cost

`default-src 'none'`, a per-render nonce for the one inline style and the one inline script,
and nothing else — no host, no scheme, no `unsafe-inline`. A test asserts the rendered bytes
reference no external URL, because the way this regresses is somebody adding one convenient
`<link>`.

Spacing and radii are transcribed from `yidam/design/tokens/` and drift-tested. Two
deviations, both stated:

- **Colour is the reader's theme.** The design system has no dark mode outside the claim
  triad, and a light card inside a dark editor is worse than one that is not brand-coloured.
- **Fonts are the editor's.** `yidam/design/tokens/fonts.css` is an `@import` from the Google
  Fonts CDN, so the brand font layer is unusable on a surface required to be offline. A test
  pins that reason rather than leaving it in a comment.

## Three guards

### Claim tags

`[verified]` / `[inference]` / `[open]` tinted inline, from the design system's claim
palette. `docs/aesthetic-direction.md` already calls these "first-class visual states"; this
is the first surface that makes them so.

**Matched wherever `count_in_source` counts** — exact bracketed tokens, anywhere in the
file. Not "inside a `description:` block", which is the narrower reading: the corpus records
absence in properties too (`estimate: "[open] — not computed"`), and a decoration that
skipped those would disagree with `yidam status` about what a claim is. That disagreement is
invisible and permanent.

Off in high-contrast themes whatever the setting says. A high-contrast theme is a stated
accessibility choice, and tinting text against it overrides a decision the reader made
deliberately.

The palette is transcribed from `yidam/design/tokens/colors.css`, and a test parses that
file and fails when the copy drifts. The dark triad was added there for this feature and
*derived* rather than picked: the light theme's border tone becomes the dark foreground,
because it is already the mid-lightness member of each triad and therefore the one legible
against both grounds.

### The read-only vendor guard

`.yidam/.vendor/` is read-only, and an edit there "is silently discarded on the next update"
(`sadhana/root/AGENTS.md`). **Nothing enforced it.** The failure is quiet and delayed: the
edit works locally, survives review, and disappears at the next
`mise run yidam-vendor-update`.

`files.readonlyInclude` at workspace scope, applied at activation, idempotent — activation
runs on every window, and a guard that rewrote the setting each time would put a settings
diff in every session's git status. Turning `yidam.vendor.protect` off *removes* the entry
rather than setting it `false`, so the setting is left as it was found.

### Schema wiring

`yidam schema --settings` works, and it is the entire editor story today: a third-party
extension, a manual step at genesis, and a second manual step whenever the schema set
changes. The copy-paste becomes a notification with a button.

**Merged, never replaced.** `yaml.schemas` is somewhere people put their own mappings, and
an "apply" that overwrote them would be a worse failure than the copy-paste it replaces.

### And the tasks

`regen`, `graph-check`, `graph-lint`, `embed`, `index-build` — as palette commands and as a
`TaskProvider`, so they reach `Run Task` too. Deliberately not every task in
`mise.yidam.toml`: it carries a dozen REGEN generators that `regen` runs in one pass, and
offering them individually re-creates the two-lists problem `yidam regen` exists to have
solved.

## Layout

| Path | |
|---|---|
| `src/binary.ts` | resolution. No `vscode` import. |
| `src/handshake.ts` | reading `format_version` and the `yidam` block. No `vscode` import. |
| `src/reports.ts` | typed views of the contract. Transcription only — nothing is derived here. |
| `src/diagnostics.ts` | the severity mapping. No `vscode` import. |
| `src/runner.ts` | spawn, per-OID cache, debounce. No `vscode` import. |
| `src/report-run.ts` | one pass over the reports, in two cached groups. No `vscode` import. |
| `src/tree/model.ts` | the five views, as data. No `vscode` import. |
| `src/vocabulary.ts` | what to offer, where to underline. No `vscode` import. |
| `src/graph.ts` | scalar reading, path arithmetic, completion ranking. No `vscode` import. |
| `src/neighborhood.ts` | grouping and the webview's HTML. No `vscode` import. |
| `src/claims.ts` | claim tokens, their spans, the palette. No `vscode` import. |
| `src/settings.ts` | what to write into settings, and when not to. No `vscode` import. |
| `src/tree/provider.ts` | `TreeNode` → `vscode.TreeItem`. The adapter, with no judgement in it. |
| `src/extension.ts` | activation, status bar, terminal actions. Thin by design. |
| `test/` | `node --test`, no Electron |

Every module that carries logic imports nothing from `vscode`, so they are exercised by
plain node. `test/contract.test.ts` drives them against the same `reports/` golden corpora
that certify the CLI's own output: a fixture whose output changes fails the parity run and
these tests together.

## Running the tests

```
mise run ci-vscode          # compile, lint, unit tests — what CI runs
npm run test:unit           # from this directory
```

Tests that need a binary **skip** when none resolves, or when the one that resolves predates
the contract — a stale `yidam` in `~/.cargo/bin` should not turn a contributor's suite red
over somebody else's work. CI sets `YIDAM_REQUIRE_CONTRACT=1`, which turns both skips into
failures, so skipping can never be how this job goes green.

## Not here yet

`@vscode/test-electron`. The `vscode`-importing code is the status bar, the diagnostic
collection, and one `TreeDataProvider` adapter with no judgement in it — every shape
decision is settled in `src/tree/model.ts` against plain data, and asserted there.
`test/manifest.test.ts` catches the class of defect that actually needs an editor to
surface — a `main` pointing at nothing, a contributed command nobody registers, a view id
nothing provides — in plain node.
