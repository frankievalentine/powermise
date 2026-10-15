---
title: The mise boundary
description: Powermise writes a dedicated mise environment and runs a real mise bootstrap; your global config stays yours.
---

Powermise integrates with an installed `mise` executable. It does not fork
mise, embed its internals, or overwrite your global configuration.

## How configuration is owned

For a selection named `engineering-agents`, the compiler emits a dedicated
mise environment file (a `config.powermise-engineering-agents.toml` style
artifact) under your mise config dir. `apply` writes that file, records
ownership in a ledger, and then runs a real `mise bootstrap --yes` so mise
itself installs the selection's tools and packages.

Config-adapter managed blocks (editor and shell integrations) are written as
marker-guarded sections, so they are identifiable and removable. Your
user-owned `~/.config/mise/config.toml` is never rewritten.

## Current boundary

| Resource | Behavior |
| --- | --- |
| `~/.config/mise/config.toml` | User-owned and untouched |
| Generated mise environment file | Written by `apply` into the mise config dir |
| Config-adapter managed blocks | Written as marker-guarded, removable sections |
| Tool and package installation | Performed by a real `mise bootstrap --yes` |
| Harness and integration configuration | Applied as managed blocks by `apply` |
| Ownership ledger and run report | Recorded by `apply` |
| Drift and diagnostics | Reported by `status` and `doctor` |

## Previews stay read-only

These commands never write state:

```sh
powermise selection compile --selection engineering-agents.toml --target darwin
powermise plan              --selection engineering-agents.toml --target darwin
powermise doctor            --name engineering-agents --target darwin
```

`compile` prints the mise TOML; `plan` shows the generated config diff plus the
native and mise dry-runs; `doctor` reports selection, config, drift, ledger, and
mise-availability findings.

## Convergence is real but safe

`apply` runs a real `mise bootstrap`, so it installs tools and packages. It is
guarded by an explicit confirmation (or `--yes` for non-interactive terminals),
is idempotent across repeats, and records a failure boundary instead of
rolling back. Use `--skip-bootstrap` to write and declare configuration without
installing. See [Safety](/ai-setups/safety/) for the full guardrail set.
