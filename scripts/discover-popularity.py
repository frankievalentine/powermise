#!/usr/bin/env python3
"""Ranks the mise registry by how often developers install each tool.

    scripts/discover-popularity.py

Popularity comes from Homebrew's public analytics: installs people asked
for (not dependencies) over the last 365 days. Each Homebrew formula is
matched to a mise registry entry by name, alias, or command; tap-qualified
names (`hashicorp/tap/terraform`) and a few renames (`kubernetes-cli` →
kubectl) are handled. Needs `mise` on PATH for the registry. Writes
apps/desktop/src/data/popularity.json.
"""
import datetime
import json
import pathlib
import subprocess
import urllib.request

REPO = pathlib.Path(__file__).resolve().parent.parent
OUT = REPO / "apps/desktop/src/data/popularity.json"
ANALYTICS = "https://formulae.brew.sh/api/analytics/install-on-request/365d.json"
# Homebrew names that differ from mise's.
RENAMES = {
    "kubernetes-cli": "kubectl",
    "openjdk": "java",
    "go-task": "task",
    "git-delta": "delta",
    "python-yq": "yq",
    "postgresql": "postgres",
    "awscli": "aws-cli",
}
LIMIT = 400


def main():
    registry = json.loads(
        subprocess.run(["mise", "registry", "--json"], check=True, capture_output=True, text=True).stdout
    )
    names = {}
    for entry in registry:
        for name in [entry["short"], *entry.get("aliases", []), *(entry.get("bins") or [])]:
            names.setdefault(name.lower(), entry["short"])

    with urllib.request.urlopen(ANALYTICS) as response:
        analytics = json.load(response)

    ranks = {}
    for item in analytics["items"]:
        formula = item["formula"].split("/")[-1].split("@")[0].lower()
        formula = RENAMES.get(formula, formula)
        short = names.get(formula) or names.get(formula.removesuffix("-cli")) or names.get(f"{formula}-cli")
        if short and short not in ranks:
            ranks[short] = len(ranks) + 1
        if len(ranks) >= LIMIT:
            break

    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_text(
        json.dumps(
            {
                "source": "Homebrew install analytics (install-on-request, 365 days), formulae.brew.sh",
                "period": f"{analytics.get('start_date')} – {analytics.get('end_date')}",
                "generated": datetime.date.today().isoformat(),
                "ranks": ranks,
            },
            indent=1,
        )
        + "\n"
    )
    print(f"{len(ranks)} ranked tools → {OUT.relative_to(REPO)}")


if __name__ == "__main__":
    main()
