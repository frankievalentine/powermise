//! Machine-wide mise care: settings, GitHub downloads, cleaning up unused
//! versions, bringing in versions other managers installed, and installing
//! mise itself when it's missing. Reads here; changes are `MiseAction`s.
use std::{
    path::PathBuf,
    process::{Command, Stdio},
};

use powermise_mise::{DetectedImport, Setting, TokenStatus, directory_size, trusted_configs};
use serde::{Deserialize, Serialize};
use tauri::AppHandle;
use tauri_plugin_dialog::DialogExt;

use crate::{CommandResult, find_program, home_dir, message, mise_commands::client};

/// Runs blocking work off the main thread.
async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> CommandResult<T> + Send + 'static,
) -> CommandResult<T> {
    tauri::async_runtime::spawn_blocking(work)
        .await
        .map_err(message)?
}

/// Every mise setting with its value and where it's set.
#[tauri::command]
pub async fn settings() -> CommandResult<Vec<Setting>> {
    blocking(|| client()?.settings().map_err(message)).await
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GithubStatus {
    /// What `mise token github` reports, as the app's mise sees it.
    token: TokenStatus,
    /// The GitHub CLI is installed and signed in, so mise can borrow its
    /// login through `github.credential_command`.
    gh_signed_in: bool,
    /// Current values of the settings that pick up existing logins.
    credential_command: String,
    use_git_credentials: bool,
}

#[tauri::command]
pub async fn github_status() -> CommandResult<GithubStatus> {
    blocking(|| {
        let client = client()?;
        let token = client.github_token().map_err(message)?;
        let settings = client.settings().map_err(message)?;
        let value = |key: &str| {
            settings
                .iter()
                .find(|setting| setting.key == key)
                .map(|setting| setting.value.clone())
        };
        Ok(GithubStatus {
            token,
            gh_signed_in: gh_signed_in(),
            credential_command: value("github.credential_command")
                .and_then(|value| value.as_str().map(str::to_owned))
                .unwrap_or_default(),
            use_git_credentials: value("github.use_git_credentials")
                .and_then(|value| value.as_bool())
                .unwrap_or(false),
        })
    })
    .await
}

/// Whether `gh auth token` succeeds. Its output (the token) is discarded.
fn gh_signed_in() -> bool {
    find_program("gh").is_some_and(|gh| {
        Command::new(gh)
            .args(["auth", "token", "--hostname", "github.com"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|status| status.success())
    })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PrunableVersion {
    tool: String,
    version: String,
    install_path: PathBuf,
    bytes: u64,
}

/// Installed versions no config uses (`mise ls --prunable --json`), with the
/// disk space each takes.
#[tauri::command]
pub async fn prunable_versions() -> CommandResult<Vec<PrunableVersion>> {
    blocking(|| {
        let inventory = client()?.prunable().map_err(message)?;
        Ok(inventory
            .into_iter()
            .flat_map(|(tool, versions)| {
                versions.into_iter().map(move |entry| PrunableVersion {
                    bytes: directory_size(&entry.install_path),
                    install_path: entry.install_path,
                    version: entry.version,
                    tool: tool.clone(),
                })
            })
            .collect())
    })
    .await
}

/// Versions nvm, nodenv, pyenv, uv, or Homebrew installed that mise can use.
#[tauri::command]
pub async fn detect_imports() -> CommandResult<Vec<DetectedImport>> {
    blocking(|| {
        let home = home_dir().ok_or("could not determine your home directory")?;
        Ok(powermise_mise::detect_imports(&home))
    })
    .await
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiskUsage {
    cache_dir: PathBuf,
    cache_bytes: u64,
    installs_dir: PathBuf,
    installs_bytes: u64,
}

/// How much space mise's download cache and installed tools take.
#[tauri::command]
pub async fn disk_usage() -> CommandResult<DiskUsage> {
    blocking(|| {
        let dirs = client()?
            .doctor()
            .map_err(message)?
            .dirs
            .ok_or("this mise doesn't report its folders")?;
        // mise's defaults, unless moved (https://mise.jdx.dev/directories.html).
        let installs = std::env::var_os("MISE_INSTALLS_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| dirs.data.join("installs"));
        // The identity layout keeps installations in a separate store (`i\`
        // beside `installs` on Windows) and only links them from `installs`.
        let store = std::env::var_os("MISE_INSTALL_STORE_DIR")
            .map(PathBuf::from)
            .or_else(|| cfg!(windows).then(|| dirs.data.join("i")))
            .filter(|store| store.is_dir() && !store.starts_with(&installs));
        Ok(DiskUsage {
            cache_bytes: directory_size(&dirs.cache),
            installs_bytes: directory_size(&installs)
                + store.as_deref().map(directory_size).unwrap_or_default(),
            cache_dir: dirs.cache,
            installs_dir: installs,
        })
    })
    .await
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrustedPath {
    path: PathBuf,
    exists: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrustReport {
    trusted: Vec<TrustedPath>,
    ignored: Vec<PathBuf>,
}

/// Folders and files mise trusts to run their hooks, tasks, and env, and the
/// configs it was told to ignore.
#[tauri::command]
pub async fn trusted_folders() -> CommandResult<TrustReport> {
    blocking(|| {
        let doctor = client()?.doctor().map_err(message)?;
        let state = doctor
            .dirs
            .map(|dirs| dirs.state)
            .ok_or("this mise doesn't report its folders")?;
        Ok(TrustReport {
            trusted: trusted_configs(&state)
                .into_iter()
                .map(|path| TrustedPath {
                    exists: path.exists(),
                    path,
                })
                .collect(),
            ignored: doctor.ignored_config_files,
        })
    })
    .await
}

/// Opens the terminal app on `mise dotfiles conflicts`, which compares both
/// sides of a sync conflict interactively.
#[tauri::command]
pub fn resolve_dotfile_conflicts() -> CommandResult<()> {
    let home = home_dir().ok_or("could not determine your home directory")?;
    let mise = crate::find_mise().ok_or("mise is not installed")?;
    crate::terminal::open(&home, &mise, &["dotfiles".into(), "conflicts".into()]).map_err(message)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AliasView {
    alias: String,
    version: String,
    /// Set in your global config (so it can be removed), not built into mise.
    custom: bool,
}

/// Version aliases for a tool, marking the ones your global config defines
/// (`[tool_alias.<tool>]`, or the older `[alias.<tool>]`).
#[tauri::command]
pub async fn tool_aliases(tool: String) -> CommandResult<Vec<AliasView>> {
    blocking(move || {
        let aliases = client()?.tool_aliases(&tool).map_err(message)?;
        let config = crate::mise_config_dir()?.join("config.toml");
        // `mise tool-alias set` writes `[tool_alias.<tool>.versions]`; older
        // configs put string values right under `[tool_alias.<tool>]` or
        // `[alias.<tool>]`.
        let custom: Vec<String> = std::fs::read_to_string(config)
            .ok()
            .and_then(|contents| contents.parse::<toml::Table>().ok())
            .map(|table| {
                let mut names = Vec::new();
                for section in ["tool_alias", "alias"] {
                    let Some(entry) = table
                        .get(section)
                        .and_then(|value| value.get(&tool))
                        .and_then(toml::Value::as_table)
                    else {
                        continue;
                    };
                    if let Some(versions) = entry.get("versions").and_then(toml::Value::as_table) {
                        names.extend(versions.keys().cloned());
                    }
                    names.extend(
                        entry
                            .iter()
                            .filter(|(_, value)| value.is_str())
                            .map(|(name, _)| name.clone()),
                    );
                }
                names
            })
            .unwrap_or_default();
        Ok(aliases
            .into_iter()
            .map(|alias| AliasView {
                custom: custom.contains(&alias.alias),
                alias: alias.alias,
                version: alias.version,
            })
            .collect())
    })
    .await
}

/// Legacy asdf/vfox plugins, read-only.
#[tauri::command]
pub async fn plugins() -> CommandResult<Vec<powermise_mise::Plugin>> {
    blocking(|| client()?.plugins().map_err(message)).await
}

/// The state of every declared system package, keyed `manager:package`
/// (`installed`, `missing`, …), from `mise bootstrap status --json`.
#[tauri::command]
pub async fn package_states() -> CommandResult<std::collections::BTreeMap<String, String>> {
    blocking(|| {
        let status = client()?.bootstrap_status().map_err(message)?;
        Ok(status
            .packages
            .into_iter()
            .flat_map(|(manager, entry)| {
                entry
                    .packages
                    .into_iter()
                    .map(move |package| (format!("{manager}:{}", package.package), package.state))
            })
            .collect())
    })
    .await
}

/// Opens the terminal app running `mise bootstrap packages use -g --yes
/// <spec>`, for apt packages, whose `sudo` needs a password.
#[tauri::command]
pub fn use_package_in_terminal(spec: String) -> CommandResult<()> {
    powermise_mise::validate_package_spec(&spec).map_err(message)?;
    let home = home_dir().ok_or("could not determine your home directory")?;
    let mise = crate::find_mise().ok_or("mise is not installed")?;
    crate::terminal::open(
        &home,
        &mise,
        &[
            "bootstrap".into(),
            "packages".into(),
            "use".into(),
            "-g".into(),
            "--yes".into(),
            spec,
        ],
    )
    .map_err(message)
}

/// A folder picker, for linking an install made outside mise.
#[tauri::command]
pub async fn pick_folder(app: AppHandle) -> CommandResult<Option<PathBuf>> {
    blocking(move || {
        Ok(app
            .dialog()
            .file()
            .blocking_pick_folder()
            .and_then(|picked| picked.into_path().ok()))
    })
    .await
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MachineStatus {
    sections: Vec<powermise_mise::MachineSection>,
    /// Dotfile checkpoints (mise 2026.10+), when history is on.
    history: Option<powermise_mise::DotfilesHistory>,
    /// The repository checkpoints sync with, if connected (2026.10+).
    origin: Option<String>,
    /// Whether this mise has dotfile history at all (2026.10+).
    supports_history: bool,
    /// The newest checkpoints, when history is on.
    checkpoints: Vec<powermise_mise::DotfileCheckpoint>,
}

/// What the global config's `[bootstrap]` and `[dotfiles]` set up on this
/// computer, and the state of each part.
#[tauri::command]
pub async fn machine_status() -> CommandResult<MachineStatus> {
    blocking(|| {
        let client = client()?;
        let sections = client.machine_status().map_err(message)?;
        let supports_history = crate::mise_commands::mise_version(&client)
            .is_some_and(|version| version >= powermise_mise::MiseVersion::RECOMMENDED);
        let (history, origin) = if supports_history {
            (
                client
                    .dotfiles_history()
                    .ok()
                    .filter(|history| history.enabled),
                client.dotfiles_origin().ok().flatten(),
            )
        } else {
            (None, None)
        };
        let checkpoints = if history.is_some() {
            client.dotfile_checkpoints(10).unwrap_or_default()
        } else {
            Vec::new()
        };
        Ok(MachineStatus {
            sections,
            history,
            origin,
            supports_history,
            checkpoints,
        })
    })
    .await
}

/// Ways to install mise on this computer, best first.
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum MiseInstaller {
    Homebrew,
    Winget,
    Scoop,
    /// mise's official install script (macOS and Linux).
    Script,
}

impl MiseInstaller {
    fn program_name(self) -> &'static str {
        match self {
            Self::Homebrew => "brew",
            Self::Winget => "winget",
            Self::Scoop => "scoop",
            Self::Script => "sh",
        }
    }

    pub(crate) fn program(self) -> Option<PathBuf> {
        if matches!(self, Self::Script) && cfg!(windows) {
            return None;
        }
        find_program(self.program_name())
    }

    pub(crate) fn arguments(self) -> Vec<String> {
        let arguments: &[&str] = match self {
            Self::Homebrew => &["install", "mise"],
            Self::Winget => &[
                "install",
                "--id",
                "jdx.mise",
                "--exact",
                "--source",
                "winget",
                "--accept-source-agreements",
                "--accept-package-agreements",
            ],
            Self::Scoop => &["install", "mise"],
            Self::Script => &["-c", "curl -fsSL https://mise.run | sh"],
        };
        arguments
            .iter()
            .map(|argument| (*argument).to_owned())
            .collect()
    }

    /// The command line as a person would type it.
    pub(crate) fn display(self) -> String {
        match self {
            Self::Script => "curl -fsSL https://mise.run | sh".into(),
            _ => format!("{} {}", self.program_name(), self.arguments().join(" ")),
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Homebrew => "Homebrew",
            Self::Winget => "winget",
            Self::Scoop => "Scoop",
            Self::Script => "mise's install script",
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallerOption {
    id: MiseInstaller,
    label: &'static str,
    command: String,
}

#[tauri::command]
pub fn mise_installers() -> Vec<InstallerOption> {
    let order: &[MiseInstaller] = if cfg!(windows) {
        &[MiseInstaller::Winget, MiseInstaller::Scoop]
    } else {
        &[MiseInstaller::Homebrew, MiseInstaller::Script]
    };
    order
        .iter()
        .filter(|installer| installer.program().is_some())
        .map(|&installer| InstallerOption {
            id: installer,
            label: installer.label(),
            command: installer.display(),
        })
        .collect()
}
