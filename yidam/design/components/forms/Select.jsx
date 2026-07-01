import React, { useState } from 'react';

export function Select({
  label, value, onChange, options = [], error, helper,
  disabled = false, placeholder, id, style: styleProp, ...rest
}) {
  const [focused, setFocused] = useState(false);
  const selectId = id || (label ? `sel-${label.toLowerCase().replace(/\s+/g, '-')}` : undefined);

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 'var(--space-1-5)', ...styleProp }}>
      {label && (
        <label htmlFor={selectId} style={{
          fontFamily: 'var(--font-ui)', fontSize: 'var(--text-sm)', fontWeight: 500,
          color: error ? '#c8342a' : 'var(--text-primary)',
        }}>
          {label}
        </label>
      )}
      <div style={{ position: 'relative' }}>
        <select
          id={selectId} value={value} onChange={onChange}
          disabled={disabled}
          onFocus={() => setFocused(true)}
          onBlur={() => setFocused(false)}
          style={{
            width: '100%', appearance: 'none', boxSizing: 'border-box',
            fontFamily: 'var(--font-ui)', fontSize: 'var(--text-sm)',
            color: value ? 'var(--text-primary)' : 'var(--text-tertiary)',
            background: disabled ? 'var(--surface-overlay)' : 'var(--surface-raised)',
            border: `1px solid ${error ? '#c8342a' : focused ? 'var(--border-focus)' : 'var(--border-ui)'}`,
            borderRadius: 'var(--radius-md)',
            padding: '8px 32px 8px 12px',
            outline: 'none', cursor: disabled ? 'not-allowed' : 'pointer',
            boxShadow: focused ? 'var(--shadow-focus-gold)' : 'var(--shadow-inset)',
            transition: 'border-color var(--duration-fast) var(--ease-standard), box-shadow var(--duration-fast) var(--ease-standard)',
            opacity: disabled ? 0.6 : 1,
          }}
          {...rest}
        >
          {placeholder && <option value="" disabled hidden>{placeholder}</option>}
          {options.map(opt => {
            const val = typeof opt === 'object' ? opt.value : opt;
            const lbl = typeof opt === 'object' ? opt.label : opt;
            return <option key={val} value={val}>{lbl}</option>;
          })}
        </select>
        <span style={{
          position: 'absolute', right: '10px', top: '50%', transform: 'translateY(-50%)',
          pointerEvents: 'none', color: 'var(--text-tertiary)',
        }}>
          <svg width="12" height="12" viewBox="0 0 12 12" fill="none">
            <path d="M2 4l4 4 4-4" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round" strokeLinejoin="round"/>
          </svg>
        </span>
      </div>
      {(error || helper) && (
        <span style={{ fontFamily: 'var(--font-ui)', fontSize: 'var(--text-xs)', color: error ? '#c8342a' : 'var(--text-tertiary)' }}>
          {error || helper}
        </span>
      )}
    </div>
  );
}
