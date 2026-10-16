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

Under **Harnesses** in the sidebar — the apps and CLIs that run AI coding
agents (Claude Code, Codex, Cursor…):

- **Browse Harnesses** — the catalog, with what each harness installs on each
  platform, and tabs for the **MCP Servers**, **Plugins**, and **Skills** they
  can bring along.
- **Your setups** — listed below it. Create one with **+**, pick
  harnesses, and the side panel shows exactly what will be installed.
  **Export…** on a setup's page saves it as a file; **Import a Setup File…** in
  a new setup opens one from another computer.

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

A setup also adds the MCP servers you pick to each agent's own settings, with
a mise dotfiles merge that owns only those servers, and installs the plugins you
pick with each agent's own plugin command. The **Plugins** tab browses every
plugin Claude Code and Codex offer, and Gemini CLI extensions by link.

## Skills

Skills are folders of instructions an agent loads when they're relevant. The
**Skills** tab installs them with the [skills CLI](https://skills.sh) from
skills.sh, which mise runs (`mise x npm:skills@1.7.2 -- skills …`):

- **Install for** — the agents new skills go to. Agents set up on this
  computer are checked to start.
- **Suggested** — popular skills from Anthropic, Vercel, and the community,
  one click each.
- **Add from GitHub** — any repository (`owner/repo`) or a skills.sh link.
  Name the skills you want, or leave that empty for all of them.
- **Installed** — what's there now, with **Update All** and **Remove…**.

Installing runs, for example:

```sh
mise x npm:skills@1.7.2 -- skills add anthropics/skills -g -y -a claude-code -a codex --skill frontend-design
```

What's installed is read from the agents' skills folders (`~/.claude/skills`,
`~/.agents/skills`, …) and the CLI's lock file, `~/.agents/.skill-lock.json`.
The CLI's anonymous install reporting is off (`DISABLE_TELEMETRY=1`) unless you
turn it on. Skills that tools mise manages ship (`mise skills ls`) are listed
too, and **Link for Claude Code** runs `mise skills sync --global`.

## Reference

- [Selections](/ai-setups/harness-selections/) and the
  [selection file format](/ai-setups/selection-schema/)
- [The catalog](/ai-setups/catalog/) and the
  [harness reference](/ai-setups/harness-reference/)
- [Platforms](/ai-setups/targets/)
