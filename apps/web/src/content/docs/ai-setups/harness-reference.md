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
   `[bootstrap.packages]`; tools that come with the system (git and curl on
   macOS) add nothing.
4. The result is one file, `conf.d/powermise-<name>.toml`, that
   `mise bootstrap` installs. See
   [From the terminal](/ai-setups/terminal/).

5. MCP servers the setup picks (each harness recommends a few) are added to
   every agent in the setup through mise: see
   [MCP servers](/ai-setups/terminal/#mcp-servers). Plugins it picks install
   with each agent's own plugin command: see
   [Plugins](/ai-setups/terminal/#plugins).

## What each harness installs

"Works on" is every platform the harness compiles for today. Installs are the
macOS keys as they appear in the generated file, including the tools for the
harness's recommended MCP servers.

| Harness | Works on | Installs on macOS, beyond the basics | Desktop apps (opt-in) | MCP servers |
| --- | --- | --- | --- | --- |
| `amp` | darwin, linux, wsl | `node`, `npm:@ampcode/cli`, `npm:@modelcontextprotocol/server-sequential-thinking`, `npm:@upstash/context7-mcp` | — | `context7`, `sequential-thinking` |
| `chatgpt` | darwin, win32 | `brew-cask:chatgpt` | — | — |
| `claude` | darwin, linux, win32, wsl | `claude-code`, `npm:@modelcontextprotocol/server-sequential-thinking`, `npm:@upstash/context7-mcp` | — | `context7`, `sequential-thinking` |
| `claude-desktop` | darwin, win32 | `npm:@modelcontextprotocol/server-sequential-thinking`, `npm:@upstash/context7-mcp`, `brew-cask:claude` | — | `context7`, `sequential-thinking` |
| `cline` | darwin, win32 | `node`, `npm:@upstash/context7-mcp`, `npm:cline`, `brew-cask:visual-studio-code` | — | `context7` |
| `codex` | darwin, linux, win32, wsl | `node`, `npm:@modelcontextprotocol/server-sequential-thinking`, `npm:@openai/codex`, `npm:@upstash/context7-mcp` | — | `context7`, `sequential-thinking` |
| `conductor` | darwin | `brew-cask:conductor` | — | — |
| `cursor` | darwin, win32 | `npm:@modelcontextprotocol/server-sequential-thinking`, `npm:@upstash/context7-mcp`, `brew-cask:cursor` | — | `context7`, `sequential-thinking` |
| `fx` | darwin, linux, wsl | `github:vercel-labs/fx` | — | — |
| `gemini` | darwin, linux, win32, wsl | `node`, `npm:@google/gemini-cli`, `npm:@modelcontextprotocol/server-sequential-thinking`, `npm:@upstash/context7-mcp` | — | `context7`, `sequential-thinking` |
| `github-copilot` | darwin, linux, win32, wsl | `copilot`, `npm:@upstash/context7-mcp` | `brew-cask:visual-studio-code` | `context7` |
| `hermes` | darwin, linux, wsl | `brew:hermes-agent` | — | — |
| `openclaw` | darwin, linux, win32, wsl | `node`, `npm:openclaw` | — | — |
| `opencode` | darwin, linux, win32, wsl | `node`, `npm:@modelcontextprotocol/server-sequential-thinking`, `npm:@opencode/cli`, `npm:@upstash/context7-mcp` | — | `context7`, `sequential-thinking` |
| `orca` | darwin, win32 | `macos-app:orca` | — | — |
| `paseo` | darwin, linux, win32, wsl | `npm:@getpaseo/cli` | — | — |
| `pi` | darwin, linux, win32, wsl | `npm:@earendil-works/pi-coding-agent` | — | — |
| `superset` | darwin | `brew-cask:superset` | — | — |
| `t3code` | darwin | `brew-cask:t3-code` | — | — |
| `zed` | darwin, win32 | `brew-cask:zed` | — | — |

## Known gaps

- **Gemini CLI is enterprise-only now.** Google retired it for personal
  accounts on June 18, 2026; it still works with a Gemini Code Assist Standard
  or Enterprise license, or a paid API key.
- **ChatGPT on Windows comes from the Microsoft Store** (`9PLM9XGG6VKS`, through
  winget). Store apps can't be pinned to a version, so it's declared `latest`.
- **Amp has no native Windows build.** It runs on Windows through WSL.
- **`hermes` has no Windows build.** It installs from Homebrew on macOS and
  Linux.
- **Desktop apps install on macOS and Windows only.** They come from Homebrew
  casks (or the vendor's checksummed download, for Orca) and winget. None of
  the catalog's apps has a Flathub listing verified by its developer, so
  there's no official Linux route yet.
- **Zed's MCP servers aren't configured.** Zed's settings file can hold
  comments, which mise won't merge into; add servers in Zed's Agent Panel.
- **`cline` needs VS Code**, which has no Linux provider. (`github-copilot`
  installs Copilot CLI everywhere; VS Code is its optional desktop app.)

## Regenerating the table

```sh
scripts/harness-matrix.sh
```

It builds the compiler, compiles each harness for `darwin`, `linux`,
`win32`, and `wsl`, and prints the table above.
