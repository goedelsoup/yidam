# Estimand

An estimand is a precisely stated causal quantity — the mathematical object that a causal
inference exercise is trying to recover. Without a stated estimand, "the effect of X on Y"
is underspecified: which population? which contrast? which counterfactual? The estimand
forces these choices into the open before identification and estimation begin.

The estimand is the target. Identification establishes whether that target is recoverable
from the observed data distribution under certain assumptions. The estimator is the procedure
that recovers it in finite samples. These three concepts are distinct and must not be
conflated: a study that identifies the LATE but estimates the ATE has not estimated what
it identified.

Common estimands in observational causal inference include the Average Treatment Effect
(ATE = E[Y(1) − Y(0)]), the Average Treatment Effect on the Treated (ATT = E[Y(1) − Y(0)|T=1]),
and the Local Average Treatment Effect (LATE = E[Y(1) − Y(0)|complier]).

See [estimand class definition](../estimand.ont.yml).
