import React, { useState } from 'react';

const CLAIM_STYLES = {
  verified:  { bg: 'var(--verified-bg)',  fg: 'var(--verified-fg)',  border: 'var(--verified-border)'  },
  inference: { bg: 'var(--inference-bg)', fg: 'var(--inference-fg)', border: 'var(--inference-border)' },
  open:      { bg: 'var(--open-bg)',      fg: 'var(--open-fg)',      border: 'var(--open-border)'      },
};

export function NodeCard({
  path, title, excerpt, outgoing = 0, incoming = 0,
  lineCount, lastModified, markers = [],
  isOpenQuestion = false, onClick, style: styleProp,
}) {
  const [hovered, setHovered] = useState(false);
  const isInteractive = typeof onClick === 'function';

  return (
    <div
      onClick={onClick}
      onMouseEnter={() => isInteractive && setHovered(true)}
      onMouseLeave={() => isInteractive && setHovered(false)}
      style={{
        background: 'var(--surface-raised)',
        border: `1px solid ${hovered ? 'var(--border-strong)' : 'var(--border-ui)'}`,
        borderRadius: 'var(--radius-xl)',
        padding: 'var(--space-5) var(--space-6)',
        cursor: isInteractive ? 'pointer' : 'default',
        transition: 'border-color var(--duration-fast) var(--ease-standard)',
        boxShadow: 'var(--shadow-xs)',
        ...styleProp,
      }}
    >
      {/* Path */}
      <div style={{
        fontFamily: 'var(--font-mono)', fontSize: 'var(--text-xs)',
        color: 'var(--text-tertiary)', marginBottom: 'var(--space-2)',
        display: 'flex', alignItems: 'center', gap: '4px',
      }}>
        {isOpenQuestion && <span style={{ color: 'var(--open-fg)', fontWeight: 500 }}>?</span>}
        {path}
      </div>

      {/* Title */}
      <div style={{
        fontFamily: 'var(--font-display)', fontSize: 'var(--text-xl)', fontWeight: 500,
        color: 'var(--text-primary)', lineHeight: 'var(--leading-snug)',
        marginBottom: excerpt ? 'var(--space-3)' : 'var(--space-4)',
      }}>
        {title}
      </div>

      {/* Excerpt */}
      {excerpt && (
        <div style={{
          fontFamily: 'var(--font-serif)', fontSize: 'var(--text-sm)',
          color: 'var(--text-secondary)', lineHeight: 'var(--leading-relaxed)',
          marginBottom: 'var(--space-4)',
          display: '-webkit-box', WebkitLineClamp: 3,
          WebkitBoxOrient: 'vertical', overflow: 'hidden',
        }}>
          {excerpt}
        </div>
      )}

      {/* Claim markers */}
      {markers.length > 0 && (
        <div style={{ display: 'flex', gap: 'var(--space-1-5)', flexWrap: 'wrap', marginBottom: 'var(--space-4)' }}>
          {markers.map((m, i) => {
            const s = CLAIM_STYLES[m] || CLAIM_STYLES.open;
            return (
              <span key={i} style={{
                display: 'inline-flex', padding: '1px 6px',
                borderRadius: 'var(--radius-sm)', fontFamily: 'var(--font-mono)',
                fontSize: 'var(--text-xs)', fontWeight: 500,
                background: s.bg, color: s.fg, border: `1px solid ${s.border}`,
              }}>
                [{m}]
              </span>
            );
          })}
        </div>
      )}

      {/* Meta */}
      <div style={{
        display: 'flex', gap: 'var(--space-5)', alignItems: 'center',
        fontFamily: 'var(--font-ui)', fontSize: 'var(--text-xs)',
        color: 'var(--text-tertiary)', borderTop: '1px solid var(--border-subtle)',
        paddingTop: 'var(--space-3)', marginTop: markers.length === 0 && !excerpt ? 0 : undefined,
      }}>
        <span title="Outgoing edges" style={{ display: 'flex', alignItems: 'center', gap: '3px' }}>
          <svg width="10" height="10" viewBox="0 0 10 10" fill="none">
            <path d="M2 5h6M6 2.5l3 2.5-3 2.5" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" strokeLinejoin="round"/>
          </svg>
          {outgoing}
        </span>
        <span title="Incoming edges" style={{ display: 'flex', alignItems: 'center', gap: '3px' }}>
          <svg width="10" height="10" viewBox="0 0 10 10" fill="none">
            <path d="M8 5H2M4 2.5L1 5l3 2.5" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" strokeLinejoin="round"/>
          </svg>
          {incoming}
        </span>
        {lineCount != null && <span>{lineCount} lines</span>}
        {lastModified && <span>{lastModified}</span>}
      </div>
    </div>
  );
}
