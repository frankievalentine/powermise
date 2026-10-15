---
title: Safety and lifecycle
description: Real installs are guarded by explicit confirmation, idempotent convergence, and an ownership ledger.
---

powermise can change your machine: `apply` writes configuration, records
ownership, and runs a real `mise bootstrap` that installs tools and packages.
That power is fenced behind deliberate guardrails.

## What can change the machine

- `apply` — writes the generated config, records ownership, and runs a real
  `mise bootstrap` (unless `--skip-bootstrap`).
- `shell install` / `shell uninstall` — add or remove a marker-guarded shell
  block.
- `prune --yes` / `uninstall --yes` — remove generated artifacts and ownership
  records.

Everything else — `registry validate`, `selection validate/show/compile`,
`plan`, `doctor`, `status`, `diff`, `exec`, and the dry-run forms of
`prune`/`uninstall`/`shell uninstall` — creates no state, config, or ledger
artifacts.

## Guardrails around apply

- **Explicit confirmation.** On an interactive terminal, `apply` prompts before
  doing anything. On a non-interactive terminal (script, CI, piped input) it
  *requires* `--yes` and refuses to run without it, so a real install can
  never happen by accident.
- **Idempotent convergence.** Re-running `apply` reproduces the same generated
  config and is not destructive; it converges rather than resets.
- **Failure boundary, not rollback.** A failed `mise bootstrap` does not roll
  back completed work. The run report records the failure boundary, independent
  operations still run, and `status` lists follow-up actions.
- **Ownership ledger.** Generated configs are recorded so `prune` and
  `uninstall` can find and remove only what powermise owns. Unmanaged
  `config.powermise-*.toml` files are reported and never touched.

## A safe workflow

1. Preview with `plan` and `doctor` — neither writes anything.
2. Converge with `apply --yes` once you have reviewed the plan.
3. Confirm with `status`; investigate drift or follow-ups with `doctor`.
4. Tear down with `uninstall --yes` and `prune --yes` when you no longer need
   the selection.

## Output you can trust in scripts

```sh
powermise status --name my-agent --output json --color never
```

The JSON envelope is `{ "command", "schema_version": 1, "payload" }`, and
exit codes are stable: `status` exits non-zero while anything is pending,
`doctor` exits non-zero on errors, and `apply` exits non-zero if any operation
or the bootstrap failed.
