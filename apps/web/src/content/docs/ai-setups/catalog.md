---
title: The curated catalog
description: powermise validates versioned harness and tool profiles, then resolves and converges them.
---

The committed powermise catalog contains versioned profiles: harnesses and the
tools, apps, integrations, and MCP servers they declare. Registry validation
checks the catalog structure and verifies that every reference resolves.

```sh
powermise registry validate
```

A selection is eligible for compilation only when powermise can:

- Resolve its requested harnesses for the target platform.
- Find a compatible mise provider for every selected tool.
- Emit a deterministic configuration without package conflicts.

## What the catalog contains

- **Harnesses** — curated AI harness profiles (for example `claude`, `codex`,
  and `gemini`). Each declares the tools it expects.
- **Tools** — versioned tool definitions with a compatible mise provider per
  target platform.
- **Apps** — curated applications a machine role may include (referenced from
  a selection's `machine_roles.<role>.include_apps`).
- **Integrations** — editor and shell integrations applied as config-adapter
  managed blocks.
- **MCP servers** — MCP server definitions attached to a harness.

You choose *which* curated harnesses to include; you do not redefine how they
are installed. The catalog is the single source of truth for what a harness
needs on a given target.

## From catalog to machine

`registry validate` checks the catalog. `selection validate` checks your
selection against it. `plan` and `compile` turn a valid selection into mise
configuration, and `apply` writes that configuration, records ownership, and
runs a real `mise bootstrap` to install the selection's tools and packages.

The catalog does not mutage your machine on its own — installation happens only
when `apply` runs a real `mise bootstrap` (or when you preview the native
commands via `plan`). Review `plan` and `doctor` before converging.

See [Harness selections](/ai-setups/harness-selections/) for how to author a selection
against this catalog, and [The mise boundary](/ai-setups/mise-boundary/) for how the
generated configuration is owned and applied.
