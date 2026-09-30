#!/usr/bin/env bash
# Run `tests/receipt_compat.rs` against the released CLI it is about (#475, amended 2026-09-30).
#
# The claim under test is that the last release reads the receipt this tree writes. That is a
# claim about a binary this tree cannot build, so this fetches it: the host's tarball from the
# `cli/v$YIDAM_PREVIOUS_VERSION` release, checked against that release's own SHA256SUMS before
# anything in it is run.
#
# The version is pinned in mise.toml rather than read as "the latest `cli/v*`": once the release
# that writes v2 is out, the latest would be it, and the test would be comparing this tree with
# itself. Move the pin to the last release that wrote the previous format when FORMAT_VERSION
# next moves.
set -euo pipefail

version="${YIDAM_PREVIOUS_VERSION:?set by the receipt-compat task in mise.toml}"

case "$(uname -s)-$(uname -m)" in
  Linux-x86_64) target=x86_64-unknown-linux-gnu ;;
  Linux-aarch64 | Linux-arm64) target=aarch64-unknown-linux-gnu ;;
  Darwin-x86_64) target=x86_64-apple-darwin ;;
  Darwin-arm64) target=aarch64-apple-darwin ;;
  *)
    echo "receipt-compat: no ${version} release asset for $(uname -s)-$(uname -m)" >&2
    exit 1
    ;;
esac

dir="yidam/cli/target/receipt-compat/${version}"
name="yidam-${version}-${target}"
base="https://github.com/goedelsoup/yidam/releases/download/cli%2Fv${version}"
mkdir -p "$dir"

if [ ! -x "$dir/$name/yidam" ]; then
  curl -fsSL --retry 3 -o "$dir/$name.tar.gz" "$base/$name.tar.gz"
  curl -fsSL --retry 3 -o "$dir/SHA256SUMS" "$base/SHA256SUMS"
  want="$(awk -v f="$name.tar.gz" '$2 == f { print $1 }' "$dir/SHA256SUMS")"
  if [ -z "$want" ]; then
    echo "receipt-compat: $name.tar.gz is not in the release's SHA256SUMS" >&2
    exit 1
  fi
  if command -v sha256sum >/dev/null; then
    got="$(sha256sum "$dir/$name.tar.gz" | awk '{ print $1 }')"
  else
    got="$(shasum -a 256 "$dir/$name.tar.gz" | awk '{ print $1 }')"
  fi
  if [ "$got" != "$want" ]; then
    echo "receipt-compat: $name.tar.gz is $got, the release says $want" >&2
    exit 1
  fi
  tar -xzf "$dir/$name.tar.gz" -C "$dir"
fi

YIDAM_PREVIOUS_BIN="$PWD/$dir/$name/yidam" \
  cargo test --manifest-path yidam/cli/Cargo.toml --locked --test receipt_compat -- --ignored
