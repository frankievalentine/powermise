# Powermise Build Plan

## Status

In progress — native macOS Phase 0 environment, tool, package, dotfile, repository dry-run, repeat-apply, and status JSON evidence is recorded. Linux, Windows, and WSL evidence remains outstanding.

### Completed to date

- [x] Reframed the product around curated harness selections; removed generic profile semantics.
- [x] Added an initial provenance-pinned catalog slice for Claude, Codex, and Gemini plus the `engineering-agents` selection fixture.
- [x] Ported the initial Powermise marketing site and core documentation structure (Astro/Starlight).
- [x] Corrected generated environment discovery to mise’s documented global `config.<environment>.toml` naming convention.
- [x] Verified the isolated fixture with mise `2026.7.18 macos-arm64 (2026-07-30)`: both global and selection files are discovered; the selection contributes `bun@1.3.14` and `brew:jq`; tool and package dry runs report expected operations without mutating the global baseline.
- [x] Established the current precedence constraint: the user-owned global `config.toml` wins a conflicting `[tools]` key over `config.<environment>.toml`; selections must be additive or reject conflicting tool ownership instead of overriding the user baseline.
- [x] Captured `mise bootstrap status --json`: top-level `packages`, `repos`, `dotfiles`, `mise_shell_activate`, `macos_defaults`, `launchd`, `systemd`, `login_shell`, `tools`, and `plugin_deps`; package entries expose availability, requested/installed versions, and state, while tool entries expose requested/resolved versions, state, and installation state.
- [x] Verified a symlink-mode dotfile dry run from an isolated `HOME`, `MISE_CONFIG_DIR`, `MISE_DATA_DIR`, and `MISE_CACHE_DIR`; the fixture source is resolved relative to the generated environment configuration and the user home is not touched.
- [x] Verified a repository dry run targeting an isolated home-directory checkout. Mise reports the repository operation without cloning or mutating the target.
- [x] Applied the isolated dotfile fixture twice with `--yes --only dotfiles`: the first run created the sandboxed symlink and the second reported all files applied. The configured source and target converge, and the global baseline remains unchanged.

### Remaining Phase 0 exit gates

- [ ] Select and pin the minimum supported mise version after clean-machine verification; the observed macOS version is evidence, not a support policy.
- [ ] Run the same isolated fixture on Linux, including an `apt:` package declaration and a repeat apply.
- [ ] Run Windows and WSL target fixtures; document unavailable mise/provider paths as explicit rejections.
- [ ] Execute a controlled Apple-Silicon `brew:` apply on an isolated test machine to prove Homebrew-free package convergence, then capture repeat-apply behavior.
- [ ] Extend the compatibility report with observed `mise bootstrap status --json` compatibility assumptions and all platform outcomes.

### Phase 1 progress

- [x] Initialized the Rust workspace with dependency flow `CLI -> registry -> domain`.
- [x] Implemented TOML loading and validation for the initial imported harness catalog and explicit harness selections.
- [x] Implemented `powermise registry validate` and `powermise selection validate`.
- [x] Added a platform-rejection test: the `engineering-agents` selection rejects Codex on native Windows because the imported manifest does not support `win32`.
- [x] Implemented deterministic role-aware selection resolution, including inherited harnesses, deduplicated tools, role exclusions, and machine-role apps; `powermise selection show` renders the resolved result.
- [x] Added a provenance-pinned tool-catalog slice covering every required tool of the curated harness set. Registry validation now rejects dangling harness-to-tool references.
- [x] Imported and structurally validated 18 migration-source harness manifests. They are being promoted into the versioned Powermise profile catalog; the final runtime must not depend on migration-source naming or paths.
- [x] Completed Powermise-native profiles with provider mappings for tools, integrations, MCP servers, and applications. The curated catalog now covers 24 typed tools, 13 MCP servers, and 4 integrations with config-adapter provider mappings plus machine-role apps; registry validation rejects dangling harness/role references and unsupported provider mappings before planning, and `selection compile`/`plan` emit config-adapter operations for resolved integrations and MCP servers.
- [x] Replaced source-file-only tool entries with 24 typed Powermise tool definitions. Each profile requirement resolves to either platform-specific provider mappings or an explicit verified unsupported reason; Conductor and Hermes Agent have no safe package-manager mapping and must fail during compile.
- [x] Added the deterministic `powermise-compiler` crate and `powermise selection compile`. It serializes target-compatible mise and Homebrew bootstrap entries from a resolved selection, prefers mise tool providers, and rejects missing or unsupported providers.

### Phase 2 progress

- [x] Added `powermise-state` atomic generated-file persistence with SHA-256 fingerprints, unchanged-write detection, same-directory temporary files, durable writes, and focused failure-preservation tests.
- [x] Added the mutation-free top-level `powermise plan` command. It resolves and validates the selection, compiles the owned artifact and operation graph, reports create/update/unchanged diffs, runs the installed `mise` bootstrap dry-run in an isolated temporary HOME/XDG/config root, streams stdout and stderr, and removes the sandbox on success or failure. Focused CLI coverage proves the requested config directory and existing user files remain unchanged.
- [x] Defined versioned, serializable plan and run-report schemas plus a provider-aware operation graph in the domain crate. Graph construction rejects duplicate IDs, missing dependencies, and cycles; topological ordering is deterministic. The compiler now emits ordered mise-tool and Homebrew-bootstrap operations with explicit ownership, verification, and declaration-only reversibility metadata.
- [x] Added deterministic generated configuration artifacts with safe environment/file naming, source and content SHA-256 headers, versioned manifests, and atomic convergent config/manifest writes. `selection compile` now emits the owned artifact format rather than unmarked TOML.
- [x] Corrected Homebrew bootstrap serialization to mise's `[bootstrap.packages]` source-qualified package keys and modeled formula and cask providers separately. The curated macOS mappings now emit `brew:` or `brew-cask:` as appropriate and pass a real `mise bootstrap --dry-run`.
- [x] Added a versioned ownership ledger with deterministic JSON persistence. It records provider/resource/requested-state metadata only for explicitly verified Powermise-owned operations, rejects external ownership and unsupported schema versions, and converges on repeat writes.

