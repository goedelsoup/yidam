import { ChangeEventHandler } from 'react';

/**
 * Custom checkbox. Checked state uses gold fill. Indeterminate state not supported —
 * if you need it, manage visually via the `checked` prop + custom label.
 */
export interface CheckboxProps {
  checked?: boolean;
  onChange?: ChangeEventHandler<HTMLInputElement>;
  label?: string;
  disabled?: boolean;
  id?: string;
}

export declare function Checkbox(props: CheckboxProps): JSX.Element;
