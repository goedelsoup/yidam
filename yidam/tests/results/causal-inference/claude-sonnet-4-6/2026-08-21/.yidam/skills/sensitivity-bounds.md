---
name: sensitivity-bounds
description: Compute Rosenbaum sensitivity bounds and E-values for an estimator result, quantifying how strong unmeasured confounding must be to overturn a finding
---

# Skill: sensitivity-bounds

Computes sensitivity bounds for a causal estimate, addressing the question: how strong
would unmeasured confounding need to be to explain away the observed treatment-outcome
association as entirely non-causal?

## What it computes

Two complementary quantities:

**Rosenbaum Γ (gamma)** — the maximum odds ratio of unmeasured confounding that the
estimate can tolerate while remaining statistically significant at a given level. A Γ of 2
means that an unmeasured confounder would need to increase the odds of treatment by a factor
of 2 (while also predicting the outcome) to explain away the finding. Appropriate for
matched designs and IPW estimators targeting the ATE/ATT.

**E-value** (VanderWeele & Ding 2017) — the minimum strength of association (on the
risk ratio scale) that an unmeasured confounder must have with both treatment and outcome
to explain away the observed effect estimate. An E-value of 3 means the confounder must
have a risk ratio of at least 3 with both treatment and outcome. Appropriate for
regression-based and IPW estimators; does not require matched data.

## Reads from corpus

- `estimand` — the causal quantity being estimated (ATE, ATT)
- `estimator` — the estimation procedure (IPW, regression, matching)
- `assumption/conditional-ignorability` — the assumption being tested for robustness

## Returns

- Rosenbaum Γ at the 5% significance level
- E-value for the point estimate and for the confidence interval bound
- Narrative: what kind of confounder (in terms of observed covariates already controlled
  for) would need to exist to achieve this confounding strength

## When to invoke

During an assessment phase, after a primary estimate has been produced. The output is a
sensitivity node committed as `compute:` with a link back to the estimator instance and
the estimand instance it assessed.

## Implementation status

**Stub** — implement in `crates/sensitivity-bounds/` as a pure calculator (no network,
no filesystem access). Inputs: point estimate, standard error (or confidence interval),
effect scale (risk ratio, odds ratio, mean difference). Outputs: Γ, E-value, narrative.
