//! MCP servers for a setup's agents, as mise dotfiles merge entries.
//!
//! Each agent keeps its MCP servers in its own config file (`~/.claude.json`,
//! `~/.codex/config.toml`, …), which the agent also writes to. A mise
//! `merge = true` entry owns only the keys in its source file and leaves the
//! rest of the target alone (<https://mise.jdx.dev/dotfiles/edits.html#merge>),
//! so the setup's fragment declares one per agent, with sources in
//! `conf.d/powermise-<name>.files/`.
//!
//! Servers mise installs run as `<mise> x <tool>@<version> -- <command>`: the
//! absolute path to mise works from desktop apps that don't see your shell's
//! `PATH`, and the tool is also added to `[tools]` so it's installed up front.
use powermise_domain::{McpClient, McpServer, McpServerCatalog, Platform, ToolProviderKind};
use serde::Serialize;
use serde_json::{Value, json};

use crate::{Compiled, CompilerError, Install, Skipped, insert};

/// `[dotfiles] "<target>/<id>" = { source = "…", merge = true }`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct DotfileMerge {
    pub source: String,
    pub merge: bool,
}

/// One agent's MCP config source, written beside the fragment.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct McpFile {
    pub client: McpClient,
    /// The agent's config file, from home (`~/.claude.json`).
    pub target: String,
    /// The source's file name in `powermise-<name>.files/`.
    pub file_name: String,
    pub contents: String,
    /// The server names it adds, in order.
    pub servers: Vec<String>,
}

/// The folder beside `powermise-<name>.toml` that holds its merge sources.
pub fn files_dir_name(setup: &str) -> String {
    format!("powermise-{setup}.files")
}

/// The merge entry id, unique to the setup.
pub fn edit_id(setup: &str) -> String {
    format!("powermise-{setup}")
}

