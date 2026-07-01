import React from 'react';

const MARKER_STYLES = {
  verified:  { bg: 'var(--verified-bg)',  fg: 'var(--verified-fg)',  border: 'var(--verified-border)'  },
  inference: { bg: 'var(--inference-bg)', fg: 'var(--inference-fg)', border: 'var(--inference-border)' },
  open:      { bg: 'var(--open-bg)',      fg: 'var(--open-fg)',      border: 'var(--open-border)'      },
};

/**
 * Inline epistemic annotation: [verified], [inference], or [open].
 * Renders in monospace to clearly distinguish from surrounding prose.
 */
export function ClaimMarker({ type = 'open', annotation }) {
  const s = MARKER_STYLES[type] || MARKER_STYLES.open;

  return (
    <span style={{
      display: 'inline-flex', alignItems: 'baseline', gap: '3px',
      padding: '1px 6px',
      borderRadius: 'var(--radius-sm)',
      fontFamily: 'var(--font-mono)',
      fontSize: '0.78em',         /* scales with surrounding text */
      fontWeight: 500,
      lineHeight: 1.6,
      whiteSpace: 'nowrap',
      verticalAlign: 'baseline',
      background: s.bg,
      color: s.fg,
      border: `1px solid ${s.border}`,
    }}>
      [{type}]
      {annotation && (
        <span style={{
          fontFamily: 'var(--font-serif)',
          fontStyle: 'italic',
          fontWeight: 400,
          marginLeft: '3px',
        }}>
          {annotation}
        </span>
      )}
    </span>
  );
}
