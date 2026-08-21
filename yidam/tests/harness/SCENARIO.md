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
  structural.json   — protocol_version, and pass/fail per S-check
  commit.log        — `git log`, for a person reading the result
  commits.tsv       — subject lines oldest first, which S4 reads
  genesis.msg       — the ROOT commit's message raw (`%B`), which S5 counts
  .yidam/           — the corpus and scaffold the checks read
```

`structural.json` records `protocol_version`, and `harness diff` refuses to compare two
snapshots that do not share one: when an S-check changes meaning, a pass→fail transition
across that boundary describes the check, not the model.

**Not yet written.** `quality.json` — a band per Q-check — has no producer. The judge is
specified in [judge.md](../judge.md) and scored in [rubric.md](../rubric.md),
and nothing invokes it; the transcript it would read is discarded rather than captured. Q1–Q7
are unmeasured, and this document should not be read as saying otherwise.
