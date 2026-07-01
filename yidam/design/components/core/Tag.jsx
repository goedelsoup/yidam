import React, { useState } from 'react';

export function Tag({ children, onRemove, variant = 'default', disabled = false }) {
  const [hovered, setHovered] = useState(false);

  const variantStyles = {
    default: { background: 'var(--ink-50)',   color: 'var(--text-primary)',   border: '1px solid var(--ink-200)' },
    gold:    { background: 'var(--gold-50)',  color: 'var(--gold-800)',       border: '1px solid var(--gold-200)' },
    rigpa:   { background: 'var(--rigpa-50)', color: 'var(--rigpa-800)',      border: '1px solid var(--rigpa-200)' },
    ma:      { background: 'var(--ma-50)',    color: 'var(--ma-800)',         border: '1px solid var(--ma-200)' },
  };

  return (
    <span style={{
      display: 'inline-flex', alignItems: 'center', gap: '5px',
      padding: '3px 8px', borderRadius: 'var(--radius-full)',
      fontFamily: 'var(--font-ui)', fontSize: 'var(--text-xs)', fontWeight: 400,
      lineHeight: 1.4, whiteSpace: 'nowrap',
      ...(variantStyles[variant] || variantStyles.default),
    }}>
      {children}
      {onRemove && !disabled && (
        <button
          onClick={(e) => { e.stopPropagation(); onRemove(); }}
          onMouseEnter={() => setHovered(true)}
          onMouseLeave={() => setHovered(false)}
          style={{
            display: 'inline-flex', alignItems: 'center', justifyContent: 'center',
            width: '14px', height: '14px', padding: 0, margin: 0,
            background: hovered ? 'var(--ink-200)' : 'transparent',
            border: 'none', borderRadius: '50%', cursor: 'pointer',
            color: 'currentColor', transition: 'background 120ms',
            flexShrink: 0,
          }}
          aria-label="Remove"
        >
          <svg width="8" height="8" viewBox="0 0 8 8" fill="none">
            <path d="M1 1l6 6M7 1L1 7" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round"/>
          </svg>
        </button>
      )}
    </span>
  );
}
