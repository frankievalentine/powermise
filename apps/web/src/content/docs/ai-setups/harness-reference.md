---
title: Harness reference
description: How agent installation is structured, what each harness installs per machine, and the current platform gaps to verify.
---

This page is the authoritative reference for **what gets installed when you select a
harness**, **how that maps to each machine's package manager**, and **which
platforms currently work**. It is derived directly from the catalog
(`catalog/profiles/*.toml`) and the compiler/lifecycle behavior in
`crates/powermise-compiler` and `crates/powermise-lifecycle`.

It is intentionally a living document: the catalog is the source of truth, and
this page should be regenerated whenever the catalog changes.

## How installation is structured

powermise is fully catalog-driven. A selection resolves against five profile
files:

| File | Role |
| --- | --- |
| `catalog/profiles/harnesses.toml` | 18 harnesses. Each `extends = "base"` and declares `required_tool_ids`, `integration_ids`, `mcp_server_ids`, and `supported_platforms`. |
| `catalog/profiles/tools.toml` | Each tool lists install providers (`MiseTool`, `BootstrapBrew`, `BootstrapBrewCask`, `Winget`) with `supported_platforms`. |
| `catalog/profiles/apps.toml` | Application bundles. Apps are opt-in via a machine role's `include_apps` — they are **not** tied to a harness. |
| `catalog/profiles/integrations.toml` | `config-adapter` entries that write managed config blocks into an agent's settings. |
| `catalog/profiles/mcp.toml` | `config-adapter` MCP server entries (Context7 and Sequential Thinking variants). |

### Resolution and provider priority

When you resolve a selection for a `--target`, powermise:

1. Merges `extends` recursively — so **every selection always pulls the base
   harness's five tools**: `git`, `curl`, `jq`, `ripgrep`, `fd`.
2. For each tool/app, picks **one** provider in this priority order:
   `MiseTool` → `BootstrapBrew` → `BootstrapBrewCask`. These become entries in
   the generated mise config (`[tools]` plus `[bootstrap.packages]`), which are
   installed by `mise bootstrap`.
3. If a tool only has a `Winget`/`Scoop` provider for that target (Windows
   native tools), it becomes a **native powermise operation** that `apply` runs
   directly through winget/scoop — it is not managed by mise.
4. Integrations and MCP servers become **config-adapter operations** that write
   a managed, reversible block into the agent's settings file. They do not
   install binaries.

## Getting started on each machine

The flow is identical on every OS; only the install backend differs.

1. **Prerequisites.** Install `mise` (required — `apply` runs `mise bootstrap`),
   and the platform package manager: **brew** on macOS/Linux, **winget or
   scoop** on Windows.
2. **Install powermise.** `git clone https://github.com/frankievalentine/powermise.git && cd powermise && cargo install --path crates/powermise-cli`
3. **Author a selection** TOML (harnesses plus optional `machine_roles` with
   `include_apps`), or use `powermise selection create` / `import`.
4. **Preview per machine.** `powermise selection show --name X --target <darwin|linux|win32|wsl>`
   then `powermise plan --name X --target <t> --yes` (plan mutates nothing).
5. **Converge.** `powermise apply --name X --target <t> --yes` writes the
   generated config to `~/.config/mise`, records ledger ownership, then runs a
   real `mise bootstrap --yes` (or `--skip-bootstrap` to only write config).
6. **Wire the shell.** `powermise shell install --name X` adds
   `export MISE_ENV=powermise-<selection>` to your shell startup so the
   environment auto-activates.

### Per-target install mechanism

| `--target` | Tool backends used | Notes |
| --- | --- | --- |
| `darwin` | `mise-tool` and `brew`/`brew-cask` bootstrap | No winget. Desktop apps (Claude, Codex, Cursor, Zed, etc.) come from brew-cask. |
| `linux` | `mise-tool` and `bootstrap-brew` | Treated as a standard Linux environment. |
| `wsl` | `mise-tool` and `bootstrap-brew` | Treated as a **distinct Linux target** — see the critical gap below. |
| `win32` | `mise-tool` plus **winget native** | Windows-only tools install through winget, run by powermise directly. |

Windows winget package IDs currently in the catalog:

| Tool | Winget ID |
| --- | --- |
| `git` | `Git.Git` |
| `curl` | `curl.curl` |
| `jq` | `jqlang.jq` |
| `ripgrep` | `BurntSushi.ripgrep.MSVC` |
| `fd` | `sharkdp.fd` |
| `vscode` | `Microsoft.VisualStudioCode` |
| `node` | `OpenJS.NodeJS` |

## What each harness installs

The **base five** (`git curl jq ripgrep fd`) are implied for every harness.
"Works on" reflects where the catalog can currently resolve and compile today
(platforms where a required tool has no provider are excluded — see gaps).

