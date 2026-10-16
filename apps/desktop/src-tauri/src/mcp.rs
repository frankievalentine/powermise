//! AI Setups › MCP Servers: which servers are on for each agent
//! (powermise_setups::mcp_panel), what each agent's own config has right
//! now, and servers people add themselves.
use std::{collections::BTreeSet, path::PathBuf};

use powermise_compiler::Skipped;
use powermise_domain::{McpClient, McpServer, McpServerCatalog};
use powermise_mise::MiseCommand;
use powermise_registry::load_mcp_catalog;
use powermise_setups::{
    CatalogPaths, CompiledPanel, McpPanel, PANEL_NAME, RemovedServer, compile_panel,
    configured_servers, expand_home, host_platform, panel_fragment_path, powermise_sources,
    read_panel, validate_custom, write_panel,
};
use serde::Serialize;

use crate::{CommandResult, layout, message, mise_commands::client, mise_config_dir};

fn mcp_catalog() -> CommandResult<McpServerCatalog> {
    let paths = CatalogPaths::resolve(None).map_err(message)?;
    load_mcp_catalog(&paths.mcp).map_err(message)
}

fn home() -> CommandResult<PathBuf> {
    crate::home_dir().ok_or_else(|| "could not determine your home directory".into())
}

fn compile(panel: &McpPanel, catalog: &McpServerCatalog) -> CommandResult<CompiledPanel> {
    let mise =
        crate::find_mise().map_or_else(|| "mise".to_owned(), |path| path.display().to_string());
    compile_panel(panel, catalog, host_platform(), &mise).map_err(message)
}

/// A server in an agent's config, and which powermise sources add it
/// (`mcp` for this page, otherwise a setup's name).
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfiguredServer {
    name: String,
    from: Vec<String>,
}

/// One agent's MCP config on this computer.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentView {
    id: McpClient,
    title: &'static str,
    /// The config file, from home.
    path: Option<String>,
    /// Why powermise can't add servers here.
    unavailable: Option<String>,
    /// Its config file (or folder) exists.
    detected: bool,
    takes_remote: bool,
    /// The servers in its config now; `None` when the file can't be read.
    configured: Option<Vec<ConfiguredServer>>,
    /// mise's state for this page's merge entry (`applied`, `differs`, …),
    /// when the page adds servers here.
    state: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct McpPanelView {
    panel: McpPanel,
    agents: Vec<AgentView>,
    fragment_path: PathBuf,
}

#[tauri::command]
pub async fn mcp_panel() -> CommandResult<McpPanelView> {
    tauri::async_runtime::spawn_blocking(read_view)
        .await
        .map_err(message)?
}

fn read_view() -> CommandResult<McpPanelView> {
    let panel = read_panel(&layout()?).map_err(message)?;
    let mise = mise_config_dir()?;
    let home = home()?;
    let target = host_platform();
    let edit = format!("merge:{}", powermise_compiler::edit_id(PANEL_NAME));
    let edits = client()
        .ok()
        .and_then(|client| client.dotfile_edits().ok())
        .unwrap_or_default();
    let agents = McpClient::ALL
        .into_iter()
        .map(|id| {
            let (path, unavailable) = match id.config_path(target) {
                Ok(path) => (Some(path), None),
                Err(reason) => (None, Some(reason.to_owned())),
            };
            let file = path.as_deref().map(|path| expand_home(path, &home));
            let detected = file.as_deref().is_some_and(|file| {
                file.exists()
                    || file
                        .parent()
                        .is_some_and(|parent| parent != home && parent.is_dir())
            });
            let sources = powermise_sources(&mise, id);
            let configured = file
                .as_deref()
                .filter(|_| unavailable.is_none())
                .and_then(|file| configured_servers(id, file))
                .map(|names| {
                    names
                        .into_iter()
                        .map(|name| ConfiguredServer {
                            from: sources
                                .iter()
                                .filter(|(_, servers)| servers.contains(&name))
                                .map(|(source, _)| source.clone())
                                .collect(),
                            name,
                        })
                        .collect()
                });
            let state = path.as_ref().and_then(|path| {
                sources
                    .iter()
                    .any(|(source, _)| source == PANEL_NAME)
                    .then(|| {
                        edits
                            .iter()
                            .find(|(target, edit_id, _)| target == path && edit_id == &edit)
                            .map_or_else(|| "not applied".to_owned(), |(_, _, state)| state.clone())
                    })
            });
            AgentView {
                id,
                title: id.title(),
                path,
                unavailable,
                detected,
                takes_remote: id.takes_remote(),
                configured,
                state,
            }
        })
        .collect();
    Ok(McpPanelView {
        panel,
        agents,
        fragment_path: panel_fragment_path(&mise),
    })
}

