#!/bin/sh
# Render the MCPB manifest for a released yidam.
#
#   render-manifest.sh <version> <platform> > manifest.json
#
# An `.mcpb` is a zip holding this file and a binary, and Claude Desktop installs
# one by drag-and-drop: no PATH, no client config file, no terminal (#421). It is
# the only channel yidam has that a person who cannot install a Rust binary can
# use, which is the whole of #420's complaint.
#
# <platform> is a Node `process.platform` value — `darwin` or `win32` — because
# that is what the manifest's `compatibility.platforms` is specified in. It is NOT
# the Rust target triple the bundle is named after: the manifest has no way to say
# which architecture it carries, so one bundle is built per target and the manifest
# inside two of them differs in nothing. Passing it anyway keeps the caller
# stating which platform it believes it is packaging for, rather than this script
# assuming darwin forever because that is all Claude Desktop ran on the day it was
# written.
#
# A SCRIPT and not a heredoc inside the workflow, for `render-formula.sh`'s reason,
# which applies here with more force: a manifest that is wrong is discovered by a
# stranger double-clicking a file, and a workflow step is testable only by cutting
# a release. This is driven by `yidam/cli/tests/mcpb_bundle.rs`.
#
# ── what this deliberately does not carry ────────────────────────────────────
#
# `tools`. The manifest schema has an optional array of them and the install UI
# renders it, which is a real gain for exactly the audience this bundle is for.
# It is still omitted, because `yidam/prelude/sdks/parity/mcp/tools.json` says of
# itself: "This file is the only place the list lives; a harness that restates it
# is a second freeze, which is how three servers ended up sharing one name out of
# five capabilities." A rendered copy would not be a restatement — but the
# descriptions there are paragraphs written for an agent choosing between tools,
# so rendering them into an install dialog means a truncation rule, and that rule
# is a second editorial voice on a frozen contract. The cost of omitting it is a
# thinner install dialog. The cost of the alternative is paid in the place this
# repository has already been burned.
#
# `compatibility.claude_desktop`. A minimum client version is a claim, and no
# version of this has ever been tested against a lower bound. An absent constraint
# is honest; a guessed one refuses installs for a reason nobody measured.

set -eu

version="${1:-}"
platform="${2:-}"
[ -n "$version" ] && [ -n "$platform" ] || {
  echo "usage: $0 <version> <darwin|win32>" >&2
  exit 2
}

# Claude Desktop runs on macOS and Windows only, so `linux` is not a value this
# manifest can carry — and the release matrix builds two Linux targets, which is
# why the bundle step selects targets rather than packaging all four. A Linux
# bundle would install nowhere.
case "$platform" in
  darwin|win32) ;;
  *)
    echo "$0: platform must be darwin or win32, not '$platform'" >&2
    echo "  Claude Desktop runs on macOS and Windows; there is nothing for a" >&2
    echo "  linux bundle to install into." >&2
    exit 1
    ;;
esac

# `.exe` is appended automatically for binary servers on Windows, so `entry_point`
# and `command` name the same extensionless path on both platforms.
cat <<JSON
{
  "manifest_version": "0.3",
  "name": "yidam",
  "display_name": "Yidam",
  "version": "$version",
  "description": "Ask a yidam corpus questions: search it, read its nodes, and walk the graph of commitments between them.",
  "long_description": "Yidam turns a git repository into a domain computer: a corpus of typed nodes, the edges between them, and gates that hold those commitments true. This extension serves one such repository to Claude over MCP — retrieval that says when it has degraded to keyword search, nodes read by id, a bidirectional graph walk, and an empty answer that tells you which kind of nothing it is.\n\nChoose the repository to serve when you install it. The server is read-only.",
  "author": {
    "name": "goedelsoup",
    "url": "https://github.com/goedelsoup"
  },
  "homepage": "https://github.com/goedelsoup/yidam",
  "documentation": "https://github.com/goedelsoup/yidam/blob/main/docs/mcp-server.md",
  "support": "https://github.com/goedelsoup/yidam/issues",
  "repository": {
    "type": "git",
    "url": "https://github.com/goedelsoup/yidam"
  },
  "license": "MIT",
  "keywords": ["knowledge-graph", "corpus", "git", "provenance", "mcp"],
  "server": {
    "type": "binary",
    "entry_point": "server/yidam",
    "mcp_config": {
      "command": "\${__dirname}/server/yidam",
      "args": ["serve", "--mcp", "--root", "\${user_config.corpus}"]
    }
  },
  "user_config": {
    "corpus": {
      "type": "directory",
      "title": "Corpus",
      "description": "The yidam repository to serve — the directory holding .yidam/. Naming a directory inside one works too.",
      "required": true
    }
  },
  "compatibility": {
    "platforms": ["$platform"]
  }
}
JSON
