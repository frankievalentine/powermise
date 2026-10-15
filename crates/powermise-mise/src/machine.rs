//! Machine-wide mise state the app shows outside any project: settings, the
//! GitHub token mise downloads with, and versions other managers installed.
use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::Serialize;

/// One mise setting, flattened from `mise settings ls --all --json-extended`.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Setting {
    /// Dotted, for example `jobs` or `node.corepack`.
    pub key: String,
    pub value: serde_json::Value,
    /// mise's type name: `boolean`, `number`, `string`, `array`, or a union
    /// such as `boolean | string`.
    #[serde(rename = "type")]
    pub kind: String,
    pub description: String,
    /// The file (or environment variable) that sets it; `None` is the default.
    pub source: Option<String>,
    pub deprecated: bool,
}

/// Flattens mise's nested settings JSON into dotted keys, sorted.
pub fn parse_settings(json: &serde_json::Value) -> Vec<Setting> {
    fn walk(prefix: &str, value: &serde_json::Value, settings: &mut Vec<Setting>) {
        let Some(object) = value.as_object() else {
            return;
        };
        for (name, entry) in object {
            let key = if prefix.is_empty() {
                name.clone()
            } else {
                format!("{prefix}.{name}")
            };
            let leaf = entry.get("type").is_some_and(serde_json::Value::is_string)
                && entry.get("value").is_some();
            if leaf {
                let description = entry
                    .get("description")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_default();
                settings.push(Setting {
                    deprecated: description.starts_with("[deprecated]"),
                    description: description
                        .trim_start_matches("[deprecated]")
                        .trim()
                        .to_owned(),
                    value: entry["value"].clone(),
                    kind: entry["type"].as_str().unwrap_or_default().to_owned(),
                    source: entry
                        .get("source")
                        .and_then(serde_json::Value::as_str)
                        .map(str::to_owned),
                    key,
                });
            } else {
                walk(&key, entry, settings);
            }
        }
    }
    let mut settings = Vec::new();
    walk("", json, &mut settings);
    settings.sort_by(|a, b| a.key.cmp(&b.key));
    settings
}

/// `mise token github`, parsed: `github.com: ghp_…1234 (source: GITHUB_TOKEN)`
/// or `github.com: (none)`.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenStatus {
    pub host: String,
    /// Masked by mise, for example `ghp_…1234`.
    pub token: Option<String>,
    pub source: Option<String>,
}

pub fn parse_token_status(output: &str) -> Option<TokenStatus> {
    let line = output.lines().find(|line| line.contains(": "))?;
    let (host, rest) = line.split_once(": ")?;
    let rest = rest.trim();
    if rest == "(none)" {
        return Some(TokenStatus {
            host: host.trim().into(),
            token: None,
            source: None,
        });
    }
    let (token, source) = match rest.split_once(" (source: ") {
        Some((token, source)) => (token, Some(source.trim_end_matches(')').to_owned())),
        None => (rest, None),
    };
    Some(TokenStatus {
        host: host.trim().into(),
        token: Some(token.trim().into()),
        source,
    })
}

/// Another version manager whose installs `mise sync` can bring into mise.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ImportSource {
    Nvm,
    Nodenv,
    NodeBrew,
    Pyenv,
    Uv,
    RubyBrew,
}

impl ImportSource {
    pub const ALL: [Self; 6] = [
        Self::Nvm,
        Self::Nodenv,
        Self::NodeBrew,
        Self::Pyenv,
        Self::Uv,
        Self::RubyBrew,
    ];

    /// The mise tool it provides versions of.
    pub fn tool(self) -> &'static str {
        match self {
            Self::Nvm | Self::Nodenv | Self::NodeBrew => "node",
            Self::Pyenv | Self::Uv => "python",
            Self::RubyBrew => "ruby",
        }
    }

    /// The `mise sync <tool>` flag.
    pub fn flag(self) -> &'static str {
        match self {
            Self::Nvm => "nvm",
            Self::Nodenv => "nodenv",
            Self::NodeBrew | Self::RubyBrew => "brew",
            Self::Pyenv => "pyenv",
            Self::Uv => "uv",
        }
    }
}

/// Versions a manager has installed on this machine.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectedImport {
    pub source: ImportSource,
    pub tool: &'static str,
    pub versions: Vec<String>,
}

