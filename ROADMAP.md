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

### M1 — Foundation ✅

- [x] `powermise-mise`: typed models for `ls`, `outdated`, `registry`, `ls-remote`,
      `config ls`, `tasks ls`, `env`, `doctor`, `version`; fixture tests captured from
      real mise.
- [x] mise version policy, checked at launch: **2026.7.18 minimum, 2026.10 recommended**.
- [x] New navigation: mise (Installed, Discover, Updates, Activity, Health) and AI Setups.
- [x] Move the existing harness screens under **AI Setups**.
- [x] **Installed**: every tool mise manages — active version, where it's configured,
      update badges.
- [x] **Discover**: searchable mise registry with tool pages and available versions.

### M2 — Changes

- [x] Install a tool globally (`mise use -g tool@version`), with a version picker.
- [x] Upgrade, uninstall, change version, and remove from global on tool pages.
- [x] **Updates**: everything outdated (including past pins, `--bump`), with "Upgrade all".
- [x] Streamed command output with cancel (whole process group), shared by every change.
- [x] Activity log: every command Powermise ran this session, with its result.
- [ ] **Clean up**: remove versions nothing uses (`mise prune --dry-run` to preview,
      then `mise prune`).
- [ ] Tool pages from `mise tool --json`: backend, tool options, and supply-chain
      checks (checksums, GitHub attestations) shown as "Verified" badges.
- [ ] Install anything mise supports beyond the registry: a backend picker for
      `npm:`, `pipx:`, `cargo:`, `go:`, `github:`, `aqua:`, `ubi:`, … (`mise backends ls`).
- [ ] Split Updates into global vs. project tools (`mise outdated --global`,
      `mise upgrade --global`; needs mise 2026.10.7+).
- [ ] Automatic updates: toggles for `tool_update.global_auto`, per-tool `auto_update`,
      and `self_update.auto` (2026.10.4+), set through `mise settings set`.
- [ ] Persist the activity log across launches.

### M3 — Projects

- [ ] Add project folders (manual add + recent; no disk scanning).
- [ ] Tools per project, including idiomatic version files (`.nvmrc`,
      `.python-version`, `.tool-versions`) and `mise install` for anything missing.
- [ ] Config files (`mise config ls`) and the layering between them: `mise.toml`,
      `mise.local.toml`, global, and system. Edit values through `mise config set`.
- [ ] **Environments** (`MISE_ENV`, `mise.<env>.toml`): a switcher showing what changes
      in `development` vs. `staging` vs. `production`.
- [ ] Environment variables from `mise env --json`, masking anything marked
      sensitive/redacted; add and remove with `mise set` / `mise unset`.
- [ ] **Tasks**: list (`mise tasks ls`), run with live output, and a form for task
      arguments, flags, and choices generated from the usage spec in
      `mise tasks info --json`. Show dependencies (`mise tasks deps`), and offer
      "Run on change" (`mise watch`). Interactive tasks open in Terminal.
- [ ] Trust flow for untrusted configs (`mise trust` / `mise untrust`), always
      user-confirmed; show which hooks (`[hooks]`) a config would run before trusting.
- [ ] Lockfiles: show whether `mise.lock` is current and refresh it (`mise lock`,
      which supports `--dry-run` and `--json`).
- [ ] "Open a terminal with this project's tools" (`mise en`).
- [ ] Generators: add a GitHub Actions workflow, git pre-commit hook, or devcontainer
      (`mise generate …`), previewing the file first.
- [ ] Per-machine `[vars]` prompts (`mise vars`, 2026.10.6+) and secrets sources
      (`mise secrets ls --json`, experimental) shown read-only at first.

### M4 — Health, settings & onboarding

- [x] **Health**: `mise doctor` findings, version vs. latest, shell activation with the
      exact line to add, config files mise reads.
- [x] mise self-update.
- [x] Explain shell-only configuration (env vars from `~/.zshrc`) that a Dock-launched
      app can't see.
- [ ] Settings browser (`mise settings ls --json`) grouped with docs links; edit through
      `mise settings set` / `unset`.
- [ ] GitHub token check: rate-limited downloads are the most common install failure.
      Show what `mise token` finds and guide `mise token github --oauth`.
- [ ] Disk usage and cache: `mise cache path` / `clear` / `prune`, and the download
      cache size limit.
- [ ] Trusted configs: review and revoke.
- [ ] Shims vs. activation, explained, with `mise reshim` when shims are in use.
- [ ] First run: detect a missing mise and guide installation; offer to bring over
      versions from nvm, pyenv, or Homebrew (`mise sync node|python|ruby`, `mise link`).
- [ ] Gate features by mise version (several above need 2026.10.x) and explain why a
      control is disabled. Capture a second fixture set from 2026.10 so both ends of
      the supported range stay tested.

