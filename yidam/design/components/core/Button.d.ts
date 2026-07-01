import { ReactNode, CSSProperties, MouseEventHandler } from 'react';

/**
 * Primary action surface. Use `primary` for the single most important action,
 * `ghost` for secondary actions alongside a primary, `subtle` for tertiary
 * or icon-adjacent contexts. Reserve `danger` for destructive confirmation steps.
 *
 * @startingPoint section="Core" subtitle="Action button — primary / ghost / subtle" viewport="500x140"
 */
export interface ButtonProps {
  /** Visual variant */
  variant?: 'primary' | 'ghost' | 'subtle' | 'danger';
  /** Size */
  size?: 'sm' | 'md' | 'lg';
  /** Disables click and applies reduced opacity */
  disabled?: boolean;
  /** HTML type attribute */
  type?: 'button' | 'submit' | 'reset';
  children: ReactNode;
  onClick?: MouseEventHandler<HTMLButtonElement>;
  style?: CSSProperties;
}

export declare function Button(props: ButtonProps): JSX.Element;
