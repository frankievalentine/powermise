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

This file is the living plan.

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
- [x] Tool pages from `mise tool --json`: backend in use, tool options, homepage,
      and download checks (checksums, GitHub attestations, SLSA, signatures).
- [x] Install anything mise supports beyond the registry: search a full
      `backend:name`, with the sources from `mise backends ls`.
- [x] Split Updates into global and per-project tools (`--global` on 2026.10.7+;
      `outdated|upgrade --local` in each added project).
- [x] Automatic updates: `tool_update.global_auto` and `self_update.auto`
      switches, shown when the installed mise has them.
- [x] Per-tool `auto_update` on tool pages (`mise config set --global
      tools.<tool>.auto_update`), when the installed mise has the option.
- [x] Persist the activity log across launches (newest 500, in powermise's state).

### M3 — Projects (first pass ✅)

- [x] Add project folders (picker, drag and drop, recents; no disk scanning).
- [x] Tools per project (project vs. inherited), **Install Missing** (`mise install`),
      **Add Tool** (`mise use` in the folder).
- [x] Call out idiomatic version files (`.nvmrc`, `.python-version`, …) mise
      isn't reading, and turn them on (`mise settings add
      idiomatic_version_file_enable_tools <tool>`).
- [x] Config files (`mise config ls`) in precedence order with plain-language roles.
- [x] Edit config values through `mise config set --file`.
- [x] **Environments** (`MISE_ENV`, `mise.<env>.toml`): a picker that previews what
      mise loads for each environment.
- [x] Environment variables from `mise env --json-extended` with their source file;
      `redact = true` and secret-looking names masked until revealed; add to
      `mise.toml` or `mise.local.toml` and remove from the defining file
      (`mise set|unset --file`); values never shown in the activity log.
- [x] **Tasks**: list, run with live output and Stop, and a form for arguments,
      flags, and choices generated from `mise tasks info --json`; dependencies noted.
- [x] Dependency tree (built from `tasks ls --json`, since `tasks deps` has no
      JSON), "Watch" (`mise watch`, installing watchexec first), and opening
      interactive tasks in the terminal app.
- [x] Trust flow: untrusted folders show a read-only preview of hooks, loaded files,
      variables, tools, and tasks before a user-confirmed `mise trust`.
- [x] Untrust (`mise untrust`) from the project page.
- [x] Lockfiles: whether `mise.lock` is current (`mise lock --dry-run --json`), and
      create or update it (`mise lock`).
- [x] "Open a terminal with this project's tools" (`mise en`).
- [x] Generators: GitHub Actions workflow, git pre-commit hook, and devcontainer
      (`mise generate …`), previewed before `--write`.
- [x] Per-machine `[vars]` (`mise vars`, 2026.10.6+): save and forget values for
      vars declared with a prompt, default, or `required`. Secret names and
      source (`mise secrets ls --json`, experimental), read-only.

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
- [x] Gate features by mise version and say why when one is unavailable: AI Setups
      (2026.10), `--global` updates (2026.10.7), `[vars]` (2026.10.6), automatic
      updates and dotfile history (2026.10). Fixture sets for 2026.7.18 and
      2026.10.10 keep both ends of the supported range tested.

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

- [x] Machine view from `mise bootstrap status --json`: every part, flattened so
      parts newer mise adds still show, with what isn't applied yet.
- [x] Preview (`mise bootstrap --dry-run`) and apply (`mise bootstrap --yes`), for
      everything or one part (`--only <part>`).
- [x] Dotfiles: their state comes with bootstrap status; on 2026.10, checkpoint
      history (`mise dotfiles status --json`), save, sync, and connecting an origin.
- [x] macOS Settings: the macos-defaults.com catalog (72 settings, generated by
      `scripts/macos-defaults-catalog.py`), saved to `conf.d/powermise-macos.toml`
      and applied with `mise bootstrap --only macos-defaults`; relaunch Dock,
      Finder, and other apps afterwards.
- [x] AI Setups are bootstrap templates: each compiles to a `conf.d` file whose
      `[bootstrap.packages]` show up in the Machine view.
- [x] Dotfile history: recent checkpoints (`mise dotfiles history --json`), preview
      and roll back to one (`rollback --to <id> --all --dry-run|--yes`), undo the
      latest operation, and resolve sync conflicts in the terminal app
      (`mise dotfiles conflicts` is interactive).

## mise coverage

What the GUI does with each area of mise. "Planned" items are in the milestones above.

| mise area | Commands | In the GUI |
|---|---|---|
| Tools | `ls`, `use`, `unuse`, `install`, `uninstall`, `upgrade`, `outdated`, `ls-remote`, `latest`, `which`, `where` | ✅ Installed, Discover, tool pages, Updates |
| Registry & backends | `registry`, `search`, `tool`, `backends`, `tool-alias` | ✅ registry, tool details (`tool --json`), any-backend installs, version aliases |
| Cleanup | `prune`, `cache` | ✅ Clean Up (`ls --prunable`, `prune --tools --yes`), cache prune/clear, sizes |
| Projects & config | `config ls/get/set`, `fmt`, `edit`, `trust`, `untrust`, `lock` | ✅ `config ls`, `config set --file`, `fmt`, `trust` with a config review, `untrust`, `lock` |
| Environment | `env`, `set`, `unset`, `vars`, `secrets`, environments (`MISE_ENV`) | ✅ `env`, `set`/`unset --file`, `vars`, `secrets` (names), environment preview |
| Tasks | `tasks ls/info/deps`, `run`, `watch` | ✅ `tasks ls/info`, `run` with arguments, dependency tree, `watch`, interactive tasks in the terminal |
| Shell | `activate`, `doctor`, `reshim`, `en`, `exec` | ✅ activation guidance, `reshim`, open a terminal (`en`) |
| Settings | `settings ls/get/set/unset/add` | ✅ Settings screen; automatic-update switches |
| Updating mise | `version`, `self-update` | ✅ |
| Machine setup | `bootstrap`, `dotfiles` | ✅ Machine view (`bootstrap status`, `--dry-run`, `--yes [--only]`), dotfile checkpoints, save, sync, origin; AI Setups |
| Migration | `sync`, `link` | ✅ Welcome and Health (`sync`), tool pages (`link`) |
| Generators | `generate github-action/git-pre-commit/devcontainer` | ✅ previewed, then `--write` |
| Auth | `token` | ✅ GitHub token status and borrowing a gh or Git login |
| Plugins (asdf/vfox) | `plugins ls/install/update/uninstall` | ✅ listed read-only on Health (legacy path; installs stay in the terminal) |

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
- [x] MCP Servers page: turn servers on per agent outside setups
      (`conf.d/powermise-mcp.toml`), read what each agent's settings list now, and
      add your own servers (npm, Python, mise tool, command, or remote, with env).
- [ ] MCP API keys: a Configure sheet that writes keys only into the agent's own
      settings, never into mise's config or powermise's files.
- [x] Official company MCP servers (GitHub, Linear, Notion, Stripe, …) in the
      catalog, with addresses from each company's domain-verified entry in the
      official MCP Registry. A full registry search was tried and dropped: it's
      an open feed of unreviewed servers, so anything beyond the curated list
      goes through Add Server.
- [x] Selection schema v2: drop `machine_roles`, `preferences.auto_prune`, and
      `preferences.update_system_packages`. Version 1 files load and are upgraded
      on the next save.
- [x] Drop the unused apps catalog (`apps.toml`: Slack, Figma, 1Password, …)
      and the Agent Tools tab that listed it; a harness's page shows what it
      installs.
- [x] Install-source order: mise → comes with the system → Homebrew → cask → apt →
      winget → Scoop. mise installs Homebrew formulas and casks itself (no
      Homebrew needed). git and curl come with macOS; codex, gemini-cli, and
      opencode install through mise everywhere; WSL gets Linux providers.
- [x] Intel Macs: Homebrew formulas are skipped (mise needs Apple Silicon for
      them) and listed in the preview; casks still install.
- [x] Linux/WSL: the one-time `sudo` for `/home/linuxbrew/.linuxbrew` runs in the
      terminal app (Apply in Terminal); Run in Terminal for apt as well.
- [ ] Linux desktop apps via Flatpak (`flatpak-user:`) once catalog apps have
      developer-verified Flathub listings (none do today).
- [ ] Fill catalog gaps: `hermes`, desktop
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
