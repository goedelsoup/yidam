import { CSSProperties } from 'react';

/**
 * Summary card for a single corpus node. Displays path, title, excerpt,
 * claim markers, and edge counts. Becomes interactive when `onClick` is provided.
 *
 * @startingPoint section="Knowledge" subtitle="Corpus node summary card" viewport="560x240"
 */
export interface NodeCardProps {
  /** Relative path within the corpus (monospace display) */
  path?: string;
  title: string;
  /** Short prose excerpt (clamped to 3 lines) */
  excerpt?: string;
  outgoing?: number;
  incoming?: number;
  lineCount?: number;
  /** Relative date string e.g. "3 days ago" */
  lastModified?: string;
  /** Array of claim types present in the node */
  markers?: ('verified' | 'inference' | 'open')[];
  /** Prepends ? to path and title */
  isOpenQuestion?: boolean;
  onClick?: () => void;
  style?: CSSProperties;
}

export declare function NodeCard(props: NodeCardProps): JSX.Element;
