#!/usr/bin/env bash
# Prints the Flathub flavour of the Flatpak manifest: the local `dir` source becomes the tagged git
# source. Usage: scripts/flathub-manifest.sh v0.1.0 <commit-sha> > io.github.nishiki001.Cloudberry.yml
set -euo pipefail
cd "$(dirname "$0")/.."
TAG=${1:?tag, e.g. v0.1.0}; COMMIT=${2:?commit sha of the tag}
python3 - "$TAG" "$COMMIT" <<'PY'
import sys, re
tag, commit = sys.argv[1:3]
s = open("packaging/flatpak/io.github.nishiki001.Cloudberry.yml").read()
s = re.sub(r"      - type: dir\n        path: ../..\n        skip: \[[^\]]*\]\n",
           f"      - type: git\n        url: https://github.com/nishiki001/cloudberry.git\n        tag: {tag}\n        commit: {commit}\n", s)
print(s, end="")
PY
