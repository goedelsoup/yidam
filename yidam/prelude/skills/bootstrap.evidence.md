# Skill: bootstrap — evidence

Why each instruction in [bootstrap.md](bootstrap.md) says what it says: the incident that
produced it, the measurement that set its threshold, and the failure it was built against.

**This file is not a step.** Nothing in it is needed to execute the skill, and a bootstrapping
agent that never opens it has skipped nothing. Each section is named by a `[why]` link in the
skill, and following one is a named read — the skill's step 1 says which reads are permitted
and this is one of them. Read a section when an instruction seems wrong for the repository in
front of you, when you are about to deviate from one, or when you are changing the skill.

The skill is the one prelude document that is executed rather than consulted, so its arguments
were written at the point where the action they argue against is most tempting — the fabrication
argument sits where the empty class is, the edge argument where the whole corpus is in view. Each
instruction in the skill keeps the one sentence of its argument that does the work at that
moment; the rest is here.

## glossary-first

*rigpa*, *ma*, *samudaya*, *sadhana*, *kuten*, *tonpa* and *sangha* are each load-bearing in the
six files below the glossary and defined in none of them. `SCRIPTURE.md` defines them too, but as
continuous argument rather than a table — you read it through to find a word. That is why
scripture stays out of the list and the glossary stands in for it there, and why the glossary
links scripture for the reasoning it does not carry itself. An agent that read the six without
the glossary scaffolded a `rigpa/`-aware repository having never been told what rigpa meant.

## named-reads-are-not-wandering

The ban on enumeration is on wandering — on listing a directory to see what turns up and
reading whatever does — and a step that names the path and the field before it reads is not
wandering. Step 5 had directed its listing of `yidam/prelude/domains/` for as long as the same
paragraph forbade any listing at all, and the unqualified ban was the half that was wrong. The
same reasoning admits this file: a `[why]` link names a section, and a section is a field.

## tests-are-not-curriculum

`yidam/tests/` holds how the yidam template tests itself — the harness, the rubric, the judge's
criteria, and each scenario's reference description of a good result. None of it teaches you
anything about the domain you are bootstrapping, and the criteria you would be scored against
are not criteria you should be optimizing toward: an agent that has read "seed nodes at a
consistent level of abstraction" will assert consistency, which is not the same as achieving it.

## seed-count-default

