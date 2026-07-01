import { ReactNode } from 'react';

/**
 * Small semantic label. Variants map to the system's color vocabulary:
 * gold for earned/primary, rigpa for resolved states, ma for active inquiry,
 * and the three claim-state variants for epistemic annotation.
 */
export interface BadgeProps {
  variant?: 'default' | 'gold' | 'rigpa' | 'ma' | 'verified' | 'inference' | 'open' | 'inverse';
  size?: 'sm' | 'md';
  /** Prepends a small colored dot */
  dot?: boolean;
  children: ReactNode;
}

export declare function Badge(props: BadgeProps): JSX.Element;