/// Checks a server someone is adding.
#[tauri::command]
pub fn check_mcp_server(server: McpServer) -> CommandResult<()> {
    validate_custom(&server, &mcp_catalog()?).map_err(message)
}

fn check_panel(panel: &McpPanel, catalog: &McpServerCatalog) -> CommandResult<()> {
    let mut ids = BTreeSet::new();
    for server in &panel.custom {
        validate_custom(server, catalog).map_err(message)?;
        if !ids.insert(server.id.as_str()) {
            return Err(format!("there are two servers named `{}`", server.id));
        }
    }
    Ok(())
}

/// A source file the page writes.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewFile {
    title: &'static str,
    target: String,
    contents: String,
}

/// A server applying takes out of an agent's config.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewRemoval {
    title: &'static str,
    server: String,
    path: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct McpPreview {
    fragment_path: PathBuf,
    /// The new `powermise-mcp.toml`, or `None` when it's deleted.
    fragment: Option<String>,
    files: Vec<PreviewFile>,
    removals: Vec<PreviewRemoval>,
    skipped: Vec<Skipped>,
    /// What mise runs afterwards, if anything.
    command: Option<String>,
}

/// What saving `panel` would write, remove, and run.
#[tauri::command]
pub fn mcp_preview(panel: McpPanel) -> CommandResult<McpPreview> {
    let catalog = mcp_catalog()?;
    check_panel(&panel, &catalog)?;
    let compiled = compile(&panel, &catalog)?;
    let saved = compile(&read_panel(&layout()?).map_err(message)?, &catalog)?;
    let (mise, home) = (mise_config_dir()?, home()?);
    let mut removals = Vec::new();
    for old in &saved.files {
        let new: BTreeSet<&String> = compiled
            .files
            .iter()
            .filter(|file| file.client == old.client)
            .flat_map(|file| &file.servers)
            .collect();
        let elsewhere: BTreeSet<String> = powermise_sources(&mise, old.client)
            .into_iter()
            .filter(|(source, _)| source != PANEL_NAME)
            .flat_map(|(_, servers)| servers)
            .collect();
        let present =
            configured_servers(old.client, &expand_home(&old.target, &home)).unwrap_or_default();
        for server in &old.servers {
            if !new.contains(server) && !elsewhere.contains(server) && present.contains(server) {
                removals.push(PreviewRemoval {
                    title: old.client.title(),
                    server: server.clone(),
                    path: old.target.clone(),
                });
            }
        }
    }
    let command = apply_steps(&compiled, &home)?
        .iter()
        .map(MiseCommand::display)
        .reduce(|all, next| format!("{all} && {next}"));
    Ok(McpPreview {
        fragment_path: panel_fragment_path(&mise),
        fragment: compiled.fragment,
        files: compiled
            .files
            .into_iter()
            .map(|file| PreviewFile {
                title: file.client.title(),
                target: file.target,
                contents: file.contents,
            })
            .collect(),
        removals,
        skipped: compiled.skipped,
        command,
    })
}

/// Saves the page: its mise file and sources, taking turned-off servers back
/// out of agents' configs. Applying the rest is the `applyMcp` action.
#[tauri::command]
pub fn write_mcp(panel: McpPanel) -> CommandResult<Vec<RemovedServer>> {
    let catalog = mcp_catalog()?;
    check_panel(&panel, &catalog)?;
    let compiled = compile(&panel, &catalog)?;
    write_panel(
        &layout()?,
        &mise_config_dir()?,
        &panel,
        &compiled,
        &home()?,
        host_platform(),
    )
    .map_err(message)
}

/// `mise install` for the servers' tools, then `mise dotfiles apply` for
/// the agents' config files.
fn apply_steps(
    compiled: &CompiledPanel,
    home: &std::path::Path,
) -> CommandResult<Vec<MiseCommand>> {
    let mut steps = Vec::new();
    if !compiled.tools.is_empty() {
        steps.push(MiseCommand::install_specs(&compiled.tools, false).map_err(message)?);
    }
    let targets: Vec<PathBuf> = compiled
        .files
        .iter()
        .map(|file| expand_home(&file.target, home))
        .collect();
    if !targets.is_empty() {
        steps.push(MiseCommand::dotfiles_apply(&targets));
    }
    Ok(steps)
}

/// The steps the saved page applies with.
pub(crate) fn saved_apply_steps() -> CommandResult<Vec<MiseCommand>> {
    let catalog = mcp_catalog()?;
    let compiled = compile(&read_panel(&layout()?).map_err(message)?, &catalog)?;
    apply_steps(&compiled, &home()?)
}
