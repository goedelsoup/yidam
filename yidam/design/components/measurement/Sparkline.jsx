import React from 'react';

// A shape, not a chart.
//
// No axes, no gridlines, no tooltip. The question a sparkline answers is "which way has this
// been going", and everything else on a chart is there to answer a different question that
// the series file answers better. `last` and `label` carry the number; the line carries the
// direction.
//
// An empty or single-point series renders as a stated absence rather than a flat line at
// zero — the same rule the coverage bar follows, for the same reason: one measurement is not
// a trend, and drawing it as one invents a history that does not exist.

const VIEW = { w: 240, h: 40, pad: 3 };

export function Sparkline({ points, label, format, higherIsWorse = false }) {
  const shown = format ? format(points[points.length - 1]) : points[points.length - 1];

  if (points.length < 2) {
    return (
      <div style={{ fontFamily: 'var(--font-ui)', fontSize: 'var(--text-xs)' }}>
        <div style={{ color: 'var(--text-primary)' }}>{label}</div>
        <div style={{ color: 'var(--run-unmeasured-fg)', marginTop: 'var(--space-1)' }}>
          {points.length === 1
            ? `One record so far (${shown}). A trend needs two.`
            : 'No records yet.'}
        </div>
      </div>
    );
  }

  const min = Math.min(...points);
  const max = Math.max(...points);
  const span = max - min || 1;
  const step = (VIEW.w - VIEW.pad * 2) / (points.length - 1);
  const y = (v) => VIEW.h - VIEW.pad - ((v - min) / span) * (VIEW.h - VIEW.pad * 2);
  const path = points.map((v, i) => `${i === 0 ? 'M' : 'L'}${VIEW.pad + i * step},${y(v)}`).join(' ');

  const delta = points[points.length - 1] - points[0];
  const worse = higherIsWorse ? delta > 0 : delta < 0;
  const stroke = delta === 0
    ? 'var(--run-unmeasured-fill)'
    : worse
      ? 'var(--run-failed-fill)'
      : 'var(--run-passed-fill)';

  return (
    <div style={{ fontFamily: 'var(--font-ui)', fontSize: 'var(--text-xs)' }}>
      <div style={{ display: 'flex', justifyContent: 'space-between', color: 'var(--text-primary)' }}>
        <span>{label}</span>
        <span style={{ fontFamily: 'var(--font-mono)' }}>{shown}</span>
      </div>
      <svg
        viewBox={`0 0 ${VIEW.w} ${VIEW.h}`}
        preserveAspectRatio="none"
        role="img"
        aria-label={`${label}: ${points.length} records, ${points[0]} to ${points[points.length - 1]}`}
        style={{ width: '100%', height: 'var(--space-10)', marginTop: 'var(--space-1)' }}
      >
        <path d={path} fill="none" stroke={stroke} strokeWidth="1.5" vectorEffect="non-scaling-stroke" />
      </svg>
      <div style={{ color: 'var(--text-secondary)' }}>
        {points.length} records · {delta === 0 ? 'unchanged' : worse ? 'worse' : 'better'} since the first
      </div>
    </div>
  );
}
