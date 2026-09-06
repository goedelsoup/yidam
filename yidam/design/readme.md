# yidam Design System

> What you commit to shapes you.

This design system serves yidam and the family of yidam-derived applications — living knowledge artifacts maintained collaboratively by humans and agents through git repositories.

**Sources consulted:** This design system was built entirely from the yidam design brief (provided as a text document). No external codebase, Figma file, or UI screenshots were available at time of authoring. All visual decisions are derived from the brief's aesthetic direction and vocabulary.

---

## What yidam is

A yidam-derived repository is a **living knowledge artifact** — a structured, evolving body of knowledge maintained collaboratively by humans and agents through a git repository. The repository's git history *is* the knowledge graph: every file is a node, every commit is an event, every markdown link is a directional edge.

The system encompasses several surfaces:
- **Bootstrap flow** — onboarding dialogue for new derived repositories
- **Corpus browser** — web interface for navigating the knowledge graph
- **Sangha resolution flow** — collective synthesis of individual positions
- **Domain computer** — connectors and calculators for domain-specific computation

---

## Design Philosophy

**Restraint over ornamentation.** Every element has a reason.

**Slow over fast.** The system values deliberateness, not velocity. Motion is not decorative.

**Legibility of history.** The graph's past is as important as its present state.

**Equality between participants.** The UI does not privilege one elector's view over another. Human and agent electors are visually equal.

**Honesty at the edges.** Uncertainty is labeled, not hidden. `[open]`, `[inference]`, `[verified]` are first-class visual states.

**Synthesis as a first-class act.** Edges and connections deserve the same visual weight as nodes.

---

## Content Fundamentals

**Register:** Serious, contemplative, precise. Neither clinical nor warm. The tone of a careful scholar.

**Voice:** Third person for system documentation. Direct address ("you") only in onboarding and confirmation flows.

**Technical vocabulary** is used exactly. "corpus", "node", "edge", "elector", "sangha" are proper terms, not metaphors. Never casually paraphrase them.

**Commit messages are testimony** — not changelogs. They name what changed in the world of knowledge, not what files were edited.

**Uncertainty is explicit.** Use "suggests", "may indicate", "according to [source]" — never overstate. The claim markers `[verified]`, `[inference]`, `[open]` are the system's epistemic vocabulary.

**No marketing language.** No superlatives, no hype, no calls to action.

**Sentence structure:** Plain, precise, complete. Favor specificity over brevity when both cannot coexist.

**Casing:** Sentence case everywhere, including navigation labels. ALL CAPS only for monospace commit hashes. Never title case except for proper nouns.

**Emoji:** Not used. Unicode typographic characters (→, ·, —, ×) are acceptable where semantically appropriate.

---

## Visual Foundations

### Colors

**Ink** is the primary neutral — a warm, slightly brown-tinted gray scale that evokes aged paper and careful ink. Use for text, borders, and most UI surfaces. See `tokens/colors.css`.

**Gold** (`--gold-500: #b88a00`) is the primary accent. Used sparingly and deliberately: primary buttons, active states, the mark in the logo, earned emphasis. It connotes value and permanence.

**Rigpa blue** (`--rigpa-500: #2d6ac8`) signals settled understanding — resolved states, links, and the `rigpa/*` branch type. Cool, stable, trustworthy.

**Ma rose** (`--ma-500: #b857a4`) signals individual voice — `ma/*` branches, elector indicators, inquiry in progress. Warm, provisional, personal.

### Themes

The system ships four named accent themes applied via `data-theme` on `<html>` or any containing element. All themes share the ink neutral scale and the rigpa/ma supporting scales; only the primary accent and its derived semantic tokens change.

| Theme | Deity | Accent scale | Character |
|-------|-------|--------------|-----------| 
| `sid` | Siddharta | gold | warm, deliberate, earned — the default |
| `tara` | Green Tara | jade | emerald, compassionate, open |
| `kal` | Mahakala | crimson | deep red, protective, wrathful |
| `manny` | Manjushri | saffron | orange-amber, incisive, illuminating |

