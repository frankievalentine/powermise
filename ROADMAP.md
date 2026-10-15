# powermise Roadmap

> **mise is the CLI. powermise is the GUI.**
>
> powermise is an independent, unofficial project — not made by, affiliated with, or
> endorsed by mise or its maintainers.
>
> powermise is a desktop companion for [mise-en-place](https://mise.jdx.dev): see every
> tool mise manages, discover new ones, keep them up to date, and work with your
> projects' tools, environments, and tasks — without memorizing commands. Setting up
> AI coding environments is one feature built on top of it.

This file is the living plan. It replaces `BUILD_PLAN.md`, which is archived at
[`docs/archive/BUILD_PLAN.md`](docs/archive/BUILD_PLAN.md) as the record of the
original AI-harness-first design.

## Principles

1. **mise owns its state.** powermise reads mise through its JSON output and changes
   it only through mise's own commands (`mise use`, `mise upgrade`, `mise settings set`,
   …). It never hand-edits mise TOML.
2. **Never hide what it's doing.** Every action shows the exact command it runs, with
   a copy button, so people who start in the app learn the CLI.
3. **Ask before trusting.** mise's config trust is a security boundary. powermise
   surfaces trust prompts; it never trusts a file automatically.
4. **Built for people who'd rather not live in a terminal**, in the spirit of
   Homebrew's BrewUI — plain language, clear consequences, undo where possible.
5. **One brand, native edges.** The same powermise look on macOS, Windows, and
   Linux — Geist type, the coral accent, platform-neutral controls — with each
   platform's own window chrome, materials, shortcuts, and dialog conventions.

## Architecture

```
apps/desktop (Tauri v2)
  ├─ React UI ──────────── runs in Tauri, or in a browser against a stub backend
  └─ src-tauri ─┬─ powermise-mise       typed mise adapter (the core)
                └─ powermise-setups     AI Setups: catalog → one mise conf.d file
                     (registry, compiler, domain, state)
apps/web (Astro)          marketing site + docs; hosts the interactive app demo
```

- **`powermise-mise`** wraps mise's JSON commands (`ls`, `outdated`, `registry`,
  `ls-remote`, `env`, `tasks ls`, `config ls`, `settings ls`, `doctor`,
  `bootstrap plan/status`) in typed models, and runs mutating commands with streamed
  output and cancellation. Parsing is tested against fixtures captured from real mise.
- **The browser stub** (`apps/desktop/src/dev`) keeps the UI runnable without Tauri.
  It powers design work and the live demo on the marketing site.
- **There is no powermise CLI.** mise is the CLI. An AI setup compiles to
  `conf.d/powermise-<name>.toml` in the mise config folder; applying it is
  `mise bootstrap --yes --only packages,tools`, checking it is
  `mise bootstrap status`, removing it is deleting the file (then optionally
  `mise prune --tools`). The docs page "From the terminal: it's just mise"
  covers it.

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
- [x] Activity log: every command powermise ran this session, with its result.
- [x] **Clean up**: list versions nothing uses with their size (`mise ls --prunable
      --json`), then `mise prune --tools --yes` — on Installed and Health.
- [ ] Tool pages from `mise tool --json`: backend, tool options, and supply-chain
      checks (checksums, GitHub attestations) shown as "Verified" badges.
- [ ] Install anything mise supports beyond the registry: a backend picker for
      `npm:`, `pipx:`, `cargo:`, `go:`, `github:`, `aqua:`, `ubi:`, … (`mise backends ls`).
- [ ] Split Updates into global vs. project tools (`mise outdated --global`,
      `mise upgrade --global`; needs mise 2026.10.7+).
- [ ] Automatic updates: toggles for `tool_update.global_auto`, per-tool `auto_update`,
      and `self_update.auto` (2026.10.4+), set through `mise settings set`.
- [ ] Persist the activity log across launches.

### M3 — Projects (first pass ✅)