/// Adds the setup's MCP servers to `compiled`: their tools, and a merge
/// entry plus source file per agent. Agents powermise can't configure on
/// `target` are reported in `skipped`.
pub fn compile_mcp(
    compiled: &mut Compiled,
    setup: &str,
    server_ids: &[String],
    clients: &[McpClient],
    catalog: &McpServerCatalog,
    target: Platform,
    mise_command: &str,
) -> Result<(), CompilerError> {
    let servers = server_ids
        .iter()
        .map(|id| {
            catalog
                .mcp_servers
                .iter()
                .find(|server| &server.id == id)
                .ok_or_else(|| CompilerError::MissingEntry {
                    kind: "mcp server",
                    id: id.clone(),
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    if servers.is_empty() || clients.is_empty() {
        return Ok(());
    }
    for server in &servers {
        if let (Some(tool), Some(version)) = (&server.tool, &server.version) {
            insert(
                &mut compiled.config.tools,
                tool.clone(),
                version.clone(),
                String::as_str,
            )?;
            compiled.installs.push(Install {
                id: server.id.clone(),
                kind: ToolProviderKind::MiseTool,
                key: tool.clone(),
                version: version.clone(),
            });
        }
    }
    for &client in clients {
        let path = match client.config_path(target) {
            Ok(path) => path,
            Err(reason) => {
                compiled.skipped.push(Skipped {
                    id: client.id().into(),
                    reason: format!("MCP servers for {}: {reason}", client.title()),
                });
                continue;
            }
        };
        let mut names = Vec::new();
        let mut entries = serde_json::Map::new();
        for server in &servers {
            match entry(client, server, mise_command) {
                Some(entry) => {
                    names.push(server.id.clone());
                    entries.insert(server.id.clone(), entry);
                }
                None => compiled.skipped.push(Skipped {
                    id: server.id.clone(),
                    reason: format!(
                        "{} only takes servers it runs itself, not remote ones",
                        client.title()
                    ),
                }),
            }
        }
        if names.is_empty() {
            continue;
        }
        let document = json!({ client.servers_key(): entries });
        let (file_name, contents) = if client.is_toml() {
            let table: toml::Table = serde_json::from_value(document)
                .map_err(|error| CompilerError::Render(error.to_string()))?;
            (
                format!("{}.toml", client.id()),
                toml::to_string(&table).map_err(CompilerError::Serialize)?,
            )
        } else {
            (
                format!("{}.json", client.id()),
                format!(
                    "{}\n",
                    serde_json::to_string_pretty(&document)
                        .map_err(|error| CompilerError::Render(error.to_string()))?
                ),
            )
        };
        compiled.config.dotfiles.insert(
            format!("{path}/{}", edit_id(setup)),
            DotfileMerge {
                source: format!("{}/{file_name}", files_dir_name(setup)),
                merge: true,
            },
        );
        compiled.mcp_files.push(McpFile {
            client,
            target: path,
            file_name,
            contents,
            servers: names,
        });
    }
    Ok(())
}

/// One server in `client`'s format, or `None` when the client can't use it.
fn entry(client: McpClient, server: &McpServer, mise: &str) -> Option<Value> {
    if let Some(url) = &server.url {
        return match client {
            McpClient::ClaudeCode | McpClient::Vscode => {
                Some(json!({ "type": "http", "url": url }))
            }
            McpClient::CopilotCli => Some(json!({ "type": "http", "url": url, "tools": ["*"] })),
            McpClient::Codex | McpClient::Cursor => Some(json!({ "url": url })),
            McpClient::GeminiCli => Some(json!({ "httpUrl": url })),
            McpClient::Opencode => Some(json!({ "type": "remote", "url": url, "enabled": true })),
            McpClient::Cline => {
                Some(json!({ "type": "streamableHttp", "url": url, "disabled": false }))
            }
            // `serverUrl` is Antigravity's legacy key; it now reads `url`.
            McpClient::Antigravity => Some(json!({ "url": url })),
            McpClient::ClaudeDesktop | McpClient::Zed => None,
        };
    }
    let (tool, version, command) = (
        server.tool.as_deref()?,
        server.version.as_deref()?,
        server.command.as_deref()?,
    );
    let mut args = vec![
        "x".to_owned(),
        format!("{tool}@{version}"),
        "--".to_owned(),
        command.to_owned(),
    ];
    args.extend(server.args.iter().cloned());
    Some(match client {
        McpClient::ClaudeCode => {
            json!({ "type": "stdio", "command": mise, "args": args, "env": {} })
        }
        McpClient::Vscode => json!({ "type": "stdio", "command": mise, "args": args }),
        McpClient::CopilotCli => {
            json!({ "type": "local", "command": mise, "args": args, "tools": ["*"] })
        }
        McpClient::Opencode => {
            let mut command = vec![mise.to_owned()];
            command.extend(args);
            json!({ "type": "local", "command": command, "enabled": true })
        }
        McpClient::Cline => json!({ "command": mise, "args": args, "disabled": false }),
        McpClient::ClaudeDesktop
        | McpClient::Codex
        | McpClient::GeminiCli
        | McpClient::Cursor
        | McpClient::Antigravity
        | McpClient::Zed => json!({ "command": mise, "args": args }),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn catalog() -> McpServerCatalog {
        let server = |id: &str, tool: Option<&str>, url: Option<&str>| McpServer {
            id: id.into(),
            title: id.into(),
            description: id.into(),
            homepage: None,
            tool: tool.map(Into::into),
            version: tool.map(|_| "1.0.0".into()),
            command: tool.map(|_| format!("{id}-mcp")),
            args: vec![],
            url: url.map(Into::into),
        };
        McpServerCatalog {
            schema_version: 1,
            catalog: "test".into(),
            catalog_revision: "1".into(),
            mcp_servers: vec![
                server("context7", Some("npm:@upstash/context7-mcp"), None),
                server("deepwiki", None, Some("https://mcp.deepwiki.com/mcp")),
            ],
        }
    }

    fn compile(clients: &[McpClient], target: Platform) -> Compiled {
        let mut compiled = Compiled::default();
        compile_mcp(
            &mut compiled,
            "work",
            &["context7".into(), "deepwiki".into()],
            clients,
            &catalog(),
            target,
            "/home/me/.local/bin/mise",
        )
        .unwrap();
        compiled
    }

    #[test]
    fn writes_a_merge_entry_and_source_per_agent() {
        let compiled = compile(&[McpClient::ClaudeCode, McpClient::Codex], Platform::Linux);
        assert_eq!(
            compiled.config.dotfiles["~/.claude.json/powermise-work"],
            DotfileMerge {
                source: "powermise-work.files/claude-code.json".into(),
                merge: true
            }
        );
        let claude: Value = serde_json::from_str(&compiled.mcp_files[0].contents).unwrap();
        assert_eq!(
            claude["mcpServers"]["context7"]["args"],
            json!(["x", "npm:@upstash/context7-mcp@1.0.0", "--", "context7-mcp"])
        );
        assert_eq!(
            claude["mcpServers"]["context7"]["command"],
            "/home/me/.local/bin/mise"
        );
        assert_eq!(claude["mcpServers"]["deepwiki"]["type"], "http");
        let codex = &compiled.mcp_files[1];
        assert_eq!(codex.target, "~/.codex/config.toml");
        assert!(
            codex.contents.contains("[mcp_servers.context7]"),
            "{}",
            codex.contents
        );
        assert!(
            codex
                .contents
                .contains("url = \"https://mcp.deepwiki.com/mcp\"")
        );
        // The server's tool installs up front.
        assert_eq!(compiled.config.tools["npm:@upstash/context7-mcp"], "1.0.0");
        let toml = compiled.config.to_toml().unwrap();
        assert!(
            toml.contains("[dotfiles.\"~/.codex/config.toml/powermise-work\"]"),
            "{toml}"
        );
    }

    #[test]
    fn reports_agents_and_servers_it_cant_configure() {
        let compiled = compile(
            &[McpClient::ClaudeDesktop, McpClient::Zed],
            Platform::Darwin,
        );
        let skipped: Vec<_> = compiled
            .skipped
            .iter()
            .map(|skip| skip.id.as_str())
            .collect();
        // Claude's desktop app runs servers itself only; Zed's settings allow comments.
        assert_eq!(skipped, ["deepwiki", "zed"]);
        assert_eq!(compiled.mcp_files.len(), 1);
        let linux = compile(&[McpClient::ClaudeDesktop], Platform::Linux);
        assert!(linux.mcp_files.is_empty());
        assert_eq!(linux.skipped[0].id, "claude-desktop");
    }

    #[test]
    fn opencode_takes_the_command_as_one_list() {
        let compiled = compile(&[McpClient::Opencode], Platform::Win32);
        let config: Value = serde_json::from_str(&compiled.mcp_files[0].contents).unwrap();
        assert_eq!(
            config["mcp"]["context7"]["command"][0],
            "/home/me/.local/bin/mise"
        );
        assert_eq!(config["mcp"]["deepwiki"]["type"], "remote");
    }
}
