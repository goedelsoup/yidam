# Directory Conventions — Evidence

Why each rule in [directories.md](directories.md) says what it says: the incident that produced
it, the measurement that set its threshold, and the failure it was built against. One section per
rule, reached by the `[why]` link beside it.

**This file is not part of the recurring read.** Nothing here is needed in order to comply with a
rule. Read a section when a rule surprises you, when you are about to argue with one, or when you
need to know how far a number can be pushed and on what evidence.

---

## created-on-first-use

An empty directory holding only a README that describes what it would contain is
indistinguishable from an abandoned one. That is the argument for deferral, not a claim that
these directories go unused — measured across fifteen derived repositories, `agents/` received 11
domain agents across 4 repositories and `docs/` received 53 files across 6; only `packages/`
stayed empty in 14 of 15. It is the same argument `sadhana/skills/README.md` makes for skills: a
repeatable need emerges from inquiry, not preemptively.

## committed-fixtures

A connector's offline fixture is a record of what a source said when it was asked. Git's
line-ending normalization rewrites that record on checkout, and for most fixtures nobody notices.
For some, the line endings *are* the property under test — a bulk export served with classic-Mac
`CR` endings, a register that serves `CRLF` — and normalizing them silently deletes the thing the
fixture exists to pin. The test then passes against what git produced rather than against what
the source served, which is the failure mode a hermetic fixture was supposed to prevent. Found in
a derived repository by a commit whose message says it exactly: the fixtures were being
normalised, so the committed record was of what git did rather than what the register served.

## check-diff-is-quiet

Across the three repositories derived from this template the median code-touching commit
introduces no new type at all, and it is the commit that lands ten that you want to be asked
about. Where it is *not* quiet, the vocabulary is what to look at: one of those repositories
declares fifteen classes while its code defines two hundred and seventy-five types, and a 7%
match rate is not a naming problem. The ontology is a contract every other check reads from the
corpus side; this is the one that reads it from the code side.

## gated-prose-numbers

Every node is checked and for a long time nothing checked the account of them — which is how a
document here ends up saying the corpus has 45 nodes across 4 classes when it has 84 across 13,
in a sentence that was true when it was written.

## drift-not-falsehood

State the limit where you build the check, because a reader who believes it catches more than it
does will stop reading. A test over the working tree answers questions about the tree. "These
four nodes have been rewritten" is a sentence about what somebody did, and two trees — one where
the work happened and one where it did not — can be identical in every respect the test can see.

## a-directory-not-named

The worked case is a repository that took on an advocacy purpose and needed somewhere to *argue*,
which the corpus forbids by construction. Putting argument in the corpus would have meant
relaxing the rule against asserting intent. It created a `dossier/` alongside the corpus instead,
with its own gate crate checking every assertion there back against the corpus claims beneath it,
and the corpus's semantics were untouched. The new purpose made the evidentiary rules *harder*,
because the output became public.

## catalog-is-indexed

The walk is not incidental: in a derived repository the catalog was 51.3% of the indexable text
against the corpus's 41.9%, and for a long time none of it was walked — so "what is searchable"
was being decided by a tool boundary rather than by anyone. The directory-level switch is
deliberate in the other direction: a rule somebody has to remember per file is a rule that gets
forgotten.

## obtained-means-fetched

A derived repository audited all 23 of its entries and found three such documents — one located,
fetched, cited by nothing, summarized nowhere — while the `[open]` claims those documents answered
had been carried for months. Nothing detects this: the flag is true, the entry is well-formed, and
every check passes. Of the two sentences the rule asks for, the second is the useful one; it is
the only thing that distinguishes an unread source from a read one.

## ttl-days-days-not-commits

Every other clock in yidam counts commits, because a corpus-state finding must be a function of
`HEAD` rather than of when you ran the report. This one is different in kind: a statute does not
become stale because you committed, and a gauge record does not stay fresh because you did not.
Pearl 2009 will say what it says in ten years, and an inventory pulled from an API will not —
which is why the interval is per entry.

