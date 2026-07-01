import { CSSProperties } from 'react';

/**
 * Elector or agent avatar. Human electors render as circles (ma-rose tint);
 * agent electors render as rounded squares (rigpa-blue tint) to distinguish
 * participant types without privileging either.
 */
export interface AvatarProps {
  /** Elector name — used to generate initials */
  name?: string;
  /** Optional image URL */
  src?: string;
  size?: 'xs' | 'sm' | 'md' | 'lg';
  /** human → circle (ma tint) · agent → rounded square (rigpa tint) */
  variant?: 'human' | 'agent';
  style?: CSSProperties;
}

export declare function Avatar(props: AvatarProps): JSX.Element;
