import { ReactNode } from 'react';

/**
 * Tabbed navigation. Two visual variants: `underline` (default, horizontal rule with gold active indicator)
 * and `pill` (segmented control style).
 * @startingPoint section="Navigation" subtitle="Tab bar — underline or pill variant" viewport="600x120"
 */
export interface TabItem {
  id: string;
  label: string;
  count?: number;
  disabled?: boolean;
}

export interface TabsProps {
  tabs: TabItem[];
  activeTab?: string;
  onChange?: (id: string) => void;
  size?: 'sm' | 'md';
  variant?: 'underline' | 'pill';
  /** Optional content panel rendered below the tab bar */
  children?: ReactNode;
}

export declare function Tabs(props: TabsProps): JSX.Element;
