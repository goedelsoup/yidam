import React, { useState } from 'react';

export function Radio({ checked = false, onChange, label, disabled = false, name, value, id }) {
  const [focused, setFocused] = useState(false);
  const radioId = id || (label ? `radio-${label.toLowerCase().replace(/\s+/g, '-')}` : undefined);

  return (
    <label htmlFor={radioId} style={{
      display: 'inline-flex', alignItems: 'flex-start', gap: 'var(--space-2)',
      cursor: disabled ? 'not-allowed' : 'pointer',
      opacity: disabled ? 0.5 : 1, userSelect: 'none',
    }}>
      <span style={{ position: 'relative', display: 'flex', flexShrink: 0, marginTop: '2px' }}>
        <input
          id={radioId} type="radio" checked={checked} onChange={onChange}
          name={name} value={value} disabled={disabled}
          onFocus={() => setFocused(true)}
          onBlur={() => setFocused(false)}
          style={{ position: 'absolute', opacity: 0, width: '100%', height: '100%', cursor: 'inherit', margin: 0 }}
        />
        <span style={{
          width: '16px', height: '16px', borderRadius: '50%',
          border: `1px solid ${focused ? 'var(--border-focus)' : checked ? 'var(--gold-500)' : 'var(--border-ui)'}`,
          background: 'var(--surface-raised)',
          display: 'flex', alignItems: 'center', justifyContent: 'center',
          boxShadow: focused ? 'var(--shadow-focus-gold)' : 'none',
          transition: 'all var(--duration-fast) var(--ease-standard)',
          flexShrink: 0,
        }}>
          {checked && (
            <span style={{ width: '7px', height: '7px', borderRadius: '50%', background: 'var(--gold-500)' }} />
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
