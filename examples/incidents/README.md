# incidents

*A worked yidam corpus: what recurs across incidents, and which fixes nobody checked.*

Thirteen instances across four classes, one catalog source, two decision records, one skill —
and, unlike the other examples here, **a history**. It ships the order it was written in, so
`yidam replay` has something to reconstruct.

## The domain, in one paragraph

A review document is about one incident and is written by somebody who has just spent a week
inside it. It is the right instrument for that incident and the wrong one for every question
worth asking afterwards, because those questions are about the *set*: which contributing
factors keep coming back, and which remediations were merged and never confirmed to work. A
folder of review documents cannot answer either, and the second one especially — the absence
of a verification is not a document, so there is nothing to file and nothing to find.

## What is illustrative and what is real

The **conventions** are real: a severity ladder where SEV1 is customer-visible loss of a core
function and the numbers rise as impact falls; a timeline of timestamped entries from detection
through mitigation to resolution; the standard review section set — summary, impact, timeline,
contributing factors, action items.

The **services and incidents are not**. There is no `checkout-api`. They carry real conventions
so the shape of a real record is legible, and they describe no outage anybody had.

## The shape of it

```text
history.toml                the order this corpus was written in
.yidam/
  corpus/
    service.ont.yml   incident.ont.yml   contributing-factor.ont.yml   remediation.ont.yml
    service/          incident/          contributing-factor/          remediation/
      checkout-api      2026-03-…          unbounded-retry               request-coalescing
      session-store     2026-05-…          cache-stampede                retry-budget
      image-resize      2026-06-…          no-backpressure               load-shed
                        2026-07-…
  catalog/incident-record.md
  decisions/root-cause-is-not-a-class.yml
  decisions/a-remediations-tag-is-about-working.yml
  skills/read-the-set-not-the-document.md
```

## What each piece is here to demonstrate

**The class that was rejected.**
[`decisions/root-cause-is-not-a-class.yml`](.yidam/decisions/root-cause-is-not-a-class.yml).
A root cause is a choice about how to tell the story, made under time pressure, by one
reviewer. Giving it a class or a privileged edge would make the ontology assert, of every
incident carrying one, that the cause has been found — the claim a good review is careful not
to make. Naming the class `contributing-factor` is itself the commitment.

**A tag that is about working, not about merging.**
[`decisions/a-remediations-tag-is-about-working.yml`](.yidam/decisions/a-remediations-tag-is-about-working.yml).
Every remediation here merged, so under the obvious reading all three would be `[verified]` and
the field would carry no information. It records whether somebody went back and looked:
`request-coalescing` held through two later expiry events, `retry-budget` merged and is
assumed, and **`load-shed` nobody checked**.

**A recurrence no single review could have found.** `contributing-factor/unbounded-retry`
appears in three of the four incidents, concluded by different reviewers who had not read each
other's documents.

**A minor incident, on purpose.** `incident/2026-07-partial-checkout-degradation` is SEV3 with
no factor concluded. It is here because a process that only records the incidents worth writing
up produces counts that measure attention rather than the system.

**A history worth replaying.** [`history.toml`](history.toml) names ten commits with their
dates. `yidam/cli/tests/example_corpus.rs` replays them when materialising this example;
examples without a manifest get the single genesis commit they always did.

## Running the gates

```sh
cp -R examples/incidents /tmp/incidents
cd /tmp/incidents && git init -q && git add -A && git commit -qm "genesis: incidents"
yidam graph-check     # 13 instances across 4 classes — all clean
yidam lint            # 0 finding(s), no errors
yidam open-questions  # two live questions
```

That gives one commit, which is enough for every gate and **not** enough for `replay` — see the
walkthrough, [docs/walkthroughs/incident-retrospectives.md](../../docs/walkthroughs/incident-retrospectives.md),
which replays `history.toml` and shows the series.
