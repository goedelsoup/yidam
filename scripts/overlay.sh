#!/usr/bin/env bash
set -euo pipefail

TARGET="${1:?Usage: mise run overlay <existing-repo-dir>}"

# Validate target
if [ ! -d "$TARGET" ]; then
  echo "error: target does not exist: $TARGET"
  echo "       use 'mise run clone' to create a new repo from the template"
  exit 1
fi

if [ ! -d "$TARGET/.git" ]; then
  echo "error: target is not a git repository: $TARGET"
  exit 1
fi

if [ -d "$TARGET/.yidam" ]; then
  echo "error: .yidam/ already exists in $TARGET — yidam is already bootstrapped"
  exit 1
fi

EXCLUDE=(
  --exclude='.git/'
  --exclude='target/'
  --exclude='node_modules/'
  --exclude='dist/'
  --exclude='.venv/'
  --exclude='venv/'
  --exclude='__pycache__/'
  --exclude='*.pyc'
  --exclude='*.pyo'
  --exclude='*.egg-info/'
  --exclude='.pnpm-store/'
  --exclude='*.tsbuildinfo'
  --exclude='*.lance/'
  --exclude='.mise.local.toml'
  --exclude='.DS_Store'
)

# yidam/ — the prelude, skills, harness, and CLI tools
rsync -a "${EXCLUDE[@]}" yidam/ "$TARGET/yidam/"
echo "yidam/ → $TARGET/yidam/"

# BOOTSTRAP.md — the agent entry point; skip if the target already has one
if [ -f "$TARGET/BOOTSTRAP.md" ]; then
  echo "BOOTSTRAP.md already exists in target — skipping (existing file kept)"
else
  cp BOOTSTRAP.md "$TARGET/BOOTSTRAP.md"
  echo "BOOTSTRAP.md → $TARGET/BOOTSTRAP.md"
fi

# sadhana/ — scaffold templates; useful for the bootstrap agent even in overlay mode
if [ -d sadhana ]; then
  rsync -a "${EXCLUDE[@]}" sadhana/ "$TARGET/sadhana/"
  echo "sadhana/ → $TARGET/sadhana/"
fi

# samudaya/ — domain seeds; only copy if seeds are present beyond README.md and examples/
if [ -d samudaya ]; then
  seed_count=$(find samudaya/ \
    -not -path 'samudaya/examples/*' \
    \( -name '*.md' -not -name 'README.md' -o -name '*.toml' -o -name '*.yml' \) \
    2>/dev/null | wc -l | tr -d ' ')
  if [ "$seed_count" -gt 0 ]; then
    rsync -a "${EXCLUDE[@]}" samudaya/ "$TARGET/samudaya/"
    echo "samudaya/ ($seed_count seed file(s)) → $TARGET/samudaya/"
  else
    echo "samudaya/ has no seeds — skipping"
  fi
fi

# mise.yidam.toml — yidam CLI tasks; user adds `extends = ["mise.yidam.toml"]` to their mise.toml
cp mise.yidam.toml "$TARGET/mise.yidam.toml"
echo "mise.yidam.toml → $TARGET/mise.yidam.toml"

echo ""
echo "yidam infrastructure overlaid on $TARGET"
echo ""
echo "Next steps:"
echo "  1. Add to your mise.toml:  extends = [\"mise.yidam.toml\"]"
echo "  2. Open $TARGET in your IDE"
echo "  3. The agent will read BOOTSTRAP.md and enter existing-repo mode"
echo ""
codium "$TARGET"
