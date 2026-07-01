import { ReactNode } from 'react';

/**
 * Removable label chip. Use for user-editable lists of concepts, ontology classes,
 * or linked node references. For read-only labels use Badge instead.
 */
export interface TagProps {
  children: ReactNode;
  /** If provided, renders a remove (×) button */
  onRemove?: () => void;
  variant?: 'default' | 'gold' | 'rigpa' | 'ma';
  disabled?: boolean;
}

export declare function Tag(props: TagProps): JSX.Element;
