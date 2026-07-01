import { ReactNode } from 'react';

/**
 * Inline epistemic annotation marker for corpus node prose.
 * Renders at 0.78em of the surrounding text size so it feels inline, not raised.
 *
 * The three types map to the system's epistemic vocabulary:
 * - verified   — supported by a committed primary source
 * - inference  — a reasonable conclusion; not directly witnessed
 * - open       — a live question; unknown, contested, or under investigation
 *
 * @startingPoint section="Knowledge" subtitle="Inline epistemic claim marker" viewport="560x80"
 */
export interface ClaimMarkerProps {
  type: 'verified' | 'inference' | 'open';
  /** Optional italic annotation appended after the bracket type */
  annotation?: string;
}

export declare function ClaimMarker(props: ClaimMarkerProps): JSX.Element;
