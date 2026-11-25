# RFC-0069 — A worktree reads the main checkout's index, and says it did

- **Status:** Draft
- **Track:** I52
- **Relates to:**
  - #1517 (the vector index is per checkout, so retrieval silently degrades in git worktrees)
  - RFC-0005 (the MCP tool contract) and RFC-0029 (the server reloads its snapshot)
  - RFC-0060 (passages), whose `meta.json` fields this reads unchanged
- **Versioning layers touched:**
  - **CLI.** No command or flag is added. `index-status` and `doctor` say when the index they
    describe belongs to the main worktree (§4).
  - **MCP contract.** Minor, 0.36.0 to 0.37.0. The `corpus` capability block and the
    `retrieve` response each gain a required `index_source` (§3).
  - **Tree.** None. `.yidam/index/` stays gitignored, and nothing new is written.
  - **Reports.** None.
  - **Prelude.** None. Nothing `yidam-vendor-update` copies changes.
  - **Docs.** The MCP server page and the index page say how a worktree finds an index (§5).
  - **The bump.** Minor for the CLI. A worktree that answered `no_index` before now answers
    from a vector index. An upgrade note says so, though no authored file moves.
- **Downstream reference cases:**
  - The record log quoted in #1517, on 0.26.0 (c694ab1). 136 of its 244 `retrieve` calls were
    served degraded. The corpus runs concurrent phases in `git worktree add` checkouts.

## Summary

`.yidam/index/` is gitignored, so it belongs to one checkout. A `git worktree add` checkout
starts without one, and `retrieve` answers by keyword under `no_index` for as long as the
worktree lives. Nobody rebuilds an index for a worktree that will be merged in a day.

This RFC adds one rule to how an index is found. When a checkout holds no index of its own, it
reads the main worktree's index, if that index was built at this checkout's HEAD or one of its
ancestors. Every surface that reports on the index says which one answered.

## Problem

### 0.1 What was measured

The record log in #1517 holds 244 `retrieve` calls. 136 of them came back with
`degraded: true` and `degraded_reason: no_index`. The corpus runs concurrent phases in
`git worktree add` checkouts, as the `phase` workflow encourages, and none of them holds an
index unless someone builds one there.

`doctor` in such a worktree warns `no <worktree>/.yidam/index`. That is true, and the repair it
names, `yidam index-build`, is one nobody takes for a checkout that lives a day.

### 0.2 What already exists

- **One place reads the index from disk.** `model.rs::load_domain_model` reads
  `paths::yidam_index_dir(root)`, which is `root/.yidam/index`, into `DomainModel.index`.
  `retrieval::load` turns that into the vector or degraded state. `serve`, `retrieve`, `query`,
  `record`, `export` and `index-push` all go through `load_domain_model`.
- **`meta.json` records the commit.** `indexed_commit` is the short hash HEAD had when
  `index-build` ran. `serve` already compares it with HEAD to compute `stale`.
- **`stale` is a tri-state.** `ServerState::stale_index` is `None` when there is no git
  repository, and otherwise `Some(indexed_commit != commit)`. An index built at an ancestor of
  HEAD already reads `stale: true`.
- **Ancestry is already asked.** `git/mod.rs` and `cmd/phase` run
  `git merge-base --is-ancestor` through the `Git` wrapper.
- **The rows are keyed by corpus-relative path.** A row names its node by the path under the
  corpus, not by an absolute path. So a row built in one checkout names the same node in
  another checkout at the same commit.

### 0.3 What does not exist

- Nothing looks for an index outside `root`.
- No surface says where the index it answered from lives. A client reads `indexed_commit` and
  has to assume it came from this checkout.

## Design

### 1. Where an index is found

A new module, `index_source`, gains one resolver, `resolve(root)`. Its answer is one of three
things:

1. **Local.** `root/.yidam/index` exists. It is used, exactly as now. A local index always wins,
   whatever commit it was built at and whatever the main worktree holds.
2. **Shared.** There is no local index, and all of the following hold:
   - `root` is inside a linked worktree. The first record of `git worktree list --porcelain`
     names the main worktree, and it is not `root`'s own top level.
   - That first record is not marked `bare`.
   - `<main>/.yidam/index/` holds `corpus.arrow`, and its `meta.json` parses and carries an
     `indexed_commit` that is not `unknown`.
   - That commit is HEAD, or `git merge-base --is-ancestor <indexed_commit> HEAD` succeeds.
3. **None.** Anything else, including a checkout with no git repository at all.

A projected mirror has no git repository, so it never reads a shared index. A main worktree
never reads one either, because the first record names its own top level.

A third answer, **refused**, is kept apart from none inside the resolver: a linked worktree
whose main worktree has an index this branch does not contain. Every reader treats it as none.
Only `doctor` reads the difference (§4).

The resolver runs once per load, in `load_domain_model`, and the answer is recorded on
`IndexData` as its source. `index-build` is not changed. It still writes `root/.yidam/index`,
and once a worktree builds its own index, that local index wins from then on.

