#!/usr/bin/env bash
#
# Install apt packages on a GitHub-hosted Ubuntu runner, falling back to the public Ubuntu
# archive when the runner's Azure mirror cannot be reached.
#
#   .github/scripts/apt-install.sh [apt-get install flags…] <package…>
#
# The runner image points apt at `azure.archive.ubuntu.com` through
# `mirror+file:/etc/apt/apt-mirrors.txt`. On 2026-11-11 that name stopped resolving on the
# runners for over an hour while every other host resolved, and GitHub posted no incident.
# Three steps install packages that way (the logo gate's rasteriser, and the aarch64 cross
# compiler in CI and in release), and they all failed with exit 100. Re-running did not help.
#
# `apt-get update` does not catch it: an unreachable mirror is a warning there ("Some index
# files failed to download. They have been ignored"), and update exits 0. So the failure
# shows up at install, and that is where this script retries: it points apt at
# `archive.ubuntu.com` and runs both steps again. The packages, and the gate that needs them,
# are unchanged. Only the place they are downloaded from changes, and only when the first
# attempt failed.
set -euo pipefail

if [[ $# -eq 0 ]]; then
  echo "usage: $0 [apt-get install flags…] <package…>" >&2
  exit 2
fi

FALLBACK="http://archive.ubuntu.com/ubuntu/"
MIRRORS="${APT_MIRRORS_FILE:-/etc/apt/apt-mirrors.txt}"
SOURCES_DIR="${APT_SOURCES_DIR:-/etc/apt}"

install() {
  sudo apt-get update && sudo apt-get install -y "$@"
}

if install "$@"; then
  exit 0
fi

echo "::warning::apt install failed against the runner's mirror; retrying against $FALLBACK"
if [[ -f "$MIRRORS" ]]; then
  echo "$FALLBACK" | sudo tee "$MIRRORS" >/dev/null
fi
# An image that names the Azure host directly rather than through the mirror list.
grep -rlF "azure.archive.ubuntu.com" "$SOURCES_DIR" 2>/dev/null \
  | xargs -r sudo perl -pi -e 's{http://azure\.archive\.ubuntu\.com/ubuntu/?}{'"$FALLBACK"'}g' \
  || true
install "$@"
