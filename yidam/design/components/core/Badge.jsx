import React from 'react';

const VARIANTS = {
  default:   { background: 'var(--ink-100)',       color: 'var(--text-secondary)', border: '1px solid var(--ink-200)'         },
  gold:      { background: 'var(--gold-50)',        color: 'var(--gold-800)',       border: '1px solid var(--gold-200)'        },
  rigpa:     { background: 'var(--rigpa-50)',       color: 'var(--rigpa-800)',      border: '1px solid var(--rigpa-200)'       },
  ma:        { background: 'var(--ma-50)',          color: 'var(--ma-800)',         border: '1px solid var(--ma-200)'          },
  verified:  { background: 'var(--verified-bg)',   color: 'var(--verified-fg)',    border: '1px solid var(--verified-border)' },
  inference: { background: 'var(--inference-bg)',  color: 'var(--inference-fg)',   border: '1px solid var(--inference-border)'},
  open:      { background: 'var(--open-bg)',       color: 'var(--open-fg)',        border: '1px solid var(--open-border)'     },
  inverse:   { background: 'var(--surface-inverse)', color: 'var(--text-inverse)', border: '1px solid transparent'           },
};

export function Badge({ variant = 'default', size = 'md', dot = false, children }) {
  const sizeStyle = size === 'sm'
    ? { padding: '1px 5px', fontSize: 'var(--text-2xs)' }
    : { padding: '2px 7px', fontSize: 'var(--text-xs)'  };

  return (
    <span style={{
      display: 'inline-flex', alignItems: 'center', gap: '4px',
      borderRadius: 'var(--radius-sm)', fontFamily: 'var(--font-ui)',
      fontWeight: 500, whiteSpace: 'nowrap', lineHeight: 1.4,
      ...sizeStyle,
      ...(VARIANTS[variant] || VARIANTS.default),
    }}>
      {dot && (
        <span style={{
          width: '5px', height: '5px', borderRadius: '50%',
          background: 'currentColor', flexShrink: 0,
        }} />
      )}
      {children}
    </span>
  );
}
