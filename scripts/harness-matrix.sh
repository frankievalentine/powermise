#!/usr/bin/env bash
# Prints the "What each harness installs" table for the harness reference
# docs by compiling every harness for every platform with the real compiler.
#
#   scripts/harness-matrix.sh
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"
cargo build --quiet --locked -p powermise-setups --example fragment
fragment="$repo_root/target/debug/examples/fragment"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

python3 - "$fragment" "$work" <<'PY'
import pathlib, re, subprocess, sys

fragment, work = sys.argv[1], pathlib.Path(sys.argv[2])
# A small reader for the catalog's flat [[harnesses]] tables (no tomllib on
# older Pythons).
def read_harnesses(text):
    for block in text.split("[[harnesses]]")[1:]:
        entry = {"id": re.search(r'^id = "([^"]+)"', block, re.M).group(1)}
        for key in ("required_tool_ids", "mcp_server_ids"):
            match = re.search(rf"^{key} = \[(.*?)\]", block, re.M | re.S)
            entry[key] = re.findall(r'"([^"]+)"', match.group(1)) if match else []
        yield entry

catalog = list(read_harnesses(pathlib.Path("catalog/profiles/harnesses.toml").read_text()))
harnesses = [h for h in catalog if h["id"] != "base"]
base = next(h for h in catalog if h["id"] == "base")
platforms = ["darwin", "linux", "win32", "wsl"]

def compile(harness, target, desktop):
    selection = work / f"{harness}.toml"
    selection.write_text(
        f'schema_version = 2\nname = "{harness}"\ncatalog = "powermise-profiles"\n'
        f'harnesses = ["{harness}"]\n\n[preferences]\ninclude_desktop_apps = {str(desktop).lower()}\n'
    )
    run = subprocess.run([fragment, str(selection), target], capture_output=True, text=True)
    if run.returncode != 0:
        return None
    keys = []
    for line in run.stdout.splitlines():
        if "=" in line and not line.startswith("#"):
            keys.append(line.split("=")[0].strip().strip('"'))
    return keys

base_tools = set(base["required_tool_ids"])
print("| Harness | Works on | Installs on macOS, beyond the basics | Desktop apps (opt-in) | MCP servers |")
print("| --- | --- | --- | --- | --- |")
for harness in sorted(harnesses, key=lambda h: h["id"]):
    hid = harness["id"]
    works = [p for p in platforms if compile(hid, p, False) is not None]
    darwin = compile(hid, "darwin", False) or []
    with_desktop = compile(hid, "darwin", True) or []
    extra = [k for k in darwin if k.split(":")[-1] not in base_tools]
    desktop = [k for k in with_desktop if k not in darwin]
    cell = lambda items: ", ".join(f"`{i}`" for i in items) or "—"
    print(
        f"| `{hid}` | {', '.join(works) or 'none yet'} | {cell(extra)} | {cell(desktop)} "
        f"| {cell(harness.get('mcp_server_ids', []))} |"
    )
PY
