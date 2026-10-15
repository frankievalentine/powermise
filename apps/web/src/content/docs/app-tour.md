---
title: App tour
description: Every screen in the powermise desktop app, and the mise command behind each action.
---

Every action in powermise opens a sheet that shows the mise command it will run.
Confirm it, and the sheet streams mise's output live; you can stop it at any
time. When it finishes, every screen refreshes from mise.

## Installed

Every tool mise manages on this computer:

- **In use** — tools a config file asks for, with the active version and where
  it's set (**Global** means your `~/.config/mise/config.toml`).
- **Installed, not in use** — older versions nothing asks for anymore.

A blue badge means a newer version is available. Click a tool to open its page.

Reads: `mise ls --json`, `mise outdated --bump --json`

## Discover

The full [mise registry](https://mise.jdx.dev/registry.html): popular tools
first, then everything else. Search matches names, aliases, and descriptions.

Reads: `mise registry --json`

## Tool pages

- **Install / Change version** — pick a version (latest by default) and install
  it as your default everywhere. Projects can still ask for their own version.
- **On this computer** — each installed version and where it's used. Remove a
  tool from your global config, or uninstall a version that nothing uses.
- **Details** — where mise installs it from (aqua, GitHub releases, a core
  plugin, …) and any aliases.

| Action | Command |
|---|---|
| Install or switch version | `mise use -g <tool>@<version>` |
| Upgrade | `mise upgrade --bump <tool>` |
| Remove from global | `mise unuse -g <tool>` |
| Uninstall one version | `mise uninstall <tool>@<version>` |

Reads: `mise ls-remote --json <tool>`

## Updates

Everything with a newer version — including tools pinned to an exact version,
which a plain `mise outdated` leaves out. Upgrading a pinned tool moves the pin
in your config to the new version.

Commands: `mise upgrade --bump <tool>`, or `mise upgrade --bump` for all.

## Activity

Every command powermise ran since it opened, with how long it took and whether
it worked. Copy any of them to run it yourself.

## Health

- **mise** — your version, the minimum powermise needs, and an update button
  when a newer mise is out (`mise self-update`).
- **Your shell** — whether mise is activated. If it isn't, powermise shows the
  line to add to your shell's startup file. It won't edit that file for you.
- **Problems mise found** and the **config files** it reads.

Reads: `mise doctor --json`, `mise version --json`

## AI Setups

The second section of the sidebar: a catalog of AI coding harnesses and a setup
editor that installs everything they need. See [AI Setups](/ai-setups/).
