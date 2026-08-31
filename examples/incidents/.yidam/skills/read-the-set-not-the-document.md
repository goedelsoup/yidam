---
name: read-the-set-not-the-document
description: The questions a retrospective process should ask across incidents, and which of them a single review document structurally cannot answer.
---

# Reading the set, not the document

A review document is about one incident and is written by somebody who has just spent a week
inside it. It is the wrong instrument for every question below, not a weak one.

## 1. Ask which factors recur before asking what caused anything

`incident -has-factor-> contributing-factor` across the whole corpus. A factor named in three
reviews by three reviewers is a finding that none of the three could have made, and the count
is the whole of it.

Recurrence is also the only defence against the reviewer effect: any one review reflects what
that reviewer looked for.

## 2. Read a recurrence count as a count of reviews, not of events

The corpus holds declared and reviewed incidents. A factor's count measures the system only to
the extent that declaring is consistent — see `catalog/incident-record`. If minor incidents
stop being reviewed, every count moves and nothing in the corpus says why.

## 3. Ask which remediations nobody checked

`remediation` nodes at `[open]`. This is the question a folder cannot answer, because the
absence of a verification is not a document — there is nothing to file. See
`decisions/a-remediations-tag-is-about-working`.

## 4. Do not read the absence of a recurrence as confirmation

A remediation that merged and has had no incident since is consistent with it working and with
nothing having tested it. That is `[inference]`, and calling it `[verified]` because time has
passed is the most common way this corpus would degrade.

## 5. Do not let a service tier stand in for blast radius

A tier is a statement about one service in isolation. `incident/2026-06-resize-queue-memory` is
a tier-3 service taking down a tier-1 path through a shared resource, and no tier predicts it.
The factor graph does.

## 6. Watch the shape over time, not only the current state

`yidam replay` reconstructs the corpus at every commit that touched it. A retrospective process
decaying does not look like an event; it looks like a slope. A snapshot cannot show one.
