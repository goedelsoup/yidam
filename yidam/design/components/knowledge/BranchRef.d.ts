/**
 * Git branch reference pill. Automatically color-codes by branch type:
 * - ma/* → ma-rose (individual position, provisional)
 * - rigpa/* → rigpa-blue (settled understanding, resolved)
 * - other → ink-neutral
 */
export interface BranchRefProps {
  /** Full branch name e.g. "ma/alice" or "rigpa/first-synthesis" */
  branch: string;
  /** Show only the stem after the prefix (ma/alice → alice) */
  short?: boolean;
  /** Optional commit hash (displayed as @short-hash) */
  commit?: string;
}

export declare function BranchRef(props: BranchRefProps): JSX.Element;
