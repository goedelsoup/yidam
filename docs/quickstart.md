# Quickstart

*From nothing to a bootstrapped repository whose gate you have watched pass, fail, and pass
again. About twenty minutes, most of it the bootstrap dialogue.*

[bootstrap-flow.md](bootstrap-flow.md) is the specification — ten steps with quality
criteria, written for someone maintaining the protocol. This is the other document: written
for someone meeting it.

**What you are here to see is the loop.** A corpus is a git history with a gate over it. The
gate is what makes an edge a commitment rather than a hyperlink, and nothing about that is
convincing until you have watched it stop you.

---

## 1. Get the CLI — two minutes, no toolchain

```sh
curl -fsSL https://raw.githubusercontent.com/goedelsoup/yidam/main/install.sh | sh
```

or `brew install goedelsoup/tap/yidam`, or `cargo binstall yidam`. If you already use mise:

```sh
mise use -g "github:goedelsoup/yidam[version_prefix=cli/v]@latest"
```

All four fetch the same prebuilt binary; see [installation](installation.md) for what
`version_prefix` is doing there, and [the README](../README.md#getting-started) for the
source build.

```sh
yidam --version
```

It should answer with a version, a build commit, and the features it carries —
`[reports tonpa]` for a released binary. If it does not answer at all, `~/.local/bin` is not
on your `PATH`.

```sh
yidam doctor
```

`doctor` is the one command that answers "is this setup sound" without needing a repository
to be sound about. Run it now, and again the moment anything behaves strangely.

---

## 2. Read a corpus before you make one — five minutes

The fastest way to understand what bootstrap is going to produce is to look at one that
already exists. [`examples/streamflow/`](../examples/streamflow/) is a worked corpus: eight
instances across three classes, on streamflow below dams.

```sh
git clone https://github.com/goedelsoup/yidam
cp -R yidam/examples/streamflow /tmp/streamflow
cd /tmp/streamflow && git init -q && git add -A && git commit -qm genesis
```

The copy-and-`git init` is not ceremony. `yidam` locates a repository with
`git rev-parse --show-toplevel`, so running it inside the yidam checkout finds yidam — which
is a template, not a corpus.

```sh
yidam graph-check
# Checked 8 instances across 3 classes — all clean.

yidam lint
# lint: 0 finding(s), no errors

yidam open-questions
# - [Base-flow separation](.yidam/corpus/concept/base-flow-separation.yml)
# - [Hydropeaking](.yidam/corpus/concept/hydropeaking.yml)
# - [Instream flow right](.yidam/corpus/concept/instream-flow-right.yml)
# - [Valley Bridge gage](.yidam/corpus/gage/valley-bridge.yml)
```

Four open questions out of eight nodes is not an unfinished corpus. It is what a corpus
looks like when the claim tags are being used honestly.

Open [`.yidam/corpus/concept/base-flow-separation.yml`](../examples/streamflow/.yidam/corpus/concept/base-flow-separation.yml)
and [`.yidam/decisions/three-classes.yml`](../examples/streamflow/.yidam/decisions/three-classes.yml).
Between them they show most of what a corpus is for: a claim that cannot be settled and says
so, and a class that was argued out of the ontology with the reason recorded.

The example's own [README](../examples/streamflow/README.md) says what each piece is
demonstrating.

---

## 3. Watch the gate stop you — three minutes

This is the part worth doing rather than reading.

Rename a node the obvious way:

```sh
mv .yidam/corpus/concept/low-flow.yml .yidam/corpus/concept/low-flow-statistics.yml
yidam graph-check
```

```text
Checked 8 instances across 3 classes — 5 clean, 3 with issues:

  .yidam/corpus/concept/hydropeaking.yml
    - broken link: ./low-flow.yml
  .yidam/corpus/concept/instream-flow-right.yml
    - broken link: ./low-flow.yml
  .yidam/corpus/gage/canyon-outlet.yml
    - broken link: ../concept/low-flow.yml
Error: 3 instance(s) have issues
```

Exit code 1. Three edges into that node now point at nothing, and no editor would have told
you — the files that broke are files you did not touch.

`yidam lint` goes further and says the finding is *new*:

```text
not in the baseline — introduced by this change:
  [dangling-edge] .yidam/corpus/concept/hydropeaking.yml
  ...
```

That distinction is what makes a gate usable on a corpus that already has known problems: it
fails on what you introduced, not on what you inherited.

Now repair it the supported way. Put the file back and use the command that knows about
edges:

```sh
mv .yidam/corpus/concept/low-flow-statistics.yml .yidam/corpus/concept/low-flow.yml
yidam rename concept/low-flow concept/low-flow-statistics
```

```text
Renamed concept/low-flow.yml → concept/low-flow-statistics.yml
4 link(s) rewritten across 4 file(s)
  ...
commit: migrate: concept/low-flow.yml → concept/low-flow-statistics.yml (3 inbound link(s) rewritten)
```

```sh
yidam graph-check
# Checked 8 instances across 3 classes — all clean.
```

Note the last line of `rename`'s output: it wrote you a commit message, in the closed
vocabulary `yidam lint --commits` enforces. `migrate:` is an operational verb — this was a
mechanical change, not a knowledge event, and the history should say so.

**`--dry-run` prints the plan and changes nothing.** Use it the first few times.

---

## 4. Make your own — the bootstrap dialogue

```sh
yidam clone ~/my-corpus
cd ~/my-corpus
```

`clone` copies the template, pins the yidam commit it came from in `.yidam.toml`, and runs
`git init`. It does **not** copy yidam's own `docs/` or `examples/` — a new repository starts
with its own ontology, not another domain's nodes.

Then open the repository with an agent and tell it what the domain is. It reads
`.claude/CLAUDE.md`, finds an empty `git log`, and enters bootstrap mode — a ten-step
sequence whose substance is step 2, an **ontology dialogue**. It will ask what the
irreducible kinds in your domain are, what relates them, and what is out of scope. The corpus
you get is only as good as those answers, so this is the part to spend time on.

If you already know some of it, write it into `samudaya/` before you start — axioms the
corpus must contain, hints about relationships, constraints on scope. See
[`samudaya/README.md`](../samudaya/README.md) for what each kind means, and
[`samudaya/examples/`](../samudaya/examples/README.md) for complete seed sets to read or copy
— genealogy, museum provenance and language documentation, one file per commitment. The
bootstrap folds those into the dialogue and then deletes the directory as an explicit
consumption event.

A seed set seeds the dialogue; it does not stand in for it. Copy the seeds that are true of
your work and delete the rest — one you did not mean is worse than one you did not write,
because the dialogue will argue for it and you will have to argue back.

The sequence ends in a **genesis commit**. From that point the repository has a history, and
`CLAUDE.md` stops routing to bootstrap.

```sh
yidam graph-check
yidam status
```

`status` rewrites the REGEN blocks in your README — it is a generator, not only a report.
That is fine here and worth knowing before you run it against a repository you only meant to
read.

---

## 5. Point an agent at it

A corpus that only a person can read is doing half its job. `yidam serve --mcp` puts it
behind an MCP server, and an agent gets semantic retrieval over the nodes, the full YAML of
any one of them, the neighbourhood around it, and the list of what is still open.

```sh
claude mcp add yidam -- yidam serve --mcp     # run from inside the repository
```

One thing to know before you do: the server finds the corpus from the directory it was
started in, so a server launched somewhere else serves an empty corpus without erroring.

The binary you installed in step 1 already carries this. `--features index` upgrades
`retrieve` from keyword to semantic search and changes nothing else; without it the server
says `degraded: true` on every retrieval and names the reason.

That, plus which tool an agent should reach for and how to read `origin`, is in
[mcp-server.md](mcp-server.md).

---

## 6. The loop, from here

That is the whole product:

1. Do the knowledge work — add nodes, link them, tag the claims honestly.
2. `yidam graph-check` and `yidam lint` before committing.
3. Commit with a verb from the closed vocabulary, so the history says which kind of event it was.

Wire steps 2 and 3 into CI with the inherited task layer, and the gate stops depending on
anyone remembering.

## Where to go next

| If you want | Read |
|---|---|
| The bootstrap protocol in full | [bootstrap-flow.md](bootstrap-flow.md) |
| What the terms mean | [vocabulary.md](vocabulary.md) |
| Why a commit is a graph event | [what-yidam-is.md](what-yidam-is.md) |
| How to write a node | [information-architecture.md](information-architecture.md) |
| How claims are tagged, and the traps | [`prelude/guidelines/agent-conduct.md`](../yidam/prelude/guidelines/agent-conduct.md) |
| What the gates actually check | [quality-rubric.md](quality-rubric.md) |
| Connecting an agent over MCP | [mcp-server.md](mcp-server.md) |
| Depending on someone else's corpus | [sharing-derivations.md](sharing-derivations.md) |
| Working with more than one elector | [sangha-resolution-flow.md](sangha-resolution-flow.md) |
