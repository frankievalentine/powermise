---
title: Troubleshooting
description: Common Powermise errors and how to resolve them.
---

A reference for the failures you are most likely to hit, and the exact fix for
each. The first section covers the desktop app; the rest covers the AI Setups
CLI.

## The app says "mise not found"

Powermise looks for mise on its `PATH`, plus `~/.local/bin`,
`~/.local/share/mise/bin`, `/opt/homebrew/bin`, and `/usr/local/bin`. If you
installed mise somewhere else, symlink it into `~/.local/bin`:

```sh
ln -s "$(command -v mise)" ~/.local/bin/mise
```

## Tools install in the app but not in my terminal

Your shell isn't activated. Open **Health** — it shows the exact line to add
to your shell's startup file — then open a new terminal.

## An install fails with a GitHub rate limit

Many tools download from GitHub releases, which limits anonymous downloads.
Check which token mise is using:

```sh
mise token github
```

On mise 2026.10 or newer, `mise token github --oauth` signs in with your GitHub
account and stores the token for you.

## My mise settings are ignored in the app

Apps opened from the Dock don't read `~/.zshrc`, so exported `MISE_*` variables
don't reach Powermise. Move them to `[settings]` in `~/.config/mise/config.toml`.
See [How Powermise uses mise](/how-it-works/#settings-in-your-shell-files).

## Health says my mise is too old

Click **Update** on the Health screen, or run `mise self-update`. Powermise
needs mise 2026.7.18 or newer.

## AI Setups CLI

The sections below apply to the `powermise` command-line tool.

## Apply refuses to run without --yes

```text
error: refusing to apply without --yes because the terminal is not interactive; re-run with --yes to proceed
```

**Cause:** you ran `apply` from a non-interactive terminal (script, CI, piped
input) without `--yes`. Powermise refuses real installs by default in that
context.

**Fix:** add `--yes`. On an interactive terminal the same command prompts
instead.

```sh
powermise apply --name my-agent --target darwin --yes
```

## Mise is not found on PATH

```text
error: could not find mise
```

**Cause:** Powermise shells out to `mise` and it is not resolvable.

**Fix:** install mise and ensure it is on `PATH`, or pass an explicit path:

```sh
powermise apply --name my-agent --target darwin --mise /usr/local/bin/mise --yes
```

## Mise bootstrap failed during apply

```text
mise bootstrap: <reason>
```

**Cause:** a tool, package, or native operation failed (network, permissions,
unavailable provider).

**Fix:** `apply` does **not** roll back. The run report records the failure
boundary, independent operations still ran, and `status` lists follow-up
actions. Address the underlying cause, then re-run `apply` — it is idempotent
and resumes from the recorded state.

```sh
powermise status --name my-agent --target darwin
powermise apply  --name my-agent --target darwin --yes
```

If you only wanted the configuration written (no install), use
`--skip-bootstrap` and run the install separately.

## Validation error: harness or provider unsupported

**Cause:** a harness ID is not in the catalog, or no compatible provider exists
for the requested `--target`.

**Fix:** confirm the harness exists and choose a supported target. Windows
native installs (`win32`) and WSL (`wsl`) are distinct — pick the one matching
where tools run. See [Targets](/ai-setups/targets/).

```sh
powermise registry validate
powermise selection validate --selection my-agent.toml --target darwin
```

## Plan or compile reports no selection

```text
error: no selection named "<name>"; import it first with `selection import`
```

**Cause:** `plan`, `compile`, and `validate` read a **file** with
`--selection <path>`, while `show`, `activate`, `apply`, `status`, `doctor`,
and `exec` act on a **stored** selection via `--name`. A selection must be
`import`ed (or `create`d / built with `interactive`) before `--name` works.

**Fix:** import first, then use `--name`.

```sh
powermise selection import --selection my-agent.toml
powermise apply --name my-agent --target darwin --yes
```

## Drift reported by status or diff

```text
status: pending
drift: <config> differs from recorded manifest
```

**Cause:** the applied configuration no longer matches what Powermise last
wrote (manual edit, external change).

**Fix:** review with `diff`, then re-run `apply` to converge.

```sh
powermise diff   --name my-agent --target darwin
powermise apply  --name my-agent --target darwin --yes
```

## Script output is hard to parse

**Cause:** default `table` output is human-facing and may include color.

**Fix:** request the stable JSON envelope and disable color.

```sh
powermise status --name my-agent --target darwin --output json --color never
```

## Landing-page animation does not appear

**Cause:** the site's chef-hat animation needs WebGL and animations enabled.
Under `prefers-reduced-motion` a static hat is shown instead. In a local dev
server, a stale Vite dependency cache can also block the `three.js` module.

**Fix:** ensure motion is enabled; if running `bun run dev`, restart the dev
server so Vite re-optimizes dependencies.
