//! Agents' plugins and extensions, managed through each agent's own CLI
//! (`mise x -- claude plugin …`, `codex plugin …`, `gemini extensions …`).
use powermise_mise::{AgentCli, AgentPlugin, gemini_extensions};
use serde::Serialize;

use crate::{CommandResult, find_mise, home_dir, message, mise_commands::client};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginReport {
    installed: Vec<AgentPlugin>,
    /// Everything the agent's marketplaces offer (Claude Code, Codex); empty
    /// for Gemini CLI, whose extensions install from a URL.
    available: Vec<AgentPlugin>,
    /// Why the agent couldn't be asked, such as it not being installed.
    error: Option<String>,
}

/// An agent's installed and available plugins.
#[tauri::command]
pub async fn agent_plugins(agent: AgentCli) -> CommandResult<PluginReport> {
    tauri::async_runtime::spawn_blocking(move || Ok(report(agent)))
        .await
        .map_err(message)?
}

fn report(agent: AgentCli) -> PluginReport {
    if agent == AgentCli::Gemini {
        return PluginReport {
            installed: home_dir()
                .map(|home| gemini_extensions(&home))
                .unwrap_or_default(),
            available: Vec::new(),
            error: None,
        };
    }
    match client()
        .map_err(|error| error.to_string())
        .and_then(|client| {
            client.agent_plugins(agent).map_err(|_| {
                format!(
                    "{} isn't installed, or couldn't list its plugins",
                    agent.program()
                )
            })
        }) {
        Ok((installed, available)) => PluginReport {
            installed,
            available,
            error: None,
        },
        Err(error) => PluginReport {
            installed: Vec::new(),
            available: Vec::new(),
            error: Some(error),
        },
    }
}

/// The ids of an agent's installed plugins (Gemini: extension names), or
/// `None` when the agent can't be asked yet (not installed).
pub(crate) fn installed_ids(agent: AgentCli) -> Option<Vec<String>> {
    let report = report(agent);
    report.error.is_none().then(|| {
        report
            .installed
            .into_iter()
            .map(|plugin| plugin.id)
            .collect()
    })
}

/// Opens the terminal app installing a plugin that signs in to a service as
/// it installs, which needs a person at the keyboard.
#[tauri::command]
pub fn install_plugin_in_terminal(agent: AgentCli, id: String) -> CommandResult<()> {
    let home = home_dir().ok_or("could not determine your home directory")?;
    let mise = find_mise().ok_or("mise is not installed")?;
    let command = agent.install(&id)?;
    crate::terminal::open(&home, &mise, command.arguments()).map_err(message)
}
