# samudaya

*Arising.* The conditions that give rise to a specific bootstrap.

`samudaya/` is a transient influence layer placed in a repository before the bootstrap agent
arrives. It shapes what the bootstrap produces — then is consumed and destroyed as part of
the genesis event. If samudaya/ survives genesis, something went wrong.

## Purpose

An yidam-derived repository can be bootstrapped from nothing, using only the generic
ontology-discovery dialogue. But when a domain has prior commitments — known concepts,
required relationships, or constraints on scope — samudaya lets you express them before the
agent begins. The bootstrap reads samudaya/, folds its commitments into the dialogue, and
scaffolds accordingly.

Samudaya seeds the ontology-discovery loop. It does not replace it.

## Contents

Each file in `samudaya/` is a markdown document with YAML frontmatter declaring its role:

```yaml
---
kind: axiom | hint | constraint | augmentation
---
```

**`axiom`** — A concept that must appear in the corpus. The bootstrap will ensure a node
exists for it. Body: name and a one-sentence framing of why it is irreducible to this domain.

**`hint`** — A suggested relationship or direction for the ontology. The bootstrap will
surface it during discovery but may discard it if the user's answers do not support it.

**`constraint`** — A scope boundary or structural restriction. Examples: "do not scaffold
`web/`", "the domain is restricted to offline inference settings".

**`augmentation`** — Additional prelude content: guidelines, conduct norms, or constitutional
extensions that apply to this derived repo. Treated as if part of the prelude during the
bootstrap run. Constitutional augmentations (extensions to [CONSTITUTION.md](../prelude/CONSTITUTION.md))
are committed into the derived repo permanently — they become domain-specific articles that
govern that repo's sangha resolutions for its lifetime. Non-constitutional augmentations
(guidelines, conduct norms) do not persist once samudaya is consumed.

## Lifecycle

1. Author places `samudaya/` files in the repository before invoking the bootstrap agent.
2. Bootstrap reads samudaya/ before beginning the ontology-discovery dialogue.
3. Bootstrap folds axioms and hints into the discovery loop; enforces constraints during
   scaffolding; treats augmentations as additional prelude.
4. After the genesis commit, the bootstrap commits the removal of `samudaya/` as an explicit
   consumption event. The commit message records what was consumed and what it influenced.
5. `samudaya/` no longer exists in the working tree. Its content is preserved in git history
   as provenance for the genesis ontology.

## What samudaya is not

- A replacement for dialogue: the bootstrap must still ask and confirm with the user
- A schema or config file: it is read by an agent, not a parser
- A persistent part of the repo: presence after genesis is an error state
