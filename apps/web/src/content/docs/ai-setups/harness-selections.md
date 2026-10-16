---
title: Selections
description: A setup is a small file naming the curated harnesses you want; powermise resolves everything else from the catalog.
---

Every AI setup you save is a **selection**: a small, portable file that names
the harnesses you want. It doesn't list tools or versions — the
[catalog](/ai-setups/catalog/) decides what each harness needs on each
platform, so the same selection works on macOS, Linux, and Windows.

```toml
schema_version = 2
name = "engineering-agents"
catalog = "powermise-profiles"
harnesses = ["claude", "codex", "gemini"]

[preferences]
include_desktop_apps = false
```

The app writes this for you when you save a setup. Selections live in
`~/.config/powermise/selections/<name>.toml` (or under `POWERMISE_HOME` or
`XDG_CONFIG_HOME/powermise` if you set them).

## How a selection resolves

1. Every setup starts with the **base** harness: Git, curl, jq, ripgrep, and fd.
2. Each harness adds the tools it requires.
3. With **Include desktop apps** on, harnesses add their desktop companions —
   VS Code for GitHub Copilot, for example — where the platform has a provider.
   Desktop-first harnesses (Claude Desktop, ChatGPT, Cursor…) always include
   theirs.
4. Each tool gets exactly one provider for the platform (see
   [Platforms](/ai-setups/targets/)), and the result compiles to one mise
   config file.

If a harness doesn't support the platform, or a tool has no provider there,
the setup editor says so before anything is saved or applied.

## Moving a setup between computers

On the setup's page, **Export…** saves it as a file (`<name>.powermise.toml`).
On the other computer, create a setup with **+** and choose **Import a Setup
File…**: the editor opens with that setup filled in, so you can review it,
rename it if the name is taken, and save. Anything the other computer's
powermise doesn't have — an agent from a newer catalog, say — is left out and
listed. The setup compiles for that computer's platform when you apply it.

Copying the selection file into `~/.config/powermise/selections/` works too. To skip powermise on
the other end entirely, copy the compiled mise file instead — see
[From the terminal](/ai-setups/terminal/#other-machines-dotfiles-and-ci).
