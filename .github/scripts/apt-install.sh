#!/usr/bin/env bash
#
# Install apt packages on a GitHub-hosted Ubuntu runner, falling back to a mirror outside
# Canonical's DNS when the runner's mirror cannot be reached.
#
#   .github/scripts/apt-install.sh [apt-get install flags…] <package…>
#
# The runner image points apt at `azure.archive.ubuntu.com` through
# `mirror+file:/etc/apt/apt-mirrors.txt`. On 2026-11-11 Canonical's authoritative
# nameservers (ns{1,2,3}.canonical.com) stopped answering. Every `*.ubuntu.com` name failed
# to resolve wherever a resolver's cache had expired, and GitHub posted no incident because
# GitHub was not the problem. Three steps install packages this way (the logo gate's
# rasteriser, and the aarch64 cross compiler in CI and in release). All three failed with
# exit 100 for hours, and re-running did not help.
#
# So the fallback is a mirror in another DNS zone, not another `ubuntu.com` host: a first
# version fell back to `archive.ubuntu.com`, which failed on the runner in the same way.
# `mirrors.edge.kernel.org` is an official Ubuntu archive mirror. A third-party mirror costs
# no integrity, because apt checks every index against Ubuntu's archive signing key whoever
# serves it. `archive.ubuntu.com` stays in the list as the second choice.
#
# `apt-get update` does not catch the failure: an unreachable mirror is a warning there
# ("Some index files failed to download. They have been ignored"), and update exits 0. So the
# failure shows up at install, and that is where this script retries. The packages, and the
# gate that needs them, are unchanged. Only the place they are downloaded from changes, and
# only when the first attempt failed.
set -euo pipefail

if [[ $# -eq 0 ]]; then
  echo "usage: $0 [apt-get install flags…] <package…>" >&2
  exit 2
fi

# A `mirror+file:` list is tried in order, so it can hold both. The sources rewrite below
# takes one URL, the first.
FALLBACKS=("http://mirrors.edge.kernel.org/ubuntu/" "http://archive.ubuntu.com/ubuntu/")
FALLBACK="${FALLBACKS[0]}"
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
  printf '%s\n' "${FALLBACKS[@]}" | sudo tee "$MIRRORS" >/dev/null
fi
# An image that names the Azure host directly rather than through the mirror list.
grep -rlF "azure.archive.ubuntu.com" "$SOURCES_DIR" 2>/dev/null \
  | xargs -r sudo perl -pi -e 's{http://azure\.archive\.ubuntu\.com/ubuntu/?}{'"$FALLBACK"'}g' \
  || true
install "$@"
