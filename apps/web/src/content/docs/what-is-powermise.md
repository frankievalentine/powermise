---
title: What is powermise?
description: powermise is an unofficial desktop companion app for mise-en-place — see, install, and update every tool mise manages, with the exact command behind every click.
---

:::note[Unofficial]
powermise is an independent, community project. It isn't made by, affiliated
with, or endorsed by [mise](https://mise.jdx.dev) or its maintainers. Please
report powermise problems to the
[powermise issue tracker](https://github.com/frankievalentine/powermise/issues),
not to mise.
:::

powermise is an unofficial desktop app for [mise-en-place](https://mise.jdx.dev), the tool
that installs and switches between versions of your languages and developer
tools. mise lives in the terminal. powermise puts it in a window:

- **Home** — a getting-started checklist and how everything stands.
- **My Tools** — every tool mise manages, the version you use, and which config
  file set it, plus **Updates**: everything with a newer version, including tools
  pinned to an exact version.
- **Discover** — the 1,000+ tools in the mise registry, most popular first and by
  category, plus everyday utilities.
- **Projects** — each folder's tools, tasks, and environment.
- **Health** — `mise doctor` in plain language: your mise version, whether your
  shell is set up, and anything that needs fixing.
- **Import** — bring over what you already have: versions from nvm, pyenv, and
  other version managers, Homebrew and your Brewfile, winget and Scoop, and
  global npm, pipx, cargo, and gem packages, with a dry run before anything
  changes.
- **AI Agents** — install a curated AI coding environment (Claude, Codex,
  Gemini, and more) in one step, and export it to your other computers. See
  [AI Setups](/ai-setups/).
- **This Computer**, **Settings**, and **History** — machine-wide setup, mise's
  settings in plain words, and every command powermise ran.

Press **⌘K** (Ctrl+K on Windows and Linux) to search everything at once.

## mise is the CLI. powermise is the GUI.

powermise doesn't replace mise or keep its own copy of your setup. It reads
what mise reports and changes things only by running mise commands — and it
shows you each command before it runs it. Anything you do in the app, you can
repeat or undo in a terminal. [How powermise uses mise](/how-it-works/) explains
the details.

## Who it's for

People who want what mise offers without memorizing its commands — and people
who know mise well but want a clear view of what's installed and what's out of
date. If you start in the app, the commands it shows are a gentle way to learn
mise itself.

## Status

powermise is an early release for macOS, Windows, and Linux. Installers aren't
code-signed yet, so the first launch takes one extra click — see
[Getting started](/getting-started/#opening-it-the-first-time).

[Get started →](/getting-started/)
