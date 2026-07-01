import { ReactNode } from 'react';

/** Hover tooltip with configurable delay. Wrap any element. */
export interface TooltipProps {
  content: ReactNode;
  children: ReactNode;
  position?: 'top' | 'bottom' | 'left' | 'right';
  /** Delay in ms before appearing (default 400) */
  delay?: number;
}

export declare function Tooltip(props: TooltipProps): JSX.Element;
