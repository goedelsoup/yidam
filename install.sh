#!/bin/sh
# Install the yidam CLI.
#
#   curl -fsSL https://raw.githubusercontent.com/goedelsoup/yidam/main/install.sh | sh
#
# Downloads the light default build (`reports` + `tonpa`) for this platform from
# the latest `cli/v*` release, verifies its checksum, and installs it. No Rust
# toolchain, no protoc, no ML runtime — those belong to `--features full`, which
# is a source build.
#
# POSIX sh, not bash: this is piped to whatever /bin/sh is, on machines whose
# shell is not a thing the reader chose.
#
# Every failure exits nonzero with a reason. An installer that half-works leaves
# a binary that is worse than no binary, because the next thing to run it cannot
# tell which one it got.

set -eu

REPO="${YIDAM_REPO:-goedelsoup/yidam}"
BIN_DIR="${YIDAM_BIN_DIR:-$HOME/.local/bin}"

fail() { printf 'error: %s\n' "$*" >&2; exit 1; }
need() { command -v "$1" >/dev/null 2>&1 || fail "$1 is required and was not found"; }

need curl
need tar

# ── which artifact does this machine want? ───────────────────────────────────
os=$(uname -s)
arch=$(uname -m)
case "$os-$arch" in
  Darwin-arm64|Darwin-aarch64) target=aarch64-apple-darwin ;;
  Darwin-x86_64)               target=x86_64-apple-darwin ;;
  Linux-x86_64|Linux-amd64)    target=x86_64-unknown-linux-gnu ;;
  Linux-aarch64|Linux-arm64)   target=aarch64-unknown-linux-gnu ;;
  *) fail "no prebuilt binary for $os-$arch — build from source: cargo install --git https://github.com/$REPO --locked yidam" ;;
esac

# ── which release? ───────────────────────────────────────────────────────────
#
# Resolved rather than hardcoded. A version baked into this script works on the
# day it is written and 404s on the next release, which is the failure mode that
# does not announce itself.
#
# The *CLI's* latest release, which `releases/latest` does not answer: it is
# repository-wide, and this repository releases four layers onto one list. This
# script asked it and then refused anything that was not `cli/v*`, so publishing
# any other layer more recently broke `curl | sh` for everyone — `editor/v0.1.0`,
# nine seconds after `cli/v0.4.0`, did exactly that. The list is returned
# newest-first, so the first `cli/v*` row in it is the answer.
tag="${YIDAM_VERSION:-}"
if [ -z "$tag" ]; then
  tag=$(curl -fsSL "https://api.github.com/repos/$REPO/releases?per_page=100" \
        | sed -n 's/.*"tag_name": *"\(cli\/v[^"]*\)".*/\1/p' | head -1)
fi
[ -n "$tag" ] || fail "could not resolve the latest CLI release of $REPO"
case "$tag" in
  cli/v*) ;;
  *) fail "resolved '$tag', which is not a CLI release (expected cli/v*)" ;;
esac
version="${tag#cli/v}"

name="yidam-$version-$target"
url="https://github.com/$REPO/releases/download/$tag/$name.tar.gz"

printf 'yidam %s (%s)\n' "$version" "$target"

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

curl -fsSL "$url" -o "$tmp/$name.tar.gz" || fail "download failed: $url"

# ── verify before installing ─────────────────────────────────────────────────
#
# If no checksum tool is present the download is DECLINED rather than trusted.
# Installing an unverified binary quietly is the one outcome worth failing for.
curl -fsSL "$url.sha256" -o "$tmp/$name.tar.gz.sha256" || fail "no checksum published for $name"
if command -v shasum >/dev/null 2>&1; then
  ( cd "$tmp" && shasum -a 256 -c "$name.tar.gz.sha256" >/dev/null ) || fail "checksum mismatch for $name"
elif command -v sha256sum >/dev/null 2>&1; then
  ( cd "$tmp" && sha256sum -c "$name.tar.gz.sha256" >/dev/null ) || fail "checksum mismatch for $name"
else
  fail "neither shasum nor sha256sum found; refusing to install an unverified binary"
fi

tar -xzf "$tmp/$name.tar.gz" -C "$tmp"
mkdir -p "$BIN_DIR"
install -m 0755 "$tmp/$name/yidam" "$BIN_DIR/yidam" 2>/dev/null \
  || { cp "$tmp/$name/yidam" "$BIN_DIR/yidam" && chmod 0755 "$BIN_DIR/yidam"; }

printf 'installed %s\n' "$BIN_DIR/yidam"
"$BIN_DIR/yidam" --version

# Say so rather than assume. A binary on disk that the shell cannot find is the
# most common way an install "fails" after succeeding.
case ":$PATH:" in
  *":$BIN_DIR:"*) ;;
  *) printf '\n%s is not on your PATH. Add it:\n  export PATH="%s:$PATH"\n' "$BIN_DIR" "$BIN_DIR" ;;
esac