/// Looks for other version managers' installs in their default locations
/// (or the location their environment variable names). Reads directory
/// names only; runs nothing.
pub fn detect_imports(home: &Path) -> Vec<DetectedImport> {
    let env_dir = |name: &str| std::env::var_os(name).map(PathBuf::from);
    let mut found = Vec::new();
    for source in ImportSource::ALL {
        let mut versions = match source {
            ImportSource::Nvm => list(
                &env_dir("NVM_DIR")
                    .unwrap_or_else(|| home.join(".nvm"))
                    .join("versions/node"),
            )
            .into_iter()
            .map(|name| name.trim_start_matches('v').to_owned())
            .collect(),
            ImportSource::Nodenv => list(
                &env_dir("NODENV_ROOT")
                    .unwrap_or_else(|| home.join(".nodenv"))
                    .join("versions"),
            ),
            ImportSource::Pyenv => list(
                &env_dir("PYENV_ROOT")
                    .unwrap_or_else(|| home.join(".pyenv"))
                    .join("versions"),
            ),
            ImportSource::Uv => {
                // uv's defaults: `%APPDATA%/uv/data/python` on Windows,
                // `$XDG_DATA_HOME/uv/python` elsewhere.
                let directory = env_dir("UV_PYTHON_INSTALL_DIR").unwrap_or_else(|| {
                    if cfg!(windows) {
                        env_dir("APPDATA")
                            .unwrap_or_else(|| home.join("AppData/Roaming"))
                            .join("uv/data/python")
                    } else {
                        env_dir("XDG_DATA_HOME")
                            .unwrap_or_else(|| home.join(".local/share"))
                            .join("uv/python")
                    }
                });
                // `cpython-3.12.4-macos-aarch64-none` → `3.12.4`.
                list(&directory)
                    .into_iter()
                    .filter_map(|name| {
                        name.strip_prefix("cpython-")
                            .and_then(|rest| rest.split('-').next())
                            .map(str::to_owned)
                    })
                    .collect()
            }
            ImportSource::NodeBrew => homebrew_versions("node"),
            ImportSource::RubyBrew => homebrew_versions("ruby"),
        };
        versions.sort();
        versions.dedup();
        if !versions.is_empty() {
            found.push(DetectedImport {
                source,
                tool: source.tool(),
                versions,
            });
        }
    }
    found
}

/// Visible subdirectory names, or nothing if the directory doesn't exist.
fn list(directory: &Path) -> Vec<String> {
    let Ok(entries) = fs::read_dir(directory) else {
        return Vec::new();
    };
    entries
        .filter_map(Result::ok)
        .filter(|entry| entry.path().is_dir())
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|name| !name.starts_with('.'))
        .collect()
}

/// Versions of a Homebrew formula and its versioned variants (`node`,
/// `node@22`) in each standard Homebrew prefix.
fn homebrew_versions(formula: &str) -> Vec<String> {
    let mut versions = Vec::new();
    for prefix in ["/opt/homebrew", "/usr/local", "/home/linuxbrew/.linuxbrew"] {
        let cellar = Path::new(prefix).join("Cellar");
        for name in list(&cellar) {
            if name == formula || name.starts_with(&format!("{formula}@")) {
                versions.extend(list(&cellar.join(&name)));
            }
        }
    }
    // Revision suffixes (`22.9.0_1`) aren't part of the version.
    versions
        .into_iter()
        .map(|version| version.split('_').next().unwrap_or_default().to_owned())
        .collect()
}

/// Folders and files mise trusts: the links in `<state>/trusted-configs`.
pub fn trusted_configs(state_dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(state_dir.join("trusted-configs")) else {
        return Vec::new();
    };
    let mut paths: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .filter_map(|entry| fs::read_link(entry.path()).ok())
        .collect();
    paths.sort();
    paths.dedup();
    paths
}

