---
title: Selection schema
description: Field-by-field reference for the Powermise selection TOML.
---

A selection is a single TOML document. It is validated against the catalog
before it can be compiled or applied. All fields are shown below with their
defaults.

```toml
schema_version = 1
name = "engineering-agents"
catalog = "powermise-profiles"
harnesses = ["claude", "codex", "gemini"]

[machine_roles.macbook]
include_apps = ["cursor"]
exclude_harnesses = []
exclude_tools = []

[preferences]
auto_prune = false
update_system_packages = false
```

## Top-level fields

| Field | Type | Required | Notes |
| --- | --- | --- | --- |
| `schema_version` | integer | yes | Currently `1`. |
| `name` | string | yes | Selection identifier, used by the lifecycle commands after import. |
| `catalog` | string | yes | Catalog label this selection resolves against (`powermise-profiles`). |
| `harnesses` | list of strings | yes | Curated harness IDs to include (for example `claude`, `codex`, `gemini`). |
| `machine_roles` | map | no | Optional per-role asset adjustments (see below). |
| `preferences` | table | no | Convergence options (see below). |

## machine_roles.<role>

A map keyed by role name. **At most one** role is applied at a time (chosen
with `--machine-role` on the command line). Each role may:

| Field | Type | Default | Notes |
| --- | --- | --- | --- |
| `include_apps` | list of strings | `[]` | Curated apps to add for this role. |
| `exclude_harnesses` | list of strings | `[]` | Harness IDs from the selection to drop. |
| `exclude_tools` | list of strings | `[]` | Specific tools to drop. |

```toml
[machine_roles.macbook]
include_apps = ["cursor"]
exclude_harnesses = []
exclude_tools = []
```

## preferences

| Field | Type | Default | Notes |
| --- | --- | --- | --- |
| `auto_prune` | bool | `false` | When true, allow pruning of stale owned artifacts during convergence. |
| `update_system_packages` | bool | `false` | Permit native/system package updates as part of convergence. |

## Resolution rules

1. Every requested harness ID must exist in the catalog.
2. Harness tool references must exist in the tool catalog.
3. Exactly one optional machine role may add or exclude curated assets.
4. Unsupported target-platform providers are reported as validation errors.
5. Compilation emits a deterministic mise configuration only after validation
   succeeds.

See [Harness selections](/harness-selections/) for the concepts and
[Targets](/targets/) for how providers are chosen per platform.
