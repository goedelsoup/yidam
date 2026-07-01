import { ChangeEventHandler } from 'react';

/** Single radio button. Group multiple with the same `name` attribute. */
export interface RadioProps {
  checked?: boolean;
  onChange?: ChangeEventHandler<HTMLInputElement>;
  label?: string;
  disabled?: boolean;
  name?: string;
  value?: string;
  id?: string;
}

export declare function Radio(props: RadioProps): JSX.Element;
