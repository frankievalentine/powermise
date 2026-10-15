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
        let installs = dirs.data.join("installs");
        Ok(DiskUsage {
            cache_bytes: directory_size(&dirs.cache),
            installs_bytes: directory_size(&installs),
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
        Ok(MachineStatus {
            sections,
            history,
            origin,
            supports_history,
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