### Phase 3 progress

- [x] Defined the domain ports `NativeInstaller` and `ConfigMutator` with typed check/plan/apply semantics plus managed-block, snapshot, drift, and restore contracts in the domain crate.
- [x] Added the `powermise-providers` crate with exact-ID noninteractive `winget`, Scoop, and global-npm installers, a pinned-immutable-source SHA-256 `verified-script` provider, a deterministic fake installer for tests, and exact dry-run/check command rendering.
- [x] Added typed JSON and comment-markered managed-block config adapters with apply/plan/check/restore, applied-fingerprint drift detection, and idempotent converges; added concrete Claude Code and VS Code adapters plus a Claude MCP-server entry model. Provider contract tests cover idempotence, drift detection, and managed-block-only restore.
- [x] Routed native-only tools and apps in the compiler: `compile_mise` skips installables that only have a native provider for the target, and `compile_plan` emits ordered `native-winget`/`native-scoop` operations with command verification and not-automated reversibility. Verified with `powermise plan --target win32 --selection selections/windows-fixture.toml`, which unifies mise tools, `native-winget:Git.Git`, `native-winget:curl.curl`, and config-adapter operations in one dry-run.
- [x] Added the `windows-fixture` selection (base + gemini + opencode) as the Windows-plan fixture; all resolved harness tools, integrations, and MCP servers have `win32` providers.

### Phase 4 progress

- [x] Added `MiseRunner::bootstrap_status` and parsed `mise bootstrap status --json` package-by-source and tool states with `follow_up_actions()` for concrete convergence hints; `powermise-mise` implements it via a sandboxed `mise --env <env> bootstrap status --json` and a `mise --version` probe for `doctor`.
- [x] Added the `powermise-lifecycle` crate: `PowermiseLayout` resolution (`--powermise-dir` / `POWERMISE_HOME` / `XDG_CONFIG_HOME/powermise` / `~/.config/powermise`), shared `CatalogPaths`/`compile_context`, a managed `SelectionStore` (create/list/read/export/import/remove), and shared `LifecycleError`.
- [x] `selection activate` compiles a managed selection and atomically writes the generated mise configuration, versioned manifest, and a `generated-config:<selection>` ledger ownership entry (provider `Generated`, `GeneratedFile`, requested state = content SHA-256). Idempotent and manifest-fingerprint-verified.
- [x] `selection diff` reports the desired create/update/unchanged change plus drift detection between the on-disk configuration and the persisted manifest content fingerprint; never writes.
- [x] `doctor` reports severity-ordered findings (selection presence, compilability, config/manifest drift, ownership ledger, mise availability) with an error exit code when error-severity findings exist; never writes.
- [x] `prune` and `uninstall` operate only over ledger-owned `Generated` artifacts: `prune --dry-run` (default) and `prune --yes` remove stale generated configs/manifests whose selection is no longer managed, report unmanaged `config.powermise-*.toml` as `SkipUnmanaged` and never touch them, and only write the ledger outside dry-run; `uninstall` (dry-run by default, `--yes` to apply) removes a named selection's generated config, manifest, and ownership entry.
- [x] `apply` converges a managed selection: writes generated config + manifest + ledger, applies every config-adapter operation as a `_powermise.<resource>` managed JSON block under `<adapters-dir>/<target>/settings.json` (idempotent, restorable), executes native winget/scoop/npm operations through their providers (verified-script skipped pending a pinned source), surfaces mise `bootstrap_status` follow-ups, persists a JSON `RunReport` to `state/run-reports/<name>.json`, and emits failure-boundary recovery guidance on any failed operation.
- [x] Added the CLI surface: `selection create|list|show|export|import|activate|diff` (plus existing validate/compile) and top-level `plan`, `doctor`, `prune`, `uninstall`, `apply` with `--powermise-dir`, `--mise-config-dir`, and `--adapters-config-dir`.
- [x] End-to-end lifecycle tests over isolated temporary layouts: cross-machine transfer reproduces identical generated bytes and manifests; activate→diff(unchanged)→prune(dry-run, no-op)→uninstall(dry-run, no-op; real removes config+manifest+ledger) stays ledger-scoped; `apply` writes the run report, records ownership, and surfaces mise follow-ups with a clean `doctor`.
- [x] Full `cargo test --workspace` (55 tests) green and `cargo clippy --workspace --all-targets` clean.

### Phase 5 progress

