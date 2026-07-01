import React from 'react';

export function Tabs({ tabs = [], activeTab, onChange, size = 'md', variant = 'underline', children }) {
  return (
    <div>
      <div
        role="tablist"
        style={{
          display: 'flex',
          borderBottom: variant === 'underline' ? '1px solid var(--border-ui)' : 'none',
          gap: variant === 'pill' ? 'var(--space-1)' : 0,
          background: variant === 'pill' ? 'var(--surface-overlay)' : 'transparent',
          borderRadius: variant === 'pill' ? 'var(--radius-lg)' : 0,
          padding: variant === 'pill' ? '3px' : 0,
        }}
      >
        {tabs.map(tab => {
          const isActive = tab.id === activeTab;
          const isDisabled = tab.disabled;
          if (variant === 'pill') {
            return (
              <button key={tab.id} role="tab" aria-selected={isActive}
                onClick={() => !isDisabled && onChange && onChange(tab.id)}
                style={{
                  padding: size === 'sm' ? '4px 12px' : '5px 14px',
                  fontFamily: 'var(--font-ui)',
                  fontSize: size === 'sm' ? 'var(--text-xs)' : 'var(--text-sm)',
                  fontWeight: isActive ? 500 : 400,
                  color: isActive ? 'var(--text-primary)' : 'var(--text-secondary)',
                  background: isActive ? 'var(--surface-raised)' : 'transparent',
                  border: 'none', borderRadius: 'var(--radius-md)',
                  cursor: isDisabled ? 'not-allowed' : 'pointer',
                  opacity: isDisabled ? 0.4 : 1,
                  boxShadow: isActive ? 'var(--shadow-sm)' : 'none',
                  transition: 'all var(--duration-fast) var(--ease-standard)',
                  whiteSpace: 'nowrap',
                }}>
                {tab.label}
              </button>
            );
          }
          return (
            <button key={tab.id} role="tab" aria-selected={isActive}
              onClick={() => !isDisabled && onChange && onChange(tab.id)}
              style={{
                padding: size === 'sm' ? '6px 12px' : '8px 16px',
                fontFamily: 'var(--font-ui)',
                fontSize: size === 'sm' ? 'var(--text-xs)' : 'var(--text-sm)',
                fontWeight: isActive ? 500 : 400,
                color: isActive ? 'var(--text-primary)' : 'var(--text-secondary)',
                background: 'transparent', border: 'none',
                borderBottom: `2px solid ${isActive ? 'var(--gold-500)' : 'transparent'}`,
                marginBottom: '-1px', cursor: isDisabled ? 'not-allowed' : 'pointer',
                opacity: isDisabled ? 0.4 : 1,
                transition: 'color var(--duration-fast) var(--ease-standard), border-color var(--duration-fast) var(--ease-standard)',
                whiteSpace: 'nowrap', display: 'flex', alignItems: 'center', gap: '6px',
              }}>
              {tab.label}
              {tab.count !== undefined && (
                <span style={{
                  background: isActive ? 'var(--gold-100)' : 'var(--ink-100)',
                  color: isActive ? 'var(--gold-800)' : 'var(--text-tertiary)',
                  padding: '0 5px', borderRadius: 'var(--radius-full)',
                  fontSize: 'var(--text-2xs)', fontWeight: 500, lineHeight: '16px',
                }}>
                  {tab.count}
                </span>
              )}
            </button>
          );
        })}
      </div>
      {children && <div style={{ paddingTop: 'var(--space-4)' }}>{children}</div>}
    </div>
  );
}
