#!/usr/bin/env bash
# Sets powermise's version everywhere it's written, ready to tag a release:
#
#   scripts/bump-version.sh 0.2.0
#   git commit -am "chore: release 0.2.0" && git tag v0.2.0 && git push origin main v0.2.0
#
# The release workflow (.github/workflows/release.yml) refuses a tag that
# doesn't match apps/desktop/src-tauri/tauri.conf.json.
set -euo pipefail

version="${1:-}"
if ! [[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.]+)?$ ]]; then
  echo "usage: scripts/bump-version.sh <version>   (for example 0.2.0)" >&2
  exit 1
fi

cd "$(dirname "$0")/.."

python3 - "$version" <<'PY'
import json, pathlib, re, sys

version = sys.argv[1]

def set_json(path):
    p = pathlib.Path(path)
    data = json.loads(p.read_text())
    data["version"] = version
    p.write_text(json.dumps(data, indent=2, ensure_ascii=False) + "\n")

def set_toml(path):
    p = pathlib.Path(path)
    text = p.read_text()
    # Only the first `version = "…"`: the [package] (or [workspace.package]) one.
    text, count = re.subn(r'(?m)^version = "[^"]*"', f'version = "{version}"', text, count=1)
    if count != 1:
        sys.exit(f"no version line in {path}")
    p.write_text(text)

set_json("apps/desktop/src-tauri/tauri.conf.json")
set_json("apps/desktop/package.json")
set_json("package.json")
set_toml("apps/desktop/src-tauri/Cargo.toml")
PY

# Refresh the lock file's entry for the app.
cargo metadata --manifest-path apps/desktop/src-tauri/Cargo.toml --format-version 1 >/dev/null

echo "powermise is now $version. Next:"
echo "  git commit -am \"chore: release $version\" && git tag v$version && git push origin main v$version"
