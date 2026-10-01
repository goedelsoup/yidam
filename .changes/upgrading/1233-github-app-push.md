### The lander can push as a GitHub App

**`yidam cluster land` can push with a short-lived GitHub App token instead of a deploy key
(#1233).** Set `[cluster] git_auth = "github-app"` and `[cluster.github_app] app_id`. Put the
App's key in the write secret as `private-key.pem`. The lander mints a token for the one
repository, with `contents: write`, and pushes over HTTPS. See
[cluster-runs.md](cluster-runs.md#push-as-a-github-app).

**What changes for you: nothing, unless you opt in.** Deploy keys stay the default. To switch,
regenerate your workflow. If you set `[cluster.egress] remote`, include the GitHub API's
addresses too.

**Update the image first.** An older binary refuses `.yidam/config.toml` once it holds
`git_auth` or `[cluster.github_app]`.