Without `retrieved`, the date is read from the commit that last touched the entry's file, which
counts a typo fix as a refresh, so it errs in the flattering direction. Expiry does not claim the
source changed: nothing here reads upstream and `doctor` does no network. Refreshing a source is
a knowledge event, and it is yours to own.

## artifacts-demonstrable

The record-and-store split is the whole design. A stale vault cannot lie, because the digest is
in the commit; and losing a vault costs no knowledge claim, only the time to re-fetch. Adopting
the field is a corpus deciding to record what it holds, not a requirement arriving in a build.

## vault-route-override

The specific assertion outranks the general one, because the general one is a config file that
somebody reorganising storage edits without reading every entry it governs. `none` is spelled
rather than omitted so that *nobody has decided* and *decided to keep it here* are different
states — an omission cannot be reviewed. An artifact whose *kind* no vault claims is reported by
`vault push` and `doctor` rather than by lint, because the defect is in the config and blaming the
catalog entry would point at the wrong file.

## redistributable-separate

A route is edited casually — somebody reorganising storage moves a dozen entries between stores
in an afternoon — and a licence is not something that edit is allowed to undo. Keeping it separate
means the reorganisation meets a refusal instead of publishing a paper.

## bytes-present-is-per-machine

Every check in `yidam lint` reads the working tree and nothing else. A gate whose verdict depended
on which machine ran it is one a corpus could not reason about: the same `HEAD` would be both
clean and broken, and neither answer could be cited. Presence is a real question, and it belongs
to a command run per machine rather than to a gate run per commit.

## a-citation-is-a-link

The checks used to match the bare slug anywhere in a node's bytes, so a node that merely mentioned
a source was reported as citing it. Under `catalog-unobtained-but-cited`, which is Error severity
and gates, that failed a build on a node containing no citation at all. The collision that
surfaced it is not exotic: these conventions recommend naming connectors after what they fetch
(`nwis`, `echo`, `census`), and a catalog entry for the source those connectors fetch from carries
the same slug by design. Any node discussing the crate tripped the check.

## max-lines-measures-prose

The prose and not the file, because a node that records where each of its edges comes from — a
`claim_tag` and a `source` per link — should not pay for that provenance out of a budget written
to stop prose sprawling. Nor should a corpus that keeps its prose in a `summary` and a `findings`
be measured on a third of what it wrote.

That no class carries a default is measured rather than timid. The bootstrap rubric caps a node at
40 lines, and across five real corpora 335 of 410 nodes exceed it — 86%, 86% and 97% in the three
mature ones — while the same corpora at their genesis commits run to a median of 35, where 40 is
right for three of four. So 40 is a *genesis* norm that a corpus grows out of, and growing out of
it is what a corpus doing its job looks like. There is no knee in the distribution to put a
steady-state number at; it runs smoothly from 20 to 534.

## implemented-by

That a class omitting the field is not checked at all is measured rather than timid — across
twelve derived corpora 129 of 157 declared classes have no type bearing their name, and matching
traits, aliases and every language in the tree makes it worse, 165 of 186. Five of those corpora
match nothing at all, and they are not behind: an ontology models a domain while `crates/` models
the pipeline that gathers evidence about it, so a class without a type is the ordinary case. What
makes the declared case gate is that the class stated a fact about `crates/`, and a missing type
contradicts it rather than merely omitting something.

## tonpa-install-exits-quietly

mise logs a failing `postinstall` hook as a warning and exits 0 regardless. A green `mise install`
is therefore consistent with no dependency having arrived at all, and the next command to read a
missing corpus reports a corpus that is missing rather than an install that failed — which sends
the reader to the wrong file. Running the task on its own and reading its exit code is the only
form that distinguishes the two.

## tonpa-ignore-unsettled

