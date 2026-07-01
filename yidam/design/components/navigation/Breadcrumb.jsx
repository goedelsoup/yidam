import React from 'react';

export function Breadcrumb({ items = [], mono = false }) {
  return (
    <nav aria-label="Breadcrumb">
      <ol style={{
        display: 'flex', alignItems: 'center', flexWrap: 'wrap',
        gap: '0', listStyle: 'none', margin: 0, padding: 0,
        fontFamily: mono ? 'var(--font-mono)' : 'var(--font-ui)',
        fontSize: 'var(--text-xs)',
      }}>
        {items.map((item, i) => {
          const isLast = i === items.length - 1;
          return (
            <li key={i} style={{ display: 'flex', alignItems: 'center' }}>
              {i > 0 && (
                <span style={{ margin: '0 var(--space-1)', color: 'var(--text-tertiary)', userSelect: 'none' }}>
                  <svg width="10" height="10" viewBox="0 0 10 10" fill="none">
                    <path d="M3 2l4 3-4 3" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" strokeLinejoin="round"/>
                  </svg>
                </span>
              )}
              {isLast ? (
                <span style={{ color: 'var(--text-primary)', fontWeight: 500 }}>{item.label}</span>
              ) : (
                <a href={item.href || '#'} style={{
                  color: 'var(--text-tertiary)', textDecoration: 'none',
                  transition: 'color var(--duration-fast)',
                }}
                  onMouseEnter={e => e.currentTarget.style.color = 'var(--text-secondary)'}
                  onMouseLeave={e => e.currentTarget.style.color = 'var(--text-tertiary)'}
                >
                  {item.label}
                </a>
              )}
            </li>
          );
        })}
      </ol>
    </nav>
  );
}
