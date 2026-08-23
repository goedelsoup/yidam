#!/usr/bin/env bash
# Cut a release tag for one layer, in the order the layers actually require.
#
#   ./release.sh <layer> <version> [--dry-run] [--yes]
#   mise run release <layer> <version>
#
# Releasing this repository requires knowing several things that are true,
# documented, and nowhere near the shell about to run `git tag`:
#
#   - `sdk/rust/v*` must be published BEFORE `cli/v*`, or the CLI's publish
#     fails on a missing `yidam-core`. That was written at the top of
#     publish-crates.yml, by the person who then tagged them in the other order
#     seven hours later.
#   - Tag patterns differ per layer: `v*` template, `sdk/rust/v*` SDK,
#     `cli/v*` CLI, `bootstrap/v*` protocol, `editor/v*` extension.
#   - A workflow fires only if it EXISTS at the ref being pushed. Adding the
#     workflow after the tag produces a tag that triggers nothing, silently.
#
# Each of those was recorded. None was enforced. This does the ordering instead
# of describing it — VERSIONING.md still explains *why* the layers version
# independently, which is the part worth reading once rather than every time.
#
# It never pushes. Creating a tag is local and reversible; pushing one starts an
# irreversible publish, so that stays a thing a person types.

set -euo pipefail

cd "$(dirname "$0")"

DRY_RUN=0
ASSUME_YES=0
LAYER=""
VERSION=""

for arg in "$@"; do
  case "$arg" in
    --dry-run) DRY_RUN=1 ;;
    --yes|-y)  ASSUME_YES=1 ;;
    -*)        printf 'unknown flag: %s\n' "$arg" >&2; exit 2 ;;
    *)         if [ -z "$LAYER" ]; then LAYER="$arg"; elif [ -z "$VERSION" ]; then VERSION="$arg"; fi ;;
  esac
done

usage() {
  cat >&2 <<'USAGE'
usage: ./release.sh <layer> <version> [--dry-run] [--yes]

layers:
  template    tag v<version>              — no workflow; derived repos pin it
  sdk/rust    tag sdk/rust/v<version>     — publishes yidam-core to crates.io
  cli         tag cli/v<version>          — binaries, tap, and yidam to crates.io
  bootstrap   tag bootstrap/v<version>    — protocol version; no workflow
  editor      tag editor/v<version>       — VS Code Marketplace

Order: sdk/rust before cli, always. The CLI depends on yidam-core by
{ path, version } and cannot publish until crates.io holds a matching one.
USAGE
}

# ── every refusal is named ───────────────────────────────────────────────────
#
# A dry run reports ALL of them rather than stopping at the first, because the
# thing being checked is a state with several independent ways of being wrong,
# and finding them one release attempt at a time is the habit this replaces.
REFUSALS=0
refuse() {
  REFUSALS=$((REFUSALS + 1))
  printf 'refused: %s: %s\n' "$1" "$2" >&2
}

[ -n "$LAYER" ] && [ -n "$VERSION" ] || { usage; exit 2; }

# ── what does this layer tag, and what serves that tag? ──────────────────────
#
# MANIFEST is the file whose declared version must equal the requested one;
# empty means the layer declares its version nowhere (the template layer is a
# tag and nothing else). WORKFLOWS is what must exist at HEAD for the tag to do
# anything at all.
case "$LAYER" in
  template)
    TAG="v$VERSION";               MANIFEST="";                                        WORKFLOWS="" ;;
  sdk/rust)
    TAG="sdk/rust/v$VERSION";      MANIFEST="yidam/prelude/sdks/rust/Cargo.toml";       WORKFLOWS=".github/workflows/publish-crates.yml" ;;
  cli)
    TAG="cli/v$VERSION";           MANIFEST="yidam/cli/Cargo.toml";                     WORKFLOWS=".github/workflows/release.yml .github/workflows/publish-crates.yml" ;;
  bootstrap)
    TAG="bootstrap/v$VERSION";     MANIFEST="yidam/tests/harness/yidam-harness/src/lib.rs"; WORKFLOWS="" ;;
  editor)
    TAG="editor/v$VERSION";        MANIFEST="yidam/editors/vscode/package.json";        WORKFLOWS=".github/workflows/editor.yml" ;;
  *)
    refuse unknown-layer "'$LAYER' is not a layer; see VERSIONING.md"
    usage
    exit 1 ;;
esac

case "$VERSION" in
  [0-9]*.[0-9]*.[0-9]*) ;;
  *) refuse bad-version "'$VERSION' is not major.minor.patch" ;;
esac

# ── the declared version must be the requested one ───────────────────────────
declared=""
if [ -n "$MANIFEST" ]; then
  if [ ! -f "$MANIFEST" ]; then
    refuse missing-manifest "$MANIFEST does not exist"
  else
    case "$MANIFEST" in
      *.toml) declared=$(sed -n 's/^version = "\(.*\)"/\1/p' "$MANIFEST" | head -1) ;;
      *.json) declared=$(sed -n 's/.*"version": *"\([^"]*\)".*/\1/p' "$MANIFEST" | head -1) ;;
      *.rs)   declared=$(sed -n 's/.*PROTOCOL_VERSION: &str = "\([^"]*\)".*/\1/p' "$MANIFEST" | head -1) ;;
    esac
    if [ "$declared" != "$VERSION" ]; then
      refuse version-mismatch "$MANIFEST declares '$declared', you asked for '$VERSION'"
    fi
  fi
fi

