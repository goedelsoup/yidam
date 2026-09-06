/**
 * The browse table, filtered in the browser — and RFC-0030's Phase 1 spike, landed.
 *
 * # Why this exists as an island at all
 *
 * #606 asks Phase 1 to find out whether the design system's React components survive client
 * bundling, because nothing had ever hydrated one: `yidam/web/docs/astro.config.mjs:243-246`
 * records that no `client:*` directive appears on any quality page, so React ran at build time
 * and none of it was ever shipped to a reader. Phase 2's forms are specified in terms of these
 * components, so the answer had to arrive before something depended on it.
 *
 * The answer is landed rather than written down. A spike run once in a throwaway tree answers
 * the question for a build nobody runs again; this one is answered by `mise run ci-editor-web`
 * every time, and `test/hydration.mjs` is what reads the answer.
 *
 * `Input` rather than a cheaper component on purpose. It is from the forms group — the exact
 * group Phase 2 needs — and it is *stateful*: `useState` for the focus ring means a page where
 * hydration silently failed looks identical until you focus the field, which is the failure
 * mode a build-output check alone would miss.
 *
 * # Why filtering is an affordance and not a verdict
 *
 * RFC-0016's rule is **TypeScript computes affordances; the CLI computes verdicts**, and this
 * file is on the side of it that got a Node process in the reversal — so the line matters here
 * more than anywhere else in the package.
 *
 * Every row below was resolved by the binary before this component saw it. `exists` is
 * `dangling_edge`'s own answer and the strikethrough renders it; `resolved` is the CLI's path
 * resolution. Substring-matching a list a person is looking at decides nothing about the
 * corpus — it decides what is on their screen, which is the definition of an affordance and
 * is why no gate in `test/boundary.mjs` has anything to say about this file.
 *
 * `.jsx` rather than `.tsx`, deliberately: `design_tokens.rs`'s consumer scan reads `css`,
 * `astro` and `jsx`, and `tsx` is not on the list (#611). Every colour here is a `var(--…)`
 * or a class from `app.css`, and writing this as `.tsx` would put it outside the gate that
 * holds that.
 */

import React, { useMemo, useState } from 'react'

import { Input } from '../../../../design/index.js'
import { nodeHref } from '../lib/graph.ts'

/** What the filter reads. Stated here rather than left for a reader to infer from misses. */
function haystack(node) {
  return [node.node, node.label ?? '', node.class ?? ''].join(' ').toLowerCase()
}

export function NodeTable({ nodes }) {
  const [query, setQuery] = useState('')

  const needle = query.trim().toLowerCase()
  const shown = useMemo(
    () => (needle === '' ? nodes : nodes.filter((node) => haystack(node).includes(needle))),
    [nodes, needle],
  )

  return (
    <>
      <div className="node-filter">
        <Input
          label="Filter"
          value={query}
          onChange={(event) => setQuery(event.target.value)}
          placeholder="node, label or class"
          size="sm"
          // The count is the helper text, so a filter that is hiding rows says so in the
          // place a person is already looking. An empty table and a filtered-to-nothing
          // table are the same picture otherwise — the mistake this surface keeps finding.
          helper={
            needle === ''
              ? `${nodes.length} ${nodes.length === 1 ? 'node' : 'nodes'}`
              : `${shown.length} of ${nodes.length}`
          }
        />
      </div>

      {shown.length === 0 ? (
        <p className="node-empty">
          No node matches <code>{query.trim()}</code>. The corpus has {nodes.length}.
        </p>
      ) : (
        <table className="corpus">
          <thead>
            <tr>
              <th>Node</th>
              <th>Class</th>
              <th>Edges</th>
            </tr>
          </thead>
          <tbody>
            {shown.map((node) => (
              <tr key={node.node}>
                <td>
                  <a href={nodeHref(node.node)}>
                    <code>{node.node}</code>
                  </a>
                  {node.label && <div className="node-label">{node.label}</div>}
                </td>
                <td>{node.class ?? '—'}</td>
                <td>
                  <ul className="edges">
                    {(node.links ?? []).map((link, index) => (
                      <li
                        key={`${link.relationship}:${link.target}:${index}`}
                        className={link.exists === false ? 'dangling' : undefined}
                      >
                        <span className="relationship">{link.relationship}</span>{' '}
                        <code>{link.resolved || link.target}</code>
                      </li>
                    ))}
                  </ul>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </>
  )
}
