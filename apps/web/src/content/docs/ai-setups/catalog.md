---
title: The curated catalog
description: The versioned harness and tool profiles that AI Setups compile from.
---

The powermise catalog is a set of versioned profiles in the repository's
`catalog/profiles/` folder: harnesses, and the tools, apps, integrations, and
MCP servers they declare. The app ships with a copy built in, so it works
offline and every setup compiles the same way on every machine.

## What the catalog contains

- **Harnesses** — curated AI harness profiles (for example `claude`, `codex`,
  and `gemini`). Each declares the tools it expects.
- **Tools** — versioned tool definitions with one or more install providers
  per platform: a mise tool, or a Homebrew, apt, winget, or Scoop package.
- **Apps** — curated desktop applications a setup can include.
- **Plugins** (`integrations.toml`) — plugins and extensions an agent installs
  with its own CLI: a plugin id for Claude Code and Codex, or a source URL for
  Gemini CLI. Each harness recommends a few.
- **MCP servers** — servers an agent can use: a program mise installs and runs
  (`tool`, `version`, `command`), or a remote server (`url`). Each harness
  recommends a few.

You choose *which* harnesses to include; you don't redefine how they're
installed. The catalog is the single source of truth for what a harness needs
on each platform.

## From catalog to machine

A setup is eligible to apply only when powermise can:

- Resolve every harness it asks for on the target platform.
- Find a compatible provider for every tool it needs.
- Emit one deterministic mise config file without package conflicts.

powermise then writes that file into mise's `conf.d` folder, and
`mise bootstrap` installs it. The catalog never changes your machine on its
own — only **Apply** does, after you've reviewed the file and the command.
MCP servers go into the agents' own settings through mise (see
[MCP servers](/ai-setups/terminal/#mcp-servers)), and plugins install with the
agents' own commands (see [Plugins](/ai-setups/terminal/#plugins)).

The catalog is validated in powermise's test suite (`cargo test --workspace`),
which checks the structure and that every reference resolves. See
[Selections](/ai-setups/harness-selections/) for how a setup picks from it, and
[From the terminal](/ai-setups/terminal/) for what the compiled file looks
like.
