# Upstream

*What an yidam-level change is, how one reaches this repository, and what a report sent from
here is worth.*

[Article I](../CONSTITUTION.md) settles that the prelude is not subject to resolution, and coins a
term for the alternative: **an yidam-level change**. [GRAPH.md](../GRAPH.md) reaches for the same
term when a commit genuinely does not fit the closed verb vocabulary. Neither document says what
one *is*, and until this one existed nothing did — Article I said what the forum is not, and the
whole of what it is was a maintainer reading repositories by hand.

This document is the other half. It is not a second constitution and it governs nothing a
resolution may do; it names a process that already runs, adds the one report shape it was
missing, and states what evidence gathered across derivations obliges.

## What an yidam-level change is

**A commit to the prelude in the yidam template repository, decided there, and reaching this
repository by a re-vendor and by nothing else.**

Five properties follow from that sentence, and each is load-bearing:

- **It is not a resolution.** No `ma/*` position and no `rigpa/*` commit alters a prelude norm,
  here or anywhere. This is Article I and it is not a technicality: a sangha that could resolve
  away the ground it stands on is governed by nothing.
- **It is decided upstream, in the open, on the issue that raised it.** Today that decision is
  made by this template's maintainer. Saying so is better than naming a body that does not exist;
  what makes the decision reviewable is that the issue, the commit and the release are all public,
  not that a quorum signed it.
- **It reaches you only when you fetch it.** `mise run yidam-vendor-update` is the whole delivery
  mechanism. Until you run it, the prelude you are bound by is the prelude you vendored. That is a
  supported state and not a lapse — you are entitled to stay pinned, and nothing gates on the pin.
- **It arrives as one reviewable event.** A `vendor:` commit, read as a diff before it is
  accepted, and given its own decision record where it changes a convention the corpus depends on.
  [directories.md](directories.md) has the procedure and the staleness report.
- **It is not announced.** Nothing upstream pushes to you and nothing knows you exist. Staleness
  is reported from outside, on your own CI, because it is invisible from the inside.

What that costs is measurable, and is stated here rather than implied. Across fifteen derived
repositories on 2026-09-22, the constitution had been amended twice since the template's genesis
commit; the current text was in force in three of them, and six still ran the text that predates
the sangha being made opt-in. All fifteen vendored constitutions were byte-identical to some
upstream revision — no derivation had edited one, and none had extended one.

## Two shapes of report, and they ask for different things

Nothing carries a finding **up**. A re-vendor carries corrections **down**, and a derived
repository is the only place these conventions meet a real corpus, so it is where most defects in
this template are actually discovered. [directories.md](directories.md) states the return path and
what makes a report actionable. What it does not distinguish is the shape of what you found.

| | A **defect** | A **misfit** |
|---|---|---|
| What it is | A prelude rule that is wrong everywhere — it contradicts itself, contradicts the tooling, or costs every derivation something | A prelude rule that is right in general and wrong *here*, because it collides with a fact about this domain |
| What proves it | One repository is enough. A rule that contradicts its own tooling does so on every corpus | One repository is a case, not yet an argument. What generalises the finding is the domain fact, not the count |
| What it asks for | A fix upstream, carried down by re-vendor | One of three answers below — and the report says which it is asking for |
| Where it goes | `gh issue create --repo goedelsoup/yidam --label "from:derived-repo"` | The same channel, `scope:prelude` as well, and the misfit template |

The distinction is worth making because the two have different repairs and only one of them is
well-served today. Defects have a form, a template and a track record. A misfit filed as a defect
argues that a rule is broken when what you mean is that it does not reach your case, and it will
be answered as though the rule were broken.

## What a misfit report asks for

Three answers are available, and a report that does not say which one it wants leaves the
judgement to somebody with less of the evidence:

1. **Narrow the norm.** The rule keeps its force and stops reaching the case. This is the right
   ask when the rule was written from evidence that did not include a domain like yours, and the
   cheapest to grant — it takes nothing away from the derivations the rule already fits.
