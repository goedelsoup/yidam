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
  default set) — no parity-surface change, no MCP contract change, no bootstrap-protocol change
- **Downstream reference case:** `examples/streamflow`'s two calculators — 190 lines of `sh` and
  `awk` in `travel-tier.sh` and 108 in `disclosure-envelope.sh`, most of which is not the rule
  either one computes
- **Specifies:** #1079. Depends on nothing; #1080 is the gluon-independent half of the same
  finding and lands first if it lands at all

## Summary

A calculator is a shell command. RFC-0026 said so on purpose, and the executor's own doc comment
gives the reason — *"a vertical slice that needed a build system to demonstrate would be
demonstrating the build system"* ([`invoke`](../../yidam/cli/src/cmd/run/exec.rs#L155-L170)). What
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
> — [`Calculators`](../domain-computer.md#L16-L21)

Three of those four are unenforced. `materialize` checks out the declared `reads` into a scratch
tree, [`invoke`](../../yidam/cli/src/cmd/run/exec.rs#L155-L170) spawns `run[0]` in it with four
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
perform an effect and typecheck — `\c -> { signals = [], x = debug.trace c }` has no `IO`
anywhere in it. For this CLI stdout is where `--format json` goes, which makes it the worst of
the effects on offer rather than a harmless one.

The general statement is the one to carry forward: **`IO` tracks the effects gluon routes through
`IO`.** It does not track an effect a primitive declared its way out of, and nothing in the type
system says a registered primitive told the truth about its own signature. A type system is a
claim about composition, not an audit of the host's registrations.

So the guarantee lives in two mechanisms, and the scope is the primary one:

- **A closed prelude.** The script is evaluated with a fixed prefix binding fifteen named pure
  modules — [`PRELUDE_MODULES`](../../yidam/cli/src/gluon_arm/mod.rs#L74-L77) — and any macro
  invocation *in the script* is refused lexically by
  [`refuse_macros`](../../yidam/cli/src/gluon_arm/entry.rs#L44-L46). Since gluon reaches a native
  module only through `import!`, a script that cannot invoke a macro can only use the names the
  prelude bound. **Closed by default:** a module nobody listed is an undefined variable, so a
  module a future gluon registers is out of reach on the day it is added rather than on the day
  somebody remembers to deny it.
- **The entry point's typecheck**, which still refuses every effect routed through `IO`, and which
  is what catches a script that is wrong about its own shape rather than about its effects. It is
  checked by the CLI and not taken on trust from the script: the host resolves the declared
  signature and reports the type it found beside the type it wanted.

A gluon calculator that so much as names `std.fs` still does not compile — but for the first
reason, not the second. [`EXCLUDED`](../../yidam/cli/src/gluon_arm/mod.rs#L85-L89) records each
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

Serde-untagged on [`run`](../../yidam/cli/src/cmd/run/manifest.rs#L172-L178), so the sequence form
parses byte-identically to today and no existing manifest changes. The table form takes exactly
one key, and an unknown key is refused at load with the arms named — the manifest's
`deny_unknown_fields` rule applied to a field that now has a shape rather than a type.

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
[`unrunnable_because`](../../yidam/cli/src/cmd/run/manifest.rs#L121-L135) already does for a
connector and a featurizer.

That is a third case on an existing mechanism rather than a new one, with one difference worth
stating: a connector's refusal is a decision and a featurizer's is an absence, and this one is
neither — it is a property of the build in hand. So the sentence names the feature, and the
manifest still **parses** in a light build, because a corpus that declares a gluon calculator is
not a malformed corpus. Refusing to parse would make `yidam lint` and `yidam graph` fail on a
repository whose only sin is a capability this binary cannot invoke.

`--features full` gains it. The default set does not, and `tests/light_build.rs` is where that is
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

## Migration & compatibility

Nothing migrates. An existing manifest's `run = ["sh", …]` parses as it does today, and a
derived repository that never writes the table form never links the engine. The tooling layer
bumps; no template version does, beyond the two documents that gain the field's second shape.

Adoption is: install a build with the feature, write the `.glu`, change one line of the
manifest. The reverse is the same move — a corpus that abandons the arm rewrites the script in
`sh` and changes the line back, and its committed computed files are unaffected either way,
because the arm changes how a number is computed and not what the computed file looks like.

The one thing that is not backward-compatible is a **corpus** carrying a gluon arm and a
**consumer** on the released light binary. That is the case `unrunnable_because` answers, and the
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
2. **Sliced or whole.** Whether the marshalled corpus is cut to the declared `reads` or always
   whole. It must be answered identically here and in #1080, and whichever way it goes,
   `guidelines/directories.md` states it beside the field.
3. **The budget's value, and whether a corpus may raise it.** A call budget a declaration can
   set is a knob that makes a hanging run a corpus's own decision; a fixed one is a number in
   the binary that one repository's judgement imposes on every other — the sentence RFC-0024
   quotes four times.
4. **What the receipt records about the script.** The shell arm gets this for free: the script
   is a file under `reads`, so editing a calculator makes its step stale. A marshalled arm reads
   no tree, so the script's own hash has to reach the input state some other way, or editing a
   calculator silently leaves its answer standing.
5. **Whether a typechecked-but-failing script is a different refusal.** A script that typechecks
   and then returns a `Computed` the corpus cannot accept is a step failure, and a script that
   does not typecheck is arguably a manifest failure — found before anything runs, like a cycle
   in `after`. There are now *three* refusals to place, not two: the macro scan refuses before
   any gluon runs, an unbound name is refused by the typechecker although it is a scope failure
   rather than a type failure, and a wrong shape is a type failure. A reader deciding which are
   manifest failures should know that the middle one reads as a type error in the output today.

   If they are manifest failures, `yidam lint` could typecheck every declared gluon calculator
   without invoking one — but it would have to build the prelude *the same way the runner does*,
   from the same [`PRELUDE_MODULES`](../../yidam/cli/src/gluon_arm/mod.rs#L74-L77), or lint and
   run disagree about what a calculator may say. A second opinion about a script's vocabulary is
   worse than no gate: it either passes something `run` refuses, or refuses something `run` would
   have accepted. That is a gate this RFC has not designed, and the constraint on it is the one
   this paragraph states.
6. **Upstream health.** Three years dormant, then two releases in two months. One maintainer's
   renewed attention is not a maintenance guarantee, and an embedded language is harder to
   replace than a policy engine. Vendoring is not an answer at this closure size.
