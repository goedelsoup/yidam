import { ReactNode } from 'react';

/**
 * Modal dialog with backdrop. Click-outside and the × button both close.
 * Scroll is contained within the body area.
 * @startingPoint section="Feedback" subtitle="Modal dialog with header, body, footer" viewport="600x360"
 */
export interface DialogProps {
  open: boolean;
  onClose?: () => void;
  title: string;
  children: ReactNode;
  /** Rendered in the footer row — typically Button elements */
  footer?: ReactNode;
  size?: 'sm' | 'md' | 'lg' | 'xl';
}

export declare function Dialog(props: DialogProps): JSX.Element;
