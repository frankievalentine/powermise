---
title: Troubleshooting
description: Common powermise errors and how to resolve them.
---

A reference for the failures you are most likely to hit, and the exact fix for
each. powermise is unofficial: if something only goes wrong in powermise, report
it on the [powermise issue tracker](https://github.com/frankievalentine/powermise/issues)
rather than to mise.

## The app says "mise not found"

powermise looks for mise on its `PATH`, plus `~/.local/bin`,
`~/.local/share/mise/bin`, `/opt/homebrew/bin`, and `/usr/local/bin`. If you
installed mise somewhere else, symlink it into `~/.local/bin`:

```sh
ln -s "$(command -v mise)" ~/.local/bin/mise
```

## Tools install in the app but not in my terminal

Your shell isn't activated. Open **Health** — it shows the exact line to add
to your shell's startup file — then open a new terminal.

## An install fails with a GitHub rate limit

Many tools download from GitHub releases, which limits anonymous downloads.
Check which token mise is using:

```sh
mise token github
```

On mise 2026.10 or newer, `mise token github --oauth` signs in with your GitHub
account and stores the token for you.

## My mise settings are ignored in the app

Apps opened from the Dock don't read `~/.zshrc`, so exported `MISE_*` variables
don't reach powermise. Move them to `[settings]` in `~/.config/mise/config.toml`.
See [How powermise uses mise](/how-it-works/#settings-in-your-shell-files).

## Health says my mise is too old, or recommends an update

Click **Update** on the Health screen, or run `mise self-update`. powermise
needs mise 2026.7.18 or newer and recommends 2026.10 or newer; on versions in
between it works, with a few newer features switched off.

## AI Setups

### Apply is greyed out

AI Setups need mise 2026.10 or newer, because they rely on mise's `conf.d`
folder and `mise bootstrap --only`. The setup's page says which version you
have; update mise from **Health**.

### The setup page says "your config wins"

Your own mise config pins the same tool, and mise uses your version — the
setup never overrides it. That's expected. To use the setup's version instead,
remove the tool from your config (`mise unuse -g <tool>`, or edit
`~/.config/mise/config.toml`).

### Apply also installs things that aren't in the setup

`mise bootstrap` installs everything your mise config asks for, including
`[bootstrap.packages]` in your own config. The review sheet lists those before
you apply. If you don't want them, remove them from your config first.

### A package fails to install

The output names the package manager that failed. Common causes:

- **Homebrew isn't installed** (macOS, Linux): install it from
  [brew.sh](https://brew.sh), then apply again.
- **winget or Scoop isn't available** (Windows): install one, then apply again.
- **apt needs your password** (Linux): run the apply command from a terminal,
  where `sudo` can ask for it:

```sh
mise bootstrap --yes --only packages,tools
```

### A harness can't be picked for this platform

Some harnesses don't support every platform, or need a tool with no provider
there. The [harness reference](/ai-setups/harness-reference/) lists where each
one works.

### I removed a setup, but its tools are still installed

Removing a setup deletes its mise config file; what it installed stays. Click
**Clean Up Unused Tools** after removing, or run `mise prune --tools`. Remove
packages with their package manager (`brew uninstall`, for example).

### I edited the generated file and my changes disappeared

`conf.d/powermise-<name>.toml` is rewritten whenever the setup is applied. Put
your changes in your own `~/.config/mise/config.toml` — it takes precedence.
