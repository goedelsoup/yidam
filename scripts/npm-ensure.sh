#!/usr/bin/env bash
# One recipe for "this package needs its dependencies", called by every mise task that runs npm.
#
# The failure it ends: `mise run <task>` on a clean checkout died inside whatever the task was
# actually doing, naming a transitive dependency rather than the missing install.
#
#   $ mise run ext-fixture
#   Error [ERR_MODULE_NOT_FOUND]: Cannot find package 'smol-toml' imported from …/test/stage.ts
#
#   $ mise run parity
#   sh: vitest: command not found
#
# Neither says `npm install`. The first was written up on #606 as the staging script failing to
# run; the script was fine and the package was empty.
#
# It survived because CI installs in a separate workflow step before invoking mise, so every
# task is exercised with `node_modules` already there — the gap existed only on the path a
# person takes, which is the path with no gate on it. `uv` hides it further by not having the
# problem: `parity` runs a TypeScript suite and a Python one side by side, and only the
# TypeScript half needs a step somebody has to remember.
#
# `npm ci` rather than `npm install`, because this only ever runs against an absent
# `node_modules` — a fresh install, which is exactly the case `ci` is for and reproducible in a
# way `install` is not. A lock that disagrees with its `package.json` fails here with npm's own
# message, which is the right moment to hear it. Packages without a lock fall back, because a
# lockless package is a legitimate state and refusing to install one would be this script
# inventing a rule.
#
# Absent, not stale: a `node_modules` that exists and is out of date is a different problem with
# a different fix, and guessing at it here would make an install nobody asked for the price of
# every task invocation.
set -euo pipefail

if [ "$#" -eq 0 ]; then
  echo "usage: npm-ensure.sh <package-dir> [package-dir...]" >&2
  exit 2
fi

for dir in "$@"; do
  if [ ! -f "$dir/package.json" ]; then
    echo "npm-ensure: $dir has no package.json" >&2
    exit 1
  fi
  [ -d "$dir/node_modules" ] && continue

  echo "npm-ensure: installing $dir (no node_modules)" >&2
  if [ -f "$dir/package-lock.json" ]; then
    npm ci --prefix "$dir"
  else
    npm install --prefix "$dir"
  fi
done
