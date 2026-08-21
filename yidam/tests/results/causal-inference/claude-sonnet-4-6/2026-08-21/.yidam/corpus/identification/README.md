# Identification

Identification is the central problem of observational causal inference. An estimand is
*identified* when it can be expressed as a functional of the observable data distribution
— when, in principle, infinite data would let us recover the causal quantity exactly. If an
estimand is not identified, no statistical estimator can recover it, regardless of sample
size.

Identification requires assumptions. In an observational study, the gap between what the
data shows and what we want to know is always bridged by claims about the data-generating
process that the data cannot verify. The identification problem is the problem of figuring
out which assumptions, if they held, would make the estimand recoverable — and then arguing
that those assumptions are credible in the domain.

The two most important identification strategies in observational causal inference are:

- **Backdoor identification**: condition on a sufficient adjustment set that blocks all
  backdoor paths from treatment to outcome. Requires conditional ignorability.

- **Instrumental variable identification**: exploit quasi-random variation in an instrument
  that is related to treatment but satisfies the exclusion restriction. Identifies the LATE.

Identification is not the same as estimation. A study can have a correctly identified
estimand and still produce biased estimates due to model misspecification, finite-sample
variance, or improper implementation of the estimator.

See [identification class definition](../identification.ont.yml).
