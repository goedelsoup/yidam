# RFC-0014 — Node rename as a sanctioned operation

- **Status:** Implemented
- **Track:** I9
- **Relates to:** RFC-0013 (node model; immutable IDs deferred), RFC-0001 (report contract — the
  gate's home), RFC-0003 (light binary — so a pre-commit hook can run it), RFC-0004 (CI enforcement)
- **Versioning layers touched:** tooling (`yidam` CLI) + template (graph-check rule) + bootstrap
  protocol (rubric S-check)
- **Downstream reference case:** Project BOSC (watermark-directory)

## Summary

"Renaming a node severs edges" is stated three times as a hazard with a single remedy — *choose the
name well*. This RFC takes the two cheap mitigations now: (1) promote the **already-implemented**
broken-link detection into an enforced **no-dangling-edges gate**, so a rename that severs an edge
*fails* instead of merely being reportable after the fact; and (2) add a sanctioned **`yidam rename`**
that rewrites inbound links atomically, so a legitimate rename never trips the gate. Immutable
frontmatter IDs — the heavier, more principled fix — are explicitly **deferred**. The gate is nearly
free because the detection it enforces already exists.

## Problem

The hazard is documented with no operational remedy:
[`directories.md:149-150`](../../yidam/prelude/guidelines/directories.md#L149-L150) — "renaming a node
severs edges, so choose well"; [`information-architecture.md:27`](../information-architecture.md#L27) —
"renaming severs edges." Edges are path-based (`links[].target` on an instance; `[label](path)` in
Markdown), so renaming `a/old.yml` → `a/new.yml` silently invalidates every inbound
`target: ../a/old.yml`.

The detection *exists but does not gate.* `graph-check` already resolves each outgoing link against
the filesystem and reports a break
([`corpus.rs:104-109`](../../yidam/cli/src/cmd/corpus.rs#L104-L109)):

```rust
let resolved = dir.join(target);
if !resolved.exists() {
    node_issues.push(format!("broken link: {target}"));
}
```

But this is a **report**, run on demand, not a **gate**. Nothing stops a rename from landing a commit
full of `broken link:` findings; they surface only if someone runs the report afterward. And there is
no operation that renames a node *without* passing through a broken intermediate state — the reverse
rewrite is manual, so in practice "choose well" is the whole defense. Two gaps: enforcement, and an
atomic rename.

## Proposal

**1 — No-dangling-edges gate.** Promote the existing broken-link finding
([`corpus.rs:108-109`](../../yidam/cli/src/cmd/corpus.rs#L108-L109)) from advisory report to enforced
invariant: a commit whose corpus contains a link to a nonexistent target **fails**. This is nearly
free — the check is already written; only its *status* changes from "printed" to "gating." Its home is
RFC-0001's report contract (a versioned rule with a golden fixture), enforced in CI via RFC-0004's
`check-drift`, and runnable in a local pre-commit hook via RFC-0003's light binary. It is the natural
sibling of the existing orphan rule ([`corpus.rs:100`](../../yidam/cli/src/cmd/corpus.rs#L100), "no
outgoing links") — orphans forbid a node with *no* edge; this forbids an edge to *no* node.

**2 — `yidam rename <old> <new>`.** A CLI command that, in a single atomic commit:

- `git mv`s the node file (history preserved);
- scans the corpus walk (`walk_corpus_instances`) for every inbound link whose `target` resolves to
  `old`, and rewrites it to `new`;
- is committed as an **operational** event — a rename is infrastructure, not an epistemic act
  ([`GRAPH.md:47-64`](../../yidam/prelude/GRAPH.md#L47-L64)) — with a message naming the count, e.g.
  `migrate: concept/old.yml → concept/new.yml (7 inbound links rewritten)`.

  **`migrate`, not `rename`.** This RFC said `rename:` until the command was built, and `rename` is
  in no verb list — `yidam lint --commits` reports it, and `classify_commit` files an operational
  commit as Epistemic, which is the double cost GRAPH.md names. `migrate` ("Data or schema moved")
  is the closest verb that exists, and GRAPH.md is explicit that reaching for the closest one beats
  inventing one. Nothing could have caught this: `every_prescribed_commit_uses_a_recognized_verb`
  scans `git commit -m` invocations under `yidam/prelude`, `sadhana` and `mise.yidam.toml`, and an
  RFC naming a message in prose is neither a shell command nor in scope.

  The command prints the subject rather than making the commit; see below.

**As built, the command rewrites and moves but does not commit.** The property this paragraph wanted
holds anyway: the gate reads the working tree, every edit lands before the move, and the working tree
is never broken. Committing on a user's behalf — from an editor's F2, say — is a larger surprise than
printing the subject and letting them. The reverse scan is the one new piece of machinery (graph-check's forward per-node check
does not build a reverse index), but it rides the corpus walk and link parser that already exist.

**3 — Defer immutable frontmatter IDs.** Stable IDs — so a node's identity survives a rename with no
link rewrite at all — are the principled end state, but they change RFC-0013's instance schema and
every derived corpus, and they interact with cross-repo links. Out of scope here; the two cheap
mitigations remove the acute foot-gun now. Noted tradeoff: with path-as-identity plus atomic local
rename, a *cross-repo* link (a consumer like BOSC mirroring yidam nodes) still breaks on rename;
immutable IDs would fix that. Revisit once RFC-0013's schema is settled.

## Migration & compatibility

Tooling (`yidam rename`) + template (the dangling-edge rule enters RFC-0001's contract) + bootstrap
protocol (a new S-check, sibling to the orphan S3). Additive and backward compatible: a corpus that is
already edge-consistent passes the gate unchanged; the command is opt-in ergonomics. BOSC gains both
once it runs the RFC-0003 light binary in CI (RFC-0004). No node-model change — this composes with
RFC-0013 but does not depend on its acceptance.

## Alternatives considered

- **Immutable frontmatter IDs now.** The stronger fix, rejected *for now* on blast radius: it rewrites
  the node model and every corpus. Deferred, not dismissed — the right revisit after RFC-0013.
- **Keep "choose well."** Rejected: a known foot-gun left un-guarded when the guard is already three
  lines of implemented detection away from enforcement.
- **Per-repo rename scripts.** Rejected: the re-implementation tax this set exists to kill. The reverse
  scan belongs in the tool the reports already live in, not in N shell scripts.

## Open questions

- **Gate as hard block or CI-enforced report?** Consistent with the set's "conformance, not hooks"
  stance (RFC-0004), the lean is: a report rule that CI gates, with a local pre-commit hook optional.
  A hard client-side block risks the same "fires only when on PATH" fate as BOSC's un-gated tasks.
- **Rename atomicity legibility.** One commit rewriting N files is efficient but dense; the message
  naming the inbound count keeps it legible. Is that enough, or should large rewrites be summarized in
  the commit body?
- **Relationship to immutable IDs.** Once IDs land, is `yidam rename` obsolete (identity no longer
  path-bound) or does it stay as the ergonomic surface that also updates human-facing labels? Likely
  the latter, but it depends on RFC-0013's final schema.
