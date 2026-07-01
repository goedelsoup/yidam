import React, { useState } from 'react';

export function Switch({ checked = false, onChange, label, disabled = false }) {
  const [focused, setFocused] = useState(false);

  return (
    <label style={{
      display: 'inline-flex', alignItems: 'center', gap: 'var(--space-3)',
      cursor: disabled ? 'not-allowed' : 'pointer',
      opacity: disabled ? 0.5 : 1, userSelect: 'none',
    }}>
      <span style={{ position: 'relative', display: 'flex', flexShrink: 0 }}>
        <input
          type="checkbox" checked={checked} onChange={onChange} disabled={disabled}
          onFocus={() => setFocused(true)}
          onBlur={() => setFocused(false)}
          style={{ position: 'absolute', opacity: 0, width: '100%', height: '100%', cursor: 'inherit', margin: 0 }}
        />
        <span style={{
          width: '36px', height: '20px', borderRadius: 'var(--radius-full)',
          background: checked ? 'var(--gold-500)' : 'var(--ink-200)',
          boxShadow: focused ? 'var(--shadow-focus-gold)' : 'none',
          transition: 'background var(--duration-base) var(--ease-standard), box-shadow var(--duration-fast) var(--ease-standard)',
          display: 'block', position: 'relative', flexShrink: 0,
        }}>
          <span style={{
            position: 'absolute', top: '3px',
            left: checked ? '19px' : '3px',
            width: '14px', height: '14px',
            borderRadius: '50%', background: 'white',
            boxShadow: 'var(--shadow-sm)',
            transition: 'left var(--duration-base) var(--ease-standard)',
          }} />
        </span>
      </span>
      {label && (
        <span style={{ fontFamily: 'var(--font-ui)', fontSize: 'var(--text-sm)', color: 'var(--text-primary)' }}>
          {label}
        </span>
      )}
    </label>
  );
}
