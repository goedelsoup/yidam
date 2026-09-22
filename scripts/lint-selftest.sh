#!/usr/bin/env bash
# Prove the shared lint contract enforces something, on every package and every file type.
#
# Two silences to catch, and they are the same silence `design-lint-selftest.sh` was written
# for. A rule oxlint does not implement is accepted at load and ignored at run. A path or an
# extension the walker never reaches reports nothing. Both read from the outside exactly like
# a codebase with nothing wrong with it.
#
# `oxlint --rules` is not used to check the first of those, the way the adherence self-test
# does: the flag still exists but prints nothing from 1.83 onward, so a name-against-inventory
# comparison would be matching against an empty string. Every rule is made to FIRE instead,
# which is the property actually wanted — that it is implemented *and* switched on.
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
config="$root/.oxlintrc.json"

oxlint_bin() {
  # Prefer a package's own oxlint over one on PATH, so the self-test grades the binary that
  # will actually run in that package's `npm run lint`.
  local pkg=$1
  if [ -x "$root/$pkg/node_modules/.bin/oxlint" ]; then
    echo "$root/$pkg/node_modules/.bin/oxlint"
  else
    command -v oxlint
  fi
}

fail() { echo "::error::$*"; exit 1; }

# ── 1. every rule the shared config names fires on something that breaks it ──
#
# One fixture per rule, in a directory of its own that extends the real config by absolute
# path. Nothing in the repository is touched.
probe=$(mktemp -d)
trap 'rm -rf "$probe"' EXIT
printf '{ "extends": ["%s"] }\n' "$config" > "$probe/.oxlintrc.json"

cat > "$probe/rules.tsx" <<'EOF'
import { useState } from 'react';
console.log('no-console');
export function C({ flag }: { flag: boolean }) {
  if (flag) { const [a] = useState(0); return <p>{a}</p>; }
  return null;
}
export function unused(x: number) { return 0; }
export const cmp = (a: unknown) => a == null;
export function dbg() { debugger; }
EOF

bin=$(oxlint_bin yidam/editors/vscode)
out=$("$bin" --config "$probe/.oxlintrc.json" "$probe/rules.tsx" 2>&1 || true)

for rule in no-console rules-of-hooks no-unused-vars eqeqeq no-debugger; do
  case "$out" in
    *"($rule)"*) ;;
    *) fail "the shared lint config names \`$rule\`, and it did not fire on a fixture that breaks it.
An unimplemented or disabled rule is accepted at load and ignored at run — whatever it was
for is not being checked, and the job is green because of it. Reproduce with:
  $bin --config $config <a file that breaks $rule>" ;;
  esac
done

# `ignoreRestSiblings` is an exemption rather than a rule, so it is checked from the other
# side: the idiom it exists for must NOT be reported, or the option has silently stopped
# applying and every rest-omit in the tree becomes an error.
cat > "$probe/rest.ts" <<'EOF'
const o = { drop: 1, keep: 2 };
const { drop: _dropped, ...rest } = o;
export default rest;
EOF
# Captured rather than piped straight into `grep -q`. Piping a single oxlint invocation
# into a matcher that exits on its first hit was observed to lose the output altogether here,
# which fails *open*: no match reads as "the exemption still works".
rest_out=$("$bin" --config "$probe/.oxlintrc.json" "$probe/rest.ts" 2>&1 || true)
if printf '%s' "$rest_out" | grep -q "no-unused-vars"; then
  fail "\`ignoreRestSiblings\` is no longer exempting \`const { drop: _x, ...rest } = o\`.
That idiom is how a key gets omitted and its binding is unused by construction."
fi

# ── 2. every package lints every file type it contains ───────────────────────
#
# Discovered rather than listed. A hardcoded set of extensions stops covering a new one
# without ever going red — the file type simply is not mentioned, and silence is the pass.
for pkg in yidam/editors/vscode yidam/editors/web yidam/web/docs; do
  dirs=$(node -e '
    const s = require(process.argv[1] + "/package.json").scripts.lint;
    process.stdout.write(s.split(/\s+/).slice(1).filter((a) => !a.startsWith("-")).join(" "));
  ' "$root/$pkg")
  [ -n "$dirs" ] || fail "$pkg has no lint script to read directories from"

  bin=$(oxlint_bin "$pkg")
  for dir in $dirs; do
    [ -d "$root/$pkg/$dir" ] || fail "$pkg lints \`$dir\`, which does not exist"
  done

  # Every package lints \`.\` — its whole tree, minus whatever .gitignore already excludes —
  # rather than a list of directories. A list has to be kept in step with the tree by hand,
  # and the failure when it is not is silence: a directory nobody added to the list is simply
  # never read. `astro.config.mjs` sat outside both Astro packages' lists exactly that way.

  # The extensions actually present under the linted directories, whatever they are.
  exts=$(cd "$root/$pkg" && find $dirs \
           \( -name node_modules -o -name dist -o -name out -o -name .astro -o -name .git \) -prune -o \
           -type f -name '*.*' -print \
           | sed 's/.*\.//' | sort -u \
           | grep -E '^(ts|tsx|js|jsx|mjs|cjs|astro)$' || true)
  [ -n "$exts" ] || fail "$pkg: found no lintable files under: $dirs"

  for ext in $exts; do
    # Planted in the first linted directory that already holds this extension, so the probe
    # sits where real files of its kind sit and inherits the same overrides.
    host=$(cd "$root/$pkg" && for d in $dirs; do
             found=$(find "$d" \( -name node_modules -o -name dist -o -name out -o -name .astro \) -prune -o \
                       -type f -name "*.$ext" -print | head -1)
             if [ -n "$found" ]; then echo "$d"; break; fi
           done)
    file="$root/$pkg/$host/__lint_selftest__.$ext"
    if [ "$ext" = "astro" ]; then
      printf -- '---\ndebugger;\n---\n<div>selftest</div>\n' > "$file"
    else
      printf 'debugger;\n' > "$file"
    fi

    set +e
    result=$( (cd "$root/$pkg" && "$bin" $dirs 2>&1) )
    code=$?
    set -e
    rm -f "$file"

    if [ $code -eq 0 ] || ! printf '%s' "$result" | grep -q "__lint_selftest__.$ext"; then
      fail "$pkg does not lint \`.$ext\` files: a \`debugger\` planted in $host/ was not reported.
oxlint walked the directory and never read the file, so every rule this config carries is
off for that extension and the gate is green regardless of what those files contain."
    fi
  done
  echo "  $pkg: lints $(echo "$exts" | tr '\n' ' ')under: $dirs"
done

echo "the shared lint contract fires on every rule it names and reaches every file type it covers"
