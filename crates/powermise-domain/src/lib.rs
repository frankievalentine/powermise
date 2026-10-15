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
    pub mcp_server_ids: Vec<String>,
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

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ConfigProviderKind {
    ConfigAdapter,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ConfigProvider {
    pub kind: ConfigProviderKind,
    pub config_target: String,
    pub supported_platforms: Vec<Platform>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct AppCatalog {
    pub schema_version: u32,
    pub catalog: String,
    pub catalog_revision: String,
    pub apps: Vec<App>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct App {
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

#[derive(Debug, Deserialize, Serialize)]
pub struct Integration {
    pub id: String,
    pub title: String,
    pub description: String,
    pub providers: Vec<ConfigProvider>,
    #[serde(default)]
    pub unsupported_reason: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct McpServerCatalog {
    pub schema_version: u32,
    pub catalog: String,
    pub catalog_revision: String,
    pub mcp_servers: Vec<McpServer>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct McpServer {
    pub id: String,
    pub title: String,
    pub description: String,
    pub providers: Vec<ConfigProvider>,
    #[serde(default)]
    pub unsupported_reason: Option<String>,
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
