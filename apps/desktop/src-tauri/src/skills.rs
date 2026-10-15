//! Agent skills: what's installed (read from the agents' skills folders and
//! the skills CLI's lock), and the skills CLI and `mise skills` commands.
use powermise_mise::{InstalledSkill, SKILLS_AGENTS, detected_skills_agents, installed_skills};
use serde::Serialize;

use crate::{CommandResult, home_dir, message, mise_commands::client};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillsAgentView {
    id: &'static str,
    title: &'static str,
    /// The agent's config folder exists, so it's probably set up here.
    detected: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolSkill {
    name: String,
    tool: String,
    version: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillsReport {
    installed: Vec<InstalledSkill>,
    agents: Vec<SkillsAgentView>,
    /// Skills that tools mise manages ship (`mise skills ls`).
    tool_skills: Vec<ToolSkill>,
    /// The skills CLI mise runs, such as `npm:skills@1.7.2`.
    cli: &'static str,
}

#[tauri::command]
pub async fn skills_report() -> CommandResult<SkillsReport> {
    tauri::async_runtime::spawn_blocking(|| {
        let home = home_dir().ok_or("could not determine your home directory")?;
        let detected = detected_skills_agents(&home);
        let tool_skills = client()
            .ok()
            .and_then(|client| client.tool_skills().ok())
            .unwrap_or_default()
            .into_iter()
            .map(|(name, tool, version)| ToolSkill {
                name,
                tool,
                version,
            })
            .collect();
        Ok(SkillsReport {
            installed: installed_skills(&home),
            agents: SKILLS_AGENTS
                .iter()
                .map(|agent| SkillsAgentView {
                    id: agent.id,
                    title: agent.title,
                    detected: detected.contains(&agent.id),
                })
                .collect(),
            tool_skills,
            cli: powermise_mise::SKILLS_CLI,
        })
    })
    .await
    .map_err(message)?
}
