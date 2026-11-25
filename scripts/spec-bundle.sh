#!/usr/bin/env bash
# Pack the spec asset a release ships: yidam-spec-<version>.tar.gz (RFC-0068 §5).
#
#   scripts/spec-bundle.sh <version> <commit> <out-dir>
#
# Assembles yidam-spec-<version>/ from the tree of <commit>, writes its manifest.json, and
# packs it into <out-dir>/yidam-spec-<version>.tar.gz with a .sha256 beside it. Every file,
# and every `versions` value, is read out of that commit with `git cat-file` and never from
# the working tree or a binary. So the asset is what `manifest.json`'s `commit` names, an
# uncommitted edit cannot ship under it, and this runs once, on any runner, with no build.
# It never calls the network.
#
# The tree, and where each part comes from:
#
#   schemas/*.json            yidam/sdks/conformance/schemas/
#   schemas/report.schema.json yidam/sdks/parity/fixtures/reports/report.schema.json
#   conformance/              yidam/sdks/conformance/{README.md,VERSION,stage.toml,cases/}
#   mcp/                      yidam/sdks/parity/mcp/
#   vocabulary.json           yidam/sdks/conformance/vocabulary.json
#
# Files are listed with `git ls-tree` at <commit>, so local scratch never ships.
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

# The commit the asset is cut from, resolved once so a moving ref cannot change under it.
commit="$(git -C "$root" rev-parse --verify --end-of-options "${commit}^{commit}")"

# The file list, NUL-separated, from that commit's tree rather than a directory walk.
git -C "$root" ls-tree -r -z --name-only "$commit" -- yidam/sdks/conformance \
  yidam/sdks/parity/mcp yidam/sdks/parity/fixtures/reports/report.schema.json > "$stage/tracked"

# Python for the copy and the manifest, run isolated (-I) and as a script on stdin, so it
# imports nothing from the tree it reads.
python3 -I - "$root" "$stage/$name" "$stage/tracked" "$version" "$commit" <<'PY'
import json, os, subprocess, sys

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

def blob(src):
    """The bytes of `src` at the commit. A path the commit does not hold is an error."""
    return subprocess.run(
        ["git", "-C", root, "cat-file", "blob", f"{commit}:{src}"],
        check=True,
        capture_output=True,
    ).stdout


def read(src):
    return blob(src).decode("utf-8")


files = {}
for src in sources:
    rel = asset_path(src)
    if rel is None:
        continue
    files[rel] = src
    target = os.path.join(dest, rel)
    os.makedirs(os.path.dirname(target), exist_ok=True)
    with open(target, "wb") as f:
        f.write(blob(src))


def contract_sha(contract):
    """The digest on CONTRACT_SHA's last record, which must be the record for `contract`.

    The ledger is appended in shipping order, so its last record names the current contract.
    It is checked rather than assumed: a VERSION bumped before its record is appended would
    otherwise pair the new version with the previous contract's digest.
    """
    records = [
        line.split()
        for line in read(MCP + "CONTRACT_SHA").splitlines()
        if line.strip() and not line.lstrip().startswith("#")
    ]
    if not records or len(records[-1]) != 2:
        sys.exit(f"spec-bundle: {MCP}CONTRACT_SHA has no `<version> <digest>` last record")
    last_version, digest = records[-1]
    if last_version != contract:
        sys.exit(
            f"spec-bundle: {MCP}VERSION is {contract}, and CONTRACT_SHA's last record is "
            f"for {last_version}"
        )
    return digest


report_schema = json.loads(read(REPORT))
mcp_contract = read(MCP + "VERSION").strip()
manifest = {
    "format_version": "1",
    "cli": version,
    "commit": commit,
    "versions": {
        "report": report_schema["properties"]["format_version"]["const"],
        "mcp_contract": mcp_contract,
        "contract_sha": contract_sha(mcp_contract),
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