# ── a tag whose workflow does not exist yet fires nothing ────────────────────
#
# Checked against HEAD, not the working tree: the tag names a commit, and a
# workflow that is only staged is a workflow the tagged ref does not have.
for wf in $WORKFLOWS; do
  if ! git cat-file -e "HEAD:$wf" 2>/dev/null; then
    refuse no-workflow "$wf is not present at HEAD, so $TAG would trigger nothing"
  fi
done

# ── git state ────────────────────────────────────────────────────────────────
branch=$(git rev-parse --abbrev-ref HEAD)
if [ "$branch" != "main" ]; then
  refuse not-main "HEAD is on '$branch'; releases are cut from main"
fi
if [ -n "$(git status --porcelain)" ]; then
  refuse dirty-tree "the working tree has uncommitted changes"
fi
if git fetch --quiet origin main 2>/dev/null; then
  if [ "$(git rev-parse HEAD)" != "$(git rev-parse origin/main)" ]; then
    behind=$(git rev-list --count HEAD..origin/main)
    ahead=$(git rev-list --count origin/main..HEAD)
    refuse not-synced "HEAD is $ahead ahead / $behind behind origin/main; a tag on an unpushed commit names a commit nobody else has"
  fi
else
  printf 'note: could not reach origin; skipping the sync check.\n' >&2
fi
if git rev-parse -q --verify "refs/tags/$TAG" >/dev/null; then
  refuse tag-exists "$TAG already exists locally"
elif git ls-remote --exit-code --tags origin "$TAG" >/dev/null 2>&1; then
  refuse tag-exists "$TAG already exists on origin"
fi

# ── the ordering, enforced rather than described ─────────────────────────────
#
# This is the precondition that failed. The CLI depends on yidam-core by
# { path, version }: cargo builds against the path here and packages the
# VERSION, so crates.io must already hold a matching yidam-core before `yidam`
# can be published at all. Asking crates.io is the only way to know — the
# manifest says what is required, never what exists.
if [ "$LAYER" = "cli" ] && [ -f yidam/cli/Cargo.toml ]; then
  core=$(sed -n 's/.*yidam-core = {.*version = "\([^"]*\)".*/\1/p' yidam/cli/Cargo.toml | head -1)
  if [ -z "$core" ]; then
    refuse core-version-unreadable "cannot tell which yidam-core version yidam/cli/Cargo.toml requires"
  elif ! command -v curl >/dev/null 2>&1; then
    refuse registry-unreachable "curl is not available, so the yidam-core precondition cannot be checked"
  else
    # The status code, not the body. `curl -f` collapses "this version does not
    # exist" and "crates.io is down" into the same empty string, and those two
    # want opposite responses: one means tag the SDK first, the other means try
    # again later. Guessing between them is how a precondition check becomes a
    # thing people learn to ignore.
    tmp=$(mktemp)
    code=$(curl -sS -m 20 -o "$tmp" -w '%{http_code}' \
             -H 'User-Agent: yidam-release' \
             "https://crates.io/api/v1/crates/yidam-core/$core" 2>/dev/null || echo 000)
    body=$(cat "$tmp"); rm -f "$tmp"
    case "$code" in
      200)
        case "$body" in
          *'"num":"'"$core"'"'*|*'"num": "'"$core"'"'*)
            printf 'yidam-core %s is on crates.io.\n' "$core" ;;
          *)
            refuse dependency-unpublished "crates.io answered for yidam-core $core without naming that version" ;;
        esac ;;
      404)
        refuse dependency-unpublished "yidam-core $core is not on crates.io — tag sdk/rust/v$core first, or the CLI's publish fails on a missing dependency" ;;
      *)
        refuse registry-unreachable "crates.io answered $code; cannot confirm yidam-core $core is published" ;;
    esac
  fi
fi

# ── signing ──────────────────────────────────────────────────────────────────
#
# `git tag -s` is what VERSIONING.md's release process writes, and it fails at
# the moment of tagging if no signing key is configured — after every other
# check has passed, which is the least useful moment to find out.
if [ -z "$(git config --get user.signingkey || true)" ]; then
  refuse no-signing-key "git has no user.signingkey; VERSIONING.md's release process signs tags"
fi

if [ "$REFUSALS" -gt 0 ]; then
  printf '\n%d refusal(s). Nothing was tagged.\n' "$REFUSALS" >&2
  exit 1
fi

# ── say what will happen, then ask ───────────────────────────────────────────
printf '\n  tag      %s\n' "$TAG"
printf '  commit   %s  %s\n' "$(git rev-parse --short HEAD)" "$(git log -1 --format=%s | cut -c1-60)"
if [ -n "$MANIFEST" ]; then printf '  version  %s (from %s)\n' "$declared" "$MANIFEST"; fi
if [ -n "$WORKFLOWS" ]; then
  printf '  fires    %s\n' "$(echo "$WORKFLOWS" | tr ' ' '\n' | sed 's|.github/workflows/||' | paste -sd', ' -)"
else
  printf '  fires    nothing — this layer is a tag consumers pin, not a publish\n'
fi
printf '\n'

if [ "$DRY_RUN" -eq 1 ]; then
  printf 'dry run: every check passed. Nothing was tagged.\n'
  exit 0
fi

if [ "$ASSUME_YES" -eq 0 ]; then
  printf 'Create this tag? [y/N] '
  read -r reply
  case "$reply" in
    y|Y|yes|YES) ;;
    *) printf 'nothing was tagged.\n'; exit 1 ;;
  esac
fi

git tag -s "$TAG" -m "$LAYER $VERSION"
printf '\ncreated %s locally. Nothing is published until you push it:\n\n  git push origin %s\n\n' "$TAG" "$TAG"