/// Total size of a directory, without following symlinks.
pub fn directory_size(path: &Path) -> u64 {
    let Ok(metadata) = fs::symlink_metadata(path) else {
        return 0;
    };
    if !metadata.is_dir() {
        return metadata.len();
    }
    fs::read_dir(path)
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .map(|entry| directory_size(&entry.path()))
                .sum()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn machine_status_flattens_every_part() {
        let json = serde_json::json!({
            "packages": {"brew": {"available": true, "packages": [
                {"package": "hello", "requested_version": "latest", "state": "missing", "installed_version": ""}
            ]}},
            "tools": [{"tool": "jq", "requested_version": "1.7.1", "resolved_version": "1.7.1", "state": "installed", "installed": true}],
            "dotfiles": {"files": [{"target": "~/.gitconfig", "source": "/c/gitconfig", "mode": "symlink", "state": "missing"}], "edits": []},
            "repos": [],
            "login_shell": null,
            "macos_defaults": {"entries": []}
        });
        let sections = parse_machine_status(&json);
        let ids: Vec<_> = sections.iter().map(|section| section.id.as_str()).collect();
        // In mise's order.
        assert_eq!(ids, ["packages", "tools", "dotfiles"]);
        assert_eq!(sections[0].items[0].name, "brew:hello");
        assert_eq!(sections[0].items[0].state.as_deref(), Some("missing"));
        assert_eq!(sections[1].items[0].detail.as_deref(), Some("1.7.1"));
        assert_eq!(sections[2].items[0].name, "~/.gitconfig");
        assert_eq!(sections[2].items[0].detail.as_deref(), Some("symlink"));

        let macos = parse_machine_status(
            &serde_json::json!({"macos_defaults": {"available": true, "entries": [
                {"domain": "com.apple.dock", "key": "autohide", "value": true, "current": "", "state": "unset"}
            ]}}),
        );
        assert_eq!(macos[0].items[0].name, "com.apple.dock autohide");
        assert_eq!(macos[0].items[0].detail.as_deref(), Some("true"));
    }

    #[test]
    fn checkpoints_parse_from_history_json() {
        // Shape captured from mise 2026.10.10 (trimmed).
        let json = serde_json::json!([
            {"id": 4, "created_at": "2026-10-14T23:20:43-07:00", "trigger": "save",
             "summary": "edited ~/.zshrc", "machine": {"id": "git", "name": "git"},
             "changes": {"since": "cdc9", "modified": ["~/.zshrc"], "truncated": false}},
            {"id": 3, "trigger": "baseline", "description": "tracked ~/.zshrc"}
        ]);
        let checkpoints = parse_checkpoints(&json);
        assert_eq!(checkpoints.len(), 2);
        assert_eq!(checkpoints[0].changed, ["~/.zshrc"]);
        assert_eq!(checkpoints[0].machine.as_deref(), Some("git"));
        assert_eq!(checkpoints[1].summary.as_deref(), Some("tracked ~/.zshrc"));
    }

    #[test]
    fn aliases_and_plugins_parse() {
        // Captured from mise 2026.7.18 and 2026.10.10.
        let aliases = parse_tool_aliases(
            "java  lts          25\nnode  lts          24\nnode  lts-argon    4 \n",
        );
        assert_eq!(aliases.len(), 3);
        assert_eq!(
            aliases[1],
            ToolAlias {
                tool: "node".into(),
                alias: "lts".into(),
                version: "24".into()
            }
        );
        let plugins =
            parse_plugins("tiny  https://github.com/mise-plugins/rtx-tiny  main 4a1c2e3\nbare\n");
        assert_eq!(
            plugins[0].url.as_deref(),
            Some("https://github.com/mise-plugins/rtx-tiny")
        );
        assert_eq!(plugins[0].revision.as_deref(), Some("main 4a1c2e3"));
        assert_eq!(plugins[1].url, None);
    }

    #[test]
    fn token_lines_parse() {
        assert_eq!(
            parse_token_status("github.com: (none)\n"),
            Some(TokenStatus {
                host: "github.com".into(),
                token: None,
                source: None
            })
        );
        assert_eq!(
            parse_token_status("github.com: ghp_…1234 (source: GITHUB_TOKEN)\n"),
            Some(TokenStatus {
                host: "github.com".into(),
                token: Some("ghp_…1234".into()),
                source: Some("GITHUB_TOKEN".into())
            })
        );
        assert_eq!(parse_token_status(""), None);
    }

    #[test]
    fn settings_flatten_with_sources_and_deprecation() {
        let json = serde_json::json!({
            "jobs": {"value": 4, "type": "number", "description": "How many jobs.", "source": "/c.toml"},
            "always_keep_download": {"value": false, "type": "boolean", "description": "[deprecated] Keep downloads."},
            "node": {"corepack": {"value": false, "type": "boolean", "description": "Corepack."}}
        });
        let settings = parse_settings(&json);
        let keys: Vec<_> = settings
            .iter()
            .map(|setting| setting.key.as_str())
            .collect();
        assert_eq!(keys, ["always_keep_download", "jobs", "node.corepack"]);
        assert!(settings[0].deprecated);
        assert_eq!(settings[0].description, "Keep downloads.");
        assert_eq!(settings[1].source.as_deref(), Some("/c.toml"));
        assert_eq!(settings[2].source, None);
    }

    #[test]
    fn imports_are_found_in_default_locations() {
        let home = std::env::temp_dir().join(format!("powermise-imports-{}", std::process::id()));
        fs::create_dir_all(home.join(".nvm/versions/node/v20.11.1")).unwrap();
        fs::create_dir_all(home.join(".nvm/versions/node/v22.3.0")).unwrap();
        fs::create_dir_all(home.join(".pyenv/versions/3.12.4")).unwrap();
        // Only read when the variables aren't set in the test environment.
        if std::env::var_os("NVM_DIR").is_none() && std::env::var_os("PYENV_ROOT").is_none() {
            let found = detect_imports(&home);
            let nvm = found
                .iter()
                .find(|entry| entry.source == ImportSource::Nvm)
                .unwrap();
            assert_eq!(nvm.versions, ["20.11.1", "22.3.0"]);
            assert_eq!(nvm.tool, "node");
            let pyenv = found
                .iter()
                .find(|entry| entry.source == ImportSource::Pyenv)
                .unwrap();
            assert_eq!(pyenv.versions, ["3.12.4"]);
        }
        let _ = fs::remove_dir_all(home);
    }
}

