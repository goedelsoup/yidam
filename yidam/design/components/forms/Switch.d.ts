import { ChangeEventHandler } from 'react';

/** Toggle switch for boolean settings. Gold when on. */
export interface SwitchProps {
  checked?: boolean;
  onChange?: ChangeEventHandler<HTMLInputElement>;
  label?: string;
  disabled?: boolean;
}

export declare function Switch(props: SwitchProps): JSX.Element;
