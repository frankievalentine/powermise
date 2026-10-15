#!/usr/bin/env python3
"""Builds the macOS settings catalog from macos-defaults.com.

    scripts/macos-defaults-catalog.py [path-to-checkout]

Without a path it downloads https://github.com/yannbertrand/macos-defaults
(MIT licensed) into a temporary folder. It reads each setting page under
`docs/<category>/<setting>.md` and keeps the facts powermise needs: the
preference domain, key, type, the values the page documents (and which one
is macOS's default), the macOS versions it was tested on, and which app has
to relaunch. Pages without a plain `defaults write` (arrays, deletes) are
skipped. Writes apps/desktop/src/data/macos-defaults.json.
"""
import io
import json
import pathlib
import re
import shlex
import sys
import tarfile
import tempfile
import urllib.request

REPO = pathlib.Path(__file__).resolve().parent.parent
OUT = REPO / "apps/desktop/src/data/macos-defaults.json"
TARBALL = "https://codeload.github.com/yannbertrand/macos-defaults/tar.gz/refs/heads/main"
SKIP_SECTIONS = ("Read current value", "Reset to default value", "Set value from UI", "Requirements")
TYPES = {"-bool": "bool", "-int": "int", "-integer": "int", "-float": "float", "-string": "string"}
CATEGORY_TITLES = {
    "activity-monitor": "Activity Monitor",
    "desktop": "Desktop",
    "dock": "Dock",
    "feedback-assistant": "Feedback Assistant",
    "finder": "Finder",
    "keyboard": "Keyboard",
    "menubar": "Menu Bar",
    "messages": "Messages",
    "misc": "Miscellaneous",
    "mission-control": "Mission Control",
    "mouse": "Mouse",
    "safari": "Safari",
    "screenshots": "Screenshots",
    "simulator": "Simulator",
    "textedit": "TextEdit",
    "timemachine": "Time Machine",
    "trackpad": "Trackpad",
    "xcode": "Xcode",
}


def checkout() -> pathlib.Path:
    if len(sys.argv) > 1:
        return pathlib.Path(sys.argv[1])
    target = pathlib.Path(tempfile.mkdtemp(prefix="macos-defaults-"))
    with urllib.request.urlopen(TARBALL) as response:
        data = response.read()
    with tarfile.open(fileobj=io.BytesIO(data), mode="r:gz") as archive:
        for member in archive.getmembers():
            # Strip the top-level folder; refuse anything that escapes.
            parts = pathlib.PurePosixPath(member.name).parts[1:]
            if not parts or ".." in parts or member.issym() or member.islnk():
                continue
            member.name = str(pathlib.PurePosixPath(*parts))
            archive.extract(member, target)
    return target


def typed(kind: str, raw: str):
    if kind == "bool":
        return raw.lower() in ("true", "yes", "1")
    if kind == "int":
        return int(raw)
    if kind == "float":
        return float(raw)
    return raw


def option_label(heading: str) -> str:
    label = re.sub(r"\(default( value)?\)", "", heading)
    label = re.sub(r"^Set to:?\s*", "", label).strip().strip("`").strip()
    return label


def parse(md: pathlib.Path):
    text = md.read_text()
    title = re.search(r"^# (.+)$", text, re.M)
    description = re.search(r"^description:\s*(.+)$", text, re.M)
    header = text.split("**Parameter type**")[0]
    tested = sorted(set(re.findall(r"^\s+- ([A-Z][A-Za-z ]+)$", header, re.M)) - {"Venura"} | (
        {"Ventura"} if "Venura" in header else set()
    ))
    setting = None
    options = []
    for block in re.finditer(r"^## (.+?)\n(.*?)(?=^## |\Z)", text, re.M | re.S):
        heading, body = block.group(1).strip(), block.group(2)
        if heading.startswith(SKIP_SECTIONS):
            continue
        command = re.search(r"```(?:bash|sh)?\n(.+?)\n```", body, re.S)
        if not command:
            continue
        line = command.group(1).strip().splitlines()[0]
        write, *rest = [part.strip() for part in line.split("&&")]
        try:
            parts = shlex.split(write)
        except ValueError:
            continue
        if parts[:2] != ["defaults", "write"] or len(parts) != 6 or parts[4] not in TYPES:
            continue
        domain, key, kind, raw = parts[2], parts[3], TYPES[parts[4]], parts[5]
        if domain == "-g":
            domain = "NSGlobalDomain"
        relaunch = next(
            (" ".join(shlex.split(step)[1:]) for step in rest if step.startswith("killall")), None
        )
        if setting is None:
            setting = {"domain": domain, "key": key, "type": kind, "relaunch": relaunch}
        elif (domain, key, kind) != (setting["domain"], setting["key"], setting["type"]):
            return None  # One page, several keys: not a single setting.
        paragraph = next(
            (chunk.strip() for chunk in body.split("```")[0].split("\n\n") if chunk.strip()), ""
        )
        options.append(
            {
                "label": option_label(heading),
                "value": typed(kind, raw),
                "isDefault": "(default" in heading,
                "note": re.sub(r"\s+", " ", paragraph),
            }
        )
    if not setting or not options:
        return None
    return {
        "id": f"{md.parent.name}/{md.stem}",
        "category": CATEGORY_TITLES.get(md.parent.name, md.parent.name.title()),
        "title": title.group(1).strip() if title else md.stem,
        "description": description.group(1).strip() if description else "",
        **setting,
        "testedOn": tested,
        "options": options,
        "source": f"https://macos-defaults.com/{md.parent.name}/{md.stem}.html",
    }


def main():
    root = checkout() / "docs"
    settings = [
        parsed
        for md in sorted(root.glob("*/*.md"))
        if md.name != "index.md" and (parsed := parse(md))
    ]
    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_text(
        json.dumps(
            {
                "source": "https://macos-defaults.com",
                "license": "MIT License, Copyright (c) 2020 Yann Bertrand — https://github.com/yannbertrand/macos-defaults",
                "settings": settings,
            },
            indent=1,
            ensure_ascii=False,
        )
        + "\n"
    )
    print(f"{len(settings)} settings → {OUT.relative_to(REPO)}")


if __name__ == "__main__":
    main()
