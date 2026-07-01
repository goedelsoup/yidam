import { ReactNode } from 'react';

/** Hierarchical path navigation. Use `mono` for corpus node paths. */
export interface BreadcrumbItem {
  label: string;
  href?: string;
}

export interface BreadcrumbProps {
  items: BreadcrumbItem[];
  /** Use monospace font — for .yidam/corpus/ style paths */
  mono?: boolean;
}

export declare function Breadcrumb(props: BreadcrumbProps): JSX.Element;
