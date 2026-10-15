---
title: How Powermise uses mise
description: Powermise reads mise's JSON output and changes things only by running mise commands. Here's exactly how.
---

Powermise is a window onto mise, not a replacement for it. Four rules keep it
that way.

## 1. mise owns its state

Powermise learns what's installed by running mise's JSON commands — `mise ls`,
`mise outdated`, `mise registry`, `mise doctor`, and others — from your home
directory. It changes things only by running mise: `mise use`, `mise upgrade`,
`mise uninstall`, `mise self-update`. It never edits a mise config file itself,
so mise's own rules (pins, file layering, trust) always apply.

## 2. You see every command first

Before anything changes, Powermise shows the exact command. While it runs, you
see mise's own output. **Stop** cancels it — including any downloads mise
started. The [Activity](/app-tour/#activity) screen keeps a list of everything
that ran.

## 3. Nothing is trusted for you

mise refuses to load a project's config until you trust it, because configs can
set environment variables and run hooks. Powermise keeps that boundary: it will
ask, show what the config would do, and never trust a file on its own.

## 4. Your terminal keeps working

There's no Powermise database of your tools. Close the app and mise still has
everything; open a terminal and `mise ls` shows what Powermise showed you.

## Things to know

### Settings in your shell files

Apps opened from the Dock or Finder don't load `~/.zshrc` or `~/.bashrc`, so
Powermise can't see `MISE_*` variables you export there. Put mise settings in
`~/.config/mise/config.toml` (under `[settings]`) and both the app and your
terminals will use them. Powermise does add the usual install locations —
`~/.local/bin`, Homebrew — to its own `PATH` so it can find mise.

### Which mise version

- **2026.10 or newer is recommended.** Powermise is designed around it, and every
  feature is available.
- **2026.7.18 is the minimum.** Older-but-supported versions work; features that
  need newer mise are switched off with a note saying why.

Health shows your version and offers `mise self-update`.

### Which config Powermise changes

Installing from Discover or a tool page sets your **global** default
(`mise use -g`). Projects that ask for a specific version keep it. Per-project
editing is coming with the Projects screen.
