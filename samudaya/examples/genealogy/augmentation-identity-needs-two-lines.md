---
kind: augmentation
constitutional: true
---

# No resolution concludes on one researcher's line

A collective resolution reads the tips of at least two electors. A conclusion drawn from one
researcher's line of work is that researcher's claim, however many records it cites, and it
stays at the standing that researcher held it at.

Genealogical proof asks for a reasonably exhaustive search. One line of search is not
exhaustive, and a resolution is where the corpus says that more than one person looked. A
resolution with one tip says that and means the opposite.

This article is permanent. Bootstrap writes it to `.yidam/constitution/` in the genesis
commit, with the rule beside it that `yidam lint` evaluates:
`augmentation-identity-needs-two-lines.rego` refuses a resolution record naming fewer than
two tips, and `augmentation-identity-needs-two-lines_test.rego` holds the cases.
