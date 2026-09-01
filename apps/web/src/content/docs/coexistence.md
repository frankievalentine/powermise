---
title: Coexistence
description: The full ownership map — what Powermise writes, where, and what it never touches.
---

Powermise is designed to live alongside an existing mise setup without taking
it over. This page is the ownership map.

## What Powermise owns

| Artifact | Location | Created by |
| --- | --- | --- |
| Stored selections | `~/.config/powermise/selections/` | `selection import` / `create` / `interactive` |
| Ownership ledger | `~/.config/powermise/` | `apply` / `activate` |
| Run report | `~/.config/powermise/` | `apply` |
| Generated mise config | `<mise-config-dir>/config.powermise-<selection>.toml` | `apply` / `activate` |
| Config-adapter managed blocks | `<powermise-dir>/adapters/` | `apply` |
| Shell integration block | your shell rc (e.g. `~/.zshrc`) | `shell install` |

The generated mise environment is named `powermise-<selection>` (for example
`powermise-engineering-agents`), and the shell block exports
`MISE_ENV=powermise-<selection>` to select it.

## What Powermise never touches

- **`~/.config/mise/config.toml`** — your user-owned global mise configuration
  is never rewritten. Powermise contributes a separate `config.powermise-*.toml`
  environment file instead.
- **Unmanaged `config.powermise-*.toml` files** — if a file matches the
  generated pattern but is not in the ledger, `prune` and `uninstall` report it
  and never remove it.
- **Your other shell configuration** — `shell install` writes only a
  marker-guarded block it can later identify and remove.

## Atomic, deterministic writes

Generated config is written deterministically for the same catalog, selection,
and target, and replaced atomically. A user-owned global mise config survives
byte-for-byte: Powermise adds an environment file beside it rather than editing
it.

## Verifying coexistence

```sh
# Your global config should be unchanged after an apply.
powermise diff --name my-agent --target darwin
powermise doctor --name my-agent --target darwin
```

`diff` shows only the Powermise-owned change; `doctor` reports config and
drift findings, exiting non-zero on errors.
