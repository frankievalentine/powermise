//! The MCP Servers page: servers turned on for each agent, outside any
//! setup. What's on is saved in `<powermise>/mcp.toml` (with the servers
//! people add themselves), and compiles to `conf.d/powermise-mcp.toml` — one
//! file powermise owns — whose mise dotfiles merge entries add the servers
//! to each agent's own config, with sources in `conf.d/powermise-mcp.files/`.
//! Turning a server off takes it back out of the agent's config, unless a
//! setup still adds it there.
use std::{
    collections::BTreeSet,
    fs, io,
    path::{Path, PathBuf},
};

use powermise_compiler::{Compiled, McpFile, Skipped, compile_mcp_plan};
use powermise_domain::{McpClient, McpServer, McpServerCatalog, Platform};
use powermise_state::write_atomically;
use serde::{Deserialize, Serialize};

use crate::{PowermiseLayout, RemovedServer, SetupsError, write_mcp_files};

/// The name the page's files use (`powermise-mcp.toml`), which setups can't
/// take.
pub const PANEL_NAME: &str = "mcp";

/// What the MCP Servers page has turned on, and the servers people added.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct McpPanel {
    #[serde(default)]
    pub servers: Vec<PanelServer>,
    #[serde(default)]
    pub custom: Vec<McpServer>,
}

/// A server, and the agents it's on for.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PanelServer {
    pub id: String,
    pub clients: Vec<McpClient>,
}

/// `<powermise>/mcp.toml`.
pub fn panel_path(layout: &PowermiseLayout) -> PathBuf {
    layout.root.join("mcp.toml")
}

/// `<mise config dir>/conf.d/powermise-mcp.toml`.
pub fn panel_fragment_path(mise_config_dir: &Path) -> PathBuf {
    mise_config_dir
        .join("conf.d")
        .join(format!("powermise-{PANEL_NAME}.toml"))
}

/// The saved page, or an empty one.
pub fn read_panel(layout: &PowermiseLayout) -> Result<McpPanel, SetupsError> {
    let path = panel_path(layout);
    match fs::read_to_string(&path) {
        Ok(text) => toml::from_str(&text).map_err(|error| SetupsError::Other {
            message: format!("could not read `{}`: {error}", path.display()),
        }),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(McpPanel::default()),
        Err(source) => Err(SetupsError::Io { path, source }),
    }
}

/// Checks a server someone added: a name, and exactly one way to run it.
pub fn validate_custom(server: &McpServer, catalog: &McpServerCatalog) -> Result<(), SetupsError> {
    let fail = |message: String| Err(SetupsError::Other { message });
    let id_ok = !server.id.is_empty()
        && server.id.len() <= 64
        && server
            .id
            .starts_with(|c: char| c.is_ascii_lowercase() || c.is_ascii_digit())
        && server
            .id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_');
    if !id_ok {
        return fail(format!(
            "`{}` can't be a server name: use lowercase letters, numbers, hyphens, and underscores",
            server.id
        ));
    }
    if catalog
        .mcp_servers
        .iter()
        .any(|entry| entry.id == server.id)
    {
        return fail(format!(
            "`{}` is already a server powermise offers",
            server.id
        ));
    }
    if server.title.trim().is_empty() {
        return fail("give the server a name".into());
    }
    let plain = |text: &str| !text.is_empty() && !text.chars().any(char::is_whitespace);
    match (&server.url, &server.tool, &server.command) {
        (Some(url), None, None) => {
            let local = ["http://localhost", "http://127.0.0.1", "http://[::1]"];
            if !plain(url)
                || !(url.starts_with("https://")
                    || local.iter().any(|prefix| url.starts_with(prefix)))
            {
                return fail(format!(
                    "`{url}` isn't a server address: use https:// (or http:// on this computer)"
                ));
            }
            if !server.args.is_empty() || !server.env.is_empty() {
                return fail("a remote server takes no arguments or environment variables".into());
            }
        }
        (None, Some(tool), Some(command)) => {
            if !plain(tool) || tool.contains('"') {
                return fail(format!("`{tool}` isn't a tool mise can install"));
            }
            if !server.version.as_deref().is_some_and(plain) {
                return fail(format!("pick a version of {tool}, or `latest`"));
            }
            if !plain(command) {
                return fail(
                    "the command is the program the tool installs, like `context7-mcp`".into(),
                );
            }
        }
        (None, None, Some(command)) => {
            if command.trim().is_empty() {
                return fail("enter the command that starts the server".into());
            }
        }
        _ => {
            return fail(
                "a server runs a tool mise installs, a command, or is at a remote address".into(),
            );
        }
    }
    for key in server.env.keys() {
        let ok = key.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_')
            && key.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_');
        if !ok {
            return fail(format!("`{key}` isn't an environment variable name"));
        }
    }
    Ok(())
}

/// The page compiled for a platform.
pub struct CompiledPanel {
    /// `conf.d/powermise-mcp.toml`, or `None` when nothing's on.
    pub fragment: Option<String>,
    pub files: Vec<McpFile>,
    /// `(tool, version)` for the servers mise installs.
    pub tools: Vec<(String, String)>,
    pub skipped: Vec<Skipped>,
}

