---
title: AI Setups
description: Install a curated AI coding environment — Claude, Codex, Gemini, and more — through mise in one step.
---

AI Setups is the part of powermise that sets up AI coding agents. Pick the
harnesses you want — Claude, Codex, Gemini, Cursor, and others — and powermise
installs everything they need through mise: the agent's CLI, the runtimes it
depends on, and optional desktop apps. AI Setups need mise 2026.10 or newer;
**Health** updates mise in one click.

## In the app

Under **AI Setups** in the sidebar:

- **Harnesses** — the catalog, with what each harness installs on each platform.
- **Agent Tools**, **MCP Servers**, **Integrations** — everything the harnesses
  can bring along.
- **My Setups** — the setups you've saved. Create one with **+**, pick
  harnesses, and the side panel shows exactly what will be installed.

**Save & Apply** opens a review: what gets installed, the exact config file
powermise will write, and the commands it runs. Click **Apply** and mise
installs everything, with its output streaming in the app. Afterwards the
setup's page shows what mise reports for each item — installed, missing, or
using a version from your own config.

## What a setup is

A setup compiles to one mise config file,
`~/.config/mise/conf.d/powermise-<name>.toml`, next to — never inside — your
own config. Applying it runs `mise bootstrap`; removing it deletes the file.
That's the whole contract, so you can do all of it from a terminal too:
[From the terminal: it's just mise](/ai-setups/terminal/).

MCP servers and integrations are listed but not configured yet; a setup marks
them **Manual setup** instead of pretending they're done.

## Reference

- [Selections](/ai-setups/harness-selections/) and the
  [selection file format](/ai-setups/selection-schema/)
- [The catalog](/ai-setups/catalog/) and the
  [harness reference](/ai-setups/harness-reference/)
- [Platforms](/ai-setups/targets/)