/// One part of `mise bootstrap status --json` (packages, tools, dotfiles,
/// repos, services, macOS defaults, …), flattened to rows the app can show
/// without knowing every part's shape — mise adds parts between releases.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MachineSection {
    /// The status key, for example `packages` or `macos_defaults`.
    pub id: String,
    pub items: Vec<MachineItem>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MachineItem {
    pub name: String,
    /// mise's state word: `installed`, `missing`, `applied`, `changed`, …
    pub state: Option<String>,
    /// A version, mode, or similar extra.
    pub detail: Option<String>,
}

/// Flattens `mise bootstrap status --json` into sections, skipping empty ones.
pub fn parse_machine_status(json: &serde_json::Value) -> Vec<MachineSection> {
    let Some(object) = json.as_object() else {
        return Vec::new();
    };
    let text = |value: &serde_json::Value, keys: &[&str]| {
        keys.iter().find_map(|key| match value.get(*key) {
            Some(serde_json::Value::String(text)) if !text.is_empty() => Some(text.clone()),
            Some(serde_json::Value::Bool(flag)) => Some(flag.to_string()),
            Some(serde_json::Value::Number(number)) => Some(number.to_string()),
            _ => None,
        })
    };
    let generic = |entry: &serde_json::Value| -> MachineItem {
        // macOS preferences: `domain` alone repeats on every row.
        if let (Some(domain), Some(key)) = (text(entry, &["domain"]), text(entry, &["key"])) {
            return MachineItem {
                name: format!("{domain} {key}"),
                state: text(entry, &["state", "status"]),
                detail: entry.get("value").map(|value| match value {
                    serde_json::Value::String(text) => text.clone(),
                    other => other.to_string(),
                }),
            };
        }
        MachineItem {
            name: text(
                entry,
                &[
                    "name", "target", "path", "tool", "package", "repo", "url", "service", "unit",
                    "label", "domain", "key", "shell", "id", "file",
                ],
            )
            .unwrap_or_else(|| entry.to_string()),
            state: text(entry, &["state", "status"]),
            detail: text(
                entry,
                &[
                    "resolved_version",
                    "installed_version",
                    "version",
                    "requested_version",
                    "mode",
                    "value",
                    "detail",
                    "message",
                ],
            ),
        }
    };
    let mut sections = Vec::new();
    for (id, value) in object {
        let items: Vec<MachineItem> = match (id.as_str(), value) {
            // `{ "<manager>": { "available": bool, "packages": [...] } }`
            ("packages", serde_json::Value::Object(managers)) => managers
                .iter()
                .flat_map(|(manager, status)| {
                    status
                        .get("packages")
                        .and_then(serde_json::Value::as_array)
                        .into_iter()
                        .flatten()
                        .map(move |package| {
                            let mut item = generic(package);
                            item.name = format!("{manager}:{}", item.name);
                            item
                        })
                })
                .collect(),
            (_, serde_json::Value::Array(entries)) => entries.iter().map(generic).collect(),
            // `{ "files": [...], "edits": [...] }`, `{ "entries": [...] }`, …
            (_, serde_json::Value::Object(parts)) => parts
                .values()
                .filter_map(serde_json::Value::as_array)
                .flatten()
                .map(generic)
                .collect(),
            // `login_shell`: an object when configured.
            (_, serde_json::Value::Null) => Vec::new(),
            (_, other) => vec![MachineItem {
                name: other.to_string(),
                state: None,
                detail: None,
            }],
        };
        if !items.is_empty() {
            sections.push(MachineSection {
                id: id.clone(),
                items,
            });
        }
    }
    sections
}

