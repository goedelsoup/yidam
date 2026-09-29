#!/usr/bin/env bash
# The controlled half of #967: does an agent handed a section link read the section?
#
#   $ scripts/section-reads-trial.sh ~/Code/goedelsoup/allen-county-ohio-affordability-2026 /tmp/trial 3
#   $ python3 scripts/section-reads.py trial /tmp/trial
#
# Run by hand, not in CI: each run is a real `claude -p` session and costs money
# (MODEL, MAX_TURNS and MAX_BUDGET_USD bound it; the defaults cost about $1.60 a run).
#
# Every run gets its own clone of CORPUS, with the remote removed so nothing it does can
# reach the source. The clone's AGENTS.md gains one occasion heading above "Before taking
# substantive action", holding exactly one route line, which is the only thing the arms vary:
#
#   file    a plain link to GRAPH.md
#   anchor  a link to GRAPH.md#the-class-contract
#   words   the same link, plus "stop at the next `##` heading" in words
#
# The task is the same for all three: write a new class declaration, `program.ont.yml`,
# which is the occasion the class contract governs. CORPUS must be a derived repository
# whose AGENTS.md carries that heading and whose vendored GRAPH.md has the section.
set -euo pipefail

corpus="${1:?usage: section-reads-trial.sh CORPUS OUT_DIR [REPS]}"
out="${2:?usage: section-reads-trial.sh CORPUS OUT_DIR [REPS]}"
reps="${3:-3}"
model="${MODEL:-claude-opus-5}"

prompt='This corpus needs a new class, `program`: a named public assistance program (for example the Housing Choice Voucher program, or HEAP). Write its class declaration at `.yidam/corpus/program.ont.yml`, following the route in AGENTS.md for this occasion. Do not create any instances and do not commit. Stop once the file is written.'

route_line() {
  case "$1" in
    file)   echo '- [Graph model](.yidam/.vendor/prelude/GRAPH.md) — the class contract' ;;
    anchor) echo '- [The class contract](.yidam/.vendor/prelude/GRAPH.md#the-class-contract)' ;;
    words)  echo '- [The class contract](.yidam/.vendor/prelude/GRAPH.md#the-class-contract) — read `## The class contract` and stop at the next `##` heading' ;;
  esac
}

setup() {
  local arm="$1" dir="$2"
  rm -rf "$dir"
  git clone -q --local "$corpus" "$dir"
  git -C "$dir" remote remove origin
  # The pinned binary is untracked, so the clone does not carry it.
  if [ -d "$corpus/.yidam/bin" ]; then cp -R "$corpus/.yidam/bin" "$dir/.yidam/bin"; fi
  python3 - "$dir/AGENTS.md" "$(route_line "$arm")" <<'PY'
import sys
path, line = sys.argv[1], sys.argv[2]
text = open(path).read()
heading = "## Before taking substantive action\n"
if heading not in text:
    sys.exit(path + ": no '" + heading.strip() + "' heading to route under")
block = (heading + "\n### Adding or changing a class\n\nBefore writing or editing a "
         "`<class>.ont.yml`, read:\n\n" + line + "\n\n### Any other work\n")
open(path, "w").write(text.replace(heading, block, 1))
PY
  git -C "$dir" -c user.name=section-reads-trial -c user.email=trial@invalid \
    commit -qam "revise: route the class occasion"
}

run() {
  local arm="$1" rep="$2" dir="$out/$1-$2"
  setup "$arm" "$dir"
  (cd "$dir" && claude -p "$prompt" --model "$model" --output-format stream-json --verbose \
    --allowedTools "Bash Read Grep Glob Write Edit" \
    --max-turns "${MAX_TURNS:-30}" --max-budget-usd "${MAX_BUDGET_USD:-2}" \
    > "$out/$arm-$rep.jsonl" 2> "$out/$arm-$rep.err")
  echo "$arm-$rep exit=$?"
}

mkdir -p "$out"
for arm in file anchor words; do
  for rep in $(seq 1 "$reps"); do
    run "$arm" "$rep" &
  done
done
wait
