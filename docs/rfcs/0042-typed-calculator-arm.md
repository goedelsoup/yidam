# RFC-0042 — A calculator whose purity is a closed scope and a typecheck, not a norm

- **Status:** Draft
- **Track:** I31
- **Relates to:**
  - RFC-0026 (the manifest and the executor this adds a second arm to, and whose shell contract stays the documented first one)
  - RFC-0024 (the bar an embedded engine is measured against, and the hermeticity argument this engine **cannot** make in the same form)
  - RFC-0003 (the light binary this deliberately stays out of)
  - RFC-0028 (the register constraint on what a run may write, which the second arm inherits unchanged)
- **Versioning layers touched:** template (`guidelines/directories.md` and `docs/domain-computer.md`
  gain the second arm's field) / tooling (`yidam` CLI, behind a cargo feature **outside** the
  default set) — no parity-surface change, no MCP contract change. The bootstrap protocol was
  untouched until #1184; see *Amended: a calculator declared before it is built*
- **Downstream reference case:** `examples/streamflow`'s calculators — 170 lines of `sh` and `awk`
  in `travel-tier.sh` and 110 in `disclosure-envelope.sh`, most of which is not the rule either one
  computes. **Landed in #1102 as `travel-tier-typed.glu`**, a third capability declaring
  `run = { gluon = … }` beside the shell one and computing the same chain fixed point. It commits
  the rule's intermediate facts — `weakest_beneath` and `chain_depth` — rather than the tier, because
  a signal name is corpus-wide and two computed files may not claim one. `disclosure-envelope.sh` is
  the pipeline's second stage and reads the first's computed signals; **#1105 put them in the
  projection**, as a `signals` field on `Corpus` carrying what the step's `reads` resolve to, and
  `a_second_stage_computes_from_a_previous_step_s_signal` in `tests/gates/gluon_arm.rs` is that rule in
  the typed arm. It is not declared beside the shell one, for the same reason the tier is not:
  the whole of its output is `reaches`, and unlike the chain rule it has no intermediate fact left
  to commit under another name
- **Specifies:** #1079. Depends on nothing; #1080 is the gluon-independent half of the same
  finding and lands first if it lands at all

## Summary

A calculator is a shell command. RFC-0026 said so on purpose, and the executor's own doc comment
gives the reason — *"a vertical slice that needed a build system to demonstrate would be
demonstrating the build system"* ([`process`](../../yidam/cli/src/cmd/run/exec.rs#L301-L313)). What
it costs is that the property `docs/domain-computer.md` states about the whole kind is
unenforceable. This RFC proposes a **second arm** on the same manifest field — a script in an
embedded, statically typed, effect-tracked language, `run = { gluon = "…" }` beside
`run = ["sh", …]` — where a calculator's purity is a closed scope plus the declared type of its
entry point, and the refusal comes before the script runs rather than after the tree is written. It is behind a cargo feature
outside the default set, and a build that cannot run one refuses it by name.

The RFC is `Draft` because the measurement does not obviously pass. RFC-0024 chose `regorus` at
**8 marginal packages**; gluon 0.18.4 costs **71** and about **+6.8 MB** on a 15 MB binary. Those
numbers are recorded below so that nobody re-measures them, and so that the decision is made
against them rather than against the design's appeal.