Usage: add `data-theme="tara"` (or `kal`, `manny`) to `<html>`. The `sid` theme is the `:root` fallback; the attribute is not required for sid. The attribute may also be scoped to a sub-tree, which is useful for comparing themes within a single view.

Tokens that change per theme: `--surface-accent`, `--surface-base` (kal only), `--text-accent`, `--text-link`, `--text-link-hover`, `--border-focus`, `--border-accent`, `--action-bg/hover/active`, `--action-fg`, and all `--phase-*` tokens. All other semantic tokens inherit unchanged from `:root`.

**Claim state colors** are reserved strictly for epistemic markers:
- Verified: muted forest green
- Inference: warm amber
- Open: slate blue

### Typography

Four families, each carrying meaning:
- **Cormorant Garamond** — display headings, titles, the wordmark. Scholarly and elegant. Use at 24px+ with generous line-height.
- **Spectral** — body prose, node content, long-form text. Highly readable serif. Use at 14–18px.
- **DM Sans** — UI labels, controls, navigation, metadata. Clean and neutral. Use at 11–16px.
- **IBM Plex Mono** — node paths, branch refs, commit hashes, code. Precise and technical. Use at 11–14px.

> ⚠ **Font substitution notice:** All four families are served from Google Fonts CDN. If offline use or exact brand fidelity is required, replace `tokens/fonts.css` with `@font-face` declarations pointing to licensed font files.

### Spacing

4px base unit. Layouts should breathe — prefer `--space-6` and above for component gaps. Tight spacing (`--space-1` to `--space-2`) only for within-element micro-gaps.

### Backgrounds

- **Page base:** `--surface-base` (`--ink-0: #faf8f4`) — warm near-white, parchment-like
- **Raised surfaces:** pure white (`#ffffff`) with `--shadow-xs` or `--shadow-sm`
- **Recessed/overlay:** `--surface-overlay` (`--ink-50`)
- **No full-bleed images, no gradients, no textures.** The corpus's content is the visual.

### Borders & Radius

1px borders in `--border-ui` (ink-200). Radii are deliberately small:
- Inline elements (badges, markers): `--radius-sm` (3px)
- Controls (inputs, buttons): `--radius-md` (4px) to `--radius-lg` (6px)
- Cards and panels: `--radius-lg` (6px) to `--radius-xl` (8px)
- No fully-rounded cards; the system favors precision over softness.

### Shadows

Minimal. Most surfaces need no shadow. Cards: `--shadow-xs`. Panels: `--shadow-sm`. Modals: `--shadow-lg`. Never use shadows for decoration.

### Motion

Slow and deliberate. Standard transitions: `var(--duration-base)` (200ms) with `var(--ease-standard)`. Entry animations: `var(--duration-slow)` (350ms) with `var(--ease-decelerate)`. No infinite loops. No bounce. Motion should feel like turning a page.

### Hover and press states

- **Ghost/subtle buttons:** background fills to `--ink-50` on hover, `--ink-100` on press
- **Primary (gold) buttons:** background lightens to `--gold-400` on hover
- **Links:** color deepens (rigpa-700) — no underline on hover unless inline text link
- **Cards (interactive):** border color strengthens to `--ink-300`; no transform/lift

### Cards

White background, `--border-ui` border (1px), `--radius-xl` (8px), `--shadow-xs`. Padding: `--space-inset-lg`. No colored left-border accent. No heavy shadow.

### Iconography

See **Iconography** section below.

---

## Iconography

