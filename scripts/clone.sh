#!/usr/bin/env bash
set -euo pipefail

TARGET="${1:?Usage: mise run clone <target-directory>}"

if [ -e "$TARGET" ]; then
  echo "error: target already exists: $TARGET"
  exit 1
fi

rsync -a \
  --exclude='.git/' \
  --exclude='target/' \
  --exclude='node_modules/' \
  --exclude='dist/' \
  --exclude='.venv/' \
  --exclude='venv/' \
  --exclude='__pycache__/' \
  --exclude='*.pyc' \
  --exclude='*.pyo' \
  --exclude='*.egg-info/' \
  --exclude='.pnpm-store/' \
  --exclude='*.tsbuildinfo' \
  --exclude='*.lance/' \
  --exclude='.mise.local.toml' \
  --exclude='.DS_Store' \
  . "$TARGET/"

cd "$TARGET"
git init -q
mise trust -q
echo "yidam template copied to $TARGET"
open .
