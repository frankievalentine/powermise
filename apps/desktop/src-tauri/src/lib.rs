//! powermise desktop backend: an unofficial GUI companion for mise.
//!
//! - `mise_commands`: what mise manages, read through its JSON output and
//!   changed only by running mise commands (streamed, cancellable, logged).
//! - `projects`: folders the person adds, read through mise from inside them.
//! - `machine`: settings, GitHub downloads, cleanup, importing versions from
//!   other managers, and installing mise itself on first run.
//! - `setups`: AI Setups — the curated catalog compiled into one mise config
//!   fragment, `conf.d/powermise-<name>.toml`, that mise then applies.
//! - This module: shared environment helpers and app startup.
mod machine;
mod macos;
mod mise_commands;
mod newer_tools;
mod plugins;
mod projects;
mod setups;
mod skills;
mod system;
mod terminal;
mod updater;

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
    /// The home directory, for showing paths as `~/…`.
    home: Option<PathBuf>,
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
        if !cfg!(windows) {
            directories.push(home.join(".local/share/mise/bin"));
        }
        directories.push(home.join(".cargo/bin"));
    }
    if cfg!(windows) {
        // Where winget, the mise installer, and Scoop put commands.
        if let Some(local) = env::var_os("LOCALAPPDATA").map(PathBuf::from) {
            directories.push(local.join("Microsoft/WinGet/Links"));
            directories.push(local.join("mise/bin"));
        }
        if let Some(home) = home_dir() {
            directories.push(home.join("scoop/shims"));
        }
    } else if cfg!(target_os = "macos") {
        directories.push(PathBuf::from("/opt/homebrew/bin"));
        directories.push(PathBuf::from("/usr/local/bin"));
    } else if cfg!(target_os = "linux") {
        directories.push(PathBuf::from("/home/linuxbrew/.linuxbrew/bin"));
        directories.push(PathBuf::from("/usr/local/bin"));
    }
    directories
}

