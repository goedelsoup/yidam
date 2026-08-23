---
name: read-a-regulated-record
description: How to read a discharge record from a regulated reach without importing the assumptions that only hold on an unregulated one.
---

# Reading a regulated record

Most streamflow statistics were defined on unregulated catchments and carry that assumption
silently. This is the check to run before computing any of them here.

## 1. Establish which side of regulation the record is on

Ask of the reach node: is `regulated` set, and by what? A record spanning an operating change
is **two records**. Pooling them into one distribution produces a statistic describing a
river that never existed.

## 2. Look for a diel signal before computing any daily statistic

If the hydrograph has a sub-daily cycle whose period matches a load curve, see
`concept/hydropeaking`. A daily mean over it reports a flow that occurs at no time of day,
and a daily minimum reports the operator's trough rather than the catchment's.

## 3. Do not cite a base-flow index without its filter

See `decisions/base-flow-index-carries-its-method`. A figure without its method and
parameters is not interpretable. This applies to figures found in the literature, not only
to ones computed here.

## 4. Tag by provenance, not by confidence

A value from the instantaneous service is **provisional** and is revised after review. It
supports `[inference]`. It does not support `[verified]`, however plausible it looks — see
`catalog/usgs-nwis`.

## 5. Write down what you looked at and did not use

Especially when a record was excluded. The exclusion rule belongs beside the claim, or the
next reader cannot tell selection from absence.
