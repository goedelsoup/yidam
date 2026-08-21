# Intervention

An intervention in causal inference is the conceptual act of externally setting a variable's
value — Pearl's *do(X = x)* — as opposed to passively observing that value arise through
the natural data-generating process. This distinction is the foundation of causal inference:
it is what separates asking "what is the probability that Y = 1 given X = 1?" from asking
"what would be the probability that Y = 1 if we *set* X = 1?"

The intervention class captures this concept as a BFO occurrent (something that unfolds in
time): an intervention is an event with a target variable and a level. Its primary role in
the corpus is to *define* estimands — a causal effect is always a contrast between two
interventions, and the estimand is precisely that contrast.

See [intervention class definition](../intervention.ont.yml).