Both answers are defensible and neither has been tested against a real dependency. Committing
`.yidam/tonpa/` makes a clone reproducible without a network and makes the dependency's text
citable by line from this repository's own history; ignoring it keeps one corpus out of another's
history and leaves `tonpa.lock` as the only record, which is what a lock file is for.

What is measurable today is that the question has not arisen: of the derived repositories
surveyed, none declares a dependency — no `.yidam/tonpa.toml` exists anywhere, so nothing has
ever been unpacked under `.yidam/tonpa/` and nothing has ever been staged from it. The gitignore
template's own organising principle is a list of things that appear in a working tree without
anyone putting them there, in a repository that stages indiscriminately by protocol, and a fetched
bundle fits that description. It is not listed. Recording the omission is more honest than
resolving it from the template side, because the first repository to install a dependency is the
one that will have the evidence.

## refs-hold-the-corpus-not-the-argument

This directory listed protocol documents only for the whole of the template's early life, on the
reasoning that a position is a branch and a branch is a ref. That is right about which nodes an
elector holds and wrong about why they hold them — and a resolution turns on the why. Once the
resolution merges, an unwritten argument is gone into the merge base, and Articles III and IV have
nothing left to be satisfied by. A derived repository accumulated 24 position files across 12
resolutions before the conventions had a slot for them.

## authorship-and-residence-are-separate

Conflating them cost the same derived repository four corpus nodes whose citations resolved for
their author and for nobody else, plus two resolutions standing on the baseline whose arguments
were not.

## private-paths-why-a-file

A repository whose privacy is load-bearing usually has that fact written in a decision record and
nowhere else, which makes it an assumption: true, relied upon, and unenforced. An assumption about
access control that looks enforced and is not is worse than one everybody knows is manual, because
nobody checks the second kind by hand.

## derived-artifacts-inherit-privacy

An index is not a file that happens to sit next to the corpus. The reason the refusal is the same
rule a bundle gets is that both cross the same line: *the artifact outlives the access*. A push
that succeeded once cannot be recalled by tightening the declaration afterwards, so the check has
to be at the push.

## egress-is-not-covered

A repository designing a search feature found the other half:

> Every verification in the spike concerns inbound access — that a stranger cannot reach the
> endpoint. None asks what the endpoint does with what it receives. […] That machinery is all
> pointed at people arriving. This channel is the site departing.

The case was a search box forwarding query text to a third-party encoder. Nothing in the corpus
left the repository; the *queries* did, and for a research corpus the queries are the research
agenda — a plainly-worded list of what is being investigated and about whom. No gate here looks at
that, and none can: an egress check would have to know every network call the domain computer
makes, and CI is hermetic precisely so that it makes none.

Apply to a channel the rule the private-paths gate applies to access: **unknown is not proof of
protected.** A comment asserting that an exposure does not exist is worse than no comment, because
it stops the next reader from looking.

## policy-local-rule-decides

A layer whose purpose is to stop the binary imposing its judgement cannot reserve the interesting
half of every judgement to the binary. Permitting what the default refused is the case that makes
the layer worth having; a layer that could only tighten would be a config field with extra syntax.

## policy-nothing-is-quiet

The rule `.yidam/private-paths` states about itself applies here too: an assumption about access
control that looks enforced and is not is worse than one everybody knows is manual. So an override
is loud in four places and gates in none of them — the repository decided, and what the tooling
owes it is visibility rather than a veto.

## bin-beside-the-pin

On a machine with two yidam repositories, a machine-wide install is last-writer-wins: repository A
builds its pin, repository B builds its own, and A now runs a binary that does not match its own
vendored prelude with nothing anywhere saying so. The guarantee the build task exists to keep —
that the binary and the prelude agree — held for exactly one repository at a time.

It was not theoretical. Three separate builds displaced the machine-wide binary during the session
that fixed this, one of them with a copy predating the `--format` flag entirely, which would have
left every JSON report unreadable.

