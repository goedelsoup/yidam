/**
 * The neighbourhood of the node you are editing.
 *
 * `export --format web` already serves reading and retrieval, and serves them better. What
 * it structurally cannot do is show the neighbourhood of a node *in flight* — it is a built
 * artifact of a committed corpus, and the node under the cursor is neither.
 *
 * # The traversal is not here
 *
 * `yidam neighbors --format json` performs it: breadth-first over the undirected view, each
 * node once at its shortest hop, with the direction of the edge it was reached by. That is
 * the same function `serve --mcp`'s `neighbors` tool calls — it moved into the light build
 * for this, so the two surfaces answer identically rather than similarly.
 *
 * What is here is layout: grouping, ordering, and HTML.
 *
 * # Styling, and one thing the design system cannot give an offline surface
 *
 * Spacing and radii are transcribed from `yidam/design/tokens/`, and a test parses those
 * files and fails when the copy drifts.
 *
 * **Colour is the reader's theme**, not the brand palette. The design system has no dark
 * mode outside the claim triad added for the decoration work, and a webview that rendered a
 * light card inside a dark editor would be worse than one that is not brand-coloured.
 *
 * **Fonts are the editor's**, because `yidam/design/tokens/fonts.css` is an `@import` from
 * the Google Fonts CDN and RFC-0016 requires this surface to be fully offline. The brand
 * font layer is unusable here until the files are vendored.
 *
 * No `vscode` import.
 */

import type { Envelope } from './reports.ts'

export interface NeighborRow {
  node: string
  class: string
  label: string
  description: string
  relationship: string
  direction: 'out' | 'in'
  hops: number
  is_node: boolean
}

export interface NeighborsReport extends Envelope {
  corpus_dir: string
  node: string
  class: string
  label: string
  description: string
  depth: number
  neighbors: NeighborRow[]
}

/** Transcribed from `yidam/design/tokens/{spacing,borders}.css`; checked against them by test. */
export const TOKENS: Record<string, string> = {
  '--space-1': '0.25rem',
  '--space-2': '0.5rem',
  '--space-3': '0.75rem',
  '--space-4': '1rem',
  '--space-6': '1.5rem',
  '--radius-sm': '3px',
  '--radius-lg': '6px',
  '--radius-full': '9999px',
  '--border-base': '1px',
}

export interface Group {
  hops: number
  direction: 'out' | 'in'
  relationship: string
  rows: NeighborRow[]
}

function label(row: NeighborRow): string {
  return row.label || row.node
}

/**
 * Hop, then direction, then relationship.
 *
 * Hop first because distance is the reader's question — *what is next to this* before *what
 * is two steps away*. Direction second because at hops > 1 it is relative to the node the
 * edge was reached from, so mixing the two inside one group would make the arrow a lie.
 *
 * Outbound before inbound: what this node asserts, then what asserts about it.
 */
export function groups(report: NeighborsReport): Group[] {
  const keyed = new Map<string, Group>()
  for (const row of report.neighbors) {
    const key = `${row.hops} ${row.direction} ${row.relationship}`
    const group = keyed.get(key) ?? {
      hops: row.hops,
      direction: row.direction,
      relationship: row.relationship,
      rows: [],
    }
    group.rows.push(row)
    keyed.set(key, group)
  }
  return [...keyed.values()]
    .map((g) => ({ ...g, rows: g.rows.slice().sort((a, b) => label(a).localeCompare(label(b))) }))
    .sort(
      (a, b) =>
        a.hops - b.hops ||
        (a.direction === b.direction ? 0 : a.direction === 'out' ? -1 : 1) ||
        a.relationship.localeCompare(b.relationship),
    )
}

/**
 * Escape for HTML text and attribute contexts alike.
 *
 * Not paranoia: a corpus is authored prose, labels and descriptions are written by people,
 * and a node whose description contains a `<` would otherwise silently truncate the panel.
 * The same escape makes injected markup inert, which matters because this content arrives
 * from a repository an editor merely opened.
 */
export function escape(text: string): string {
  return text
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#39;')
}

function tokenBlock(): string {
  return Object.entries(TOKENS)
    .map(([k, v]) => `  ${k}: ${v};`)
    .join('\n')
}

