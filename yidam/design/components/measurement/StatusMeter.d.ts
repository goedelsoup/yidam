/**
 * A test run's outcome as a proportional bar.
 *
 * `asserted` is required and is what the pass segment is drawn from — never `passed`. A
 * runtime skip is recorded by a runner as a pass, so a suite that exercised nothing has the
 * same `passed` as one that exercised everything; `asserted` is the count that separates
 * them, and requiring it is what stops a page from drawing a fully-skipped suite green.
 */
export interface StatusMeterProps {
  /** Tests that ran and exercised their subject. `passed` minus the skips among it. */
  asserted: number;
  failed?: number;
  /** Gated plus ignored. A skip is not a pass and is not a failure. */
  skipped?: number;
  label?: string;
  size?: 'sm' | 'md';
}

export declare function StatusMeter(props: StatusMeterProps): JSX.Element;
