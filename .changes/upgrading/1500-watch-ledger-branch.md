### The watch ledger moves to its own branch

**`watch.yml` now commits its rows to a `yidam/watch` branch, not the default branch (#1500, RFC-0055).**

A quiet month was thirty heartbeat commits in a history every reader counts.
The ledger keeps its path and format, on an orphan branch that holds nothing else.
The `watch` job in `ci.yml` fetches it from there and reads it with the same gate.
The round's guard is stricter: the checkout may hold no change at all.

**What changes for you: nothing, unless your watch is armed.**
Copy both workflows from `sadhana/github/workflows/` and move the ledger in the same change:

```bash
git show HEAD:.github/watch/heartbeats.tsv > "$TMPDIR/heartbeats.tsv"
git switch --orphan yidam/watch
mkdir -p .github/watch && cp "$TMPDIR/heartbeats.tsv" .github/watch/heartbeats.tsv
git add .github/watch/heartbeats.tsv
git commit -m "establish: the watch ledger, moved off main"
git push origin yidam/watch
git switch -
git rm .github/watch/heartbeats.tsv
```

Then commit the deletion with the two workflows.
A ledger left on the default branch fails the gate, and the failure prints these commands.
Old watch commits stay in history; only new rounds go to the branch.

A watch of your own, with a gate that compiles its ledger in, can read it from `origin/yidam/watch` instead.
