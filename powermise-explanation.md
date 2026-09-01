Here's how agent installation is structured, grounded in the actual catalog (catalog/profiles/*.toml) and the compiler/lifecycle code.
1. How it's structured
Everything is catalog-driven and resolved per --target:
- harnesses.toml — 18 harnesses. Each extends = "base" and declares required_tool_ids, integration_ids, mcp_server_ids, supported_platforms.
- tools.toml — each tool lists providers (MiseTool, BootstrapBrew, BootstrapBrewCask, Winget) with supported_platforms.
- apps.toml — application bundles (currently only cursor, darwin brew-cask). Apps are opt-in via a machine_role include_apps, not per-harness.
- integrations.toml / mcp.toml — config-adapter entries; these write managed config blocks into the agent's settings (they don't install binaries).
Resolution merges extends (so every selection always pulls base's 5 tools: git, curl, jq, ripgrep, fd), then per target it picks one provider per tool in priority order: MiseTool → BootstrapBrew → BootstrapBrewCask become entries in the generated mise config (installed by mise bootstrap); Winget/Scoop become native Powermise operations (run by apply on win32).
2. Steps to get started, per machine
The flow is identical on every OS; only the install backend differs.
1. Prereqs: mise installed (required — apply runs mise bootstrap); and the platform PM: brew (macOS/Linux), winget or scoop (Windows).
2. Install Powermise: git clone https://github.com/frankievalentine/powermise.git && cd powermise && cargo install --path crates/powermise-cli
3. Author a selection TOML (harnesses + optional machine_roles with include_apps), or powermise selection create/import.
4. Preview per machine: powermise selection show --name X --target <darwin|linux|win32|wsl> then powermise plan --name X --target <t> --yes (plan mutates nothing).
5. Converge: powermise apply --name X --target <t> --yes → writes generated config to ~/.config/mise, records ledger ownership, then runs real mise bootstrap --yes (or --skip-bootstrap to only write config).
6. Shell wiring: powermise shell install --name X adds export MISE_ENV=powermise-<selection> to zshrc/bashrc so the env auto-activates.
Per-target mechanism:
- darwin — most agents via mise-tool (mise-managed) or brew/brew-cask bootstrap; no winget.
- linux / wsl — mise-tool + bootstrap-brew; treated as Linux providers (wsl is a distinct Linux target).
- win32 — cross-platform tools via mise-tool; Windows-only tools (git, curl, jq, ripgrep, fd, vscode, node) via winget native (Git.Git, curl.curl, jqlang.jq, BurntSushi.ripgrep.MSVC, sharkdp.fd, Microsoft.VisualStudioCode, OpenJS.NodeJS).
3. What each harness installs
Base 5 (git curl jq ripgrep fd) are implied for every harness below. "Platforms" = targets where it currently compiles (gaps flagged).
Harness
base
antigravity
claude
cline
codex
conductor
cursor
gemini
github-copilot
goose
hermes
openclaw
opencode
paseo
pi
superset
t3code
zed
4. App lists (to verify completeness)
Apps are not per-harness — they come from a machine_roles.<role>.include_apps in the selection. The entire app catalog today is:
- cursor → brew-cask, darwin only.
So on Linux/Windows there are currently no app packages, and selecting cursor as an app only works on macOS. Integrations/MCP above are config-only (managed blocks in the agent's settings), not installed binaries.
Gaps worth your verification
The harness supported_platforms claims are broader than what the tools actually support, so these currently fail to resolve outside the flagged platforms: claude (linux/win32/wsl lack claude-app), codex (linux/wsl lack codex-app), cursor/superset/zed (linux/wsl lack a non-darwin provider), github-copilot & cline (linux/wsl lack a vscode provider), and conductor/hermes are uninstallable everywhere (empty tool providers).
I can run powermise selection compile --target <t> per harness to produce the exact mise TOML/native op list as the definitive check — say the word and I'll build the CLI and generate all of them.
