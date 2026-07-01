import React, { useState, useRef } from 'react';

const POSITION_STYLE = {
  top:    { bottom: 'calc(100% + 7px)', left: '50%', transform: 'translateX(-50%)' },
  bottom: { top:    'calc(100% + 7px)', left: '50%', transform: 'translateX(-50%)' },
  left:   { right:  'calc(100% + 7px)', top:  '50%', transform: 'translateY(-50%)' },
  right:  { left:   'calc(100% + 7px)', top:  '50%', transform: 'translateY(-50%)' },
};

export function Tooltip({ content, children, position = 'top', delay = 400 }) {
  const [visible, setVisible] = useState(false);
  const timer = useRef(null);

  function show() {
    clearTimeout(timer.current);
    timer.current = setTimeout(() => setVisible(true), delay);
  }
  function hide() {
    clearTimeout(timer.current);
    setVisible(false);
  }

  return (
    <span
      style={{ position: 'relative', display: 'inline-flex' }}
      onMouseEnter={show}
      onMouseLeave={hide}
      onFocus={show}
      onBlur={hide}
    >
      {children}
      {visible && content && (
        <span style={{
          position: 'absolute', zIndex: 1000,
          background: 'var(--ink-900)',
          color: 'var(--text-inverse)',
          padding: '5px 9px',
          borderRadius: 'var(--radius-md)',
          fontFamily: 'var(--font-ui)',
          fontSize: 'var(--text-xs)',
          fontWeight: 400,
          whiteSpace: 'nowrap',
          boxShadow: 'var(--shadow-md)',
          pointerEvents: 'none',
          lineHeight: 1.4,
          ...(POSITION_STYLE[position] || POSITION_STYLE.top),
        }}>
          {content}
        </span>
      )}
    </span>
  );
}
