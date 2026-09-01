/**
 * A series as a shape — direction, not magnitude.
 *
 * Fewer than two points renders as a stated absence rather than a flat line: one measurement
 * is not a trend, and drawing it as one invents a history that does not exist.
 */
export interface SparklineProps {
  /** Oldest first. The last point is the one shown as a number. */
  points: number[];
  label: string;
  /** Renders the headline number. Without it the raw value is shown. */
  format?: (value: number) => string;
  /** Which direction is a regression. Decides the stroke colour, nothing else. */
  higherIsWorse?: boolean;
}

export declare function Sparkline(props: SparklineProps): JSX.Element;
