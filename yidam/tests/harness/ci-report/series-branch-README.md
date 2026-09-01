# quality-series

This branch is **not code**. It holds one file, `series.jsonl`, and one record per push to
`main`: what the gates measured, in the order they measured it.

It is written by the `ci (series)` job in `.github/workflows/ci.yml` and read by the docs
build, which renders it at <https://goedelsoup.github.io/yidam/main/quality/trends/>. Nothing else
reads it, and **nothing reads it to decide whether CI passes**. If a number here disagrees
with a gate, the gate is right and the series has a bug.

## Why a branch of its own

RFC-0025 left the choice open and asked for it to be settled in #468. Git, because git is the
one store this repository already trusts and a Pages artifact has no history older than the
last deploy. A branch of its own rather than a file on `main`, because a bot commit per push
would land in `git log main` beside real work, would race a human push, and — `ci.yml` being
`on: push: branches: [main]` — would re-trigger the run that wrote it.

## The format

JSONL: one JSON object per line, appended, never rewritten. A reader that cannot parse a line
skips it and says so rather than refusing the file, because one truncated write from a
cancelled job must not blank a year of history.

A record is keyed on its commit. Re-running a workflow replaces that commit's record rather
than adding a second point for it.

The shape is `Record` in `yidam/tests/harness/ci-report/src/series.rs`, on `main`.

## Editing this branch

Don't. It is append-only by machine. If a record is wrong, the thing that produced it is
wrong — fix that on `main` and the next push records correctly. Rewriting history here would
make the sparklines disagree with the reports they were derived from, and nothing would catch
it.
