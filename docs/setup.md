# Clean-Machine Setup (AI Setups CLI)

How to build the `powermise` CLI and converge a selected AI workstation on a
fresh machine. For the desktop app, see the
[getting started guide](../apps/web/src/content/docs/getting-started.md).

## Prerequisites

- Rust toolchain (`cargo`, tested against 1.93)
- `mise` installed and on `PATH` (the integration pins a minimum supported
  version; check the release notes before relying on new bootstrap flags)
- `git` (not required at runtime)

## Build

```sh
cargo build --release
cargo test --workspace      # optional: run the full suite
```

The binary is `target/release/powermise`.

## Create or import a selection

Interactive (browse the catalog, pick harnesses):

```sh
powermise selection interactive --name my-agent
```

From an existing selection file:

```sh
powermise selection import --selection my-agent.toml
powermise selection list
```

## Preview

Never writes state:

```sh
powermise plan --selection selections/my-agent.toml --target darwin
powermise doctor --name my-agent --target darwin
```

`plan` shows the generated mise config diff and the native dry-run;
`doctor` reports selection, config, drift, ledger, and mise availability
findings and exits non-zero on errors.

## Converge

```sh
powermise apply --name my-agent --target darwin          # interactive confirmation
powermise apply --name my-agent --target darwin --yes    # unattended (scripts, CI)
powermise status --name my-agent --target darwin
```

- `apply` writes the generated config, records ownership, then runs a real
  `mise bootstrap --yes` against your mise config dir — this installs the
  selection's tools and Homebrew packages on the machine. It then applies
  config-adapter managed blocks, executes native operations, verifies each
  operation, persists a run report, and lists anything mise still owes as
  follow-up actions.
- `apply` requires an explicit `--yes` on non-interactive terminals; a piped
  or scripted run without it is refused before anything is written. Use
  `--skip-bootstrap` to write and declare everything without installing
  (follow-ups then show what a later real bootstrap would converge).
- A failed bootstrap never rolls back completed work: the run report records
  the failure boundary, independent config-adapter/native operations still
  run, and re-running `apply` is idempotent.
- `status` reports change/drift/ledger/run-report state plus real
  `mise bootstrap status` follow-ups; it exits non-zero while anything is
  pending.

## Use the environment

Per command:

```sh
powermise exec --name my-agent --target darwin -- mise --version
```

Persistent (opt-in, marker-guarded, removable):

```sh
powermise shell install --name my-agent --shell zsh         # or bash
powermise shell uninstall --name my-agent --shell zsh       # dry-run; --yes to apply
```

PowerShell (pwsh) is supported on every operating system; the managed block
goes into the pwsh all-hosts profile at `~/.config/powershell/profile.ps1`:

```sh
powermise shell install --name my-agent --shell powershell
```

## Lazy-install stubs (optional)

Stubs are the cross-OS equivalent of Omarchy's lazy `~/.local/bin` launchers:
one small script per mise-managed tool that runs the tool through the
selection's mise environment and installs it on first use. Nothing is
installed up front — the first invocation of `claude`, `rg`, or `jq` resolves
the version from the selection's generated configuration and installs it
through mise. Activate (or apply) first, because stubs execute against the
generated configuration:

```sh
powermise selection activate --name my-agent --target darwin
powermise stub install --name my-agent --target darwin          # writes ~/.local/bin/<command>
powermise stub install --name my-agent --target darwin --bin-dir ~/bin
powermise stub remove --name my-agent                            # dry-run
powermise stub remove --name my-agent --yes                      # removes stubs + ownership
```

- One stub per tool whose resolved provider is a mise tool; Homebrew/winget
  provider tools are real `PATH` binaries and get no stub.
- On `win32` targets the stubs are `.cmd` scripts in the bin directory.
- Stubs are ledger-owned: `uninstall` and `prune` remove them together with
  the rest of a selection's artifacts, and Powermise refuses to overwrite a
  stub file it does not own.

## Desktop companion apps

Harnesses are CLI-first: the required tooling of a harness always resolves on
every platform the harness claims. Desktop companions follow two modes:

- **Desktop-first harnesses** (`desktop_required = true`: conductor, cursor,
  superset, t3code, zed) install their desktop app by default when the
  harness is selected — the app is the deliverable. Interactive setup informs
  you and asks nothing; a machine role's `exclude_tools` can still drop the
  app.
- **CLI-first harnesses** (claude, codex) treat the desktop app as an
  optional companion: interactive setup asks `also install desktop apps where
  available? [y/N]`, and in a selection file you set
  `include_desktop_apps = true` under `[preferences]`.

In both modes a companion joins the resolved tool set only on platforms where
a verified provider exists (mise tool, Homebrew, winget, or Scoop).
Unavailable companions are reported by `selection show` and `plan` as
`unavailable desktop apps` — never fatal, never silently skipped.

On Linux, system-level tools resolve through the distribution's `apt` channel
where declared (git, curl, jq, ripgrep); mise-tool providers stay the first
choice for version-managed tools.

## Machine transfer

Selections export and reimport identically:

```sh
powermise selection export --name my-agent > transfer.toml
# on the new machine:
powermise selection import --selection transfer.toml
```

Re-running `apply` on the new machine reproduces byte-identical generated
config and manifests; repeated `apply` is convergent, not destructive.

## Safety commands

```sh
powermise prune                # dry-run: lists ledger-owned candidates only
powermise prune --yes          # remove stale generated configs/manifests
powermise uninstall --name my-agent            # dry-run
powermise uninstall --name my-agent --yes      # remove generated config + manifest + ownership
```

Dry-runs create no state, config, or ledger artifacts. Unmanaged
`config.powermise-*.toml` files are reported and never touched.

## Output formats

```sh
powermise status --name my-agent --output json   # schema-stable, color-free
powermise plan ... --color never                 # respect NO_COLOR in scripts
```

## Support matrix (current status)

| Target | Validation status |
| --- | --- |
| macOS (Apple Silicon) | End-to-end fixtures and CLI tests green; real clean-machine apply is an open acceptance gate |
| Linux | Fixture-validated; `apt:` bootstrap channel declared for base tools; not yet clean-machine tested |
| Windows (`win32`) | Fixture-validated dry-runs only; winget/Scoop IDs source-verified against winget-pkgs and Scoop manifests; no real Windows package installs are claimed until clean-machine evidence exists |
| WSL | Fixture target exists; treated as a distinct Linux target sharing the apt channel, not native Windows |

## Catalog versioning policy

Every catalog provider pins an explicit resolved version (validated —
`latest` is rejected except for `bootstrap-apt`, which tracks the
distribution's own channel). Versions were last resolved 2026-08-31 against
mise `ls-remote`, Homebrew, npm, winget-pkgs manifests, and Scoop manifests
(`catalog_revision = "2"`). Updating pins is a deliberate catalog revision,
which changes the generated configuration content and therefore surfaces as
drift/plan updates on converged machines.

Before a target appears in the matrix as "clean-machine verified", it must pass
the acceptance gates: user-owned global mise config survives byte-for-byte, a
generated config is deterministic and atomically replaced, plan mutates nothing,
repeated apply converges, and every shipped asset has explicit platform and
ownership behavior.

## Layout

See [Coexistence](coexistence.md) for the full ownership map. In short:
selections and state under `~/.config/powermise/`, generated config as a
separate environment file under the mise config dir, user-owned mise config
untouched.