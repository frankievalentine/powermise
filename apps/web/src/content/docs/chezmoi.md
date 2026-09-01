---
title: Using Powermise with chezmoi
description: Keep Powermise for the curated AI-harness environment and chezmoi for your personal dotfiles and selection sources.
---

Powermise and a dotfile manager solve different layers. Used together they
give you a reproducible agentic workstation: Powermise generates and owns the
*curated* environment, while a dotfile manager distributes your *personal*
dotfiles and your Powermise selection sources across machines.

[chezmoi](https://www.chezmoi.io) is the most mature dotfile manager (templating,
per-host/OS differentiation, `run_` scripts, secrets). It is written in Go;
Rust alternatives exist (for example `dotter`, `dotling`, `yui`, `roost`,
`adot`), but chezmoi is the recommended companion here because it is the most
complete and battle-tested.

## Division of labor

| Concern | Owned by | How |
| --- | --- | --- |
| Curated AI-harness environment | Powermise | `apply` generates `config.powermise-<selection>.toml` and runs `mise bootstrap` |
| Curated app/skill/MCP dotfiles | Powermise (catalog-declared) | Compiled into the generated config; converged by mise |
| Your personal dotfiles (`.gitconfig`, `.zshrc`, …) | chezmoi | Managed as chezmoi source state |
| Your selection source files | chezmoi | Distributed as files; imported by Powermise |

Powermise never manages your personal dotfiles, and chezmoi never needs to
understand mise environments. They meet at one point: a chezmoi `run_once_`
script that runs `powermise apply` on each machine.

## Workflow

1. **Place your selection source files under chezmoi.** Keep them as plain
   `*.toml` files (for example `~/selections/my-agent.toml`).
2. **Manage your personal dotfiles with chezmoi** as you normally would.
3. **Add a `run_once_` script** that imports each managed selection and applies
   it for the current machine's target.
4. **Select the environment in your shell** — either via Powermise's own
   `shell install`, or by letting chezmoi template the env line (see below).

## Example run_once_ script

```bash
#!/usr/bin/env bash
# chezmoi run_once_ script: converge the Powermise environment from managed selections.
set -euo pipefail

TARGET="${CHEZMOI_POWERTARGET:-darwin}"

if ! command -v mise >/dev/null 2>&1; then
  echo "mise not found; install from https://mise.jdx.dev" >&2
  exit 1
fi

for sel in "$HOME/selections"/*.toml; do
  [ -e "$sel" ] || continue
  name="$(basename "$sel" .toml)"
  powermise selection import --selection "$sel"
  powermise apply --name "$name" --target "$TARGET" --yes
done
```

The script imports each selection into the Powermise store (idempotent), then
converges it. Because `apply` is idempotent and requires `--yes` outside an
interactive terminal, a non-interactive chezmoi run is safe.

## Shell environment selection

Powermise's `shell install` writes a marker-guarded block into `~/.zshrc` /
`~/.bashrc` that exports `MISE_ENV=powermise-<selection>`. If chezmoi also owns
that rc file, let chezmoi own the line instead and skip `shell install`:

```bash
# in chezmoi's .zshrc.tmpl
export MISE_ENV=powermise-my-agent
```

That single line is the entire block Powermise would have written, so the two
tools never fight over the file.

## Why not have Powermise manage personal dotfiles?

Powermise's scope is the curated agentic stack — the harnesses, tools, apps,
skills, and MCP servers it provision, plus their owned dotfiles. Your personal
dotfiles are a different problem (templating, secrets, per-host divergence of
files you author). chezmoi already solves that well, so Powermise stays focused
and the two compose cleanly.

See [Commands](/commands/), [Lifecycle](/lifecycle/), and
[Troubleshooting](/troubleshooting/) for the underlying Powermise behavior.
