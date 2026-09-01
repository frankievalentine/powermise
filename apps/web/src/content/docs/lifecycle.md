---
title: Lifecycle & ownership
description: How Powermise stores selections, owns generated config, tracks drift, and converges idempotently.
---

Powermise does more than print config: it owns what it writes, tracks drift
against what it applied, and converges repeatably. This page explains the
lifecycle model.

## Store and layout

- **Selections and state** live under `~/.config/powermise/` (override with
  `--powermise-dir`). `selection import` / `create` / `interactive` write here.
- **Generated config** is written as a dedicated environment file under your
  mise config dir (override with `--mise-config-dir`).
- **Config-adapter managed blocks** (editor and shell integrations) live under
  `<powermise-dir>/adapters` (override with `--adapters-config-dir`).
- **Your user-owned `~/.config/mise/config.toml` is never rewritten.**

See [Coexistence](/coexistence/) for the full ownership map.

## Declare vs. converge

Two commands write configuration; only one installs.

- `selection activate --name <name> --target <t>` writes the generated config
  into the mise environment and records ownership, **without** running a
  bootstrap. Use it when you want Powermise to own and emit the configuration
  but not install anything yet.
- `apply --name <name> --target <t>` does the same write **and** runs a real
  `mise bootstrap --yes`, applies managed blocks, executes native operations,
  verifies each one, persists a run report, and lists follow-up actions.

`apply --skip-bootstrap` is equivalent to the converge path minus the real
install: configuration is written and owned, and the follow-ups show what a
later real bootstrap would do.

## Idempotent convergence

Re-running `apply` reproduces the same generated config and is not destructive.
It converges toward the declared state rather than resetting the machine. A
failed `mise bootstrap` does **not** roll back completed work:

- the run report records the failure boundary;
- independent config-adapter and native operations still run;
- `status` lists what mise still owes as follow-up actions;
- re-running `apply` resumes from the recorded state.

## Drift and verification

- `status` reports the generated config change, any drift from the last applied
  state, ledger ownership, and the run report.
- `diff` shows the generated config change and drift for a stored selection.
- `doctor` reports selection, config, drift, ledger, and mise-availability
  findings (exits non-zero on errors).

## Tear down

- `uninstall --name <name>` (dry-run) / `--yes` removes the generated config,
  manifest, and ownership record.
- `prune` (dry-run) / `--yes` removes stale ledger-owned artifacts only.
  Unmanaged `config.powermise-*.toml` files are reported and never touched.

## Machine transfer

Selections export and reimport identically, so a selection moves between
machines unchanged:

```sh
powermise selection export --name my-agent > transfer.toml
# on the new machine:
powermise selection import --selection transfer.toml
powermise apply --name my-agent --target darwin --yes
```

Re-running `apply` on the new machine reproduces byte-identical generated
config and manifests.
