# Confounding

Confounding is the central obstacle to causal inference from observational data. A
confounding variable — a confounder — is causally related to both the treatment and the
outcome, creating an association between them that is not mediated by the treatment itself.
When a confounder is present and not adequately controlled for, a naive comparison of
treated and untreated units will attribute the confounder's effect to the treatment.

The fundamental problem: in an observational study, units are not assigned to treatment at
random. They select into treatment based on characteristics that also affect the outcome.
Those characteristics — whether measured or unmeasured — are confounders. The entire
enterprise of identification in observational causal inference is the enterprise of ruling
out confounding as an explanation for the treatment-outcome association.

Confounding is a property of the *data-generating process* (the causal structure of the
world), not of the data itself. An observed correlation does not reveal confounding; the
absence of confounding cannot be verified from observed data alone.

See [confounding class definition](../confounding.ont.yml).
