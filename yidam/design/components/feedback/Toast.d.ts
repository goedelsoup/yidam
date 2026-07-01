import { ReactNode } from 'react';

/** Transient notification. Render in a fixed-position toast region at the bottom of the viewport. */
export interface ToastProps {
  message: string;
  type?: 'info' | 'success' | 'error' | 'warning';
  /** Optional secondary line */
  detail?: string;
  /** If provided, renders a dismiss button */
  onDismiss?: () => void;
}

export declare function Toast(props: ToastProps): JSX.Element;
