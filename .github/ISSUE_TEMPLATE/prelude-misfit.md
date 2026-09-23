---
name: A prelude norm that does not fit a domain
about: A rule that is right in general and wrong for one derived repository's domain
title: ''
labels: "from:derived-repo, scope:prelude"
---

<!--
This is not the defect template, and the difference decides how the report is answered.

  A DEFECT is a rule that is wrong everywhere — it contradicts itself, contradicts the
  tooling, or costs every derivation something. One repository is enough to demonstrate it.
  Use "Finding from a derived repository" instead.

  A MISFIT is a rule that is right in general and wrong HERE, because it collides with a
  fact about this domain. One repository is a case, not yet an argument: what generalises
  it is the domain fact, not the count.

As with a defect, please do not send corpus content. The domain material that exposed the
collision often should not leave your repository — name the fact, not the nodes.
-->

## The norm

<!-- The rule, and the prelude file and line that states it. Quote the sentence. -->

## The domain fact it collides with

<!--
The part upstream cannot reconstruct. Not "this is inconvenient" but "in this domain, X is
true, and the rule assumes not-X." A rule written from evidence that did not include a
domain like yours is the common case, and this is where that becomes visible.
-->

## What this repository did in the meantime

<!--
Carried the cost, narrowed it with a kuten, overrode it under `.yidam/policy/`, or worked
around it. "Carried the cost" is a complete answer. If you edited anything under
`.yidam/.vendor/`, say so — that edit is discarded at the next re-vendor.
-->

## What you are asking for

<!-- Pick one. A report that does not say leaves the judgement to somebody with less evidence. -->

- [ ] **Narrow the norm** — the rule keeps its force upstream and stops reaching this case
- [ ] **Sanction a local departure** — the rule stands, and this repository is licensed to depart
- [ ] **Hold the norm and say why** — a refusal on the record, so the case is not re-argued

## Pin

<!-- The `commit` from your .yidam.toml. The norm may already have been narrowed upstream. -->

```
commit = ""
```