### M5 — Marketing site & docs

- [x] Reposition the site: "A desktop app for mise."
- [x] Hero with a WebGL dithered color field (three.js), pointer-reactive, paused
      offscreen, still under reduced motion.
- [x] A working, interactive demo of the real desktop UI (built in demo mode on the
      browser stub), with a GSAP scroll tour that steers it screen by screen.
- [x] Rewrite docs: what Powermise is, getting started with the app, an app tour,
      and how Powermise uses mise. AI Setups docs moved under `/ai-setups/`.
- [x] Keep the CLI reference for AI Setups.
- [x] The dithered chef's hat moves to the closing call-to-action.
- [ ] Screenshots / short screen recordings for docs pages once installers ship.
- [ ] Download page with per-platform installers (with M6).

### M6 — Release

- [ ] CI builds for macOS (universal), Windows, and Linux.
- [ ] macOS signing and notarization; Windows signing.
- [ ] Auto-update through GitHub Releases.
- [ ] Download page with per-platform installers.

### M7 — Machine setup (mise bootstrap)

mise's newer `bootstrap` and `dotfiles` features configure a whole machine: system
packages (Homebrew, apt, dnf, snap), repositories, dotfiles, macOS defaults and
login items, and launchd/systemd services. AI Setups already compiles to
`[bootstrap.packages]`.

- [ ] Machine view from `mise bootstrap status --json`: what's applied, missing, or
      drifted, by section.
- [ ] Apply with a preview (`mise bootstrap --dry-run`, then `mise bootstrap --yes`).
- [ ] Dotfiles: status (`mise dot status --json`), save, and sync.
- [ ] Rebuild AI Setups on top of this view, so a setup is a bootstrap template.

## mise coverage

What the GUI does with each area of mise. "Planned" items are in the milestones above.

| mise area | Commands | In the GUI |
|---|---|---|
| Tools | `ls`, `use`, `unuse`, `install`, `uninstall`, `upgrade`, `outdated`, `ls-remote`, `latest`, `which`, `where` | ✅ Installed, Discover, tool pages, Updates |
| Registry & backends | `registry`, `search`, `tool`, `backends`, `tool-alias` | ✅ registry · planned: tool details, any-backend installs, aliases |
| Cleanup | `prune`, `cache` | planned (M2, M4) |
| Projects & config | `config ls/get/set`, `fmt`, `edit`, `trust`, `untrust`, `lock` | planned (M3) |
| Environment | `env`, `set`, `unset`, `vars`, `secrets`, environments (`MISE_ENV`) | planned (M3) |
| Tasks | `tasks ls/info/deps`, `run`, `watch` | planned (M3) |
| Shell | `activate`, `doctor`, `reshim`, `en`, `exec` | ✅ activation guidance · planned: open terminal, reshim |
| Settings | `settings ls/get/set/unset` | planned (M4) |
| Updating mise | `version`, `self-update` | ✅ |
| Machine setup | `bootstrap`, `dotfiles` | planned (M7) |
| Migration | `sync`, `link` | planned (M4 onboarding) |
| Generators | `generate github-action/git-pre-commit/devcontainer/bootstrap` | planned (M3) |
| Auth | `token` | planned (M4) |
| Plugins (asdf/vfox) | `plugins ls/install/update/uninstall` | later: listed read-only, legacy path |

**Not planned:** `mcp` (an agent integration, not a GUI surface — but AI Setups could
point agents at `mise mcp`), `oci`, `deps`, `daemons`, `installs`, `packslip`, and
`ssh` (experimental or niche), `test-tool` (plugin authoring), `completion`,
`deactivate`, `shell` (terminal-session only), `implode` (too destructive for a button),
and `patrons`/`sponsors`.

## AI Setups (secondary feature)

Continues as one section of the app and the `powermise` CLI.

- [ ] Configure MCP servers and integrations for real. Needs catalog data per entry
      (launch command, args, credentials) and each agent's settings location and format.
      Until then they're reported as "Manual setup".
- [ ] `uninstall` also removes run reports.
- [ ] Record the mise config directory in the ledger so `uninstall` targets the
      directory a setup was applied to.
- [ ] Linux, Windows, and WSL verification of the apply path.
- [ ] Offer mise's own MCP server (`mise mcp`) to agents, so they can see and install
      the tools a project uses.

## Decisions

| Decision | Choice |
|---|---|
| Desktop framework | Tauri v2 |
| Audience | People less comfortable in a terminal |
| Writing mise config | Only through mise's commands, with the command shown |
| Project discovery | Manual add + recents |
| mise versions | 2026.7.18 minimum; 2026.10 recommended; newer features gated by version |
| Name | Powermise ("mise-en-place", chef's hat) |

## Open questions

- Whether "Setups" (app) and "selections" (CLI) should share one name.
