---
title: Getting started with the CLI
description: Install Powermise, build a selection, preview it, and converge a real mise environment.
---

The `powermise` CLI is the terminal side of [AI Setups](/ai-setups/). It turns a small, reviewable selection file into a real, working
[mise](https://mise.jdx.dev) environment for a curated set of AI harnesses.
This guide walks from install to a converged machine.

## Prerequisites

- **mise** installed and on `PATH` (the integration pins a minimum supported
  version; check the release notes before relying on new bootstrap flags).
- **Rust** toolchain (`cargo`, tested against 1.93) to build Powermise.
- **git** to clone the repository (not required at runtime).

## Install Powermise

```sh
git clone https://github.com/frankievalentine/powermise.git
cd powermise
cargo install --path crates/powermise-cli
```

`cargo install --path` builds and places the `powermise` binary on your `PATH`.
Prefer a local build during development:

```sh
cargo build --release
# the binary is ./target/release/powermise
```

Verify the install:

```sh
powermise --help
```

## Build your first selection

A *selection* is a named set of curated harness IDs plus an optional machine
role. You can write one by hand or use the interactive builder.

**Interactive (recommended for first run):**

```sh
powermise selection interactive --name my-agent
```

It browses the catalog, lets you pick harnesses (Claude, Codex, Gemini, and
friends), and writes the selection into your store.

**From a file:**

```sh
powermise selection import --selection my-agent.toml
powermise selection list
```

A minimal selection looks like this:

```toml
schema_version = 1
name = "my-agent"
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

See [Harness selections](/ai-setups/harness-selections/) for the full schema.

## Preview before you change anything

These commands never write state:

```sh
powermise registry validate
powermise selection validate --selection my-agent.toml --target darwin
powermise selection show    --name my-agent --target darwin
powermise selection compile --selection my-agent.toml --target darwin
powermise plan              --selection my-agent.toml --target darwin
powermise doctor            --name my-agent --target darwin
```

- `selection show` prints the resolved harnesses, tools, apps, integrations,
  and MCP servers.
- `selection compile` prints the generated mise TOML for review.
- `plan` shows the generated config diff, the native dry-run commands, and the
  `mise` dry-run output.
- `doctor` reports selection, config, drift, ledger, and mise-availability
  findings, and exits non-zero on errors.

## Converge the machine

```sh
powermise apply --name my-agent --target darwin          # interactive confirmation
powermise apply --name my-agent --target darwin --yes    # unattended (scripts, CI)
powermise status --name my-agent --target darwin
```

`apply` does the real work:

1. Writes the generated config into a dedicated mise environment file.
2. Records ownership in a ledger.
3. Runs a real `mise bootstrap --yes` against your mise config dir — this
   installs the selection's tools and packages.
4. Applies config-adapter managed blocks, executes native operations,
   verifies each one, persists a run report, and lists anything mise still
   owes as follow-up actions.

Two flags shape `apply`:

- `--yes` — skip the interactive prompt. **Required on non-interactive
  terminals** (scripts, CI, piped input). A piped run without `--yes` is
  refused before anything is written, so a real install can never happen by
  accident.
- `--skip-bootstrap` — write and declare everything, but do **not** run the
  real `mise bootstrap`. The follow-ups then show what a later real bootstrap
  would converge. Useful when you only want Powermise to emit and own the
  configuration.

A failed bootstrap never rolls back completed work: the run report records the
failure boundary, independent operations still run, and re-running `apply` is
idempotent. `status` exits non-zero while anything is still pending.

## Use the environment

Run a command inside the selection's environment:

```sh
powermise exec --name my-agent --target darwin -- mise --version
```

Or make it persistent with a marker-guarded shell block (idempotent, removable):

```sh
powermise shell install   --name my-agent --shell zsh   # or bash
powermise shell uninstall --name my-agent --shell zsh   # dry-run; --yes to apply
```

## Tear down safely

```sh
powermise uninstall --name my-agent        # dry-run
powermise uninstall --name my-agent --yes  # remove generated config + manifest + ownership
powermise prune                            # dry-run: list stale ledger-owned artifacts
powermise prune --yes                      # remove them
```

Dry-runs create no state, config, or ledger artifacts. Unmanaged
`config.powermise-*.toml` files are reported and never touched.

## Output formats

```sh
powermise status --name my-agent --output json   # schema-stable, color-free envelope
powermise plan   --selection my-agent.toml --color never   # respect NO_COLOR in scripts
```

Every command accepts `--output table|json` (default `table`) and
`--color auto|always|never`. The JSON envelope is
`{ "command", "schema_version": 1, "payload" }`.

## Where things live

- Selections and state: `~/.config/powermise/`
- Generated config: a separate environment file under your mise config dir
- User-owned `~/.config/mise/config.toml`: untouched

See [The mise boundary](/ai-setups/mise-boundary/) and
[Coexistence](/ai-setups/coexistence/) for the full ownership map, and
[Safety](/ai-setups/safety/) for the guardrails around real installs.
