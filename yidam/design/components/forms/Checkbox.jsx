import React, { useState } from 'react';

export function Checkbox({ checked = false, onChange, label, disabled = false, id }) {
  const [focused, setFocused] = useState(false);
  const cbId = id || (label ? `cb-${label.toLowerCase().replace(/\s+/g, '-')}` : undefined);

  return (
    <label htmlFor={cbId} style={{
      display: 'inline-flex', alignItems: 'flex-start', gap: 'var(--space-2)',
      cursor: disabled ? 'not-allowed' : 'pointer',
      opacity: disabled ? 0.5 : 1, userSelect: 'none',
    }}>
      <span style={{ position: 'relative', display: 'flex', flexShrink: 0, marginTop: '2px' }}>
        <input
          id={cbId} type="checkbox" checked={checked} onChange={onChange}
          disabled={disabled}
          onFocus={() => setFocused(true)}
          onBlur={() => setFocused(false)}
          style={{ position: 'absolute', opacity: 0, width: '100%', height: '100%', cursor: 'inherit', margin: 0 }}
        />
        <span style={{
          width: '16px', height: '16px', borderRadius: 'var(--radius-xs)',
          border: `1px solid ${focused ? 'var(--border-focus)' : checked ? 'var(--gold-500)' : 'var(--border-ui)'}`,
          background: checked ? 'var(--gold-500)' : 'var(--surface-raised)',
          display: 'flex', alignItems: 'center', justifyContent: 'center',
          boxShadow: focused ? 'var(--shadow-focus-gold)' : 'none',
          transition: 'all var(--duration-fast) var(--ease-standard)',
          flexShrink: 0,
        }}>
          {checked && (
            <svg width="10" height="8" viewBox="0 0 10 8" fill="none">
              <path d="M1 4l3 3 5-6" stroke="var(--action-fg)" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round"/>
            </svg>
          )}
        </span>
      </span>
      {label && (
        <span style={{
          fontFamily: 'var(--font-ui)', fontSize: 'var(--text-sm)',
          color: 'var(--text-primary)', lineHeight: 1.5,
        }}>
          {label}
        </span>
      )}
    </label>
  );
}
