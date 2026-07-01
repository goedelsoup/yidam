import React from 'react';

export function BranchRef({ branch, short = false, commit }) {
  const isMa    = branch?.startsWith('ma/');
  const isRigpa = branch?.startsWith('rigpa/');

  const styles = isMa ? {
    background: 'var(--ma-50)',    color: 'var(--ma-700)',    border: '1px solid var(--ma-200)',
  } : isRigpa ? {
    background: 'var(--rigpa-50)', color: 'var(--rigpa-700)', border: '1px solid var(--rigpa-200)',
  } : {
    background: 'var(--ink-50)',   color: 'var(--ink-600)',   border: '1px solid var(--ink-200)',
  };

  const prefix = isMa ? 'ma/' : isRigpa ? 'rigpa/' : '';
  const stem   = branch?.slice(prefix.length) ?? branch;
  const display = short ? stem : branch;

  return (
    <span style={{ display: 'inline-flex', alignItems: 'center', gap: '5px', flexShrink: 0 }}>
      <span style={{
        display: 'inline-flex', alignItems: 'center',
        padding: '1px 7px', borderRadius: 'var(--radius-sm)',
        fontFamily: 'var(--font-mono)', fontSize: 'var(--text-xs)',
        fontWeight: 400, whiteSpace: 'nowrap', lineHeight: 1.6,
        ...styles,
      }}>
        {display}
      </span>
      {commit && (
        <span style={{
          fontFamily: 'var(--font-mono)', fontSize: 'var(--text-xs)',
          color: 'var(--text-tertiary)',
        }}>
          @{commit.slice(0, 7)}
        </span>
      )}
    </span>
  );
}
