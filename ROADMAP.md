# Powermise Roadmap

> **mise is the CLI. Powermise is the GUI.**
>
> Powermise is a desktop companion for [mise-en-place](https://mise.jdx.dev): see every
> tool mise manages, discover new ones, keep them up to date, and work with your
> projects' tools, environments, and tasks — without memorizing commands. Setting up
> AI coding environments is one feature built on top of it.

This file is the living plan. It replaces `BUILD_PLAN.md`, which is archived at
[`docs/archive/BUILD_PLAN.md`](docs/archive/BUILD_PLAN.md) as the record of the
original AI-harness-first design.

## Principles

1. **mise owns its state.** Powermise reads mise through its JSON output and changes
   it only through mise's own commands (`mise use`, `mise upgrade`, `mise settings set`,
   …). It never hand-edits mise TOML.
2. **Never hide what it's doing.** Every action shows the exact command it runs, with
   a copy button, so people who start in the app learn the CLI.
3. **Ask before trusting.** mise's config trust is a security boundary. Powermise
   surfaces trust prompts; it never trusts a file automatically.
4. **Built for people who'd rather not live in a terminal**, in the spirit of
   Homebrew's BrewUI — plain language, clear consequences, undo where possible.
5. **Native feel.** macOS-first chrome and motion (Emil Kowalski's apple-design
   guidance), with Linux and Windows builds from the same codebase.

## Architecture

```
apps/desktop (Tauri v2)
  ├─ React UI ──────────── runs in Tauri, or in a browser against a stub backend
  └─ src-tauri ─┬─ powermise-mise       typed mise adapter (the core)
                └─ AI Setups modules    lifecycle, compiler, registry, providers, domain, state
apps/web (Astro)          marketing site + docs; hosts the interactive app demo
crates/powermise-cli      `powermise` CLI, scoped to AI Setups
```

- **`powermise-mise`** wraps mise's JSON commands (`ls`, `outdated`, `registry`,
  `ls-remote`, `env`, `tasks ls`, `config ls`, `settings ls`, `doctor`,
  `bootstrap status`) in typed models, and runs mutating commands with streamed
  output and cancellation. Parsing is tested against fixtures captured from real mise.
- **The browser stub** (`apps/desktop/src/dev`) keeps the UI runnable without Tauri.
  It powers design work and the live demo on the marketing site.
- **The CLI** stays for AI Setups (`powermise apply`, `status`, …). Powermise does
  not grow a second general-purpose CLI — that's mise's job.

## Milestones

### M0 — Checkpoint ✅

- [x] Bug fixes from the October review (status probing, catalog lookup, Windows
      providers, symlink-safe writes, name validation, stub recursion, exit codes).
- [x] Tauri desktop app with the harness catalog, setup editor, and apply flow.

### M1 — Foundation

- [ ] `powermise-mise`: typed models for `ls`, `outdated`, `registry`, `ls-remote`,
      `doctor`; fixture tests captured from real mise.
- [ ] Minimum supported mise version, checked at launch with a clear upgrade path.
- [ ] New navigation: Installed, Discover, Updates, Projects, Health, and AI Setups.
- [ ] Move the existing harness screens under **AI Setups**.
- [ ] **Installed**: every tool mise manages — active version, where it's configured,
      update badges.
- [ ] **Discover**: searchable mise registry with tool detail and available versions.

### M2 — Changes

- [ ] Install a tool globally (`mise use -g tool@version`), with a version picker.
- [ ] Upgrade, uninstall, and change version from Installed.
- [ ] **Updates**: everything outdated, with "Upgrade all".
- [ ] Streamed command output with cancel, shared by every mutating action.
- [ ] Activity log: every command Powermise ran, with its result.

### M3 — Projects

- [ ] Add project folders (manual add + recent; no disk scanning).
- [ ] Per project: tools, environment variables (secrets masked), and config files
      (`mise config ls`).
- [ ] Tasks: list and run with live output (`mise tasks ls`, `mise run`).
- [ ] Trust flow for untrusted configs (`mise trust`), always user-confirmed.

### M4 — Health & onboarding

- [ ] **Health**: `mise doctor` findings in plain language with fixes.
- [ ] Settings browser (`mise settings ls`), editing via `mise settings set`.
- [ ] mise self-update.
- [ ] First run: detect a missing mise and guide installation.
- [ ] Explain shell-only configuration (env vars from `~/.zshrc`) that a Finder-launched
      app can't see.

### M5 — Marketing site & docs

- [ ] Reposition the site: "A desktop app for mise."
- [ ] Hero: a working, interactive demo of the desktop app (the real UI on the
      browser stub), staged with GSAP and a WebGL backdrop.
- [ ] Rewrite docs: getting started with the app, each screen, how Powermise uses
      mise, and AI Setups as its own section.
- [ ] Keep the CLI reference for AI Setups.

### M6 — Release

- [ ] CI builds for macOS (universal), Windows, and Linux.
- [ ] macOS signing and notarization; Windows signing.
- [ ] Auto-update through GitHub Releases.
- [ ] Download page with per-platform installers.

## AI Setups (secondary feature)

Continues as one section of the app and the `powermise` CLI.

- [ ] Configure MCP servers and integrations for real. Needs catalog data per entry
      (launch command, args, credentials) and each agent's settings location and format.
      Until then they're reported as "Manual setup".
- [ ] `uninstall` also removes run reports.
- [ ] Record the mise config directory in the ledger so `uninstall` targets the
      directory a setup was applied to.
- [ ] Linux, Windows, and WSL verification of the apply path.

## Decisions

| Decision | Choice |
|---|---|
| Desktop framework | Tauri v2 |
| Audience | People less comfortable in a terminal |
| Writing mise config | Only through mise's commands, with the command shown |
| Project discovery | Manual add + recents |
| Name | Powermise ("mise-en-place", chef's hat) |

## Open questions

- Minimum mise version to support (installed here: 2026.7.18; latest: 2026.10.10).
- Whether "Setups" (app) and "selections" (CLI) should share one name.
