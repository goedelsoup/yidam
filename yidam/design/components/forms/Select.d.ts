import { CSSProperties, ChangeEventHandler } from 'react';

/**
 * Styled native select dropdown. Options can be strings or { value, label } objects.
 * Native select is used intentionally — consistent accessibility without JS complexity.
 */
export interface SelectOption {
  value: string;
  label: string;
}

export interface SelectProps {
  label?: string;
  value?: string;
  onChange?: ChangeEventHandler<HTMLSelectElement>;
  options?: (string | SelectOption)[];
  error?: string;
  helper?: string;
  disabled?: boolean;
  placeholder?: string;
  id?: string;
  style?: CSSProperties;
}

export declare function Select(props: SelectProps): JSX.Element;
