---
title: Harness reference
description: What each harness installs, on which platforms, and the catalog gaps still open.
---

What gets installed when you pick a harness, and where it works. The catalog
(`catalog/profiles/*.toml`) is the source of truth; the table below is
generated from it by compiling every harness for every platform.

## How a harness becomes a mise config

1. Every setup includes the **base** harness: `git`, `curl`, `jq`,
   `ripgrep`, and `fd`.
2. Each harness adds its required tools, and — with **Include desktop apps**
   on — its desktop companions where the platform has a provider.
3. Each tool gets one provider for the platform, chosen in a fixed order (see
   [Platforms](/ai-setups/targets/#how-providers-are-chosen)). mise tools go in
   `[tools]`; Homebrew, apt, winget, and Scoop packages go in
   `[bootstrap.packages]`.
4. The result is one file, `conf.d/powermise-<name>.toml`, that
   `mise bootstrap` installs. See
   [From the terminal](/ai-setups/terminal/).

Integrations and MCP servers are listed for each harness but not configured:
they show as **Manual setup**.

## What each harness installs

"Works on" is every platform the harness compiles for today. Installs are the
macOS keys as they appear in the generated file.

| Harness | Works on | Installs on macOS, beyond the basics | Desktop apps (opt-in) | MCP servers |
| --- | --- | --- | --- | --- |
| `antigravity` | darwin, linux | — | — | — |
| `claude` | darwin, linux, win32, wsl | `claude-code` | `brew-cask:claude` | `claude-context7`, `claude-sequential-thinking` |
| `cline` | darwin, win32 | `node`, `npm:cline`, `brew-cask:visual-studio-code` | — | `cline-context7` |
| `codex` | darwin, linux, win32 | `node`, `brew-cask:codex` | `brew-cask:codex-app` | `codex-context7`, `codex-sequential-thinking` |
| `conductor` | darwin | `brew-cask:conductor` | — | — |
| `cursor` | darwin | `brew-cask:cursor` | — | `cursor-context7`, `cursor-sequential-thinking` |
| `fx` | darwin, linux, wsl | `github:vercel-labs/fx` | — | — |
| `gemini` | darwin, linux, win32 | `node`, `brew:gemini-cli` | — | `gemini-context7`, `gemini-sequential-thinking` |
| `github-copilot` | darwin, win32 | `brew-cask:visual-studio-code` | — | — |
| `goose` | darwin, linux, win32 | `github:aaif-goose/goose` | — | — |
| `hermes` | none yet | — | — | — |
| `openclaw` | darwin, linux, win32 | `node`, `npm:openclaw` | — | — |
| `opencode` | darwin, linux, win32 | `node`, `brew:opencode` | — | `opencode-context7`, `opencode-sequential-thinking` |
| `paseo` | darwin, linux, win32, wsl | `npm:@getpaseo/cli` | — | — |
| `pi` | darwin, linux, win32, wsl | `npm:@earendil-works/pi-coding-agent` | — | — |
| `superset` | darwin | `brew-cask:superset` | — | — |
| `t3code` | darwin | `brew-cask:t3-code` | — | — |
| `zed` | darwin | `brew-cask:zed` | — | `zed-context7`, `zed-sequential-thinking` |

## Known gaps

- **`hermes` can't be installed anywhere yet.** Its vendor publishes only
  install scripts, with no package-manager or mise identifier.
- **WSL coverage is thin.** `codex`, `gemini`, `goose`, `openclaw`, and
  `opencode` have no WSL provider for one of their tools, so they don't
  compile there.
- **Desktop apps are macOS-only.** They come from Homebrew casks; Linux and
  Windows have no desktop app providers in the catalog yet.
- **`cline` and `github-copilot` need VS Code**, which has no Linux provider.

## Regenerating the table

```sh
scripts/harness-matrix.sh
```

It builds the compiler, compiles each harness for `darwin`, `linux`,
`win32`, and `wsl`, and prints the table above.
