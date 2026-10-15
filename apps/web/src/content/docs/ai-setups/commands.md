---
title: Commands reference
description: Every Powermise command, flag, target, and exit code.
---

This page is the complete command reference. For end-to-end workflows see
[Getting started](/ai-setups/getting-started/).

## Global flags

These apply to every command:

| Flag | Values | Default | Notes |
| --- | --- | --- | --- |
| `--color` | `auto` \| `always` \| `never` | `auto` | `auto` disables color when not a TTY. |
| `--output` | `table` \| `json` | `table` | JSON emits a stable `{ "command", "schema_version": 1, "payload" }` envelope. |

## Targets

`--target` accepts `darwin`, `linux`, `win32`, or `wsl`. See
[Targets](/ai-setups/targets/).

## Catalog override flags

Available on `registry`, `selection`, `plan`, `apply`, `doctor`, `status`, and
`exec`:

| Flag | Default |
| --- | --- |
| `--catalog` | `catalog/profiles/harnesses.toml` |
| `--tools` | `catalog/profiles/tools.toml` |
| `--apps` | `catalog/profiles/apps.toml` |
| `--integrations` | `catalog/profiles/integrations.toml` |
| `--mcp` | `catalog/profiles/mcp.toml` |

## Environment override flags

Available where a selection store or mise path is needed:

| Flag | Default | Notes |
| --- | --- | --- |
| `--powermise-dir` | `~/.config/powermise` | Powermise store root. |
| `--mise-config-dir` | resolved from `MISE_CONFIG_DIR` / `XDG_CONFIG_HOME` / `~/.config/mise` | Where generated config is written. |
| `--adapters-config-dir` | `<powermise-dir>/adapters` | Where config-adapter managed blocks live. |
| `--mise` | `mise` | Path to the mise executable. |

## registry

```sh
powermise registry validate [--catalog ...] [--tools ...] [--apps ...] [--integrations ...] [--mcp ...]
```

Validates the catalog structure and that every harness tool reference exists.

## selection

```sh
powermise selection create     --name <name> --selection <path> [env]
powermise selection import     --selection <path> [--name <name>] [env]
powermise selection list       [env]
powermise selection export     --name <name> [env]
powermise selection show       --name <name> --target <t> [--machine-role <r>] [catalog]
powermise selection validate   --selection <path> --target <t> [--machine-role <r>] [catalog]
powermise selection compile    --selection <path> --target <t> [--machine-role <r>] [catalog]
powermise selection activate   --name <name> --target <t> [--machine-role <r>] [catalog] [env]
powermise selection diff       --name <name> --target <t> [--machine-role <r>] [catalog] [env]
powermise selection interactive --name <name> [--machine-role <r>] [catalog] [env]
```

- `create` / `import` place a selection into your store (`~/.config/powermise`).
  `import` infers the name from the file when `--name` is omitted.
- `list` prints stored selection names.
- `export` prints a selection's TOML to stdout (machine transfer).
- `show` / `validate` / `compile` resolve and inspect; `compile` prints the
  generated mise TOML. None of these write state.
- `activate` writes the generated config into the mise environment and records
  ownership **without** running a bootstrap.
- `diff` shows the generated config change and any drift from the last applied
  state.
- `interactive` launches a guided builder that writes a selection into the
  store.

## plan

```sh
powermise plan --selection <path> --target <t> [--machine-role <r>] [catalog] [env]
```

Previews the generated config diff, the native dry-run commands, and the
`mise` dry-run output. Never writes state.

## doctor

```sh
powermise doctor --name <name> --target <t> [--machine-role <r>] [catalog] [env]
```

Reports selection, config, drift, ledger, and mise-availability findings.
Exits non-zero on errors.

## status

```sh
powermise status --name <name> --target <t> [--machine-role <r>] [catalog] [env]
```

Reports change/drift/ledger/run-report state plus real `mise bootstrap status`
follow-ups. Exits non-zero while anything is pending.

## exec

```sh
powermise exec --name <name> --target <t> [--machine-role <r>] -- <command...> [catalog] [env]
```

Runs a command inside the selection's environment. Everything after `--` is the
command and its arguments.

## prune

```sh
powermise prune [--yes] [env]
```

Dry-run by default: lists ledger-owned stale artifacts only. `--yes` removes
them. Unmanaged files are reported and never touched.

## uninstall

```sh
powermise uninstall --name <name> [--yes] [env]
```

Dry-run by default: shows what would be removed (generated config, manifest,
ownership). `--yes` removes it.

## apply

```sh
powermise apply --name <name> --target <t> [--machine-role <r>] [--skip-bootstrap] [--yes] [catalog] [env]
```

Converges the machine: writes the generated config, records ownership, then
runs a real `mise bootstrap --yes` (unless `--skip-bootstrap`). Requires an
interactive confirmation, or `--yes` on non-interactive terminals (a piped run
without `--yes` is refused before anything is written).

## shell

```sh
powermise shell install   --name <name> [--shell auto|zsh|bash] [env]
powermise shell uninstall --name <name> [--shell auto|zsh|bash] [--yes] [env]
```

Installs or removes a marker-guarded block in the shell startup file that
selects the selection's mise environment. Idempotent; uninstall is dry-run
unless `--yes`.

## Exit codes

| Command | Non-zero when |
| --- | --- |
| `registry validate`, `selection validate` | validation fails |
| `doctor` | any error-severity finding |
| `status` | anything is pending (not satisfied) |
| `exec` | the executed command fails |
| `apply` | any operation failed or the bootstrap failed |

Otherwise commands exit `0`. JSON output uses the same semantics.