/// Compiles what's on for `target`. Servers no longer offered are left out.
pub fn compile_panel(
    panel: &McpPanel,
    catalog: &McpServerCatalog,
    target: Platform,
    mise_command: &str,
) -> Result<CompiledPanel, SetupsError> {
    let find = |id: &str| {
        catalog
            .mcp_servers
            .iter()
            .chain(&panel.custom)
            .find(|server| server.id == id)
    };
    let mut clients = BTreeSet::new();
    for entry in &panel.servers {
        clients.extend(entry.clients.iter().copied());
    }
    let plan: Vec<(McpClient, Vec<&McpServer>)> = clients
        .into_iter()
        .map(|client| {
            let servers = panel
                .servers
                .iter()
                .filter(|entry| entry.clients.contains(&client))
                .filter_map(|entry| find(&entry.id))
                .collect();
            (client, servers)
        })
        .collect();
    let mut compiled = Compiled::default();
    compile_mcp_plan(&mut compiled, PANEL_NAME, &plan, target, mise_command)?;
    let fragment = if compiled.mcp_files.is_empty() {
        None
    } else {
        Some(format!(
            "# Written by powermise: the MCP servers turned on in Harnesses › MCP\n\
             # Servers. mise installs their tools and merges each agent's servers\n\
             # into that agent's own config file. Turn servers off in powermise to\n\
             # take them back out of those files.\n\n{}",
            compiled.config.to_toml()?
        ))
    };
    Ok(CompiledPanel {
        fragment,
        tools: compiled.config.tools.into_iter().collect(),
        files: compiled.mcp_files,
        skipped: compiled.skipped,
    })
}

/// Saves the page and writes its mise file and merge sources, taking servers
/// that were turned off back out of the agents' configs.
pub fn write_panel(
    layout: &PowermiseLayout,
    mise_config_dir: &Path,
    panel: &McpPanel,
    compiled: &CompiledPanel,
    home: &Path,
    target: Platform,
) -> Result<Vec<RemovedServer>, SetupsError> {
    let path = panel_path(layout);
    let text = toml::to_string_pretty(panel).map_err(|source| SetupsError::TomlSerialize {
        path: path.clone(),
        source,
    })?;
    write_atomically(&path, text.as_bytes())?;
    let fragment = panel_fragment_path(mise_config_dir);
    match &compiled.fragment {
        Some(contents) => {
            write_atomically(&fragment, contents.as_bytes())?;
        }
        None => match fs::remove_file(&fragment) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(source) => {
                return Err(SetupsError::Io {
                    path: fragment,
                    source,
                });
            }
        },
    }
    write_mcp_files(mise_config_dir, PANEL_NAME, &compiled.files, home, target)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn catalog() -> McpServerCatalog {
        toml::from_str(include_str!("../../../catalog/profiles/mcp.toml")).unwrap()
    }

    fn own(id: &str) -> McpServer {
        McpServer {
            id: id.into(),
            title: "Mine".into(),
            description: "Added by you".into(),
            homepage: None,
            tool: Some("npm:example-mcp".into()),
            version: Some("latest".into()),
            command: Some("example-mcp".into()),
            args: vec![],
            url: None,
            env: Default::default(),
        }
    }

    #[test]
    fn checks_servers_people_add() {
        let catalog = catalog();
        assert!(validate_custom(&own("mine"), &catalog).is_ok());
        assert!(validate_custom(&own("context7"), &catalog).is_err());
        assert!(validate_custom(&own("Not OK"), &catalog).is_err());
        let mut remote = own("remote");
        (remote.tool, remote.version, remote.command) = (None, None, None);
        remote.url = Some("http://example.com/mcp".into());
        assert!(validate_custom(&remote, &catalog).is_err());
        remote.url = Some("https://example.com/mcp".into());
        assert!(validate_custom(&remote, &catalog).is_ok());
        let mut env = own("env");
        env.env.insert("BAD KEY".into(), "x".into());
        assert!(validate_custom(&env, &catalog).is_err());
    }

    #[test]
    fn compiles_each_agents_servers_into_one_file() {
        let panel = McpPanel {
            servers: vec![
                PanelServer {
                    id: "context7".into(),
                    clients: vec![McpClient::ClaudeCode, McpClient::Codex],
                },
                PanelServer {
                    id: "mine".into(),
                    clients: vec![McpClient::Codex],
                },
                PanelServer {
                    id: "gone".into(),
                    clients: vec![McpClient::Cursor],
                },
            ],
            custom: vec![own("mine")],
        };
        let compiled = compile_panel(&panel, &catalog(), Platform::Linux, "mise").unwrap();
        let fragment = compiled.fragment.unwrap();
        assert!(fragment.contains("[dotfiles.\"~/.claude.json/powermise-mcp\"]"));
        assert!(fragment.contains("source = \"powermise-mcp.files/codex.toml\""));
        assert!(!fragment.contains("cursor"), "{fragment}");
        assert_eq!(compiled.files.len(), 2);
        assert_eq!(compiled.files[1].servers, ["context7", "mine"]);
        assert!(
            compiled
                .tools
                .contains(&("npm:example-mcp".into(), "latest".into()))
        );

        let empty =
            compile_panel(&McpPanel::default(), &catalog(), Platform::Linux, "mise").unwrap();
        assert!(empty.fragment.is_none() && empty.files.is_empty());
    }
}