pub(crate) fn home_dir() -> Option<PathBuf> {
    powermise_setups::home_dir()
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

/// Finds a program on PATH, or in the usual install locations even if they
/// didn't exist when the app started (mise installed from the app lands in
/// one of those).
pub(crate) fn find_program(name: &str) -> Option<PathBuf> {
    let names: Vec<String> = if cfg!(windows) {
        vec![format!("{name}.exe"), format!("{name}.cmd")]
    } else {
        vec![name.to_owned()]
    };
    let path = env::var_os("PATH").unwrap_or_default();
    env::split_paths(&path)
        .chain(extra_path_dirs())
        .flat_map(|directory| names.iter().map(move |name| directory.join(name)))
        .find(|candidate| candidate.is_file())
}

/// Finds the `mise` executable.
pub(crate) fn find_mise() -> Option<PathBuf> {
    find_program("mise")
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
        home: home_dir(),
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

/// Hosts the app links to: mise's docs and GitHub (tokens, releases).
const LINK_HOSTS: &[&str] = &[
    "mise.jdx.dev",
    "github.com",
    "macos-defaults.com",
    "skills.sh",
    "docs.devin.ai",
];

/// Opens a link in the default browser. Only https links to [`LINK_HOSTS`],
/// so nothing a page shows can make the app open arbitrary programs or files.
#[tauri::command]
fn open_url(url: String) -> CommandResult<()> {
    let host = url
        .strip_prefix("https://")
        .and_then(|rest| rest.split(['/', '#', '?']).next())
        .unwrap_or_default();
    let allowed = LINK_HOSTS.contains(&host)
        && !url.chars().any(|character| {
            character.is_whitespace() || character.is_control() || character == '"'
        });
    if !allowed {
        return Err(format!("powermise doesn't open links like `{url}`"));
    }
    let mut command = if cfg!(target_os = "macos") {
        std::process::Command::new("open")
    } else if cfg!(windows) {
        // `explorer` opens a URL in the default browser without a shell.
        std::process::Command::new("explorer")
    } else {
        std::process::Command::new("xdg-open")
    };
    command.arg(&url).spawn().map(|_| ()).map_err(message)
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
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(mise_commands::MiseState::load())
        .invoke_handler(tauri::generate_handler![
            get_environment,
            reveal_path,
            open_url,
            mise_commands::mise_info,
            plugins::agent_plugins,
            skills::skills_report,
            plugins::install_plugin_in_terminal,
            mise_commands::installed_tools,
            mise_commands::outdated_tools,
            mise_commands::tool_info,
            mise_commands::backends,
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
            projects::project_updates,
            projects::generate_preview,
            projects::open_terminal,
            setups::get_catalog,
            setups::list_selections,
            setups::save_selection,
            setups::export_setup,
            system::system_checks,
            updater::app_version,
            updater::check_app_update,
            updater::install_app_update,
            newer_tools::newer_tools,
            newer_tools::newer_tools_preview,
            newer_tools::write_newer_tools,
            system::system_fix_in_terminal,
            setups::read_setup_file,
            setups::preview_selection,
            setups::write_setup,
            setups::setup_status,
            setups::bootstrap_plan,
            setups::remove_setup,
            setups::bootstrap_in_terminal,
            machine::settings,
            machine::github_status,
            machine::prunable_versions,
            machine::detect_imports,
            machine::shell_conflicts,
            machine::global_packages,
            machine::installed_programs,
            machine::import_scan,
            machine::read_brewfile,
            machine::read_package_list,
            machine::import_file_preview,
            machine::write_import,
            machine::mise_installers,
            machine::disk_usage,
            machine::trusted_folders,
            machine::pick_folder,
            machine::machine_status,
            machine::resolve_dotfile_conflicts,
            machine::tool_aliases,
            machine::plugins,
            machine::package_states,
            machine::use_package_in_terminal,
            macos::macos_settings,
            macos::set_macos_default,
        ])
        .menu(app_menu)
        .on_menu_event(|app, event| {
            if event.id() == updater::CHECK_MENU_ID {
                updater::check_from_menu(app.clone());
            }
        })
        .setup(|app| {
            if let Some(window) = tauri::Manager::get_webview_window(app, "main") {
                fit_to_screen(&window);
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running the powermise desktop app");
}

/// The window opens at its default size (1200×800) unless the screen's usable
/// area is smaller; then it takes 92% of that area, centred, so nothing opens
/// off-screen or with scrollbars it doesn't need.
fn fit_to_screen(window: &tauri::WebviewWindow) {
    let Ok(Some(monitor)) = window.current_monitor() else {
        return;
    };
    let scale = monitor.scale_factor();
    let area = monitor.work_area().size.to_logical::<f64>(scale);
    let Ok(size) = window.outer_size() else {
        return;
    };
    let size = size.to_logical::<f64>(scale);
    let (max_width, max_height) = (area.width * 0.92, area.height * 0.92);
    if size.width <= max_width && size.height <= max_height {
        return;
    }
    let _ = window.set_size(tauri::LogicalSize::new(
        size.width.min(max_width),
        size.height.min(max_height),
    ));
    let _ = window.center();
}

/// The standard menu (macOS: the app menu with About powermise, Edit,
/// Window, …), with Check for Updates… under About powermise. Windows and
/// Linux have no menu bar here; Settings has the same check there.
fn app_menu(app: &tauri::AppHandle) -> tauri::Result<tauri::menu::Menu<tauri::Wry>> {
    let menu = tauri::menu::Menu::default(app)?;
    if cfg!(target_os = "macos")
        && let Some(tauri::menu::MenuItemKind::Submenu(app_submenu)) =
            menu.items()?.into_iter().next()
    {
        let check = tauri::menu::MenuItem::with_id(
            app,
            updater::CHECK_MENU_ID,
            "Check for Updates…",
            true,
            None::<&str>,
        )?;
        app_submenu.insert(&check, 1)?;
    }
    Ok(menu)
}
