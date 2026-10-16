//! The MCP config sources beside a setup's fragment, and taking servers back
//! out of agents' config files.
//!
//! mise's merge entries add keys but never remove them: deleting an entry, or
//! a key from its source, leaves the key in the agent's file
//! (<https://mise.jdx.dev/dotfiles/edits.html#conflicts>). So when a setup
//! drops a server, or is removed, powermise deletes exactly the servers its
//! previous source files added — by name, under the agent's servers key — and
//! leaves everything else in the file as it was.
use std::{
    collections::BTreeSet,
    fs, io,
    path::{Path, PathBuf},
};

use powermise_compiler::{McpFile, files_dir_name};
use powermise_domain::{McpClient, Platform};

use crate::SetupsError;

/// `<mise config dir>/conf.d/powermise-<name>.files`.
pub fn files_dir(mise_config_dir: &Path, setup: &str) -> PathBuf {
    mise_config_dir.join("conf.d").join(files_dir_name(setup))
}

/// A server powermise took out of an agent's config.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
pub struct RemovedServer {
    pub client: McpClient,
    pub server: String,
    pub path: PathBuf,
}

/// Writes `files` into the setup's folder, deletes sources it no longer
/// needs, and removes servers the old sources added that the new ones don't.
pub fn write_mcp_files(
    mise_config_dir: &Path,
    setup: &str,
    files: &[McpFile],
    home: &Path,
    target: Platform,
) -> Result<Vec<RemovedServer>, SetupsError> {
    let directory = files_dir(mise_config_dir, setup);
    let removed = remove_stale_servers(&directory, files, home, target)?;
    if files.is_empty() {
        remove_dir(&directory)?;
        return Ok(removed);
    }
    fs::create_dir_all(&directory).map_err(|source| io_error(&directory, source))?;
    let keep: BTreeSet<&str> = files.iter().map(|file| file.file_name.as_str()).collect();
    for entry in read_dir(&directory)? {
        let name = entry.file_name().to_string_lossy().into_owned();
        if !keep.contains(name.as_str()) {
            let path = entry.path();
            fs::remove_file(&path).map_err(|source| io_error(&path, source))?;
        }
    }
    for file in files {
        powermise_state::write_atomically(
            &directory.join(&file.file_name),
            file.contents.as_bytes(),
        )?;
    }
    Ok(removed)
}

/// Takes every server the setup added out of the agents' configs and deletes
/// its sources, for removing the setup.
pub fn remove_mcp_files(
    mise_config_dir: &Path,
    setup: &str,
    home: &Path,
    target: Platform,
) -> Result<Vec<RemovedServer>, SetupsError> {
    let directory = files_dir(mise_config_dir, setup);
    let removed = remove_stale_servers(&directory, &[], home, target)?;
    remove_dir(&directory)?;
    Ok(removed)
}

/// Servers in the sources on disk that `files` no longer has, removed from
/// their agents' configs.
fn remove_stale_servers(
    directory: &Path,
    files: &[McpFile],
    home: &Path,
    target: Platform,
) -> Result<Vec<RemovedServer>, SetupsError> {
    let mut removed = Vec::new();
    for entry in read_dir(directory)? {
        let name = entry.file_name().to_string_lossy().into_owned();
        let Some(client) = client_for(&name) else {
            continue;
        };
        let Ok(old) = fs::read_to_string(entry.path()) else {
            continue;
        };
        let keep: BTreeSet<&str> = files
            .iter()
            .filter(|file| file.file_name == name)
            .flat_map(|file| file.servers.iter().map(String::as_str))
            .collect();
        let stale: Vec<String> = server_names(client, &old)
            .into_iter()
            .filter(|server| !keep.contains(server.as_str()))
            .collect();
        if stale.is_empty() {
            continue;
        }
        let Ok(path) = client.config_path(target) else {
            continue;
        };
        let path = expand_home(&path, home);
        for server in remove_servers(client, &path, &stale)? {
            removed.push(RemovedServer {
                client,
                server,
                path: path.clone(),
            });
        }
    }
    Ok(removed)
}