## bin-invocation-shadowing

The rule above is about where a binary gets *written*. The same hazard arrives from where one gets
*found*: a `yidam` left in `~/.cargo/bin` by any earlier install — including one predating the
`--root` convention — shadows `.yidam/bin/yidam` for any process whose `PATH` puts cargo's
directory first. That is not a mistake anyone made; it is what a shell sourcing a Rust environment
does by default.

## bin-in-mise-toml-not-yidam

`[env]` in a mise task file declares a task named `env`, and `_.path` is an unknown field, which
orphans every task in the file. The paragraph in `directories.md` asserted the opposite for as
long as the declaration sat in the file that could not hold it, so the guarantee it named was one
nothing delivered.

## bin-quiet-failure

An older binary lacking a subcommand exits with `unrecognized subcommand 'regen'`, and inside a
script with output redirected — which is how a regen step is usually written — that is
indistinguishable from success. `regen --check` is a real backstop, but it fires on the next full
run; between the no-op and that run the repository holds a stale generated block and the command
that refreshes it reports nothing wrong.

## capabilities-provenance-invented

A commit saying `compute: low-flow through August`, written by a person after invoking a
calculator by hand, is an account written after the fact and checkable against nothing. The
manifest is what makes the same subject line a claim something verified.

## reads-writes-load-bearing

Both are decidable before the step runs rather than after it has produced a tree. A check applied
afterwards has to decide what to do with output already written; a check applied to the
declaration refuses with nothing committed.

## capability-declares-its-own-implementation

It is not a formality. The step stands in the scratch tree and nowhere else, so reading the script
there is what puts it in the input state — and a calculator whose script was not declared would
compute a new answer while its receipt said nothing had changed.

## after-epistemic

What a step declaring an epistemic verb writes lands on a proposal branch and is not in the tree a
dependent would read, so waiting for it would mean waiting for a person to merge.

## ageing-days

A connector's input state can sit unchanged across a year in which everything it describes moved.
The interval draws the same distinction a source's TTL draws, where an expiry does not claim the
upstream changed, only that nobody has looked. There is no default for the reason every interval
in yidam is declared: a number in the tool is one repository's judgement arriving in another that
never agreed to it.

## epistemic-runs-have-no-escape-hatch

There is no route to declare in the manifest and no policy key to set, and the reason is that a
repository which could write that permission for itself could license its own runs to author
`establish:` on its own baseline — the safety argument would become a config value. Deciding that
a question is answered stays an act a person performs, and the commit vocabulary already drew that
line.

## receipt-has-no-timestamp

The absence also makes a receipt a pure function of its input state, which is what lets a re-run
over an unchanged corpus write no commit at all rather than an empty one per invocation.

## receipt-records-a-look

This is the one commit that changes no output, so the report says so in those words rather than
reporting a file as written — a report that named a file would send the reader to look for a diff
that does not exist.

## computed-output-placement

A computed quantity is a fact about a calculation, and a class property is read as a fact about
the subject. So writing a derived figure onto a node asserts, silently and for every instance,
that the figure is a measurement — and nothing in the node records the method that produced it.

## computed-declares-its-own-readability

`.yidam/computed/` was written from the day a calculator was declared and nothing read it. The
two shipped calculators emitted a `method:` block, a per-node table and a `summary:` block in one
file, which is a good file for a person and gives a reader no way to tell which part is an
assertion about a node. A reader guessing — treating any top-level list of mappings as a table —
would have read `tiers:` as eight signals about nodes named `verified`, `inference` and `open`.

So the file declares its own readability and the reader never guesses. The cost is one line per
calculator; the alternative is a heuristic that is wrong in a way nothing reports, on a directory
whose entire failure mode is being silently unread.

## computed-keyed-by-the-reference-grammar

