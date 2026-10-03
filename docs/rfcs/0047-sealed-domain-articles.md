# RFC-0047 — A domain article is sealed by the genesis commit

- **Status:** Implemented
- **Track:** I34
- **Relates to:**
  - RFC-0024 (the policy layer, whose authoritative override is what a domain article must not be subject to, and whose "Composition" section named tighten-only and deferred where it is declared)
  - RFC-0026 (§3 item 2: a constitutional family's composition rule is declared in the Rust that owns the family, not in the policy)
  - #561 (`2fcbd19`, Article V's text: no exception to it is licensed "by a domain extension")
- **Versioning layers touched:** template (bootstrap skill, CONSTITUTION.md's Domain extensions section, directories.md). Tooling: four `yidam lint` checks and a `samudaya-audit` warning. No SDK, parity or MCP contract change. No migration: a repository with no `.yidam/constitution/` sees nothing new.
- **Downstream reference case:** none yet. No derived repository has taken a constitutional augmentation. The two seed sets that carry one are `samudaya/examples/genealogy/` and `samudaya/examples/language-documentation/`.

## Summary

The constitution lets a derived repository add domain articles at genesis, permanently. Until
now such an article was prose appended to a vendored file, and nothing checked it. This RFC puts
each article in `.yidam/constitution/`, with an optional Rego rule and its cases beside it.
`yidam lint` reads the rule **from the genesis commit**, not from the working tree. A repository
that violates the rule fails lint. An edit to the rule after genesis is reported and not obeyed.

## Problem

Three defects, found in order.

**Nothing read the article.** The Domain extensions section of
[CONSTITUTION.md](../../yidam/prelude/CONSTITUTION.md) licensed articles that "must be consistent
with Articles I–VI". None of the Article V checks (#272–#275) read that section. The epic's test
was "a resolution that violates the constitution fails CI". It held for yidam's articles and not
for a derived repository's own.

**The article did not survive a re-vendor.** The bootstrap skill said to "append it to the repo's
copy of the constitution". That copy is `.yidam/.vendor/prelude/CONSTITUTION.md`.
`yidam-vendor-update` deletes that directory first
([mise.yidam.toml](../../mise.yidam.toml#L717): *"rm -rf .yidam/.vendor/prelude"*). So the first upgrade erased every article the
genesis commit had appended. "Permanent" meant "until the next upgrade".

**The obvious home is editable.** A rule in `.yidam/policy/` is authoritative under RFC-0024: a
local file supersedes the rule for its package outright. An article there could be loosened by
the corpus it binds, by one commit.

## Proposal

### 1 — Where an article lives

Bootstrap copies a `constitutional: true` augmentation to `.yidam/constitution/<stem>.md` in the
genesis commit. When the seed has `<stem>.rego` and `<stem>_test.rego` beside it, those are copied
too. Nothing is appended to the vendored constitution.

The rule is ordinary Rego with one required shape: a partial set `deny` of messages. Its input is
the sangha report (`yidam sangha --format json`) and the corpus's nodes as `{file, class}`.

### 2 — The genesis commit is the pin

`yidam lint` finds the root commit and reads `.yidam/constitution/` from it with `git ls-tree` and
`git cat-file`. The working tree is compared by blob id. A file whose id differs, or which is
gone, is reported as `domain-article-edited`. A file added after genesis is reported the same way
and never evaluated.

A hash file was the alternative. A hash file would sit in the tree, and the corpus could edit it
alongside the rule. The genesis commit cannot be edited without rewriting history. A rewrite
changes every commit id, so everyone holding the repository would see it.

A deleted article still binds. Its rule is read from the genesis commit whether or not the file
is on disk.

### 3 — Composition is in the Rust (RFC-0026 §3 item 2)

Each rule is loaded into its own engine (`Policies::sealed`). The engine holds that rule and its
cases, and nothing else. It loads no built-in defaults and nothing from `.yidam/policy/`. So no
local file can supersede an article, because no local file is in the engine that evaluates one.

That is the composition rule, and it lives in the Rust that owns the family. RFC-0024 deferred the
question of where; RFC-0026 answered it; this is the first family built under that answer.

### 4 — Tighten-only is the rule's shape (#561)

Article V says no exception to it is licensed "by a domain extension". The mechanism keeps that
by construction. A rule answers only with refusals. A refusal fails lint. There is no `allow` a
rule can set, so an article can add a refusal and cannot remove one yielded elsewhere.

A rule that cannot answer is not a permission. A `deny` that is undefined, or that errors, is
reported as a violation at Error.

### 5 — Four checks

| Check | Severity | Fires on |
|---|---|---|
| `domain-article-violated` | Error | a sealed rule's refusal, or a rule that could not answer |
| `domain-article-edited` | Error | a file in `.yidam/constitution/` that differs from, or is missing from, the genesis commit |
| `domain-article-unproven` | Warn | a sealed rule with no sealed `_test.rego`, or a failing case |
| `domain-article-unverifiable` | Warn | articles on disk with no genesis commit visible, such as a shallow clone |

A shallow clone evaluates nothing. Its root commit is a graft, not the genesis commit, so reading
rules from it would obey whatever the clone happened to start at.

`yidam samudaya-audit` warns before genesis when a constitutional augmentation has no rule beside
it, or a rule with no cases.

## What this does not do

- **It does not give a rule the whole corpus.** The input is the sangha report and each node's
  file and class. The language-documentation article asks that a source state what it forbids,
  and a rule for it needs catalog frontmatter in the input. It ships as prose, and the audit says
  so.
- **It does not check that an article is consistent with Articles I–VI.** The shape keeps an
  article from removing a refusal. Whether its prose contradicts an article is still the review
  `samudaya-audit` asks a person for.
- **It does not migrate an appended article.** None exists. On 2026-09-29 all thirteen derived
  repositories on disk carried the template's Domain extensions section unchanged.