/// The client a source file belongs to, from its name (`codex.toml`).
fn client_for(file_name: &str) -> Option<McpClient> {
    let id = file_name.rsplit_once('.')?.0;
    ALL_CLIENTS.into_iter().find(|client| client.id() == id)
}

const ALL_CLIENTS: [McpClient; 11] = [
    McpClient::ClaudeCode,
    McpClient::ClaudeDesktop,
    McpClient::Codex,
    McpClient::GeminiCli,
    McpClient::Opencode,
    McpClient::Cursor,
    McpClient::Vscode,
    McpClient::CopilotCli,
    McpClient::Cline,
    McpClient::Amp,
    McpClient::Zed,
];

/// The server names in a source file.
fn server_names(client: McpClient, contents: &str) -> Vec<String> {
    if client.is_toml() {
        contents
            .parse::<toml::Table>()
            .ok()
            .and_then(|table| table.get(client.servers_key())?.as_table().cloned())
            .map(|servers| servers.keys().cloned().collect())
            .unwrap_or_default()
    } else {
        serde_json::from_str::<serde_json::Value>(contents)
            .ok()
            .and_then(|value| value.get(client.servers_key())?.as_object().cloned())
            .map(|servers| servers.keys().cloned().collect())
            .unwrap_or_default()
    }
}

/// Removes `servers` from the agent's config at `path`, keeping the rest of
/// the file — key order and indentation for JSON, comments and formatting for
/// TOML. A missing or unreadable file is left alone. Returns what it removed.
fn remove_servers(
    client: McpClient,
    path: &Path,
    servers: &[String],
) -> Result<Vec<String>, SetupsError> {
    let Ok(contents) = fs::read_to_string(path) else {
        return Ok(Vec::new());
    };
    let key = client.servers_key();
    let mut removed = Vec::new();
    let updated = if client.is_toml() {
        let Ok(mut document) = contents.parse::<toml_edit::DocumentMut>() else {
            return Ok(Vec::new());
        };
        if let Some(table) = document
            .get_mut(key)
            .and_then(|item| item.as_table_like_mut())
        {
            for server in servers {
                if table.remove(server).is_some() {
                    removed.push(server.clone());
                }
            }
        }
        document.to_string()
    } else {
        // JSON with comments (or anything else) doesn't parse, and stays as is.
        let Ok(mut value) = serde_json::from_str::<serde_json::Value>(&contents) else {
            return Ok(Vec::new());
        };
        if let Some(table) = value
            .get_mut(key)
            .and_then(serde_json::Value::as_object_mut)
        {
            for server in servers {
                if table.shift_remove(server).is_some() {
                    removed.push(server.clone());
                }
            }
        }
        let indent = json_indent(&contents);
        let mut out = Vec::new();
        let formatter = serde_json::ser::PrettyFormatter::with_indent(indent.as_bytes());
        let mut serializer = serde_json::Serializer::with_formatter(&mut out, formatter);
        serde::Serialize::serialize(&value, &mut serializer)
            .map_err(|error| io_error(path, io::Error::other(error)))?;
        let mut text = String::from_utf8(out).unwrap_or_default();
        if contents.ends_with('\n') {
            text.push('\n');
        }
        text
    };
    if !removed.is_empty() {
        powermise_state::write_atomically(path, updated.as_bytes())?;
    }
    Ok(removed)
}

/// The indentation a JSON file uses (two spaces when it can't tell).
fn json_indent(contents: &str) -> String {
    contents
        .lines()
        .nth(1)
        .map(|line| {
            line.chars()
                .take_while(|c| *c == ' ' || *c == '\t')
                .collect::<String>()
        })
        .filter(|indent| !indent.is_empty())
        .unwrap_or_else(|| "  ".into())
}

/// `~/x` → `<home>/x`.
pub fn expand_home(path: &str, home: &Path) -> PathBuf {
    path.strip_prefix("~/")
        .map(|rest| home.join(rest))
        .unwrap_or_else(|| PathBuf::from(path))
}

