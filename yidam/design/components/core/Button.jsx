import React, { useState } from 'react';

const SIZES = {
  sm: { padding: '5px 12px', fontSize: 'var(--text-xs)',  minHeight: '30px' },
  md: { padding: '7px 16px', fontSize: 'var(--text-sm)',  minHeight: '36px' },
  lg: { padding: '10px 20px', fontSize: 'var(--text-base)', minHeight: '42px' },
};

function getVariantStyle(variant, hovered, pressed) {
  if (variant === 'primary') return {
    background: pressed ? 'var(--action-bg-active)' : hovered ? 'var(--action-bg-hover)' : 'var(--action-bg)',
    color: 'var(--action-fg)',
    borderColor: pressed ? 'var(--action-bg-active)' : hovered ? 'var(--action-bg-hover)' : 'var(--action-bg)',
  };
  if (variant === 'ghost') return {
    background: pressed ? 'var(--action-ghost-bg-active)' : hovered ? 'var(--action-ghost-bg-hover)' : 'transparent',
    color: 'var(--action-ghost-fg)',
    borderColor: 'var(--action-ghost-border)',
  };
  if (variant === 'subtle') return {
    background: pressed ? 'var(--ink-100)' : hovered ? 'var(--ink-50)' : 'transparent',
    color: 'var(--text-secondary)',
    borderColor: 'transparent',
  };
  if (variant === 'danger') return {
    background: pressed ? 'var(--danger-bg-active)' : hovered ? 'var(--danger-bg-hover)' : 'var(--danger-bg)',
    color: 'var(--danger-fg)',
    borderColor: 'transparent',
  };
  return {};
}

export function Button({
  variant = 'primary',
  size = 'md',
  disabled = false,
  type = 'button',
  children,
  onClick,
  style: styleProp,
  ...rest
}) {
  const [hovered, setHovered] = useState(false);
  const [pressed, setPressed] = useState(false);

  return (
    <button
      type={type}
      disabled={disabled}
      onClick={onClick}
      onMouseEnter={() => !disabled && setHovered(true)}
      onMouseLeave={() => { setHovered(false); setPressed(false); }}
      onMouseDown={() => !disabled && setPressed(true)}
      onMouseUp={() => setPressed(false)}
      style={{
        display: 'inline-flex', alignItems: 'center', justifyContent: 'center',
        gap: '6px', fontFamily: 'var(--font-ui)', fontWeight: 500,
        letterSpacing: '0.01em', borderRadius: 'var(--radius-md)',
        border: '1px solid transparent', cursor: disabled ? 'not-allowed' : 'pointer',
        opacity: disabled ? 0.45 : 1, transition: 'var(--transition-colors)',
        whiteSpace: 'nowrap', lineHeight: 1, userSelect: 'none',
        ...SIZES[size],
        ...getVariantStyle(variant, hovered && !disabled, pressed && !disabled),
        ...styleProp,
      }}
      {...rest}
    >
      {children}
    </button>
  );
}
