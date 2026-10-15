---
title: Harness selections
description: Selections reference curated powermise harness profiles, not generic profiles.
---

A powermise selection is a portable declaration for a focused AI harness set.
The selected profile IDs resolve their declared tools for a target platform; a
selection does not duplicate those definitions.

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

## Fields

- `schema_version` — `1`.
- `name` — the selection's identifier, used by `apply`, `status`, `doctor`,
  and friends after the selection is imported into your store.
- `catalog` — the catalog label this selection resolves against
  (`powermise-profiles` by default).
- `harnesses` — the curated harness IDs you want (for example `claude`,
  `codex`, `gemini`).
- `machine_roles.<role>` — optional, at most one. Adds or excludes curated
  assets for a role (apps to include, harnesses or tools to exclude).
- `preferences` — convergence options such as `auto_prune` and
  `update_system_packages`.

## Resolution rules

1. Every requested harness ID must exist in the catalog.
2. Harness tool references must exist in the tool catalog.
3. Exactly one optional machine role can add or exclude curated assets.
4. Unsupported target-platform providers are reported as validation errors.
5. Compilation emits a deterministic mise configuration only after validation
   succeeds.

## Storing and reusing a selection

`selection validate`, `selection compile`, and `plan` read a selection file
directly with `--selection <path>`. The lifecycle commands — `show`,
`activate`, `diff`, `apply`, `status`, `doctor`, `exec`, and `uninstall` —
operate on a selection you have placed in your store with `import` or
`create`:

```sh
powermise selection import --selection engineering-agents.toml
powermise selection list
powermise selection show --name engineering-agents --target darwin
```

Selections export and reimport identically, so a selection moves between
machines unchanged:

```sh
powermise selection export --name engineering-agents > transfer.toml
# on the new machine:
powermise selection import --selection transfer.toml
```

The selection file is user-editable and reviewable. Ownership, drift, and
lifecycle are real features of the current CLI: `apply` writes the generated
config and a ledger, and `status`/`doctor` report drift against them.
