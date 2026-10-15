---
title: Shell integration
description: Run a selection's environment per command or persistently through a marker-guarded shell block.
---

Once a selection is applied, you can use its mise environment two ways: a
single command via `exec`, or a persistent shell block via `shell install`.

## Per command: exec

```sh
powermise exec --name my-agent --target darwin -- mise --version
```

Everything after `--` runs inside the selection's environment. This is the
safest way to use a selection: the environment is active only for that command
and leaves your surrounding shell untouched.

## Persistent: shell install

```sh
powermise shell install --name my-agent --shell zsh   # or bash, or omit for auto-detect
```

This inserts a **marker-guarded** block into your shell startup file (for
example `~/.zshrc`) that selects the selection's mise environment. The block is:

- **Idempotent** — re-running `install` does not duplicate it.
- **Removable** — `shell uninstall` removes exactly the managed block.

```sh
powermise shell uninstall --name my-agent --shell zsh   # dry-run
powermise shell uninstall --name my-agent --shell zsh --yes
```

The marker guards mean powermise can find and remove only what it added; your
other shell configuration is left alone.

## Which to choose

- Use `exec` for scripts, CI, and one-off commands where you want the
  environment scoped to a single invocation.
- Use `shell install` for interactive daily use, when you want the selection's
  tools available in every new shell.

Both rely on the same applied environment, so they stay consistent with
`status` and `doctor`.
