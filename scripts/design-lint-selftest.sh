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

fail() { echo "::error::$*"; exit 1; }

# ── 1. it reports on a file that breaks it ───────────────────────────────────
if oxlint --config "$config" --deny-warnings "$fixture" >/dev/null 2>&1; then
  echo "::error::the adherence lint found nothing in $fixture, which breaks it on purpose."
  echo "A lint with nothing to say exits zero and reads like a lint with nothing to complain"
  echo "about. Run it yourself to see what it did not catch:"
  echo "  oxlint --config $config --deny-warnings $fixture"
  exit 1
fi

# ── 2. every rule it names is one the linter resolves ────────────────────────
#
# Not `oxlint --rules`. That flag is not broken so much as conditional: 1.66.0 added output
# format auto-detection, and the agent formatter it can select does not implement the rules
# listing at all. Under it `--rules` prints nothing and still exits zero, so a comparison
# against it is a comparison against the empty string — every rule reads as unimplemented,
# and the check reports two rules that are implemented and working as the very defect it
# exists to catch. #877.
#
# What made that worth replacing rather than patching with `--format=default`: the detection
# keys on environment variables — `AI_AGENT`, `CLAUDECODE`, `CURSOR_AGENT` and others. CI
# sets none of them, so the check passes there. Run it from inside a coding agent and it
# fails, naming working rules, and the suggested repair is to delete them from the config.
# The one environment where someone is positioned to act on the message is the only one that
# produces it. Upstream: oxc-project/oxc#26343, open, unfixed as of 1.85.0.
#
# `--print-config` is the resolved config — the rules oxlint decided to run, rather than what
# it says about itself. It answers the same whoever is asking, and an unknown rule key does
# not survive it: dropped silently through 1.42, refused by name from 1.60 on. Either way it
# is absent from the answer, and every implemented key is present.
probe=$(mktemp -d)
trap 'rm -rf "$probe"' EXIT

# Read the resolved rule names, or nothing at all if the config was refused.
resolved_rules() {
  local out code
  set +e
  out=$(oxlint --config "$1" --print-config 2>&1)
  code=$?
  set -e
  [ $code -eq 0 ] || return 1
  printf '%s' "$out" | node -e '
    let s = ""; process.stdin.on("data", (d) => (s += d)).on("end", () => {
      const rules = (JSON.parse(s).rules ?? {});
      process.stdout.write(Object.keys(rules).map(bare).join("\n"));
    });
    function bare(n) { return n.includes("/") ? n.split("/").pop() : n; }
  '
}

# The negative control, and the reason this half cannot rot the way `--rules` did. That flag
# went quiet without changing its exit code, so the comparison kept running against nothing
# and read as a pass — for nineteen releases, and then only ever failed where no CI job would
# see it. A mechanism for detecting an unknown rule is asked, on every run, to detect one.
node -e '
  const fs = require("fs");
  const cfg = JSON.parse(fs.readFileSync(process.argv[1], "utf8"));
  cfg.rules["no-such-rule-as-this"] = "error";
  cfg.rules["react/no-such-rule-as-this"] = "error";
  fs.writeFileSync(process.argv[2], JSON.stringify(cfg, null, 2));
' "$config" "$probe/spiked.oxlintrc.json"

if spiked=$(resolved_rules "$probe/spiked.oxlintrc.json"); then
  case "$spiked" in
    *no-such-rule-as-this*)
      fail "oxlint resolved \`no-such-rule-as-this\` as a rule, so \`--print-config\` no longer
tells an implemented rule from an invented one. That is the same failure \`oxlint --rules\`
had: the mechanism went quiet, the comparison kept passing, and a config full of dead rule
names would read as enforced. Find what this linter version does report an unknown key
through before trusting this check again." ;;
  esac
fi

configured=$(node -e '
  const cfg = JSON.parse(require("fs").readFileSync(process.argv[1], "utf8"));
  const bare = (n) => (n.includes("/") ? n.split("/").pop() : n);
  process.stdout.write(Object.keys(cfg.rules ?? {}).map(bare).join("\n"));
' "$config")
[ -n "$configured" ] || fail "$config names no rules at all, which is the same nothing as not being there"

if ! resolved=$(resolved_rules "$config"); then
  fail "oxlint refused $config outright. It prints the reason itself — run:
  oxlint --config $config --print-config
A rule name it does not recognise is named there one per line. Check that the rule exists in
this version of oxlint before deleting it: the rules in this config are implemented, and a
checker reporting otherwise has been the bug twice now."
fi

missing=$(comm -23 <(printf '%s\n' "$configured" | sort -u) <(printf '%s\n' "$resolved" | sort -u) | tr '\n' ' ')
missing=${missing% }

if [ -n "$missing" ]; then
  fail "$config configures rules oxlint does not resolve: $missing
An unknown rule key is accepted and ignored. Whatever those rules were for is not being
checked, and every other job touching the design system is green because of it."
fi

echo "the adherence lint reports on $fixture and names only rules oxlint resolves"
