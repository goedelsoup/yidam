import React, { useState } from 'react';

export function Input({
  label, value, onChange, placeholder, error, helper,
  disabled = false, size = 'md', prefix, suffix,
  type = 'text', id, style: styleProp, ...rest
}) {
  const [focused, setFocused] = useState(false);
  const inputId = id || (label ? `input-${label.toLowerCase().replace(/\s+/g, '-')}` : undefined);

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 'var(--space-1-5)', ...styleProp }}>
      {label && (
        <label htmlFor={inputId} style={{
          fontFamily: 'var(--font-ui)', fontSize: 'var(--text-sm)', fontWeight: 500,
          color: error ? '#c8342a' : 'var(--text-primary)',
        }}>
          {label}
        </label>
      )}
      <div style={{ position: 'relative', display: 'flex', alignItems: 'center' }}>
        {prefix && (
          <span style={{
            position: 'absolute', left: '10px', color: 'var(--text-tertiary)',
            display: 'flex', alignItems: 'center', pointerEvents: 'none',
          }}>
            {prefix}
          </span>
        )}
        <input
          id={inputId} type={type} value={value} onChange={onChange}
          placeholder={placeholder} disabled={disabled}
          onFocus={() => setFocused(true)}
          onBlur={() => setFocused(false)}
          style={{
            width: '100%', boxSizing: 'border-box',
            fontFamily: 'var(--font-ui)',
            fontSize: size === 'sm' ? 'var(--text-xs)' : 'var(--text-sm)',
            color: 'var(--text-primary)',
            background: disabled ? 'var(--surface-overlay)' : 'var(--surface-raised)',
            border: `1px solid ${error ? '#c8342a' : focused ? 'var(--border-focus)' : 'var(--border-ui)'}`,
            borderRadius: 'var(--radius-md)',
            padding: size === 'sm' ? '5px 10px' : '8px 12px',
            paddingLeft: prefix ? '32px' : undefined,
            paddingRight: suffix ? '32px' : undefined,
            outline: 'none',
            boxShadow: focused ? 'var(--shadow-focus-gold)' : 'var(--shadow-inset)',
            transition: 'border-color var(--duration-fast) var(--ease-standard), box-shadow var(--duration-fast) var(--ease-standard)',
            cursor: disabled ? 'not-allowed' : 'text',
            opacity: disabled ? 0.6 : 1,
          }}
          {...rest}
        />
        {suffix && (
          <span style={{
            position: 'absolute', right: '10px', color: 'var(--text-tertiary)',
            display: 'flex', alignItems: 'center', pointerEvents: 'none',
          }}>
            {suffix}
          </span>
        )}
      </div>
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
