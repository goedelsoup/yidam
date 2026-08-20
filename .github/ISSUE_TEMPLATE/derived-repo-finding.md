---
name: Finding from a derived repository
about: A defect in the prelude, CLI, CI, or conventions, found while doing domain work
title: ''
labels: from:derived-repo
---

<!--
Findings from derived repos are the highest-signal input this project gets: a derived
repo is the only place these conventions meet a real corpus. Please do not send corpus
content — a finding is about the template, and the domain material that exposed it
often should not leave your repository.
-->

## What is wrong

<!-- The file and line in the prelude, CLI, or workflow — not only the symptom. -->

## What it cost

<!--
The part upstream cannot reconstruct, and the part that decides priority: commits
spent, checks that passed while wrong, how long it ran before anyone noticed.
-->

## Pin

<!-- The `commit` from your .yidam.toml. A defect already fixed upstream is a re-vendor. -->

```
commit = ""
```

## Workaround in place

<!-- Where, if anywhere — so it can be removed when the fix lands. "None" is fine. -->
