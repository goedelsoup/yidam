import React, { useState } from 'react';

export function Card({
  children,
  padding = 'md',
  shadow = 'xs',
  border = true,
  onClick,
  as: Tag = 'div',
  style: styleProp,
  ...rest
}) {
  const [hovered, setHovered] = useState(false);
  const isInteractive = typeof onClick === 'function';

  const paddingSizes = {
    none: '0',
    sm:   'var(--space-4)',
    md:   'var(--space-5) var(--space-6)',
    lg:   'var(--space-6) var(--space-8)',
  };

  const shadows = {
    none: 'none',
    xs:   'var(--shadow-xs)',
    sm:   'var(--shadow-sm)',
    md:   'var(--shadow-md)',
  };

  return (
    <Tag
      onClick={onClick}
      onMouseEnter={() => isInteractive && setHovered(true)}
      onMouseLeave={() => isInteractive && setHovered(false)}
      style={{
        background: 'var(--surface-raised)',
        borderRadius: 'var(--radius-xl)',
        border: border ? `1px solid ${hovered && isInteractive ? 'var(--border-strong)' : 'var(--border-ui)'}` : 'none',
        boxShadow: shadows[shadow] || shadows.xs,
        padding: paddingSizes[padding] || paddingSizes.md,
        cursor: isInteractive ? 'pointer' : 'default',
        transition: 'border-color var(--duration-base) var(--ease-standard)',
        ...styleProp,
      }}
      {...rest}
    >
      {children}
    </Tag>
  );
}
