import React from 'react';

function initials(name) {
  if (!name) return '?';
  const parts = name.trim().split(/[\s\/]+/);
  if (parts.length >= 2) return (parts[0][0] + parts[1][0]).toUpperCase();
  return parts[0].slice(0, 2).toUpperCase();
}

const SIZES = {
  xs: { size: '24px', font: 'var(--text-2xs)' },
  sm: { size: '32px', font: 'var(--text-xs)'  },
  md: { size: '40px', font: 'var(--text-sm)'  },
  lg: { size: '52px', font: 'var(--text-base)'},
};

export function Avatar({ name, src, size = 'md', variant = 'human', style: styleProp }) {
  const s = SIZES[size] || SIZES.md;
  const isAgent = variant === 'agent';

  const baseStyle = {
    width: s.size, height: s.size,
    borderRadius: isAgent ? 'var(--radius-md)' : '50%',
    display: 'inline-flex', alignItems: 'center', justifyContent: 'center',
    fontFamily: 'var(--font-ui)', fontSize: s.font, fontWeight: 500,
    flexShrink: 0, overflow: 'hidden',
    background: isAgent ? 'var(--rigpa-100)' : 'var(--ma-100)',
    color: isAgent ? 'var(--rigpa-800)' : 'var(--ma-800)',
    border: `1px solid ${isAgent ? 'var(--rigpa-200)' : 'var(--ma-200)'}`,
    userSelect: 'none',
    ...styleProp,
  };

  if (src) {
    return (
      <span style={baseStyle}>
        <img src={src} alt={name || ''} style={{ width: '100%', height: '100%', objectFit: 'cover', display: 'block' }} />
      </span>
    );
  }

  return <span style={baseStyle}>{initials(name)}</span>;
}
