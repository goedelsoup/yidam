import { ReactNode, CSSProperties, ElementType } from 'react';

/**
 * Raised content container. White background, 1px border, small radius.
 * Becomes interactive (border strengthens on hover) when `onClick` is provided.
 *
 * @startingPoint section="Core" subtitle="Content container card" viewport="500x180"
 */
export interface CardProps {
  children: ReactNode;
  padding?: 'none' | 'sm' | 'md' | 'lg';
  shadow?: 'none' | 'xs' | 'sm' | 'md';
  /** Show 1px border (default true) */
  border?: boolean;
  /** Providing onClick makes the card interactive with a hover state */
  onClick?: () => void;
  /** Rendered HTML element or component (default div) */
  as?: ElementType;
  style?: CSSProperties;
}

export declare function Card(props: CardProps): JSX.Element;
