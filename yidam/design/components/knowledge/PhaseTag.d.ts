/**
 * Phase type indicator for agent inquiry phases.
 * Four phases: Investigation, Extraction, Synthesis, Assessment.
 * Each maps to a distinct tint from the color vocabulary.
 */
export interface PhaseTagProps {
  phase: 'Investigation' | 'Extraction' | 'Synthesis' | 'Assessment';
}

export declare function PhaseTag(props: PhaseTagProps): JSX.Element;