const STYLE = `
:root {
${tokenBlock()}
}
* { box-sizing: border-box; }
body {
  margin: 0;
  padding: var(--space-4);
  font-family: var(--vscode-font-family);
  font-size: var(--vscode-font-size);
  color: var(--vscode-foreground);
  background: var(--vscode-editor-background);
}
.centre {
  border: var(--border-base) solid var(--vscode-focusBorder);
  border-radius: var(--radius-lg);
  padding: var(--space-3) var(--space-4);
  margin-bottom: var(--space-6);
}
.centre h1 { margin: 0; font-size: 1.15em; }
.centre p { margin: var(--space-2) 0 0; color: var(--vscode-descriptionForeground); }
.chip {
  display: inline-block;
  font-family: var(--vscode-editor-font-family);
  font-size: 0.85em;
  padding: 0 var(--space-2);
  border-radius: var(--radius-full);
  border: var(--border-base) solid var(--vscode-panel-border);
  color: var(--vscode-descriptionForeground);
}
/* The edge carries the same weight as the node it points at — the whole reason this view
   exists rather than a list of filenames. */
.edge {
  display: flex;
  align-items: baseline;
  gap: var(--space-2);
  margin: var(--space-4) 0 var(--space-1);
  font-family: var(--vscode-editor-font-family);
}
.edge .arrow { color: var(--vscode-textLink-foreground); }
.edge .rel { font-weight: 600; }
.edge .hop { margin-left: auto; }
ul { list-style: none; margin: 0; padding: 0 0 0 var(--space-4); }
li { padding: var(--space-1) 0; }
a {
  color: var(--vscode-textLink-foreground);
  text-decoration: none;
  cursor: pointer;
}
a:hover { text-decoration: underline; }
.detached { color: var(--vscode-descriptionForeground); cursor: default; }
.empty { color: var(--vscode-descriptionForeground); }
.controls { margin-bottom: var(--space-4); display: flex; gap: var(--space-2); }
button {
  font: inherit;
  padding: var(--space-1) var(--space-3);
  border-radius: var(--radius-sm);
  border: var(--border-base) solid var(--vscode-panel-border);
  background: transparent;
  color: var(--vscode-foreground);
  cursor: pointer;
}
button[aria-pressed="true"] {
  background: var(--vscode-button-background);
  color: var(--vscode-button-foreground);
}
`.trim()

const SCRIPT = `
const vscode = acquireVsCodeApi()
document.addEventListener('click', (e) => {
  const node = e.target.closest('[data-node]')
  if (node) { vscode.postMessage({ type: 'open', node: node.dataset.node }); return }
  const depth = e.target.closest('[data-depth]')
  if (depth) vscode.postMessage({ type: 'depth', depth: Number(depth.dataset.depth) })
})
`.trim()

function item(row: NeighborRow): string {
  const name = escape(label(row))
  const meta = row.class ? `<span class="chip">${escape(row.class)}</span>` : ''
  // A reached target that is not a corpus node — an `.ont.yml`, or a broken edge — is shown
  // and not linked. Hiding it would make the neighbourhood disagree with the graph.
  if (!row.is_node) {
    return `<li><span class="detached">${escape(row.node)}</span> <span class="chip">not a node</span></li>`
  }
  return `<li><a data-node="${escape(row.node)}" title="${escape(row.description)}">${name}</a> ${meta}</li>`
}

/**
 * The whole page, self-contained.
 *
 * No stylesheet link, no script src, no font import, no image. The repo's CI is hermetic and
 * this surface holds the same line — a test asserts the rendered HTML references no external
 * host at all.
 */
export function render(report: NeighborsReport, nonce: string): string {
  const centre = report.node
    ? `<div class="centre">
      <h1>${escape(report.label || report.node)} <span class="chip">${escape(report.class)}</span></h1>
      <p>${escape(report.description)}</p>
    </div>`
    : `<p class="empty">Open a corpus node to see its neighbourhood.</p>`

  const body =
    report.node && report.neighbors.length === 0
      ? `<p class="empty">Nothing within ${report.depth} hop(s) — no edges out, and nothing points here.</p>`
      : groups(report)
          .map(
            (g) => `<div class="edge">
      <span class="arrow">${g.direction === 'out' ? '→' : '←'}</span>
      <span class="rel">${escape(g.relationship)}</span>
      <span class="chip hop">${g.hops} hop${g.hops === 1 ? '' : 's'}</span>
    </div>
    <ul>${g.rows.map(item).join('')}</ul>`,
          )
          .join('\n')

  const controls = report.node
    ? `<div class="controls">${[1, 2]
        .map(
          (d) =>
            `<button data-depth="${d}" aria-pressed="${d === report.depth}">${d} hop${d === 1 ? '' : 's'}</button>`,
        )
        .join('')}</div>`
    : ''

  return `<meta http-equiv="Content-Security-Policy" content="default-src 'none'; style-src 'nonce-${nonce}'; script-src 'nonce-${nonce}';">
<style nonce="${nonce}">${STYLE}</style>
${centre}
${controls}
${body}
<script nonce="${nonce}">${SCRIPT}</script>`
}
