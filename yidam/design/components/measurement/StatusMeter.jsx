import React from 'react';

// The green segment is `asserted`, never `passed`.
//
// A runtime skip is recorded by a test runner as a pass: the process ran and did not fail.
// So a suite that announced two skips and exercised nothing has the same `passed` as a suite
// that exercised everything, and a meter drawn from `passed` renders the two identically.
// `asserted` is the count that separates them, and it is a required prop so that a page
// cannot draw this bar without having been given it.

const SEGMENTS = [
  { key: 'asserted', fill: 'var(--run-passed-fill)',  label: 'asserted' },
  { key: 'failed',   fill: 'var(--run-failed-fill)',  label: 'failed'   },
  { key: 'skipped',  fill: 'var(--run-skipped-fill)', label: 'skipped'  },
];

const HEIGHTS = { sm: 'var(--space-1)', md: 'var(--space-2)' };

export function StatusMeter({ asserted, failed = 0, skipped = 0, label, size = 'md' }) {
  const counts = { asserted, failed, skipped };
  const total = asserted + failed + skipped;
  const nothing = total === 0;
  const inert = !nothing && asserted === 0 && failed === 0;

  return (
    <div style={{ fontFamily: 'var(--font-ui)', fontSize: 'var(--text-xs)' }}>
      {label && (
        <div style={{ color: 'var(--text-primary)', marginBottom: 'var(--space-1)' }}>{label}</div>
      )}
      <div
        role="img"
        aria-label={
          nothing
            ? 'nothing ran'
            : SEGMENTS.map((s) => `${counts[s.key]} ${s.label}`).join(', ')
        }
        style={{
          display: 'flex',
          height: HEIGHTS[size] || HEIGHTS.md,
          borderRadius: 'var(--radius-sm)',
          overflow: 'hidden',
          background: 'var(--run-unmeasured-fill)',
        }}
      >
        {!nothing && SEGMENTS.map((s) =>
          counts[s.key] > 0 ? (
            <span
              key={s.key}
              style={{ width: `${(counts[s.key] / total) * 100}%`, background: s.fill }}
            />
          ) : null,
        )}
      </div>
      <div
        style={{
          display: 'flex',
          gap: 'var(--space-3)',
          marginTop: 'var(--space-1)',
          color: 'var(--text-secondary)',
        }}
      >
        {SEGMENTS.map((s) => (
          <span key={s.key}>
            {counts[s.key]} {s.label}
          </span>
        ))}
      </div>
      {nothing && (
        <div style={{ color: 'var(--run-unmeasured-fg)', marginTop: 'var(--space-1)' }}>
          Nothing ran.
        </div>
      )}
      {inert && (
        <div style={{ color: 'var(--run-skipped-fg)', marginTop: 'var(--space-1)' }}>
          Ran and asserted nothing.
        </div>
      )}
    </div>
  );
}
