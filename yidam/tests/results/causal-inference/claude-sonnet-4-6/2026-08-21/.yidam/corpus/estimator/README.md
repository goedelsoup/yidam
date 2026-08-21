# Estimator

An estimator is a statistical procedure — a function of the observed data — that recovers
a causal estimand from a finite sample when the required identification conditions hold.
The estimator is the operational realization of an identification strategy: identification
establishes *that* the estimand is recoverable in principle; the estimator establishes *how*
to recover it in practice, at what rate, and with what bias-variance properties.

The relationship between estimator and assumption is strict: an estimator is consistent for
its target estimand only when the identification assumptions hold. Misspecification of the
model underlying the estimator, or violation of the identification assumption, breaks
consistency without producing an obvious diagnostic. The estimator will continue to produce
a number; it will simply not be the right one.

The two workhorses of observational causal inference correspond to the two identification
strategies in this corpus:

- **Inverse probability weighting (IPW)**: operationalizes backdoor identification by
  reweighting observations by the inverse of the propensity score P(T|X).
- **Two-stage least squares (2SLS)**: operationalizes IV identification by instrumenting
  treatment with Z and regressing outcome on the predicted treatment value from stage 1.

See [estimator class definition](../estimator.ont.yml).
