//! Typed models of mise's `--json` output. Fields mise may omit are optional or
//! defaulted, and unknown fields are ignored, so newer mise releases that add
//! data keep parsing. Shapes are pinned by fixtures captured from real mise
//! (`tests/fixtures`).
use std::{collections::BTreeMap, path::PathBuf};

use serde::{Deserialize, Serialize};

/// The config file that requested a tool version.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ConfigSource {
    /// The config format, for example `mise.toml` or `.tool-versions`.
    #[serde(rename = "type")]
    pub kind: String,
    pub path: PathBuf,
}

/// One version of a tool from `mise ls --json`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ToolVersion {
    pub version: String,
    /// The version a config file asked for (`"1.8"` resolving to `"1.8.2"`).
    #[serde(default)]
    pub requested_version: Option<String>,
    pub install_path: PathBuf,
    /// Absent for installed versions no config file requests.
    #[serde(default)]
    pub source: Option<ConfigSource>,
    #[serde(default)]
    pub installed: bool,
    #[serde(default)]
    pub active: bool,
}

/// `mise ls --json`: versions by tool name.
pub type ToolInventory = BTreeMap<String, Vec<ToolVersion>>;

/// One entry of `mise outdated --json`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct OutdatedTool {
    pub name: String,
    #[serde(default)]
    pub requested: Option<String>,
    /// The installed version, or `None` when the requested one is missing.
    #[serde(default)]
    pub current: Option<String>,
    /// With `--bump`: the newer version that would replace a pinned request.
    #[serde(default)]
    pub bump: Option<String>,
    pub latest: String,
    #[serde(default)]
    pub source: Option<ConfigSource>,
}

/// `mise outdated --json`: outdated tools by name.
pub type OutdatedReport = BTreeMap<String, OutdatedTool>;

/// One tool of `mise registry --json`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RegistryEntry {
    pub short: String,
    #[serde(default)]
    pub backends: Vec<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub aliases: Vec<String>,
}

/// One version of `mise ls-remote --json <tool>`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RemoteVersion {
    pub version: String,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub release_url: Option<String>,
}

/// One config file of `mise config ls --json`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ConfigFile {
    pub path: PathBuf,
    #[serde(default)]
    pub tools: Vec<String>,
}

/// One task of `mise tasks ls --json` (the fields powermise shows).
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Task {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub aliases: Vec<String>,
    pub source: PathBuf,
    #[serde(default)]
    pub dir: Option<PathBuf>,
    #[serde(default)]
    pub hide: bool,
    #[serde(default)]
    pub run: Vec<String>,
}

/// `mise doctor --json` (the fields powermise shows).
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Doctor {
    pub version: String,
    /// Whether `mise activate` is hooked into the current shell.
    #[serde(default)]
    pub activated: bool,
    #[serde(default)]
    pub shims_on_path: bool,
    #[serde(default)]
    pub self_update_available: bool,
    #[serde(default)]
    pub config_files: Vec<PathBuf>,
    #[serde(default)]
    pub errors: Vec<String>,
    #[serde(default)]
    pub warnings: Vec<String>,
    /// The shell mise was run from, when it can tell.
    #[serde(default)]
    pub shell: Option<ShellInfo>,
    /// Where mise keeps its cache, installs, shims, and state.
    #[serde(default)]
    pub dirs: Option<MiseDirs>,
    /// Config files the person told mise to ignore (`mise trust --ignore`).
    #[serde(default)]
    pub ignored_config_files: Vec<PathBuf>,
}

/// The `dirs` section of `mise doctor --json`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct MiseDirs {
    pub cache: PathBuf,
    pub config: PathBuf,
    pub data: PathBuf,
    pub shims: PathBuf,
    pub state: PathBuf,
}

/// The `shell` section of `mise doctor --json`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ShellInfo {
    /// For example `/bin/zsh`.
    pub name: String,
    #[serde(default)]
    pub version: Option<String>,
}

/// `mise version --json`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct VersionInfo {
    /// For example `2026.7.18 macos-arm64 (2026-07-30)`.
    pub version: String,
    #[serde(default)]
    pub latest: Option<String>,
}

/// `mise bootstrap plan --json`: what applying the config would change.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct BootstrapPlan {
    #[serde(default)]
    pub resources: Vec<PlanResource>,
    #[serde(default)]
    pub summary: PlanSummary,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PlanResource {
    pub id: PlanResourceId,
    #[serde(default)]
    pub current: String,
    #[serde(default)]
    pub desired: String,
    /// `create`, `update`, `remove`, `unchanged`, or `unknown`.
    #[serde(default)]
    pub action: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PlanResourceId {
    /// For example `package`, `tool`, `dotfile`, `service`.
    pub kind: String,
    /// For packages, `manager:package` (`brew:jq`).
    pub name: String,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct PlanSummary {
    #[serde(default)]
    pub create: u32,
    #[serde(default)]
    pub update: u32,
    #[serde(default)]
    pub remove: u32,
    #[serde(default)]
    pub unchanged: u32,
    #[serde(default)]
    pub unknown: u32,
}

/// The parts of `mise bootstrap status --json` an AI setup needs: packages by
/// manager, and tools.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct BootstrapStatus {
    #[serde(default)]
    pub packages: BTreeMap<String, PackageManagerStatus>,
    #[serde(default)]
    pub tools: Vec<ToolStatus>,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct PackageManagerStatus {
    /// Whether the manager (Homebrew, apt, WinGet, …) is on this machine.
    #[serde(default)]
    pub available: bool,
    #[serde(default)]
    pub packages: Vec<PackageStatus>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PackageStatus {
    pub package: String,
    #[serde(default)]
    pub requested_version: String,
    /// `installed`, `missing`, `outdated`, …
    #[serde(default)]
    pub state: String,
    #[serde(default)]
    pub installed_version: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ToolStatus {
    pub tool: String,
    #[serde(default)]
    pub requested_version: String,
    #[serde(default)]
    pub resolved_version: String,
    #[serde(default)]
    pub state: String,
    #[serde(default)]
    pub installed: bool,
}

/// A calendar version `YYYY.M.P` as mise numbers its releases.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct MiseVersion {
    pub year: u32,
    pub minor: u32,
    pub patch: u32,
}

impl MiseVersion {
    /// The oldest mise powermise supports: the release its `--json` contracts
    /// were captured from.
    pub const MINIMUM: Self = Self {
        year: 2026,
        minor: 7,
        patch: 18,
    };

    /// The release powermise recommends and is designed around. Older
    /// supported versions work, but features that need newer mise (for
    /// example `outdated --global`, auto-updates, `[vars]`) are disabled.
    pub const RECOMMENDED: Self = Self {
        year: 2026,
        minor: 10,
        patch: 0,
    };

    /// Parses the leading `YYYY.M.P` of a mise version string.
    pub fn parse(text: &str) -> Option<Self> {
        let mut parts = text.split_whitespace().next()?.split('.');
        let mut next = || parts.next()?.parse::<u32>().ok();
        Some(Self {
            year: next()?,
            minor: next()?,
            patch: next()?,
        })
    }
}

impl std::fmt::Display for MiseVersion {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}.{}.{}", self.year, self.minor, self.patch)
    }
}
