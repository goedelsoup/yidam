### A gather runs on a cluster, and each peer's answer has a receipt

**`yidam cluster workflow` now adds a gather's tasks for each file in `.yidam/gathers/` (#1217).**
Each peer is asked in its own pod, and nothing but the lander holds a git credential.

**What changes for you: a local `yidam gather` now commits receipts too.**
They land at `.yidam/runs/gather/<name>/<peer>.yml`, one per peer that answered or came back empty.
A gather branch you already proposed holds none, so a repeat run builds a different tree.
Pass `--force` to replace it, or delete the branch first.
A corpus without `.yidam/gathers/` gets the same workflow as before.
