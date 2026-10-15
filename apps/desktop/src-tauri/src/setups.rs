//! AI Setups: the curated harness catalog, compiled into one mise config
//! fragment per setup (`~/.config/mise/conf.d/powermise-<name>.toml`).
//!
//! powermise writes that file — the only file it writes in mise's config
//! directory — and mise does the rest: `mise bootstrap` installs it, `mise
//! bootstrap status` reports on it, and removing the setup deletes the file.
//! The same commands work from a terminal; there is no powermise CLI.
use std::{env, fs, path::PathBuf};

use powermise_compiler::Install;
use powermise_domain::{
    AppCatalog, Catalog, IntegrationCatalog, McpServerCatalog, Platform, Preferences,
    SELECTION_SCHEMA_VERSION, Selection, ToolCatalog, ToolProviderKind,
};
use powermise_mise::{BootstrapPlan, MiseCommand};
use powermise_registry::{
    load_app_catalog, load_catalog, load_integration_catalog, load_mcp_catalog, load_tool_catalog,
};
use powermise_setups::{
    CatalogPaths, RemovedServer, SelectionStore, compile_setup, expand_home, inspect_fragment,
    remove_fragment, remove_mcp_files, write_mcp_files,
};
use powermise_state::{GeneratedFileChange, fragment_path};
use serde::{Deserialize, Serialize};

use crate::{CommandResult, layout, message, mise_commands::client, mise_config_dir};

fn catalog_paths() -> CommandResult<CatalogPaths> {
    CatalogPaths::resolve(None).map_err(message)
}

/// The command that applies setups, shown to people and run by the app.
/// When the setup adds MCP servers, `mise dotfiles apply` for those agents'
/// config files follows.
fn apply_command(mcp: &[McpView]) -> String {
    let mut command = MiseCommand::bootstrap_packages_and_tools().display();
    if !mcp.is_empty() {
        let targets: Vec<PathBuf> = mcp.iter().map(|file| PathBuf::from(&file.target)).collect();
        command.push_str(" && ");
        command.push_str(&MiseCommand::dotfiles_apply(&targets).display());
    }
    command
}

/// One agent config a setup adds MCP servers to.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct McpView {
    client: String,
    title: String,
    /// The agent's config file, from home (`~/.claude.json`).
    target: String,
    servers: Vec<String>,
    /// From `mise dotfiles status`: `applied`, `differs`, … (status only).
    #[serde(skip_serializing_if = "Option::is_none")]
    state: Option<String>,
}

fn mcp_views(files: &[powermise_compiler::McpFile]) -> Vec<McpView> {
    files
        .iter()
        .map(|file| McpView {
            client: file.client.id().into(),
            title: file.client.title().into(),
            target: file.target.clone(),
            servers: file.servers.clone(),
            state: None,
        })
        .collect()
}

/// The agent config files (absolute) that applying setup `name` merges MCP
/// servers into on this computer.
pub(crate) fn mcp_targets(name: &str) -> CommandResult<Vec<PathBuf>> {
    let layout = layout()?;
    let target = powermise_setups::host_platform();
    let setup = compile_setup(
        &catalog_paths()?,
        &layout.selection_path(name),
        target,
        crate::find_mise().as_deref(),
    )
    .map_err(message)?;
    let home = crate::home_dir().ok_or("could not determine your home directory")?;
    Ok(setup
        .compiled
        .mcp_files
        .iter()
        .map(|file| expand_home(&file.target, &home))
        .collect())
}

// ---------------------------------------------------------------------------
// Catalog
// ---------------------------------------------------------------------------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogBundle {
    revision: String,
    harnesses: Catalog,
    tools: ToolCatalog,
    apps: AppCatalog,
    integrations: IntegrationCatalog,
    mcp: McpServerCatalog,
}

#[tauri::command]
pub fn get_catalog() -> CommandResult<CatalogBundle> {
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
// Saved setups
// ---------------------------------------------------------------------------

/// What the setup editor sends: the parts of a selection the app edits.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectionDraft {
    name: String,
    harnesses: Vec<String>,
    /// The MCP servers picked; `None` keeps the harnesses' recommendations.
    #[serde(default)]
    mcp_servers: Option<Vec<String>>,
    include_desktop_apps: bool,
}