### 2. What is refused, and why ancestry

An index built at a commit that is not an ancestor of HEAD is refused. The worktree then reads
as it does today, `no_index`.

The rule exists because an index is a set of rows, each naming a node. Built at an ancestor, its
rows name nodes this branch inherited. Some may have been edited or removed since, which is the
same thing `stale: true` already says about a local index built before the last commit. Built on
another branch, its rows can name nodes this branch never had. A ranking over those is about a
different corpus, and `stale` cannot express that.

The rule has a cost, and it is stated here rather than discovered. The main worktree is usually
on the baseline. If the baseline moves on and the main checkout rebuilds its index, that index
is built at a commit the worktree's branch does not contain. The worktree is refused until it
merges the baseline or builds its own index. `doctor` names this case by its cause (§4), so the
repair is one sentence away.

Filtering a foreign index down to rows whose node exists here was considered and rejected. It
would hide nodes that are new on this branch and present nowhere in the index, and present the
result as a ranking with nothing missing.

### 3. The contract

`index_source` is added in two places. Its values are frozen:

- `"local"`: the index under this checkout's own `.yidam/index`.
- `"shared"`: the main worktree's index, accepted under §1.
- `"remote"`: the index declared in `[index.remote]`.
- `null`: no index answered.

**In the `corpus` capability block,** beside `indexed_commit`. It describes the index the
server loaded, and it is `null` exactly when no index was found, local or remote. A remote
index records no `indexed_commit`, so the two keys are not null together. `stale` keeps its
definition and is computed against whichever index loaded. A shared index at an ancestor reads
`stale: true`, which is the truth about it.

**In the `retrieve` response,** as a required key. It names the index the vector answer came
from, and it is `null` whenever `degraded` is true. A server answers it on every arm, so a
client never has to tell "not reported" from "no index".

No path is in the contract. A server reached over HTTP runs on another machine, and a path on
that machine is no repair the client can make. The CLI surfaces in §4 print the path.

No new `degraded_reason` is added. A refused shared index degrades under `no_index`, because
building an index is the repair either way. The contract's precedence rule already says the
nearer cause wins.

The contract moves to 0.37.0, and the version history records the change. The Rust server is
the only implementation in this repository that reports `indexed_commit`, so it is the only one
that changes.

### 4. The CLI surfaces

- **`index-status`** describes the index the resolver finds. Its JSON report gains
  `index_source`, `local` or `shared`, and `index_path`, the directory it read. Both are null
  when no index was found. They are not `source` and `path` because the report schema is one
  flat namespace, and `vocabulary` already reports a `source`. The prose names the main worktree
  when the source is shared.
- **`doctor`** reports a shared index as a pass, naming its path. When a linked
  worktree has no local index and the main worktree's was refused, the warning says why, for
  example: *the main worktree's index was built at `abc1234`, which this branch does not
  contain.* It offers both repairs: merge the baseline, or run `yidam index-build` here.
- **`serve`'s banner** prints the source beside `indexed_commit`.
- **`export`** and **`bundle`** package only a local index. A shared index was built for a
  different checkout and stamped with that checkout's commit, and an export carries a claim
  about the tree it was made from. Without a local index, they write no index, as now.
- **`index-push`** pushes only a local index, for the same reason.

### 5. Docs

The MCP server page gains a short section on worktrees: what is read, when it is refused, and
which field says so. The index page states the resolution order in §1.

## Alternatives

### An `[index] path` setting

A corpus would name where its index lives, and every checkout would read it. That repairs the
same symptom only for corpora that configure it, and the case in #1517 had not. It also
separates the index from the commit check. A configured path has no main checkout to compare
ancestry against, so it would read any index at any commit. It is left out. A corpus that needs
an index somewhere unusual can still build one locally.

### Accept any index and mark it stale

This has better recall on a long-lived branch. It returns rows for nodes the branch may never
have held, under a flag that already means "behind", not "elsewhere". Rejected for §2's reason.

### Build into the shared index

`index-build` in a worktree would write the main worktree's index. Two worktrees building at
once would overwrite each other, and the main checkout's index would move without anyone there
asking. Rejected.

## Testing

- A suite test builds a corpus, indexes it in the main checkout, adds a worktree at the same
  commit, and asserts that `retrieve` there answers by vector with `index_source: "shared"`.
- The same test commits in the worktree and asserts `stale: true` with the shared source.
- A test moves the main checkout ahead, rebuilds its index, and asserts the worktree reads
  `no_index`. `doctor`'s warning names the refused commit.
- A test builds a local index in the worktree and asserts it wins, with `index_source: "local"`.
- The contract's cases gain the `index_source` key on every `retrieve` arm. The capability-block
  case asserts that `index_source` is null when no index was found.
- `export` in a worktree with only a shared index writes no index.

## Unresolved

- Whether a bare common dir with several linked worktrees should search them for an index. A
  bare repository has no main worktree, so §1 finds none. No case reported it.