A node had eleven string spellings in this toolkit before a grammar was written for it, and the
grammar exists because every reader had invented its own. A computed file keyed on a twelfth —
a bare stem, a class-scoped id, a path with or without `.yml` — would be a form nothing else
parses and that every later reader has to be taught.

The revision pin is the case worth stating. `gage/canyon-outlet@abc1234` parses, so accepting it
and ignoring the revision would attach a signal computed against one commit to the node as it
stands now. That is the failure the chain rule in `agent-conduct.md` exists to prevent, arriving
by a different door: an answer travelling further than what it was computed from.

## computed-signal-names-are-repository-wide

The alternative was a per-file prefix — `travel-tier.travels_as` — which resolves every collision
and invents a second name for every signal. Both spellings then exist forever: the one the
calculator emits and the one a query has to use, with the file's own name load-bearing in the
second. Renaming a calculator would rename every signal it computes.

Refusing instead makes the collision a thing somebody fixes once, in the calculator, and keeps
the name a query uses the name the calculator wrote. The refusal names both files because either
one of them is the one to change and a reader cannot tell which from a message naming one.

## authorship-why

`broken-prose-link` shipped knowing about exactly one such directory, `.yidam/.vendor/`, on a
rationale that generalizes perfectly: a defect in the prelude is fixed upstream and adopted by
re-vendoring, so reporting one here hands this repository a finding it cannot act on.

A consumer that is not a vendoring repository met the same wall from the other side. Its first
gated run produced **43 broken prose links under `docs/` at error severity, and fifteen of them
were inside a directory whose own README says it is a frozen, unmodified copy of an upstream
project at a fork point.** Editing those to satisfy a linter falsifies the record the directory
exists to keep. It baselined them and moved on, which is the wrong instrument: a baseline records
*we accept this violation*, when the truth is *this file is not ours*. The two decay differently —
a baselined entry that is later repaired fails the build, by design, so for a frozen import the
gate came to depend on nobody ever re-syncing it.

## authorship-requires-an-addressee

`generated` and `imported` are claims about where material came from, and the whole weight of the
mechanism is that they still report. A finding that says whose defect it is is worth more than one
that says nothing — the generated half in the case above was the same defect twice, a generator
emitting corpus-relative link targets into a file that landed in a sibling directory.

## authorship-region-stale

A manifest permitted to be wrong drifts exactly as a lint baseline does. It is Warn rather than
Error for the reason the file exists at all: an imported region is re-synced upstream on somebody
else's schedule, and gating on it would make this build's colour depend on that.

## vendor-excludes-cli-and-harness

A vendored copy of the CLI is a fork that will never be rebuilt — the `yidam` binary is installed
from the pinned origin (`mise run yidam-build`), not compiled from a snapshot. A vendored copy of
the harness brings `HARNESS.md`, whose links point at scenario files the derived repo does not
have.

## prelude-domains-not-wholesale

Bootstrap keeps only the libraries a calculator named — none, in the common case — so a wholesale
copy would restore all fifteen on every update, silently reversing a choice made at genesis.

## mise-yidam-toml-is-inherited

It was omitted from the update originally on the reasoning that the update should touch nothing
outside `.yidam/` — which left it with no update path at all. A derived repository froze the copy
it was born with permanently, including a prescribed commit verb this project later found its own
lint rejects.

## staleness-reported-from-outside

`GRAPH.md` is the escalation because that is where the closed commit vocabulary lives, it is the
part of the prelude this repository has already written history against, and history cannot be
rewritten to match a verb. Neither check gates because a build that goes red because upstream
moved is a build somebody switches off.

## sending-a-finding-back

The cost of having no return path is not hypothetical. On one day, upstream added a verb to the
closed vocabulary, citing a derived repository's use of it as the evidence the vocabulary had a
gap; that same repository spent four commits the same day removing the verb, on the correct
reasoning that no gap existed in the prelude *it* could see. Both were right about their own
evidence. Neither could see the other, and the derived repo ended further from upstream than it
started.
