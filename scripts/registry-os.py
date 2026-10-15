#!/usr/bin/env python3
"""Records which operating systems each mise registry tool supports.

    uv run --no-project --python 3.12 scripts/registry-os.py   (needs Python 3.11+)

`mise registry --json` lists every tool on every OS, but some only install on
some systems (xcodegen on macOS, tmux on Linux and macOS): mise skips them
elsewhere. Their `os` lists live in mise's registry files, so this reads them
from mise's GitHub repository. Writes apps/desktop/src/data/registry-os.json:
`{ "tool": ["linux", "macos"] }` for each limited tool (and its aliases).
"""
import io
import json
import pathlib
import tarfile
import tomllib
import urllib.request

REPO = pathlib.Path(__file__).resolve().parent.parent
OUT = REPO / "apps/desktop/src/data/registry-os.json"
TARBALL = "https://codeload.github.com/jdx/mise/tar.gz/refs/heads/main"


def main():
    with urllib.request.urlopen(TARBALL) as response:
        archive = tarfile.open(fileobj=io.BytesIO(response.read()), mode="r:gz")
    limited = {}
    for member in archive:
        parts = member.name.split("/")
        if not member.isfile() or len(parts) != 3 or parts[1] != "registry" or not parts[2].endswith(".toml"):
            continue
        entry = tomllib.loads(archive.extractfile(member).read().decode())
        systems = sorted(set(entry.get("os") or []) & {"linux", "macos", "windows"})
        if not entry.get("os") or len(systems) == 3:
            continue
        for name in [parts[2].removesuffix(".toml"), *entry.get("aliases", [])]:
            limited[name] = systems
    OUT.write_text(
        json.dumps({"source": "mise registry `os` lists (github.com/jdx/mise/registry)", "os": dict(sorted(limited.items()))}, indent=1)
        + "\n"
    )
    print(f"{len(limited)} OS-limited names → {OUT.relative_to(REPO)}")


if __name__ == "__main__":
    main()