- [x] Added `MiseRunner::exec` (and `MiseExecutable::exec`) that runs a command inside a selection's mise environment with inherited stdio and a validated environment name; the CLI `powermise exec --name <selection> -- <command...>` propagates the child exit status (non-zero mapped to the CLI failure code).
- [x] Added the combined `powermise status --name <selection>` command: it reports the desired generated-config change, manifest drift, ownership-ledger state, run-report presence, real `mise bootstrap status` follow-ups, and native provider presence (winget/scoop/npm checks), and exits non-zero while any follow-up or drift remains.
- [x] End-to-end lifecycle tests for `status` (converged selection reports satisfied once mise follow-ups clear) and `exec` (resolves the selection environment and reports success); full workspace suite green and clippy clean.
- [x] Built the CLI presentation layer (`crates/powermise-cli/src/presentation.rs`): global `--color auto|always|never` (honors `NO_COLOR`, auto = TTY && NO_COLOR unset) and `--output table|json`; aligned, header-ruled tables for selection lists, plan operations, and apply results; semantic status symbols; and a color-free schema-stable JSON envelope `{ "command", "schema_version": 1, "payload" }` for every subcommand with exit codes that mirror table mode (doctor errors, status pending, exec child status). Verified: `--color always` emits ANSI when piped, `NO_COLOR=1` and `--color never` stay plain.
- [x] Replaced the naive full-file TOML dump with an LCS line diff (`powermise-state::render_diff`) so `plan`/`apply` show exactly the changed lines with surrounding context (`---`/`+++` headers, `-`/`+`/` ` prefixes).
- [x] Presentation-layer coverage green: full workspace suite (17 test suites) and `cargo clippy --workspace --all-targets` clean.
- [x] Added the interactive harness-selection flow (`powermise selection interactive`): numbers the harness catalog, multi-selects harnesses by comma-separated indices or `all`, optionally collects apps for a `--machine-role`, and writes the exact same `Selection` model `selection create` produces through the managed `SelectionStore` (tested end-to-end with piped input).
- [x] Added opt-in shell integration (`powermise shell install|uninstall`): installs a marker-guarded block (`# >>> powermise:<name> >>>` … `# <<< powermise:<name> <<<`) in `.zshrc`/`.bashrc` that exports `MISE_ENV=powermise-<name>`; install is idempotent, uninstall is dry-run by default (`--yes` to apply) and never touches content outside the markers. `zsh` and `bash` only. Tests prove install/uninstall preserve surrounding user content.
- [x] Hardened rc path resolution: an explicit `home` override fully determines the startup file; `$ZDOTDIR` is only consulted during real env detection, so tests can never write outside their sandbox. (Discovered via test pollution of a real config; fish support was dropped and the polluted file removed.)
- [x] Published `docs/coexistence.md` (ownership boundary table, actual `~/.config/powermise/` layout, no `active-selection` pointer, managed config-adapter blocks, marker-guarded shell integration, and the never-touch rules), and `docs/setup.md` (clean-machine build, interactive/import, plan/apply/status/exec, shell opt-in, machine transfer, safety commands, and an honest support matrix). Every documented command was verified against the built CLI.
- [x] Added TTY progress states (completes Phase 5 item 4): `apply` threads an optional progress reporter through its phases (write config, record ledger, per-operation apply, persist run report); the CLI renders these as human-readable lines on stderr only when stdout is a terminal and output is the table format (never in piped or `--output json` runs). Verified: progress streams under a pty, stays silent when piped.
- [x] Hardened test temporaries against parallel-collision flakes: the `powermise-providers` and `powermise-lifecycle` test directory helpers now include a monotonic counter so sibling tests running in parallel threads cannot share a temp path; full workspace suite is green and stable across repeated runs.
- [x] Added the acceptance-gate #1 regression test: a user-owned `mise/config.toml` survives `activate`, `diff`, `doctor`, `status`, `apply`, `prune`, and `uninstall` byte-for-byte (lifecycle test) and `selection import`/`plan`/`apply` at the CLI boundary (CLI test); `apply` writes only its env-scoped `config.powermise-<selection>.toml`. Both pass.
- [x] Resolved the apply-convergence design tension (BUILD_PLAN "Apply" step 4 vs Phase 4's delegated convergence): `apply` now converges for real by default. After writing the generated config, manifest, and ledger it runs `mise bootstrap --yes` against the machine's mise config dir (`MiseRunner::bootstrap_apply`; real HOME/data dirs preserved, config dir pinned to the directory Powermise wrote, working dir pinned so project-local configs cannot join). Mise-owned operations are then verified post-bootstrap via sandboxed status probes ("converged by real mise bootstrap"); a failed bootstrap marks those operations failed with recovery guidance, still persists the run report, and lets independent config-adapter/native operations proceed. `--skip-bootstrap` preserves the prior declare-only behavior with follow-up surfacing, and a selection without mise-owned resources skips the bootstrap explicitly.
- [x] Added the BUILD_PLAN "Apply" step 1 confirmation gate: interactive terminals get an explicit warning plus y/N prompt (real installs disclosed); non-interactive runs are refused without `--yes` before any write. Table mode renders a `mise bootstrap:` state line, JSON gains a schema-stable `bootstrap` field (`converged|failed|skipped` with reason/message), and exit codes are non-zero when any operation or the bootstrap fails. Command shape verified against installed mise 2026.7.18; full workspace suite green and clippy clean.
- [x] Added the CI matrix (BUILD_PLAN Phase 5 item 6): `.github/workflows/ci.yml` builds, clippy-lints, and tests `powermise` on ubuntu/macos/windows with `mise` installed on the runner (the `global_config` integration test needs real `mise` for its dry-run; all other command tests stub mise or use the fake-mise harness). Hermetic fixture-driven plan tests now cover the `win32` target via `selections/windows-fixture.toml` (asserting native `winget` operations compile) and prove the distinct `wsl` target rejects a Windows-only fixture rather than treating it as native Windows.
- [x] Established the release policy (BUILD_PLAN Phase 5 item 7): `docs/release.md` declares the minimum mise compatibility policy, semantic-versioning and JSON-envelope contract, update policy, and per-target release artifacts produced from the CI matrix. The minimum mise version remains provisional until Phase 0 clean-machine validation.
- [x] Added cross-OS lazy-install stubs (ported from Omarchy's `omarchy-mise-install` pattern, adapted to the ownership model): `powermise stub install|remove` writes one launcher script per `mise-tool`-provided tool of an activated selection (`exec mise x --env powermise-<selection> <package> -- <command> "$@"` on Unix, a `.cmd` equivalent on `win32`), so tools install on first use instead of up front. Stubs are ledger-owned `Generated`/`StubScript` resources: `uninstall` and `prune` remove them with the rest of a selection's artifacts, reinstall is idempotent, and Powermise refuses to overwrite a stub file it does not own (another selection's or the user's). The catalog's mise-tool providers now declare explicit `command` names (for example `claude-code` ships `claude`, `ripgrep` ships `rg`); the compiler derives fallbacks from the package id. Verified against real mise 2026.7.18: first stub invocation lazily installs only the invoked tool through the selection environment and later invocations are instant.
- [x] Added PowerShell shell integration for win32 parity: `Shell::Powershell` writes the marker-guarded `$env:MISE_ENV = "powermise-<selection>"` block to the pwsh all-hosts profile (`~/.config/powershell/profile.ps1` on every OS), with the same idempotent install and marker-scoped dry-run uninstall guarantees as zsh/bash; `Shell::detect` defaults to PowerShell on Windows and recognizes `pwsh`/`powershell` in `$SHELL`.
- [x] Added the `bootstrap-apt` provider kind serialized as `apt:<package>` in mise's `[bootstrap.packages]` (mise natively manages apt/dnf/pacman/apk/flatpak sources), declared for git/curl/jq/ripgrep on linux/wsl with priority mise-tool > apt > brew, satisfying the Phase 0 Linux gate's `apt:` package declaration requirement.
- [x] Reworked harnesses to be CLI-first with opt-in desktop companion apps: `required_tool_ids` now carries only CLI tooling that resolves on every claimed platform, and a new `desktop_tool_ids` field carries desktop companions (claude-app, codex-app, cursor, superset, zed, conductor, t3code) installed only when the selection sets `preferences.include_desktop_apps` (interactive setup prompts when a selected harness ships one). Companions join the resolved tool set only where a verified provider exists; unavailable companions are reported through `ResolvedSelection.unavailable_desktop_ids` and `selection show`/`plan` output without failing the compile. Resolution verified: engineering-agents + desktop on darwin includes claude-app/codex-app; on linux both report unavailable while the CLI tool set converges.
- [x] Filled verified provider gaps after source-checking every claim: conductor gains its real Homebrew cask (was uninstallable everywhere), codex CLI gains the win32 `npm:@openai/codex` mise provider (npm ships win32 binaries) plus win32 claims for its config adapters, cursor gains win32 Scoop and zed gains win32 Scoop providers (Scoop manifests verified; no winget-pkgs manifest exists for either), and the codex harness/config adapters now claim win32. Fixed two wrong winget IDs found during verification: `curl.curl` is canonically `cURL.cURL`, and the ripgrep MSVC package lives under `BurntSushi.ripgrep.MSVC` as verified in winget-pkgs. Hermes stays unsupported with an accurate reason (vendor scripts only; pinned verified-script is the intended future path). cline/github-copilot platform claims narrowed to where their required vscode channel exists (darwin/win32).
- [x] Pinned the entire catalog (`catalog_revision = "2"`, resolved 2026-08-31): every tool/app provider now declares an explicit version resolved against its own channel (mise `ls-remote`, Homebrew, npm, winget-pkgs manifests, Scoop manifests), with registry validation rejecting `latest` everywhere except `bootstrap-apt` providers, which deliberately track the distribution channel. Updating pins is a catalog revision, making convergence deterministic and drift visible on machines.
- [x] Fixed a latent interactive-selection bug found while wiring the desktop prompt: `prompt_ids` returned menu index strings that were stored verbatim (`harnesses = ["3"]`), producing invalid selections; indices now map to real harness/app IDs at both call sites, and the interactive tests assert ID mapping.
- [x] Added the desktop-first harness mode: `desktop_required = true` marks harnesses whose desktop app is the deliverable (conductor, cursor, superset, t3code, zed), so the companion installs by default on selection — no opt-in prompt, still excludable via a machine role's `exclude_tools` — while claude/codex companions stay prompt-based opt-ins. Registry validation now fails loudly when a desktop-first harness claims a platform whose companion has no provider. Verified: selecting `conductor` with no preferences resolves the cask-backed app into the tool set.

## Product Decision

Powermise is a curated AI-harness workstation configuration compiler for mise.

It compiles one or more explicitly selected Powermise harness profiles — plus their declared tools, integrations, MCP servers, applications, and optional machine-role overrides — into:

1. an environment-specific mise configuration for declarative workstation convergence; and
2. a small set of Powermise-owned operations for capabilities mise does not provide, chiefly native Windows package installation and application/harness configuration.

The source of truth is the versioned Powermise profile catalog, its compatibility registry, and an explicit selection file. Migration inputs are one-time import material, not runtime dependencies. The generated mise configuration is a deterministic deployment artifact.

## Goals

- Use mise for versioned developer tools, declarative package installation, repositories, dotfiles, shell activation, macOS settings, user services, and bootstrap status/dry-run where supported.
- Preserve the user outcome: recreate a selected AI workstation on a new machine, inspect planned changes, synchronize it safely, diagnose drift, and retain machine-role overrides.
- Retain native Windows support through Powermise-owned `winget`, `scoop`, npm, first-party PowerShell, and integrity-verified script providers.
- Keep `~/.config/mise/config.toml` user-owned. Powermise must never rewrite it.
- Make every generated file attributable, atomically written, previewable, and removable without taking ownership of unrelated user configuration.
- Provide a Rust CLI with a small, testable core and a subprocess boundary to the installed mise executable.

## Non-goals

- Do not fork or embed the mise codebase.
- Do not reimplement mise’s tool-version, package-manager, dotfile, repository, shell, macOS-default, LaunchAgent, or systemd convergence engines.
- Do not silently activate or merge more than one Powermise harness selection.
- Do not use a hosted sync service in the initial release.
- Do not add destructive automatic pruning.
- Do not claim support for an asset or platform until it has an executable installer path and a verification check.

## Architecture Decision: Integrate with Mise, Do Not Clone It

### Decision

Powermise invokes a separately installed, version-gated `mise` executable. It generates mise TOML and consumes supported mise command output; it does not copy, vendor, or call mise internals as a Rust library.

### Evidence

- Mise is MIT licensed and its root crate defines a `mise` binary (`src/main.rs`), not a reusable public library crate. `src/lib.rs` is absent in the upstream repository at the investigated revision.
- Mise bootstrap already converges packages, repositories, dotfiles, shell activation, macOS defaults, LaunchAgents, Linux systemd user units, login-shell settings, tools, and hooks, with dry-run and status commands.
- Mise uses TOML configuration environments selected by `MISE_ENV` or `--env`; environment configuration merges with the global config.

### Consequences

**Good**

- Powermise inherits mise’s mature Rust implementation for tools and supported package managers without a permanent fork burden.
- Mise upgrades remain independent, security patches are adopted by updating the supported version range, and the Powermise binary remains small.
- The integration boundary is explicit and mockable in tests.

**Costs**

- Powermise must pin and test a minimum mise version before relying on a bootstrap capability.
- Mise output and JSON schemas used by Powermise are an external compatibility surface.
- Unsupported platforms or package backends need Powermise providers rather than a forced mise workaround.

### Rejected alternatives

| Alternative | Rejection reason |
| Fork all of mise | Duplicates a rapidly evolving large Rust binary and makes Powermise responsible for its security, tool backends, package managers, and upstream compatibility. |
| Depend on mise source as an internal Cargo crate | Upstream currently exposes a binary crate, not a stable library API. Depending on internal modules would be a de facto fork with unstable boundaries. |
| Keep the complete executor | Continues duplicate implementation of package, tool, repo, dotfile, and platform bootstrap behavior that mise already provides. |
| Generate one permanent config per harness | A curated harness selection can compose compatible harnesses and machine-role overrides. Independent harness configs risk unintended additive merging. |

## Configuration and Harness-Selection Model

### File ownership

```text
~/.config/mise/
  config.toml                              # user-owned global mise baseline
  config.powermise-<selection>.toml        # generated, active only when explicitly selected

  catalog/
    profiles/                              # Powermise-owned harness profile catalog
  selections/
    <selection>.toml                       # curated harness selection and machine-role overrides
  state/
    active-selection.toml                  # Powermise-owned selection pointer
    ledger.json                            # ownership, snapshots, fingerprints
    runs/<timestamp>.json                  # plan/apply reports
  generated/
    <selection>.manifest.json              # compiler output and source fingerprint
```

`~/.config/mise/config.toml` remains the user’s general system configuration. Powermise writes only `config.powermise-<selection>.toml` and only by atomic rename.

### Mise environment selection

Mise natively discovers `config.<environment>.toml` in its global config directory when `MISE_ENV=<environment>` or `mise --env <environment>` is used. Powermise compiles a selection named `engineering-agents` to the `powermise-engineering-agents` environment.

Initial commands are explicit:

```bash
powermise selection list
powermise plan --selection engineering-agents
powermise apply --selection engineering-agents
powermise exec --selection engineering-agents -- mise ls --current
```

`apply` invokes `mise bootstrap --env powermise-<selection>`; `exec` invokes mise with the same environment. An optional shell integration may later export the selected environment for direct `mise` use. It must be opt-in, marked, removable, and never overwrite existing shell setup.

### Curated harness selection format

Powermise selections name actual Powermise harness profiles such as `claude`, `codex`, and `gemini`; they do not create generic “personal” or “work” profiles. Migration inputs may seed the catalog, but the committed Powermise profile manifests are the only runtime source. Selections may include only profiles and narrowly scoped, curated overrides required for a target machine.

```toml
schema_version = 1
name = "engineering-agents"
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

The compiler resolves the selected harnesses and exactly one optional machine role before generating output. Each harness brings only dependencies declared by the imported, audited catalog. It must reject unknown harness IDs, unsupported target platforms, unresolved provider mappings, conflicting configuration ownership, and invalid machine overrides.


### Generated mise configuration

A generated file contains only resources that mise can own:

```toml
# Generated by Powermise. Source selection: engineering-agents. Do not edit.
# selection_sha256 = "..."
# registry_sha256 = "..."

[tools]
node = "24"
bun = "1.3.14"

[bootstrap.packages]
"brew:gh" = "latest"
"brew-cask:visual-studio-code" = "latest"

[bootstrap.repos]
"~/src/dotfiles" = { url = "git@github.com:example/dotfiles.git", ref = "main" }

[dotfiles]
"~/.gitconfig" = { mode = "symlink" }
```

Do not encode arbitrary harness or MCP configuration as untracked shell hooks merely because mise can run hooks. Powermise must apply those configurations through an owned config adapter with before/after snapshots, a check, and a ledger entry.

## Responsibility Boundary

| Capability | Mise | Powermise |
| --- | --- | --- |
| Language and developer-tool versions | Executes `[tools]` | Resolves requested tools and emits valid TOML |
| Homebrew formulae and supported casks | Executes `brew:` / `brew-cask:` without a preinstalled Homebrew | Selects packages, detects unsupported architectures/cask forms, presents plan |
| apt, dnf, pacman, apk, flatpak, mas | Executes supported package declarations | Maps registry assets and platform policy |
| Repositories, dotfiles, shell activation | Executes declarative sections | Determines selection ownership and generated configuration boundary |
| macOS defaults, LaunchAgents, systemd user units | Executes supported declarations | Curates entries and verification checks |
| Windows package installation | No bootstrap-provider dependency | `winget`, `scoop`, npm, first-party PowerShell, and verified-script providers |
| Harness, MCP, editor, and application config | Not a mise domain | Typed configuration adapters, snapshots, drift detection, restoration |
| Harness selections and machine roles | Config environments only | Curated selection files, resolver, activation, export/import |
| Unified plan, status, doctor, diff, prune | Bootstrap-specific status | Cross-provider orchestration, ledger, policy, user-facing explanation |

## Powermise Components

Use a Rust workspace. Keep the domain layer independent from TOML parsing, filesystem access, process execution, and terminal UI.

```text
powermise/
  Cargo.toml
  crates/
    powermise-domain/         # selection, registry, plan, operation, ownership models
    powermise-registry/       # catalog loading, validation, compatibility resolution
    powermise-compiler/       # resolved selection -> mise TOML + native operation graph
    powermise-mise/           # mise executable port; TOML renderer; status/dry-run parser
    powermise-providers/      # winget, scoop, npm, verified script, config adapters
    powermise-state/          # XDG paths, atomic files, ledger, snapshots, run reports
    powermise-cli/            # clap commands, prompts, presentation, exit codes
  registry/
    harnesses/
    domains/
    tools/
    apps/
    integrations/
    mcp/
  tests/
    fixtures/
    integration/
  docs/
    adr/
```

Dependency direction:

```text
CLI -> compiler / registry / state
compiler -> domain
mise adapter + providers -> domain ports
registry + state -> domain
```

The domain defines ports for `MiseRunner`, `NativeInstaller`, `ConfigMutator`, `StateStore`, `Clock`, and `ProcessEnvironment`. Tests use fakes for all ports; process and filesystem behavior is tested separately with temporary directories.

## Plan and Apply Semantics

### Compile

1. Detect OS, architecture, shell, and supported package providers.
2. Load and validate the harness selection plus optional machine role.
3. Load and validate the registry.
4. Resolve selected assets, dependencies, exclusions, scopes, platform capabilities, and config ownership.
5. Compile supported assets to deterministic mise TOML sections.
6. Compile unsupported-by-mise assets to typed native operations.
7. Compute generated-file changes and compare Powermise ledger snapshots.
8. Produce a single operation graph with provider, ownership, verification, and reversibility metadata.

### Preview

`powermise plan` must not mutate state.

- Render the resolved harness selection and platform.
- Show generated mise-file diff.
- Run `mise bootstrap --env powermise-<selection> --dry-run` only after rendering the generated config in an isolated temporary config root or equivalent testable execution context.
- Render native-operation dry-run output from the same operation graph.
- Report unsupported assets, config conflicts, expected restarts, follow-up actions, drift, and prune candidates.

### Apply

1. Require confirmation unless `--yes` is supplied.
2. Write generated mise TOML atomically.
3. Execute only pre-mise native prerequisites that the operation graph explicitly requires.
4. Run `mise bootstrap --env powermise-<selection> --yes`.
5. Execute Powermise-owned configuration adapters and post-mise native operations.
6. Verify every claimed install/configuration check.
7. Persist run report, file snapshots, generated-config fingerprint, and ownership ledger only after successful verification of each operation.
8. Return a structured partial-failure report if a later operation fails. Never erase evidence of already-applied operations.

### Status, doctor, diff, prune, uninstall

- `status`: combine active selection, generated config fingerprint, `mise bootstrap status --json`, native provider checks, and ledger state.
- `doctor`: distinguish missing, drifted, unsupported, externally managed, and unknown state.
- `diff`: show desired harness selection against discovered provider/configuration state; never infer ownership from mere presence.
- `prune --dry-run`: list only Powermise-ledger-owned resources no longer resolved by the active selection.
- `prune --yes`: remove only after snapshot/fingerprint checks still match or explicit `--force-drift` approval.
- `uninstall --dry-run`: preview managed config blocks, generated mise configs, and ledger-owned assets; never remove the user’s global mise config or unmanaged tools.

## Registry Design

### Reuse and provenance

Create a clean catalog commit with provenance. Reuse the following concepts and data, then tailor them to the new provider model:

| Source asset | Powermise treatment |
| --- | --- |
| Upstream harness manifests and their referenced registry entries | Import as a versioned catalog snapshot with provenance. Retain stable harness IDs, descriptions, dependencies, platform declarations, and checks. Audit every referenced asset and replace installer fields with provider mappings. |
| Harness selection and machine-role semantics | Define a Powermise TOML selection schema. Preserve explicit harness inclusion plus machine-role include/exclude resolution; do not import generic upstream profiles. |
| Registry validation rules | Reimplement in Rust with `serde`/validation types. Preserve unique-ID, missing-reference, unsupported-platform, and executable-install-path validation. |
| Platform detector | Reimplement in Rust; preserve `darwin`, `linux`, `win32`, and `wsl` as distinct target semantics. |
| Plan, diff, doctor, prune, uninstall outcomes | Preserve the user-facing contracts, but rebuild them over the compiler operation graph and mise/native providers. |
| Ledger and file snapshots | Preserve ownership and drift principles. Track generated mise files and Powermise config adapters; do not duplicate mise’s package database. |
| JSON/TOML configuration patching | Port only the specific typed operations needed by harness/editor/MCP adapters. Keep snapshots and idempotence. |
| Documentation and marketing site | Copy the Astro/Starlight site into this repository, tailor all product copy, commands, terminology, catalog presentation, metadata, links, and generated-docs inputs to Powermise. Preserve the accessible theme, navigation, and content architecture. |

### Do not copy

- The managed runtime/updater and repository-cloning installation model.
- Unix Homebrew execution, generic package execution, tool execution, and dotfile/repository convergence now delegated to mise.
- Legacy CLI wiring, output formatting, and TypeScript implementation details.
- Any installer rule without a verified current upstream source and a clear Powermise provider.

### Registry provider mapping

Each asset may resolve to one or more provider-specific operations:

```toml
id = "github-cli"
kind = "tool"
supported_platforms = ["darwin", "linux", "win32", "wsl"]
check = { command = "gh", args = ["--version"] }

[providers.mise]
tools = { github-cli = "latest" }
packages = ["brew:gh", "apt:gh"]

[providers.winget]
id = "GitHub.cli"
exact = true

[providers.scoop]
name = "gh"
```

The actual final schema must separate tool-version entries from OS package entries, give each mapping explicit supported targets, and validate that every declared target has an automated path or an explicit manual-only rejection.

## Windows Contract

Windows support is a first-class acceptance criterion, not an afterthought.

### Target categories

| Target | Primary execution |
| --- | --- |
| Windows developer tools supported by mise | Generated `[tools]` and `mise install` |
| Windows applications/packages with winget | Powermise winget provider |
| Windows packages available through Scoop | Powermise Scoop provider |
| Global npm CLIs | Powermise npm provider or mise tool post-install only when safe and verifiable |
| First-party PowerShell installers | Powermise verified PowerShell provider |
| macOS/Linux-only asset | Resolver rejection with platform-specific explanation |
| WSL | Distinct Linux-on-WSL target; do not treat it as native Windows |

### Provider requirements

- All Windows operations implement `check`, `plan`, `apply`, and machine-readable result reporting.
- `winget` installs use exact IDs and noninteractive arguments where supported.
- Scoop checks buckets before use and reports required user action instead of silently adding an untrusted bucket.
- Remote script installers require pinned immutable source/version and SHA-256 verification before execution.
- PowerShell commands must be argument-array based, not string-concatenated shell commands.
- CI runs registry validation and fixture-driven plan/apply/doctor tests for `win32` and `wsl` even when real package installation is not possible.

## Security and Safety Rules

- Require `mise trust`/trust state before applying generated project or user configuration; never bypass mise’s trust model.
- Generate TOML with a serializer, not string interpolation.
- Write generated config, selection state, snapshots, and run reports via same-filesystem temporary file plus atomic rename.
- Redact tokens and secrets from plans, state, logs, and snapshots.
- Verify immutable remote installer artifacts before execution; reject unpinned remote shell snippets.
- Default every destructive command to preview and explicit confirmation.
- Distinguish “present” from “owned”; only Powermise-owned, unchanged assets qualify for automatic prune candidates.

## Delivery Phases

### Phase 0 — Mise Compatibility Spike

**Purpose:** prove the integration boundary before creating the product architecture.

1. Pin the tested minimum mise version after a clean-machine validation.
2. Install mise through its supported installer on macOS, Linux, and Windows test environments where available.
3. Generate a fixture `config.powermise-engineering-agents.toml` containing `[tools]`, `brew:`, `apt:`, dotfiles, and a harness-selection environment.
4. Verify `mise bootstrap --env powermise-engineering-agents --dry-run`, `--yes`, `status --json`, and repeat apply.
5. Validate direct Homebrew-free `brew:` behavior on Apple Silicon macOS and record unsupported Intel macOS behavior.
6. Verify config precedence with a user-owned `config.toml`, a harness-selection environment config, and a project-local `mise.toml`.
7. Establish whether all data needed for Powermise plan/status can be obtained from stable mise CLI output; document any parsing contracts.

**Exit criteria:** written compatibility report; one repeatable fixture passes on macOS and Linux; Windows gaps are explicit; no Powermise production code depends on mise internals.

### Phase 1 — Rust Foundation and Registry Import

1. Initialize Rust workspace and crate boundaries above.
2. Add structured diagnostics, JSON output, logging, XDG paths, atomic persistence, and test fixtures.
3. Port audited registry data into the Powermise registry format with source provenance.
4. Implement registry schema validation, platform matrix validation, and deterministic loader ordering.
5. Implement harness-selection and machine-role resolution from TOML.
6. Implement `powermise registry validate`, `powermise selection validate`, and `powermise selection import-catalog`.

**Exit criteria:** imported harness catalog and selection fixtures validate identically on macOS/Linux/Windows targets; invalid references and unsupported platform mappings fail before planning.

### Phase 2 — Compiler and Mise Backend

1. Define typed operation graph, plan/report schema, and deterministic ordering.
2. Implement resolver from harness selection + role + target platform to selected asset set.
3. Implement TOML compiler for `[tools]`, bootstrap packages, repos, dotfiles, supported system settings, and safe hooks.
4. Implement mise executable discovery, version gate, `--env` execution, dry-run, and status JSON adapter.
5. Implement generated-file fingerprinting, headers, atomic writes, and ownership records.
6. Implement `powermise plan`, `apply`, and `status` for mise-managed assets.

**Exit criteria:** a selected fixture produces byte-stable generated TOML; plan is mutation-free; apply converges on repeat; the user global config remains unchanged.

### Phase 3 — Native Provider and Configuration Adapters

1. Implement provider port plus fake provider for tests.
2. Port `winget` exact-ID provider and Scoop provider with platform-specific checks.
3. Port npm global provider only where it remains necessary after mise tool resolution.
4. Implement verified first-party PowerShell and script provider.
5. Port minimal typed JSON/TOML config adapters for one harness, one editor, and one MCP server.
6. Add file snapshots, drift checks, idempotence, and reversible managed-block lifecycle.

**Exit criteria:** a Windows fixture produces a unified dry-run; no native Windows package is delegated to an unsupported mise path; one end-to-end harness selection configures its tool, app integration, and MCP entry idempotently.

### Phase 4 — Harness Selection Lifecycle and Safety Operations

1. Implement named selection create/list/show/export/import/activate semantics.
2. Implement machine-role selection and cross-machine import fixture.
3. Implement `diff`, `doctor`, `prune --dry-run`, `prune --yes`, and `uninstall --dry-run` over ledger-owned resources.
4. Surface mise bootstrap follow-up actions alongside Powermise provider follow-up actions.
5. Add failure-boundary reporting and recovery guidance for partial applies.

**Exit criteria:** a selection transfer recreates desired state on an isolated target; dry-run creates no state/config/ledger artifacts; drift is detected without destructive repair; prune excludes unmanaged assets.

### Phase 5 — UX, Documentation, and Release Readiness

1. Add an interactive harness-selection flow that emits the same selection model as noninteractive commands.
2. Add shell integration only as explicit opt-in install/uninstall commands.
3. Revisit the marketing and documentation visual system after the CLI and catalog contracts stabilize. The current implementation is provisional. Before every marketing or documentation UI decision, read and apply `.agents/skills/SKILL.md`; before acceptance, audit Mise’s actual palette and spacing tokens, then integrate selected Canvas UI components from its registry/source rather than local visual approximations; retain semantic HTML and reduced-motion fallbacks. Generate the support matrix and catalog reference pages from the Powermise profile catalog.
4. Implement a CLI presentation layer after command contracts stabilize: semantic status symbols, aligned tables, readable TOML diffs, and progress states for TTY output; `--color auto|always|never` must respect `NO_COLOR`, while `--output json` remains color-free and schema-stable.
5. Publish a migration guide for legacy harness selections and clearly document coexistence with user-managed mise config.
6. Add CI matrix for macOS, Linux, Windows, and WSL-target plan fixtures. (done: `ci.yml` + hermetic win32/wsl plan-fixture tests)
7. Establish release artifacts, update policy, and minimum mise compatibility policy. (done: `docs/release.md`)

**Exit criteria:** documented clean-machine setup works on all claimed targets; harness-selection import, plan, apply, status, doctor, and uninstall are represented by end-to-end fixtures; Windows support claims are evidence-backed.

## Test Strategy

| Test layer | Contract |
| --- | --- |
| Domain unit tests | Harness-selection merge/exclusion, dependency resolution, platform rejection, ownership eligibility, operation order. |
| Registry tests | Every ID resolves; references are valid; every supported platform has an executable provider/check; unsupported combinations reject explicitly. |
| Compiler snapshots | Given harness selection + target + registry, emitted mise TOML and native plan are deterministic. |
| Adapter contract tests | Mise runner, winget, Scoop, npm, script, and config adapters expose equivalent check/plan/apply result semantics. |
| Filesystem integration tests | Generated config writes are atomic; user global mise config is untouched; interrupted writes leave prior complete file intact. |
| Mise fixture tests | `--dry-run` mutates nothing, first apply converges, second apply is no-op or reports satisfied state, status reports expected state. |
| End-to-end CLI tests | Harness-selection import/export, machine roles, plan, apply, diff, doctor, prune, and uninstall across isolated XDG/HOME directories. |
| CI platform tests | macOS, Linux, Windows, and WSL target resolution plus Windows native-provider fixtures. |

## Acceptance Gates

Do not call Powermise ready until all conditions hold:

1. A user-owned global mise configuration survives all Powermise commands byte-for-byte.
2. A generated active harness-selection config is deterministic, atomically replaced, and only loaded when its Powermise environment is selected.
3. `powermise plan` and its mise/native subplans mutate neither configuration nor ledger state.
4. Repeated `powermise apply` converges without duplicate configuration or destructive replacement.
5. Every shipped harness/app/integration/MCP selection has explicit platform and ownership behavior.
6. Windows and WSL are separately validated; no unsupported native Windows asset is silently attempted.
7. Homebrew-free mise bootstrap on supported Apple Silicon macOS is covered by a real integration test; Intel macOS behavior is rejected or manually classified before install.
8. Generated configuration, provider results, snapshots, and ledger entries are sufficient for doctor, drift reporting, and ownership-safe prune.
9. Remote installation sources are immutable and integrity-verified.

## Open Decisions to Resolve During Phase 0

These have safe defaults in this plan; confirm them before Phase 1 implementation:

| Decision | Default | Why it matters |
| --- | --- | --- |
| Product name | `powermise` | CLI name, config directory, and generated environment prefix. |
| Implementation language | Rust | Aligns with mise, gives native Windows support, and avoids bundling Bun for the control plane. |
| Mise coupling | External executable with minimum-version gate | Avoids a fork and unstable internal Rust APIs. |
| Selection storage | Powermise TOML files referencing audited harness IDs | Makes curation explicit and prevents generic profile drift. |
| Active-selection mechanism | Explicit `--selection` commands first; opt-in shell integration later | Avoids silently changing a user’s global mise behavior. |
| Config boundary | `config.powermise-<selection>.toml` only | Preserves the user-owned global mise config. |
| Windows strategy | Powermise native provider layer | Preserves current coverage where mise bootstrap lacks an equivalent package backend. |
| Initial catalog | Audit and import existing entries | Retains hard-won manifests but prevents blind migration of unverified installers. |

## Feature Tracks

### Curated Dotfile Management

**Status:** proposed (design draft).

**Purpose**
Applications, skills, and MCP servers that Powermise provisions ship with their
own dotfiles (for example `~/.claude.json`, MCP server config, editor
integration files, and skill definition files). Managing those dotfiles is a
first-class part of curating an agentic-forward system, not an afterthought.
This track makes curated-component dotfiles owned, previewable, drift-checked
resources instead of opaque shell hooks.

**Scope**
- In scope: dotfiles *declared by the curated catalog* for apps, skills, and
  MCP servers (and harness/app config files that are not pure symlink
  dotfiles).
- Out of scope: the user's *personal* dotfiles. Those remain a user choice,
  optionally managed by a companion dotfile manager (see the site guide
  "Using Powermise with chezmoi"). Powermise must never take ownership of
  unrelated user configuration.

**Design**
- Catalog schema extension: `app` / `skill` / `mcp` profiles may declare a
  `dotfiles` list. Each entry names a target path, a source (inline content or
  a curated template with parameters), ownership, a verification check, and a
  reversibility mode (restore-via-managed-block or declare-only).
- Compiler promotion: `ResourceKind::Dotfile` (already present in the plan
  model) becomes a real operation type in the graph with provider, ownership,
  verification, and reversibility metadata. The compiler emits pure
  symlink/copy dotfiles into the generated mise `[dotfiles]` section (mise
  converges them), and routes app/skill/MCP config files that need snapshots
  and drift detection through config-adapter managed blocks.
- Convergence: mise performs the actual dotfile application (symlink/copy). This
  preserves the non-goal "do not reimplement mise's dotfile convergence
  engine." Powermise owns the *declaration*, the ownership ledger entry, drift
  detection, and the curated source — not the link mechanics.
- Lifecycle integration: `plan` previews dotfile operations (target, mode,
  diff); `apply` writes generated config + ledger and lets mise converge;
  `status` / `diff` / `doctor` surface dotfile drift and ownership; `prune` /
  `uninstall` remove only ledger-owned dotfiles and never touch unmanaged files.
- Safety: dry-run first, atomic writes, marker-guarded managed blocks, explicit
  `--yes`, and secrets redacted from plans/ledger (secret material is delegated
  to mise/age or the user's companion manager, never stored by Powermise).

**Open questions**
- Template parameter model: which variables (target, machine role, selection
  name, host) are available to curated dotfile templates, and how they compose
  with `machine_roles`.
- Verification contracts for dotfiles whose "correctness" is app-specific.
- Whether chezmoi-managed selection sources and Powermise-owned curated
  dotfiles need a shared machine-identity handshake (the companion guide
  already covers the distributable-selection pattern).

**Acceptance**
- A curated app's dotfiles are declared in the catalog, compiled into the
  generated mise config, converged by mise, recorded in the ledger, and visible
  in `status` / `diff` / `doctor`; `uninstall` removes them; user global
  dotfiles remain untouched.

## Sources

- [mise bootstrap](https://mise.jdx.dev/bootstrap.html)
- [mise configuration](https://mise.jdx.dev/configuration.html)
- [mise configuration environments](https://mise.jdx.dev/configuration/environments.html)
- [mise Homebrew bootstrap manager](https://mise.jdx.dev/bootstrap/packages/brew.html)
- [mise upstream Cargo manifest](https://github.com/jdx/mise/blob/main/Cargo.toml)
- Upstream source references: `ROADMAP.md`, `registry/harnesses/`, `packages/core/src/registry/schema.ts`, `packages/core/src/state/profile.ts`, and `apps/web/`
