# RFC-0016 — An editor surface for derived repositories (`yidam` for VS Code)

- **Status:** Implemented
- **Track:** I11
- **Relates to:** RFC-0001 (the report contract this consumes), RFC-0003 (the light binary it
  depends on), RFC-0004 (drift detection it surfaces), RFC-0005 (the sibling agent surface),
  RFC-0014 (`yidam rename`, which becomes F2), RFC-0015 (`--format json` convention)
- **Versioning layers touched:** tooling (`yidam` CLI + a new editor client) —
  [Layer 4](../../VERSIONING.md#layer-4--tooling), declared in answer to this RFC
- **Downstream reference case:** Project BOSC (watermark-directory)

## Summary

Every rule a yidam-derived repository enforces — the graph gate, the thirteen lint checks, the
closed commit vocabulary, claim tags, REGEN freshness, the read-only vendor boundary — is
enforced at commit time or in CI. None of it is visible while a contributor is writing the node
that breaks it. This RFC proposes a VS Code extension that moves those rules to the point of
authorship, and — more importantly — the CLI work that has to land first for such an extension
to exist without repeating the mistake the whole RFC set is about.

The load-bearing claim: **the extension is not the first deliverable.** The reports render prose
to stdout and nothing else. An extension built today would either scrape rendered text or
re-implement the reports in TypeScript, and the second is precisely what BOSC did in Python —
~1,600 lines, already drifted, documented in [the RFC index](README.md). Phase 0 is therefore a
JSON report contract in the Rust CLI. The extension is a consumer of it, and the boundary is
absolute: **TypeScript may compute affordances; only the CLI may compute verdicts.**

## Problem

### The rules are enforced everywhere except where the work happens

The derived-repo scaffold is explicit about what a contributor must run before committing —
`mise run graph-check`, `mise run graph-lint`, `mise run regen`
([`sadhana/root/CLAUDE.md`](../../sadhana/root/CLAUDE.md)) — and CI re-runs all three plus the
commit-vocabulary check and a REGEN staleness diff
([`sadhana/github/workflows/ci.yml`](../../sadhana/github/workflows/ci.yml), the `corpus` job).
That is a correct gate and a poor feedback loop. A contributor writes a node, links it to a
target that does not exist, saves, writes four more, and learns about the first one several
minutes later from a red build — by which point the working memory that would have made the fix
cheap is gone.

The rules themselves are unusually well specified, which is what makes this worth building.
Thirteen checks with stable ids and three severities live in
[`lint/checks.rs`](../../yidam/cli/src/cmd/lint/checks.rs); each is a `Check` carrying `id`,
`title`, `severity`, `rationale`, and a `Vec<Violation{node, detail}>`
([`lint/model.rs:44-74`](../../yidam/cli/src/cmd/lint/model.rs#L44-L74)). This is already an
editor diagnostic in all but serialization.

### The reports render prose, not data

There is no `--format json` on any command. The full surface is 26 subcommand variants, one feature-gated
([`main.rs:18-146`](../../yidam/cli/src/main.rs#L18-L146)) and every report writes human text:
`graph-check` and `lint` print findings and exit nonzero, `corpus-index` and `catalog-audit`
rewrite REGEN blocks in place, `status` and `phases` print tables. The only structured output in
the tree is written *to disk* for other purposes — `embed.config.json`, `meta.json`, bundle
manifests, the MCP server's own JSON-RPC frames.

Two RFCs already assume the convention exists. RFC-0015 specifies `yidam log --format json`
"consistent with the report-JSON convention the set uses elsewhere (RFC-0001, RFC-0004)", and
RFC-0001 makes structured issue lists — `[[issue]] node/kind/message` — "the load-bearing
contract" for parity fixtures. The convention is referenced twice and implemented zero times.

### Edges are strings the editor cannot see through

A corpus link is a filesystem-relative path inside YAML:

```yaml
links:
  - target: ../concept/confounding.yml
    relationship: depends-on
```

resolved as `dir.join(target)` against the instance's own directory
([`corpus.rs:110-112`](../../yidam/cli/src/cmd/corpus.rs#L110-L112)). To VS Code that is an
opaque scalar: no ctrl-click, no completion, no squiggle on a typo, and — the expensive one — no
rename. The docs warn three separate times that renaming a node severs edges, and RFC-0014
exists because the manual repair is unreliable. The editor is where renames actually happen, and
it is currently the one place with no guard at all.

### A naive diagnostics port would be wrong, and then ignored

`yidam lint` does not ask *is the corpus clean?* It asks *did this change make it less clean?*,
gating against `.yidam/lint-baseline.yml`
([`lint/mod.rs`](../../yidam/cli/src/cmd/lint/mod.rs), and the rationale in its module doc:
conflating the two "produces a gate that is either permanently red or permanently ignored").
Two distinct things fail it — a violation not in the baseline, and a baseline entry that no
longer occurs ([`lint/mod.rs:626-660`](../../yidam/cli/src/cmd/lint/mod.rs#L626-L660)).

An extension that renders every finding as an Error reproduces exactly the failure the baseline
was designed to prevent, one layer up: a Problems panel permanently full of inherited debt is a
Problems panel nobody reads. The severity a diagnostic gets must be a function of *baseline
membership*, not of check severity alone. And the stale-baseline failure is not a per-file
diagnostic at all — it is a repository-level condition with a repository-level fix.

### The vendor boundary is honor-system

`.yidam/.vendor/` is read-only, and an edit there "is silently discarded on the next update"
([`sadhana/root/AGENTS.md`](../../sadhana/root/AGENTS.md)). Nothing enforces this. The failure
is quiet and delayed: the edit works locally, survives review, and disappears at the next
`mise run yidam-vendor-update`. VS Code has had a one-line fix for this since 1.74
(`files.readonlyInclude`) that no derived repo sets.

### The one editor integration that exists is a copy-paste

`yidam schema --settings` prints a `yaml.schemas` mapping for the user to paste into
`.vscode/settings.json` ([`schema.rs:195-232`](../../yidam/cli/src/cmd/schema.rs#L195-L232)).
It works, and it is the whole of the editor story. It requires a third-party extension, a manual
step at genesis, and a second manual step whenever the schema set changes. The doc comment on
`editor_settings` names Neovim and Helix as targets too — a signal worth keeping (see Phase 3).

## Proposal

### The boundary

> **TypeScript computes affordances. The CLI computes verdicts.**

An affordance is a navigation or authoring convenience whose failure mode is *not helping*:
go-to-definition, completion, a hover, a decoration. A verdict is a statement about whether the
corpus is sound: every lint finding, every graph-check issue, every commit-vocabulary judgement,
every open-question classification. Verdicts cross the process boundary as JSON from the pinned
binary; the extension renders them and never derives them. When the extension cannot reach the
binary it says so and shows nothing, rather than falling back to a second opinion.

This is the rule the RFC set exists to establish, applied to its next consumer before that
consumer is written.

### Phase 0 — the JSON report contract (CLI, Rust)

The prerequisite. Nothing in later phases is worth starting first.

Add `--format text|json` (default `text`, byte-identical to today) to `lint`, `graph-check`,
`status`, `open-questions`, `corpus-index`, `catalog-audit`, `phases`, and `diff`. The lint data
model already has the right shape; this is largely `#[derive(Serialize)]` plus an emitter.

```jsonc
// yidam lint --format json
{
  "format_version": "1",
  "yidam": { "version": "0.1.0", "commit": "bf7d203", "features": ["reports"] },
  "root": "/abs/path/to/repo",
  "gate": {
    "passed": false,
    "new_violations": 2,          // not in the baseline — these fail CI
    "baselined_violations": 41,   // inherited debt — these do not
    "stale_baseline_entries": [   // listed but no longer occurring — these also fail CI
      { "check": "orphan-in", "node": ".yidam/corpus/concept/tailwater.yml" }
    ]
  },
  "checks": [
    {
      "id": "dangling-edge",
      "title": "Link target does not resolve",
      "severity": "error",
      "rationale": "An edge to a file that does not exist is not an edge...",
      "violations": [
        {
          "node": ".yidam/corpus/concept/low-flow.yml",
          "detail": "broken link: ../concept/assimilative-capacit.yml",
          "in_baseline": false,
          "span": { "line": 14, "col": 15, "end_line": 14, "end_col": 52 }
        }
      ]
    }
  ]
}
```

Three constraints on that shape, each of which the naive version gets wrong:

1. **`in_baseline` is per violation, not per check.** It is what determines diagnostic severity
   downstream. Without it the extension cannot tell debt from regression, and the panel becomes
   noise.
2. **`span` must not enter the baseline's identity.** The baseline compares on
   `(check id, node)` — deliberately, and the field docs say so
   ([`lint/model.rs:44-51`](../../yidam/cli/src/cmd/lint/model.rs#L44-L51)). Adding a line
   number to the *comparison* would make the baseline churn on every edit above a violation.
   `span` is output-only, best-effort, and omissible; a violation with no span anchors at the
   file's first line.
3. **`format_version` and the `yidam` block are the handshake.** The extension is versioned
   independently of the binary a given repo pins in `.yidam.toml`. It must detect skew and
   degrade loudly, never mis-parse. An unknown major `format_version`, or a binary predating the
   flag, disables verdict features and says why.

Also in Phase 0, because they are cheap and the extension needs them:

- `yidam log [--epistemic|--operational] [--format json]` — RFC-0015 as written. The extension's
  history view is its natural front end, and the classifier is already parity-certified.
- `yidam lint --format json --path <file>` — single-file scoping. The reports walk the whole
  corpus per run, which is fine at hundreds of nodes and is not fine on every save at thousands.
  Ship whole-corpus first; add scoping when a real corpus makes it necessary, not before.

**Cost:** small. The types exist, the checks exist, the baseline diff exists. This is
serialization plus a flag, with golden fixtures that fold into RFC-0001's `reports/` family.

### Phase 1 — the thin client

A VS Code extension at `yidam/editors/vscode/`, activating on the presence of `.yidam/` or
`.yidam.toml` in any workspace folder.

**Binary resolution**, in order, decided once at activation and re-checked on
`.yidam.toml` change: `yidam.path` setting → `yidam` on `PATH` → mise shim resolved from the
workspace → not found. Not found is a first-class state with one action — run
`mise run yidam-build` in a terminal — and every verdict feature disabled until it resolves.
The extension never bundles, downloads, or builds a binary; the repo's provenance model says
which commit governs this corpus, and only `.yidam.toml` gets to say it. This is the point at
which RFC-0003's publishable light binary stops being a nicety: `cargo install` from a cloned
pin ([`mise.yidam.toml`, `[yidam-build]`](../../mise.yidam.toml)) is a poor first-run experience
for someone who just opened a folder.

**Diagnostics.** `lint --format json` and `graph-check --format json` on save, on git ref change,
and on demand, debounced, results cached per git OID. Mapping:

| Condition | VS Code severity |
|---|---|
| `severity: error`, `in_baseline: false` | Error — this is what fails CI |
| `severity: warn` \| `info`, `in_baseline: false` | Warning / Information |
| any severity, `in_baseline: true` | Hint, tagged `Unnecessary`-style faded, source `yidam (baseline)` |
| `stale_baseline_entries` | not a diagnostic — a Health-view item with a **Bless baseline** action |

Every diagnostic carries its check `id` as `code` and its `rationale` as the hover, so
`--explain` is available without a second command. `yidam.lint.showBaselined` (default `true`,
as Hints) lets a repo with heavy inherited debt quiet them.

**Views** — one `yidam` activity-bar container:

- **Corpus** — classes → instances, open questions marked, from `corpus-index --format json`.
- **Open questions** — flat, click to open. The single most-asked question of a research repo.
- **Phases** — `ma/*` and `rigpa/*` branches with owner, start date, commit count, from
  `phases --format json`; checkout on click. The branch model is the most distinctive thing
  about these repos and is currently invisible in every editor.
- **Health** — graph gate, lint gate, REGEN freshness, index freshness, vendor drift; each row
  an action (`Regen`, `Bless baseline`, `Re-vendor`, `Build index`).
- **Sangha** — electors, positions, resolutions; rendered only when governance is collective.

**Guards and wiring:**

- Apply `files.readonlyInclude` for `.yidam/.vendor/**` at activation (workspace scope,
  idempotent, reversible via setting). The honor-system boundary becomes an enforced one.
- Offer to apply `yidam schema --settings` when `.yidam/schemas/` exists and the mapping is
  absent or stale — the copy-paste step becomes a notification with a button.
- Contribute tasks for the `mise.yidam.toml` task layer so `regen`, `graph-check`, `graph-lint`,
  `embed`, `index-build` are reachable from the command palette.
- Status bar: `N nodes · M open · index 3 commits stale`, click → Health.

**Commit vocabulary in the SCM box.** VS Code's commit input is a real text document with
language id `scminput`. Register a completion provider offering the closed verb list from
`GRAPH.md`, and a diagnostic for a subject that carries a `(scope)` suffix or an unlisted verb —
the rule stated in [`sadhana/root/AGENTS.md`](../../sadhana/root/AGENTS.md) and checked by
`yidam lint --commits` today, moved to before the commit rather than after it. The verb list is
read from the vendored `GRAPH.md`, not hardcoded — it is a closed vocabulary that the prelude
owns and re-vendoring may change.

**Claim tags.** Decorate `[verified]` / `[inference]` / `[open]` in markdown and in YAML
`description` blocks, using the design system's existing claim tokens (`--verified-*`,
`--inference-*`, `--open-*`; see
[`colors-claim.card.html`](../../yidam/design/guidelines/colors-claim.card.html)). The aesthetic
direction already calls these "first-class visual states"; this is the first surface that makes
them so. Theme-aware, and off by default in high-contrast themes.

### Phase 2 — navigation and authoring

Affordances only, per the boundary. All of it is `dir.join(target)` and a file-existence check —
navigation, not judgement. When it is wrong you fail to jump; the verdict still comes from
`lint`.

- **Definition / references** on `target:` scalars — ctrl-click through an edge; find all inbound
  edges to the open node (the reverse traversal `used-by` gives catalog entries and nothing gives
  corpus nodes today).
- **Completion** on `target:` — existing instances, class-filtered where the ontology constrains
  the relationship; on `relationship:` — verbs declared in the relevant `.ont.yml`.
- **Hover** on a link — the target's `label` and `description`, so an edge can be checked without
  leaving the node.
- **New node** command — pick class, name, label; scaffold from the `.ont.yml` property list;
  require an outgoing link before the file is written, because a node with no edge is a lint
  error the moment it exists.
- **Neighborhood webview** — the open node at depth 1–2, same semantics as the MCP `neighbors`
  tool, styled from `yidam/design/tokens/`, fully offline (no CDN; the repo's CI is hermetic and
  the extension should hold the same line).

### Phase 3 — `yidam serve --lsp`, and rename

Phase 2's affordances are TypeScript because they are cheap and harmless. The moment the editor
needs to make a *judgement* incrementally — live diagnostics as you type rather than on save,
and above all **rename** — that logic must not be in TypeScript.

Add `yidam serve --lsp`, a sibling to the existing `serve --mcp`, in the light `reports` feature
set. Note that `serve` is today gated behind the heavy `index` feature
([`lib.rs:23`](../../yidam/cli/src/lib.rs#L23)); LSP needs none of fastembed, lancedb, or protoc
and must not inherit that gate.

The prize is rename. RFC-0014 proposes `yidam rename` as an atomic operation with a dangling-edge
gate; `textDocument/rename` is its natural trigger. F2 on a node, every inbound `target:`
rewritten in one transaction, refused outright if any edge would dangle. That closes the failure
the docs warn about three times and currently guard zero times.

And it is how Neovim and Helix get all of this for free — users the codebase already names in
[`schema.rs:191-194`](../../yidam/cli/src/cmd/schema.rs#L191-L194). The VS Code extension becomes
a thin LSP client plus the views, decorations, and SCM integration that are genuinely
VS Code-shaped.

### Where it lives, and how it is tested

**In this repository**, at `yidam/editors/vscode/`, a sibling of `yidam/cli` and `yidam/web`.
The extension and the JSON contract it consumes must version together; a separate repository
re-creates, between the extension and the CLI, exactly the drift this RFC set exists to close.
The vendor step copies only `yidam/prelude/`, so derived repos carry none of it.

- Its own `mise.toml` subproject; a `vscode` job in [CI](../../.github/workflows/ci.yml) running
  compile, lint, and `@vscode/test-electron`.
- Integration fixtures are RFC-0001's `reports/` golden corpora, reused: the same trees that
  certify the reports become the repos the extension is driven against. A fixture whose golden
  output changes fails both the parity run and the extension tests, which is the point.
- Published to the VS Code Marketplace and Open VSX as `goedelsoup.yidam`.

### What this deliberately is not

- Not a re-implementation of any report in TypeScript.
- Not a bundled, downloaded, or auto-built binary — `.yidam.toml` decides which yidam governs a
  corpus.
- Not a general markdown-graph tool. Foam and Dendron cover backlinks and wiki-links well. The
  value here is the semantics they cannot know: claim tags, the closed commit vocabulary, the
  baseline ratchet, epistemic-vs-operational commits, `ma/` and `rigpa/`, vendor provenance.
- Not a bootstrap wizard. Genesis is an ontology dialogue with an agent
  ([`BOOTSTRAP.md`](../../BOOTSTRAP.md)); a form that skips the dialogue produces exactly the
  unconsidered ontology the dialogue exists to prevent. The extension may offer `yidam clone` and
  `yidam overlay` — scaffolding — and stops there.

## Migration & compatibility

Nothing in a derived repository changes. Phase 0 is additive: `--format text` remains the
default and remains byte-identical, so CI, REGEN blocks, and every existing consumer are
untouched. The extension is opt-in per user, never required, and a repo whose contributors do
not install it behaves exactly as it does today — CI is still the gate of record.

Version skew is handled by the handshake, not by hope: `format_version` plus the binary's own
version and feature list, checked at activation and on `.yidam.toml` change. Older binaries mean
degraded features and a stated reason. RFC-0003's light binary and RFC-0004's `yidam sync` both
make this materially better and neither blocks Phase 1.

## Alternatives considered

- **Extension first, scrape the rendered text.** Rejected. The output is prose designed for
  people and is free to change; a scraper makes it a frozen contract nobody agreed to, and the
  first reworded message ships a silently wrong Problems panel.
- **Re-implement the reports in TypeScript.** Rejected, emphatically. This is BOSC's ~1,600 lines
  in a different language, in the repository whose RFC index documents that as the failure to
  avoid. It would also be the *second* re-derivation of `classify_commit` alone.
- **LSP first, skip the thin client.** Rejected as the opening move, not on merit. `serve --lsp`
  is the better long-run architecture and Phase 3 commits to it. But roughly half the value here
  — tree views, SCM completion, task wiring, the readonly guard, status bar — is not LSP-shaped
  at all, and the JSON contract is needed either way. Ship the cheap surface that expresses the
  system's promise before the expensive one that supports it, on RFC-0015's own reasoning.
- **A web app instead of an editor.** Rejected as a substitute; `export --format web` already
  exists and serves reading and retrieval. It cannot put a squiggle under the link you just
  mistyped, which is where the friction actually is.
- **Ship it as a Claude Code plugin / MCP-only surface.** Rejected as a substitute. `serve --mcp`
  already gives agents the corpus (RFC-0005). This RFC is about the *human* in the editor, and
  the two surfaces are complementary — both are consumers of the same contract.

## Open questions

- ~~**Which layer versions this?**~~ **Settled.** [VERSIONING.md](../../VERSIONING.md) now
  declares **Layer 4 — Tooling**, holding the CLI and the editor client as independently-tagged
  artifacts joined by `format_version` — the same arrangement Layer 2 already uses for three SDK
  packages joined by `parity/VERSION`. The extension's version is independent of the CLI's, and
  neither is what the two negotiate on.
- **Does `span` belong in the contract at all,** or should the extension resolve ranges itself
  from the YAML? Client-side resolution keeps the baseline's identity obviously clean and costs
  nothing in the CLI; server-side is more accurate for checks whose subject is not a literal in
  the file (`orphan-in`, `catalog-used-by-drift`). Lean: optional and best-effort server-side,
  client-side fallback — but this is genuinely unsettled.
- **Should `lint` gain `--path` scoping in Phase 0 or on demand?** Whole-corpus-per-save is
  correct and fast at the scale any current corpus has reached. Adding scoping speculatively
  risks a second code path whose findings differ from the gate's — which would be a verdict
  computed twice, the exact thing this RFC forbids.
- **How much of the sangha belongs in an editor?** Positions and resolutions are governed by
  [the constitution](../../yidam/prelude/CONSTITUTION.md) and Article V limits what synthesis may
  do. A read-only Sangha view is clearly safe. Anything that *writes* a position or drafts a
  resolution from the editor needs the governance RFCs (0009, 0011, 0012) settled first, and is
  out of scope here.
- **Multi-root workspaces.** A workspace holding two derived repos, or a derived repo nested in a
  larger monorepo, needs a per-folder binary and per-folder diagnostics. Straightforward but not
  free, and worth deciding before the first release rather than after.
