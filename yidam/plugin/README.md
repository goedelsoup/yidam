# The yidam Claude Code plugin

`/plugin install yidam@yidam` — the MCP server and the practice it enforces, in one step.

```
/plugin marketplace add goedelsoup/yidam
/plugin install yidam@yidam
```

## What it installs

**The server.** [`.mcp.json`](.mcp.json) registers `yidam serve --mcp` over stdio, which is
the surface RFC-0005 froze: thirteen tools over the corpus, a capability block that declares
its holes at connect time, and an `absence` field on every empty answer saying which kind of
nothing it is.

**Five skills**, in [`skills/`](skills/), one per decision the corpus constrains:

| Skill | Fires before | Calls |
|---|---|---|
| `writing-a-corpus-commit` | a commit subject | `check_subject` |
| `tagging-a-claim` | an evidence tag | `claim_tags` |
| `linking-a-node` | a link between nodes | `licensed_edges` |
| `citing-a-dependency` | a `cites:` into a dependency | `check_citation` |
| `reading-a-corpus` | answering from the corpus | `retrieve`, `get_node`, `query`, … |

Four of those tools exist for exactly this reason. The commit vocabulary, the evidence tags
and the licensed edges are documented in the prelude, and an agent that has to hold that
prose in context complies by having remembered; the tools make it cheap to ask instead. The
skills are what put the asking at the point in the loop where the decision is made.

## What the skills are not

They do not restate the rules. A skill carries *when to ask* and the traps in reading the
answer; the tool carries the content; the prelude carries the reasoning, which is what makes
a rule arguable. A second copy of a closed vocabulary is a second thing to hold in step, and
[`yidam/cli/tests/claude_plugin.rs`](../cli/tests/claude_plugin.rs) fails if a skill writes a
name the frozen contract does not define — or if a tool exists that no skill says when to
reach for.

`bootstrap.md`, the prelude's only skill, does not travel. It is an agent prompt for an
**empty** repository, and this plugin installs into one that already exists.

## What it does not carry

**A binary.** Install `yidam` from any channel in
[docs/installation.md](../../docs/installation.md) first. If it is not on `PATH`,
[`scripts/serve.sh`](scripts/serve.sh) refuses with the install line rather than failing as a
dead server; set `YIDAM_BIN` if it lives somewhere the launcher cannot guess.

The launcher also refuses a directory with no `.yidam/`. A plugin is installed once and
Claude Code starts its servers in every project — and `serve` locates the corpus with
`git rev-parse --show-toplevel` without asking whether it found the repository you meant, so
outside a corpus it starts, serves nothing, and answers every tool with an empty result.
Refusing is louder.

## Releasing

The marketplace is [`.claude-plugin/marketplace.json`](../../.claude-plugin/marketplace.json)
at the repository root, so `/plugin marketplace add goedelsoup/yidam` reads the default branch
and no tag is involved. That is deliberate: four layers already publish onto one release list
(see [VERSIONING.md](../../VERSIONING.md)), and a fifth tag prefix on it buys nothing here.
`version` in [`.claude-plugin/plugin.json`](.claude-plugin/plugin.json) is what a person sees
in `/plugin list`; bump it when the skills or the launcher change.

Validate before pushing:

```sh
claude plugin validate --strict .              # the marketplace manifest
claude plugin validate --strict yidam/plugin   # the plugin manifest
```
