import React, { useState } from 'react';

export function Textarea({
  label, value, onChange, placeholder, error, helper,
  disabled = false, rows = 4, id, style: styleProp, ...rest
}) {
  const [focused, setFocused] = useState(false);
  const areaId = id || (label ? `ta-${label.toLowerCase().replace(/\s+/g, '-')}` : undefined);

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 'var(--space-1-5)', ...styleProp }}>
      {label && (
        <label htmlFor={areaId} style={{
          fontFamily: 'var(--font-ui)', fontSize: 'var(--text-sm)', fontWeight: 500,
          color: error ? '#c8342a' : 'var(--text-primary)',
        }}>
          {label}
        </label>
      )}
      <textarea
        id={areaId} value={value} onChange={onChange}
        placeholder={placeholder} disabled={disabled} rows={rows}
        onFocus={() => setFocused(true)}
        onBlur={() => setFocused(false)}
        style={{
          width: '100%', boxSizing: 'border-box',
          fontFamily: 'var(--font-serif)',
          fontSize: 'var(--text-sm)',
          color: 'var(--text-primary)',
          background: disabled ? 'var(--surface-overlay)' : 'var(--surface-raised)',
          border: `1px solid ${error ? '#c8342a' : focused ? 'var(--border-focus)' : 'var(--border-ui)'}`,
          borderRadius: 'var(--radius-md)',
          padding: '8px 12px',
          outline: 'none', resize: 'vertical',
          boxShadow: focused ? 'var(--shadow-focus-gold)' : 'var(--shadow-inset)',
          lineHeight: 'var(--leading-relaxed)',
          transition: 'border-color var(--duration-fast) var(--ease-standard), box-shadow var(--duration-fast) var(--ease-standard)',
          cursor: disabled ? 'not-allowed' : 'text',
          opacity: disabled ? 0.6 : 1,
        }}
        {...rest}
      />
      {(error || helper) && (
        <span style={{
          fontFamily: 'var(--font-ui)', fontSize: 'var(--text-xs)',
          color: error ? '#c8342a' : 'var(--text-tertiary)',
        }}>
          {error || helper}
        </span>
      )}
    </div>
  );
}
