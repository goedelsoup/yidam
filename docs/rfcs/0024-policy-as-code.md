# RFC-0024 — The rule a repository writes about itself (policy as code)

- **Status:** Draft
- **Track:** I19
- **Relates to:** RFC-0023 (the vault guards this re-expresses, and the store whose push is the
  first egress channel yidam owns), RFC-0003 (the light binary this must run in), RFC-0001 (the
  report contract `policy check --format json` emits on), RFC-0018 (the precedent that a new
  language surface is a CLI surface and **not** a fourth parity function), RFC-0008 (Article V
  — the second family this deliberately does not implement)
- **Versioning layers touched:** template (the prelude gains `policy/`; `directories.md` gains
  a section) / tooling (`yidam` CLI implements it) — **no parity-surface change, no MCP
  contract change, no bootstrap-protocol change**
- **Parent epic:** #436 — this RFC specifies **#438** through **#441**, one per phase

## Summary

The binary refuses things. It refuses a push whose record says nothing about redistribution,
an edge a class did not license, a commit whose verb is not in the vocabulary. Every one of
those refusals is a rule, and every one is written in Rust, shipped with the binary, and
unavailable to the corpus it governs until somebody cuts a release.

That is correct for a rule every corpus shares and wrong for a rule only one corpus holds. The
repository already knows this and has said so four times, in four different files:

> A value compiled into the binary would be one repository's judgement imposed on every other.

Each time, the answer was one more field in `.yidam/config.toml`. There are four of them now,
and the fifth is already implied by a constitution that permits per-domain articles no released
binary has ever seen.

This specifies the general answer:

> **A policy is a committed file. Git stores the rule; the binary evaluates it.**

