import { CSSProperties, ChangeEventHandler } from 'react';

/**
 * Multi-line text input for long-form corpus content, commit message authoring,
 * and node description entry. Uses --font-serif for a more contemplative editing feel.
 */
export interface TextareaProps {
  label?: string;
  value?: string;
  onChange?: ChangeEventHandler<HTMLTextAreaElement>;
  placeholder?: string;
  error?: string;
  helper?: string;
  disabled?: boolean;
  rows?: number;
  id?: string;
  style?: CSSProperties;
}

export declare function Textarea(props: TextareaProps): JSX.Element;
