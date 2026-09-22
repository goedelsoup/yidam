/**
 * The one write this surface can perform, as two buttons and the report that came back.
 *
 * # Why this is an island and not a form
 *
 * There is nothing to type. `propose` takes `{ dry_run }` and drafts from the committed
 * corpus's own findings, so the whole of what a person contributes here is *whether* — and
 * whether to look first. Preview and write are the same run with one flag flipped; the
 * report a preview shows is the report the write would make, less the branch. That is why
 * "Preview" is the primary action and "Write" the subtle one: the second is the first plus a
 * branch, and a person who has not read the first has nothing to compare the branch to.
 *
 * # What this file decides
 *
 * Which of the two `POST`s to send, and how to lay out what came back. Every sentence it
 * renders — the branch name, the verbs, a skip's reason, a refusal's token — is the binary's.
 * RFC-0016's rule for this package is that TypeScript computes affordances and the CLI
 * computes verdicts, and a write is the case where that line is load-bearing: a component
 * that decided a proposal "looked fine" and wrote would be a second gate, wrong on the day
 * the first one changed.
 *
 * The `409` branch renders the refusal's text whole. Its leading token
 * (`capability-not-supported`, `propose-refused`) is frozen by the contract, so a person can
 * search for it and find the same words the MCP client would have been given.
 *
 * `.jsx` rather than `.tsx` for the reason `NodeTable.jsx` gives: the design-token consumer
 * scan reads `jsx` and not `tsx` (#611), and every colour here is a class from `app.css`.
 */

import React, { useState } from 'react'

import { Button } from '../../../../design/index.js'

async function post(dryRun) {
  const response = await fetch(`/api/act/propose?dry_run=${dryRun ? 'true' : 'false'}`, {
    method: 'POST',
  })
  // Every status the route sends carries a JSON body; a non-JSON body is a server that is
  // not this one and is reported as such rather than parsed at.
  let body
  try {
    body = await response.json()
  } catch {
    body = { ok: false, error: `${response.status} with a body that was not JSON` }
  }
  return { status: response.status, body }
}

function Proposals({ report, dryRun }) {
  const wrote = report.written !== null && report.written !== undefined
  return (
    <div className="proposal">
      <p className="proposal-head">
        {wrote ? (
          <>
            Wrote <code>{report.written.branch}</code>
            {' — '}
            {report.written.commits.length}{' '}
            {report.written.commits.length === 1 ? 'commit' : 'commits'}. The baseline is
            unmoved; review the branch as commits and delete it to reject.
          </>
        ) : dryRun ? (
          <>
            Preview — nothing written. This is what a write would put on{' '}
            <code>{report.branch}</code> from <code>{report.head.slice(0, 10)}</code>.
          </>
        ) : (
          <>Nothing to propose from {report.head.slice(0, 10)}; no branch was written.</>
        )}
      </p>

      {report.proposals.length > 0 && (
        <ul className="proposals">
          {report.proposals.map((p, index) => (
            <li key={`${p.verb}:${p.subject}:${index}`}>
              <span className="tag">{p.verb}</span> <code>{p.subject}</code>
              {p.node && (
                <>
                  {' '}
                  <code>{p.node}</code>
                </>
              )}
              <p className="rationale">
                <code>{p.check}</code> — {p.detail}
              </p>
              {p.paths && p.paths.length > 0 && (
                <p className="rationale proposal-paths">{p.paths.join(', ')}</p>
              )}
            </li>
          ))}
        </ul>
      )}

      {report.skipped.length > 0 && (
        <details>
          <summary>
            {report.skipped.length} {report.skipped.length === 1 ? 'finding' : 'findings'} the
            gate reports and propose will not draft from
          </summary>
          <ul className="proposals skipped">
            {report.skipped.map((s, index) => (
              <li key={`${s.check}:${s.node}:${index}`}>
                <code>{s.check}</code>
                {s.node && (
                  <>
                    {' '}
                    <code>{s.node}</code>
                  </>
                )}
                <p className="rationale">{s.reason}</p>
              </li>
            ))}
          </ul>
        </details>
      )}

      {wrote && (
        <ul className="proposal-commits">
          {report.written.commits.map((line, index) => (
            <li key={index}>
              <code>{line}</code>
            </li>
          ))}
        </ul>
      )}
    </div>
  )
}

export function Propose() {
  const [busy, setBusy] = useState(false)
  const [last, setLast] = useState(null)

  async function run(dryRun) {
    setBusy(true)
    try {
      const answer = await post(dryRun)
      setLast({ dryRun, ...answer })
    } catch (e) {
      setLast({ dryRun, status: 0, body: { ok: false, error: String(e) } })
    } finally {
      setBusy(false)
    }
  }

  return (
    <div className="propose">
      <div className="propose-actions">
        <Button size="sm" disabled={busy} onClick={() => run(true)}>
          Preview proposal
        </Button>
        <Button size="sm" variant="subtle" disabled={busy} onClick={() => run(false)}>
          Write proposal branch
        </Button>
        {busy && <span className="rationale">running…</span>}
      </div>

      {last && last.body.ok && <Proposals report={last.body.result} dryRun={last.dryRun} />}

      {last && !last.body.ok && (
        <p className={last.status === 409 ? 'proposal-refused' : 'shell-unavailable'} role="alert">
          {last.status === 409 ? 'Refused: ' : ''}
          {last.body.error}
        </p>
      )}
    </div>
  )
}
