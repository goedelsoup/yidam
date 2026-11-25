#!/usr/bin/env bash
# Pack the spec asset a release ships: yidam-spec-<version>.tar.gz (RFC-0068 §5).
#
#   scripts/spec-bundle.sh <version> <commit> <out-dir>
#
# Assembles yidam-spec-<version>/ from committed sources only, writes its manifest.json, and
# packs it into <out-dir>/yidam-spec-<version>.tar.gz with a .sha256 beside it. Every
# `versions` value is read from one committed file and never from a binary, so this runs
# once, on any runner, with no build. It never calls the network.
#
# The tree, and where each part comes from:
#
#   schemas/*.json            yidam/sdks/conformance/schemas/
#   schemas/report.schema.json yidam/sdks/parity/fixtures/reports/report.schema.json
#   conformance/              yidam/sdks/conformance/{README.md,VERSION,stage.toml,cases/}
#   mcp/                      yidam/sdks/parity/mcp/
#   vocabulary.json           yidam/sdks/conformance/vocabulary.json
#
# Files are listed with `git ls-files`, so local scratch under those directories never ships.
# Held by yidam/cli/tests/suite/spec_bundle.rs.
set -euo pipefail

if [ "$#" -ne 3 ]; then
  echo "usage: scripts/spec-bundle.sh <version> <commit> <out-dir>" >&2
  exit 2
fi

version="$1"
commit="$2"
out="$3"
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

mkdir -p "$out"
out="$(cd "$out" && pwd)"
stage="$(mktemp -d)"
trap 'rm -rf "$stage"' EXIT
name="yidam-spec-${version}"

# The file list, NUL-separated, from the index rather than a directory walk.
git -C "$root" ls-files -z -- yidam/sdks/conformance yidam/sdks/parity/mcp \
  yidam/sdks/parity/fixtures/reports/report.schema.json > "$stage/tracked"

# Python for the copy and the manifest, run isolated (-I) and as a script on stdin, so it
# imports nothing from the tree it reads.
python3 -I - "$root" "$stage/$name" "$stage/tracked" "$version" "$commit" <<'PY'
import json, os, re, shutil, sys

root, dest, tracked, version, commit = sys.argv[1:6]

CONFORMANCE = "yidam/sdks/conformance/"
MCP = "yidam/sdks/parity/mcp/"
REPORT = "yidam/sdks/parity/fixtures/reports/report.schema.json"


def asset_path(src):
    """Where a committed file lands in the asset, or None when it does not ship."""
    if src == REPORT:
        return "schemas/report.schema.json"
    if src.startswith(MCP):
        return "mcp/" + src[len(MCP):]
    if src.startswith(CONFORMANCE + "schemas/"):
        return src[len(CONFORMANCE):]
    if src == CONFORMANCE + "vocabulary.json":
        return "vocabulary.json"
    if src.startswith(CONFORMANCE):
        return "conformance/" + src[len(CONFORMANCE):]
    return None


with open(tracked, "rb") as f:
    sources = sorted(p.decode() for p in f.read().split(b"\0") if p)

files = {}
for src in sources:
    if not os.path.isfile(os.path.join(root, src)):
        continue  # tracked but deleted in the working tree
    rel = asset_path(src)
    if rel is None:
        continue
    files[rel] = src
    target = os.path.join(dest, rel)
    os.makedirs(os.path.dirname(target), exist_ok=True)
    shutil.copyfile(os.path.join(root, src), target)


def read(rel):
    with open(os.path.join(root, rel), encoding="utf-8") as f:
        return f.read()


def contract_sha():
    """The digest on CONTRACT_SHA's last record: the one naming the current contract."""
    records = [
        line.split()
        for line in read(MCP + "CONTRACT_SHA").splitlines()
        if line.strip() and not line.startswith("#")
    ]
    return records[-1][1]


report_schema = json.loads(read(REPORT))
manifest = {
    "format_version": "1",
    "cli": version,
    "commit": commit,
    "versions": {
        "report": report_schema["properties"]["format_version"]["const"],
        "mcp_contract": read(MCP + "VERSION").strip(),
        "contract_sha": contract_sha(),
        "vocabulary": json.loads(read(CONFORMANCE + "vocabulary.json"))["format_version"],
        "conformance": read(CONFORMANCE + "VERSION").strip(),
    },
    "files": dict(sorted(files.items())),
}
with open(os.path.join(dest, "manifest.json"), "w", encoding="utf-8") as f:
    json.dump(manifest, f, indent=2)
    f.write("\n")
PY

# COPYFILE_DISABLE keeps macOS tar from adding ._ entries for extended attributes.
COPYFILE_DISABLE=1 tar -czf "$out/$name.tar.gz" -C "$stage" "$name"
(cd "$out" && shasum -a 256 "$name.tar.gz" > "$name.tar.gz.sha256")
echo "wrote $out/$name.tar.gz"
