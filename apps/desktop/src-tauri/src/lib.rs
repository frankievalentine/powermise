//! powermise desktop backend: an unofficial GUI companion for mise.
//!
//! - `mise_commands`: what mise manages, read through its JSON output and
//!   changed only by running mise commands (streamed, cancellable, logged).
//! - `projects`: folders the person adds, read through mise from inside them.
//! - `setups`: AI Setups — the curated catalog compiled into one mise config
//!   fragment, `conf.d/powermise-<name>.toml`, that mise then applies.
//! - This module: shared environment helpers and app startup.
mod mise_commands;
mod projects;
mod setups;

use std::{
    env,
    path::{Path, PathBuf},
};

use powermise_domain::Platform;
use powermise_setups::{PowermiseLayout, host_platform, resolve_mise_config_dir};
use serde::Serialize;

/// Errors cross the IPC boundary as their display message.
pub(crate) type CommandResult<T> = Result<T, String>;

pub(crate) fn message(error: impl std::fmt::Display) -> String {
    error.to_string()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct EnvironmentInfo {
    host_target: Platform,
    powermise_dir: PathBuf,
    mise_config_dir: PathBuf,
    mise_path: Option<PathBuf>,
}

/// Directories GUI-launched processes commonly miss: apps started from Finder,
/// the Dock, or a launcher don't inherit the login shell's PATH, so mise (and
/// the package managers it drives) would otherwise be invisible.
fn extra_path_dirs() -> Vec<PathBuf> {
    let mut directories = Vec::new();
    if let Some(home) = home_dir() {
        directories.push(home.join(".local/bin"));
        directories.push(home.join(".local/share/mise/bin"));
        directories.push(home.join(".cargo/bin"));
    }
    if cfg!(target_os = "macos") {
        directories.push(PathBuf::from("/opt/homebrew/bin"));
        directories.push(PathBuf::from("/usr/local/bin"));
    } else if cfg!(target_os = "linux") {
        directories.push(PathBuf::from("/home/linuxbrew/.linuxbrew/bin"));
        directories.push(PathBuf::from("/usr/local/bin"));
    }
    directories
}

pub(crate) fn home_dir() -> Option<PathBuf> {
    env::var_os("HOME")
        .or_else(|| env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

/// The inherited PATH plus the common tool directories it's missing.
fn augmented_path() -> Option<std::ffi::OsString> {
    let current = env::var_os("PATH").unwrap_or_default();
    let mut directories = env::split_paths(&current).collect::<Vec<_>>();
    for directory in extra_path_dirs() {
        if directory.is_dir() && !directories.contains(&directory) {
            directories.push(directory);
        }
    }
    env::join_paths(directories).ok()
}

/// Finds the `mise` executable on PATH.
pub(crate) fn find_mise() -> Option<PathBuf> {
    let names: &[&str] = if cfg!(windows) {
        &["mise.exe", "mise.cmd"]
    } else {
        &["mise"]
    };
    let path = env::var_os("PATH")?;
    env::split_paths(&path)
        .flat_map(|directory| names.iter().map(move |name| directory.join(name)))
        .find(|candidate| candidate.is_file())
}

pub(crate) fn layout() -> CommandResult<PowermiseLayout> {
    PowermiseLayout::resolve(None).map_err(message)
}

pub(crate) fn mise_config_dir() -> CommandResult<PathBuf> {
    resolve_mise_config_dir(None).map_err(message)
}

#[tauri::command]
fn get_environment() -> CommandResult<EnvironmentInfo> {
    Ok(EnvironmentInfo {
        host_target: host_platform(),
        powermise_dir: layout()?.root,
        mise_config_dir: mise_config_dir()?,
        mise_path: find_mise(),
    })
}

/// Reveals a path in the platform file manager.
#[tauri::command]
fn reveal_path(path: PathBuf) -> CommandResult<()> {
    let target = if path.is_dir() {
        path
    } else {
        path.parent().map(Path::to_path_buf).unwrap_or(path)
    };
    let opener = if cfg!(target_os = "macos") {
        "open"
    } else if cfg!(windows) {
        "explorer"
    } else {
        "xdg-open"
    };
    std::process::Command::new(opener)
        .arg(&target)
        .spawn()
        .map(|_| ())
        .map_err(message)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    if let Some(path) = augmented_path() {
        // SAFETY: runs first in `main`, before Tauri or any other thread
        // starts, so nothing can read the environment concurrently.
        unsafe { env::set_var("PATH", path) };
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(mise_commands::MiseState::default())
        .invoke_handler(tauri::generate_handler![
            get_environment,
            reveal_path,
            mise_commands::mise_info,
            mise_commands::installed_tools,
            mise_commands::outdated_tools,
            mise_commands::registry,
            mise_commands::remote_versions,
            mise_commands::doctor,
            mise_commands::describe_action,
            mise_commands::run_action,
            mise_commands::cancel_job,
            mise_commands::activity,
            projects::list_projects,
            projects::add_project,
            projects::forget_project,
            projects::project_overview,
            projects::task_details,
            setups::get_catalog,
            setups::list_selections,
            setups::save_selection,
            setups::preview_selection,
            setups::write_setup,
            setups::setup_status,
            setups::bootstrap_plan,
            setups::remove_setup,
        ])
        .run(tauri::generate_context!())
        .expect("error while running the powermise desktop app");
}