- [x] Add project folders (picker, drag and drop, recents; no disk scanning).
- [x] Tools per project (project vs. inherited), **Install Missing** (`mise install`),
      **Add Tool** (`mise use` in the folder).
- [ ] Call out idiomatic version files (`.nvmrc`, `.python-version`) as sources.
- [x] Config files (`mise config ls`) in precedence order with plain-language roles.
- [ ] Edit config values through `mise config set`.
- [ ] **Environments** (`MISE_ENV`, `mise.<env>.toml`): a switcher showing what changes
      in `development` vs. `staging` vs. `production`.
- [x] Environment variables from `mise env --json-extended` with their source file;
      `redact = true` and secret-looking names masked until revealed; add to
      `mise.toml` or `mise.local.toml` and remove from the defining file
      (`mise set|unset --file`); values never shown in the activity log.
- [x] **Tasks**: list, run with live output and Stop, and a form for arguments,
      flags, and choices generated from `mise tasks info --json`; dependencies noted.
- [ ] Dependency graph (`mise tasks deps`), "Run on change" (`mise watch`), and
      opening interactive tasks in a terminal.
- [x] Trust flow: untrusted folders show a read-only preview of hooks, loaded files,
      variables, tools, and tasks before a user-confirmed `mise trust`.
- [x] Untrust (`mise untrust`) from the project page.
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
- [x] Settings browser (`mise settings ls --all --json-extended`) grouped, with
      sources and a docs link; edit through `mise settings set` / `unset`. Lists are
      read-only for now.
- [x] GitHub token check on Health and Welcome: what `mise token github` finds, and
      one-click ways to borrow an existing login (`gh auth token` as
      `github.credential_command`, or `github.use_git_credentials`). Native
      `--oauth` needs a GitHub App client ID, so it's linked, not offered.
- [x] Disk usage and cache: installed-tools and cache sizes (from `mise doctor`'s
      `dirs`), `mise cache prune` / `clear`.
- [x] Trusted configs: review (`<state>/trusted-configs`) and revoke (`mise untrust`),
      on Health and the project page; ignored configs listed.
- [x] Shims vs. activation, explained, with `mise reshim` when shims are in use.
- [x] First run (Welcome): install a missing mise (Homebrew or mise.run on
      macOS/Linux, winget or Scoop on Windows), shell activation, bringing over
      versions from nvm, nodenv, pyenv, uv, or Homebrew (`mise sync`), and GitHub.
- [x] `mise link` from tool pages, with a folder picker.
- [x] Edit list settings (comma-separated, as `mise settings set` takes them).
- [ ] Gate features by mise version (several above need 2026.10.x) and explain why a
      control is disabled. Capture a second fixture set from 2026.10 so both ends of
      the supported range stay tested.

### M5 — Marketing site & docs

- [x] Reposition the site: "A desktop app for mise."
- [x] Hero with a WebGL dithered color field (three.js), pointer-reactive, paused
      offscreen, still under reduced motion.
- [x] A working, interactive demo of the real desktop UI (built in demo mode on the
      browser stub), with a GSAP scroll tour that steers it screen by screen.
- [x] Rewrite docs: what powermise is, getting started with the app, an app tour,
      and how powermise uses mise. AI Setups docs moved under `/ai-setups/`.
- [x] Replace the AI Setups CLI reference with "From the terminal: it's just mise".
- [x] The dithered chef's hat moves to the closing call-to-action.
- [ ] Screenshots / short screen recordings for docs pages once installers ship.
- [ ] Download page with per-platform installers (with M6).

### Cross-platform design ✅ (first pass)

- [x] Brand system: bundled Geist / Geist Mono, coral accent, warm neutrals,
      platform-neutral controls, visible focus rings, 32px targets.
- [x] Per-platform chrome via merged Tauri configs: macOS overlay title bar and
      vibrancy; Windows custom title bar over Mica; Linux system frame.
- [x] Per-platform conventions: ⌘ vs. Ctrl shortcut labels (⌘F / Ctrl+F search),
      dialog button order, scrollbars, platform-aware copy.
