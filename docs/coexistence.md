# Coexistence with User-Managed Mise Configuration

Powermise generates mise configuration for its selections. It must never own,
rewrite, or remove your global mise config.

## The ownership boundary

| Resource | Owner | Where |
| --- | --- | --- |
| Global mise config (`config.toml`) | You (user) | `~/.config/mise/config.toml` (or `$MISE_CONFIG_DIR`) — never touched |
| Generated selection config | Powermise (ledger) | `<mise-config-dir>/config.powermise-<selection>.toml` |
| Lazy-install stubs | Powermise (ledger) | `~/.local/bin/<command>` (or `--bin-dir`); never overwritten unless ledger-owned |
| Versioned manifests | Powermise | `~/.config/powermise/state/manifests/` |
| Run reports | Powermise | `~/.config/powermise/state/run-reports/` |
| Ownership ledger | Powermise | `~/.config/powermise/state/ledger.json` |
| Managed config-adapter blocks | Powermise | `~/.config/powermise/adapters/<target>/settings.json` |
| Shell activation block | Powermise (opt-in) | `~/.zshrc` / `~/.bashrc`, marker-guarded |

The generated config is a **separate environment file**. It is only loaded when
the Powermise environment is selected:

- `MISE_ENV=powermise-<selection>` (exported by `powermise shell install`), or
- `mise --env powermise-<selection> ...` (used by `powermise plan`, `apply`,
  `status`, and `exec`).

Your global `config.toml` merges as usual; the generated file never overwrites
it, and no Powermise command writes to it. This is enforced by an acceptance
gate and covered by a regression test that asserts a user-owned config survives
every command byte-for-byte.

## What apply installs

`powermise apply --yes` converges for real: after writing its own files it runs
`mise bootstrap --yes` scoped to `MISE_CONFIG_DIR` and the selection's
environment, which installs the declared tools and Homebrew packages through
mise — into mise's normal install locations, exactly as if you had run the
bootstrap yourself. It then executes native package providers (winget, Scoop,
npm) for Windows/npm-only assets. Two boundaries hold throughout:

- Powermise only ever points mise at files it generated; your global config,
  dotfiles, and repositories are converged by mise itself from your own
  configuration.
- `--skip-bootstrap` (or refusing the interactive confirmation) keeps apply in
  declare-only mode: files are written, ownership is recorded, and pending
  installs are surfaced as follow-up actions instead of being executed.

## Powermise-owned state

All Powermise state lives under the Powermise root, resolved from the first
match of:

1. `--powermise-dir <path>`
2. `$POWERMISE_HOME`
3. `$XDG_CONFIG_HOME/powermise`
4. `~/.config/powermise`

```text
~/.config/powermise/
├── selections/            # managed selection files (one per name)
├── state/
│   ├── manifests/         # persisted generated-config manifests
│   ├── run-reports/       # per-selection apply run reports
│   └── ledger.json        # ownership ledger (what Powermise may prune/uninstall)
└── adapters/
    └── <target>/          # settings.json with managed _powermise blocks
```

There is intentionally no `active-selection` pointer: a selection is always
named explicitly (`--name`) or chosen at plan time (`--selection <file>`), and
the runtime environment is the mise environment, not a Powermise global state.

## Managed config-adapter blocks

When a selection declares integrations or MCP servers, `apply` writes a managed
JSON block per operation under `_powermise.<marker>` in
`<adapters-dir>/<target>/settings.json` (e.g. `claude/settings.json`). Only the
marked keys are Powermise-owned:

- Idempotent: a second apply changes nothing when the block already matches.
- Reversible: `powermise uninstall --name <selection> --yes` and
  `powermise prune --yes` remove only ledger-owned artifacts.
- Your other settings in the same file are preserved byte-for-byte.

## Shell integration (opt-in only)

`powermise shell install --name <selection>` appends a marker-guarded block to
`~/.zshrc`, `~/.bashrc`, or the pwsh all-hosts profile
(`~/.config/powershell/profile.ps1` with `--shell powershell`):

```sh
# >>> powermise:<selection> >>>
# Managed by Powermise. Do not edit.
export MISE_ENV=powermise-<selection>
# <<< powermise:<selection> <<<
```

The PowerShell variant writes `$env:MISE_ENV = "powermise-<selection>"` inside
the same marker style.

Nothing is installed automatically. `uninstall` is dry-run by default and only
ever removes the block between the markers; surrounding user content is never
modified.

## Lazy-install stubs

`powermise stub install` writes small launcher scripts (one per mise-managed
tool) into `~/.local/bin` or an explicit `--bin-dir`. Each script runs
`mise x --env powermise-<selection> <package> -- <command> "$@"`, so the tool
installs on first use — Powermise never writes the user's global mise config
to make a stub work. Before writing a stub Powermise checks ownership:

- A byte-identical existing file converges silently.
- A differing file with a ledger record from another selection is refused.
- A differing file with no ledger record (a file of yours) is refused.

`powermise stub remove` (dry-run by default) and `uninstall`/`prune` remove
only the stubs named in the ownership ledger.

## What Powermise never does

- Never rewrites `~/.config/mise/config.toml` or any file it does not own.
- Never prunes or uninstalls a tool or config it does not own per the ledger.
- Never modifies shell startup files except the marker-guarded block that
  `powermise shell install` wrote itself.
- Never acts on unmanaged `config.powermise-*.toml` files — `prune` reports
  them as `SkipUnmanaged` and leaves them alone.