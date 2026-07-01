import { ReactNode, CSSProperties, ChangeEventHandler } from 'react';

/**
 * Labelled text input with focus ring, error state, and optional prefix/suffix decorators.
 * @startingPoint section="Forms" subtitle="Text input with label and error state" viewport="480x160"
 */
export interface InputProps {
  label?: string;
  value?: string;
  onChange?: ChangeEventHandler<HTMLInputElement>;
  placeholder?: string;
  error?: string;
  helper?: string;
  disabled?: boolean;
  size?: 'sm' | 'md';
  /** Leading decorator (icon or text) */
  prefix?: ReactNode;
  /** Trailing decorator (icon or text) */
  suffix?: ReactNode;
  type?: string;
  id?: string;
  style?: CSSProperties;
}

export declare function Input(props: InputProps): JSX.Element;
