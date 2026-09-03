#!/bin/sh
# Start `yidam serve --mcp` for the repository Claude Code is working in.
#
# `.mcp.json` could name the binary directly. It names this instead, because the two
# ways that configuration fails are both silent and this is where they can be made to
# speak.
#
#   1. `yidam` is not installed. The plugin installs from a marketplace and carries no
#      binary, so this is the ordinary first run. A bare `command: "yidam"` surfaces as a
#      failed MCP server and no instruction; here it surfaces as the install line.
#
#   2. The directory is not a corpus. A plugin is installed once and Claude Code starts
#      its servers in *every* project, so this is the ordinary case rather than the odd one.
#      `serve` refuses this too (#549) — checking here is not the only line of defence, it
#      is the one that can say it in terms of the plugin, and it avoids starting a 50 MB
#      binary in every non-corpus project to be told no.
#
# Everything here writes to stderr. stdout carries JSON-RPC frames and a stray line on it
# corrupts the protocol.
set -eu

die() {
    printf '%s\n' "$@" >&2
    exit 1
}

# The same directory `serve` will resolve, resolved first: `git rev-parse --show-toplevel`
# with the working directory as the fallback. Deciding on one directory and serving another
# is the footgun this script exists to close, so it `cd`s there rather than trusting that
# the two agree.
root=$(git rev-parse --show-toplevel 2>/dev/null) || root=$PWD
[ -n "$root" ] || root=$PWD
cd "$root"

# `.yidam/`, not `.yidam/corpus/`: a repository bootstrapped an hour ago has the directory
# and nothing in it yet, and an empty corpus is not an absent one.
[ -d .yidam ] || die \
    "yidam: $root is not a yidam corpus (no .yidam/ directory)." \
    "  The yidam MCP server serves one derived repository. Derive one with" \
    "  \`yidam clone <target>\`, or overlay this repository with \`yidam overlay .\`." \
    "  If this project is not a corpus, disable the yidam plugin for it."

# YIDAM_BIN first, so a build that is not on PATH — `.local/bin/yidam` from
# `mise run yidam-build`, or a `cargo install --path` target — can be named without
# editing the plugin.
if [ -n "${YIDAM_BIN:-}" ]; then
    bin=$YIDAM_BIN
    [ -x "$bin" ] || die "yidam: YIDAM_BIN is set to $bin, which is not an executable file."
elif bin=$(command -v yidam 2>/dev/null); then
    :
elif [ -x "$root/.local/bin/yidam" ]; then
    bin=$root/.local/bin/yidam
else
    die \
        "yidam: the \`yidam\` binary is not on PATH." \
        "  Install it:" \
        "    curl -fsSL https://raw.githubusercontent.com/goedelsoup/yidam/main/install.sh | sh" \
        "  Other channels — Homebrew, mise, cargo binstall — are in docs/installation.md." \
        "  Already installed somewhere else? Set YIDAM_BIN to its path."
fi

exec "$bin" serve --mcp "$@"
