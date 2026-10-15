---
title: Selection file format
description: Field-by-field reference for the powermise selection TOML.
---

The file powermise saves for each AI setup, in
`~/.config/powermise/selections/<name>.toml`. You don't need to edit it — the
setup editor does — but it's plain TOML if you want to.

```toml
schema_version = 2
name = "engineering-agents"
catalog = "powermise-profiles"
harnesses = ["claude", "codex", "gemini"]

[preferences]
include_desktop_apps = false
```

## Fields

| Field | Type | Required | Notes |
| --- | --- | --- | --- |
| `schema_version` | integer | yes | Currently `2`. Version `1` files still load. |
| `name` | string | yes | Lowercase letters, numbers, and single hyphens. Must match the file name, and names the mise file: `conf.d/powermise-<name>.toml`. |
| `catalog` | string | yes | The catalog this selection resolves against: `powermise-profiles`. |
| `harnesses` | list of strings | yes | Curated harness IDs, for example `claude`, `codex`, `gemini`. |
| `preferences.include_desktop_apps` | bool | no | Default `false`. Adds each harness's desktop companion apps where the platform has a provider. |

## Version 1 files

Selections written by the retired powermise CLI are `schema_version = 1` and
may also contain `machine_roles`, `preferences.auto_prune`, and
`preferences.update_system_packages`. They still load: powermise reads them as
version 2, ignores those fields, and writes version 2 the next time the setup is
saved. Tool cleanup is now `mise prune --tools`, and package updates are mise's
(see [From the terminal](/ai-setups/terminal/)).

A file with a newer `schema_version` than this powermise knows is skipped
rather than misread.

See [Selections](/ai-setups/harness-selections/) for how a selection resolves.
