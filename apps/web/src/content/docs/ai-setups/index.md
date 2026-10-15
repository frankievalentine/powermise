---
title: AI Setups
description: Install a curated AI coding environment — Claude, Codex, Gemini, and more — through mise in one step.
---

AI Setups is the part of powermise that sets up AI coding agents. Pick the
harnesses you want — Claude, Codex, Gemini, Cursor, and others — and powermise
installs everything they need through mise: the agent's CLI, the runtimes it
depends on, and optional desktop apps.

## In the app

Under **AI Setups** in the sidebar:

- **Harnesses** — the catalog, with what each harness installs on each platform.
- **Agent Tools**, **MCP Servers**, **Integrations** — everything the harnesses
  can bring along.
- **My Setups** — the setups you've saved. Create one with **+**, pick
  harnesses, and watch the side panel show exactly what will be installed.
  **Save & Apply** previews every change before anything runs.

## How it relates to the rest of powermise

A setup compiles to a dedicated mise configuration file
(`config.powermise-<name>.toml`) that powermise owns, next to — never inside —
your own mise config. Applying it runs `mise bootstrap`. Remove the setup and
that file goes with it. [The mise boundary](/ai-setups/mise-boundary/) has the
details.

MCP servers and integrations are listed but not configured yet; applying a
setup marks them **Manual setup** instead of pretending they're done.

## From the terminal

AI Setups also has a CLI, `powermise`, for scripts and CI:
[getting started with the CLI](/ai-setups/getting-started/) and the
[command reference](/ai-setups/commands/).
