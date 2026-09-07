#!/usr/bin/env bash
# Keep the logo one mark, and keep it legible at the sizes it is actually drawn at.
#
# Three failures, all of which had already happened by the time this was written:
#
#   1. The mark existed in three hand-kept copies that had drifted apart, with nothing to say
#      which was canonical. Every asset is generated now, so drift is a diff.
#   2. A second, unrelated mark shipped in the editor for months. Nobody added it on purpose;
#      it was drawn for a surface the design system did not cover, and then it stayed. So this
#      does not check a list of known files — it goes looking for asset files that no
#      generator claims, which is the only way a fourth copy gets caught the day it appears.
#   3. The favicon was the wide lockup, and 90.5% of its inked pixels at 16px fell under half
#      opacity. Legibility is measured, not asserted.
#
# The `counters` check is the one that is easy to skip and should not be. Ink coverage alone
# calls a mark that has closed up into a solid blob a pass — it has more ink than the drawing
# it replaced, and none of the form. A candidate whose enclosed spaces do not survive from
# 128px to 16px is not the mark that was drawn.
set -euo pipefail

# The gate, per kind. A mark has to survive a browser tab; a lockup is sized by its height in
# a header and never drawn below 20px tall. The floors are set from what the redraw actually
# measures, rounded outward: the wordmark bottoms out at 47.2% and the lockup at 52.9%.
# Not sacred — change them here and the whole tree is re-judged.

repo_root=$(git rev-parse --show-toplevel 2>/dev/null || pwd)
cd "$repo_root"

fail() { echo "::error::$*"; exit 1; }

# Loudly, rather than by skipping. The gate is measured on real renders, so without a
# rasteriser there is no gate — and a guard that quietly passes when it cannot run is worse
# than no guard, because the green tick says it ran.
command -v rsvg-convert >/dev/null || fail \
  "rsvg-convert is not installed, so the mark cannot be rendered and cannot be measured.
    macOS:  brew install librsvg
    Debian: sudo apt-get install librsvg2-bin"


# ── 1. every generated file is what the generator produces ───────────────────
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
node scripts/mark-generate.mjs --out "$tmp" >/dev/null

