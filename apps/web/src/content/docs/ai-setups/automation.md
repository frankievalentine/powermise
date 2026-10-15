---
title: Automation & CI
description: Drive powermise from scripts and CI with stable JSON, explicit --yes, and predictable exit codes.
---

powermise is built to be automated. Three features make that safe: a
schema-stable JSON output, an explicit opt-in for non-interactive convergence,
and predictable exit codes.

## Stable JSON output

Every command accepts `--output json`. The result is a color-free envelope:

```json
{
  "command": "apply",
  "schema_version": 1,
  "payload": { "...": "command-specific fields" }
}
```

The envelope shape (`command`, `schema_version`, `payload`) is a contract; the
`payload` schema is versioned via `schema_version`. Scripts should parse the
envelope, not scrape table text.

```sh
powermise status --name my-agent --target darwin --output json --color never
```

`--color never` (or respecting `NO_COLOR`) keeps output free of terminal
escape codes in logs.

## Non-interactive convergence requires --yes

`apply` prompts for confirmation on an interactive terminal. On a
non-interactive terminal — a script, CI job, or piped input — it **requires**
`--yes` and refuses to run without it. This guarantees a real `mise bootstrap`
(which installs tools and packages) can never start by accident.

```sh
powermise apply --name my-agent --target darwin --yes
```

Use `--skip-bootstrap` when you only want powermise to write and own the
configuration in automation, deferring the real install:

```sh
powermise apply --name my-agent --target darwin --skip-bootstrap --yes
```

## Predictable exit codes

| Command | Exits non-zero when |
| --- | --- |
| `registry validate`, `selection validate` | validation fails |
| `doctor` | any error-severity finding |
| `status` | anything is pending (not satisfied) |
| `exec` | the executed command fails |
| `apply` | any operation failed or the bootstrap failed |

All other commands exit `0` on success. Dry-runs (`plan`, `prune`,
`uninstall`, `shell uninstall`) create no state and still report `0` when the
dry-run succeeds.

## A minimal CI shape

```sh
# 1. Validate the catalog and the selection up front.
powermise registry validate
powermise selection validate --selection ci.toml --target linux

# 2. Preview without writing anything.
powermise plan --selection ci.toml --target linux --output json

# 3. Converge, explicitly, and fail the job on any pending state.
powermise apply --name ci --target linux --yes
powermise status --name ci --target linux --output json
```

Because `status` exits non-zero while anything is pending, the final check
makes the job fail if convergence did not complete.