2. **Sanction a local departure.** The rule stands and this repository is licensed to depart from
   it, through one of the three routes below. The right ask when the collision is genuinely local.
3. **Hold the norm, and say why.** The misfit is real, the rule costs this repository something,
   and the cost is the right price. A refusal on the record is a result: it stops the same case
   being re-argued from scratch, and the reasoning is what the next derivation reads.

There is a fourth thing a report can turn into and must not: **a local workaround nobody upstream
ever sees.** It is invisible to every other derived repository, it is discarded at the next
re-vendor, and it leaves this repository further from upstream than it started. A misfit that is
worth a workaround is worth an issue.

## What to do in the meantime

Three routes let a derivation depart from a prelude default without editing anything it inherited.
All three already exist, and none of them is new here:

- **A kuten profile** narrows the loop — the phase types, the verb subset, the question pressure.
  It may not widen the model, and its binding rule is in [kuten/README.md](../kuten/README.md).
- **`.yidam/policy/`** overrides a rule's severity, and is authoritative where it speaks. A local
  rule may be more permissive and may not be silent; the override is visible to `policy check`,
  `lint` and `doctor`. [directories.md](directories.md) has the layout.
- **A domain extension to the constitution**, appended at genesis by a samudaya augmentation.
  Permanent in the repository that took it, and consistent with Articles I–VI by construction.

**Measured, so you know what you are reaching for.** Across the same fifteen repositories on
2026-09-22, two had adopted a kuten, none had a `.yidam/policy/` at all, and none carried a domain
extension. These are supported routes with almost no users, not well-worn paths.

If the case fits none of them, the answer is to carry the cost and leave the issue open. That is
an unsatisfying instruction and it is the honest one: a fourth route invented ahead of a case that
needed it would be a mechanism nobody uses, and this layer has shipped several of those.

**Do not edit anything under `.yidam/.vendor/`.** It is discarded at the next re-vendor, it is
invisible upstream, and `yidam cohort` reads the history for exactly this — a commit outside
`vendor:`, `consume:` or `genesis:` touching the vendored tree.

## What evidence from several derivations is worth

`yidam cohort` reads a set of derived repositories and reports, per prelude norm, how many held,
how many lost, how many could not have held it (the vendored prelude predates the rule) and how
many were never asked the question. Its rows are norms and its columns are repositories, which is
the framing and not the formatting: **a norm that every derivation fails is one finding about the
norm, not five findings about five corpora.**

Two rules follow, and they bind the template repository rather than this one. They are stated here
anyway, because a derivation is entitled to know what its report is worth, and a rule about
upstream that only upstream can read is the arrangement this document exists to end.

- **A change to a prelude norm states the cohort reading it rests on** — the norm, the date of the
  run, and held/lost/vintage/unmeasurable over the occasions the question could be asked of. Where
  no reading was taken, the change says so. Unmeasured is a normal state and not a confession.
- **A norm that loses a majority of its occasions must be answered** — amended, narrowed, or kept
  with the reason on the record. Answering is not the same as changing: a rule that loses
  everywhere and is kept anyway is a defensible outcome, and an unexamined one is not.

**Neither gates anything**, and the reason is not squeamishness. The norm list `cohort` reads is
hand-maintained and is not the set of norms the prelude states; a rule written upstream tomorrow is
invisible to it until somebody adds a row. A gate over an instrument with that property would
measure the roster rather than the practice.

**A reading is evidence about a rule as one instrument can read it.** The standing example is the
norm that was withdrawn before it shipped: it read as lost in twelve of thirteen repositories,
which looked like the strongest result in the run, and it was quoted accurately from a document
that argues the opposite two sections further down. A cohort reading tells you a rule lost. It does
not tell you the rule was right.

## What this is not

It is not a vote, and no count of derivations carries a decision on its own. It is not a
notification channel — nothing upstream knows this repository exists. It is not a licence to edit
the prelude locally while a report is open. And it is not a sangha: the forum for a prelude change
is an issue upstream, precisely because Article I put it outside the one this repository has.
