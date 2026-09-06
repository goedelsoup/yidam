import React from 'react';

const PHASE = {
  Investigation: { bg: 'var(--phase-investigation-bg)', fg: 'var(--phase-investigation-fg)', border: 'var(--rigpa-100)' },
  Extraction:    { bg: 'var(--phase-extraction-bg)',    fg: 'var(--phase-extraction-fg)',    border: 'var(--gold-200)'  },
  Synthesis:     { bg: 'var(--phase-synthesis-bg)',     fg: 'var(--phase-synthesis-fg)',     border: 'var(--ma-200)'   },
  Assessment:    { bg: 'var(--phase-assessment-bg)',    fg: 'var(--phase-assessment-fg)',    border: 'var(--jade-200)'  },
};

export function PhaseTag({ phase }) {
  const s = PHASE[phase] || PHASE.Investigation;
  return (
    <span style={{
      display: 'inline-flex', alignItems: 'center',
      padding: '2px 8px',
      borderRadius: 'var(--radius-sm)',
      fontFamily: 'var(--font-ui)',
      fontSize: 'var(--text-xs)',
      fontWeight: 500,
      letterSpacing: 'var(--tracking-wider)',
      textTransform: 'uppercase',
      whiteSpace: 'nowrap',
      background: s.bg, color: s.fg,
      border: `1px solid ${s.border}`,
    }}>
      {phase}
    </span>
  );
}
