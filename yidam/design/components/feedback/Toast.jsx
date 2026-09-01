import React from 'react';

const VARIANTS = {
  info:    { bg: 'var(--ink-900)',      fg: 'var(--ink-50)',  dot: 'var(--ink-400)'   },
  // Every variant here is light text on a dark ground, which is what `--text-inverse` is
  // for — and what `info` beside them already resolves to. Not `--action-fg`: that is the
  // text on the action BUTTON, and under the default sid theme the action button is light
  // gold, so `--action-fg` is near-black ink. It would have put black text on these.
  success: { bg: 'var(--verified-fg)',  fg: 'var(--text-inverse)', dot: 'var(--verified-fg-dark)' },
  error:   { bg: 'var(--danger-bg)',    fg: 'var(--danger-fg)',    dot: 'var(--danger-accent)'    },
  warning: { bg: 'var(--gold-800)',     fg: 'var(--text-inverse)', dot: 'var(--gold-200)'         },
};

export function Toast({ message, type = 'info', detail, onDismiss }) {
  const s = VARIANTS[type] || VARIANTS.info;

  return (
    <div style={{
      display: 'inline-flex', alignItems: 'flex-start', gap: 'var(--space-3)',
      padding: 'var(--space-3) var(--space-4)',
      background: s.bg, color: s.fg,
      borderRadius: 'var(--radius-xl)',
      boxShadow: 'var(--shadow-xl)',
      fontFamily: 'var(--font-ui)',
      maxWidth: '440px', minWidth: '240px',
      border: '1px solid rgba(255,255,255,0.1)',
    }}>
      <span style={{
        width: '6px', height: '6px', borderRadius: '50%', background: s.dot,
        flexShrink: 0, marginTop: '5px',
      }} />
      <div style={{ flex: 1, minWidth: 0 }}>
        <div style={{ fontSize: 'var(--text-sm)', fontWeight: 500, lineHeight: 1.4 }}>{message}</div>
        {detail && (
          <div style={{ fontSize: 'var(--text-xs)', opacity: 0.75, marginTop: '2px', lineHeight: 1.4 }}>{detail}</div>
        )}
      </div>
      {onDismiss && (
        <button onClick={onDismiss} style={{
          background: 'none', border: 'none', cursor: 'pointer',
          color: 'inherit', opacity: 0.65, padding: '1px', flexShrink: 0,
          display: 'flex', alignItems: 'center', marginTop: '2px',
        }}>
          <svg width="12" height="12" viewBox="0 0 12 12" fill="none">
            <path d="M2 2l8 8M10 2L2 10" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round"/>
          </svg>
        </button>
      )}
    </div>
  );
}
