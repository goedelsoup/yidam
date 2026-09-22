#!/usr/bin/env bash
# Prove the adherence lint enforces something, then that it names only rules that exist.
#
# Both halves are about the same silence. A linter that accepts a rule it does not implement
# reports nothing and exits zero, which reads from the outside exactly like a codebase with
# nothing wrong with it — and that is the state `yidam/design/_adherence.oxlintrc.json` was in
# from the day it was written until #467 pointed a fixture at it.
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
config=yidam/design/_adherence.oxlintrc.json
fixture=yidam/tests/design-lint-selftest

fail() { echo "::error::$*"; exit 1; }

# `oxlint` comes from the `design-lint` task's own `tools`, so run it through mise
# (`mise run design-lint`) rather than directly. Said here because the failure without it is
# section 2's message — the config was refused — which is a true report of what oxlint's exit
# code looked like and entirely the wrong thing to go and look at.
command -v oxlint >/dev/null 2>&1 || fail "no \`oxlint\` on PATH. This script is a step of the
\`design-lint\` task, which provisions it: run \`mise run design-lint\`."

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

# ── 3. every rule it names actually fires ────────────────────────────────────
#
# Resolving is not enforcing. A rule can be implemented, spelled correctly, and still catch
# nothing — switched off by an `overrides` block, or configured against nothing at all. Both
# have happened to this config: `no-restricted-imports` spent its life disabled everywhere by
# an override meant to exempt one file (#467), and `react/forbid-elements` sat at
# `"forbid": []` from the initial design-tool sync until #881, forbidding no element on any
# file, forever. Every check above passed throughout, and so did the lint.
#
# So the fixture is held to its name: it breaks EVERY rule, and each one is required to say
# so. This is the property `scripts/lint-selftest.sh` already requires of the shared config —
# implemented *and* switched on — and the reason it could not be required here before is that
# one of the two rules could not fire by construction.
#
# `-f json` rather than the default renderer: the diagnostic format is chosen automatically
# when it is not stated, and #877 is what happens when a check parses whichever output the
# linter felt like producing. Captured whole before it is parsed, because piping one oxlint
# run straight into a matcher has been observed to lose the output here — and losing it fails
# open, which is the one direction a self-test must never fail.
report=$(oxlint --config "$config" "$fixture" -f json 2>/dev/null || true)

# An unparseable report is its own failure, not an empty one. Left to fall through it would
# read as "every rule was silent" and print a message naming rules that are working — which
# is the whole of #877, reproduced in the check written to replace it.
if ! fired=$(printf '%s' "$report" | node -e '
  let s = ""; process.stdin.on("data", (d) => (s += d)).on("end", () => {
    const diags = JSON.parse(s).diagnostics;
    if (!Array.isArray(diags)) throw new TypeError("no diagnostics array in the report");
    // `code` reads `eslint(no-restricted-imports)` — the plugin, then the rule it ran.
    const names = diags.map((d) => (/\(([^)]+)\)/.exec(d.code ?? "") ?? [])[1]).filter(Boolean);
    process.stdout.write([...new Set(names)].join("\n"));
  });
' 2>/dev/null); then
  fail "oxlint's \`-f json\` report could not be read, so which rules fired is unknown.
This is not the same as no rule firing, and it must not be reported as one. Run:
  oxlint --config $config $fixture -f json
If the shape of that output has changed, this check needs to learn the new shape — not to be
deleted, and not to be believed when it says a working rule was silent."
fi

silent=$(comm -23 <(printf '%s\n' "$configured" | sort -u) <(printf '%s\n' "$fired" | sort -u) | tr '\n' ' ')
silent=${silent% }

if [ -n "$silent" ]; then
  fail "these rules resolve but did not fire on $fixture: $silent
The fixture is called breaks-every-rule because that is its job. A rule that reports nothing
on it is enforcing nothing anywhere — implemented and spelled right and switched off, which
is how \`no-restricted-imports\` spent its life (#467) and how \`react/forbid-elements\` spent
its own (#881). Either the rule stopped applying, or the fixture stopped breaking it. Run:
  oxlint --config $config $fixture
Whichever it is, do not fix it by deleting the rule until you know which."
fi

# ── 4. it reads every file type a consumer can be written in ─────────────────
#
# The last silence, and the one #611 was filed about. Until #611 this lint's path was
# `yidam/design`, so the rule against reaching past `index.js` could not fire on the only code
# in a position to reach — and the task now walks the whole repository instead. A walk covers a
# file type or it does not, and the failure either way is silence: oxlint reads a `.tsx`
# consumer and reports nothing on a `.cjs` one, and from the outside those look identical.
#
# So every extension present in the tree is made to fire, discovered rather than listed — a
# listed set stops covering a new file type without ever going red, which is the shape of the
# defect #611 found in three gates at once. The candidate extensions are the same set
# `lint-selftest.sh` grades the shared config against; that definition is shared on purpose, so
# the two self-tests cannot disagree about what a lintable file is.
#
# `-A correctness` here, matching the repository-wide invocation in `mise.toml`: suppressing
# oxlint's default category is what keeps this config from becoming a second opinion about
# correctness, and this is the proof it does not also suppress the rule the config names.
#
# `import` rather than `require` in the probe, because the rule does not cover `require` — a
# deep `require('…/components/core/Badge.jsx')` in a `.js` file is reported by nothing, which
# was measured, not assumed. Every package here is `"type": "module"`, so that gap is not
# reachable today; a CommonJS consumer is the thing that would make it reachable.
printf '{ "extends": ["%s"] }\n' "$root/$config" > "$probe/.oxlintrc.json"

# `sed` then `grep`, not one `sed` with alternation: BSD sed has no `\|` in a basic regex, so
# the combined form matches nothing and the discovery reads as an empty tree — which this
# script's own floor assertion caught on the first run. `lint-selftest.sh` splits it the same
# way for the same reason.
exts=$(git -C "$root" ls-files \
         | sed 's/.*\.//' | sort -u \
         | grep -E '^(ts|tsx|js|jsx|mjs|cjs|astro)$' || true)
[ -n "$exts" ] || fail "no lintable file type found in the tree at all; this walk is wrong"

# A discovered set proves a type the moment a file of it exists — which is one commit *after*
# the surface whose coverage was the point. `tsx` is the case: #611 put it in scope across three
# gates and this tree holds no `.tsx` file, so discovery alone would have proved every type but
# the one the issue was about.
#
# So the declared set is unioned in, read out of the gate that declares it rather than retyped
# here. Retyping it is the roster again: `tsx` hardcoded in this script covers `tsx` and goes
# quiet for whatever the next extension is. Read this way, an extension cannot enter the token
# gate's scope without this script demanding oxlint prove it — and if oxlint cannot read that
# type, the failure below says so instead of the type being covered by nobody.
#
# `css` is dropped because oxlint does not parse CSS at all, so a probe would prove nothing
# about it. The CSS consumers are covered from the other side, by `design_tokens.rs` itself,
# which reads them directly.
declared=$(sed -n 's/^const CONSUMER_EXTENSIONS[^=]*= *&\[\(.*\)\];$/\1/p' \
             "$root/yidam/cli/tests/design_tokens.rs" \
             | tr -d '"' | tr ',' '\n' | tr -d ' ' | grep -v '^$' | grep -vx css)
[ -n "$declared" ] || fail "could not read \`CONSUMER_EXTENSIONS\` out of
yidam/cli/tests/design_tokens.rs. That const is what the token gate declares in scope, and this
script unions it into the types it makes oxlint prove — silently probing a smaller set if the
read fails, which is the shape of defect #611 was filed about. Check the const's spelling:
  grep -n CONSUMER_EXTENSIONS yidam/cli/tests/design_tokens.rs"

exts=$(printf '%s\n%s\n' "$exts" "$declared" | grep -v '^$' | sort -u)

for ext in $exts; do
  case "$ext" in
    astro)
      printf -- '---\nimport { Badge } from "../../design/components/core/Badge.jsx";\n---\n<div>{Badge}</div>\n' \
        > "$probe/consumer.$ext" ;;
    *)
      printf "import { Badge } from '../../design/components/core/Badge.jsx';\nexport default Badge;\n" \
        > "$probe/consumer.$ext" ;;
  esac
  out=$(oxlint --config "$probe/.oxlintrc.json" -A correctness "$probe/consumer.$ext" 2>&1 || true)
  case "$out" in
    *no-restricted-imports*) ;;
    *) fail "the adherence lint read a \`.$ext\` consumer that imports a component internal and
reported nothing. That file type exists in this tree, so whatever this lint is for is not
happening in it — and a walk that skips an extension is indistinguishable from a tree with
nothing wrong in it. oxlint 1.42 reports on ts, tsx, js, jsx, mjs and astro, and on \`.cjs\`
it reports nothing at all. Reproduce with:
  oxlint --config $config -A correctness <a .$ext file importing design/components/core/…>" ;;
  esac
done

echo "the adherence lint fires every rule it names on $fixture, names only rules oxlint \
resolves, and reaches every file type in the tree: $(printf '%s' "$exts" | tr '\n' ' ')"
