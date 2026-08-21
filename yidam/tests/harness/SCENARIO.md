# Scenario schema — harness contract

This document specifies the scenario format from the harness's perspective: what
`scenario::load()` must parse, what struct it returns, and how the fields drive
harness behavior. For guidance on writing scenarios, see [tests/scenarios/README.md](../scenarios/README.md).

---

## File format

Scenarios are Markdown files with YAML frontmatter. They live at
`tests/scenarios/<id>.md` where `<id>` is a stable kebab-case identifier.
Renaming the file breaks result history — the `id` field and the filename must match.

```
tests/scenarios/
  causal-inference.md
  <id>.md
```

---

## Frontmatter schema

```yaml
---
# Required
id: <kebab-case string, must match filename without extension>
domain: <one-line domain description>
central_question: <the question this repo exists to investigate>
seed_concepts:
  - name: <string>
    hint: <one-sentence anchor for the domain owner agent>
good_bootstrap_looks_like: <1–2 sentences describing a successful bootstrap result>

# Optional
min_harness: <semver string — minimum PROTOCOL_VERSION this scenario is valid for>
tags: [<string>, ...]   # for filtering: "language:python", "domain:science", etc.
---
```

All fields in the frontmatter are parsed and validated at load time. Unknown keys
are rejected with a parse error (no silent ignoring of typos).

---

## Rust contract

`scenario::load(path: &Path) -> Result<Scenario>` parses the YAML frontmatter and
returns:

```rust
pub struct Scenario {
    pub id: String,
    pub domain: String,
    pub central_question: String,
    pub seed_concepts: Vec<SeedConcept>,
    pub good_bootstrap_looks_like: String,
    pub min_harness: Option<semver::Version>,
    pub tags: Vec<String>,
}

pub struct SeedConcept {
    pub name: String,
    pub hint: String,
}
```

Validation performed at load time:
- `id` must match the file stem exactly
- `seed_concepts` must have ≥1 entry
- `min_harness`, if present, must parse as valid semver
- If `min_harness` is present and greater than `PROTOCOL_VERSION`, the harness
  aborts with a clear message rather than running and producing invalid results

---

## How fields drive harness behavior

### Domain owner agent seeding

The domain owner agent receives a system prompt constructed from the scenario:

```
You are the domain owner for a repository about: {domain}

The central question this repository investigates is:
{central_question}

You know the following concepts and can elaborate on them when asked:
{for each seed_concept}
- {name}: {hint}
{/for}

Stay within what is described above. Do not volunteer information that
the bootstrap agent does not ask for. Do not fill in the ontology
yourself — let the bootstrap agent drive the inquiry.
```

The domain owner is intentionally constrained: hints are anchors, not definitions.
A hint that answers a question the bootstrap would ask is too detailed.

### Judge agent context

`good_bootstrap_looks_like` is passed verbatim to the judge agent as the Q7 reference:

> Q7: Does the resulting ontology match `good_bootstrap_looks_like` from the scenario?

The judge compares the actual corpus structure against this description. It is the
only quality criterion that is scenario-specific rather than universal.

### Structural check binding

Structural checks (S1–S7 in `tests/rubric.md`) apply uniformly to all scenarios.
There is no per-scenario override mechanism — if a check needs to be scenario-aware,
it should be a quality check assessed by the judge, not a structural check.

---

## Snapshot path

Results for a scenario are written to:

```
tests/results/<id>/<model>/<YYYY-MM-DD>/
  structural.json   — protocol_version, the run record, and pass/fail per S-check
  transcript.jsonl  — the agent's event stream, streamed during the run
  commit.log        — `git log`, for a person reading the result
  commits.tsv       — subject lines oldest first, which S4 reads
  genesis.msg       — the ROOT commit's message raw (`%B`), which S5 counts
  .yidam/           — the corpus and scaffold the checks read
```

`structural.json` records `protocol_version`, and `harness diff` refuses to compare two
snapshots that do not share one: when an S-check changes meaning, a pass→fail transition
across that boundary describes the check, not the model.

It also records a **run record** read back off the transcript — the model requested and the
model the session resolved to, session id, turn count, duration, cost, and the tool calls the
permission layer refused. That last field is not bookkeeping. `claude --print` runs under the
default permission mode, where every `Write` is denied and the process still exits 0 reporting
success; a run in that state writes no corpus, and without the denials recorded the structural
verdict reads as a model that produced nothing. The harness passes
`--permission-mode bypassPermissions` — safe because the agent acts on a disposable copy, and
guarded by a check that refuses to run inside the template — so a denial now means something
went wrong rather than something was never configured.

`quality.json` holds the judge's verdict — a band per Q-criterion with the evidence it rests
on — and is written when a run is scored. Scoring is opt-in (`--judge`, or `--judge-model`)
because it costs a second model call, so an absent `quality.json` means the run was not
scored, never that there was nothing to report. `harness judge` re-scores a captured result
without re-running the bootstrap.

The criteria the judge is held to are read out of [rubric.md](../rubric.md) rather than
restated in the harness, so adding a criterion to the document holds the judge to it. A reply
that skips a criterion, invents one, scores one twice, or gives a band with no evidence is
rejected rather than recorded.

**Q1 remains unmeasurable**, and capturing the transcript did not change that. The record
carries `turns_before_first_write` — the evidence Q1 wants — but the harness inlines the
scenario into the bootstrap's prompt and tells the agent that no domain owner is present. An
agent with nobody to ask asks nothing, so the count reads 0 for a reason that has nothing to
do with the model. Q1 becomes measurable when a responder exists, not before.
