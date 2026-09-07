#!/bin/sh
# Install the yidam CLI.
#
#   curl -fsSL https://raw.githubusercontent.com/goedelsoup/yidam/main/install.sh | sh
#
# Downloads the light default build for this platform from
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
#
# The API rate-limits anonymous callers at 60 requests an hour, per IP, and answers 403
# when that is spent — which reads here as "there is no release" unless it is told apart.
# It is not hypothetical: this script is run from CI runners and from behind shared NAT,
# where the hour's allowance belongs to everyone on the address. So: use a token when the
# environment already has one, never require one, and when it fails say which failure it
# was. `YIDAM_VERSION=cli/v1.2.3` skips the call entirely and is the answer for anyone the
# throttle keeps catching.
tag="${YIDAM_VERSION:-}"
if [ -z "$tag" ]; then
  tmp=$(mktemp -d)
  trap 'rm -rf "$tmp"' EXIT
  token="${GITHUB_TOKEN:-${GH_TOKEN:-}}"
  # Overridable so the failure branches below are reachable by a test, and because a
  # GitHub Enterprise host answers the same API at a different origin.
  api="${YIDAM_API:-https://api.github.com}/repos/$REPO/releases?per_page=100"
  # No `-f`: it turns every HTTP error into exit 22 with no body and no headers, which is
  # the information this needs. A transport failure still exits nonzero, and `status`
  # stays 000 so the two are told apart below.
  if [ -n "$token" ]; then
    status=$(curl -sSL -o "$tmp/releases.json" -D "$tmp/headers" -w '%{http_code}' \
             -H "Authorization: Bearer $token" "$api") || status=000
  else
    status=$(curl -sSL -o "$tmp/releases.json" -D "$tmp/headers" -w '%{http_code}' "$api") || status=000
  fi
  if [ "$status" != "200" ]; then
    remaining=$(sed -n 's/^[Xx]-[Rr]ate[Ll]imit-[Rr]emaining: *\([0-9]*\).*/\1/p' "$tmp/headers" | tail -1)
    if [ "$status" = "403" ] || [ "$status" = "429" ]; then
      if [ "${remaining:-}" = "0" ]; then
        reset=$(sed -n 's/^[Xx]-[Rr]ate[Ll]imit-[Rr]eset: *\([0-9]*\).*/\1/p' "$tmp/headers" | tail -1)
        # BSD `date` and GNU `date` spell this differently and neither accepts the other's
        # flag, so both are tried and the raw epoch is the fallback. A reader told to wait
        # should be told until when.
        if [ -n "${reset:-}" ]; then
          reset=$(date -r "$reset" 2>/dev/null || date -d "@$reset" 2>/dev/null || printf 'epoch %s' "$reset")
        fi
        fail "GitHub is rate-limiting this address (HTTP $status, allowance spent${reset:+, resets $reset}).
       This is a throttle, not a missing release. Either set GITHUB_TOKEN to raise the
       limit, or skip the lookup: YIDAM_VERSION=cli/vX.Y.Z sh install.sh"
      fi
      fail "GitHub refused the release listing with HTTP $status. If this address is not
       rate-limited (remaining: ${remaining:-unknown}), the refusal is something else and
       the response is: $(head -c 200 "$tmp/releases.json")"
    fi
    [ "$status" = "000" ] && fail "could not reach $api"
    fail "the release listing for $REPO answered HTTP $status"
  fi
  tag=$(sed -n 's/.*"tag_name": *"\(cli\/v[^"]*\)".*/\1/p' "$tmp/releases.json" | head -1)
  [ -n "$tag" ] || fail "$REPO has published no cli/v* release (the listing held $(grep -c tag_name "$tmp/releases.json") releases)"
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

# Created above when the tag was resolved through the API; a `YIDAM_VERSION` run skips
# that branch and arrives here with none.
[ -n "${tmp:-}" ] || { tmp=$(mktemp -d); trap 'rm -rf "$tmp"' EXIT; }

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