impl SelectionDraft {
    fn into_selection(self, catalog: &str) -> Selection {
        Selection {
            schema_version: SELECTION_SCHEMA_VERSION,
            name: self.name,
            catalog: catalog.into(),
            harnesses: self.harnesses,
            mcp_servers: self.mcp_servers,
            preferences: Preferences {
                include_desktop_apps: self.include_desktop_apps,
            },
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedSelection {
    name: String,
    path: PathBuf,
    harnesses: Vec<String>,
    /// The MCP servers picked, or `None` for the harnesses' recommendations.
    mcp_servers: Option<Vec<String>>,
    include_desktop_apps: bool,
    /// The mise fragment this setup writes.
    fragment_path: PathBuf,
    /// Whether that fragment exists, i.e. mise currently includes the setup.
    applied: bool,
}

#[tauri::command]
pub fn list_selections() -> CommandResult<Vec<SavedSelection>> {
    let layout = layout()?;
    let mise = mise_config_dir()?;
    let store = SelectionStore::new(&layout);
    let mut saved = Vec::new();
    for name in store.list().map_err(message)? {
        // Skip files the store can't load rather than failing the sidebar.
        let Ok(selection) = store.read(&name) else {
            continue;
        };
        let fragment = fragment_path(&mise, &name);
        saved.push(SavedSelection {
            path: layout.selection_path(&name),
            applied: fragment.is_file(),
            fragment_path: fragment,
            harnesses: selection.harnesses,
            mcp_servers: selection.mcp_servers,
            include_desktop_apps: selection.preferences.include_desktop_apps,
            name,
        });
    }
    Ok(saved)
}

#[tauri::command]
pub fn save_selection(draft: SelectionDraft, replace: bool) -> CommandResult<PathBuf> {
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
// Preview and apply
// ---------------------------------------------------------------------------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedView {
    harness_ids: Vec<String>,
    tool_ids: Vec<String>,
    desktop_app_ids: Vec<String>,
    unavailable_desktop_ids: Vec<String>,
    integration_ids: Vec<String>,
    mcp_server_ids: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallView {
    tool_id: String,
    kind: ToolProviderKind,
    /// The `[tools]` or `[bootstrap.packages]` key it compiles to.
    key: String,
    version: String,
}

impl From<Install> for InstallView {
    fn from(install: Install) -> Self {
        Self {
            tool_id: install.id,
            kind: install.kind,
            key: install.key,
            version: install.version,
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewView {
    resolved: ResolvedView,
    installs: Vec<InstallView>,
    fragment_path: PathBuf,
    fragment_contents: String,
    change: GeneratedFileChange,
    apply_command: String,
    /// The agents' config files the setup adds MCP servers to.
    mcp: Vec<McpView>,
    /// Tools left out on this machine, with why (Homebrew formulas on an
    /// Intel Mac).
    skipped: Vec<powermise_compiler::Skipped>,
    /// On Linux and WSL, Homebrew's folder needs a one-time `sudo` to create;
    /// set when this setup needs it and it isn't ready yet.
    homebrew_setup: Option<PathBuf>,
}

/// Where mise installs Homebrew formulas on Linux.
const LINUX_HOMEBREW_PREFIX: &str = "/home/linuxbrew/.linuxbrew";

/// The Homebrew folder to set up first, when installing on this Linux or
/// WSL machine needs Homebrew and the folder isn't there and writable yet.
fn homebrew_setup_needed(
    target: Platform,
    installs: &[powermise_compiler::Install],
) -> Option<PathBuf> {
    let linux = matches!(target, Platform::Linux | Platform::Wsl)
        && powermise_setups::host_platform() == target;
    let uses_homebrew = installs
        .iter()
        .any(|install| install.kind == ToolProviderKind::BootstrapBrew);
    let prefix = PathBuf::from(LINUX_HOMEBREW_PREFIX);
    let writable = || {
        let probe = prefix.join(".powermise-write-test");
        let ok = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&probe)
            .is_ok();
        let _ = std::fs::remove_file(&probe);
        ok
    };
    (linux && uses_homebrew && !(prefix.is_dir() && writable())).then_some(prefix)
}

/// Opens the terminal app running `mise bootstrap --yes --only
/// packages,tools`, so a one-time `sudo` (to create Homebrew's folder on
/// Linux) can ask for a password.
#[tauri::command]
pub fn bootstrap_in_terminal() -> CommandResult<()> {
    let home = crate::home_dir().ok_or("could not determine your home directory")?;
    let mise = crate::find_mise().ok_or("mise is not installed")?;
    crate::terminal::open(
        &home,
        &mise,
        &[
            "bootstrap".into(),
            "--yes".into(),
            "--only".into(),
            "packages,tools".into(),
        ],
    )
    .map_err(message)
}

/// The draft name used while a new setup is still unnamed.
const DRAFT_NAME: &str = "untitled";

/// Compiles a draft without saving it or writing anything mise reads.
#[tauri::command]
pub fn preview_selection(draft: SelectionDraft, target: Platform) -> CommandResult<PreviewView> {
    let paths = catalog_paths()?;
    let catalog = load_catalog(&paths.harnesses).map_err(message)?;
    let mut selection = draft.into_selection(&catalog.catalog);
    if powermise_state::validate_selection_name(&selection.name).is_err() {
        selection.name = DRAFT_NAME.into();
    }
    let scratch = env::temp_dir().join(format!("powermise-desktop-{}", std::process::id()));
    fs::create_dir_all(&scratch).map_err(message)?;
    let draft_path = scratch.join(format!("{}.toml", selection.name));
    fs::write(&draft_path, toml::to_string(&selection).map_err(message)?).map_err(message)?;

    let setup = compile_setup(&paths, &draft_path, target, crate::find_mise().as_deref())
        .map_err(message)?;
    let mise = mise_config_dir()?;
    let change = inspect_fragment(&setup.fragment, &mise).map_err(message)?;
    let resolved = setup.resolved;
    let mcp = mcp_views(&setup.compiled.mcp_files);
    Ok(PreviewView {
        resolved: ResolvedView {
            harness_ids: resolved.harness_ids,
            tool_ids: resolved.tool_ids,
            desktop_app_ids: resolved.desktop_app_ids,
            unavailable_desktop_ids: resolved.unavailable_desktop_ids,
            integration_ids: resolved.integration_ids,
            mcp_server_ids: resolved.mcp_server_ids,
        },
        homebrew_setup: homebrew_setup_needed(target, &setup.compiled.installs),
        skipped: setup.compiled.skipped,
        installs: setup
            .compiled
            .installs
            .into_iter()
            .map(InstallView::from)
            .collect(),
        fragment_path: setup.fragment.path(&mise),
        fragment_contents: setup.fragment.contents().to_owned(),
        change,
        apply_command: apply_command(&mcp),
        mcp,
    })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WrittenSetup {
    path: PathBuf,
    change: GeneratedFileChange,
    /// MCP servers the setup no longer has, taken out of agents' configs.
    removed_servers: Vec<RemovedServer>,
}

/// Writes a saved setup's fragment into mise's `conf.d`. Installing it is a
/// separate, streamed `mise bootstrap` run.
#[tauri::command]
pub fn write_setup(name: String, target: Platform) -> CommandResult<WrittenSetup> {
    let layout = layout()?;
    SelectionStore::new(&layout).read(&name).map_err(message)?;
    let setup = compile_setup(
        &catalog_paths()?,
        &layout.selection_path(&name),
        target,
        crate::find_mise().as_deref(),
    )
    .map_err(message)?;
    let mise = mise_config_dir()?;
    let change = inspect_fragment(&setup.fragment, &mise).map_err(message)?;
    setup.fragment.write(&mise).map_err(message)?;
    // The MCP sources only matter on the computer the setup is applied to.
    let removed_servers = if target == powermise_setups::host_platform() {
        let home = crate::home_dir().ok_or("could not determine your home directory")?;
        write_mcp_files(&mise, &name, &setup.compiled.mcp_files, &home, target).map_err(message)?
    } else {
        Vec::new()
    };
    Ok(WrittenSetup {
        path: setup.fragment.path(&mise),
        change,
        removed_servers,
    })
}

/// `mise bootstrap plan --json`: everything applying would change, including
/// parts of the person's config that aren't from powermise.
#[tauri::command]
pub async fn bootstrap_plan() -> CommandResult<BootstrapPlan> {
    tauri::async_runtime::spawn_blocking(|| client()?.bootstrap_plan().map_err(message))
        .await
        .map_err(message)?
}

// ---------------------------------------------------------------------------
// Status
// ---------------------------------------------------------------------------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemStatus {
    tool_id: String,
    key: String,
    kind: ToolProviderKind,
    /// From `mise bootstrap status`: `installed`, `missing`, `outdated`, …
    state: String,
    installed: bool,
    /// When a tool's active version comes from another config (yours wins).
    overridden_by: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SetupStatus {
    name: String,
    fragment_path: PathBuf,
    /// The fragment on disk differs from what the setup compiles to now (an
    /// edit, or a newer catalog): applying rewrites it.
    change: GeneratedFileChange,
    items: Vec<ItemStatus>,
    apply_command: String,
    /// Each agent config the setup adds MCP servers to, with mise's state.
    mcp: Vec<McpView>,
}

/// What mise reports for each thing a setup installs.
#[tauri::command]
pub async fn setup_status(name: String, target: Platform) -> CommandResult<SetupStatus> {
    tauri::async_runtime::spawn_blocking(move || {
        let layout = layout()?;
        let setup = compile_setup(
            &catalog_paths()?,
            &layout.selection_path(&name),
            target,
            crate::find_mise().as_deref(),
        )
        .map_err(message)?;
        let mise = mise_config_dir()?;
        let change = inspect_fragment(&setup.fragment, &mise).map_err(message)?;
        let client = client()?;
        let status = client.bootstrap_status().map_err(message)?;
        let tools = client.tools(None).unwrap_or_default();
        let fragment = setup.fragment.path(&mise);
        let items = setup
            .compiled
            .installs
            .into_iter()
            .map(|install| {
                let (state, installed, overridden_by) = match install.key.split_once(':') {
                    _ if install.kind == ToolProviderKind::System => {
                        ("included with the system".to_owned(), true, None)
                    }
                    Some((manager, package)) if install.kind != ToolProviderKind::MiseTool => {
                        let found = status.packages.get(manager).and_then(|manager| {
                            manager
                                .packages
                                .iter()
                                .find(|entry| entry.package == package)
                        });
                        match found {
                            Some(entry) => (entry.state.clone(), entry.state == "installed", None),
                            None => ("not applied".to_owned(), false, None),
                        }
                    }
                    _ => {
                        let short = install.key.rsplit(':').next().unwrap_or(&install.key);
                        let versions = tools.get(&install.key).or_else(|| tools.get(short));
                        let active = versions
                            .and_then(|versions| versions.iter().find(|version| version.active));
                        let overridden_by = active
                            .and_then(|version| version.source.as_ref())
                            .filter(|source| source.path != fragment)
                            .map(|source| source.path.display().to_string());
                        let installed = versions.is_some_and(|versions| {
                            versions
                                .iter()
                                .any(|version| version.installed && version.source.is_some())
                        });
                        (
                            if installed { "installed" } else { "missing" }.to_owned(),
                            installed,
                            overridden_by,
                        )
                    }
                };
                ItemStatus {
                    tool_id: install.id,
                    key: install.key,
                    kind: install.kind,
                    state,
                    installed,
                    overridden_by,
                }
            })
            .collect();
        let edit = format!("merge:{}", powermise_compiler::edit_id(&name));
        let edits = client.dotfile_edits().unwrap_or_default();
        let mut mcp = mcp_views(&setup.compiled.mcp_files);
        for view in &mut mcp {
            view.state = Some(
                edits
                    .iter()
                    .find(|(path, id, _)| path == &view.target && id == &edit)
                    .map_or_else(|| "not applied".to_owned(), |(_, _, state)| state.clone()),
            );
        }
        Ok(SetupStatus {
            name,
            fragment_path: fragment,
            change,
            items,
            apply_command: apply_command(&mcp),
            mcp,
        })
    })
    .await
    .map_err(message)?
}

// ---------------------------------------------------------------------------
// Remove
// ---------------------------------------------------------------------------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemovedSetup {
    removed: Vec<PathBuf>,
    /// MCP servers the setup had added, taken out of agents' configs.
    removed_servers: Vec<RemovedServer>,
    /// Suggested follow-up: delete tool versions nothing uses anymore.
    cleanup_command: String,
}

/// Removes a setup: deletes its mise fragment and its saved setup file.
/// Installed packages and tools stay; the cleanup command removes unused
/// tool versions.
#[tauri::command]
pub fn remove_setup(name: String) -> CommandResult<RemovedSetup> {
    let layout = layout()?;
    let mise = mise_config_dir()?;
    let mut removed = Vec::new();
    let home = crate::home_dir().ok_or("could not determine your home directory")?;
    let removed_servers = remove_mcp_files(&mise, &name, &home, powermise_setups::host_platform())
        .map_err(message)?;
    if remove_fragment(&mise, &name).map_err(message)? {
        removed.push(fragment_path(&mise, &name));
    }
    removed.push(
        SelectionStore::new(&layout)
            .remove(&name)
            .map_err(message)?,
    );
    Ok(RemovedSetup {
        removed,
        removed_servers,
        cleanup_command: MiseCommand::prune_tools().display(),
    })
}
