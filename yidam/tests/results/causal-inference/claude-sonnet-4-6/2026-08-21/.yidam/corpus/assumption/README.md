# Assumption

Assumptions are the price of identification in observational studies. Without randomization,
establishing causal effects requires claiming something about the data-generating process
that the data itself cannot verify. An assumption is exactly such a claim: a condition about
the world that, if it holds, makes an estimand identifiable.

The three most consequential assumptions in observational causal inference are:

- **Conditional ignorability** (also called unconfoundedness or selection on observables):
  potential outcomes are independent of treatment given observed covariates. This is the
  key assumption behind regression adjustment, IPW, and matching estimators.

- **SUTVA** (Stable Unit Treatment Value Assumption): each unit's potential outcome depends
  only on its own treatment, not on others' treatments (no interference), and there is only
  one version of each treatment level (no hidden treatment heterogeneity).

- **Exclusion restriction**: an instrument affects the outcome only through its effect on
  the treatment — the instrument has no direct path to the outcome.

Assumptions are irreducibly domain-specific. Whether conditional ignorability holds in a
given study is a question about the real causal structure of the phenomenon, not about the
data. The corpus documents these assumptions as nodes, records which estimators and
identification strategies depend on them, and tracks the study designs and domain knowledge
that bear on whether each holds.

See [assumption class definition](../assumption.ont.yml).
