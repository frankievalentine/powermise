//! Powermise desktop backend: a GUI companion for mise.
//!
//! - `mise_commands`: what mise manages, read through mise's JSON output and
//!   changed only by running mise commands.
//! - This module: shared environment helpers and the AI Setups feature, which
//!   calls the same lifecycle code as the `powermise` CLI so anything done here
//!   can be repeated, inspected, or undone from a terminal.
mod mise_commands;

use std::{
    env, fs,
    path::{Path, PathBuf},
};

use powermise_compiler::chosen_provider;
use powermise_domain::{
    AppCatalog, Catalog, IntegrationCatalog, McpServerCatalog, Operation, OperationResult,
    Platform, Preferences, RunReport, Selection, ToolCatalog, ToolProviderKind,
};
use powermise_lifecycle::{
    ApplyOptions, BootstrapOutcome, CatalogPaths, PowermiseLayout, SelectionStore, compile_context,
    host_platform, inspect_config, resolve_mise_config_dir,
};
use powermise_mise::MiseExecutable;
use powermise_registry::{
    load_app_catalog, load_catalog, load_integration_catalog, load_mcp_catalog, load_tool_catalog,
};
use powermise_state::GeneratedFileChange;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

/// Errors cross the IPC boundary as their display message.
pub(crate) type CommandResult<T> = Result<T, String>;

pub(crate) fn message(error: impl std::fmt::Display) -> String {
    error.to_string()
}

// ---------------------------------------------------------------------------
// Environment
// ---------------------------------------------------------------------------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct EnvironmentInfo {
    host_target: Platform,
    powermise_dir: PathBuf,
    mise_config_dir: PathBuf,
    mise_path: Option<PathBuf>,
    mise_version: Option<String>,
}

/// Directories GUI-launched processes commonly miss: apps started from Finder
/// or the Dock do not inherit the login shell's PATH, so mise (and the
/// Homebrew it drives) would otherwise be invisible.
fn extra_path_dirs() -> Vec<PathBuf> {
    let home = home_dir();
    let mut directories = Vec::new();
    if let Some(home) = &home {
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

/// The PATH this process should run with: the inherited PATH plus the
/// common tool directories it is missing.
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

fn mise_runner() -> CommandResult<MiseExecutable> {
    find_mise().map(MiseExecutable::new).ok_or_else(|| {
        "mise is not installed. Install it from https://mise.jdx.dev, then try again.".into()
    })
}

fn layout() -> CommandResult<PowermiseLayout> {
    PowermiseLayout::resolve(None).map_err(message)
}

fn catalog_paths() -> CommandResult<CatalogPaths> {
    CatalogPaths::resolve(None).map_err(message)
}

fn mise_config_dir() -> CommandResult<PathBuf> {
    resolve_mise_config_dir(None).map_err(message)
}

#[tauri::command]
fn get_environment() -> CommandResult<EnvironmentInfo> {
    let mise_path = find_mise();
    let mise_version = mise_path
        .as_ref()
        .and_then(|path| MiseExecutable::new(path).version().ok());
    Ok(EnvironmentInfo {
        host_target: host_platform(),
        powermise_dir: layout()?.root,
        mise_config_dir: mise_config_dir()?,
        mise_path,
        mise_version,
    })
}

// ---------------------------------------------------------------------------
// Catalog
// ---------------------------------------------------------------------------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CatalogBundle {
    revision: String,
    harnesses: Catalog,
    tools: ToolCatalog,
    apps: AppCatalog,
    integrations: IntegrationCatalog,
    mcp: McpServerCatalog,
}

#[tauri::command]
fn get_catalog() -> CommandResult<CatalogBundle> {
    let paths = catalog_paths()?;
    let harnesses = load_catalog(&paths.harnesses).map_err(message)?;
    Ok(CatalogBundle {
        revision: harnesses.catalog_revision.clone(),
        harnesses,
        tools: load_tool_catalog(&paths.tools).map_err(message)?,
        apps: load_app_catalog(&paths.apps).map_err(message)?,
        integrations: load_integration_catalog(&paths.integrations).map_err(message)?,
        mcp: load_mcp_catalog(&paths.mcp).map_err(message)?,
    })
}

// ---------------------------------------------------------------------------
// Selections ("setups")
// ---------------------------------------------------------------------------

/// What the setup editor sends: the parts of a selection the app edits.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SelectionDraft {
    name: String,
    harnesses: Vec<String>,
    include_desktop_apps: bool,
}