**Icon library:** [Lucide Icons](https://lucide.dev/) via CDN. Stroke icons, 1.5px stroke-width, 16px and 20px sizes. No filled icons.

**Usage in HTML:**
```html
<script src="https://unpkg.com/lucide@latest/dist/umd/lucide.min.js"></script>
<!-- then: -->
<i data-lucide="circle" style="width:16px;height:16px;stroke-width:1.5"></i>
<script>lucide.createIcons();</script>
```

**Key icons used in yidam:**

| Intent | Lucide name |
|--------|-------------|
| Corpus node | `file-text` |
| Open question | `help-circle` |
| Edge / link | `arrow-right` |
| Branch | `git-branch` |
| Graph view | `network` |
| Merge / synthesis | `git-merge` |
| Verified | `check-circle` |
| Inference | `arrow-up-right` |
| Search | `search` |
| Elector / agent | `user` / `bot` |
| Phase: Investigation | `compass` |
| Phase: Extraction | `download` |
| Phase: Synthesis | `layers` |
| Phase: Assessment | `scale` |

> ⚠ **Substitution notice:** No custom icon set exists in the yidam codebase at time of authoring. Lucide was chosen for its stroke-weight consistency and sparse aesthetic. If yidam develops a custom icon set, replace CDN references and update this section.

No emoji used anywhere in the system.

---

## File Structure

```
index.js                ← the entry point importers use — every component, re-exported
styles.css              ← global entry point (imports only)
tokens.css              ← the token bundle a consumer imports (see _ds_manifest.json)
tokens/                 ← design tokens
  colors.css
  typography.css
  spacing.css
  borders.css
  shadows.css
  motion.css
  semantic.css
  fonts.css
assets/
  logo.svg              ← full logo (mark + wordmark)
  logo-mark.svg         ← mark only
  wordmark.svg          ← text only
guidelines/             ← foundation specimen cards (@dsCard)
components/
  core/                 ← Button, Badge, Tag, Card, Avatar
  forms/                ← Input, Textarea, Select, Checkbox, Radio, Switch
  navigation/           ← Tabs, Breadcrumb, Tooltip
  feedback/             ← Dialog, Toast
  knowledge/            ← ClaimMarker, NodeCard, PhaseTag, BranchRef
  measurement/          ← StatusMeter, CoverageBar
ui_kits/
  corpus/               ← Corpus Browser interactive prototype
```

---

## Component Index

| Component | Group | Description |
|-----------|-------|-------------|
| Button | core | Primary, ghost, subtle, danger variants |
| Badge | core | Small semantic label; claim-state variants |
| Tag | core | Removable label chip |
| Card | core | Content container with optional hover |
| Avatar | core | Elector/agent avatar with initials |
| Input | forms | Text input with label + error |
| Textarea | forms | Multi-line input |
| Select | forms | Dropdown select |
| Checkbox | forms | Labeled checkbox |
| Radio | forms | Labeled radio button |
| Switch | forms | Toggle switch |
| Tabs | navigation | Tabbed navigation |
| Breadcrumb | navigation | Hierarchical path |
| Tooltip | navigation | Hover hint |
| Dialog | feedback | Modal dialog |
| Toast | feedback | Transient notification |
| ClaimMarker | knowledge | [verified] / [inference] / [open] inline marker |
| NodeCard | knowledge | Corpus node summary card |
| PhaseTag | knowledge | Phase type indicator |
| BranchRef | knowledge | ma/* and rigpa/* branch reference |
| StatusMeter | measurement | A test run as a bar — asserted / failed / skipped |
| CoverageBar | measurement | Line coverage with a third state: unmeasured |

**`measurement/` has no card preview yet.** The `.card.html` specimens render out of
`_ds_bundle.js`, which is design-tool output carrying source hashes; only the tool can rewrite
it, so a component added by hand has no preview until the next sync. `design_system.rs`
asserts the gap rather than leaving it to be found by opening a blank card, and the live
surface at `/yidam/main/quality/` renders both components against real data in the meantime.

---

## Importing

```js
import { StatusMeter, CoverageBar } from '../path/to/yidam/design/index.js';
```

`index.js` is the door, and `_adherence.oxlintrc.json` forbids reaching past it into
`components/<group>/`. Both halves are recent: the rule had been in the config since it was
written and there was no `index.js` to point at, and the rule was disabled everywhere by an
`overrides` block besides. The quality pages (#467) are the system's first consumer.

Tokens come separately and are plain CSS:

```css
@import "../path/to/yidam/design/tokens.css";
```

---

## UI Kit Index

| Kit | Path | Description |
|-----|------|-------------|
| Corpus Browser | `ui_kits/corpus/` | Full interactive prototype of the corpus web interface |