fn read_dir(directory: &Path) -> Result<Vec<fs::DirEntry>, SetupsError> {
    match fs::read_dir(directory) {
        Ok(entries) => Ok(entries
            .filter_map(Result::ok)
            .filter(|entry| entry.path().is_file())
            .collect()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(source) => Err(io_error(directory, source)),
    }
}

fn remove_dir(directory: &Path) -> Result<(), SetupsError> {
    match fs::remove_dir_all(directory) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(io_error(directory, source)),
    }
}

fn io_error(path: &Path, source: io::Error) -> SetupsError {
    SetupsError::Io {
        path: path.to_path_buf(),
        source,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let path =
            std::env::temp_dir().join(format!("powermise-mcp-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        path
    }

    fn file(client: McpClient, servers: &[&str], contents: &str) -> McpFile {
        McpFile {
            client,
            target: client.config_path(Platform::Linux).unwrap(),
            file_name: format!(
                "{}.{}",
                client.id(),
                if client.is_toml() { "toml" } else { "json" }
            ),
            contents: contents.into(),
            servers: servers.iter().map(|server| (*server).into()).collect(),
        }
    }

    #[test]
    fn dropping_a_server_takes_only_it_out_of_the_agents_config() {
        let root = temp("drop");
        let (home, mise) = (root.join("home"), root.join("mise"));
        fs::create_dir_all(home.join(".codex")).unwrap();
        fs::write(
            home.join(".claude.json"),
            "{\n    \"numStartups\": 3,\n    \"mcpServers\": {\n        \"mine\": {},\n        \"context7\": {},\n        \"memory\": {}\n    }\n}\n",
        )
        .unwrap();
        fs::write(
            home.join(".codex/config.toml"),
            "model = \"o3\" # mine\n\n[mcp_servers.context7]\ncommand = \"mise\"\n\n[mcp_servers.mine]\ncommand = \"x\"\n",
        )
        .unwrap();
        let both = [
            file(
                McpClient::ClaudeCode,
                &["context7", "memory"],
                r#"{"mcpServers":{"context7":{},"memory":{}}}"#,
            ),
            file(
                McpClient::Codex,
                &["context7"],
                "[mcp_servers.context7]\ncommand = \"mise\"\n",
            ),
        ];
        write_mcp_files(&mise, "work", &both, &home, Platform::Linux).unwrap();

        // Drop `memory` from Claude and Codex entirely.
        let fewer = [file(
            McpClient::ClaudeCode,
            &["context7"],
            r#"{"mcpServers":{"context7":{}}}"#,
        )];
        let removed = write_mcp_files(&mise, "work", &fewer, &home, Platform::Linux).unwrap();
        let mut names: Vec<_> = removed
            .iter()
            .map(|entry| (entry.client.id(), entry.server.as_str()))
            .collect();
        names.sort();
        assert_eq!(names, [("claude-code", "memory"), ("codex", "context7")]);

        assert_eq!(
            fs::read_to_string(home.join(".claude.json")).unwrap(),
            "{\n    \"numStartups\": 3,\n    \"mcpServers\": {\n        \"mine\": {},\n        \"context7\": {}\n    }\n}\n"
        );
        assert_eq!(
            fs::read_to_string(home.join(".codex/config.toml")).unwrap(),
            "model = \"o3\" # mine\n\n[mcp_servers.mine]\ncommand = \"x\"\n"
        );
        assert!(!files_dir(&mise, "work").join("codex.toml").exists());

        // Removing the setup takes the rest out and deletes the folder.
        remove_mcp_files(&mise, "work", &home, Platform::Linux).unwrap();
        let claude = fs::read_to_string(home.join(".claude.json")).unwrap();
        assert!(!claude.contains("context7") && claude.contains("mine"));
        assert!(!files_dir(&mise, "work").exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn leaves_files_it_cannot_parse_alone() {
        let root = temp("jsonc");
        let path = root.join("settings.json");
        let jsonc = "{\n  // comment\n  \"mcpServers\": { \"context7\": {} }\n}\n";
        fs::write(&path, jsonc).unwrap();
        assert!(
            remove_servers(McpClient::Cursor, &path, &["context7".into()])
                .unwrap()
                .is_empty()
        );
        assert_eq!(fs::read_to_string(&path).unwrap(), jsonc);
        fs::remove_dir_all(root).unwrap();
    }
}
