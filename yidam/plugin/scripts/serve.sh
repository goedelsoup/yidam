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
#      It used to name `yidam clone` and `yidam overlay`, which is where #950 came in:
#      both derive from the checkout the shell is standing in, so neither does anything
#      for the person reading the message. The message now names the repairs that work
#      from any directory — `yidam init`, which writes the smallest corpus where the
#      shell is standing (#1035), and the `starting-a-corpus` skill for the part that
#      needs a conversation rather than a command.
#
# Everything here writes to stderr. stdout carries JSON-RPC frames and a stray line on it
# corrupts the protocol.
set -eu

die() {
    printf '%s\n' "$@" >&2
    exit 1
}

# The same directory `serve` will resolve, resolved first. Deciding on one directory and
# serving another is the footgun this script exists to close, so it `cd`s there rather than
# trusting that the two agree.
#
# A non-empty `YIDAM_ROOT` names it, as it does for every command that reads a corpus. That
# is how one shared corpus is served to sessions opened in other projects (#1520): set it in
# the shell or in Claude Code's settings `env`. It is exported again as an absolute path, so
# the binary reads the directory checked here and not one relative to where the `cd` landed.
# Unset or empty, the root is `git rev-parse --show-toplevel`, with the working directory as
# the fallback.
#
# The rest of the environment reaches the server unchanged through `exec`, including
# `FASTEMBED_CACHE_DIR` and the `HF_*` variables the model download reads.
if [ -n "${YIDAM_ROOT:-}" ]; then
    root=$(cd "$YIDAM_ROOT" 2>/dev/null && pwd -P) || die \
        "yidam: YIDAM_ROOT is set to $YIDAM_ROOT, which is not a directory." \
        "  Point it at a yidam corpus, or unset it to serve the project Claude Code opened."
    [ -d "$root/.yidam" ] || die \
        "yidam: YIDAM_ROOT is set to $root, which is not a yidam corpus (no .yidam/ directory)." \
        "  Point it at a yidam corpus, or unset it to serve the project Claude Code opened."
    YIDAM_ROOT=$root
    export YIDAM_ROOT
else
    root=$(git rev-parse --show-toplevel 2>/dev/null) || root=$PWD
    [ -n "$root" ] || root=$PWD
fi
cd "$root"

# `.yidam/`, not `.yidam/corpus/`: a repository bootstrapped an hour ago has the directory
# and nothing in it yet, and an empty corpus is not an absent one.
[ -d .yidam ] || die \
    "yidam: $root is not a yidam corpus (no .yidam/ directory)." \
    "  Run \`yidam init\` here: it writes the few files a corpus needs, where you" \
    "  are standing, and every placeholder says what to replace it with." \
    "  Or ask the agent to \"make this a yidam corpus\" — the plugin ships a" \
    "  starting-a-corpus skill that decides the classes with you first." \
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