node scripts/mark-generate.mjs --manifest > "$tmp/manifest.json"
mapfile -t manifest < <(node -e '
  JSON.parse(require("fs").readFileSync(process.argv[1], "utf8")).forEach((a) => console.log(a.path));
' "$tmp/manifest.json")

for rel in "${manifest[@]}"; do
  if [ ! -f "$rel" ]; then
    fail "$rel is generated but not committed. Run: mise run mark-generate"
  fi
  if ! cmp -s "$rel" "$tmp/$rel"; then
    echo "::error::$rel differs from what scripts/mark-generate.mjs produces."
    echo "The assets are generated, so a hand edit here is lost on the next run. Change the"
    echo "geometry in the generator instead, then: mise run mark-generate"
    diff -u "$rel" "$tmp/$rel" | head -20 || true
    exit 1
  fi
done

# ── 2. no asset file the generator does not claim ────────────────────────────
#
# Discovered, not listed. A hardcoded set stops covering new files without ever going red,
# which is exactly how the editor's second mark went unnoticed.
declared=$(printf '%s\n' "${manifest[@]}" | sort)
found=$(git ls-files \
  'yidam/design/assets/*' \
  'yidam/web/docs/public/*.svg' \
  'yidam/web/docs/src/assets/*' \
  'yidam/editors/vscode/resources/*' \
  | grep -Ei '\.(svg|png)$' | sort)

# icon.png is generated from icon.svg by the mise task rather than by the node generator,
# so it is declared here rather than in the manifest.
declared=$(printf '%s\nyidam/editors/vscode/resources/icon.png\n' "$declared" | sort -u)

stray=$(comm -23 <(echo "$found") <(echo "$declared") || true)
if [ -n "$stray" ]; then
  echo "::error::asset files that no generator produces:"
  echo "$stray" | sed 's/^/    /'
  echo "Every logo asset is generated from one geometry so that there is exactly one mark."
  echo "Add it to scripts/mark-generate.mjs, or delete it."
  exit 1
fi

# ── 3. the gate ──────────────────────────────────────────────────────────────
#
# Scored in three passes because the three kinds are not fitted the same way: a mark is fitted
# into a square slot the way a tab does it, and the other two are fitted by height the way a
# header does it.
for kind in mark lockup wordmark; do
  fit=height; [ "$kind" = mark ] && fit=box
  case $kind in
    mark)     sizes=16,20,24,32,128 ;;
    lockup)   sizes=20,24,32,64 ;;
    wordmark) sizes=16,20,24,64 ;;
  esac
  mapfile -t of_kind < <(node -e '
    JSON.parse(require("fs").readFileSync(process.argv[1], "utf8"))
      .filter((a) => a.kind === process.argv[2]).forEach((a) => console.log(a.path));
  ' "$tmp/manifest.json" "$kind")
  [ ${#of_kind[@]} -eq 0 ] && continue
  node scripts/mark-score.mjs "${of_kind[@]}" --sizes="$sizes" --fit="$fit" --json \
    > "$tmp/scores-$kind.json"
done

node -e '
  const fs = require("fs");
  const tmp = process.argv[1];

  // What each kind promises. Only a mark is square, budgeted, and required to keep its
  // counters — a lockup carries a wordmark whose bowls close below its minimum size, which is
  // what a minimum size is for.
  const CONTRACT = {
    mark:     { floor: 60, square: true,  maxElements: 5, counters: true },
    lockup:   { floor: 50, square: false, maxElements: Infinity, counters: false },
    wordmark: { floor: 45, square: false, maxElements: Infinity, counters: false },
  };

  const problems = [];
  let scored = 0;
  for (const [kind, contract] of Object.entries(CONTRACT)) {
    const file = `${tmp}/scores-${kind}.json`;
    if (!fs.existsSync(file)) continue;
    for (const r of JSON.parse(fs.readFileSync(file, "utf8"))) {
      scored++;
      const name = r.path;
      // True of every kind: no ornament, and no text that depends on a font being present.
      if (r.markers > 0) problems.push(`${name}: ${r.markers} arrowhead(s)`);
      if (r.textNodes > 0) problems.push(`${name}: ${r.textNodes} live <text>; outline it`);
      if (r.distinctStrokeWidths > 1) {
        problems.push(`${name}: ${r.distinctStrokeWidths} stroke widths (${r.strokeWidths}); edges and nodes carry one weight`);
      }
      if (contract.square && (!r.viewBox || r.viewBox.w !== r.viewBox.h)) {
        problems.push(`${name}: viewBox is not square, and every hard surface is`);
      }
      const drawn = r.elements - (r.plate ? 1 : 0);
      if (drawn > contract.maxElements) {
        problems.push(`${name}: ${drawn} drawn elements, budget is ${contract.maxElements}`);
      }
      const big = r.renders[r.renders.length - 1];
      for (const s of r.renders) {
        const pct = s.legibility * 100;
        if (pct < contract.floor) {
          problems.push(`${name}: ${pct.toFixed(1)}% legible at ${s.box}px, ${kind} floor is ${contract.floor}%`);
        }
        if (contract.counters && s.counters !== big.counters) {
          problems.push(`${name}: ${s.counters} counter(s) at ${s.box}px but ${big.counters} at ${big.box}px — the form closes up`);
        }
      }
    }
  }
  if (problems.length) {
    console.error("::error::the logo does not clear its own gate:");
    for (const p of problems) console.error("    " + p);
    process.exit(1);
  }
  console.error(`${scored} generated assets clear the gate for their kind`);
' "$tmp"

echo "one mark, generated, legible from 16px up"
