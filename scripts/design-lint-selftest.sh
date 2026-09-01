#!/usr/bin/env bash
# Prove the adherence lint enforces something, then that it names only rules that exist.
#
# Both halves are about the same silence. A linter that accepts a rule it does not implement
# reports nothing and exits zero, which reads from the outside exactly like a codebase with
# nothing wrong with it — and that is the state `yidam/design/_adherence.oxlintrc.json` was in
# from the day it was written until #467 pointed a fixture at it.
set -euo pipefail

config=yidam/design/_adherence.oxlintrc.json
fixture=yidam/tests/design-lint-selftest

# ── 1. it reports on a file that breaks it ───────────────────────────────────
if oxlint --config "$config" --deny-warnings "$fixture" >/dev/null 2>&1; then
  echo "::error::the adherence lint found nothing in $fixture, which breaks it on purpose."
  echo "A lint with nothing to say exits zero and reads like a lint with nothing to complain"
  echo "about. Run it yourself to see what it did not catch:"
  echo "  oxlint --config $config --deny-warnings $fixture"
  exit 1
fi

# ── 2. every rule it names is one the linter implements ──────────────────────
#
# `oxlint --rules` is the linter's own inventory. A key that is not in it is accepted at
# load and ignored at run — no warning, no exit code, no rule.
rules=$(oxlint --rules)
missing=$(node -e '
  const cfg = JSON.parse(require("fs").readFileSync(process.argv[1], "utf8"));
  const inventory = process.argv[2];
  const unknown = Object.keys(cfg.rules ?? {}).filter((name) => {
    const bare = name.includes("/") ? name.split("/").pop() : name;
    return !inventory.includes(`| ${bare} `) && !inventory.includes(`| ${bare}`.padEnd(2));
  });
  process.stdout.write(unknown.join(" "));
' "$config" "$rules")

if [ -n "$missing" ]; then
  echo "::error::$config configures rules oxlint does not implement: $missing"
  echo "An unknown rule key is accepted and ignored. Whatever those rules were for is not"
  echo "being checked, and the job is green because of it."
  exit 1
fi

echo "the adherence lint reports on $fixture and names only rules oxlint implements"