| Harness | Works on | Extra tools | Integrations | MCP servers |
| --- | --- | --- | --- | --- |
| `base` | darwin, linux, win32 | — | — | — |
| `antigravity` | darwin, linux, win32 | — (base only) | — | — |
| `claude` | **darwin only** | `claude-code`, `claude-app` (brew-cask) | claude-github, claude-mem | claude-context7, claude-sequential-thinking |
| `cline` | **darwin, win32** | `vscode`, `node`, `cline` | — | cline-context7 |
| `codex` | **darwin only** | `node`, `codex`, `codex-app` (brew-cask) | codex-gmail | codex-context7, codex-sequential-thinking |
| `conductor` | ❌ none | `conductor` (no provider) | — | — |
| `cursor` | **darwin only** | `cursor` (brew-cask) | — | cursor-context7, cursor-sequential-thinking |
| `gemini` | darwin, linux, win32 | `node`, `gemini-cli` | gemini-workspace | gemini-context7, gemini-sequential-thinking |
| `github-copilot` | **darwin, win32** | `vscode` | — | — |
| `goose` | darwin, linux, win32 | `goose` | — | — |
| `hermes` | ❌ none | `hermes-agent` (no provider) | — | — |
| `openclaw` | darwin, linux, win32 | `node`, `openclaw` | — | — |
| `opencode` | darwin, linux, win32 | `node`, `opencode` | — | opencode-context7, opencode-sequential-thinking |
| `paseo` | darwin, linux, win32 | `paseo` | — | — |
| `pi` | darwin, linux, win32 | `pi` | — | — |
| `superset` | **darwin only** | `superset` (brew-cask) | — | — |
| `t3code` | darwin only | `t3code` (brew-cask) | — | — |
| `zed` | **darwin only** | `zed` (brew-cask) | — | zed-context7, zed-sequential-thinking |
| `fx` | darwin, linux, wsl | `fx` (github release) | — | — |

### Notable provider details

- `claude-code`, `opencode`, and the npm-based agents (`cline`, `openclaw`,
  `paseo`, `pi`, `goose`, `gemini-cli`, `codex`) install via `mise-tool`
  (mise-managed) where available, falling back to brew (darwin/linux) or
  winget/brew-cask as declared.
- Desktop apps (`claude-app`, `codex-app`, `cursor`, `zed`, `superset`,
  `t3code`) are **brew-cask only** — they exist solely on macOS today.
- `hermes-agent` and `conductor` declare **no package-manager provider** (their
  catalogs document vendor scripts/DMGs), so selecting those harnesses fails to
  compile on every platform.
- `fx` installs from its GitHub release via `mise-tool` (`github:vercel-labs/fx`).
  It is a single static Zig binary and is model-agnostic (Vercel AI Gateway,
  OpenAI Codex, or xAI Grok). Releases are **Apple-Silicon macOS only** and
  **Linux only** (no Windows build), so Intel Macs cannot install it. Its MCP
  and ACP connections are configured by the user inside fx, not by powermise.

## App lists (machine roles, not harnesses)

Apps are **not** attached to harnesses. They come from a selection's
`machine_roles.<role>.include_apps`. The entire app catalog today:

| App | Provider | Platform |
| --- | --- | --- |
| `cursor` | `bootstrap-brew-cask` (`cursor`) | **darwin only** |

Consequences to verify:

- On Linux and Windows there are **currently no installable app packages**.
- Selecting `cursor` as an app only resolves on macOS.
- Integrations and MCP servers above are **config-only** — they write managed
  blocks into the agent's settings and never install a binary.

## Known gaps to verify before a clean-machine run

These are catalog/support mismatches discovered while building this reference.
They should be fixed in the catalog or explicitly accepted as limitations:

1. **`wsl` is non-functional for every harness.** The base tools `git` and
   `curl` declare no `wsl` provider (brew is darwin/linux only; winget is
   win32). Because `git`/`curl` are required by every selection, `resolve`/
   `compile` fails on `wsl` regardless of harness. WSL is therefore not
   currently installable until those two tools gain a `wsl` provider.
2. **Harness `supported_platforms` are broader than the tools support.** The
   following silently fail to resolve outside the "Works on" column above
   because a required tool lacks a provider there:
   - `claude` / `codex` / `cursor` / `superset` / `zed` — desktop/app tools are
     brew-cask darwin-only, so they fail on Linux and WSL.
   - `github-copilot` and `cline` — `vscode` has no Linux provider, so they fail
     on Linux and WSL.
   - `codex` advertises `wsl` support but has no `wsl` tool providers at all.
3. **`conductor` and `hermes` cannot be installed anywhere** — their single
   required tool has an empty provider list (vendor DMG/script only).
4. **Cross-platform harnesses are rare.** Only `paseo` and `pi` resolve on all
   four targets today; `gemini`, `goose`, `openclaw`, `opencode`, `github-copilot`,
   and `cline` cover darwin + win32 (and Linux where `vscode`/cask isn't
   required).

### Regenerating this page

To regenerate the table from the catalog, build the CLI and run, for each
harness and target:

```sh
powermise selection compile --target <darwin|linux|win32|wsl> <harness>.toml
```

A successful compile prints the exact `[tools]` and `[bootstrap.packages]`; a
failure reports the missing provider, which is what populates the "Works on"
column above.
