---
title: How powermise uses mise
description: powermise reads mise's JSON output and changes things only by running mise commands. Here's exactly how.
---

powermise is a window onto mise, not a replacement for it. Four rules keep it
that way.

## 1. mise owns its state

powermise learns what's installed by running mise's JSON commands — `mise ls`,
`mise outdated`, `mise registry`, `mise doctor`, and others — from your home
directory. It changes things only by running mise: `mise use`, `mise upgrade`,
`mise uninstall`, `mise self-update`. It never edits a mise config file itself,
so mise's own rules (pins, file layering, trust) always apply.

## 2. You see every command first

Before anything changes, powermise shows the exact command. While it runs, you
see mise's own output. **Stop** cancels it — including any downloads mise
started. The [Activity](/app-tour/#activity) screen keeps a list of everything
that ran.

## 3. Nothing is trusted for you

mise refuses to load a project's config until you trust it, because configs can
set environment variables and run hooks. powermise keeps that boundary: before
asking, it reads the config itself — without running anything — and shows its
hooks, the files it loads, the variables it sets, and its tasks. It never trusts
a folder on its own.

## 4. Your terminal keeps working

There's no powermise database of your tools. Close the app and mise still has
everything; open a terminal and `mise ls` shows what powermise showed you.

## Things to know

### Settings in your shell files

Apps opened from the Dock or Finder don't load `~/.zshrc` or `~/.bashrc`, so
powermise can't see `MISE_*` variables you export there. Put mise settings in
`~/.config/mise/config.toml` (under `[settings]`) and both the app and your
terminals will use them. powermise does add the usual install locations —
`~/.local/bin`, Homebrew — to its own `PATH` so it can find mise.

### Which mise version

- **2026.10 or newer is recommended.** powermise is designed around it, and every
  feature is available.
- **2026.7.18 is the minimum.** Older-but-supported versions work; features that
  need newer mise are switched off with a note saying why.
  AI Setups are one of them: they need 2026.10.

Health shows your version and offers `mise self-update`.

### Which config powermise changes

Installing from Discover or a tool page sets your **global** default
(`mise use -g`). Projects that ask for a specific version keep it. On a project's
page, **Add Tool** and environment changes run inside that folder and edit its
own config (`mise use`, `mise set --file …`).

Applying an AI setup is the one time powermise writes a mise config file
itself: `conf.d/powermise-<name>.toml` in your mise config folder, and only
that file. It then runs `mise bootstrap --yes --only packages,tools`. See
[From the terminal](/ai-setups/terminal/).