/// The `history` part of `mise dotfiles status --json` (2026.10+): saved
/// checkpoints of tracked files.
#[derive(Clone, Debug, Default, serde::Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DotfilesHistory {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default, alias = "tracked_files")]
    pub tracked_files: u64,
    #[serde(default)]
    pub checkpoints: u64,
    #[serde(default)]
    pub latest: Option<serde_json::Value>,
    #[serde(default)]
    pub unsaved: Option<serde_json::Value>,
    #[serde(default, alias = "pending_operations")]
    pub pending_operations: u64,
}

/// One saved checkpoint from `mise dotfiles history --json` (2026.10+).
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DotfileCheckpoint {
    pub id: u64,
    pub created_at: Option<String>,
    /// `save`, `edit`, `baseline`, `apply`, …
    pub trigger: Option<String>,
    pub summary: Option<String>,
    pub machine: Option<String>,
    /// Paths it changed (modified, added, or removed), as mise lists them.
    pub changed: Vec<String>,
}

/// Reads `mise dotfiles history --json` defensively: it's new, so only the
/// fields the app shows are required to look the way they do today.
pub fn parse_checkpoints(json: &serde_json::Value) -> Vec<DotfileCheckpoint> {
    let text = |value: &serde_json::Value, key: &str| {
        value
            .get(key)
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned)
    };
    json.as_array()
        .into_iter()
        .flatten()
        .filter_map(|entry| {
            let id = entry.get("id")?.as_u64()?;
            let changed = entry
                .get("changes")
                .and_then(serde_json::Value::as_object)
                .map(|changes| {
                    changes
                        .values()
                        .filter_map(serde_json::Value::as_array)
                        .flatten()
                        .filter_map(|path| path.as_str().map(str::to_owned))
                        .collect()
                })
                .unwrap_or_default();
            Some(DotfileCheckpoint {
                id,
                created_at: text(entry, "created_at"),
                trigger: text(entry, "trigger"),
                summary: text(entry, "summary").or_else(|| text(entry, "description")),
                machine: entry
                    .get("machine")
                    .and_then(|machine| text(machine, "name")),
                changed,
            })
        })
        .collect()
}

/// A version alias, like `node lts → 24` (`mise tool-alias ls`).
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolAlias {
    pub tool: String,
    pub alias: String,
    pub version: String,
}

/// Parses `mise tool-alias ls`: `tool  alias  version` per line.
pub fn parse_tool_aliases(output: &str) -> Vec<ToolAlias> {
    output
        .lines()
        .filter_map(|line| {
            let mut words = line.split_whitespace();
            let (tool, alias, version) = (words.next()?, words.next()?, words.next()?);
            Some(ToolAlias {
                tool: tool.into(),
                alias: alias.into(),
                version: version.into(),
            })
        })
        .collect()
}

/// A legacy asdf or vfox plugin (`mise plugins ls --urls`).
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Plugin {
    pub name: String,
    pub url: Option<String>,
    /// The Git ref and commit, when mise lists them.
    pub revision: Option<String>,
}

pub fn parse_plugins(output: &str) -> Vec<Plugin> {
    output
        .lines()
        .filter_map(|line| {
            let words: Vec<&str> = line.split_whitespace().collect();
            let (name, rest) = words.split_first()?;
            Some(Plugin {
                name: (*name).into(),
                url: rest.first().map(|url| (*url).into()),
                revision: (rest.len() > 1).then(|| rest[1..].join(" ")),
            })
        })
        .collect()
}