**Amended after implementation (#1087).** The engine landed in #1088, and building it disproved
this RFC's central claim in its original form: the entry point's type is *necessary and not
sufficient*, because `std.debug.trace` is declared `a -> ()` and prints to stdout. The mechanism
that closes it is a closed prelude — an allowlist of fifteen pure `std` modules plus a lexical
refusal of every macro invocation — with the typecheck beside it. What survives is a weaker claim
than *purity is a typecheck* and a stronger one than a denylist's; the section below states it,
and the title changed with it. Nothing about the cost measurement or the decision rule changed.

## Problem

### The norm is documented, and enforced only on what a step wrote

`docs/domain-computer.md` types the whole kind by four properties:

> Pure, deterministic transforms — No network, no filesystem; same input always produces same
> output
>
> — [`Calculators`](../domain-computer.md#L23-L28)

Three of those four are unenforced. `materialize` checks out the declared `reads` into a scratch
tree, [`process`](../../yidam/cli/src/cmd/run/exec.rs#L301-L313) spawns `run[0]` in it with five
environment variables, and the executor afterwards refuses a step that wrote outside `writes`.
That is a real guarantee about **writes** and about nothing else. A calculator may read the
clock, resolve a hostname, open a socket, or read any path on the machine outside its scratch
tree, and the run will commit its output with a receipt that says the input state was the
declared one. Determinism is asserted by the manifest's ageing rule — a step with no
`ageing_days` is re-run only when its inputs move — and a calculator that read the clock makes
that assertion false without making anything red.

This is the shape RFC-0024 names in its own engine section: a property enforced by review is a
property a reviewer can forget. The difference is that RFC-0024 could close it by resolving
features, and here there is no `Cargo.toml` to close it in, because the thing being invoked is
whatever the corpus author wrote.

### The shell contract's second cost, and the half that is separable

A calculator that is handed bytes does the CLI's work again before it can do its own. #1080
records it in full: `travel-tier.sh` parses node YAML by regex, re-implements link-target
resolution as an awk `function resolve(base, t)` against the one resolver in
`corpus/edges.rs`, and sorts under `LC_ALL=C` so that file order is a property of the corpus
rather than of the locale — all three of which the CLI already does, once, and already knows
what to do with a malformed file.

That half is separable and is **not** this RFC's argument. #1080 fixes it by materializing the
resolved graph beside the bytes, and a shell calculator gets the benefit without an interpreter.
What remains for an embedded language is the part #1080 cannot reach: the resolved graph arrives
as a JSON file that a shell script must still parse, and reading it is still an effect nothing
can refuse. An arm that receives a typed value has no parse to get wrong and no file to read.

### Why this is a second arm and not a replacement

The shell contract is four environment variables and a working directory, and its cost of entry
is a text editor. It is what makes a calculator writable by somebody who will not install a
toolchain, and it is what the two worked examples and the walkthrough teach. Nothing in this
RFC removes it, deprecates it, or moves it out of the default build. The claim is narrower:
there is a class of calculator — corpus in, computed file out, no world outside the corpus —
for which a typed arm is strictly better, and that class is every calculator this repository has
yet written.

## The engine, and the measurement it has to survive

[gluon](https://github.com/gluon-lang/gluon) 0.18.4, `default-features = false`. Measured
2026-09-26 against `main` at `3111beb` with a probe crate on the 1.88 pin, against the bar
RFC-0024 set — marginal packages on the real default build, build scripts, hermeticity by
feature resolution.

### The count

| | regorus (RFC-0024) | gluon 0.18.4 |
|---|---|---|
| Marginal packages on the default build | **8** on 154 | **71** on 179 — 47 new names, 24 old-major duplicates (`syn 1`, `futures 0.1`, `hashbrown 0.12`, `indexmap 1`, `parking_lot 0.11`, `bitflags 1`, `cfg-if 0.1`, `crossbeam-utils 0.7`) |
| Build scripts in the closure | 0 | **0** |
| Licences outside `deny.toml`'s allowlist | 0 | **0** |
| Release binary delta | — | about **+6.8 MB** on a ~15 MB default binary (7.20 MB probe against a 0.40 MB hello-world, default release profile) |
| Compiles on the 1.88 pin | yes | only through `incompatible-rust-versions = "fallback"`, which `.cargo/config.toml` already sets — `ordered-float 5.5` wants 1.90 and the resolver falls back to 5.4 |
| VM init / first compile+run / cached rerun | — | <5 ms / 5–11 ms / 0.2 ms |
| Upstream | — | no release 2023-09 → 2026-07, then 0.18.3 (2026-07) and 0.18.4 (2026-08) |

The two rows that matter are the first and the fourth, and they point the same way. **71 is
nine times the number this repository has ever accepted for an embedded engine**, and 24 of them
are old-major duplicates of crates already in the graph — which is not merely size but a second
copy of `syn`, `futures` and `hashbrown` in every `cargo audit` and every `cargo deny` run from
now on. The zero-build-script row is the one that makes the proposal admissible at all: it is
the row RFC-0024 called *"the load-bearing half"*, because the aarch64 cross-compile in
`release.yml` is the job that fails on a dependency needing a C toolchain, and this closure
cannot be that dependency.

The 1.88 row is worth reading as a warning rather than a cost. The fallback setting is already
there, so nothing has to change — but the reason it is needed is that gluon's closure reaches
crates published against a newer pin than this repository holds, and that gap widens on its own.

### Hermeticity is not a cargo feature here, and this RFC says so plainly

RFC-0024's security argument is one sentence: *the guarantee is enforced by the crate's feature
resolution, and a reviewer who forgets it cannot weaken it.* **That sentence cannot be written
about gluon.** With default features off, `VmBuilder::build` still registers `std.fs`, `std.io`,
`std.process`, `std.env`, `std.thread` and `std.channel` unconditionally. They are not
`cfg`-gated the way `regex`, `web` and `random` are, and no feature combination removes them.

Every one of those primitives routes its effect through `IO<_>`, and gluon's standard library
offers no pure escape from `IO` — no `unsafePerformIO`, no `runIO` reachable from a script. So a
calculator whose entry point is declared `Corpus -> Computed`, and **not**
`Corpus -> IO Computed`, cannot perform *those* effects, and the refusal arrives at typecheck.
That is earlier than the eval-time refusal RFC-0024 records as Rego's sharp edge: Rego says
*could not find function `http.send`* when the offending line is reached, and a policy whose
network call sits on an untaken branch passes every test.

#### The type is necessary and not sufficient, which building it is how we found out

An earlier draft of this section said the entry point's type was **the** mechanism, with a
denylist `Importer` as defence in depth. That is wrong, and the correction is the most useful
thing #1088 produced (#1087).

`std.debug.trace` is declared `a -> ()`. It is registered unconditionally, like the six above,
and it is `println!` to stdout. So a function whose declared type is *exactly* a calculator's can
perform an effect and typecheck — `\c -> let noise = debug.trace c in { signals = [], summary = [] }`
has the calculator's exact type and no `IO` anywhere in it. For this CLI stdout is where `--format json` goes, which makes it the worst of
the effects on offer rather than a harmless one.

The general statement is the one to carry forward: **`IO` tracks the effects gluon routes through
`IO`.** It does not track an effect a primitive declared its way out of, and nothing in the type
system says a registered primitive told the truth about its own signature. A type system is a
claim about composition, not an audit of the host's registrations.

So the guarantee lives in two mechanisms, and the scope is the primary one:

- **A closed prelude.** The script is evaluated with a fixed prefix binding fifteen named pure
  modules — [`PRELUDE_MODULES`](../../yidam/cli/src/gluon_arm/mod.rs#L108-L111) — and any macro
  invocation *in the script* is refused lexically by
  [`refuse_macros`](../../yidam/cli/src/gluon_arm/entry.rs#L213-L215). Since gluon reaches a native
  module only through `import!`, a script that cannot invoke a macro can only use the names the
  prelude bound. **Closed by default:** a module nobody listed is an undefined variable, so a
  module a future gluon registers is out of reach on the day it is added rather than on the day
  somebody remembers to deny it.
- **The entry point's typecheck**, which still refuses every effect routed through `IO`, and which
  is what catches a script that is wrong about its own shape rather than about its effects. It is
  checked by the CLI and not taken on trust from the script: the host resolves the declared
  signature and reports the type it found beside the type it wanted.

A gluon calculator that so much as names `std.fs` still does not compile — but for the first
reason, not the second. [`EXCLUDED`](../../yidam/cli/src/gluon_arm/mod.rs#L119-L123) records each
module that is deliberately unreachable beside the reason, `debug` among them, and the reason
given there is this subsection in one line. The claim that survives review is therefore weaker
than the earlier draft's and *stronger* than a denylist's, and it is the one the module doc now
carries.

### What the numbers do not settle, and the decision rule

They do not settle it either way, which is why this is a `Draft` and not an `Accepted`. The
decision rule this RFC proposes:

> 71 marginal packages and +6.8 MB are acceptable **only** outside the default set, and only for
> a capability the light binary refuses by name rather than degrades on. If the arm cannot be
> gated — if anything in the default build ends up linking it — the proposal fails on the first
> row of the table and should be closed.

## Proposal

### A second arm on the same field, additive

`run` becomes a sum: the sequence it already is, or a table naming one language and one script.

```toml
[capability.travel-tier]
kind   = "calculator"
run    = { gluon = ".yidam/capabilities/travel-tier.glu" }
reads  = [".yidam/corpus/**"]
writes = [".yidam/computed/**"]
verb   = "compute"
```

A sum on [`run`](../../yidam/cli/src/cmd/run/manifest.rs#L213-L239), so the sequence form parses
byte-identically to today and no existing manifest changes. The table form takes `gluon` and an
optional `calls`, and an unknown key is refused at load with the arms named — the manifest's
`deny_unknown_fields` rule applied to a field that now has a shape rather than a type.

**Implemented with a hand-written `Deserialize` rather than `#[serde(untagged)]`, which is where
this section was wrong.** Untagged reads the shape correctly and reports `data did not match any
variant of untagged enum Run` when it does not, naming neither the field nor what was wrong with
it. A manifest is a file a person writes by hand, and the two arms are told apart by *array versus
table* — a distinction an error message can state. Serialization stays derived-untagged, so a
receipt already committed for a shell step keeps its exact bytes.

Everything else on the declaration is unchanged and keeps its present meaning: `writes` still
bounds what may be committed, `after` still orders the graph, `verb` still decides where the
commit lands, `ageing_days` still means what RFC-0026 §6 says. A second arm on `run` is not a
second kind, and `Kind::Calculator` stays one value.

### Behind a feature, outside the default set

`calculators-gluon`, and RFC-0024's argument for ungating `regorus` deliberately does not
transfer. That argument was *a build that cannot evaluate policy is a build that cannot refuse*
— the light binary must be able to apply a rule, so the engine that applies rules belongs in it.
A light build that cannot run a gluon calculator is in no such position: it refuses the step by
name, commits nothing, and says which feature would run it, which is exactly what
[`Kind::unrunnable_because`](../../yidam/cli/src/cmd/run/manifest.rs#L121-L135) already does for a
connector and a featurizer.

That is a third case on an existing mechanism rather than a new one, with one difference worth
stating: a connector's refusal is a decision and a featurizer's is an absence, and this one is
neither — it is a property of the build in hand. Which is why the predicate gained a **sibling**
rather than a third arm. `Kind::unrunnable_because` is a property of the taxonomy and the same in
every build; [`Run::unrunnable_because`](../../yidam/cli/src/cmd/run/manifest.rs#L409-L429) is a
property of this binary; and only the whole declaration knows it has to ask both, so
[`Capability::unrunnable_because`](../../yidam/cli/src/cmd/run/manifest.rs#L520-L531) asks the kind
first. A featurizer declaring a `.glu` is refused for being a featurizer, because no build has that
executor and the feature is not the thing its author can act on. So the sentence names the feature, and the
manifest still **parses** in a light build, because a corpus that declares a gluon calculator is
not a malformed corpus. Refusing to parse would make `yidam lint` and `yidam graph` fail on a
repository whose only sin is a capability this binary cannot invoke.

`--features full` gains it. The default set does not, and `tests/gates/light_build.rs` is where that is
asserted rather than in prose.

### The calculator receives a typed corpus, not a scratch tree

No `git checkout-index` for a corpus-only step. The CLI already holds the parsed corpus in
memory, and the arm marshals it through gluon's `VmType`/`Pushable` derives over
`corpus::{Node, Class, Edges}` — the same accessors `lint::input::Input` already exposes to the
checks. The entry point is `Corpus -> Computed`; `Computed` is the shape the computed file
already takes, and the CLI serializes it.

What the step reads is therefore a value, not a path, and two consequences follow. The
receipt's input hash covers the corpus commit as it already does, and there is nothing else the
step could have read — which is the difference between #1080's improvement and this arm.
`reads` keeps its meaning for the **slice** the value carries, so that a calculator's
declaration still bounds what it may see; whether the value is sliced to the globs or always
whole is the same question #1080 has to answer, and both surfaces answer it the same way or the
two arms diverge.

A step that genuinely needs bytes — prose, a binary, a file the resolved form does not carry —
declares the shell arm. That is not a fallback, it is the arm that fits.

**The value carries what previous steps computed, too (#1105).** As implemented the projection was
`nodes` and `classes` alone, which made the arm's reach *calculators that read only the graph* — a
pipeline's second stage was outside it, because a typed entry point handed no signals had no input
for the rule it would compute. `Corpus` gained a `signals` field: the `.yidam/computed/` files the
step's `reads` resolve to, read back through `computed::Signals` and carried as the same rows the
entry point returns. It is a field on `Corpus` rather than on each node deliberately — the tree a
step stands in holds only what it declared, so a second stage that reads one computed file and no
corpus path is handed signals about nodes that are not in `nodes`. Attaching them to the node would
have made the typed arm need a wider declaration than the shell arm for the same rule, which is
backwards for an arm whose claim is that `reads` bounds the value exactly.

### A budget through the VM hook

`set_hook` with `CALL_FLAG`, returning `Interrupted` past a call budget: the counterpart of
`regorus`'s `ExecutionTimerConfig`, and there for the same reason — a committed script that
loops is CI that hangs rather than CI that fails. A budget in calls rather than wall-clock
because a call count is a function of the corpus and the script, and a deadline is a function of
the machine; a run whose refusal depends on which runner picked it up is a run whose receipt
means less than it claims. The budget's value, and whether a corpus may raise it, is an open
question below.

### The smaller proof of concept, if one is wanted first

A corpus-declared REGEN generator, `Corpus -> String`. `regen.rs` is a closed list of Rust
`fn() -> Result<()>` invoked from one place, so a generator a corpus writes has no sandbox to
design, no receipt to extend, no `writes` to enforce and no gate semantics to decide — the
existing `--check` mode already compares generated text against committed text, and a stale
block is already a finding. It exercises the marshalling, the entry-point typecheck and the
budget against a surface where being wrong costs a diff. #1071 asks for a `count` generator in
Rust and is the adjacent case: if that lands as Rust, this PoC has one fewer reason to exist.

## What this does not touch

The shell arm, which stays and stays the documented first slice, and the four environment
variables it is invoked with. The three-kind taxonomy — this is a second way to write a
calculator, not a fourth kind. The parity surface: a manifest is Rust-only and `manifest.rs`
says why, and a second arm does not make it a parity function. The MCP contract, which names no
capability's implementation. The receipt format, whose input hash already covers the corpus
commit. `policy/` and `regorus`, which decide a different question and share no code with this.
And the bootstrap scaffold, which writes the manifest with no capability in it.

## Amended: a calculator declared before it is built (#1184)

The section above said this RFC left the bootstrap alone, and it did. #1063's review of seven
derived repositories found what that left behind: most of the 45 skills in their `.yidam/skills/`
were calculator specs, because step 7 wrote an approved calculator the seeded corpus could not
yet run as a skill stub. A skill is a procedure an agent follows. The runner never reads one, so
the place that would have run the calculator never heard of it.

**`run` may be absent, and absent is its own shape.** A declaration with no `run` parses to
`Run::Unbuilt`: its `reads`, `writes` and `verb` are validated like any other, so the contract
exists before the program does. `run = []` is still refused as empty, because a typo should not
read as a stub.

**It is refused, not skipped.** Every plan holding it is refused before anything runs, the way a
connector's is, in every build. So a bare `yidam run` refuses the whole manifest until the
calculator gains a `run` or its declaration is deleted. The alternative was to skip it and list it
in the report. That was rejected because a run reporting success over a declared computation that
never happened is the gate that is green because it is not looking. `yidam doctor` reports the
step as *declared and not built* rather than *never run*, since the second sends its reader to
run it.

A cluster workflow asks the kind and the absence, but not this build's feature. It is generated on
one machine for pods running another image, and only the first two are true of every image.

Bootstrap step 7 now writes the declaration, not the stub, and `implement:` names it.

## Migration & compatibility

Nothing migrates. An existing manifest's `run = ["sh", …]` parses as it does today, and a
derived repository that never writes the table form never links the engine. The tooling layer
bumps; no template version does, beyond the two documents that gain the field's second shape.

Adoption is: install a build with the feature, write the `.glu`, change one line of the
manifest. The reverse is the same move — a corpus that abandons the arm rewrites the script in
`sh` and changes the line back, and its committed computed files are unaffected either way,
because the arm changes how a number is computed and not what the computed file looks like.

The one thing that is not backward-compatible is a **corpus** carrying a gluon arm and a
**consumer** on the released light binary. That is the case `Capability::unrunnable_because` answers, and the
answer is a named refusal with nothing committed. A corpus that wants to be runnable by the
downloaded binary writes the shell arm; the manifest is the place that choice is visible, which
is the property RFC-0026 built it for.

## Alternatives considered

**A Rust plugin (`dlopen`, or a WASM module).** WASM is the serious one: `wasmtime` gives a
sandbox enforced by the runtime rather than by a type, which is the stronger property on the
axis this RFC is weakest. It costs a dependency closure larger than gluon's, a toolchain a
corpus author must install to produce a `.wasm` at all, and a component model to design for
passing a corpus across the boundary. It is the right answer if the threat model is a hostile
calculator; this one is a mistaken calculator, and a committed script a reviewer reads is not
the same artifact as an opaque binary.

**Rhai, or another dynamically typed embedded scripting language.** Far cheaper — a handful of
marginal packages — and it buys nothing this RFC is about. An effect is refused at call time or
by not registering the function, which is RFC-0024's sharp edge again; there is no declared
entry-point type to check, so purity returns to being a norm. If the conclusion is that 71
packages is too many, the fallback is **not** a cheaper interpreter with weaker guarantees, it
is #1080 and the shell arm.

**Lua via `mlua`.** Same objection as Rhai, plus a C dependency, which is the row that decides
the aarch64 cross-compile.

**Extend the shell contract instead (#1080 alone).** The honest baseline, and the thing to do
first regardless. It removes the awk, and it leaves the purity norm exactly as unenforceable as
it is today.

**A denylist `Importer`, or a denylist of module names in any form.** Rejected, and implementing
the arm turned this from a preference into the decisive reason. A denylist is a list somebody
maintains, which is the guard-list shape this repository has filed against itself before — and
worse here, because the module it would have missed is the one that matters. `std.debug` is
pure-typed; anybody deriving a denylist from *which modules perform effects* leaves it off.

What replaced it is an **allowlist**, which is the same shape turned around and is not the same
failure mode. Fifteen names is a list somebody maintains too, but it is closed by default: the
cost of forgetting an entry is a calculator author who cannot use `std.json` until somebody adds
it, and the cost of forgetting an entry on a denylist is a script that reaches the network. The
list is also small enough to justify item by item, which the RFC-0024 allowlist argument turns
on, and a test holds it disjoint from the excluded set so the two cannot drift into agreeing.

The entry point's type remains beside it, and is necessary rather than sufficient — see the
subsection above for the measurement that settled which is which.

## Open questions

1. **Does 71 clear the bar at all?** This RFC does not assert that it does. The decision rule
   above is the form the answer should take, and the answer belongs in review, not here.
2. ~~**Sliced or whole.**~~ **Answered in #1091: sliced, the same slice #1080 hands a shell step.**
   The typed arm marshals the corpus out of the materialized input tree rather than out of the
   reader the CLI holds, so `reads` bounds what a calculator can see in both arms by construction
   rather than by two rules that have to agree. `guidelines/directories.md` states it beside the
   field, once, for both.
3. ~~**The budget's value, and whether a corpus may raise it.**~~ **Answered in #1091: declarable,
   with the binary's default where it is absent.** `run = { gluon = "…", calls = N }`, and the
   argument is `ageing_days`': a number compiled in is one corpus's judgement arriving in another
   that never agreed to it, and a calculator over ten thousand nodes and one over ten are not the
   same computation. `calls` sits **inside** the table rather than beside it, which buys two things
   for free — it is part of `run`, so it is already in the input state and a changed budget re-runs
   the step, and declaring it on a sequence arm is refused by the same rule that refuses any other
   unknown key. The default is resolved where it is spent and not at parse time, because it lives
   behind the feature and a light build must still validate the declaration.
4. ~~**What the receipt records about the script.**~~ **Answered in #1091: both mechanisms, not
   either.** The manifest refuses a typed declaration whose `reads` do not cover its own `.glu`, so
   the script is in `files` and the shell arm's rule holds unchanged; *and* the receipt carries a
   `script_sha256`, taken from those resolved inputs. The second is deliberately redundant, which is
   the point of it: the input state should not *depend* on the coverage rule staying in place, and a
   receipt that names the program it ran is readable without reconstructing which of many files was
   the program. It is one function — `Receipt::script_sha256(cap, files)` — because `run` resolves
   its inputs from a commit and `doctor` from the working tree, and two independent digests of the
   same script would report a typed step stale forever. That is #1080's "one builder, one answer"
   repeated one field along.
5. ~~**Whether a typechecked-but-failing script is a different refusal.**~~ **Answered in #1091
   and #1099: all three are manifest failures, reported as three checks, and told apart.** A
   script that typechecks and then returns a `Computed` the corpus cannot accept stays a **step**
   failure — nothing decides it without running the script. The three ways a script is not a
   calculator at all are decidable from the declaration, like a cycle in `after`, and `yidam lint`
   now decides them:

   | check | refusal | needs |
   |---|---|---|
   | `calculator-script` | the path does not read, or the script invokes a macro | text |
   | `calculator-scope` | a name the closed prelude does not bind | the typechecker |
   | `calculator-type` | not `Corpus -> Computed` | the typechecker |

   The constraint this question turned on is satisfied by construction rather than by agreement.
   `lint` does not build a prelude, inject a type or decide what a calculator is: it calls
   `entry::admit`, the one function [`evaluate`](../../yidam/cli/src/gluon_arm/mod.rs#L199)
   opens with, and `tests/gates/gluon_arm.rs::one_admission_answers_both_callers` asserts those are the
   only two callers. There is no second opinion to diverge, so the paragraph below — a gate that
   either passes something `run` refuses or refuses something `run` would have accepted — names a
   failure mode the design has no place to hold. That is #1080's "one builder, one answer" again.

   The middle refusal no longer reads as a type error. It is classified structurally, from
   gluon's `UndefinedVariable` and `UndefinedType`, and reported as a scope failure that prints
   what the prelude binds — in `run` and `lint` alike, because both go through the one function.

   The split across builds is by **what each build can answer**. The macro scan and
   `PRELUDE_MODULES` are text and a list of names, so `calculator-script` gates identically in the
   light binary `install.sh` downloads; the other two need a VM and are absent from that build's
   report. Absent rather than empty: a binary that held no script to the prelude should not report
   that it did. A baseline entry for either is carried through that build, not resolved (#1114).
6. **Upstream health.** Three years dormant, then two releases in two months. One maintainer's
   renewed attention is not a maintenance guarantee, and an embedded language is harder to
   replace than a policy engine. Vendoring is not an answer at this closure size.