- [x] Platform switcher in the browser preview and the website demo.
- [ ] Verify on real Windows 11 and Linux (GNOME and KDE): Mica, caption buttons,
      drag regions, high-DPI scaling, and WebKitGTK font rendering.
- [ ] Windows 11 snap-layout flyout on our maximize button (custom title bars
      don't get it for free; Win+Z and edge snapping still work).
- [ ] In-window app menu on Windows and Linux (About, Check for Updates, Quit);
      confirm the native macOS menu bar includes Edit for copy/paste.
- [ ] Follow the system light/dark change live on all three, and respect Windows
      high-contrast mode.

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
| Cleanup | `prune`, `cache` | ✅ `prune --tools` after removing a setup · planned: cleanup screen (M4) |
| Projects & config | `config ls/get/set`, `fmt`, `edit`, `trust`, `untrust`, `lock` | ✅ `config ls`, `trust` with a config review · planned: `untrust`, `lock`, `fmt` |
| Environment | `env`, `set`, `unset`, `vars`, `secrets`, environments (`MISE_ENV`) | ✅ `env`, `set`/`unset --file` · planned: `vars`, `secrets`, environments |
| Tasks | `tasks ls/info/deps`, `run`, `watch` | ✅ `tasks ls/info`, `run` with arguments · planned: `deps`, `watch` |
| Shell | `activate`, `doctor`, `reshim`, `en`, `exec` | ✅ activation guidance · planned: open terminal, reshim |
| Settings | `settings ls/get/set/unset` | planned (M4) |
| Updating mise | `version`, `self-update` | ✅ |
| Machine setup | `bootstrap`, `dotfiles` | ✅ AI Setups (`bootstrap plan/status`, `bootstrap --only packages,tools`) · planned: machine view (M7) |
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

One section of the app. A setup is a selection file
(`~/.config/powermise/selections/<name>.toml`) compiled into one mise file,
`conf.d/powermise-<name>.toml`, that `mise bootstrap` installs. Needs mise 2026.10.

- [x] Retire the powermise CLI, lifecycle ledger, and native winget/Scoop runner:
      mise 2026.10 covers all of it (`conf.d`, `bootstrap --only`, `bootstrap
      status`, winget/Scoop packages, `prune --tools`).
- [x] Version-gate Apply on mise 2026.10 and point at Health to update.
- [x] `scripts/verify-setup-fragment.sh` checks a compiled setup against real mise
      in CI; `scripts/harness-matrix.sh` regenerates the harness reference table.
- [ ] Configure MCP servers and integrations for real. Needs catalog data per entry
      (launch command, args, credentials) and each agent's settings location and format.
      Until then they're reported as "Manual setup".
- [x] Selection schema v2: drop `machine_roles`, `preferences.auto_prune`, and
      `preferences.update_system_packages`. Version 1 files load and are upgraded
      on the next save.
- [ ] An "Extra apps" picker for setups. `catalog/profiles/apps.toml` (Slack,
      Figma, 1Password, …) was only reachable through machine roles, so it has
      no way in today.
- [ ] Fill catalog gaps: WSL providers for several harnesses, `hermes`, desktop
      apps on Linux and Windows (see the harness reference).
- [ ] Linux, Windows, and WSL verification of the apply path.
- [ ] Offer mise's own MCP server (`mise mcp`) to agents, so they can see and install
      the tools a project uses.

## Decisions

| Decision | Choice |
|---|---|
| Desktop framework | Tauri v2 |
| Audience | People less comfortable in a terminal |
| Writing mise config | Only through mise's commands, with the command shown — except AI Setups, which write only their own `conf.d/powermise-<name>.toml` |
| powermise CLI | None. mise is the CLI; the app shows the mise command for everything |
| Project discovery | Manual add + recents |
| mise versions | 2026.7.18 minimum; 2026.10 recommended; newer features gated by version |
| Name | powermise ("mise-en-place", chef's hat) |

## Open questions

- Whether "selections" (the file format) should be renamed "setups" to match the app.
