import React from 'react';

// Three states, not two, and the third is neutral.
//
// `covered` and `uncovered` are lines a test could have executed. `unmeasured` is lines in
// files the build did not compile at all — a statement about the build, not about the tests.
// Adding them to `uncovered` produces the number #464 exists to prevent: one that calls the
// whole feature-gated index path untested because a pull request does not build it.
//
// So the percentage is computed over `covered + uncovered` only, and `unmeasured` is drawn
// in the neutral family beside it with the feature set that explains it. A page that has
// nothing to put in `features` is a page that cannot say which build a number is about,
// which is why it is required rather than defaulted.

export function CoverageBar({ covered, uncovered, unmeasured = 0, features, label }) {
  const measured = covered + uncovered;
  const total = measured + unmeasured;
  const pct = measured > 0 ? Math.round((covered / measured) * 100) : null;

  return (
    <div style={{ fontFamily: 'var(--font-ui)', fontSize: 'var(--text-xs)' }}>
      <div
        style={{
          display: 'flex',
          justifyContent: 'space-between',
          marginBottom: 'var(--space-1)',
          color: 'var(--text-primary)',
        }}
      >
        <span>{label}</span>
        <span style={{ fontFamily: 'var(--font-mono)' }}>
          {pct === null ? 'not measured' : `${pct}%`}
        </span>
      </div>
      <div
        role="img"
        aria-label={
          pct === null
            ? 'no lines were measured'
            : `${covered} covered, ${uncovered} uncovered, ${unmeasured} unmeasured`
        }
        style={{
          display: 'flex',
          height: 'var(--space-2)',
          borderRadius: 'var(--radius-sm)',
          overflow: 'hidden',
          background: 'var(--run-unmeasured-fill)',
        }}
      >
        {total > 0 && covered > 0 && (
          <span style={{ width: `${(covered / total) * 100}%`, background: 'var(--run-passed-fill)' }} />
        )}
        {total > 0 && uncovered > 0 && (
          <span style={{ width: `${(uncovered / total) * 100}%`, background: 'var(--run-failed-fill)' }} />
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
        <span>{covered} covered</span>
        <span>{uncovered} uncovered</span>
        <span style={{ color: 'var(--run-unmeasured-fg)' }}>{unmeasured} unmeasured</span>
      </div>
      <div style={{ marginTop: 'var(--space-1)', color: 'var(--run-unmeasured-fg)' }}>
        Measured under{' '}
        <span style={{ fontFamily: 'var(--font-mono)' }}>
          {features.length > 0 ? features.join(', ') : 'an unstated feature set'}
        </span>
        . Unmeasured lines were not compiled into this build and are not a claim that they
        are untested.
      </div>
    </div>
  );
}
