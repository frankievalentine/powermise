---
title: What is Powermise?
description: Powermise compiles curated AI-harness selections into real, owned mise configuration.
---

Powermise is a workstation configuration compiler for a deliberate collection
of AI harnesses. It resolves explicit Powermise profiles such as Claude, Codex,
and Gemini, then compiles, owns, and converges compatible tool providers into a
[mise](https://mise.jdx.dev) environment.

It does not create broad **personal** or **work** profiles. A selection is a
named, reviewable set of actual harness IDs and an optional machine role.

## What it does

- Validates the curated harness, tool, app, integration, and MCP registries.
- Resolves a selection for macOS (`darwin`), Linux (`linux`), Windows
  (`win32`), or WSL (`wsl`).
- Compiles deterministic mise configuration.
- Writes that configuration into a dedicated mise environment, records
  ownership in a ledger, and runs a real `mise bootstrap` to install the
  selection's tools and packages.
- Tracks drift, runs diagnostics, and supports idempotent re-apply, prune, and
  uninstall.

Your `~/.config/mise/config.toml` remains user-owned and untouched. Powermise
works through a separate generated environment file plus marker-guarded managed
blocks.

## Why a curated set

Powermise is opinionated on purpose. The catalog is a fixed, versioned set of
harnesses and their declared tools, so a selection compiles the same way for
everyone on a given target. You choose *which* curated harnesses you want; you
do not redefine how they are installed.

## How a selection becomes a machine

1. **Validate** the catalog and your selection.
2. **Show / compile / plan** to preview the resolved result and the generated
   mise config — none of these write state.
3. **Apply** to converge: write the config, record ownership, and run a real
   `mise bootstrap` (with an explicit confirmation or `--yes`).
4. **Status / doctor** to confirm the machine matches the selection.
5. **Exec / shell install** to use the environment, and **prune / uninstall**
   to tear it down.

[Get started →](/getting-started/)
