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

- **Installed** — every tool mise manages, the version you use, and which config
  file set it.
- **Discover** — search the 1,000+ tools in the mise registry and install any
  version.
- **Updates** — everything with a newer version, including tools pinned to an
  exact version. Upgrade one, or all of them.
- **Health** — `mise doctor` in plain language: your mise version, whether your
  shell is set up, and anything that needs fixing.
- **Activity** — every command powermise ran and how it went.
- **AI Setups** — install a curated AI coding environment (Claude, Codex,
  Gemini, and more) in one step. See [AI Setups](/ai-setups/).

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

powermise is pre-release. The mise screens, Projects, and AI Setups work today. The [roadmap](https://github.com/frankievalentine/powermise/blob/main/ROADMAP.md)
has the full plan.

[Get started →](/getting-started/)
