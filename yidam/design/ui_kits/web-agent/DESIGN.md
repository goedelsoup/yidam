# Web agent — offline browser agent UI

Design specification for the static, offline-capable browser agent produced by
`yidam export --format web` (#39/#40). A single page loads a `.yiz` bundle,
embeds queries in-browser (transformers.js), searches the packed vector index,
and — when WebGPU is available — drives a WebLLM model for RAG-grounded
generation over the retrieved nodes.

The reference implementation lives in `yidam/cli/assets/web/` and is emitted
by the exporter; this document is the contract it follows. The sibling
prototype `../corpus/index.html` establishes the visual language this design
extends (sidebar-free here: the agent is a focused search surface, not the
full repo browser).

---

## Layout grid

Desktop-first, 1280px+ viewport. A single 3-region column layout — no sidebar
navigation; search is the navigation.

```
┌──────────────────────────────────────────────────────────────┐
│ header      logo · domain name · provenance line             │ 56px
├──────────────────────────────────────┬───────────────────────┤
│                                      │                       │
│ main column (flex 1, max 720px,      │ right panel (400px,   │
│ centered when panel closed)          │ slides in/out)        │
│                                      │                       │
│   search bar                         │   node detail         │
│   result cards                       │   — or —              │
│                                      │   chat thread         │
│                                      │                       │
├──────────────────────────────────────┴───────────────────────┤
│ status bar  domain · nodes · index model · webllm status     │ 32px
└──────────────────────────────────────────────────────────────┘
```

- Main column: `max-width: 720px`, `margin-inline: auto`, gutter `--space-8`.
- Right panel: fixed `400px`, `border-left: 1px solid var(--border-ui)`,
  slides with `--motion-slow` (deliberateness over snappiness).
- Below 1024px the right panel overlays instead of docking. Mobile is
  explicitly out of scope; nothing may *break* below 768px, but no layout
  effort is spent there.

## The seven states

### 1 · Empty / load

Centered column, generous whitespace. Logo mark, one serif sentence
("A domain computer. Load a bundle to search its knowledge graph."), then a
drag-and-drop target (dashed `--border-ui-strong`, `--radius-xl`, hover →
`--border-focus`) with a ghost-button alternative ("Load bundle…" file
picker). When the exporter co-locates `bundle.yiz`, this state is skipped:
the page auto-fetches and goes straight to state 2. The drop target remains
the fallback when auto-fetch fails (e.g. opened via `file://`).

### 2 · Loading

Same centered column; the drop target is replaced by a load sequence list.
Each stage is one row: label (ui font), progress (mono), state glyph.

```
bundle          fetched · 2.1 MB
decompress      done
vector index    1,204 rows · 384 dims
corpus          1,204 nodes parsed
embedding model downloading · 14.2 / 22.6 MB    ← only on first visit
```

Stages complete top-to-bottom; the list stays visible until the embedder is
ready, then the whole column cross-fades to state 3. Never a blank screen,
never an indeterminate spinner as the only signal. Bundle parsing yields to
the event loop between tar entries so the page stays responsive at 10K nodes.

### 3 · Ready / search

Primary state. Search bar at the top of the main column (full width, 44px,
`--surface-raised`, focus ring `--border-focus`). Under it, result cards
(component below). Before the first query, the card area shows the corpus at
a glance: class chips with counts, and the open-question count as an `[open]`
badge. Query embedding + cosine over the in-memory index is synchronous and
must feel instant (<50ms typical); a 250ms debounce absorbs typing.

Status bar (bottom, fixed): `domain · N nodes · index: <model_id> · webllm:
ready|unavailable|off`. Mono, `--text-tertiary`, 11px.

### 4 · Node detail panel

Slides in from the right when a result card (or link chip, or source chip)
is clicked. Contents top-to-bottom:

- breadcrumb path (mono, tertiary): `corpus / <class> / <name>`
- node label as display-font heading; `?`-prefixed labels keep the `?`
  (open questions are first-class, not decorated away)
- class badge + `[verified]`/`[inference]`/`[open]` markers found in the body
- full node content in serif, 16px/1.8 (YAML source rendered as definition
  list; unknown fields shown, not hidden)
- **link minimap**: one-hop SVG star — this node centered, outgoing edges
  right (gold arrows), incoming edges left. Click any neighbor to re-center.
- "Ask about this node" ghost button — seeds the chat input with the node
  label and switches the panel to chat (only when WebLLM is available).

### 5 · Chat (WebGPU available)

Same right panel, tab-switched with node detail ("detail | chat", underline
tabs). Before first use, an explicit consent card: model name, download size
("Llama-3.2-1B · ~880 MB download, cached by your browser"), a gold "Download
and enable" action and a ghost "Not now". No download starts without consent.

Thread: user turns right-aligned ui font; assistant turns serif, streamed.
Below each assistant turn, a **sources row**: node chips (label + class
badge) for every node passed as context — click opens detail. Grounding is
visible, always.

### 6 · Chat (WebGPU unavailable)

Same panel position — named, not hidden: a quiet card reading "On-device
generation requires WebGPU. Retrieval is fully available — use the results
to explore the corpus." `--surface-overlay`, no error color: absence of a
capability is not a failure state. No disabled inputs, no greyed ghosts.

### 7 · Graph view

Full-screen overlay (ghost button "graph" in the header). Force-directed
layout of the whole corpus: nodes colored by class (cycling ink/gold/rigpa/ma
family hues), edges as 1px gold at 40% opacity, arrowheads only at hover.
Class filter chips top-left; click a node → detail panel over the graph.
Escape closes. At >2K nodes, render labels only at hover. This state is
optional in the first implementation — the exporter may ship without it, but
the header button must then be absent entirely (never present-but-broken).

## Component inventory

| Component | Used in | Notes / DS mapping |
|---|---|---|
| Search bar | 3 | forms input tokens; debounced |
| Result card | 3 | `NodeCard` lineage: label, one-line description, class badge, similarity score (mono, right), link counts |
| Class badge | 3, 4, 5 | `Badge` component, default variant |
| Claim marker | 4 | `ClaimMarker` — `[verified]` / `[inference]` / `[open]` |
| Load stage row | 2 | label + mono progress + glyph |
| Drop target | 1 | dashed border, full-column |
| Detail panel | 4, 5, 6 | 400px right dock; tabs when chat exists |
| Link minimap | 4 | one-hop SVG star, gold edges |
| Chat turn | 5 | user (ui font) / assistant (serif, streamed) |
| Source chip | 5 | node label + class; opens detail |
| Consent card | 5 | model size disclosure before download |
| Status bar | 3–6 | mono, fixed bottom |
| Banner | 6, errors | `--surface-overlay`, never alarm-red for capability absence |

## Loading / progress UX detail

| Stage | Signal shown | Failure behavior |
|---|---|---|
| fetch bundle | byte count | fall back to state 1 drop target (expected under `file://`) |
| gunzip + untar | per-entry yield, entry count | error banner with the tar offset; bundle likely corrupt |
| arrow parse | row count · dims | "index absent" → keyword-only mode, stated in status bar |
| corpus parse | node count | — |
| embedder init | download MB progress (first visit), then "cached" | "embedding model unavailable" → keyword search, stated plainly |
| webllm init | consent first; then staged progress from the engine | banner per state 6 |

Degraded retrieval (no index or no embedder) is labeled in both the status
bar and each response payload (`"degraded": true`) — honesty at the edges.

## Token mapping

| Role | Token |
|---|---|
| Page background | `--surface-base` |
| Cards, panels, search bar | `--surface-raised` + `--border-ui` + `--shadow-xs` |
| Primary action (consent, load) | `--action-bg` / `--action-fg` |
| Ghost actions | `--action-ghost-*` |
| Node label headings | `--font-display`, weight 400 |
| Node body text | `--font-serif`, 16px / 1.8 |
| Paths, scores, hashes, status bar | `--font-mono`, `--text-tertiary` |
| UI chrome | `--font-ui`, 12–13px |
| Similarity score, edges | `--gold-500` family |
| Claim markers | `--status-{verified,inference,open}-*` |
| Focus | `--border-focus` |
| Panel motion | `--motion-slow` |

Sentence case everywhere. No emoji; `→ · —` where semantically earned. Copy
register per the design system: precise, unhurried, no marketing.