A flat default of 13 was measured (#583, A0) against thirteen derived repositories that recorded
a `corpus_depth`: it was accepted twice in thirteen, and the median chosen was 26 — double it.
"Two per class, minimum 13" would have produced 24–36 for most of that population, which is where
people actually landed. The default is computed from the sketch rather than fixed because the
fixed number was the one nobody kept.

## single-elector-is-an-answer

"One elector, or several?" is not a choice between two modes of the same weight. The sangha is
a real protocol with real overhead, scaffolded at genesis — step 3 creates five files for it —
and a repository that names one elector but adopts it anyway has paid for machinery it does not
use. It is also the more capable-sounding option, which is why the skill says not to choose it
for that reason: a person who cannot name a second elector has already answered the question,
and the honest record of that answer is `single-elector`, adoptable later when a second elector
can actually be named.

## do-not-rank-profiles

A practice the user does not recognize as theirs is a declaration that will be diverged from on
the first commit, and divergence is the one thing the kuten layer exists to make legible. A
recommendation from the agent is the shortest route to a declaration nobody holds: the user
accepts the ranked option, the corpus records it, and every subsequent `kuten check` reports a
divergence that was manufactured at genesis.

## revision-is-copied

The revision is recorded because a kuten is read at the vintage the repository holds and never
at upstream's current one — and if the two ever disagree, after a re-vendor say, `yidam kuten`
says so in `AGENTS.md` rather than quietly picking one. A revision typed from memory is a
declaration of a vintage nobody vendored, and the disagreement it produces is between the record
and a profile that never existed.

## create-on-first-use

An empty directory with a README explaining what it would contain is indistinguishable from an
abandoned one, which is the argument for deferral — not the count of what arrived. Measured
across fifteen derived repositories, `packages/` stayed empty in 14 of 15, but `agents/` received
11 domain agents across 4 repositories and `docs/` received 53 files across 6: the deferral does
not mean these directories go unused, only that they are created the day a repeatable need for
them emerges rather than speculatively at genesis — the same argument `sadhana/skills/README.md`
already makes for skills ("Add skills when a repeatable procedure emerges from inquiry — not
preemptively").

## overwrite-do-not-merge

Yidam's copies of the six root files describe yidam — its harness, its CLI workspace, its
bootstrap-mode entry check. Left in place they are wrong the moment genesis is written, and
merged they are half wrong, which is harder to notice. `PRACTICE.md` is the exception because
the template's history is a CLI's, not a corpus's: there is nothing of yidam's in that file worth
overwriting, so it installs new.

## replace-the-workflows-directory

The instruction is a replacement rather than a merge because yidam's own workflows each name a
layout that does not survive genesis: `ci.yml` builds `yidam/cli` and `yidam/tests/harness`,
paths step 8 deletes, so it would go green having compiled nothing; `release.yml` publishes the
yidam CLI's binaries from a repository that has no CLI to publish; `docs.yml`, `editor.yml`,
`install-channels.yml`, `publish-crates.yml`, and `tap.yml` each reference a directory or a
publishing target this repository does not have. Naming and overwriting only two of them, as this
step used to, leaves the rest behind — nothing objects to correct YAML naming a path that used to
exist, and the first push a derived repository makes to a remote is the moment one of them runs
and fails (#589).

## enumerate-the-workflows

Naming the workflow files in prose is exactly the drift that left `index.yml` uninstalled for as
long as it existed alongside `ci.yml` and `release.yml`, its own header claiming an install the
step never performed. A list in the skill is a second copy of the directory listing, and the two
copies disagree the day a file is added to one of them.

## dotless-template-names

`gitattributes` and `gitignore` are spelled without their dots for the same reason `root/` and
`github/` are: `ls sadhana/` is a step in the skill and a dotfile would not appear in it.
`.gitattributes` arrives holding only comments — the rule about connector fixtures and line
endings, which costs nothing until the first connector lands and is unrecoverable advice
afterwards.

## gitignore-is-not-generic

Yidam's own `.gitignore` ignores `.local/` — where *its* binary installs — and a path under
`yidam/tests/`, which the vendor step in step 8 deletes; the rule outlives the directory it names
by the length of the repository's life. What a derived repository needs instead is organized
around a hazard it has and yidam does not: both the skill and `PROTOCOL.md` prescribe
`git add -A`, so anything that appears in the working tree without somebody putting it there is
one prescribed command away from the corpus.

## one-concept-per-class

Two ideas fused into a single class — `site-and-region`, `event-or-interval` — cannot be linked
to separately afterwards, and the edge that wanted only one of them has nowhere to land. The
"and"/"or" test is the cheapest detector: a name that needs a conjunction is naming two things.

## only-named-domains-are-vendored

A repository that names no domain gets no `domains/` directory, and that is the right outcome —
fourteen of the fifteen are wrong for any given corpus, and a library nothing can build is
indistinguishable from an abandoned one. Naming a domain is cheap and reversible; carrying all
fifteen is neither. The bar is whether a calculator in the table would call a function in it,
because "sounds adjacent" is how all fifteen get named.

## a-link-is-not-an-edge

A link to a README, to a directory, or to the class definition alone is a citation rather than a
relationship: it satisfies the count and adds no knowledge. The `instance-of` link is the case
that most often stands in for an edge, because every instance has one and it is correctly typed —
which is exactly why it cannot discharge the requirement.

## one-level-of-abstraction

A corpus whose nodes are three fields and one named specimen reads as two corpora, and the edges
between the levels carry the confusion rather than resolving it: an edge from a field to a
specimen is neither a generalization nor an instance, and nothing downstream can say which it
was meant to be.

## leave-the-class-empty

The pressure to fabricate is strongest at seeding and the reason is worth stating, because the
`corpus_depth` you are short of is a number a user picked and the empty directory looks like a
failure to meet it. A fabricated instance would not look like a placeholder. It would be
well-formed, correctly typed, correctly linked, and it would pass every check this repository
runs — `graph-check` reads structure and `edge-target-class` asks whether an edge landed on the
right class, and neither asks whether a claim is true. It would sit among the sourced nodes and
be lent credibility by every one of them.

The asymmetry decides it. An empty class is a gap visible to everyone who opens the directory,
it costs nothing but the seeding work to close, and it is closed correctly the first time someone
supplies the real material. A fabricated instance has to be *found* before it can be removed, and
until it is found the corpus asserts it.

`corpus_depth` is not revised at seeding for the same reason: it was chosen before anyone knew
what the sources covered, and the gap between the target and the seeded count is information the
record should carry, not a discrepancy to edit away.

## name-the-starved-calculator

The last clause of the seed-scope record is the one that is easy to leave out and matters most.
A calculator whose inputs are all in the empty classes is a stub for a reason that has nothing
to do with the calculator, and step 7 will not be able to tell the difference between that and a
calculator that was stubbed because its logic was unclear.

## only-edges-you-can-defend

Step 7 reads the whole corpus at once, and every pair of instances looks like it could be
related — which is the condition under which a plausible relationship gets written as a settled
one. The cost is asymmetric: a missing edge is a gap somebody finds and fills, while a wrong edge
is something the corpus now asserts, made credible by every correct edge around it. The
one-sentence test exists because it is the cheapest thing that separates the two: a relationship
you can state is one you can be wrong about, and one you cannot state is one nobody can check.

## after-genesis-not-before

The implied edges are an `establish:` — understanding the ontology entailed and nobody had
written down — and the remaining stubs are an `implement:`, because a stub is structure and not
a finding. Both come *after* `genesis:` and not before, which is the opposite of the order their
steps appear in, for the reason a root commit cannot have a parent: there is nowhere to put them.
Step 7 does the work and step 8 records it, and step 8 states the whole sequence in one place so
that no step writes a commit the sequence does not name.

## resume-from-doctor

A bootstrap interrupted before `genesis:` lands leaves a state that is silent: it looks identical
to a directory nobody has touched yet, right up until someone opens it and finds twenty files
`doctor` cannot see because there is no commit to run it against (#579). Nothing before step 8
needs to be redone on resume, since steps 2–7 only wrote files and step 8 has not yet committed
any of them.

## genesis-message-is-testimony

The genesis commit is the first event in the knowledge graph. It should read like one: a list of
filenames is a diff summary, and a paragraph that would fit any domain is boilerplate. Neither is
testimony about what the corpus now knows, and testimony is what a commit message in this
repository is for.

## vendor-exactly-one-directory

Everything under `yidam/` except `prelude/` is yidam's own machinery — the CLI source, the
bootstrap test harness, the design notes, the docs site — and none of it is readable, runnable,
or updatable from inside a derived repo. Carrying it produces a stale fork of the CLI that will
never be rebuilt and a `HARNESS.md` whose links point at scenarios the repo does not have.

## prune-the-domains

The same argument as vendoring one directory, applied one level down. A derived repository has
no task that builds these libraries, no workspace that includes them, and no CI job that runs
them; the `domain-parity` gate that keeps them honest is yidam's and does not travel. Fifteen
unbuildable libraries is the stale-fork outcome arriving through the one directory the vendor
step allows. `prelude/domains/` is around 320 of the roughly 540 files the vendor step moves, and
the majority of the bytes.

## template-root-is-enforced

The instruction to delete two files named two files while `yidam clone` was delivering eleven
more root paths nobody had decided about — `install.sh`, `release.sh`, the two `render-*.sh`,
`deny.toml`, `.config/`, `scripts/`, `.claude-plugin/`, `packages/` holding yidam's own demo
shell, the rest of `.github/`, and a `.vscode/` whose launch config points into
`yidam/editors/vscode`, a directory the vendor step deletes three commands earlier. A repository a
few hours old reported all of them (#807). They are now excluded from the copy itself, and
`template_root.rs` asks of **every** tracked path whether the protocol names it — so the next
file added to the template root is asked without anyone remembering to ask.

## run-the-gate

A repository whose gate has never been run is not a gap in coverage; it is the difference
between a repository that works and one that merely exists, and it is answerable in four
commands. A bootstrap that hands over a repository whose gate it has never run has not finished;
it has stopped.

## regen-cannot-be-skipped

The `<!-- REGEN: ... -->` blocks are generated from a corpus that did not exist when the template
was written, so every one of them is stale on arrival — a bare scaffold with no nodes at all
reports ten stale blocks. Because `.github/workflows/ci.yml` runs `yidam regen --check`, the
repository's first push fails on generated content nobody wrote until the command has been run
and its output committed.

## fix-while-warm

Findings from the first gate run are about work that was written minutes ago by the agent
reading them, which is the cheapest they will ever be to act on. A `catalog-uncited` or a
`missing-property` at this point is a step-4 or step-6 mistake still warm; the same finding six
months from now is archaeology.