impl SelectionDraft {
    fn into_selection(self, catalog: &str) -> Selection {
        Selection {
            schema_version: 1,
            name: self.name,
            catalog: catalog.into(),
            harnesses: self.harnesses,
            machine_roles: Default::default(),
            preferences: Preferences {
                include_desktop_apps: self.include_desktop_apps,
                ..Default::default()
            },
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SavedSelection {
    name: String,
    path: PathBuf,
    harnesses: Vec<String>,
    include_desktop_apps: bool,
    /// Whether a generated configuration has been written for it.
    applied: bool,
    last_run: Option<RunReport>,
}

fn read_last_run(layout: &PowermiseLayout, name: &str) -> Option<RunReport> {
    let contents = fs::read(layout.run_report_path(name)).ok()?;
    serde_json::from_slice(&contents).ok()
}

#[tauri::command]
fn list_selections() -> CommandResult<Vec<SavedSelection>> {
    let layout = layout()?;
    let store = SelectionStore::new(&layout);
    let mut saved = Vec::new();
    for name in store.list().map_err(message)? {
        // Skip files the store cannot load rather than failing the sidebar.
        let Ok(selection) = store.read(&name) else {
            continue;
        };
        saved.push(SavedSelection {
            path: layout.selection_path(&name),
            applied: layout.manifest_path(&name).exists(),
            last_run: read_last_run(&layout, &name),
            harnesses: selection.harnesses,
            include_desktop_apps: selection.preferences.include_desktop_apps,
            name,
        });
    }
    Ok(saved)
}

#[tauri::command]
fn save_selection(draft: SelectionDraft, replace: bool) -> CommandResult<PathBuf> {
    let layout = layout()?;
    let store = SelectionStore::new(&layout);
    let catalog = load_catalog(&catalog_paths()?.harnesses).map_err(message)?;
    let selection = draft.into_selection(&catalog.catalog);
    if replace && layout.selection_path(&selection.name).exists() {
        store.remove(&selection.name).map_err(message)?;
    }
    store.create(&selection).map_err(message)
}

// ---------------------------------------------------------------------------
// Preview
// ---------------------------------------------------------------------------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ResolvedView {
    harness_ids: Vec<String>,
    tool_ids: Vec<String>,
    desktop_app_ids: Vec<String>,
    unavailable_desktop_ids: Vec<String>,
    app_ids: Vec<String>,
    integration_ids: Vec<String>,
    mcp_server_ids: Vec<String>,
}

/// How one resolved tool is installed, as the compiler chose it.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct InstallView {
    tool_id: String,
    kind: ToolProviderKind,
    package: String,
    version: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PreviewView {
    resolved: ResolvedView,
    installs: Vec<InstallView>,
    operations: Vec<Operation>,
    config_path: PathBuf,
    config_contents: String,
    change: GeneratedFileChange,
    apply_command: String,
}

/// The draft name used while a new setup is still unnamed.
const DRAFT_NAME: &str = "untitled";

#[tauri::command]
fn preview_selection(draft: SelectionDraft, target: Platform) -> CommandResult<PreviewView> {
    let paths = catalog_paths()?;
    let catalog = load_catalog(&paths.harnesses).map_err(message)?;
    let mut selection = draft.into_selection(&catalog.catalog);
    if powermise_state::validate_selection_name(&selection.name).is_err() {
        selection.name = DRAFT_NAME.into();
    }
    let name = selection.name.clone();

    // Compile from a scratch copy so previewing never touches managed state.
    let scratch = env::temp_dir().join(format!("powermise-desktop-{}", std::process::id()));
    fs::create_dir_all(&scratch).map_err(message)?;
    let draft_path = scratch.join(format!("{name}.toml"));
    fs::write(&draft_path, toml::to_string(&selection).map_err(message)?).map_err(message)?;

    let context = compile_context(&paths, &draft_path, target, None).map_err(message)?;
    let mise_config_dir = mise_config_dir()?;
    let change = inspect_config(&context, &name, &mise_config_dir).map_err(message)?;
    let tools = load_tool_catalog(&paths.tools).map_err(message)?;
    let installs = context
        .resolved
        .tool_ids
        .iter()
        .filter_map(|tool_id| {
            let tool = tools.tools.iter().find(|tool| &tool.id == tool_id)?;
            let provider = chosen_provider(&tool.providers, target)?;
            Some(InstallView {
                tool_id: tool_id.clone(),
                kind: provider.kind.clone(),
                package: provider.package.clone(),
                version: provider.version.clone(),
            })
        })
        .collect();
    let resolved = context.resolved;
    Ok(PreviewView {
        installs,
        resolved: ResolvedView {
            harness_ids: resolved.harness_ids,
            tool_ids: resolved.tool_ids,
            desktop_app_ids: resolved.desktop_app_ids,
            unavailable_desktop_ids: resolved.unavailable_desktop_ids,
            app_ids: resolved.app_ids,
            integration_ids: resolved.integration_ids,
            mcp_server_ids: resolved.mcp_server_ids,
        },
        operations: context.plan.operations.operations().to_vec(),
        config_path: context.artifact.config_path(&mise_config_dir),
        config_contents: context.artifact.contents().to_owned(),
        change,
        apply_command: apply_command(&name, target),
    })
}

fn target_label(target: Platform) -> &'static str {
    match target {
        Platform::Darwin => "darwin",
        Platform::Linux => "linux",
        Platform::Win32 => "win32",
        Platform::Wsl => "wsl",
    }
}

fn apply_command(name: &str, target: Platform) -> String {
    format!(
        "powermise apply --name {name} --target {} --yes",
        target_label(target)
    )
}

// ---------------------------------------------------------------------------
// Apply, status, uninstall
// ---------------------------------------------------------------------------

#[derive(Serialize)]
#[serde(rename_all = "camelCase", tag = "state")]
enum BootstrapView {
    Converged,
    Failed { message: String },
    Skipped { reason: String },
}

impl From<BootstrapOutcome> for BootstrapView {
    fn from(outcome: BootstrapOutcome) -> Self {
        match outcome {
            BootstrapOutcome::Converged => Self::Converged,
            BootstrapOutcome::Failed { message } => Self::Failed { message },
            BootstrapOutcome::Skipped { reason } => Self::Skipped { reason },
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ApplyView {
    selection: String,
    results: Vec<OperationResult>,
    bootstrap: BootstrapView,
    follow_ups: Vec<String>,
    recovery_guidance: Vec<String>,
    command: String,
}

/// The event carrying human-readable apply progress to the window.
const APPLY_PROGRESS_EVENT: &str = "apply-progress";

#[tauri::command]
async fn apply_selection(
    app: AppHandle,
    name: String,
    target: Platform,
) -> CommandResult<ApplyView> {
    tauri::async_runtime::spawn_blocking(move || {
        let runner = mise_runner()?;
        let layout = layout()?;
        let paths = catalog_paths()?;
        let mise_config_dir = mise_config_dir()?;
        let adapters_config_dir = layout.root.join("adapters");
        let progress = |step: &str| {
            let _ = app.emit(APPLY_PROGRESS_EVENT, step.to_owned());
        };
        let options = ApplyOptions {
            layout: &layout,
            paths: &paths,
            target,
            machine_role: None,
            mise_config_dir: &mise_config_dir,
            adapters_config_dir: &adapters_config_dir,
            run_mise_bootstrap: true,
            progress: Some(&progress),
        };
        let report = powermise_lifecycle::apply(&runner, &options, &name).map_err(message)?;
        Ok(ApplyView {
            command: apply_command(&name, target),
            selection: report.selection,
            results: report.results,
            bootstrap: report.bootstrap.into(),
            follow_ups: report.mise_follow_ups,
            recovery_guidance: report.recovery_guidance,
        })
    })
    .await
    .map_err(message)?
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct NativeView {
    operation_id: String,
    package: String,
    present: bool,
    detail: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StatusView {
    selection: String,
    satisfied: bool,
    change: GeneratedFileChange,
    drift: Option<String>,
    ledger_owned: bool,
    follow_ups: Vec<String>,
    native: Vec<NativeView>,
    command: String,
}

#[tauri::command]
async fn selection_status(name: String, target: Platform) -> CommandResult<StatusView> {
    tauri::async_runtime::spawn_blocking(move || {
        let runner = mise_runner()?;
        let layout = layout()?;
        let paths = catalog_paths()?;
        let mise_config_dir = mise_config_dir()?;
        let report = powermise_lifecycle::status(
            &runner,
            &layout,
            &paths,
            &name,
            target,
            None,
            &mise_config_dir,
        )
        .map_err(message)?;
        Ok(StatusView {
            command: format!(
                "powermise status --name {name} --target {}",
                target_label(target)
            ),
            selection: report.selection,
            satisfied: report.satisfied,
            change: report.change,
            drift: report.drift,
            ledger_owned: report.ledger_owned,
            follow_ups: report.mise_follow_ups,
            native: report
                .native
                .into_iter()
                .map(|entry| NativeView {
                    operation_id: entry.operation_id,
                    package: entry.package,
                    present: entry.present,
                    detail: entry.detail,
                })
                .collect(),
        })
    })
    .await
    .map_err(message)?
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RemoveView {
    removed: Vec<PathBuf>,
    commands: Vec<String>,
}

/// Removes a setup completely: its generated configuration, manifest, stubs,
/// and ledger entries, then the setup file itself. Installed tools stay in
/// mise's data directory, exactly as `powermise uninstall` leaves them.
#[tauri::command]
fn remove_selection(name: String) -> CommandResult<RemoveView> {
    let layout = layout()?;
    let mise_config_dir = mise_config_dir()?;
    let report =
        powermise_lifecycle::uninstall(&layout, &name, &mise_config_dir, false).map_err(message)?;
    let mut removed = report
        .actions
        .into_iter()
        .map(|action| action.path)
        .collect::<Vec<_>>();
    removed.push(
        SelectionStore::new(&layout)
            .remove(&name)
            .map_err(message)?,
    );
    Ok(RemoveView {
        removed,
        commands: vec![
            format!("powermise uninstall --name {name} --yes"),
            format!("rm {}", layout.selection_path(&name).display()),
        ],
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
        .manage(mise_commands::MiseState::default())
        .invoke_handler(tauri::generate_handler![
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
            get_environment,
            get_catalog,
            list_selections,
            save_selection,
            preview_selection,
            apply_selection,
            selection_status,
            remove_selection,
            reveal_path,
        ])
        .run(tauri::generate_context!())
        .expect("error while running the Powermise desktop app");
}