That is RFC-0023's sentence about bytes, applied to judgement. The engine is
[`regorus`](https://github.com/microsoft/regorus) — a pure-Rust Rego interpreter — chosen by
measurement against this repository's real dependency graph, the way #412 chose the transport.

The first family is **disclosure**: what this repository may let leave. It is chosen first
because it is the family where the duplication is real, the consequence is irreversible, and
there are two tested Rust functions to prove the first policy equivalent to before anything
depends on it.

## Problem

### One question, answered four times, in two languages

*Is this path declared private, and does that stop these bytes leaving?*

| | Where | Predicate |
|---|---|---|
| 1 | [`ci.yml:104`](../../sadhana/github/workflows/ci.yml) | bash loop, `find` per entry, placeholder exemption inline |
| 2 | [`release.yml:77`](../../sadhana/github/workflows/release.yml) | a **different** bash loop, bidirectional containment, the same exemption re-inlined |
| 3 | [`policy.rs:110`](../../yidam/cli/src/vault/policy.rs) — `is_private`, under `may_push` | **one-directional** prefix match |
| 4 | [`policy.rs:192`](../../yidam/cli/src/vault/policy.rs) — `derived_may_push` | **bidirectional** intersection, plus `holds_content` |

Rows 3 and 4 live in one file and disagree deliberately: a record-bearing artifact is judged by
what its record says, a computed one by what it was built from, and those are different
questions about the same manifest. That distinction is correct and this RFC preserves it.

Rows 2 and 4 are the problem. They answer the **same** question — may this material reach a
published bundle — in two languages, and the repository keeps them agreeing by testing that one
mirrors the other.

### The mirror has already cost a defect

**#443.** [`publish_guard.rs:45`](../../yidam/cli/tests/publish_guard.rs) now pins the workflow's
directory list to `vault::derived_sources`, and its comment records why:

> until #443 it named the three directories the archive carries as files and reasoned that
> `index/` was "generated rather than authored" — true of the file, false of its contents.

A bundle carries `index/corpus.arrow`. `model::VectorRow` has a `text` column. `cmd/embed.rs`
fills that column from the catalog as well as the corpus. So a private catalog entry's prose
shipped inside the archive while no catalog *file* did, and every check passed.

The fix was right. The mechanism it added is still a mirror, and it runs in one direction: the
assertion is that the workflow line *contains* every entry `derived_sources` returns. It cannot
see a directory the workflow names that the bundle no longer carries, and it says nothing about
the loop around the list — the `find` invocation, the placeholder exemption, the bidirectional
containment — each of which is transcribed rather than shared.

**A mirror is what you build when two implementations of one rule cannot be reduced to one.**
This RFC is about removing the reason they cannot.

### The constitution licenses rules no binary can carry

[`CONSTITUTION.md`](../../yidam/prelude/CONSTITUTION.md) permits domain augmentation, and names
what a valid augmentation looks like:

> Bootstrap-time augmentations from `samudaya/` may append domain-specific articles here.
> Extensions are committed into the derived repo during the genesis event and **become part of
> that repo's constitution permanently.**
>
> Examples of valid extensions: quorum requirements for a specific domain's resolutions,
> constraints on which node types may be resolved collectively vs. individually, additional
> legibility requirements specific to the domain's communication norms.

A quorum threshold is one domain's number. Compiling it in would be `[lint] escalate_after`'s
imposition one layer up, and shipping a field for every possible extension is not a design.

#253's ambition test is *"a resolution that violates the constitution fails CI."* Its four
children — #272, #273, #274, #275 — are about Articles I–VI and the resolution record, which
are universal and are correctly Rust. **None of them reads the extensions section.** So a
domain article today is prose in a vendored file, binding permanently, checked by nothing.

This RFC does not implement that. It is named because it is the second family the layer exists
for, and because naming it is what stops the disclosure design from foreclosing it — see
[Composition](#composition-and-the-one-decision-this-rfc-defers).

### The escape hatch does not scale

`[lint] escalate_after`, `[propose] withdraw_uncited_after`, `[catalog] ttl_days`, `[due]`.
Four fields, four arguments, four releases — and every one of those arguments is right. What is
missing is not another field. It is somewhere for a repository to write a rule about itself
without waiting for a version of a binary it does not build.

## Design

### Rust computes facts; Rego decides

[`lint/model.rs:3`](../../yidam/cli/src/cmd/lint/model.rs) already draws this line:

> A check reports; it does not decide what is acceptable. That decision belongs to the
> baseline, and keeping the two apart is what lets a corpus carry known debt without either
> silencing the check or wedging the gate shut.

The policy layer is the same split, one level up. The binary walks the tree, parses the
frontmatter, resolves the routes, and knows which directories a bundle carries. None of that is
a judgement and none of it belongs in a policy. What the policy receives is a finished
description of one situation; what it returns is a verdict about it.

### The decision contract

**Input** — one JSON document, built by the binary, per decision:

```json
{
  "decision": "disclose/record",
  "repo":    { "private_paths": ["dossier"], "is_private": true },
  "subject": { "rel": ".yidam/catalog/pearl-2009.md", "kind": "catalog",
               "sha256": "…", "redistributable": true, "bytes": 12345 },
  "destination": { "vault": "sangha",
                   "audience": "The sangha. A licence to read, not to host." }
}
```

**Output** — one rule per decision, at a fixed name:

```rego
package yidam.disclose.record

decision := {"allow": count(deny) == 0, "deny": deny}

deny contains {
    "rule": "unstated-redistribution",
    "msg": sprintf("%s does not say whether these bytes may be redistributed", [input.subject.rel]),
} if {
    not is_boolean(input.subject.redistributable)
}
```

`decision` is a `:=` rule with no body, so in a well-formed policy it cannot be undefined. That
matters, because the three engine outcomes are not interchangeable and the binary must not
collapse them:

| Engine result | Meaning | The binary does |
|---|---|---|
| `Ok(object)` carrying `allow` and `deny` | the policy answered | render the verdict |
| `Ok(Undefined)` | the rule exists and did not fire | **error, exit nonzero** |
| `Err(_)` — missing package, absent builtin, timeout | the policy cannot answer | **error, exit nonzero** |

> **Authoritative does not mean absence permits.** The policy text decides. A policy that fails
> to answer is a failure, not a yes.

`allow: true` beside a non-empty `deny` is a contradiction in the policy, and is an error rather
than a permit for the same reason.

### Three decisions over two subject kinds

RFC-0023's implementation split disclosure into two questions with different evidence, and this
inherits the split rather than flattening it:

- A **record-bearing** artifact — a catalog entry naming fetched bytes — is judged by *what its
  record says*: `redistributable`, and whether the record's own path is declared private.
- A **derived** artifact — an index, an embedding set, a bundle — has no record, because nobody
  fetched it. `derived_may_push` judges it by *what it was built from*, and
  [`policy.rs:162`](../../yidam/cli/src/vault/policy.rs) says why that is the only honest
  question: *"An index is not a file that happens to sit in `.yidam/index/`; it is a re-encoding
  of everything walked to build it."*

| Decision | Subject | What it replaces, in P2 |
|---|---|---|
| `disclose.at_rest` | any working-tree file, given `repo.is_private` | `ci.yml`'s loop |
| `disclose.record` | a catalog artifact and its record | `vault::may_push` |
| `disclose.derived` | a computed artifact and its sources | `vault::derived_may_push` **and** `release.yml`'s loop |

One shared library, `yidam.disclose.lib`, carries the three predicates as **named functions**:

```rego
under(rel, declared)        # a record's own path — one-directional
intersects(src, declared)   # a source directory — both directions
with_content(paths)         # a placeholder is not material
all_paths(paths)            # …and where that distinction does not apply
```

Today the first two are inline predicates in one Rust file and two more inlined in bash.
Naming them, so that a reader can ask which one a rule used, is most of the value on offer.

**`holds_content` is not among them, and #438 is why.** Whether a directory contains a file is
a filesystem walk, and Rego has no filesystem — so the binary computes it and each declared
path arrives as `{"path": …, "holds_content": …}`. The *fact* moved to Rust and the *judgement*
— whether a placeholder counts as material — stayed in `lib.rego`, which is where the split
this RFC opens with actually falls once something has to run.

### Two things that are deliberately not decisions

**Routing.** [`policy.rs:118`](../../yidam/cli/src/vault/policy.rs) states the separation and
the reason it must hold:

> **Whether, not where.** Routing is `Vaults::route`'s question, and the two are kept apart
> because they fail differently: a route is edited casually by somebody reorganising storage,
> and a licence is not something that edit is allowed to undo.

So `vault: none`, `holds`, and the unroutable cases get no rule, and the decision input carries
no `vault:` field taken from the record. A caller asks both questions and needs both answers;
the policy answers one of them.

**`audience` as a rule.** Every vault must declare one and
[`config.rs:49`](../../yidam/cli/src/vault/config.rs) is explicit that nothing can check it. It
belongs in the input so a refusal can quote the destination back — which `cmd/vault.rs` already
does — and **no rule may branch on its prose.** A policy that pattern-matched an audience string
would be asserting it had read a sentence written for a human.

### Where policy lives, and why the vendored copy is not the authority

| | Path | Editable | Role |
|---|---|---|---|
| Default | `yidam/prelude/policy/disclose/*.rego` | upstream only | `include_str!`'d into the binary; vendored to `.yidam/.vendor/prelude/policy/` as the readable copy |
| Repository | `.yidam/policy/*.rego` | yes | overrides by package name; **authoritative when present** |

One file, not two: the bytes the binary embeds are the bytes the prelude vendors, so the copy a
reader inspects is the rule that ran.

The vendored copy cannot be the place a repository edits, and `directories.md` already says why
about the whole of `.yidam/.vendor/`:

> **Read-only.** […] An edit here is silently discarded the next time the prelude is
> re-vendored, and until then it is a local divergence nobody can see.

### Authoritative, and the argument that was made against it

**Decided: the repository's policy is the rule.** Where `.yidam/policy/` defines a decision,
that definition decides — including a rule more permissive than the default. Absent
`.yidam/policy/`, the embedded default decides, and behaviour is identical to today.

The alternative was **tighten-only** — `allow := builtin_allow AND policy_allow`, a policy able
to add refusals and never to remove one — and it was recommended, on Article I's ground that the
prelude is not subject to resolution. It was declined, and the reasoning is recorded here rather
than lost: a layer whose whole purpose is to stop the binary imposing one repository's judgement
on another cannot begin by reserving the interesting half of every judgement to the binary.

**The cost is real and is not waved away.** A repository can edit `.yidam/policy/disclose.rego`
so that nothing is ever refused, and every gate stays green. That is the same shape
`.yidam/private-paths` was built to end:

> An assumption about access control that looks enforced and is not is worse than one everybody
> knows is manual, because nobody checks the second kind by hand.

The remedy there was not prohibition. It was to make the declaration explicit. This RFC applies
that to itself: **a repository may loosen a rule; it may not do so silently.** #441 specifies the
three places that becomes visible — a `policy-override` lint finding at `Info`, a `doctor` line,
and the override diff `policy check` already produces.

`policy check` compares *text*, and says so. Whether a local rule is genuinely more permissive
than the default is a question about all possible inputs, and answering it needs a solver this
does not have. Report that the rule is local; do not claim to know which way it moved.

### Composition, and the one decision this RFC defers

Disclosure is authoritative. **The constitution cannot be**, and the reason is textual rather
than a matter of taste — Article I says the prelude is not subject to resolution, and the
extensions section says an augmentation *"may not contradict Articles I–VI."* An extension that
removed a refusal Article V imposes would be exactly that contradiction.

So composition is a property of a **family**, not of the layer, and this RFC fixes it only for
disclosure. When the constitutional family is built it needs the tighten-only composition
declined above, and the open question is where that gets declared — in the policy itself, or in
the Rust that owns the family. It is named here so that the disclosure design does not
foreclose it, and left open because nothing yet needs it settled.

## The engine, and why it is decided by measurement

`regorus` 0.11, `default-features = false, features = ["arc", "regex"]`.

### The count

| | |
|---|---|
| Marginal packages | **8** on a 154-package default build — `regorus`, `lazy_static`, `num-bigint`, `num-integer`, `num-traits`, `spin`, `thiserror`, `thiserror-impl` |
| Build-script dependencies | **zero** — `cargo tree -e build` over its closure is empty |
| C, protoc, CMake, system libraries | none |

Measured against this repository's real graph, the way #412 measured the transport. The
zero-build-script result is the load-bearing half: the aarch64 cross-compile in `release.yml` is
the job that fails on a dependency needing a C toolchain for the target, and a closure with no
build scripts cannot be that dependency.

Enabling the `std` feature would cost **16** rather than 8 — it pulls `rand` and `parking_lot` —
and nothing here needs it. The `ExecutionTimerConfig` wall-clock limit, which is what stops a
committed policy hanging CI, works without it.

### Hermeticity is a property of the dependency graph, not of review

The features deliberately left off are the security argument. Both confirmed by running them:

```
http.send   → error: could not find function http.send      (no `http` feature)
time.now_ns → error: could not find function time.now_ns    (no `time`/`std` feature)
```

A committed policy cannot make a network call, so CI stays hermetic — the property the privacy
job exists to preserve, and which `directories.md` names as the reason no egress check can be
built here. It cannot read a clock, so evaluation is deterministic and goldens are stable.

This is worth stating as a design property rather than a footnote: **the guarantee is enforced
by the crate's feature resolution, and a reviewer who forgets it cannot weaken it.** The way to
break it is to add a feature, which is a diff in `Cargo.toml` and not a diff in a `.rego` file.

### The sharp edge, found by probing rather than by reading

Both refusals happen at **evaluation**, not at parse. A policy calling `http.send` compiles
clean and fails at the moment the decision is needed — which, for a gate, is the worst available
time.

So `yidam policy check` carries a `BUILTIN_ALLOWLIST` and scans `get_ast_as_json()` for call
targets outside it. **This is not the mechanism** — the absent cargo feature is — and the
module must say so, or the next reader deletes it as redundant. What it buys is *when* you find
out.

### Rejected

- **Shelling out to the `opa` binary.** A runtime dependency on a tool nobody has installed, in
  a CLI whose whole distribution argument (RFC-0003) is that it installs as one artifact. It
  would also put a network-capable interpreter behind the gate, discarding the entire
  hermeticity argument above.
- **Compiling policy to WASM and evaluating it in all three SDKs.** The cross-language story is
  genuinely attractive here, given the parity problem the RFC index opens with. It needs a WASM
  runtime — `wasmtime` is the realistic choice and it is an order of magnitude past the whole
  current dependency graph. Deferred, not refused: if the SDKs ever need to evaluate policy,
  this is the shape to revisit. Today they parse Markdown and gate nothing.
- **A bespoke rule DSL.** RFC-0018 invented a query language and argued for it, and the argument
  turned on the ontology: there was no existing language that could typecheck against `.ont.yml`.
  There is no equivalent gap here. Rego is a decided language for exactly this problem, has a
  test format, and is the thing a reader may already know.
- **A fifth config field.** It is what the last four times produced, and it produces a
  vocabulary of knobs rather than a place to write a rule.

## Feature gating — why this one is ungated

`vault-s3` is in the default set because *"PR CI never compiles `--features index`, so gated
code ships that no pull request has built."* That argument applies here too, and a second one
goes further.

[`vault/mod.rs:15`](../../yidam/cli/src/vault/mod.rs) states the shape:

> This module is **ungated**. […] the feature buys the *network*, and reading a file, resolving
> a path and hashing bytes are none of them.

Evaluating a policy is none of them either. And under the authoritative model, **a build that
cannot evaluate policy is a build that cannot refuse** — gating it would make the light binary
the one with no guard, which inverts the reason the light binary exists. So: ungated, and the
eight packages are the price.

## Phasing

| | Issue | What lands |
|---|---|---|
| P0 | #438 | the engine, the decision contract, the allowlist, the default `disclose.rego`. **No caller.** |
| P1 | #439 | `yidam policy check` / `eval` / `test`, the `.rego` tests, and the equivalence proof |
| P2 | #440 | the four call sites swap; `release.yml` asks instead of mirroring; the list assertion in `publish_guard.rs` retires |
| P3 | #441 | an override becomes visible — `policy-override` at `Info`, a `doctor` line |

P0 and P1 change no behaviour. A repository with no `.yidam/policy/` is bit-identical to today,
and `vault push` still calls `may_push`.

## Testing

**The equivalence proof is the phase-1 deliverable**, and it is what makes P2 a swap rather than
a rewrite of the guard on the only path that can leak. Two matrices, because RFC-0023 left two
Rust guards:

```
disclose.record   ↔  vault::may_push
    redistributable ∈ {absent, true, false}
  × rel             ∈ {under a declared path, outside one, the `dossiers` name-prefix trap}

disclose.derived  ↔  vault::derived_may_push
    kind            ∈ {Index, Embeddings, Bundle}
  × declared        ∈ {inside a source dir, containing one, disjoint}
  × content         ∈ {a real file, README.md only, .gitkeep only}
```

Same verdict, and the same *reason* wherever the message is the contract rather than prose.

**The `content` axis is the one that will be got wrong.** `holds_content`'s placeholder
exemption is what a naive transcription drops, and dropping it passes every other case in the
matrix while making the feature unusable for a repository that declared its intent before it had
anything to protect — which is the order `directories.md` asks people to work in.

Beyond it:

- **Mutate before trusting green.** Break one rule in `disclose.rego` on purpose and confirm the
  equivalence test goes red. A file-scanning test that looks at nothing passes, and a policy file
  answers a grep the same way its comments do.
- **Prove the hermeticity claim rather than citing it.** A test that adds a policy calling
  `http.send` and asserts the evaluation errors. It is a claim about a feature set, which is
  exactly the kind that rots silently when somebody enables a feature to fix something else.
- Goldens for `policy check` and `policy eval --format json`, beside `tests/goldens/query/`.

## What this does not touch

- **No runtime egress check.** `directories.md`'s "What this does not cover" stands unchanged: an
  egress check would have to know every network call the domain computer makes, and CI is
  hermetic precisely so that it makes none. This layer gates a channel the binary owns. It does
  not learn about the ones it does not.
- **No lint check and no severity map.** The gate's judgement is a plausible third family and is
  not specified here.
- **No parity-surface change**, following RFC-0018.
- **No MCP contract change.** `serve` discloses nodes to an agent, which is a disclosure channel
  and a tempting fourth decision. It is out of scope because it is a *runtime* channel and the
  paragraph above governs it.

## What #438 found

Three things the design did not survive contact with, each recorded because each changed code.

### Rego has no filesystem, and that is where the split really falls

Above, in the library list. The RFC named `holds_content` as a policy function; it is a
filesystem walk, so it became an input field. The line between fact and judgement is not where
it first looked, and it moved *towards* the design rather than away from it.

### Cross-package function calls must be fully qualified

`import data.yidam.disclose.lib` followed by `lib.under(…)` resolves for a **rule** and not for
a **function**. regorus 0.11 reports `could not find function lib.under` — at evaluation, which
is to say when a decision is needed. Measured in every form: bare import, `as` alias, and
importing the function itself; all three fail, and the fully qualified
`data.yidam.disclose.lib.under(…)` works.

So the shipped policies are verbose on purpose, and each carries a comment saying so. Tidying
one into an import is a change that compiles, passes `policy check`, and fails the first time
somebody pushes.

### The `ast` feature is free, so the builtin scan reads the tree rather than the text

`get_ast_as_json` is behind a feature that resolves **zero** additional packages. That is worth
having: a text scan for `http.send` is answered just as well by a comment explaining why
`http.send` is forbidden, and the default policy contains exactly such a comment. The scan walks
`Call.fcn` — a `Var` for an unqualified builtin, a `RefDot` chain for a namespaced one — and a
test pins that a comment naming a forbidden builtin is not a call.

The scan denies by **namespace**, resting on a property of Rego worth stating so it can be
argued with: every builtin that reads the world is namespaced, and the unqualified ones
(`count`, `sprintf`, `startswith`, `concat`) are pure by construction. If a future Rego grows an
unqualified impure builtin, this check stops being sufficient — and the feature resolution in
`Cargo.toml` is still what actually refuses it.

## Decided since drafting

**Open question 1 — `at_rest` and the privacy job: lazy install.** The job keeps its shell
early-exit, so a repository that has declared nothing pays the runner-second it pays today and
nothing more. It installs the binary and calls `yidam policy eval` only when
`.yidam/private-paths` exists. This preserves what the job was built for — *the rule is one file
away rather than a workflow edit nobody remembers to make* — while putting the decision where
material actually exists. #440 implements it.

**Open question 4 — where a broken policy is caught: `yidam doctor`.** No new mise task and no
new CI job in derived repositories. `doctor` compiles every policy, runs the builtin scan, and
reports which decisions are local; it is already offline and read-only and this reads two
directories. It lands with #441's doctor line rather than as separate machinery.

## Open questions

1. **What happens to an override when the default moves?** A repository overrides
   `disclose.derived`; the next re-vendor changes the default it diverged from. Nothing detects
   that, and it is the same shape as the `cli_ref` pin RFC-0004 found enforced by nothing.
2. **Where does a family declare its composition?** Named in
   [Composition](#composition-and-the-one-decision-this-rfc-defers). In the policy, or in the Rust
   that owns the family — undecided, and nothing needs it until the constitutional family exists.
3. **Should a denial carry a severity?** Every disclosure denial refuses. A constitutional or
   lint-severity family would want `Warn`, and the report contract already distinguishes them.
   Not answered here, because inventing the field before a family needs it is how the four
   config knobs happened.
