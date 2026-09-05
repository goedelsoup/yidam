# RFC-0029 — A write is a capability a server declares, not a transport it happens to have (the MCP write tier)

- **Status:** Draft
- **Track:** I24
- **Relates to:** RFC-0005 (the frozen contract this changes on the record; `tools.json` is the
  canonical list and this RFC's build bumps it), RFC-0020 (whose recorded decline of an MCP
  `propose` tool this amends rather than overrides — see §7), RFC-0026 (whose open question 2 this
  answers, and whose run invariant is what makes answering it safe), RFC-0027 (the projection this
  must not collide with on a contract version, and the corrected form of the #426 constraint),
  RFC-0017 / RFC-0018 (the precedent that a contract change is one RFC plus a `tools.json` bump)
- **Versioning layers touched:** **none in this PR — this is the decision, not the build.** Once
  accepted, the build under this RFC touches SDK+parity (`mcp/tools.json` gains the `act` tier and
  capability; new conformance cases) and tooling (the Rust CLI implements it). See §5 for the
  version this change takes and the ordering with RFC-0027's `profiles` change.
- **Parent epic:** #460 — this RFC is the **decision half of #474**, detached from its build half
  (#472 → #473 order it; nothing here waits on them). It also unblocks **#576** (§3) and gates
  **#429**'s consolidation (§2).
- **Downstream reference case:** none yet — `examples/streamflow`, by construction, when the build
  lands.

## Summary

RFC-0005 froze thirteen MCP tool names and every one of them reads. #474 states the consequence:
*the agent that can be told a source expired cannot do the one thing that discharges it, and the
layer is worth a great deal less behind a shell prompt.* Adding the first write-capable tool is a
contract change, not an addition, and #474's own instruction is that it be argued on the record
before code. This RFC is that argument. It decides four things and coordinates a fifth:

> A write-capable tool lives in the **same tier mechanism** the contract already has — a
> capability-named tier, refused in the frozen `capability-not-supported` shape — under **two
> declaration rules the read tiers do not carry**: the capability is **opt-in, never inferred**
> from what the server could do, and it is **declarable only where a git author identity exists**
> — stdio/local today; HTTP only after #427 supplies an author.

The second rule is the sentence the record has been missing. It makes this RFC **compose with**
epic #420's D2 (*"propose opens only once an author exists, so #429 stays blocked"*) instead of
silently overriding it: D2's phase boundary becomes a declaration rule of the contract itself, and
#427 is recorded as the shared prerequisite of #429 and this RFC's HTTP arm.

Every recommendation here is reversible at review, and each section says what reversing it costs.

## 1 — The venue, corrected

#474's definition of done says the decision *"lands in RFC-0025 (or an amendment)"*. RFC-0025 is
the quality-surface RFC — *the instrument, turned around* — and it declares **no MCP contract
change**. The number is a fossil: #470's body planned the orchestrator RFC as 0025, and it landed
as RFC-0026, which also declares *"no MCP contract change in this RFC"* and carries the tier
question as its open question 2. RFC-0020's alternatives run the other way — it declines an MCP
`propose` tool outright, with reasoning. So the amendment set was ambiguous across four documents,
the issue thread where the argument was supposed to happen has zero comments, and #576 declined a
CLI-only fallback specifically to wait on a decision nobody owned.

The precedent for where a contract change lands is RFC-0017 and RFC-0018: **one RFC per contract
change, plus a `tools.json` bump** — never an edit to RFC-0005's prose, which has not tracked the
contract's growth from four tools to thirteen and is not the canonical list (`tools.json` says of
itself: *"this file is the only place the list lives"*). The decision therefore lands **here**, and
the four documents it touches are amended with dated blocks in the same change (§7), so nothing is
overridden off the record.

## 2 — The decisions

### 2.1 Tier placement: the same mechanism, with opt-in declaration

**The question** (#474, verbatim): does a write-capable tool live in the same tier, gated by a
declared server capability — the pattern `refuse_unbacked` in `cmd/serve/tools.rs` already
implements — or in a separate tier a server opts into?

**The case for a separate tier** is that writes differ in kind, not degree. A read tier's absence
is a statement of *ability* — a projected mirror carries no live git refs, so it cannot back
`neighbors`-over-HEAD and says so. A write tier's absence is a statement of *permission*: the
deployment will not, which is a policy, and folding policy into an ability vocabulary invites a
server to "discover" it can write. A separate mechanism would make the difference structurally
impossible to blur.

**The case for the same tier** is that the mechanism is already exactly right and a second one is
the failure this contract exists to close. `tools.json`'s tiers *are* capability names; `backs()`
checks the declaration; `refuse_unbacked` emits the frozen token — *"capability-not-supported:
`{name}` is served only by a server declaring the `{tier}` capability"* — and #474 itself says
that shape *"should be reused rather than re-invented."* A separate tier needs a second list, a
second refusal, and a second agreement check between flag and tool list; a second freeze is how
three servers ended up sharing one name out of five capabilities.

**Decision: the same tier mechanism, and the difference in kind becomes a declaration rule rather
than a second structure.** The contract gains one tier — recommended name **`act`** — that behaves
like every other tier on the wire (declared in the capability block, absent from `tools/list` when
undeclared, refused in the frozen shape on call) and differs in exactly one place: **an `act`
declaration is opt-in configuration, never inferred.** Every existing tier is declared true when
the server can back it; `act` is declared true only when the deployment explicitly says so, and a
server that could write but was not told to declares false. The separate-tier option's real
content — writes are permission, not ability — is kept; its cost — a second mechanism — is not.

Reversing this at review means specifying the second mechanism: its refusal token, its
declaration syntax, and its conformance harness, none of which exist today.

### 2.2 The identity gate: composing with D2 instead of overriding it

Nothing in #474's body says who authors an MCP-triggered epistemic commit. Epic #420's D2 answers
the neighbouring question for HTTP — *read-only through Phase 2; `propose` opens only once an
author exists, so #429 stays blocked* — and #427's note sharpens it: whichever auth shape is
chosen *"has to answer what identity reaches `propose`, not merely who may connect."* Meanwhile
#460 records that #429 *"overlaps #474 and should be resolved with it rather than twice."* The
record holds both positions and no sentence joining them; `serve-http` ships in the default
feature set, and a hand-added claude.ai connector offers auth **"None"** — so a write tier landed
without the joining sentence becomes URL-reachable with no author, and the git history that *is*
the knowledge graph records what a process did rather than who decided it.

**Decision — the composition sentence:**

> The `act` capability is declarable only where a git author identity exists. Over stdio that
> identity exists today: the server is a subprocess of a person's shell inside their checkout, and
> `propose`'s author/committer split (RFC-0020, RFC-0026 §5 — *"the tool drafted and a person ran
> it"*) already records them. Over HTTP no author exists until #427 lands in a shape that yields a
> **stable subject claim** mapped onto a committer identity — so an HTTP server MUST NOT declare
> `act` until then, and **#427 is the shared prerequisite of #429 and this RFC's HTTP arm.**

This is D2 made contract rather than phase: instead of one epic holding the write path closed by
sequencing, the contract holds it closed by declaration rule, on every transport, for every
server, including ones this repository did not write. #429 then stops being a second answer to the
same question — it becomes the HTTP instance of this rule, resolved with #474 as #460 already
intended. What this deliberately does **not** decide is the shape of the remote author itself
(registry row vs verified subject claim); that argument opens when #420's Phase 3 is actually
taken up, and it belongs to #427.

Reversing this at review means either opening HTTP writes with the server as author — which #429
rules out on the system's own terms (*"authorship is not metadata in this system"*) — or
re-blocking the whole tier on #427, which parks stdio-local writes on a question they do not have.

### 2.3 `cycle` joins the `act` tier

#576's `yidam cycle` is a read-only report — *"cycle itself authors nothing at all — it reports"*
— and it is blocked on this decision deliberately, because a cycle report shipped as an MCP tool
today would be *"a fourteenth read-only tool telling an agent what to do next while it still
cannot act."* #576 forswore a competing answer, but a narrow settlement of the write question
alone would still leave nobody having said where a read-only report **addressed to acting agents**
sits.

**Decision: `cycle` joins whichever tier the write tools receive — the `act` tier as recommended
above.** The tier's meaning is thereby *addressed to an agent that can act here*, not *performs a
write*: a server that declares `act` serves the actions and the report that orients them; one that
does not serves neither, and an agent never reads "here is your next act" from a surface where it
cannot act. This is recorded here precisely so that this RFC's settlement **mechanically unblocks
#576** — no residual placement call remains for that issue to make or renounce.

Reversing this at review — `cycle` into a read tier — is cheap and coherent (it reads), at the
cost of reopening #474's complaint one tool later.

### 2.4 The #426 constraint, restated in its corrected form

#474 and RFC-0026's open question 2 both carry the constraint in its retracted form: the ChatGPT
connector requires (or "wants") two specific tool names against RFC-0005's thirteen, and the write
tier must not make that worse. RFC-0027 corrected the premise — *"the exclusivity was mine, not
the vendor's"* — and replaced it with the mechanism this RFC inherits: **a profile is a projection
of the canonical list**, serving exactly what it names and refusing canonical names with
`not-in-profile`.

Restated under the correction, the constraint discharges rather than binds: the `openai` profile
projects `search` and `fetch` and nothing else, both read-only by the vendor's own requirement, so
a canonical list that grows an `act` tier changes **nothing the profile serves** — the projection
simply does not name the new tools, exactly as it does not name eleven of the current thirteen. A
write tier plausibly cannot worsen #426 at all. The clause survives only as a conformance
obligation the build inherits: a profile case asserting the projection is unchanged by the tier's
presence, and that a profiled server refuses `act` tools with `not-in-profile` like any other
unprojected canonical name.

### 2.5 Contract-version ordering

RFC-0027's migration section reserves `0.12.0 → 0.13.0` for the `profiles` addition. That version
is gone: commit 8b49753, three hours after the RFC landed, consumed 0.13.0 for the handshake
corpus block, and cli/v0.9.0 released it. Live `tools.json` reads contract 0.13.0 with no
`profiles` key. Three writers to one frozen contract — banner shipped, profile pending, write tier
pending — and no recorded ordering is how the collision happened.

**Decision, in two parts.** First, the rule that prevents the recurrence: **a contract version is
claimed at landing, never reserved in prose.** An RFC's migration section names the *kind* of bump
(minor, additive) and takes its number from wherever `tools.json` stands on the day it merges.
Second, the expected ordering onto the next free versions: **RFC-0027's `profiles` change lands
first, at 0.14.0; this RFC's `act` tier lands second, at 0.15.0.** The profiles change is further
along — RFC-0027 is written and gated only on the one question that needs a live connector (#428)
— while this RFC's build follows its acceptance and #460's build track. Both are additive minors.
If the landing order inverts, the numbers swap under the claim-at-landing rule and nothing else
changes; what is binding is that they are **two separate minors, never one** — two contract
changes sharing a version is two changes sharing a rollback.

RFC-0027 receives a dated coordination note to this effect (§7).

## 3 — What the run invariant contributes

The reason a write tier is arguable *now*, when RFC-0020 declined it, is RFC-0026:

> A run authors **operational** commits directly. Every **epistemic** commit it produces goes to a
> proposal branch, and nothing merges itself.

— and the invariant is mechanically testable: classify every commit a run wrote by leading verb
and assert the epistemic ones are all on a `propose/*` ref, with `classify_commit` a parity
function fixtured in three SDKs. #474's DoD already demands this held over MCP exactly as over the
CLI. So the property RFC-0020's decline protected — a person reading the branch — is no longer
enforced by keeping the transport a shell; it is enforced below the transport, on the commits
themselves, whoever triggered them. §7 records this as an amendment to RFC-0020, not an override.

## 4 — What this does not touch

- **No code.** This RFC is the argument #474 said must happen before code. The build — the `act`
  tier in `tools.json`, the CLI dispatch, the conformance cases, the end-to-end streamflow
  demonstration, the over-MCP invariant test — lands under this RFC once it is accepted, as
  #474's build half, in #460's build order.
- **No `tools.json` change in this PR.** The canonical list changes on the record when the build
  lands (§2.5 for the version it takes). A frozen contract that quietly grew is worse than one
  that changed on the record — and one that changed in the argument's PR is the same defect from
  the other side.
- **The remote author's shape.** Registry row or verified subject claim, and what committer
  identity a wire request yields — #427's question, opened when Phase 3 is taken up (§2.2).
- **The thirteen read tools, the profiles mechanism, the node model, synthesis.** Unchanged.
  `cmd/sangha.rs` stays read-only and Article V is not in play: an `act` tool runs strictly below
  resolution, exactly as `propose` does.

## 5 — Migration & compatibility

When the build lands (not now):

- **Parity layer.** `mcp/tools.json` gains the `act` tier and capability; contract takes the next
  free minor at landing (expected 0.15.0, after `profiles` at 0.14.0 — §2.5). New conformance
  cases: an `act`-declaring server passes the act cases; a non-declaring server refuses in the
  frozen shape and has them skipped; capability flag and tool list checked for agreement — all of
  it the existing harness, pointed at one more tier.
- **Rust CLI.** stdio `serve --mcp` may declare `act` behind explicit configuration; the HTTP
  server refuses to declare it until #427's author exists (a startup error, not a silent
  downgrade).
- **TS and Python servers, derived repositories.** Unaffected until they opt in. A server that
  declares nothing serves the thirteen read tools it serves today; the change is additive for
  every existing consumer.

## 6 — Alternatives considered

- **A separate opt-in tier mechanism.** Argued in §2.1 and declined: its real content survives as
  the opt-in declaration rule; its structural cost (second list, second refusal token, second
  agreement check) does not.
- **Gate the write tier on transport instead of identity** — stdio may write, HTTP may not, full
  stop. Simpler, and wrong twice: it hard-codes today's accident (stdio implies a local person)
  into the contract, and it forecloses #429 permanently instead of gating it on #427. The identity
  gate subsumes it: today the two rules license identical surfaces; they diverge exactly when #427
  lands, which is the point.
- **Wait for #427 before deciding anything.** That parks stdio-local writes — `due` discharged by
  the agent that read it, #576's whole epic — on an OAuth question they do not have. D3 parked
  #427 on its own merits; inheriting that park here would let the deferred question gate the
  undeferred one.
- **Decide it in RFC-0026 as an amendment.** RFC-0026 declares no MCP contract change and its
  scope is what a run may author, not what a server may serve; the RFC-0017/0018 precedent is one
  RFC per contract change. RFC-0026 gets the dated answered-here note instead (§7).

## 7 — The amendment set

Landed with this RFC, as dated blocks in the named files, so the record changes where it stands
rather than by supersession at a distance:

- **RFC-0005** — a dated pointer block: `tools.json` is the canonical list; the contract grew on
  the record through RFC-0017, RFC-0018, RFC-0019 and the issue-level bumps the parity README
  documents; RFC-0027 (profiles) and this RFC (the `act` tier) extend it next.
- **RFC-0020** — its decline of an MCP `propose` tool is **amended, not overridden**: the decline
  reasoned from *"a tool proposing to a tool, and the whole design turns on a person reading the
  branch"*, and the person reading the branch is preserved — RFC-0026's invariant enforces it on
  the commits rather than at the transport, and §2.2's gate ensures the tool call has an author.
  What lapses is only the inference that the transport must therefore stay shell-only.
- **RFC-0026** — open question 2 is answered here; a dated note says so and points at this RFC,
  including the discharge of its #426 clause (§2.4).
- **RFC-0027** — a dated version-coordination note: 0.13.0 was consumed by 8b49753 and released
  in cli/v0.9.0; `profiles` takes the next free minor at landing (expected 0.14.0), under §2.5's
  claim-at-landing rule.

#474's DoD correction ("lands in RFC-0025" → lands here) and the #427/#429 composition
cross-references are tracker comments, posted when this RFC is accepted for review — they change
issue text, which this repository amends by comment, not by RFC.

## Open questions

- **The tier's name.** `act` is recommended because §2.3 puts a read-only report in it — a tier
  named `write` holding a tool that only reads would misdescribe itself. `run` collides with the
  orchestrator's noun. Reversible at review with no downstream cost until the build lands.
- **One `act` tier or two?** If a future surface wants *plan* separated from *perform* — an agent
  that may draft proposals but not trigger runs — the single tier splits. Left open because
  nothing yet needs it settled, and an additive split is a minor under §2.5's rule.
- **What explicit configuration declares `act`?** A `[serve]` key, a flag, or the capability
  manifest RFC-0026 introduces (`.yidam/capabilities.toml`) — the build decides under this RFC,
  with the constraint from §2.1 that it is configuration, never inference.
