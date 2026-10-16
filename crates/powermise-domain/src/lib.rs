//! The AI Setups data model: the curated harness catalog (harnesses, tools,
//! apps, integrations, MCP servers) and a person's selection from it.
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Platform {
    Darwin,
    Linux,
    Win32,
    Wsl,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Catalog {
    pub schema_version: u32,
    pub catalog: String,
    pub catalog_revision: String,
    pub harnesses: Vec<Harness>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Harness {
    pub id: String,
    pub title: String,
    pub description: String,
    pub kind: String,
    pub category: String,
    pub extends: Option<String>,
    pub supported_platforms: Vec<Platform>,
    /// The CLI tooling every installation of this harness needs. Harnesses are
    /// CLI-first: these resolve on every supported platform.
    pub required_tool_ids: Vec<String>,
    /// Desktop companion tools, installed only when the selection opts in
    /// (`preferences.include_desktop_apps`) and only on platforms where a
    /// provider exists. Unavailable companions are reported, never fatal.
    #[serde(default)]
    pub desktop_tool_ids: Vec<String>,
    /// Desktop-first harness: the companions are the deliverable and install
    /// by default when the harness is selected (still excludable through a
    /// machine role's `exclude_tools`). Every claimed platform must have a
    /// provider for every companion — validated in the registry.
    #[serde(default)]
    pub desktop_required: bool,
    pub integration_ids: Vec<String>,
    /// MCP servers a new setup with this harness starts with; the setup can
    /// add or remove any (`Selection::mcp_servers`).
    pub mcp_server_ids: Vec<String>,
    /// The agent CLI that installs this harness's plugins (`claude`, `codex`,
    /// or `gemini`), when it has them.
    #[serde(default)]
    pub plugin_agent: Option<String>,
    /// Where the agent reads its MCP servers, if powermise can configure it.
    #[serde(default)]
    pub mcp_client: Option<McpClient>,
    /// The desktop companion's MCP config, used when the desktop apps are in.
    #[serde(default)]
    pub desktop_mcp_client: Option<McpClient>,
    pub source_file: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ToolCatalog {
    pub schema_version: u32,
    pub catalog: String,
    pub catalog_revision: String,
    pub tools: Vec<Tool>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ToolProviderKind {
    MiseTool,
    /// Comes with the operating system (git and curl on macOS): nothing to
    /// install, so it adds nothing to the mise file.
    System,
    BootstrapApt,
    BootstrapBrew,
    BootstrapBrewCask,
    /// A Mac app from the vendor's download, verified by checksum
    /// (`macos-app:` in `[bootstrap.packages]`), for apps without a usable
    /// Homebrew cask. Needs `url`, `sha256`, `artifact`, and `arch`.
    MacosApp,
    Winget,
    Scoop,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ToolProvider {
    pub kind: ToolProviderKind,
    pub package: String,
    #[serde(default = "default_tool_version")]
    pub version: String,
    /// The executable name a `mise-tool` provider places on `PATH`, when it
    /// differs from the package name (for example `claude-code` ships `claude`
    /// and `ripgrep` ships `rg`). Only meaningful for `mise-tool` providers.
    #[serde(default)]
    pub command: Option<String>,
    pub supported_platforms: Vec<Platform>,
    /// `macos-app` only: the download, its SHA-256, and the app bundle in it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifact: Option<String>,
    /// The Mac architecture a download is for; others skip this provider.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arch: Option<MacArch>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum MacArch {
    Arm64,
    X64,
}

fn default_tool_version() -> String {
    "latest".into()
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Tool {
    pub id: String,
    pub title: String,
    pub description: String,
    pub providers: Vec<ToolProvider>,
    #[serde(default)]
    pub unsupported_reason: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct IntegrationCatalog {
    pub schema_version: u32,
    pub catalog: String,
    pub catalog_revision: String,
    pub integrations: Vec<Integration>,
}

/// A plugin or extension an agent installs with its own CLI.
#[derive(Debug, Deserialize, Serialize)]
pub struct Integration {
    pub id: String,
    pub title: String,
    pub description: String,
    /// The agent CLI that installs it: `claude`, `codex`, or `gemini`.
    pub agent: String,
    /// What that CLI installs: a plugin id such as
    /// `github@anthropic-plugin-directory`, or a Gemini extension's source URL.
    pub plugin: String,
    /// The installed extension's name, when it differs from `plugin` (Gemini
    /// installs from a URL but removes by name).
    #[serde(default)]
    pub installed_name: Option<String>,
    #[serde(default)]
    pub homepage: Option<String>,
}

/// The agent CLIs that install plugins.
pub const PLUGIN_AGENTS: [&str; 3] = ["claude", "codex", "gemini"];

#[derive(Debug, Deserialize, Serialize)]
pub struct McpServerCatalog {
    pub schema_version: u32,
    pub catalog: String,
    pub catalog_revision: String,
    pub mcp_servers: Vec<McpServer>,
}

/// An MCP server: a program mise installs and runs (`tool`, `version`,
/// `command`), a remote server at `url`, or — for servers people add
/// themselves — a `command` already on their computer.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct McpServer {
    pub id: String,
    pub title: String,
    pub description: String,
    #[serde(default)]
    pub homepage: Option<String>,
    /// The mise tool that provides the server, such as `npm:@upstash/context7-mcp`.
    #[serde(default)]
    pub tool: Option<String>,
    #[serde(default)]
    pub version: Option<String>,
    /// The executable the tool puts on `PATH`.
    #[serde(default)]
    pub command: Option<String>,
    #[serde(default)]
    pub args: Vec<String>,
    /// A remote (streamable HTTP) server.
    #[serde(default)]
    pub url: Option<String>,
    /// Environment variables the server runs with (not secrets: they're
    /// written in files powermise keeps).
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub env: std::collections::BTreeMap<String, String>,
}

/// An agent's MCP configuration file. powermise adds servers to it with a
/// mise dotfiles merge entry, which owns only those keys.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum McpClient {
    ClaudeCode,
    ClaudeDesktop,
    Codex,
    GeminiCli,
    Opencode,
    Cursor,
    Vscode,
    CopilotCli,
    Cline,
    Amp,
    Zed,
}

impl McpClient {
    pub fn id(self) -> &'static str {
        match self {
            Self::ClaudeCode => "claude-code",
            Self::ClaudeDesktop => "claude-desktop",
            Self::Codex => "codex",
            Self::GeminiCli => "gemini-cli",
            Self::Opencode => "opencode",
            Self::Cursor => "cursor",
            Self::Vscode => "vscode",
            Self::CopilotCli => "copilot-cli",
            Self::Cline => "cline",
            Self::Amp => "amp",
            Self::Zed => "zed",
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Self::ClaudeCode => "Claude Code",
            Self::ClaudeDesktop => "Claude Desktop",
            Self::Codex => "Codex",
            Self::GeminiCli => "Gemini CLI",
            Self::Opencode => "OpenCode",
            Self::Cursor => "Cursor",
            Self::Vscode => "VS Code",
            Self::CopilotCli => "Copilot CLI",
            Self::Cline => "Cline",
            Self::Amp => "Amp",
            Self::Zed => "Zed",
        }
    }

    /// The config file on `target`, from the home directory, or why
    /// powermise can't configure this client there.
    pub fn config_path(self, target: Platform) -> Result<String, &'static str> {
        // VS Code's user folder, which Cline's settings also live under.
        let vscode = match target {
            Platform::Darwin => "~/Library/Application Support/Code/User",
            Platform::Win32 => "~/AppData/Roaming/Code/User",
            Platform::Linux | Platform::Wsl => "~/.config/Code/User",
        };
        Ok(match self {
            Self::ClaudeCode => "~/.claude.json".into(),
            Self::ClaudeDesktop => match target {
                Platform::Darwin => {
                    "~/Library/Application Support/Claude/claude_desktop_config.json".into()
                }
                Platform::Win32 => "~/AppData/Roaming/Claude/claude_desktop_config.json".into(),
                Platform::Linux | Platform::Wsl => {
                    return Err("Claude's desktop app isn't available on Linux");
                }
            },
            Self::Codex => "~/.codex/config.toml".into(),
            Self::GeminiCli => "~/.gemini/settings.json".into(),
            Self::Opencode => "~/.config/opencode/opencode.json".into(),
            Self::Cursor => "~/.cursor/mcp.json".into(),
            Self::Vscode => format!("{vscode}/mcp.json"),
            Self::CopilotCli => "~/.copilot/mcp-config.json".into(),
            Self::Cline => format!(
                "{vscode}/globalStorage/saoudrizwan.claude-dev/settings/cline_mcp_settings.json"
            ),
            Self::Amp => "~/.config/amp/settings.json".into(),
            Self::Zed => {
                return Err(
                    "Zed's settings file can hold comments, which mise won't merge into; add servers in Zed's Agent Panel settings",
                );
            }
        })
    }

    /// The key that holds the servers, by name.
    pub fn servers_key(self) -> &'static str {
        match self {
            Self::Codex => "mcp_servers",
            Self::Opencode => "mcp",
            Self::Vscode => "servers",
            Self::Amp => "amp.mcpServers",
            Self::Zed => "context_servers",
            _ => "mcpServers",
        }
    }

    /// Every client, in the order powermise lists them.
    pub const ALL: [Self; 11] = [
        Self::ClaudeCode,
        Self::ClaudeDesktop,
        Self::Codex,
        Self::GeminiCli,
        Self::Opencode,
        Self::Cursor,
        Self::Vscode,
        Self::CopilotCli,
        Self::Cline,
        Self::Amp,
        Self::Zed,
    ];

    /// Whether it connects to remote servers (Claude's desktop app only runs
    /// servers itself).
    pub fn takes_remote(self) -> bool {
        !matches!(self, Self::ClaudeDesktop | Self::Zed)
    }

    /// Whether the config file is TOML (Codex); the rest are JSON.
    pub fn is_toml(self) -> bool {
        self == Self::Codex
    }
}

/// The selection file format powermise writes.
///
/// Version 1 (written by the retired CLI) also had `machine_roles`,
/// `preferences.auto_prune`, and `preferences.update_system_packages`. A
/// version 1 file still loads: those fields are ignored, and the selection
/// is upgraded to this version (see [`Selection::upgrade`]) and rewritten as
/// version 2 the next time it's saved.
pub const SELECTION_SCHEMA_VERSION: u32 = 2;

#[derive(Debug, Deserialize, Serialize)]
pub struct Selection {
    pub schema_version: u32,
    pub name: String,
    pub catalog: String,
    pub harnesses: Vec<String>,
    /// MCP servers to configure. Unset means the harnesses' own picks.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mcp_servers: Option<Vec<String>>,
    /// Plugins to install. Unset means the harnesses' own picks.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub integrations: Option<Vec<String>>,
    #[serde(default)]
    pub preferences: Preferences,
}

impl Selection {
    /// Brings a selection read from disk to [`SELECTION_SCHEMA_VERSION`].
    /// Returns `false` for a version this build doesn't know.
    pub fn upgrade(&mut self) -> bool {
        match self.schema_version {
            1 | SELECTION_SCHEMA_VERSION => {
                self.schema_version = SELECTION_SCHEMA_VERSION;
                true
            }
            _ => false,
        }
    }
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct Preferences {
    /// Opt in to installing the harnesses' desktop companion tools on
    /// platforms where a provider exists.
    #[serde(default)]
    pub include_desktop_apps: bool,
}
