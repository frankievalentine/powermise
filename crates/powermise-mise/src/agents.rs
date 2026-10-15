//! Plugins and extensions for AI agents, through each agent's own CLI.
//!
//! Claude Code and Codex list their plugins as JSON (`<cli> plugin list
//! --json --available`, the same shape for both) and install them with
//! `plugin install`/`plugin add`; Gemini CLI installs extensions from a URL
//! (`gemini extensions install <source> --consent`) and keeps them in
//! `~/.gemini/extensions/<name>/`. powermise runs each CLI through `mise x`,
//! so a mise-installed agent is found from the app.
use std::{fs, path::Path};

use serde::{Deserialize, Serialize};

use crate::MiseCommand;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum AgentCli {
    Claude,
    Codex,
    Gemini,
}

impl AgentCli {
    pub fn program(self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::Codex => "codex",
            Self::Gemini => "gemini",
        }
    }

    /// `mise x -- <cli> …` that installs `id` (a plugin id, or for Gemini a
    /// source URL).
    pub fn install(self, id: &str) -> Result<MiseCommand, String> {
        validate(id)?;
        Ok(match self {
            Self::Claude => {
                MiseCommand::exec("claude", &["plugin", "install", id, "--scope", "user"])
            }
            Self::Codex => MiseCommand::exec("codex", &["plugin", "add", id]),
            Self::Gemini => {
                MiseCommand::exec("gemini", &["extensions", "install", id, "--consent"])
            }
        })
    }

    /// `mise x -- <cli> …` that removes `id` (for Gemini, the extension name).
    pub fn remove(self, id: &str) -> Result<MiseCommand, String> {
        validate(id)?;
        Ok(match self {
            Self::Claude => {
                MiseCommand::exec("claude", &["plugin", "uninstall", id, "--scope", "user"])
            }
            Self::Codex => MiseCommand::exec("codex", &["plugin", "remove", id]),
            Self::Gemini => MiseCommand::exec("gemini", &["extensions", "uninstall", id]),
        })
    }
}

/// Plugin ids, extension names, and https source URLs only: nothing that
/// reads as a flag or holds whitespace.
fn validate(id: &str) -> Result<(), String> {
    let ok = !id.is_empty()
        && !id.starts_with('-')
        && id.len() <= 512
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "@._-/:+".contains(c));
    if ok && (!id.contains(':') || id.starts_with("https://")) {
        Ok(())
    } else {
        Err(format!(
            "`{id}` isn't a plugin id or https URL powermise can pass on"
        ))
    }
}

/// One plugin or extension.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentPlugin {
    /// What install and remove take: `github@anthropic-plugin-directory`, or
    /// a Gemini extension's name.
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub marketplace: Option<String>,
    pub version: Option<String>,
    pub installed: bool,
    pub enabled: bool,
    /// The plugin signs in to a service while installing (Codex's
    /// `authPolicy = "ON_INSTALL"`), so it installs in the terminal.
    pub signs_in_on_install: bool,
}

/// `{ "installed": [...], "available": [...] }` from `<cli> plugin list
/// --json --available`: `(installed, available)`, available excluding what's
/// installed.
pub fn parse_plugin_list(json: &serde_json::Value) -> (Vec<AgentPlugin>, Vec<AgentPlugin>) {
    let read = |key: &str, installed: bool| -> Vec<AgentPlugin> {
        json.get(key)
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|item| {
                let text = |field: &str| {
                    item.get(field)
                        .and_then(serde_json::Value::as_str)
                        .filter(|text| !text.is_empty())
                        .map(str::to_owned)
                };
                let flag = |field: &str| item.get(field).and_then(serde_json::Value::as_bool);
                // Codex says `pluginId` everywhere; Claude Code says `id` for
                // installed plugins.
                let id = text("pluginId").or_else(|| text("id"))?;
                Some(AgentPlugin {
                    name: text("name")
                        .unwrap_or_else(|| id.split('@').next().unwrap_or(&id).to_owned()),
                    description: text("description"),
                    marketplace: text("marketplaceName"),
                    version: text("version"),
                    installed: flag("installed").unwrap_or(installed),
                    enabled: flag("enabled").unwrap_or(installed),
                    signs_in_on_install: text("authPolicy").as_deref() == Some("ON_INSTALL"),
                    id,
                })
            })
            .collect()
    };
    let installed = read("installed", true)
        .into_iter()
        .filter(|plugin| plugin.installed)
        .collect::<Vec<_>>();
    let available = read("available", false)
        .into_iter()
        .filter(|plugin| !plugin.installed && !installed.iter().any(|done| done.id == plugin.id))
        .collect();
    (installed, available)
}

/// Gemini CLI's installed extensions, from
/// `~/.gemini/extensions/<name>/gemini-extension.json`.
pub fn gemini_extensions(home: &Path) -> Vec<AgentPlugin> {
    let Ok(entries) = fs::read_dir(home.join(".gemini/extensions")) else {
        return Vec::new();
    };
    let mut extensions: Vec<AgentPlugin> = entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let manifest: serde_json::Value = serde_json::from_str(
                &fs::read_to_string(entry.path().join("gemini-extension.json")).ok()?,
            )
            .ok()?;
            let name = manifest
                .get("name")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
                .unwrap_or_else(|| entry.file_name().to_string_lossy().into_owned());
            Some(AgentPlugin {
                id: name.clone(),
                name,
                description: manifest
                    .get("description")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_owned),
                version: manifest
                    .get("version")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_owned),
                installed: true,
                enabled: true,
                ..AgentPlugin::default()
            })
        })
        .collect();
    extensions.sort_by(|a, b| a.name.cmp(&b.name));
    extensions
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_claude_and_codex_plugin_lists() {
        let json = serde_json::json!({
            "installed": [
                {"pluginId": "github@dir", "name": "github", "version": "1.0", "installed": true, "enabled": false},
                // Claude Code's installed entries.
                {"id": "commit-commands@dir", "version": "8f81", "scope": "user", "enabled": true}
            ],
            "available": [
                {"pluginId": "github@dir", "name": "github", "description": "GitHub"},
                {"pluginId": "gmail@openai", "name": "gmail", "installed": false, "authPolicy": "ON_INSTALL"}
            ]
        });
        let (installed, available) = parse_plugin_list(&json);
        assert_eq!(installed.len(), 2);
        assert!(installed[0].installed && !installed[0].enabled);
        assert_eq!(installed[1].name, "commit-commands");
        assert!(installed[1].enabled);
        assert_eq!(available.len(), 1);
        assert_eq!(available[0].id, "gmail@openai");
        assert!(available[0].signs_in_on_install);
    }

    #[test]
    fn commands_go_through_mise_and_refuse_odd_ids() {
        assert_eq!(
            AgentCli::Claude
                .install("github@anthropic-plugin-directory")
                .unwrap()
                .display(),
            "mise x -- claude plugin install github@anthropic-plugin-directory --scope user"
        );
        assert_eq!(
            AgentCli::Gemini
                .install("https://github.com/gemini-cli-extensions/workspace")
                .unwrap()
                .display(),
            "mise x -- gemini extensions install https://github.com/gemini-cli-extensions/workspace --consent"
        );
        assert!(AgentCli::Codex.install("--force").is_err());
        assert!(AgentCli::Codex.install("a b").is_err());
        assert!(AgentCli::Gemini.install("file:///etc/passwd").is_err());
    }
}
