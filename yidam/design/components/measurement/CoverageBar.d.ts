/**
 * Line coverage as a bar with three states.
 *
 * The percentage is computed over `covered + uncovered` only. `unmeasured` — lines in files
 * the build did not compile — is drawn beside them in the neutral family and excluded from
 * the arithmetic, because counting it as uncovered is the number that would call the whole
 * feature-gated path untested.
 */
export interface CoverageBarProps {
  covered: number;
  uncovered: number;
  /** Added lines in files this build did not compile. Not a coverage gap. */
  unmeasured?: number;
  /** The cargo features the measurement was taken under. Required: a number whose build is
   *  unstated cannot be read. */
  features: string[];
  label?: string;
}

export declare function CoverageBar(props: CoverageBarProps): JSX.Element;
